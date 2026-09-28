//! Online-only intents of a signed-in session (PLAN §12.6): devices, history and revert,
//! server search, AI status, integrity, vault export/import and Admin → Users. Results that
//! screens show later are cached in `remote_cache` (so they still render offline, marked by
//! their fetch time); everything else is returned directly.

use std::io::Read as _;

use super::Session;
use crate::error::{CoreError, CoreResult};
use crate::net::{AdminUpdate, AdminUserInfo, NetError};
use crate::store::cache;
use crate::view::build;
use crate::view::model::{
    AdminUserItem, AdminUsersView, Availability, ExportSummary, ImportSummary, NewUserRequest,
    NoteDiffView, SearchMode, SearchView,
};
use crate::view::{Topics, ViewCtx};

/// Notes (`.md` entries outside `.trash/`) in an export zip.
pub fn zip_note_count(bytes: &[u8]) -> u32 {
    let Ok(mut archive) =
        zip::ZipArchive::<std::io::Cursor<&[u8]>>::new(std::io::Cursor::new(bytes))
    else {
        return 0;
    };
    let mut n = 0u32;
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            let name = f.name();
            let md = std::path::Path::new(name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if md && !name.starts_with(".trash/") {
                n += 1;
            }
        }
    }
    n
}

fn write_file(path: &str, bytes: &[u8]) -> CoreResult<()> {
    if path.trim().is_empty() {
        return Err(CoreError::invalid("path", "empty"));
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

impl Session {
    fn online<T>(&self, r: Result<T, NetError>) -> CoreResult<T> {
        match r {
            Ok(v) => Ok(v),
            Err(e) => {
                self.account_failure(&e)?;
                Err(e.into())
            }
        }
    }

    fn cache_put<T: serde::Serialize>(&self, key: &str, value: &T) -> CoreResult<()> {
        self.write(|c, now| {
            cache::put(c, key, value, now)?;
            Ok(((), Topics::REMOTE | Topics::SETTINGS | Topics::ACCOUNT))
        })
    }

    /// Re-reads the devices, AI status, integrity warnings and (admins) the approval queue.
    /// Offline → `offline`; each part that answers is cached.
    pub async fn refresh_settings(&self) -> CoreResult<()> {
        let url = self.server_url();
        let api = self.env.account_api.clone();
        let devices = self.online(api.devices(url.clone(), self.tokens()).await)?;
        self.cache_put(cache::DEVICES, &devices)?;
        if let Ok(s) = api.ai_status(url.clone(), self.tokens()).await {
            self.cache_put(cache::AI_STATUS, &s)?;
        }
        if let Ok(w) = api.integrity(url.clone(), self.tokens()).await {
            self.cache_put(cache::INTEGRITY, &w)?;
        }
        let is_admin =
            self.read(|c, _| Ok(build::account_summary(c, "")?.is_some_and(|a| a.is_admin)))?;
        if is_admin && let Ok(users) = api.admin_users(url, self.tokens()).await {
            let pending = users.iter().filter(|u| u.status == "pending").count();
            self.cache_put(cache::ADMIN_PENDING, &u32::try_from(pending).unwrap_or(0))?;
        }
        Ok(())
    }

    /// Renames a device.
    pub async fn rename_device(&self, device_id: &str, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::invalid("name", "empty"));
        }
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .rename_device(url, self.tokens(), device_id.to_owned(), name.to_owned())
                .await,
        )?;
        self.refresh_settings().await
    }

    /// Signs another device out (this device signs out with `sign_out`).
    pub async fn revoke_device(&self, device_id: &str) -> CoreResult<()> {
        if self
            .read(|c, _| crate::sync::apply::device_id(c))?
            .as_deref()
            == Some(device_id)
        {
            return Err(CoreError::invalid("device", "this_device"));
        }
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .revoke_device(url, self.tokens(), device_id.to_owned())
                .await,
        )?;
        self.refresh_settings().await
    }

    /// Reminders on/off for any device ("Deliver to"). This device goes through the outbox
    /// (`set_reminders_enabled`); other devices are changed on the server directly.
    pub async fn set_device_reminders(&self, device_id: &str, enabled: bool) -> CoreResult<()> {
        if self
            .read(|c, _| crate::sync::apply::device_id(c))?
            .as_deref()
            == Some(device_id)
        {
            return self.set_reminders_enabled(enabled);
        }
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .set_device_reminders(url, self.tokens(), device_id.to_owned(), enabled)
                .await,
        )?;
        self.refresh_settings().await
    }

    /// Fetches a note's history (shown in the note's history panel and "v7").
    pub async fn refresh_history(&self, note_id: &str) -> CoreResult<()> {
        let url = self.server_url();
        let revisions = self.online(
            self.env
                .account_api
                .note_history(url, self.tokens(), note_id.to_owned())
                .await,
        )?;
        self.write(|c, now| {
            cache::put(c, &cache::history_key(note_id), &revisions, now)?;
            Ok(((), Topics::REMOTE | Topics::NOTES))
        })
    }

    /// A revision compared with the note's current content.
    pub async fn note_revision_diff(
        &self,
        note_id: &str,
        commit: &str,
    ) -> CoreResult<NoteDiffView> {
        let url = self.server_url();
        let rev = self.online(
            self.env
                .account_api
                .note_revision(url, self.tokens(), note_id.to_owned(), commit.to_owned())
                .await,
        )?;
        self.read(|c, ctx| {
            let current = crate::store::notes::current(c, note_id)?
                .map(|n| n.content)
                .unwrap_or_default();
            Ok(NoteDiffView {
                note_id: note_id.to_owned(),
                commit: rev.commit.clone(),
                summary: crate::format::diff::summary(&rev.content, &current, ctx.lang),
                lines: crate::format::diff::line_diff(&rev.content, &current),
            })
        })
    }

    /// Reverts a note to a revision (online; the change arrives by the next pull, which
    /// rebases any queued local edits onto it).
    pub async fn revert_note(&self, note_id: &str, commit: &str) -> CoreResult<()> {
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .revert_note(url, self.tokens(), note_id.to_owned(), commit.to_owned())
                .await,
        )?;
        self.refresh_history(note_id).await
    }

    /// Semantic / hybrid search on the server (keyword search stays local).
    pub async fn search_remote(
        &self,
        query: &str,
        mode: SearchMode,
        folder: Option<String>,
    ) -> CoreResult<SearchView> {
        let ctx = self.ctx();
        let mode_str = match mode {
            SearchMode::Keyword => "keyword",
            SearchMode::Semantic => "semantic",
            SearchMode::Hybrid => "hybrid",
        };
        let empty = |availability| SearchView {
            query: query.to_owned(),
            mode,
            results: Vec::new(),
            availability,
            available_modes: crate::search::available_modes(&ctx),
            folder: folder.clone(),
        };
        if query.trim().is_empty() {
            return Ok(empty(Availability::Available));
        }
        let url = self.server_url();
        let hits = match self
            .env
            .account_api
            .search(
                url,
                self.tokens(),
                query.to_owned(),
                mode_str.to_owned(),
                50,
            )
            .await
        {
            Ok(h) => h,
            Err(e) if e.is_offline() => return Ok(empty(Availability::Offline)),
            Err(NetError::Api { status: 503, .. } | NetError::NotAvailable { .. }) => {
                return Ok(empty(Availability::NotYetAvailable));
            }
            Err(e) => {
                return Err(self
                    .online::<()>(Err(e))
                    .err()
                    .unwrap_or(CoreError::Offline));
            }
        };
        self.read(|c, ctx| crate::search::remote_view(c, ctx, query, mode, folder.clone(), hits))
    }

    /// Downloads the vault export (`GET /export`) to `path`.
    pub async fn export_vault(&self, path: &str) -> CoreResult<ExportSummary> {
        let url = self.server_url();
        let bytes = self.online(self.env.account_api.export_vault(url, self.tokens()).await)?;
        write_file(path, &bytes)?;
        let labels = self.ctx().labels();
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let notes = zip_note_count(&bytes);
        Ok(ExportSummary {
            path: path.to_owned(),
            size_bytes: size,
            note_count: notes,
            label: build::export_label(&labels, size, notes),
        })
    }

    /// Imports a zip archive (`POST /import`); the imported notes arrive by pull.
    pub async fn import_vault(&self, path: &str) -> CoreResult<ImportSummary> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut bytes)?;
        let url = self.server_url();
        let (imported, skipped) = self.online(
            self.env
                .account_api
                .import_vault(url, self.tokens(), bytes)
                .await,
        )?;
        Ok(ImportSummary { imported, skipped })
    }

    fn admin_ctx(&self) -> CoreResult<(ViewCtx, String)> {
        let (is_admin, me) = self.read(|c, _| {
            let a = build::account_summary(c, "")?;
            Ok((
                a.as_ref().is_some_and(|a| a.is_admin),
                a.map(|a| a.user_id).unwrap_or_default(),
            ))
        })?;
        if !is_admin {
            return Err(CoreError::invalid("account", "not_admin"));
        }
        Ok((self.ctx(), me))
    }

    fn admin_item(&self, u: AdminUserInfo) -> CoreResult<AdminUserItem> {
        let (ctx, me) = self.admin_ctx()?;
        Ok(build::admin_user_item(&ctx, &me, u))
    }

    /// Admin → Users, filtered by `query` (username or display name, normalised).
    pub async fn admin_users(&self, query: &str) -> CoreResult<AdminUsersView> {
        let Ok((ctx, me)) = self.admin_ctx() else {
            return Ok(AdminUsersView {
                availability: Availability::NotAllowed,
                pending: Vec::new(),
                users: Vec::new(),
                query: query.to_owned(),
                deletion_preview_label: None,
            });
        };
        let url = self.server_url();
        match self.env.account_api.admin_users(url, self.tokens()).await {
            Ok(users) => {
                let pending = users.iter().filter(|u| u.status == "pending").count();
                self.cache_put(cache::ADMIN_PENDING, &u32::try_from(pending).unwrap_or(0))?;
                let mut view = build::admin_users(&ctx, &me, users, query);
                // An older server without `GET /admin/settings` keeps the generic wording.
                view.deletion_preview_label = self
                    .env
                    .account_api
                    .admin_settings(self.server_url(), self.tokens())
                    .await
                    .ok()
                    .map(|grace| build::deletion_preview_label(&ctx, grace));
                Ok(view)
            }
            Err(e) if e.is_offline() => Ok(AdminUsersView {
                availability: Availability::Offline,
                pending: Vec::new(),
                users: Vec::new(),
                query: query.to_owned(),
                deletion_preview_label: None,
            }),
            Err(e) => {
                self.account_failure(&e)?;
                Err(e.into())
            }
        }
    }

    /// Approves a sign-up.
    pub async fn approve_user(&self, id: &str) -> CoreResult<AdminUserItem> {
        self.admin_ctx()?;
        let u = self.online(
            self.env
                .account_api
                .admin_approve(self.server_url(), self.tokens(), id.to_owned())
                .await,
        )?;
        self.admin_item(u)
    }

    /// Rejects a sign-up.
    pub async fn reject_user(&self, id: &str) -> CoreResult<AdminUserItem> {
        self.admin_ctx()?;
        let u = self.online(
            self.env
                .account_api
                .admin_reject(self.server_url(), self.tokens(), id.to_owned())
                .await,
        )?;
        self.admin_item(u)
    }

    async fn admin_patch(
        &self,
        id: &str,
        update: AdminUpdate,
    ) -> CoreResult<(AdminUserItem, Option<String>)> {
        let (_, me) = self.admin_ctx()?;
        if me == id {
            return Err(CoreError::invalid("user", "self"));
        }
        let (u, password) = self.online(
            self.env
                .account_api
                .admin_update(self.server_url(), self.tokens(), id.to_owned(), update)
                .await,
        )?;
        Ok((self.admin_item(u)?, password))
    }

    /// Changes a user's role (`admin` | `member`).
    pub async fn set_user_role(&self, id: &str, role: &str) -> CoreResult<AdminUserItem> {
        if !matches!(role, "admin" | "member") {
            return Err(CoreError::invalid("role", "unknown"));
        }
        self.admin_patch(
            id,
            AdminUpdate {
                role: Some(role.to_owned()),
                ..AdminUpdate::default()
            },
        )
        .await
        .map(|(u, _)| u)
    }

    /// Disables (`false`) or enables (`true`) an account.
    pub async fn set_user_enabled(&self, id: &str, enabled: bool) -> CoreResult<AdminUserItem> {
        self.admin_patch(
            id,
            AdminUpdate {
                status: Some(if enabled { "active" } else { "disabled" }.to_owned()),
                ..AdminUpdate::default()
            },
        )
        .await
        .map(|(u, _)| u)
    }

    /// Resets a password: the one-time temporary password (shown once).
    pub async fn reset_password(&self, id: &str) -> CoreResult<String> {
        let (_, password) = self
            .admin_patch(
                id,
                AdminUpdate {
                    reset_password: true,
                    ..AdminUpdate::default()
                },
            )
            .await?;
        password.ok_or_else(|| CoreError::Internal("reset without a temporary password".into()))
    }

    /// Schedules an account's deletion (D25): the account with its `deletion_label`.
    pub async fn schedule_deletion(&self, id: &str) -> CoreResult<AdminUserItem> {
        let (_, me) = self.admin_ctx()?;
        if me == id {
            return Err(CoreError::invalid("user", "self"));
        }
        let u = self.online(
            self.env
                .account_api
                .admin_schedule_deletion(self.server_url(), self.tokens(), id.to_owned())
                .await,
        )?;
        self.admin_item(u)
    }

    /// Cancels a scheduled deletion.
    pub async fn cancel_deletion(&self, id: &str) -> CoreResult<AdminUserItem> {
        self.admin_ctx()?;
        let u = self.online(
            self.env
                .account_api
                .admin_cancel_deletion(self.server_url(), self.tokens(), id.to_owned())
                .await,
        )?;
        self.admin_item(u)
    }

    /// Creates an active account.
    pub async fn create_user(&self, r: NewUserRequest) -> CoreResult<AdminUserItem> {
        self.admin_ctx()?;
        if !matches!(r.role.as_str(), "admin" | "member") {
            return Err(CoreError::invalid("role", "unknown"));
        }
        if u32::try_from(r.password.chars().count()).unwrap_or(0) < super::MIN_PASSWORD_LENGTH {
            return Err(CoreError::invalid("password", "too_short"));
        }
        let u = self.online(
            self.env
                .account_api
                .admin_create(self.server_url(), self.tokens(), r)
                .await,
        )?;
        self.admin_item(u)
    }

    /// Re-reads the AI activity feed (the last 50 AI decisions).
    pub async fn refresh_ai_activity(&self) -> CoreResult<()> {
        let url = self.server_url();
        let d = self.online(
            self.env
                .account_api
                .ai_decisions(url, self.tokens(), 50)
                .await,
        )?;
        self.write(|c, now| {
            cache::put(c, cache::AI_DECISIONS, &d, now)?;
            Ok(((), Topics::REMOTE | Topics::NOTES | Topics::ENTITIES))
        })
    }

    /// Undoes (rejects) an AI decision (D13): the change is reverted by the server and the
    /// link never re-proposed; the result arrives by pull.
    pub async fn reject_ai_decision(&self, id: &str) -> CoreResult<()> {
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .reject_ai_decision(url, self.tokens(), id.to_owned())
                .await,
        )?;
        self.refresh_ai_activity().await
    }

    /// Points an AI decision at another entity ("this Ahmed is Ahmed Fathy", D13).
    pub async fn repoint_ai_decision(
        &self,
        id: &str,
        target_id: &str,
        hint: Option<String>,
    ) -> CoreResult<()> {
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .repoint_ai_decision(
                    url,
                    self.tokens(),
                    id.to_owned(),
                    target_id.to_owned(),
                    hint,
                )
                .await,
        )?;
        self.refresh_ai_activity().await
    }

    /// Changes the type of an AI relation.
    pub async fn retype_ai_decision(&self, id: &str, rel_type: &str) -> CoreResult<()> {
        if rel_type.parse::<vault_format::RelationKey>().is_err() {
            return Err(CoreError::invalid("rel_type", "unknown_relation"));
        }
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .retype_ai_decision(url, self.tokens(), id.to_owned(), rel_type.to_owned())
                .await,
        )?;
        self.refresh_ai_activity().await
    }

    /// Fetches the similarity edges of the global map (computed on the server, never stored).
    pub async fn refresh_similarity(&self) -> CoreResult<()> {
        let url = self.server_url();
        let edges = self.online(
            self.env
                .account_api
                .similarity_edges(url, self.tokens())
                .await,
        )?;
        self.write(|c, now| {
            cache::put(c, cache::SIMILARITY, &edges, now)?;
            Ok(((), Topics::REMOTE | Topics::NOTES))
        })
    }

    /// Saves a mind-map layout as `maps/<name>.canvas` (JSON Canvas, PLAN §6.8): one file
    /// node per placed note and the local graph's edges between them. Returns the map's path.
    pub async fn save_layout(
        &self,
        center_id: &str,
        name: &str,
        positions: &[crate::view::model::NodePosition],
    ) -> CoreResult<String> {
        // Checked before sanitising: the sanitiser turns a blank name into `Untitled`, which
        // would silently replace whatever map already has that name.
        if name.trim().is_empty() {
            return Err(CoreError::invalid("name", "empty"));
        }
        let name = vault_format::filename::sanitize_file_name(name.trim());
        let content =
            self.read(|c, ctx| crate::graph::layout_canvas(c, ctx, center_id, positions))?;
        let url = self.server_url();
        self.online(
            self.env
                .account_api
                .put_map(url, self.tokens(), name, content)
                .await,
        )
    }
}
