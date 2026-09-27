//! `POST /auth/signup`, `/auth/login`, `/auth/refresh`, `/auth/logout` (PLAN §7.5, §8, D6,
//! D22).

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_index::types::{Platform, UserRole};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::service::{self, IssuedSession, LoginInput, NewAccount};
use crate::auth::{AccountError, AuthState, Authenticated};
use crate::routes::me::AccountStatus;
use crate::wire::{MsgPack, MsgPackConfig, Problem};

/// Body limit of the account endpoints (passwords and names only).
pub const AUTH_BODY_LIMIT: usize = 8 * 1024;

/// The app platform a device runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DevicePlatform {
    /// Android.
    Android,
    /// iOS.
    Ios,
    /// macOS.
    Macos,
    /// Windows.
    Windows,
    /// Linux.
    Linux,
}

impl From<DevicePlatform> for Platform {
    fn from(p: DevicePlatform) -> Self {
        match p {
            DevicePlatform::Android => Self::Android,
            DevicePlatform::Ios => Self::Ios,
            DevicePlatform::Macos => Self::Macos,
            DevicePlatform::Windows => Self::Windows,
            DevicePlatform::Linux => Self::Linux,
        }
    }
}

impl From<Platform> for DevicePlatform {
    fn from(p: Platform) -> Self {
        match p {
            Platform::Android => Self::Android,
            Platform::Ios => Self::Ios,
            Platform::Macos => Self::Macos,
            Platform::Windows => Self::Windows,
            Platform::Linux => Self::Linux,
        }
    }
}

/// `POST /auth/signup` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SignupRequest {
    /// 3–32 letters, digits, `.`, `_`, `-` (any script); compared case- and
    /// confusable-insensitively.
    pub username: String,
    /// At least the configured minimum length.
    pub password: String,
    /// 1–100 characters.
    pub display_name: String,
}

/// An account awaiting approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PendingAccount {
    /// Account ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Username as entered.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Always `pending`.
    pub status: AccountStatus,
}

/// `POST /auth/login` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Username (case- and confusable-insensitive).
    pub username: String,
    /// Password.
    pub password: String,
    /// Name of this device, shown in the device list (1–100 characters).
    pub device_name: String,
    /// Platform of this device.
    pub platform: DevicePlatform,
}

/// `POST /auth/refresh` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RefreshRequest {
    /// The current refresh token (single use).
    pub refresh_token: String,
}

/// Tokens of a device session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AuthSession {
    /// The user.
    #[schema(value_type = String, format = "ulid")]
    pub user_id: Ulid,
    /// The device created at login.
    #[schema(value_type = String, format = "ulid")]
    pub device_id: Ulid,
    /// The session (revocable individually).
    #[schema(value_type = String, format = "ulid")]
    pub session_id: Ulid,
    /// Bearer token for API calls (EdDSA-signed, short-lived).
    pub access_token: String,
    /// When the access token expires.
    pub access_token_expires_at: DateTime<Utc>,
    /// Single-use refresh token; `POST /auth/refresh` returns the next one.
    pub refresh_token: String,
    /// When the refresh token (and the session) expires.
    pub refresh_token_expires_at: DateTime<Utc>,
    /// The account is scheduled for deletion: only `GET /me`, `GET /me/export`,
    /// `POST /me/confirm-deletion` and logout work.
    pub export_only: bool,
    /// The password was reset by an admin: change it with `PATCH /me` first.
    pub password_change_required: bool,
}

impl From<IssuedSession> for AuthSession {
    fn from(s: IssuedSession) -> Self {
        Self {
            user_id: s.user.as_ulid(),
            device_id: s.device.as_ulid(),
            session_id: s.session.as_ulid(),
            access_token: s.access.token,
            access_token_expires_at: s.access.expires_at,
            refresh_token: s.refresh_token,
            refresh_token_expires_at: s.refresh_expires_at,
            export_only: s.export_only,
            password_change_required: s.must_change_password,
        }
    }
}

