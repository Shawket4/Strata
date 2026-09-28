//! Network access (PLAN §12.1 `net/`): the generated `strata-client` (D15) behind small traits
//! the rest of the core depends on, so sync, auth and online features are testable against
//! fakes and against a real in-process server (`tests/server_e2e.rs`).
//!
//! | Trait | Implementation over `strata-client` | Endpoints |
//! |---|---|---|
//! | [`AccountApi`] | [`client::ClientAccountApi`] | auth, `/me` (+ export, deletion), devices, admin users, note history/revert, search, AI status, integrity, vault export/import, Ask |
//! | [`SyncApi`] | [`client::ClientSyncApi`] | `/sync/bootstrap`, `/sync/changes`, `/sync/push` (bodies converted to the shared `sync-model` types) |
//! | [`EventsApi`] | [`client::ClientEventsApi`] | the `/events` WebSocket (`streams::events`, resumable) |
//!
//! Graph endpoints (`/graph`, `/graph/local`, `/maps`) are not in the contract yet; the maps
//! are computed from the local cache with `graph-algo` meanwhile.

pub mod client;
pub mod server_url;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::sync::model::{BootstrapPage, ChangesPage, OpOutcome, SyncOp};
use crate::view::model::{NewUserRequest, Platform};

/// Why a network call failed, classified for the sync engine and the UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// The server could not be reached (a fake, or a stream that gave up).
    #[error("offline: {0}")]
    Offline(String),
    /// The request never got a response: TLS, DNS, connect, timeout or another I/O failure.
    /// Treated like [`NetError::Offline`] everywhere ([`NetError::is_offline`]); the reason
    /// reaches the UI as the `offline` failure's `reason`.
    #[error("unreachable ({reason}): {detail}")]
    Unreachable {
        /// Stable code of [`strata_client::TransportKind`] (`tls`, `dns`, `connect`,
        /// `timeout`, `network`).
        reason: String,
        /// The transport error, for logs.
        detail: String,
    },
    /// `401` after the refresh attempt: the session is gone.
    #[error("unauthorized")]
    Unauthorized,
    /// `401 invalid_credentials` at login.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// `403 account_pending`.
    #[error("account pending")]
    AccountPending,
    /// `403 account_rejected`.
    #[error("account rejected")]
    AccountRejected,
    /// `403 account_disabled`.
    #[error("account disabled")]
    AccountDisabled,
    /// `403 account_deletion_pending`.
    #[error("account deletion pending")]
    AccountDeletionPending,
    /// `410 epoch_changed`: re-bootstrap.
    #[error("epoch changed")]
    EpochChanged,
    /// `429 rate_limited`.
    #[error("rate limited")]
    RateLimited,
    /// The endpoint is not in the API contract yet.
    #[error("endpoint {endpoint} is not available yet")]
    NotAvailable {
        /// Operation name.
        endpoint: String,
    },
    /// Any other problem.
    #[error("{status} {problem_type}")]
    Api {
        /// HTTP status.
        status: u16,
        /// Problem slug.
        problem_type: String,
    },
    /// The response did not match the contract.
    #[error("protocol: {0}")]
    Protocol(String),
}

impl NetError {
    /// Whether retrying later may succeed (transport problems, 5xx, rate limits).
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Offline(_) | Self::Unreachable { .. } | Self::RateLimited => true,
            Self::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Whether the server could not be reached (offline, or a transport failure).
    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline(_) | Self::Unreachable { .. })
    }
}

impl From<NetError> for CoreError {
    fn from(e: NetError) -> Self {
        match e {
            NetError::Offline(_) => Self::Offline,
            NetError::Unreachable { reason, .. } => Self::Unreachable { reason },
            NetError::Unauthorized => Self::SessionExpired,
            NetError::InvalidCredentials => Self::InvalidCredentials,
            NetError::AccountPending => Self::AccountPending,
            NetError::AccountRejected => Self::AccountRejected,
            NetError::AccountDisabled => Self::AccountDisabled,
            NetError::AccountDeletionPending => Self::AccountDeletionPending,
            NetError::RateLimited => Self::RateLimited,
            NetError::NotAvailable { endpoint } => Self::NotAvailable { feature: endpoint },
            NetError::EpochChanged => Self::Server {
                status: 410,
                problem_type: "epoch_changed".to_owned(),
            },
            NetError::Api {
                status,
                problem_type,
            } => Self::Server {
                status,
                problem_type,
            },
            NetError::Protocol(m) => Self::Internal(m),
        }
    }
}

