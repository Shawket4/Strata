//! The trait implementations over the generated `strata-client`.
//!
//! Sync bodies: the server encodes and decodes the shared `sync-model` types directly and the
//! contract's `Sync*` schemas mirror them field for field (a server test proves the bytes
//! round-trip). The device therefore converts between the generated types and `sync-model`
//! by re-encoding as named-map `MessagePack` ([`convert`]) — no handwritten wire code (L13).

use std::fmt;

use futures::future::BoxFuture;
use serde::Serialize;
use serde::de::DeserializeOwned;
use strata_client::streaming::{StreamEvent, StreamOptions, Subscription};
use strata_client::{Client, operations, streams, types};

use crate::net::{
    AccountApi, AdminUpdate, AdminUserInfo, AiDecisionInfo, AiStatusInfo, AskEvent, AskStream,
    DeviceInfo, EventSignal, EventStream, EventsApi, IntegrityInfo, MeInfo, MeUpdate, NetError,
    RemoteHit, RevisionContent, RevisionInfo, SessionTokens, SimilarityEdge, SyncApi, Tokens,
    classify,
};
use crate::sync::model::{BootstrapPage, ChangesPage, OpOutcome, SyncOp};
use crate::view::model::{NewUserRequest, Platform};

fn client(server_url: &str, tokens: Option<Tokens>) -> Result<Client, NetError> {
    let mut b = Client::builder(server_url).timeout(std::time::Duration::from_secs(30));
    if let Some(t) = tokens {
        b = b.tokens(t);
    }
    b.build().map_err(|e| classify(&e))
}

fn ulid(id: &str, what: &str) -> Result<ulid::Ulid, NetError> {
    ulid::Ulid::from_string(id).map_err(|e| NetError::Protocol(format!("{what} id: {e}")))
}

/// Converts between two serde types with the same `MessagePack` shape (generated contract type
/// ↔ `sync-model` type).
pub fn convert<A: Serialize, B: DeserializeOwned>(a: &A) -> Result<B, NetError> {
    let bytes = rmp_serde::to_vec_named(a).map_err(|e| NetError::Protocol(e.to_string()))?;
    rmp_serde::from_slice(&bytes).map_err(|e| NetError::Protocol(e.to_string()))
}

fn platform(p: Platform) -> types::DevicePlatform {
    match p {
        Platform::Android => types::DevicePlatform::Android,
        Platform::Ios => types::DevicePlatform::Ios,
        Platform::Macos => types::DevicePlatform::Macos,
        Platform::Windows => types::DevicePlatform::Windows,
        Platform::Linux => types::DevicePlatform::Linux,
    }
}

fn session(s: types::AuthSession) -> SessionTokens {
    SessionTokens {
        user_id: s.user_id.to_string(),
        device_id: s.device_id.to_string(),
        session_id: s.session_id.to_string(),
        access_token: s.access_token,
        access_expires_at: s.access_token_expires_at,
        refresh_token: s.refresh_token,
        refresh_expires_at: s.refresh_token_expires_at,
        export_only: s.export_only,
        password_change_required: s.password_change_required,
    }
}

fn me_info(m: types::Me) -> MeInfo {
    MeInfo {
        id: m.id.to_string(),
        username: m.username,
        display_name: m.display_name,
        role: m.role.to_string(),
        status: m.status.to_string(),
        timezone: m.timezone,
        ui_language: m.ui_language.to_string(),
        deletion_at: m.deletion_at,
        password_change_required: m.password_change_required,
    }
}

fn device(d: types::Device) -> DeviceInfo {
    DeviceInfo {
        id: d.id.to_string(),
        name: d.name,
        platform: d.platform.to_string(),
        created: d.created,
        last_seen: d.last_seen,
        current: d.current,
        reminders_enabled: d.reminders_enabled,
    }
}

fn admin_user(u: types::AdminUser) -> AdminUserInfo {
    AdminUserInfo {
        id: u.id.to_string(),
        username: u.username,
        display_name: u.display_name,
        role: u.role.to_string(),
        status: u.status.to_string(),
        created: u.created,
        deletion_at: u.deletion_at,
        export_downloaded_at: u.export_downloaded_at,
        password_change_required: u.password_change_required,
    }
}

