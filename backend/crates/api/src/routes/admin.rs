//! Account administration (PLAN §7.5 "Account & admin", D22, D25): list, create, approve,
//! reject, disable/enable, change role, reset password, schedule and cancel deletion.
//!
//! Every handler re-checks in the database that the caller is an active admin (a demotion
//! applies from the next request, whatever the access token says), and every change writes
//! one audit-log entry. Admins manage accounts only: nothing here reads vault data, and there
//! is no route to another user's export.

use actix_web::{Responder, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::UserId;
use strata_index::accounts::User;
use strata_index::types::{RevokeReason, UserRole, UserStatus};
use ulid::Ulid;
use utoipa::{IntoParams, ToSchema};

use crate::auth::password::temporary_password;
use crate::auth::service::{self, NewAccount, audit, revoke_all_sessions};
use crate::auth::{AccountError, AuthState, Authenticated};
use crate::routes::me::{AccountStatus, Role};
use crate::wire::{MsgPack, MsgPackConfig, Problem};

/// An account as admins see it: status and dates, never content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AdminUser {
    /// Account ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Username as entered.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: Role,
    /// Status.
    pub status: AccountStatus,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Approved (or admin-created) at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    /// Rejected at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_at: Option<DateTime<Utc>>,
    /// Disabled at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<DateTime<Utc>>,
    /// When deletion was scheduled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion_requested_at: Option<DateTime<Utc>>,
    /// When the account will be purged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion_at: Option<DateTime<Utc>>,
    /// When the user downloaded their export (admins never see the export itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_downloaded_at: Option<DateTime<Utc>>,
    /// The account is on a temporary password.
    pub password_change_required: bool,
}

impl From<User> for AdminUser {
    fn from(u: User) -> Self {
        Self {
            id: u.id.as_ulid(),
            password_change_required: u.must_change_password,
            username: u.username,
            display_name: u.display_name,
            role: u.role.into(),
            status: u.status.into(),
            created: u.created,
            approved_at: u.approved_at,
            rejected_at: u.rejected_at,
            disabled_at: u.disabled_at,
            deletion_requested_at: u.deletion_requested_at,
            deletion_at: u.deletion_at,
            export_downloaded_at: u.export_downloaded_at,
        }
    }
}

/// Query of `GET /admin/users`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListUsersQuery {
    /// Only accounts with this status.
    pub status: Option<AccountStatus>,
}

/// `POST /admin/users` body: an active account, created by an admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateUser {
    /// Username (same rules as sign-up).
    pub username: String,
    /// Initial password.
    pub password: String,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: Role,
}

/// Status an admin can set directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettableStatus {
    /// Re-enable a disabled account.
    Active,
    /// Disable: every session ends immediately.
    Disabled,
}

/// `PATCH /admin/users/{id}` body; absent fields are unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdateUser {
    /// Disable or re-enable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<SettableStatus>,
    /// New role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// Replace the password with a one-time temporary password (returned once); the user must
    /// change it at next sign-in, and every session ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_password: Option<bool>,
}

/// Result of `PATCH /admin/users/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UpdateUserResult {
    /// The account after the change.
    pub user: AdminUser,
    /// The temporary password, present only when `reset_password` was set. Shown once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary_password: Option<String>,
}

/// `GET /admin/settings`: server settings an admin needs before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AdminSettings {
    /// The grace period (seconds) between scheduling an account's deletion and its purge
    /// (D25): `DELETE /admin/users/{id}` sets `deletion_at` to now plus this.
    pub deletion_grace_secs: i64,
}

/// The caller's account if it is an active admin; `403 forbidden` otherwise.
async fn require_admin(state: &AuthState, auth: &Authenticated) -> Result<User, AccountError> {
    match state.accounts.user_by_id(auth.user_id()).await? {
        Some(u) if u.role == UserRole::Admin && u.status == UserStatus::Active => Ok(u),
        _ => Err(AccountError::Forbidden),
    }
}

async fn target(state: &AuthState, id: Ulid) -> Result<User, AccountError> {
    state
        .accounts
        .user_by_id(UserId::from_ulid(id))
        .await?
        .ok_or(AccountError::NotFound)
}

