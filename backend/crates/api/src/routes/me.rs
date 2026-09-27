//! `GET /me`, `PATCH /me`, `GET /me/export`, `POST /me/confirm-deletion` (PLAN §7.5
//! "Account & admin", D25).
//!
//! Per-user settings (UI language, timezone, preferences) live in the user-owned `settings`
//! table, read and written through the caller's `UserScope`; every change appends `setting`
//! change-log rows (one per synced record, `sync_model::settings`) so devices pull it. Account fields (display name,
//! password) live in `users`, reached through the `strata_accounts` role.

use std::collections::BTreeMap;
use std::str::FromStr;

use actix_web::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_index::ScopedTx;
use strata_index::accounts::User;
use strata_index::repo::settings;
use strata_index::types::{RevokeReason, UserRole, UserStatus};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::service::{self, check_password};
use crate::auth::{AccountError, AuthState, Authenticated, export, username};
use crate::sync::records::put_setting_logged;
use crate::wire::{MsgPack, MsgPackConfig, Problem, ZIP};

/// Setting key of the UI language.
pub const SETTING_UI_LANGUAGE: &str = "ui_language";
/// Setting key of the timezone.
pub const SETTING_TIMEZONE: &str = "timezone";
/// Setting key of the free-form preferences.
pub const SETTING_PREFERENCES: &str = "preferences";

/// Most preference entries.
pub const MAX_PREFERENCES: usize = 64;
/// Longest preference key (bytes).
pub const MAX_PREFERENCE_KEY: usize = 64;
/// Longest preference value (bytes).
pub const MAX_PREFERENCE_VALUE: usize = 1024;

/// A zip archive (`application/zip`), the body of `GET /me/export`.
#[derive(Debug, Clone, Copy)]
pub struct ZipArchive;

impl utoipa::PartialSchema for ZipArchive {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, SchemaFormat, Type};
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Binary)))
            .description(Some("A zip archive."))
            .build()
            .into()
    }
}

impl ToSchema for ZipArchive {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ZipArchive")
    }
}

/// Account role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Manages accounts; cannot read other users' vaults.
    Admin,
    /// Regular user.
    Member,
}

impl From<UserRole> for Role {
    fn from(r: UserRole) -> Self {
        match r {
            UserRole::Admin => Self::Admin,
            UserRole::Member => Self::Member,
        }
    }
}

impl From<Role> for UserRole {
    fn from(r: Role) -> Self {
        match r {
            Role::Admin => Self::Admin,
            Role::Member => Self::Member,
        }
    }
}

/// Account lifecycle status (D22, D25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    /// Signed up, awaiting approval.
    Pending,
    /// Approved and usable.
    Active,
    /// Disabled by an admin.
    Disabled,
    /// Sign-up rejected.
    Rejected,
    /// Scheduled for deletion (export-only sessions).
    DeletionPending,
}

impl From<UserStatus> for AccountStatus {
    fn from(s: UserStatus) -> Self {
        match s {
            UserStatus::Pending => Self::Pending,
            UserStatus::Active => Self::Active,
            UserStatus::Disabled => Self::Disabled,
            UserStatus::Rejected => Self::Rejected,
            UserStatus::DeletionPending => Self::DeletionPending,
        }
    }
}

impl From<AccountStatus> for UserStatus {
    fn from(s: AccountStatus) -> Self {
        match s {
            AccountStatus::Pending => Self::Pending,
            AccountStatus::Active => Self::Active,
            AccountStatus::Disabled => Self::Disabled,
            AccountStatus::Rejected => Self::Rejected,
            AccountStatus::DeletionPending => Self::DeletionPending,
        }
    }
}

/// UI language of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    /// English (LTR).
    #[default]
    En,
    /// Arabic (RTL).
    Ar,
}

/// The signed-in user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Me {
    /// Account ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Username as entered at sign-up.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: Role,
    /// Account status.
    pub status: AccountStatus,
    /// When the account was created.
    pub created: DateTime<Utc>,
    /// UI language (default `en`).
    pub ui_language: UiLanguage,
    /// IANA timezone for dates, tasks and reminders (default: the server's).
    pub timezone: String,
    /// Free-form client preferences.
    pub preferences: BTreeMap<String, String>,
    /// The password was reset by an admin and must be changed with `PATCH /me`.
    pub password_change_required: bool,
    /// This session is export-only (account scheduled for deletion).
    pub export_only: bool,
    /// When a scheduled deletion will be purged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion_at: Option<DateTime<Utc>>,
}

/// `PATCH /me` body: every field is optional; absent fields are unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdateMe {
    /// New display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// New password; requires `current_password`. Other sessions are signed out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_password: Option<String>,
    /// The current password (required with `new_password`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_password: Option<String>,
    /// UI language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_language: Option<UiLanguage>,
    /// IANA timezone, e.g. `Africa/Cairo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Replaces all preferences (at most 64 entries; keys ≤ 64 bytes, values ≤ 1024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferences: Option<BTreeMap<String, String>>,
}

