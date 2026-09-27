//! Network access (PLAN §12.1 `net/`): the generated `strata-client` (D15) behind two small
//! traits the rest of the core depends on, so sync and auth are testable against fakes and the
//! transport can grow as endpoints land.
//!
//! | Trait | Implementation over `strata-client` | Status |
//! |---|---|---|
//! | [`AccountApi`] | [`client::ClientAccountApi`]: `signup`, `login`, `refresh`, `logout`, `get_me`, `update_device`, `admin_list_users` | endpoints exist; wired |
//! | [`SyncApi`] | [`client::ClientSyncApi`] | `/sync/bootstrap`, `/sync/changes`, `/sync/push` are not in the contract yet: every call returns [`NetError::NotAvailable`] |
//! | [`EventsApi`] | [`client::ClientEventsApi`] | `/events` stream not in the contract yet: returns [`NetError::NotAvailable`] |

pub mod client;

use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;

use crate::error::CoreError;
use crate::sync::model::{BootstrapPage, ChangesPage, OpOutcome, PushOp};
use crate::view::model::{AdminUserItem, Platform};

/// Why a network call failed, classified for the sync engine and the UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// The server could not be reached (DNS, TCP, TLS, timeout).
    #[error("offline: {0}")]
    Offline(String),
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
            Self::Offline(_) | Self::RateLimited => true,
            Self::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

impl From<NetError> for CoreError {
    fn from(e: NetError) -> Self {
        match e {
            NetError::Offline(_) => Self::Offline,
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

/// Account endpoints.
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
    ) -> BoxFuture<'_, Result<Vec<AdminUserItem>, NetError>>;
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
    fn push(&self, ops: Vec<PushOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>>;
}

/// The `/events` stream: each item only means "something changed, pull" (§12.4).
pub trait EventsApi: Send + Sync + fmt::Debug {
    /// Waits for the next event (or a reset). Errors end the subscription; the caller
    /// reconnects with backoff.
    fn next_event(&self) -> BoxFuture<'_, Result<(), NetError>>;
}

/// Classifies a `strata-client` error.
pub fn classify(e: &strata_client::Error) -> NetError {
    use strata_client::{ApiError, Error};
    match e {
        Error::Transport(t) => NetError::Offline(t.to_string()),
        Error::WebSocket(w) => NetError::Offline(w.to_string()),
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
