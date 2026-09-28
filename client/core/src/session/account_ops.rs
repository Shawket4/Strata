//! Account intents of the core (`/me`, the export-only deletion flow, the unsynced-ops export
//! of a disabled or deleted account, §12.7, D25).

use std::fmt::Write as _;

use chrono_tz::Tz;

use super::{Core, online::zip_note_count};
use crate::error::{CoreError, CoreResult};
use crate::net::MeUpdate;
use crate::store::{account, cache, outbox};
use crate::view::Topics;
use crate::view::build;
use crate::view::model::{ExportSummary, SessionState};

impl Core {
    async fn update_me(&self, update: MeUpdate) -> CoreResult<()> {
        let session = self.session()?;
        let me = match self
            .env()
            .account_api
            .update_me(session.server_url()?, session.tokens(), update)
            .await
        {
            Ok(me) => me,
            Err(e) => {
                session.account_failure(&e)?;
                self.publish_state()?;
                return Err(e.into());
            }
        };
        session.write(|c, _| {
            account::update(c, |a| {
                a.display_name.clone_from(&me.display_name);
                a.timezone.clone_from(&me.timezone);
                a.ui_language.clone_from(&me.ui_language);
                a.password_change_required = me.password_change_required;
            })?;
            // Labels, dates and reminders depend on the zone and language.
            Ok(((), Topics::ALL))
        })?;
        self.publish_state()?;
        Ok(())
    }

    /// Changes the password (also leaves `PasswordChangeRequired`); other sessions end.
    pub async fn change_password(&self, current: &str, new: &str) -> CoreResult<SessionState> {
        if u32::try_from(new.chars().count()).unwrap_or(0) < super::MIN_PASSWORD_LENGTH {
            return Err(CoreError::invalid("new_password", "too_short"));
        }
        self.update_me(MeUpdate {
            current_password: Some(current.to_owned()),
            new_password: Some(new.to_owned()),
            ..MeUpdate::default()
        })
        .await?;
        self.state()
    }

    /// Sets the UI language (`en` | `ar`).
    pub async fn set_ui_language(&self, code: &str) -> CoreResult<()> {
        if !matches!(code, "en" | "ar") {
            return Err(CoreError::invalid("ui_language", "unknown"));
        }
        self.update_me(MeUpdate {
            ui_language: Some(code.to_owned()),
            ..MeUpdate::default()
        })
        .await
    }

    /// Sets the time zone (IANA name).
    pub async fn set_timezone(&self, iana: &str) -> CoreResult<()> {
        if iana.parse::<Tz>().is_err() {
            return Err(CoreError::invalid("timezone", "unknown"));
        }
        self.update_me(MeUpdate {
            timezone: Some(iana.to_owned()),
            ..MeUpdate::default()
        })
        .await
    }

    /// Sets the display name.
    pub async fn set_display_name(&self, name: &str) -> CoreResult<()> {
        if name.trim().is_empty() {
            return Err(CoreError::invalid("display_name", "empty"));
        }
        self.update_me(MeUpdate {
            display_name: Some(name.trim().to_owned()),
            ..MeUpdate::default()
        })
        .await
    }

    /// Downloads the account's own export (`GET /me/export`, also in the export-only
    /// session of a deletion-pending account) to `path`.
    pub async fn download_export(&self, path: &str) -> CoreResult<ExportSummary> {
        let session = self.session()?;
        let bytes = match self
            .env()
            .account_api
            .export_me(session.server_url()?, session.tokens())
            .await
        {
            Ok(b) => b,
            Err(e) => {
                session.account_failure(&e)?;
                return Err(e.into());
            }
        };
        if path.trim().is_empty() {
            return Err(CoreError::invalid("path", "empty"));
        }
        std::fs::write(path, &bytes)?;
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let notes = zip_note_count(&bytes);
        let labels = session.ctx().labels();
        session.write(|c, now| {
            cache::put(c, cache::EXPORT, &(size, notes), now)?;
            Ok(((), Topics::ACCOUNT))
        })?;
        self.publish_state()?;
        Ok(ExportSummary {
            path: path.to_owned(),
            size_bytes: size,
            note_count: notes,
            label: build::export_label(&labels, size, notes),
        })
    }

    /// "Delete now" (D25): confirms the deletion on the server, then removes the account's
    /// local data. Refused while ops are unsynced unless `force`.
    pub async fn delete_account_now(&self, force: bool) -> CoreResult<SessionState> {
        let session = self.session()?;
        let unsynced = session.unsynced()?;
        if unsynced > 0 && !force {
            return Err(CoreError::PendingChanges { count: unsynced });
        }
        self.env()
            .account_api
            .confirm_deletion(session.server_url()?, session.tokens())
            .await?;
        session.cancel_all_notifications()?;
        self.wipe(session)?;
        self.state()
    }
}

/// Writes the account's unsynced ops as a readable Markdown file (the "Export them first"
/// of a disabled or deleted account, §12.7): one section per op with the note it touches and,
/// for creates and edits, the full text.
pub fn export_unsynced(conn: &rusqlite::Connection, path: &str) -> CoreResult<u32> {
    use crate::sync::model::Op;
    if path.trim().is_empty() {
        return Err(CoreError::invalid("path", "empty"));
    }
    let ops = outbox::live(conn)?;
    let mut md = String::from("# Unsynced changes\n");
    for op in &ops {
        let title = match crate::store::write::LocalEntity::parse(&op.local_entity) {
            crate::store::write::LocalEntity::Note(id) => {
                crate::store::notes::current(conn, &id)?.map(|n| n.path)
            }
            _ => None,
        };
        let _ = write!(
            md,
            "\n## {} · {}\n\n",
            op.kind().as_str(),
            title.as_deref().unwrap_or(&op.entity_id)
        );
        let _ = writeln!(md, "- Queued: {}", op.created);
        let body = match &op.op {
            Op::NoteCreate(p) => Some(p.content.clone()),
            Op::NoteUpdate(p) => Some(p.content.clone()),
            Op::Capture(p) => Some(p.text.clone()),
            Op::TaskCreate(p) => Some(p.text.clone()),
            _ => None,
        };
        if let Some(body) = body {
            let fence = if body.contains("```") { "````" } else { "```" };
            let _ = write!(md, "\n{fence}markdown\n{}\n{fence}\n", body.trim_end());
        } else {
            let payload = rmp_serde::from_slice::<rmpv::Value>(&op.op.payload_bytes()?)
                .map(|v: rmpv::Value| v.to_string())
                .unwrap_or_default();
            let _ = writeln!(md, "- Details: `{payload}`");
        }
    }
    std::fs::write(path, md)?;
    Ok(u32::try_from(ops.len()).unwrap_or(u32::MAX))
}
