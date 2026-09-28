//! Account operations shared by the endpoints and `stratad`: sessions, sign-up, login,
//! refresh-token rotation, account creation, audit entries and the purge job.

use std::net::IpAddr;

use actix_web::HttpRequest;
use chrono::{DateTime, Utc};
use strata_common::{AuditId, DeviceId, IdGenerator, SessionId, UserId};
use strata_index::AccountsDb;
use strata_index::accounts::AccountsTx;
use strata_index::accounts::{AuditEntry, NewUser, RefreshOutcome, RefreshToken, Session, User};
use strata_index::types::{Platform, RevokeReason, UserRole, UserStatus};

use crate::auth::AuthState;
use crate::auth::error::AccountError;
use crate::auth::password::{MAX_PASSWORD_BYTES, PasswordHasher};
use crate::auth::provision::VaultProvisioner;
use crate::auth::rate_limit::Limited;
use crate::auth::tokens::{
    IssuedToken, TokenRole, TokenStatus, TokenSubject, hash_refresh_token, new_refresh_token,
};
use crate::auth::username::{self, NameError};

/// A newly opened or refreshed device session.
#[derive(Debug, Clone)]
pub struct IssuedSession {
    /// The user.
    pub user: UserId,
    /// The device.
    pub device: DeviceId,
    /// The session.
    pub session: SessionId,
    /// Access token.
    pub access: IssuedToken,
    /// Refresh token (plaintext; only its hash is stored).
    pub refresh_token: String,
    /// When the refresh token (and the session) expires.
    pub refresh_expires_at: DateTime<Utc>,
    /// Export-only session (account `deletion_pending`).
    pub export_only: bool,
    /// The account must change its password (admin reset).
    pub must_change_password: bool,
}

/// The client address used for rate limiting.
pub fn client_ip(req: &HttpRequest, trust_forwarded_for: bool) -> String {
    if trust_forwarded_for
        && let Some(ip) = req
            .connection_info()
            .realip_remote_addr()
            .and_then(parse_ip)
    {
        return ip.to_string();
    }
    req.peer_addr()
        .map_or_else(|| "unknown".to_owned(), |a| a.ip().to_string())
}

fn parse_ip(value: &str) -> Option<IpAddr> {
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<std::net::SocketAddr>().ok().map(|a| a.ip()))
}

fn limited(reason: &'static str) -> impl Fn(Limited) -> AccountError {
    move |l| AccountError::RateLimited {
        retry_after_secs: l.retry_after_secs,
        reason,
    }
}

/// Checks a new password against the length rules (`pointer` names the body field).
pub fn check_password(
    password: &str,
    min_chars: usize,
    pointer: &'static str,
) -> Result<(), AccountError> {
    if password.chars().count() < min_chars || password.len() > MAX_PASSWORD_BYTES {
        return Err(AccountError::InvalidField {
            pointer,
            code: "invalid_password",
            message: format!(
                "passwords have at least {min_chars} characters and at most {MAX_PASSWORD_BYTES} bytes"
            ),
        });
    }
    Ok(())
}

/// Validates a device name (same rules as display names).
pub fn check_device_name(name: &str) -> Result<String, AccountError> {
    username::parse_display_name(name).map_err(|_| AccountError::InvalidField {
        pointer: "/device_name",
        code: "invalid_device_name",
        message: "device names have 1 to 100 characters and no control characters".to_owned(),
    })
}

/// Appends an audit-log entry.
pub async fn audit(
    accounts: &AccountsDb,
    ids: &dyn IdGenerator,
    actor: Option<UserId>,
    action: &str,
    target: UserId,
    at: DateTime<Utc>,
) -> Result<(), AccountError> {
    accounts
        .append_audit(&AuditEntry {
            id: AuditId::generate(ids),
            actor_id: actor,
            action: action.to_owned(),
            target: format!("user:{target}"),
            at,
        })
        .await?;
    Ok(())
}

/// Input of [`create_active_account`].
#[derive(Debug, Clone)]
pub struct NewAccount<'a> {
    /// Username as entered.
    pub username: &'a str,
    /// Display name.
    pub display_name: &'a str,
    /// Initial password.
    pub password: &'a str,
    /// Role.
    pub role: UserRole,
}