/// Register an account; it waits for admin approval (D22).
#[utoipa::path(
    post,
    path = "/auth/signup",
    tag = "auth",
    operation_id = "signup",
    security(()),
    request_body = SignupRequest,
    responses(
        (status = 201, description = "Created in `pending` state: no vault and no session until an admin approves it.", body = PendingAccount),
        (status = 409, description = "`username_taken`: the name (or a confusable spelling of it) is in use.", body = Problem),
        (status = 422, description = "`invalid_body`: codes `invalid_username`, `invalid_display_name`, `invalid_password`, or a decoding error.", body = Problem),
        (status = 429, description = "`rate_limited`: too many sign-ups from this address or overall, or too many accounts awaiting approval. See `Retry-After`.", body = Problem),
    ),
)]
pub async fn signup(
    req: HttpRequest,
    state: web::Data<AuthState>,
    body: MsgPack<SignupRequest>,
) -> Result<impl Responder, AccountError> {
    let ip = service::client_ip(&req, state.settings.trust_forwarded_for);
    let body = body.into_inner();
    let user = service::signup(
        &state,
        &ip,
        &NewAccount {
            username: &body.username,
            display_name: &body.display_name,
            password: &body.password,
            role: UserRole::Member,
        },
    )
    .await?;
    Ok(MsgPack(PendingAccount {
        id: user.id.as_ulid(),
        username: user.username,
        display_name: user.display_name,
        status: AccountStatus::from(user.status),
    })
    .customize()
    .with_status(StatusCode::CREATED))
}

/// Sign in as a new device.
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    operation_id = "login",
    security(()),
    request_body = LoginRequest,
    responses(
        (status = 200, description = "A new device session. For an account scheduled for deletion the session is export-only.", body = AuthSession),
        (status = 401, description = "`invalid_credentials`: unknown username or wrong password.", body = Problem),
        (status = 403, description = "After a correct password: `account_pending` (awaiting approval), `account_rejected` or `account_disabled`.", body = Problem),
        (status = 422, description = "`invalid_body`: code `invalid_device_name`, or a decoding error.", body = Problem),
        (status = 429, description = "`rate_limited`: too many attempts from this address or for this username. See `Retry-After`.", body = Problem),
    ),
)]
pub async fn login(
    req: HttpRequest,
    state: web::Data<AuthState>,
    body: MsgPack<LoginRequest>,
) -> Result<MsgPack<AuthSession>, AccountError> {
    let ip = service::client_ip(&req, state.settings.trust_forwarded_for);
    let body = body.into_inner();
    let session = service::login(
        &state,
        &ip,
        LoginInput {
            username: &body.username,
            password: &body.password,
            device_name: &body.device_name,
            platform: body.platform.into(),
        },
    )
    .await?;
    Ok(MsgPack(session.into()))
}

/// Exchange a refresh token for new tokens (rotation).
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "auth",
    operation_id = "refresh",
    security(()),
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "New access and refresh tokens; the presented refresh token is spent.", body = AuthSession),
        (status = 401, description = "`unauthorized`: unknown, expired or revoked refresh token. Presenting a spent token again revokes the whole device session.", body = Problem),
    ),
)]
pub async fn refresh(
    state: web::Data<AuthState>,
    body: MsgPack<RefreshRequest>,
) -> Result<MsgPack<AuthSession>, AccountError> {
    let session = service::refresh(&state, &body.refresh_token).await?;
    Ok(MsgPack(session.into()))
}

/// Sign out: revoke this device session.
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    operation_id = "logout",
    responses((status = 204, description = "The session is revoked; its tokens stop working immediately.")),
)]
pub async fn logout(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<HttpResponse, AccountError> {
    service::logout(&state, auth.session).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Mounts the auth routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .app_data(MsgPackConfig::default().with_body_limit(AUTH_BODY_LIMIT))
            .route("/signup", web::post().to(signup))
            .route("/login", web::post().to(login))
            .route("/refresh", web::post().to(refresh))
            .route("/logout", web::post().to(logout)),
    );
}