/// Tokens of a device session (login and refresh results).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionTokens {
    /// User ID.
    pub user_id: String,
    /// Device ID.
    pub device_id: String,
    /// Session ID.
    pub session_id: String,
    /// Access token.
    pub access_token: String,
    /// Access token expiry.
    pub access_expires_at: DateTime<Utc>,
    /// Refresh token (single use).
    pub refresh_token: String,
    /// Refresh token expiry.
    pub refresh_expires_at: DateTime<Utc>,
    /// Export-only session.
    pub export_only: bool,
    /// The password must be changed first.
    pub password_change_required: bool,
}

impl fmt::Debug for SessionTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionTokens")
            .field("user_id", &self.user_id)
            .field("device_id", &self.device_id)
            .field("export_only", &self.export_only)
            .finish_non_exhaustive()
    }
}

/// `GET /me`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeInfo {
    /// User ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: String,
    /// Status.
    pub status: String,
    /// Timezone.
    pub timezone: String,
    /// UI language.
    pub ui_language: String,
    /// Deletion date.
    pub deletion_at: Option<DateTime<Utc>>,
    /// Password change required.
    pub password_change_required: bool,
}

/// Supplies bearer tokens to authenticated calls (implemented by [`crate::auth`]).
pub type Tokens = Arc<dyn strata_client::TokenProvider>;

/// A device of the account (`GET /devices`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// ID.
    pub id: String,
    /// Name.
    pub name: String,
    /// Platform.
    pub platform: String,
    /// Signed in at.
    pub created: DateTime<Utc>,
    /// Last request.
    pub last_seen: DateTime<Utc>,
    /// The calling device.
    pub current: bool,
    /// Reminders delivered to it.
    pub reminders_enabled: bool,
}

/// An account as an admin sees it (`GET /admin/users`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminUserInfo {
    /// ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// `admin` | `member`.
    pub role: String,
    /// `pending` | `active` | `disabled` | `rejected` | `deletion_pending`.
    pub status: String,
    /// Created (requested).
    pub created: DateTime<Utc>,
    /// Scheduled purge.
    pub deletion_at: Option<DateTime<Utc>>,
    /// Export downloaded.
    pub export_downloaded_at: Option<DateTime<Utc>>,
    /// Must change the password.
    pub password_change_required: bool,
}

/// Fields of `PATCH /me`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeUpdate {
    /// Current password (password change).
    pub current_password: Option<String>,
    /// New password.
    pub new_password: Option<String>,
    /// Display name.
    pub display_name: Option<String>,
    /// UI language (`en` | `ar`).
    pub ui_language: Option<String>,
    /// IANA time zone.
    pub timezone: Option<String>,
    /// Preferences to set.
    pub preferences: Option<BTreeMap<String, String>>,
}

/// An admin's change to an account (`PATCH /admin/users/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminUpdate {
    /// `admin` | `member`.
    pub role: Option<String>,
    /// `active` | `disabled`.
    pub status: Option<String>,
    /// Replace the password with a one-time temporary password.
    pub reset_password: bool,
}

/// A revision of a note (`GET /notes/{id}/history`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionInfo {
    /// Commit.
    pub commit: String,
    /// When.
    pub at: DateTime<Utc>,
    /// `user` | `ai` | `system`.
    pub author: String,
    /// Message.
    pub message: String,
    /// Path then.
    pub path: String,
    /// `added` | `modified` | `renamed` | `deleted`.
    pub change: String,
}

/// A note's content at a revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionContent {
    /// Commit.
    pub commit: String,
    /// Content.
    pub content: String,
    /// Path.
    pub path: String,
}

/// A server search hit (`GET /search`).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteHit {
    /// Note.
    pub id: String,
    /// Title.
    pub title: String,
    /// Path.
    pub path: String,
    /// Kind.
    pub kind: String,
    /// Snippet.
    pub snippet: Option<String>,
    /// Score.
    pub score: f64,
}

/// `GET /ai/status`, trimmed to what the app shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStatusInfo {
    /// AI on for the account.
    pub enabled: bool,
    /// Provider name.
    pub provider: Option<String>,
    /// Paused until.
    pub paused_until: Option<DateTime<Utc>>,
    /// Pause reason.
    pub paused_reason: Option<String>,
    /// Queue depth.
    pub queue_depth: u64,
    /// Failed jobs a retry would run again (absent in statuses cached before it existed).
    #[serde(default)]
    pub failed_jobs: u64,
    /// Today's tokens used by the account.
    pub tokens_used: u64,
    /// The account's daily token limit (0 = none).
    pub tokens_limit: u64,
    /// Notes with current embeddings / live notes.
    pub embedded: Option<(u64, u64)>,
}