/// Validated fields of a new account.
struct ValidAccount {
    username: username::Username,
    display_name: String,
}

fn validate_account(
    input: &NewAccount<'_>,
    min_password: usize,
) -> Result<ValidAccount, AccountError> {
    let username =
        username::parse_username(input.username).map_err(|e| AccountError::name("/username", e))?;
    let display_name = username::parse_display_name(input.display_name)
        .map_err(|e: NameError| AccountError::name("/display_name", e))?;
    check_password(input.password, min_password, "/password")?;
    Ok(ValidAccount {
        username,
        display_name,
    })
}

async fn insert_user(
    accounts: &AccountsDb,
    new: &NewUser,
    now: DateTime<Utc>,
) -> Result<User, AccountError> {
    if accounts
        .user_by_username(&new.username_normalized)
        .await?
        .is_some()
    {
        return Err(AccountError::UsernameTaken);
    }
    match accounts.create_user(new, now).await {
        Ok(user) => Ok(user),
        Err(e) if e.is_unique_violation() => Err(AccountError::UsernameTaken),
        Err(e) => Err(e.into()),
    }
}

/// Creates an active account with its vault (`POST /admin/users`, `stratad create-user`).
/// `actor` is the creating admin (`None` for the CLI). Writes the `user.create` audit entry.
#[allow(clippy::too_many_arguments)] // the composition root's parts, spelled out
pub async fn create_active_account(
    accounts: &AccountsDb,
    passwords: &PasswordHasher,
    vaults: &dyn VaultProvisioner,
    ids: &dyn IdGenerator,
    min_password: usize,
    input: &NewAccount<'_>,
    actor: Option<UserId>,
    now: DateTime<Utc>,
) -> Result<User, AccountError> {
    let valid = validate_account(input, min_password)?;
    let new = NewUser {
        id: UserId::generate(ids),
        username: valid.username.display,
        username_normalized: valid.username.key,
        display_name: valid.display_name,
        password_hash: passwords.hash(input.password)?,
        role: input.role,
        status: UserStatus::Active,
        approved_by: actor,
    };
    let user = insert_user(accounts, &new, now).await?;
    vaults.provision(user.id).await?;
    audit(accounts, ids, actor, "user.create", user.id, now).await?;
    Ok(user)
}

/// `POST /auth/signup` (D22): a `pending` account, no vault, no session.
pub async fn signup(
    state: &AuthState,
    ip: &str,
    input: &NewAccount<'_>,
) -> Result<User, AccountError> {
    state
        .limiters
        .signup_ip
        .check(ip)
        .map_err(limited("too many sign-ups from this address"))?;
    state
        .limiters
        .signup_global
        .check("global")
        .map_err(limited("too many sign-ups; try again later"))?;
    let valid = validate_account(input, state.settings.min_password_length)?;
    let pending = state.accounts.count_pending().await?;
    if pending >= i64::from(state.settings.max_pending_signups) {
        return Err(AccountError::RateLimited {
            retry_after_secs: 3600,
            reason: "too many accounts are awaiting approval; try again later",
        });
    }
    let now = state.clock.now();
    let new = NewUser {
        id: UserId::generate(state.ids.as_ref()),
        username: valid.username.display,
        username_normalized: valid.username.key,
        display_name: valid.display_name,
        password_hash: state.passwords.hash(input.password)?,
        role: UserRole::Member,
        status: UserStatus::Pending,
        approved_by: None,
    };
    let user = insert_user(&state.accounts, &new, now).await?;
    audit(
        &state.accounts,
        state.ids.as_ref(),
        Some(user.id),
        "user.signup",
        user.id,
        now,
    )
    .await?;
    Ok(user)
}

/// Login input.
#[derive(Debug, Clone, Copy)]
pub struct LoginInput<'a> {
    /// Username as typed.
    pub username: &'a str,
    /// Password.
    pub password: &'a str,
    /// Name for the new device.
    pub device_name: &'a str,
    /// Device platform.
    pub platform: Platform,
}