fn role(r: &str) -> Result<types::Role, NetError> {
    match r {
        "admin" => Ok(types::Role::Admin),
        "member" => Ok(types::Role::Member),
        other => Err(NetError::Protocol(format!("role {other}"))),
    }
}

/// [`AccountApi`] over the generated client.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientAccountApi {}

macro_rules! call {
    ($url:expr, $tokens:expr, |$c:ident| $body:expr) => {{
        let url = $url;
        let tokens = $tokens;
        Box::pin(async move {
            let $c = client(&url, Some(tokens))?;
            $body.await.map_err(|e| classify(&e))
        })
    }};
}

impl AccountApi for ClientAccountApi {
    fn signup(
        &self,
        server_url: String,
        username: String,
        password: String,
        display_name: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            let body = types::SignupRequest {
                display_name,
                password,
                username,
            };
            let r = operations::signup(&c, &body)
                .await
                .map_err(|e| classify(&e))?;
            Ok(r.username)
        })
    }

    fn login(
        &self,
        server_url: String,
        username: String,
        password: String,
        device_name: String,
        p: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            let body = types::LoginRequest {
                device_name,
                password,
                platform: platform(p),
                username,
            };
            operations::login(&c, &body)
                .await
                .map(session)
                .map_err(|e| classify(&e))
        })
    }

    fn refresh(
        &self,
        server_url: String,
        refresh_token: String,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            operations::refresh(&c, &types::RefreshRequest { refresh_token })
                .await
                .map(session)
                .map_err(|e| classify(&e))
        })
    }

    fn logout(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<(), NetError>> {
        call!(server_url, tokens, |c| operations::logout(&c))
    }

    fn me(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::get_me(&c)
                .await
                .map(me_info)
                .map_err(|e| classify(&e))
        })
    }

    fn set_device_reminders(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        enabled: bool,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let body = types::UpdateDevice {
                name: None,
                reminders_enabled: Some(enabled),
            };
            operations::update_device(&c, ulid(&device_id, "device")?, &body)
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
    }

    fn admin_users(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<AdminUserInfo>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let users = operations::admin_list_users(&c, None)
                .await
                .map_err(|e| classify(&e))?;
            Ok(users.into_iter().map(admin_user).collect())
        })
    }

    fn admin_settings(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<i64, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::admin_settings(&c)
                .await
                .map(|s| s.deletion_grace_secs)
                .map_err(|e| classify(&e))
        })
    }

    fn update_me(
        &self,
        server_url: String,
        tokens: Tokens,
        u: MeUpdate,
    ) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let ui_language = match u.ui_language.as_deref() {
                None => None,
                Some("ar") => Some(types::UiLanguage::Ar),
                Some("en") => Some(types::UiLanguage::En),
                Some(other) => return Err(NetError::Protocol(format!("language {other}"))),
            };
            let body = types::UpdateMe {
                current_password: u.current_password,
                display_name: u.display_name,
                new_password: u.new_password,
                preferences: u.preferences.map(|p| p.into_iter().collect()),
                timezone: u.timezone,
                ui_language,
            };
            operations::update_me(&c, &body)
                .await
                .map(me_info)
                .map_err(|e| classify(&e))
        })
    }

    fn devices(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<DeviceInfo>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let list = operations::list_devices(&c)
                .await
                .map_err(|e| classify(&e))?;
            Ok(list.into_iter().map(device).collect())
        })
    }

    fn rename_device(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        name: String,
    ) -> BoxFuture<'_, Result<DeviceInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let body = types::UpdateDevice {
                name: Some(name),
                reminders_enabled: None,
            };
            operations::update_device(&c, ulid(&device_id, "device")?, &body)
                .await
                .map(device)
                .map_err(|e| classify(&e))
        })
    }

    fn revoke_device(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::delete_device(&c, ulid(&device_id, "device")?)
                .await
                .map_err(|e| classify(&e))
        })
    }

    fn admin_approve(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::admin_approve_user(&c, ulid(&user_id, "user")?)
                .await
                .map(admin_user)
                .map_err(|e| classify(&e))
        })
    }

    fn admin_reject(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::admin_reject_user(&c, ulid(&user_id, "user")?)
                .await
                .map(admin_user)
                .map_err(|e| classify(&e))
        })
    }

    fn admin_update(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
        u: AdminUpdate,
    ) -> BoxFuture<'_, Result<(AdminUserInfo, Option<String>), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let status = match u.status.as_deref() {
                None => None,
                Some("active") => Some(types::SettableStatus::Active),
                Some("disabled") => Some(types::SettableStatus::Disabled),
                Some(other) => return Err(NetError::Protocol(format!("status {other}"))),
            };
            let body = types::UpdateUser {
                reset_password: u.reset_password.then_some(true),
                role: u.role.as_deref().map(role).transpose()?,
                status,
            };
            let r = operations::admin_update_user(&c, ulid(&user_id, "user")?, &body)
                .await
                .map_err(|e| classify(&e))?;
            Ok((admin_user(r.user), r.temporary_password))
        })
    }

    fn admin_schedule_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::admin_delete_user(&c, ulid(&user_id, "user")?)
                .await
                .map(admin_user)
                .map_err(|e| classify(&e))
        })
    }

    fn admin_cancel_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::admin_cancel_deletion(&c, ulid(&user_id, "user")?)
                .await
                .map(admin_user)
                .map_err(|e| classify(&e))
        })
    }

    fn admin_create(
        &self,
        server_url: String,
        tokens: Tokens,
        r: NewUserRequest,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let body = types::CreateUser {
                display_name: r.display_name,
                password: r.password,
                role: role(&r.role)?,
                username: r.username,
            };
            operations::admin_create_user(&c, &body)
                .await
                .map(admin_user)
                .map_err(|e| classify(&e))
        })
    }

    fn export_me(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<u8>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::export_me(&c)
                .await
                .map(|b| b.to_vec())
                .map_err(|e| classify(&e))
        })
    }

    fn confirm_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        call!(server_url, tokens, |c| operations::confirm_deletion(&c))
    }

    fn export_vault(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<u8>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::export_vault(&c)
                .await
                .map(|b| b.to_vec())
                .map_err(|e| classify(&e))
        })
    }

    fn import_vault(
        &self,
        server_url: String,
        tokens: Tokens,
        zip: Vec<u8>,
    ) -> BoxFuture<'_, Result<(u32, u32), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let r = operations::import_vault(&c, zip.into())
                .await
                .map_err(|e| classify(&e))?;
            let n = |v: &Vec<String>| u32::try_from(v.len()).unwrap_or(u32::MAX);
            Ok((n(&r.imported), n(&r.skipped)))
        })
    }

    fn note_history(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
    ) -> BoxFuture<'_, Result<Vec<RevisionInfo>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let h = operations::get_note_history(&c, ulid(&note_id, "note")?)
                .await
                .map_err(|e| classify(&e))?;
            Ok(h.revisions
                .into_iter()
                .map(|r| RevisionInfo {
                    commit: r.commit,
                    at: r.at,
                    author: r.author.to_string(),
                    message: r.message,
                    path: r.path,
                    change: r.change.to_string(),
                })
                .collect())
        })
    }

    fn note_revision(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
        commit: String,
    ) -> BoxFuture<'_, Result<RevisionContent, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let r = operations::get_note_revision(&c, ulid(&note_id, "note")?, &commit)
                .await
                .map_err(|e| classify(&e))?;
            Ok(RevisionContent {
                commit: r.commit,
                content: r.content,
                path: r.path,
            })
        })
    }

    fn revert_note(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
        commit: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::revert_note(
                &c,
                ulid(&note_id, "note")?,
                &types::RevertNoteRequest { commit },
            )
            .await
            .map(|_| ())
            .map_err(|e| classify(&e))
        })
    }

    fn search(
        &self,
        server_url: String,
        tokens: Tokens,
        query: String,
        mode: String,
        limit: u32,
    ) -> BoxFuture<'_, Result<Vec<RemoteHit>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let mode = match mode.as_str() {
                "semantic" => types::SearchMode::Semantic,
                "hybrid" => types::SearchMode::Hybrid,
                _ => types::SearchMode::Keyword,
            };
            let r = operations::search(&c, &query, Some(&mode), Some(u64::from(limit)))
                .await
                .map_err(|e| classify(&e))?;
            Ok(r.hits
                .into_iter()
                .map(|h| RemoteHit {
                    id: h.id.to_string(),
                    title: h.title,
                    path: h.path,
                    kind: h.kind.to_string(),
                    snippet: h.snippet,
                    score: h.score,
                })
                .collect())
        })
    }

    fn ai_status(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<AiStatusInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let s = operations::ai_status(&c).await.map_err(|e| classify(&e))?;
            Ok(AiStatusInfo {
                enabled: s.enabled,
                provider: s.provider.map(|p| p.name),
                paused_until: s.paused.as_ref().and_then(|p| p.until),
                paused_reason: s.paused.map(|p| p.reason),
                queue_depth: s.queue_depth,
                failed_jobs: s.failed_jobs,
                tokens_used: s.usage.input_tokens + s.usage.output_tokens,
                tokens_limit: s.limits.per_user_daily_tokens,
                embedded: s.embeddings.map(|e| (e.embedded, e.total)),
            })
        })
    }

    fn retry_failed_jobs(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<u64, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let r = operations::retry_failed_jobs(&c)
                .await
                .map_err(|e| classify(&e))?;
            Ok(r.requeued)
        })
    }

    fn integrity(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<IntegrityInfo>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let i = operations::get_integrity(&c)
                .await
                .map_err(|e| classify(&e))?;
            Ok(i.warnings
                .into_iter()
                .map(|w| IntegrityInfo {
                    id: w.id.to_string(),
                    kind: w.kind,
                    path: w.path,
                    created: w.created,
                })
                .collect())
        })
    }

    fn ask(
        &self,
        server_url: String,
        tokens: Tokens,
        question: String,
        scope: Option<String>,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::ask(&c, &types::AskRequest { question, scope })
                .await
                .map(|s| s.id.to_string())
                .map_err(|e| classify(&e))
        })
    }

    fn ask_stream(
        &self,
        server_url: String,
        tokens: Tokens,
        ask_id: String,
    ) -> Result<Box<dyn AskStream>, NetError> {
        let c = client(&server_url, Some(tokens))?;
        let sub = streams::ask_stream(
            &c,
            ulid(&ask_id, "ask")?,
            StreamOptions {
                max_reconnect_attempts: Some(3),
                ..StreamOptions::default()
            },
        )
        .map_err(|e| classify(&e))?;
        Ok(Box::new(ClientAskStream { sub }))
    }

    fn ai_decisions(
        &self,
        server_url: String,
        tokens: Tokens,
        limit: u32,
    ) -> BoxFuture<'_, Result<Vec<AiDecisionInfo>, NetError>> {
        Box::pin(Self::decisions(server_url, tokens, limit))
    }

    fn reject_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::reject_ai_decision(&c, ulid(&id, "decision")?)
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
    }

    fn repoint_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        target_id: String,
        hint: Option<String>,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let body = types::RepointRequest {
                hint,
                target_id: ulid(&target_id, "target")?,
            };
            operations::repoint_ai_decision(&c, ulid(&id, "decision")?, &body)
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
    }

    fn retype_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        rel_type: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let body = types::RetypeRequest { type_: rel_type };
            operations::retype_ai_decision(&c, ulid(&id, "decision")?, &body)
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
    }

    fn similarity_edges(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<SimilarityEdge>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let g = operations::get_graph(&c, Some("similarity"), None, Some(true), None, None)
                .await
                .map_err(|e| classify(&e))?;
            Ok(g.edges
                .into_iter()
                .filter(|e| e.kind == "similarity")
                .map(|e| SimilarityEdge {
                    src: e.source.clone(),
                    dst: e.target.clone(),
                    score: e.weight.or(e.confidence),
                })
                .collect())
        })
    }

    fn put_map(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        content: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let current = match operations::get_map(&c, &id).await {
                Ok(m) => Some(m.version),
                Err(e) => match classify(&e) {
                    NetError::Api { status: 404, .. } => None,
                    other => return Err(other),
                },
            };
            operations::put_map(
                &c,
                &id,
                current.as_deref(),
                &types::PutMapRequest { content },
            )
            .await
            .map(|m| m.path)
            .map_err(|e| classify(&e))
        })
    }

    fn save_ask(
        &self,
        server_url: String,
        tokens: Tokens,
        ask_id: String,
        title: Option<String>,
        created: chrono::DateTime<chrono::Utc>,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::save_ask(
                &c,
                ulid(&ask_id, "ask")?,
                &types::SaveAskRequest {
                    created,
                    force: Some(true),
                    title,
                },
            )
            .await
            .map(|n| n.id.to_string())
            .map_err(|e| classify(&e))
        })
    }
}