/// An integrity warning (`GET /integrity`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityInfo {
    /// ID.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Path.
    pub path: Option<String>,
    /// When.
    pub created: DateTime<Utc>,
}

/// An AI decision (`GET /ai-decisions`, §9.8 D13 activity feed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiDecisionInfo {
    /// ID.
    pub id: String,
    /// `relation`, `entity_mention`, `concept`, `custody_event`, `task_suggestion`,
    /// `filing`, `correction`.
    pub kind: String,
    /// Relation type (relations).
    pub rel_type: Option<String>,
    /// One-line summary.
    pub summary: String,
    /// Source note.
    pub source_note_id: Option<String>,
    /// Source title.
    pub source_title: Option<String>,
    /// Target ID.
    pub target_id: String,
    /// Target name.
    pub target_name: Option<String>,
    /// Confidence.
    pub confidence: Option<f64>,
    /// When.
    pub created: DateTime<Utc>,
    /// Undone at.
    pub reverted_at: Option<DateTime<Utc>>,
    /// The suggestion it came from.
    pub suggestion_id: Option<String>,
}

/// A similarity edge (`GET /graph?types=similarity`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarityEdge {
    /// Source note.
    pub src: String,
    /// Target note.
    pub dst: String,
    /// Cosine.
    pub score: Option<f64>,
}

/// One event of an Ask answer stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskEvent {
    /// Answer text.
    Tokens(String),
    /// A cited source.
    Citation {
        /// 1-based index.
        index: u32,
        /// Note.
        note_id: String,
        /// Path.
        path: String,
        /// Title.
        title: String,
        /// Block ID.
        block_id: Option<String>,
        /// Link target.
        target: String,
    },
    /// The final answer.
    Done(String),
}

/// An Ask answer stream (`GET /ask/{id}`).
pub trait AskStream: Send {
    /// The next event; `None` at the end.
    fn next(&mut self) -> BoxFuture<'_, Option<Result<AskEvent, NetError>>>;
}

fn not_available<T: Send + 'static>(endpoint: &str) -> BoxFuture<'static, Result<T, NetError>> {
    let e = NetError::NotAvailable {
        endpoint: endpoint.to_owned(),
    };
    Box::pin(async move { Err(e) })
}