/// `POST /auth/login`: verifies credentials, then the account status, then opens a device
/// session (export-only for `deletion_pending` accounts).
pub async fn login(
    state: &AuthState,
    ip: &str,
    input: LoginInput<'_>,
) -> Result<IssuedSession, AccountError> {
    let key = username::login_key(input.username);
    state
        .limiters
        .login_ip
        .check(ip)
        .map_err(limited("too many login attempts from this address"))?;
    state
        .limiters
        .login_user
        .check(&key)
        .map_err(limited("too many login attempts for this account"))?;
    let device_name = check_device_name(input.device_name)?;
    // No username contains NUL (and PostgreSQL text cannot hold it): an unknown user.
    let found = if key.contains('\0') {
        None
    } else {
        state.accounts.user_by_username(&key).await?
    };
    let Some(user) = found else {
        state.passwords.verify_dummy(input.password);
        return Err(AccountError::InvalidCredentials);
    };
    if !state.passwords.verify(input.password, &user.password_hash) {
        return Err(AccountError::InvalidCredentials);
    }
    let status = match user.status {
        UserStatus::Pending => return Err(AccountError::AccountPending),
        UserStatus::Rejected => return Err(AccountError::AccountRejected),
        UserStatus::Disabled => return Err(AccountError::AccountDisabled),
        UserStatus::Active => TokenStatus::Active,
        UserStatus::DeletionPending => TokenStatus::DeletionPending,
    };
    let now = state.clock.now();
    // One transaction: a failure anywhere leaves no device, session or token behind.
    let mut tx = state.accounts.begin().await?;
    if state.passwords.needs_rehash(&user.password_hash) {
        let rehashed = state.passwords.hash(input.password)?;
        tx.set_password_hash(user.id, &rehashed, user.must_change_password, now)
            .await?;
    }
    let ids = state.ids.as_ref();
    let device = tx
        .create_device(
            user.id,
            DeviceId::generate(ids),
            &device_name,
            input.platform,
            now,
        )
        .await?;
    let session = Session {
        user_id: user.id,
        id: SessionId::generate(ids),
        device_id: device.id,
        created: now,
        expires: now + state.settings.session_ttl,
        revoked_at: None,
        revoked_reason: None,
        export_only: status == TokenStatus::DeletionPending,
    };
    tx.create_session(&session).await?;
    let issued = issue_tokens(state, &mut tx, &user, &session).await?;
    tx.commit().await?;
    Ok(issued)
}

/// Stores a new refresh token for `session` in `tx` and signs an access token. Nothing is
/// visible until the caller commits `tx`.
async fn issue_tokens(
    state: &AuthState,
    tx: &mut AccountsTx,
    user: &User,
    session: &Session,
) -> Result<IssuedSession, AccountError> {
    let now = state.clock.now();
    let refresh_token = new_refresh_token()?;
    tx.insert_refresh_token(&RefreshToken {
        user_id: user.id,
        token_hash: hash_refresh_token(&refresh_token),
        session_id: session.id,
        issued: now,
        expires: session.expires,
        used_at: None,
    })
    .await?;
    let status = if session.export_only {
        TokenStatus::DeletionPending
    } else {
        TokenStatus::Active
    };
    let access = state.tokens.issue(&TokenSubject {
        user: user.id,
        session: session.id,
        device: session.device_id,
        role: TokenRole::from(user.role),
        status,
    })?;
    Ok(IssuedSession {
        user: user.id,
        device: session.device_id,
        session: session.id,
        access,
        refresh_token,
        refresh_expires_at: session.expires,
        export_only: session.export_only,
        must_change_password: user.must_change_password,
    })
}