fn not_self(admin: &User, target: &User, what: &'static str) -> Result<(), AccountError> {
    if admin.id == target.id {
        Err(AccountError::StateConflict(what))
    } else {
        Ok(())
    }
}

/// List accounts, optionally by status.
#[utoipa::path(
    get,
    path = "/admin/users",
    tag = "admin",
    operation_id = "admin_list_users",
    params(ListUsersQuery),
    responses(
        (status = 200, description = "Accounts, oldest first.", body = Vec<AdminUser>),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
    ),
)]
pub async fn list_users(
    state: web::Data<AuthState>,
    auth: Authenticated,
    query: web::Query<ListUsersQuery>,
) -> Result<MsgPack<Vec<AdminUser>>, AccountError> {
    require_admin(&state, &auth).await?;
    let users = state
        .accounts
        .list_users(query.status.map(UserStatus::from))
        .await?;
    Ok(MsgPack(users.into_iter().map(AdminUser::from).collect()))
}

/// Server settings an admin needs before acting (the deletion grace period, so the purge date
/// can be shown before a deletion is scheduled).
#[utoipa::path(
    get,
    path = "/admin/settings",
    tag = "admin",
    operation_id = "admin_settings",
    responses(
        (status = 200, description = "The settings.", body = AdminSettings),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
    ),
)]
pub async fn settings(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<MsgPack<AdminSettings>, AccountError> {
    require_admin(&state, &auth).await?;
    Ok(MsgPack(AdminSettings {
        deletion_grace_secs: state.settings.deletion_grace.num_seconds(),
    }))
}

/// Create an active account (with its vault).
#[utoipa::path(
    post,
    path = "/admin/users",
    tag = "admin",
    operation_id = "admin_create_user",
    request_body = CreateUser,
    responses(
        (status = 201, description = "Created, active, vault provisioned.", body = AdminUser),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`username_taken`.", body = Problem),
        (status = 422, description = "`invalid_body`: codes `invalid_username`, `invalid_display_name`, `invalid_password`, or a decoding error.", body = Problem),
    ),
)]
pub async fn create_user(
    state: web::Data<AuthState>,
    auth: Authenticated,
    body: MsgPack<CreateUser>,
) -> Result<impl Responder, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let body = body.into_inner();
    let user = service::create_active_account(
        &state.accounts,
        &state.passwords,
        state.vaults.as_ref(),
        state.ids.as_ref(),
        state.settings.min_password_length,
        &NewAccount {
            username: &body.username,
            display_name: &body.display_name,
            password: &body.password,
            role: body.role.into(),
        },
        Some(admin.id),
        state.clock.now(),
    )
    .await?;
    Ok(MsgPack(AdminUser::from(user))
        .customize()
        .with_status(actix_web::http::StatusCode::CREATED))
}

/// Approve a pending sign-up: the account becomes active and its vault is created.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/approve",
    tag = "admin",
    operation_id = "admin_approve_user",
    params(("id" = Ulid, Path, description = "Account ID (ULID).")),
    responses(
        (status = 200, description = "Approved.", body = AdminUser),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`account_state_conflict`: the account is not pending.", body = Problem),
    ),
)]
pub async fn approve_user(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
) -> Result<MsgPack<AdminUser>, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let user = target(&state, id.into_inner()).await?;
    if user.status != UserStatus::Pending {
        return Err(AccountError::StateConflict("the account is not pending"));
    }
    let created = state.vaults.provision(user.id).await?;
    let now = state.clock.now();
    let Some(approved) = state.accounts.approve_user(user.id, admin.id, now).await? else {
        if created {
            state.vaults.deprovision(user.id).await?;
        }
        return Err(AccountError::StateConflict("the account is not pending"));
    };
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        "user.approve",
        user.id,
        now,
    )
    .await?;
    Ok(MsgPack(approved.into()))
}

/// Reject a pending sign-up.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/reject",
    tag = "admin",
    operation_id = "admin_reject_user",
    params(("id" = Ulid, Path, description = "Account ID (ULID).")),
    responses(
        (status = 200, description = "Rejected; the account can never sign in.", body = AdminUser),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`account_state_conflict`: the account is not pending.", body = Problem),
    ),
)]
pub async fn reject_user(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
) -> Result<MsgPack<AdminUser>, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let user = target(&state, id.into_inner()).await?;
    let now = state.clock.now();
    let rejected = state
        .accounts
        .reject_user(user.id, now)
        .await?
        .ok_or(AccountError::StateConflict("the account is not pending"))?;
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        "user.reject",
        user.id,
        now,
    )
    .await?;
    Ok(MsgPack(rejected.into()))
}