/// Account and online endpoints. Methods beyond sign-in have defaults that answer
/// [`NetError::NotAvailable`], so test doubles implement only what they exercise.
#[allow(unused_variables)] // defaults ignore their arguments
pub trait AccountApi: Send + Sync + fmt::Debug {
    /// `POST /auth/signup`; returns the registered username.
    fn signup(
        &self,
        server_url: String,
        username: String,
        password: String,
        display_name: String,
    ) -> BoxFuture<'_, Result<String, NetError>>;
    /// `POST /auth/login`.
    fn login(
        &self,
        server_url: String,
        username: String,
        password: String,
        device_name: String,
        platform: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>>;
    /// `POST /auth/refresh`.
    fn refresh(
        &self,
        server_url: String,
        refresh_token: String,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>>;
    /// `POST /auth/logout`.
    fn logout(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<(), NetError>>;
    /// `GET /me`.
    fn me(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<MeInfo, NetError>>;
    /// `PATCH /devices/{id}` `{reminders_enabled}`.
    fn set_device_reminders(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        enabled: bool,
    ) -> BoxFuture<'_, Result<(), NetError>>;
    /// `GET /admin/users`.
    fn admin_users(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<AdminUserInfo>, NetError>>;
    /// `PATCH /me`.
    fn update_me(
        &self,
        server_url: String,
        tokens: Tokens,
        update: MeUpdate,
    ) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        not_available("update_me")
    }
    /// `GET /devices`.
    fn devices(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<DeviceInfo>, NetError>> {
        not_available("list_devices")
    }
    /// `PATCH /devices/{id}` `{name}`.
    fn rename_device(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        name: String,
    ) -> BoxFuture<'_, Result<DeviceInfo, NetError>> {
        not_available("update_device")
    }
    /// `DELETE /devices/{id}`.
    fn revoke_device(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("delete_device")
    }
    /// `GET /admin/settings`: the deletion grace period in seconds (D25).
    fn admin_settings(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<i64, NetError>> {
        not_available("admin_settings")
    }
    /// `POST /admin/users/{id}/approve`.
    fn admin_approve(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        not_available("admin_approve_user")
    }
    /// `POST /admin/users/{id}/reject`.
    fn admin_reject(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        not_available("admin_reject_user")
    }
    /// `PATCH /admin/users/{id}`: the account and the temporary password of a reset.
    fn admin_update(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
        update: AdminUpdate,
    ) -> BoxFuture<'_, Result<(AdminUserInfo, Option<String>), NetError>> {
        not_available("admin_update_user")
    }
    /// `DELETE /admin/users/{id}` (schedules the deletion).
    fn admin_schedule_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        not_available("admin_delete_user")
    }
    /// `POST /admin/users/{id}/cancel-deletion`.
    fn admin_cancel_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
        user_id: String,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        not_available("admin_cancel_deletion")
    }
    /// `POST /admin/users`.
    fn admin_create(
        &self,
        server_url: String,
        tokens: Tokens,
        request: NewUserRequest,
    ) -> BoxFuture<'_, Result<AdminUserInfo, NetError>> {
        not_available("admin_create_user")
    }
    /// `GET /me/export` (zip bytes).
    fn export_me(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<u8>, NetError>> {
        not_available("export_me")
    }
    /// `POST /me/confirm-deletion`.
    fn confirm_deletion(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("confirm_deletion")
    }
    /// `GET /export` (zip bytes).
    fn export_vault(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<u8>, NetError>> {
        not_available("export_vault")
    }
    /// `POST /import` (zip bytes): (imported, skipped).
    fn import_vault(
        &self,
        server_url: String,
        tokens: Tokens,
        zip: Vec<u8>,
    ) -> BoxFuture<'_, Result<(u32, u32), NetError>> {
        not_available("import_vault")
    }
    /// `GET /notes/{id}/history`, newest first.
    fn note_history(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
    ) -> BoxFuture<'_, Result<Vec<RevisionInfo>, NetError>> {
        not_available("get_note_history")
    }
    /// `GET /notes/{id}/history/{commit}`.
    fn note_revision(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
        commit: String,
    ) -> BoxFuture<'_, Result<RevisionContent, NetError>> {
        not_available("get_note_revision")
    }
    /// `POST /notes/{id}/revert`.
    fn revert_note(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
        commit: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("revert_note")
    }
    /// `GET /search?q=&mode=`.
    fn search(
        &self,
        server_url: String,
        tokens: Tokens,
        query: String,
        mode: String,
        limit: u32,
    ) -> BoxFuture<'_, Result<Vec<RemoteHit>, NetError>> {
        not_available("search")
    }
    /// `GET /ai/status`.
    fn ai_status(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<AiStatusInfo, NetError>> {
        not_available("ai_status")
    }
    /// `POST /ai/jobs/retry`: how many failed jobs were queued again.
    fn retry_failed_jobs(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<u64, NetError>> {
        not_available("retry_failed_jobs")
    }
    /// `GET /integrity`.
    fn integrity(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<IntegrityInfo>, NetError>> {
        not_available("get_integrity")
    }
    /// `POST /ask`: the answer's ID.
    fn ask(
        &self,
        server_url: String,
        tokens: Tokens,
        question: String,
        scope: Option<String>,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        not_available("ask")
    }
    /// `POST /notes/{id}/thread`: a follow-up question about one note; returns the answer's
    /// ID (streamed from `GET /ask/{id}`).
    fn ask_about_note(
        &self,
        server_url: String,
        tokens: Tokens,
        note_id: String,
        question: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        not_available("ask_about_note")
    }
    /// `GET /ask/{id}` (WebSocket).
    fn ask_stream(
        &self,
        server_url: String,
        tokens: Tokens,
        ask_id: String,
    ) -> Result<Box<dyn AskStream>, NetError> {
        Err(NetError::NotAvailable {
            endpoint: "ask_stream".to_owned(),
        })
    }
    /// `GET /ai-decisions?limit=`, newest first.
    fn ai_decisions(
        &self,
        server_url: String,
        tokens: Tokens,
        limit: u32,
    ) -> BoxFuture<'_, Result<Vec<AiDecisionInfo>, NetError>> {
        not_available("list_ai_decisions")
    }
    /// `POST /ai-decisions/{id}/reject` (undo).
    fn reject_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("reject_ai_decision")
    }
    /// `POST /ai-decisions/{id}/repoint`.
    fn repoint_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        target_id: String,
        hint: Option<String>,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("repoint_ai_decision")
    }
    /// `POST /ai-decisions/{id}/retype`.
    fn retype_ai_decision(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        rel_type: String,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        not_available("retype_ai_decision")
    }
    /// Similarity edges of the whole graph (`GET /graph?types=similarity`).
    fn similarity_edges(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<SimilarityEdge>, NetError>> {
        not_available("get_graph")
    }
    /// `PUT /maps/{id}` (JSON Canvas), creating or replacing: the map's vault path.
    fn put_map(
        &self,
        server_url: String,
        tokens: Tokens,
        id: String,
        content: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        not_available("put_map")
    }
    /// `POST /ask/{id}/save`: the new note's ID.
    fn save_ask(
        &self,
        server_url: String,
        tokens: Tokens,
        ask_id: String,
        title: Option<String>,
        created: chrono::DateTime<chrono::Utc>,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        not_available("save_ask")
    }
}