/// `POST /auth/refresh`: rotates a refresh token. A replayed token revokes its whole
/// session (reuse detection); a session whose account can no longer hold it is revoked.
/// Spending the old token, touching the device and storing the new token are one
/// transaction: if any step fails, the old token stays valid and no new one exists.
pub async fn refresh(
    state: &AuthState,
    refresh_token: &str,
) -> Result<IssuedSession, AccountError> {
    const INVALID: AccountError = AccountError::Unauthorized("invalid or expired refresh token");
    let now = state.clock.now();
    let mut tx = state.accounts.begin().await?;
    let spent = match tx
        .use_refresh_token(&hash_refresh_token(refresh_token), now)
        .await?
    {
        RefreshOutcome::Invalid => return Err(INVALID),
        RefreshOutcome::Reused(token) => {
            state.revocations.revoke_sessions([token.session_id]);
            tx.commit().await?;
            tracing::warn!(
                user = %token.user_id,
                session = %token.session_id,
                "refresh token reuse detected; session revoked"
            );
            return Err(AccountError::Unauthorized(
                "refresh token reuse detected; the session was revoked",
            ));
        }
        RefreshOutcome::Fresh(token) => token,
    };
    let session = tx.session_by_id(spent.session_id).await?.ok_or(INVALID)?;
    let user = tx.user_by_id(spent.user_id).await?.ok_or(INVALID)?;
    let allowed = match user.status {
        UserStatus::Active => !session.export_only,
        UserStatus::DeletionPending => session.export_only,
        _ => false,
    };
    if !allowed {
        state.revocations.revoke_sessions([session.id]);
        tx.revoke_session(session.id, RevokeReason::Admin, now)
            .await?;
        tx.commit().await?;
        return Err(AccountError::Unauthorized("the session is no longer valid"));
    }
    tx.touch_device(user.id, session.device_id, now).await?;
    let issued = issue_tokens(state, &mut tx, &user, &session).await?;
    tx.commit().await?;
    Ok(issued)
}

/// `POST /auth/logout`: revokes the caller's session.
pub async fn logout(state: &AuthState, session: SessionId) -> Result<(), AccountError> {
    state.revocations.revoke_sessions([session]);
    state
        .accounts
        .revoke_session(session, RevokeReason::Logout, state.clock.now())
        .await?;
    Ok(())
}

/// Revokes every live session of `user` (in memory first, then in the database).
pub async fn revoke_all_sessions(
    state: &AuthState,
    user: UserId,
    reason: RevokeReason,
) -> Result<Vec<SessionId>, AccountError> {
    let revoked = state
        .accounts
        .revoke_user_sessions(user, reason, state.clock.now())
        .await?;
    state.revocations.revoke_sessions(revoked.iter().copied());
    Ok(revoked)
}

/// Purges one account whose deletion is due (D25): removes the user's directory, then in one
/// transaction deletes the `users` row — every user-owned row cascades from it — and writes
/// one `user.purge` audit entry. Returns false if the account is not `deletion_pending`
/// (e.g. the deletion was cancelled meanwhile).
pub async fn purge_account(
    state: &AuthState,
    user: UserId,
    now: DateTime<Utc>,
) -> Result<bool, AccountError> {
    let Some(row) = state.accounts.user_by_id(user).await? else {
        return Ok(false);
    };
    if row.status != UserStatus::DeletionPending || row.deletion_at.is_none_or(|at| at > now) {
        return Ok(false);
    }
    // From here on every token of this user is refused, whatever happens below.
    state.revocations.mark_purged(user);
    state.vaults.deprovision(user).await?;
    let mut tx = state.accounts_pool.begin().await?;
    let deleted = sqlx::query(
        "DELETE FROM users WHERE id = $1 AND status = 'deletion_pending' AND deletion_at <= $2",
    )
    .bind(user)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if deleted != 1 {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, target, at) VALUES ($1, NULL, 'user.purge', $2, $3)",
    )
    .bind(AuditId::generate(state.ids.as_ref()))
    .bind(format!("user:{user}"))
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(user = %user, "account purged");
    Ok(true)
}

/// The purge job (D25): purges every `deletion_pending` account whose grace period ended at
/// or before `now`. Failures are logged and retried on the next run. Returns the purged
/// users.
pub async fn purge_due_accounts(
    state: &AuthState,
    now: DateTime<Utc>,
) -> Result<Vec<UserId>, AccountError> {
    let mut purged = Vec::new();
    for user in state.accounts.users_due_for_purge(now).await? {
        match purge_account(state, user, now).await {
            Ok(true) => purged.push(user),
            Ok(false) => {}
            Err(err) => tracing::error!(user = %user, error = %err, "account purge failed"),
        }
    }
    Ok(purged)
}