/// Disable or enable an account, change its role, or reset its password.
#[utoipa::path(
    patch,
    path = "/admin/users/{id}",
    tag = "admin",
    operation_id = "admin_update_user",
    params(("id" = Ulid, Path, description = "Account ID (ULID).")),
    request_body = UpdateUser,
    responses(
        (status = 200, description = "Updated. Disabling or resetting the password ends every session of the account immediately.", body = UpdateUserResult),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`account_state_conflict`: the status change does not apply to the account's state, or an admin tried to disable or demote themselves.", body = Problem),
    ),
)]
pub async fn update_user(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
    body: MsgPack<UpdateUser>,
) -> Result<MsgPack<UpdateUserResult>, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let mut user = target(&state, id.into_inner()).await?;
    let body = body.into_inner();
    check_update(&admin, &user, &body)?;
    if let Some(status) = body.status {
        user = apply_status(&state, &admin, &user, status).await?;
    }
    if let Some(role) = body.role
        && UserRole::from(role) != user.role
    {
        let now = state.clock.now();
        user = state
            .accounts
            .set_role(user.id, role.into(), now)
            .await?
            .ok_or(AccountError::NotFound)?;
        let action = match role {
            Role::Admin => "user.role.admin",
            Role::Member => "user.role.member",
        };
        audit(
            &state.accounts,
            state.ids.as_ref(),
            Some(admin.id),
            action,
            user.id,
            now,
        )
        .await?;
    }
    let mut temporary = None;
    if body.reset_password == Some(true) {
        let (updated, password) = reset_password(&state, &admin, &user).await?;
        user = updated;
        temporary = Some(password);
    }
    Ok(MsgPack(UpdateUserResult {
        user: user.into(),
        temporary_password: temporary,
    }))
}

/// Checks every requested change against the current state before any is applied.
fn check_update(admin: &User, user: &User, body: &UpdateUser) -> Result<(), AccountError> {
    match body.status {
        Some(SettableStatus::Disabled) => {
            not_self(admin, user, "admins cannot disable their own account")?;
            if user.status != UserStatus::Active {
                return Err(AccountError::StateConflict(
                    "only active accounts can be disabled",
                ));
            }
        }
        Some(SettableStatus::Active) if user.status != UserStatus::Disabled => {
            return Err(AccountError::StateConflict(
                "only disabled accounts can be enabled",
            ));
        }
        _ => {}
    }
    if body.role == Some(Role::Member) {
        not_self(admin, user, "admins cannot demote themselves")?;
    }
    if body.reset_password == Some(true)
        && !matches!(user.status, UserStatus::Active | UserStatus::Disabled)
    {
        return Err(AccountError::StateConflict(
            "passwords can only be reset on active or disabled accounts",
        ));
    }
    Ok(())
}

/// Disables (revoking every session at once) or re-enables an account.
async fn apply_status(
    state: &AuthState,
    admin: &User,
    user: &User,
    status: SettableStatus,
) -> Result<User, AccountError> {
    let now = state.clock.now();
    let (updated, action) = match status {
        SettableStatus::Disabled => {
            // Block in memory first (fail closed), then persist and revoke.
            state
                .revocations
                .update_user(user.id, |f| f.disabled = true);
            let updated = state.accounts.disable_user(user.id, now).await?.ok_or(
                AccountError::StateConflict("only active accounts can be disabled"),
            )?;
            revoke_all_sessions(state, user.id, RevokeReason::UserDisabled).await?;
            (updated, "user.disable")
        }
        SettableStatus::Active => {
            let updated = state.accounts.enable_user(user.id, now).await?.ok_or(
                AccountError::StateConflict("only disabled accounts can be enabled"),
            )?;
            state
                .revocations
                .update_user(user.id, |f| f.disabled = false);
            (updated, "user.enable")
        }
    };
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        action,
        user.id,
        now,
    )
    .await?;
    Ok(updated)
}