fn decision(d: types::AiDecision) -> AiDecisionInfo {
    AiDecisionInfo {
        id: d.id.to_string(),
        kind: d.kind,
        rel_type: d.type_,
        summary: d.summary,
        source_note_id: d.source_note_id.map(|n| n.to_string()),
        source_title: d.source_title,
        target_id: d.target_id,
        target_name: d.target_name,
        confidence: d.confidence.map(f64::from),
        created: d.created,
        reverted_at: d.reverted_at,
        suggestion_id: d.suggestion_id.map(|s| s.to_string()),
    }
}

impl ClientAccountApi {
    /// `GET /ai-decisions`.
    async fn decisions(
        url: String,
        tokens: Tokens,
        limit: u32,
    ) -> Result<Vec<AiDecisionInfo>, NetError> {
        let c = client(&url, Some(tokens))?;
        let r = operations::list_ai_decisions(&c, Some(i64::from(limit)))
            .await
            .map_err(|e| classify(&e))?;
        Ok(r.items.into_iter().map(decision).collect())
    }
}

struct ClientAskStream {
    sub: Subscription<types::AskFrame>,
}

impl AskStream for ClientAskStream {
    fn next(&mut self) -> BoxFuture<'_, Option<Result<AskEvent, NetError>>> {
        Box::pin(async move {
            loop {
                match self.sub.next().await? {
                    Err(e) => return Some(Err(classify(&e))),
                    // A resumed answer stream never resets (the answer is kept 30 min).
                    Ok(StreamEvent::Reset { .. }) => {}
                    Ok(StreamEvent::Data { payload, .. }) => {
                        return Some(Ok(match payload {
                            types::AskFrame::Tokens { text } => AskEvent::Tokens(text),
                            types::AskFrame::Citation {
                                block_id,
                                index,
                                note_id,
                                path,
                                ref_: _,
                                target,
                                title,
                            } => AskEvent::Citation {
                                index: index.get(),
                                note_id: note_id.to_string(),
                                path,
                                title,
                                block_id,
                                target,
                            },
                            types::AskFrame::Done { answer } => AskEvent::Done(answer),
                        }));
                    }
                }
            }
        })
    }
}