fn invalid(pointer: &'static str, code: &'static str, message: &str) -> AccountError {
    AccountError::InvalidField {
        pointer,
        code,
        message: message.to_owned(),
    }
}

fn decode_setting<T: serde::de::DeserializeOwned>(value: Option<Vec<u8>>) -> Option<T> {
    value.and_then(|bytes| rmp_serde::from_slice(&bytes).ok())
}

fn encode_setting<T: Serialize>(value: &T) -> Result<Vec<u8>, AccountError> {
    rmp_serde::to_vec_named(value).map_err(|e| AccountError::Internal(e.to_string()))
}

async fn load_user(state: &AuthState, auth: &Authenticated) -> Result<User, AccountError> {
    state
        .accounts
        .user_by_id(auth.user_id())
        .await?
        .ok_or(AccountError::Unauthorized("account deleted"))
}

async fn me_view(
    state: &AuthState,
    auth: &Authenticated,
    user: User,
    tx: &mut ScopedTx,
) -> Result<Me, AccountError> {
    let ui_language =
        decode_setting(settings::get_setting(tx, SETTING_UI_LANGUAGE).await?).unwrap_or_default();
    let timezone = decode_setting(settings::get_setting(tx, SETTING_TIMEZONE).await?)
        .unwrap_or_else(|| state.settings.default_timezone.clone());
    let preferences =
        decode_setting(settings::get_setting(tx, SETTING_PREFERENCES).await?).unwrap_or_default();
    Ok(Me {
        id: user.id.as_ulid(),
        password_change_required: user.must_change_password,
        username: user.username,
        display_name: user.display_name,
        role: user.role.into(),
        status: user.status.into(),
        created: user.created,
        ui_language,
        timezone,
        preferences,
        export_only: auth.export_only,
        deletion_at: user.deletion_at,
    })
}

/// The signed-in user, role and settings.
#[utoipa::path(
    get,
    path = "/me",
    tag = "account",
    operation_id = "get_me",
    responses((status = 200, description = "The caller's account and settings.", body = Me)),
)]
pub async fn get_me(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<MsgPack<Me>, AccountError> {
    let user = load_user(&state, &auth).await?;
    let mut tx = state.app_db.begin(auth.scope()).await?;
    let me = me_view(&state, &auth, user, &mut tx).await?;
    tx.commit().await?;
    Ok(MsgPack(me))
}

fn check_preferences(prefs: &BTreeMap<String, String>) -> Result<(), AccountError> {
    let ok = prefs.len() <= MAX_PREFERENCES
        && prefs.iter().all(|(k, v)| {
            !k.is_empty() && k.len() <= MAX_PREFERENCE_KEY && v.len() <= MAX_PREFERENCE_VALUE
        });
    if ok {
        Ok(())
    } else {
        Err(invalid(
            "/preferences",
            "invalid_preferences",
            "at most 64 preferences; keys of 1 to 64 bytes, values up to 1024 bytes",
        ))
    }
}

/// Change the password, display name, UI language, timezone or preferences.
#[utoipa::path(
    patch,
    path = "/me",
    tag = "account",
    operation_id = "update_me",
    request_body = UpdateMe,
    responses(
        (status = 200, description = "The updated account. A password change signs out every other session.", body = Me),
        (status = 422, description = "`invalid_body`: codes `current_password_required`, `current_password_mismatch`, `invalid_password`, `invalid_display_name`, `invalid_timezone`, `invalid_preferences`, or a decoding error.", body = Problem),
    ),
)]
pub async fn update_me(
    state: web::Data<AuthState>,
    auth: Authenticated,
    body: MsgPack<UpdateMe>,
) -> Result<MsgPack<Me>, AccountError> {
    let body = body.into_inner();
    let now = state.clock.now();
    let mut user = load_user(&state, &auth).await?;

    // Validate everything before changing anything.
    let display_name = body
        .display_name
        .as_deref()
        .map(username::parse_display_name)
        .transpose()
        .map_err(|e| AccountError::name("/display_name", e))?;
    if let Some(tz) = &body.timezone
        && chrono_tz::Tz::from_str(tz).is_err()
    {
        return Err(invalid(
            "/timezone",
            "invalid_timezone",
            "not an IANA timezone",
        ));
    }
    if let Some(prefs) = &body.preferences {
        check_preferences(prefs)?;
    }
    let new_hash = match &body.new_password {
        None => None,
        Some(new_password) => {
            let current = body.current_password.as_deref().ok_or_else(|| {
                invalid(
                    "/current_password",
                    "current_password_required",
                    "changing the password requires current_password",
                )
            })?;
            if !state.passwords.verify(current, &user.password_hash) {
                return Err(invalid(
                    "/current_password",
                    "current_password_mismatch",
                    "current_password is not the account's password",
                ));
            }
            check_password(
                new_password,
                state.settings.min_password_length,
                "/new_password",
            )?;
            Some(state.passwords.hash(new_password)?)
        }
    };

    if let Some(name) = display_name {
        sqlx::query("UPDATE users SET display_name = $2, updated = $3 WHERE id = $1")
            .bind(user.id)
            .bind(&name)
            .bind(now)
            .execute(&state.accounts_pool)
            .await?;
        user.display_name = name;
    }
    if let Some(hash) = new_hash {
        let updated = state
            .accounts
            .set_password_hash(user.id, &hash, false, now)
            .await?
            .ok_or(AccountError::Unauthorized("account deleted"))?;
        let others: Vec<strata_common::SessionId> = sqlx::query_scalar(
            "UPDATE sessions SET revoked_at = $3, revoked_reason = $4 \
             WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL RETURNING id",
        )
        .bind(user.id)
        .bind(auth.session)
        .bind(now)
        .bind(RevokeReason::Logout)
        .fetch_all(&state.accounts_pool)
        .await?;
        state.revocations.revoke_sessions(others);
        state
            .revocations
            .update_user(user.id, |f| f.must_change_password = false);
        user = updated;
    }

    // Each changed setting record gets a change-log row, so the user's devices pull it.
    let mut tx = state.app_db.begin(auth.scope()).await?;
    if let Some(lang) = body.ui_language {
        put_setting_logged(&mut tx, SETTING_UI_LANGUAGE, &encode_setting(&lang)?, now).await?;
    }
    if let Some(tz) = &body.timezone {
        put_setting_logged(&mut tx, SETTING_TIMEZONE, &encode_setting(tz)?, now).await?;
    }
    if let Some(prefs) = &body.preferences {
        put_setting_logged(&mut tx, SETTING_PREFERENCES, &encode_setting(prefs)?, now).await?;
    }
    let mut auth = auth;
    auth.0.must_change_password = false;
    let me = me_view(&state, &auth, user, &mut tx).await?;
    tx.commit().await?;
    Ok(MsgPack(me))
}