/// Replaces the password with a temporary one (returned) and ends every session.
async fn reset_password(
    state: &AuthState,
    admin: &User,
    user: &User,
) -> Result<(User, String), AccountError> {
    let now = state.clock.now();
    let password = temporary_password()?;
    let hash = state.passwords.hash(&password)?;
    state
        .revocations
        .update_user(user.id, |f| f.must_change_password = true);
    let updated = state
        .accounts
        .set_password_hash(user.id, &hash, true, now)
        .await?
        .ok_or(AccountError::NotFound)?;
    revoke_all_sessions(state, user.id, RevokeReason::Admin).await?;
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        "user.password_reset",
        user.id,
        now,
    )
    .await?;
    Ok((updated, password))
}

/// Schedule the account's deletion (D25): sessions end now; the user can still sign in to an
/// export-only session until the purge.
#[utoipa::path(
    delete,
    path = "/admin/users/{id}",
    tag = "admin",
    operation_id = "admin_delete_user",
    params(("id" = Ulid, Path, description = "Account ID (ULID).")),
    responses(
        (status = 200, description = "Scheduled: `deletion_pending` with the purge time in `deletion_at`.", body = AdminUser),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`account_state_conflict`: the account is not active or disabled, or is the caller's own.", body = Problem),
    ),
)]
pub async fn delete_user(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
) -> Result<MsgPack<AdminUser>, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let user = target(&state, id.into_inner()).await?;
    not_self(&admin, &user, "admins cannot delete their own account")?;
    if !matches!(user.status, UserStatus::Active | UserStatus::Disabled) {
        return Err(AccountError::StateConflict(
            "only active or disabled accounts can be scheduled for deletion",
        ));
    }
    let now = state.clock.now();
    state
        .revocations
        .update_user(user.id, |f| f.deletion_pending = true);
    let scheduled = state
        .accounts
        .schedule_deletion(user.id, admin.id, now, now + state.settings.deletion_grace)
        .await?
        .ok_or(AccountError::StateConflict(
            "only active or disabled accounts can be scheduled for deletion",
        ))?;
    // A disabled account being deleted keeps no "disabled" block: it may export.
    state
        .revocations
        .update_user(user.id, |f| f.disabled = false);
    revoke_all_sessions(&state, user.id, RevokeReason::DeletionScheduled).await?;
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        "user.delete.schedule",
        user.id,
        now,
    )
    .await?;
    Ok(MsgPack(scheduled.into()))
}

/// Cancel a scheduled deletion: the account is active again.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/cancel-deletion",
    tag = "admin",
    operation_id = "admin_cancel_deletion",
    params(("id" = Ulid, Path, description = "Account ID (ULID).")),
    responses(
        (status = 200, description = "Active again; export-only sessions end (the user signs in normally).", body = AdminUser),
        (status = 403, description = "`forbidden`: the caller is not an active admin. Also `account_deletion_pending` and `password_change_required`, as on every secured operation.", body = Problem),
        (status = 409, description = "`account_state_conflict`: no deletion is scheduled.", body = Problem),
    ),
)]
pub async fn cancel_deletion(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
) -> Result<MsgPack<AdminUser>, AccountError> {
    let admin = require_admin(&state, &auth).await?;
    let user = target(&state, id.into_inner()).await?;
    let now = state.clock.now();
    let restored = state
        .accounts
        .cancel_deletion(user.id, now)
        .await?
        .ok_or(AccountError::StateConflict("no deletion is scheduled"))?;
    revoke_all_sessions(&state, user.id, RevokeReason::Admin).await?;
    state
        .revocations
        .update_user(user.id, |f| f.deletion_pending = false);
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(admin.id),
        "user.delete.cancel",
        user.id,
        now,
    )
    .await?;
    Ok(MsgPack(restored.into()))
}

/// Mounts the admin routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/settings", web::get().to(settings));
    cfg.service(
        web::scope("/admin/users")
            .app_data(MsgPackConfig::default().with_body_limit(8 * 1024))
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/{id}", web::patch().to(update_user))
            .route("/{id}", web::delete().to(delete_user))
            .route("/{id}/approve", web::post().to(approve_user))
            .route("/{id}/reject", web::post().to(reject_user))
            .route("/{id}/cancel-deletion", web::post().to(cancel_deletion)),
    );
}