/// Sync endpoints (PLAN §7.5 "Sync").
pub trait SyncApi: Send + Sync + fmt::Debug {
    /// `GET /sync/bootstrap?cursor=`.
    fn bootstrap(&self, cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>>;
    /// `GET /sync/changes?since=&epoch=&limit=` (`410` → [`NetError::EpochChanged`]).
    fn changes(
        &self,
        since: u64,
        epoch: u64,
        limit: u32,
    ) -> BoxFuture<'_, Result<ChangesPage, NetError>>;
    /// `POST /sync/push`: per-op results, in order.
    fn push(&self, ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>>;
}

/// What an `/events` frame means for the device (§12.4: events only trigger pulls).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSignal {
    /// Something changed: pull. `seq` is the resume point to persist.
    Changed {
        /// Seq.
        seq: u64,
    },
    /// Events were missed: pull (the changes feed is authoritative).
    Reset {
        /// The server's head seq.
        seq: u64,
    },
    /// The account was closed (`disabled`, `deletion_pending`, `deleted`); the stream ends.
    AccountClosed {
        /// Seq.
        seq: u64,
        /// Reason.
        reason: String,
    },
}

/// One `/events` subscription.
pub trait EventStream: Send {
    /// The next signal; `None` when the server ended the stream. Errors end it too (the
    /// caller reconnects with backoff, resuming from the last seq).
    fn next(&mut self) -> BoxFuture<'_, Option<Result<EventSignal, NetError>>>;
}

/// The `/events` stream (§12.4, D24).
pub trait EventsApi: Send + Sync + fmt::Debug {
    /// Subscribes, resuming after `resume_from`.
    fn subscribe(
        &self,
        server_url: &str,
        tokens: Tokens,
        resume_from: Option<u64>,
    ) -> Result<Box<dyn EventStream>, NetError>;
}

/// Classifies a `strata-client` error.
pub fn classify(e: &strata_client::Error) -> NetError {
    use strata_client::{ApiError, Error};
    match e {
        Error::Transport(_) | Error::WebSocket(_) if e.transport_kind().is_some() => {
            NetError::Unreachable {
                reason: e
                    .transport_kind()
                    .map_or("network", strata_client::TransportKind::code)
                    .to_owned(),
                detail: e.to_string(),
            }
        }
        Error::Transport(t) => NetError::Offline(t.to_string()),
        Error::StreamClosed { operation, .. } => NetError::Offline((*operation).to_owned()),
        Error::Api(api) => {
            let p = api.problem();
            match api.as_ref() {
                ApiError::Unauthorized(_) => NetError::Unauthorized,
                ApiError::AccountPending(_) => NetError::AccountPending,
                ApiError::AccountDisabled(_) => NetError::AccountDisabled,
                ApiError::AccountDeletionPending(_) => NetError::AccountDeletionPending,
                ApiError::EpochChanged(_) => NetError::EpochChanged,
                ApiError::RateLimited(_) => NetError::RateLimited,
                _ => match p.type_.as_str() {
                    "invalid_credentials" => NetError::InvalidCredentials,
                    "account_rejected" => NetError::AccountRejected,
                    _ if p.status == 401 => NetError::Unauthorized,
                    _ => NetError::Api {
                        status: u16::try_from(p.status).unwrap_or(500),
                        problem_type: p.type_.clone(),
                    },
                },
            }
        }
        Error::UnexpectedResponse { status, .. } if *status >= 500 => NetError::Api {
            status: *status,
            problem_type: "internal".to_owned(),
        },
        other => NetError::Protocol(other.to_string()),
    }
}