/// Download the caller's own vault as a zip.
#[utoipa::path(
    get,
    path = "/me/export",
    tag = "account",
    operation_id = "export_me",
    responses(
        (status = 200, description = "The caller's vault (notes, attachments, `.meta/`; no git history), streamed as a zip. Available to export-only sessions.", content_type = "application/zip", body = inline(ZipArchive)),
    ),
)]
pub async fn export_me(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<HttpResponse, AccountError> {
    // The path comes from the authenticated scope, never from the request.
    let user = auth.scope().user_id();
    let (rx, task) = export::stream_vault_zip(state.vaults.vault_dir(user));
    // Chunks as they are written; once the archive is complete the download is recorded
    // (admins see only that timestamp) before the response ends.
    let body = futures_util::stream::unfold(
        Some((rx, task, state.clone())),
        move |step| async move {
            let (mut rx, task, state) = step?;
            match rx.recv().await {
                Some(Ok(chunk)) => Some((Ok(chunk), Some((rx, task, state)))),
                Some(Err(err)) => Some((Err(err), None)),
                None => match task.await {
                    Ok(Ok(())) => {
                        let now = state.clock.now();
                        if let Err(err) = state.accounts.mark_export_downloaded(user, now).await {
                            tracing::warn!(user = %user, error = %err, "cannot record the export download");
                        }
                        None
                    }
                    Ok(Err(err)) => Some((Err(err), None)),
                    Err(err) => Some((Err(std::io::Error::other(err)), None)),
                },
            }
        },
    );
    Ok(HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, ZIP))
        .insert_header((
            CONTENT_DISPOSITION,
            "attachment; filename=\"strata-export.zip\"",
        ))
        .streaming(body))
}

/// End the deletion grace period now: the account and its vault are purged.
#[utoipa::path(
    post,
    path = "/me/confirm-deletion",
    tag = "account",
    operation_id = "confirm_deletion",
    responses(
        (status = 204, description = "The account, its vault and all its data were deleted; every token stops working."),
        (status = 409, description = "`account_state_conflict`: the account is not scheduled for deletion.", body = Problem),
    ),
)]
pub async fn confirm_deletion(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<HttpResponse, AccountError> {
    let now = state.clock.now();
    state
        .accounts
        .confirm_deletion(auth.user_id(), now)
        .await?
        .ok_or(AccountError::StateConflict(
            "the account is not scheduled for deletion",
        ))?;
    service::purge_account(&state, auth.user_id(), now).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Mounts the `/me` routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/me")
            .app_data(MsgPackConfig::default().with_body_limit(32 * 1024))
            .route(web::get().to(get_me))
            .route(web::patch().to(update_me)),
    )
    .route("/me/export", web::get().to(export_me))
    .route("/me/confirm-deletion", web::post().to(confirm_deletion));
}