/// [`SyncApi`] over the generated client.
#[derive(Clone)]
pub struct ClientSyncApi {
    client: Client,
}

impl fmt::Debug for ClientSyncApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientSyncApi")
            .field("base_url", &self.client.base_url())
            .finish()
    }
}

impl ClientSyncApi {
    /// A sync API for `server_url` authenticated by `tokens`.
    pub fn new(server_url: &str, tokens: Tokens) -> Result<Self, NetError> {
        Ok(Self {
            client: client(server_url, Some(tokens))?,
        })
    }
}

impl SyncApi for ClientSyncApi {
    fn bootstrap(&self, cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        Box::pin(async move {
            let page = operations::sync_bootstrap(&self.client, cursor.as_deref(), None)
                .await
                .map_err(|e| classify(&e))?;
            convert(&page)
        })
    }

    fn changes(
        &self,
        since: u64,
        epoch: u64,
        limit: u32,
    ) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        Box::pin(async move {
            let page = operations::sync_changes(&self.client, since, epoch, Some(u64::from(limit)))
                .await
                .map_err(|e| classify(&e))?;
            convert(&page)
        })
    }

    fn push(&self, ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        Box::pin(async move {
            let request = sync_model::PushRequest { ops };
            let body: types::SyncPushRequest = convert(&request)?;
            let response = operations::sync_push(&self.client, &body)
                .await
                .map_err(|e| classify(&e))?;
            let response: sync_model::PushResponse = convert(&response)?;
            response
                .check_answers(&request)
                .map_err(|e| NetError::Protocol(e.to_string()))?;
            Ok(response.results)
        })
    }
}

/// A transport that could not be built (e.g. an invalid server URL): every call fails with
/// the build error.
#[derive(Debug, Clone)]
pub struct BrokenSyncApi(pub NetError);

impl SyncApi for BrokenSyncApi {
    fn bootstrap(&self, _cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }

    fn changes(&self, _: u64, _: u64, _: u32) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }

    fn push(&self, _ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }
}

/// [`EventsApi`] over the generated `streams::events` subscription (D24).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientEventsApi {}

/// Payloads are decoded here, not by the subscription: an event type this build does not
/// know (a newer server's) is still a change to pull, never an error that ends the stream.
struct ClientEventStream {
    sub: Subscription<rmpv::Value>,
}

/// The signal of one `/events` payload.
fn event_signal(seq: u64, payload: rmpv::Value) -> EventSignal {
    match rmpv::ext::from_value::<types::Event>(payload) {
        Ok(types::Event::AccountDisabled { reason }) => EventSignal::AccountClosed {
            seq,
            reason: reason.to_string(),
        },
        Ok(_) | Err(_) => EventSignal::Changed { seq },
    }
}

impl EventStream for ClientEventStream {
    fn next(&mut self) -> BoxFuture<'_, Option<Result<EventSignal, NetError>>> {
        Box::pin(async move {
            Some(match self.sub.next().await? {
                Err(e) => Err(classify(&e)),
                Ok(StreamEvent::Reset { seq }) => Ok(EventSignal::Reset { seq }),
                Ok(StreamEvent::Data { seq, payload }) => Ok(event_signal(seq, payload)),
            })
        })
    }
}

impl EventsApi for ClientEventsApi {
    fn subscribe(
        &self,
        server_url: &str,
        tokens: Tokens,
        resume_from: Option<u64>,
    ) -> Result<Box<dyn EventStream>, NetError> {
        let c = client(server_url, Some(tokens))?;
        // `streams::events`, with raw payloads (see `ClientEventStream`).
        let sub = Subscription::new(
            &c,
            "/api/v1/events".to_owned(),
            "events",
            true,
            StreamOptions {
                resume_from,
                // The session's loop reconnects with its own backoff and resume point.
                reconnect: false,
                ..StreamOptions::default()
            },
        );
        Ok(Box::new(ClientEventStream { sub }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, rmpv::Value)]) -> rmpv::Value {
        rmpv::Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (rmpv::Value::from(*k), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn every_event_is_a_change_except_account_closure_and_unknown_types_do_not_fail() {
        let started = map(&[
            ("type", "job.started".into()),
            ("id", "01K5DSSE0000000000000JOB01".into()),
            ("kind", "file_inbox".into()),
        ]);
        let unknown = map(&[("type", "insight.drafted".into()), ("id", 7.into())]);
        let closed = map(&[
            ("type", "account.disabled".into()),
            ("reason", "disabled".into()),
        ]);
        assert_eq!(
            [
                event_signal(3, started),
                event_signal(4, unknown),
                event_signal(5, rmpv::Value::Nil),
                event_signal(6, closed),
            ],
            [
                EventSignal::Changed { seq: 3 },
                EventSignal::Changed { seq: 4 },
                EventSignal::Changed { seq: 5 },
                EventSignal::AccountClosed {
                    seq: 6,
                    reason: "disabled".into()
                },
            ]
        );
    }

    #[tokio::test]
    async fn unreachable_server_is_offline() {
        // Port 9 (discard) on localhost is closed in the test environment: connection refused.
        let r = ClientAccountApi {}
            .login(
                "http://127.0.0.1:9".into(),
                "u".into(),
                "p".into(),
                "d".into(),
                Platform::Linux,
            )
            .await;
        assert!(
            matches!(&r, Err(NetError::Unreachable { reason, .. }) if reason == "connect"),
            "{r:?}"
        );
        let sync = ClientSyncApi::new(
            "http://127.0.0.1:9",
            std::sync::Arc::new(strata_client::StaticToken("t".into())),
        )
        .expect("client");
        let r = sync.changes(1, 1, 10).await;
        assert!(
            matches!(&r, Err(NetError::Unreachable { reason, .. }) if reason == "connect"),
            "{r:?}"
        );
    }

    #[test]
    fn sync_types_convert_both_ways() {
        let page = sync_model::ChangesPage {
            epoch: 3,
            changes: Vec::new(),
            next_seq: 9,
            has_more: false,
        };
        let generated: types::SyncChangesPage = convert(&page).expect("to contract type");
        let back: sync_model::ChangesPage = convert(&generated).expect("back");
        assert_eq!(back, page);
    }
}
