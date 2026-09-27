//! Account-management data access through the `strata_accounts` role: users, invites, the
//! audit log, and the account-bridge tables (devices, sessions, refresh tokens). This role has
//! no grants on vault data, so nothing here can read notes even through a bug.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use strata_common::{AuditId, DeviceId, InviteId, SessionId, UserId};

use crate::error::Result;
use crate::types::{Platform, PushProvider, RevokeReason, UserRole, UserStatus};

/// The `strata_accounts` database handle.
#[derive(Debug, Clone)]
pub struct AccountsDb {
    pool: PgPool,
}

/// A `users` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct User {
    /// ID.
    pub id: UserId,
    /// Username as entered.
    pub username: String,
    /// Normalised username (unique).
    pub username_normalized: String,
    /// Display name.
    pub display_name: String,
    /// Argon2id PHC string.
    pub password_hash: String,
    /// Role.
    pub role: UserRole,
    /// Lifecycle status.
    pub status: UserStatus,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Last change.
    pub updated: DateTime<Utc>,
    /// Approving admin (None for admin-created or CLI-created accounts).
    pub approved_by: Option<UserId>,
    /// Approval time.
    pub approved_at: Option<DateTime<Utc>>,
    /// Rejection time.
    pub rejected_at: Option<DateTime<Utc>>,
    /// Disable time.
    pub disabled_at: Option<DateTime<Utc>>,
    /// Admin who scheduled deletion.
    pub deletion_requested_by: Option<UserId>,
    /// When deletion was scheduled.
    pub deletion_requested_at: Option<DateTime<Utc>>,
    /// When the purge runs (set iff `deletion_pending`).
    pub deletion_at: Option<DateTime<Utc>>,
    /// When the user downloaded their export.
    pub export_downloaded_at: Option<DateTime<Utc>>,
    /// Set by an admin password reset: the account is restricted until the user chooses a new
    /// password (`PATCH /me`).
    pub must_change_password: bool,
}

/// Input for [`AccountsDb::create_user`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    /// ID.
    pub id: UserId,
    /// Username as entered.
    pub username: String,
    /// Normalised username.
    pub username_normalized: String,
    /// Display name.
    pub display_name: String,
    /// Argon2id PHC string.
    pub password_hash: String,
    /// Role.
    pub role: UserRole,
    /// `Pending` for self-signup (D22); `Active` for admin/CLI-created accounts (approved now).
    pub status: UserStatus,
    /// Approving admin for admin-created accounts.
    pub approved_by: Option<UserId>,
}

/// An `invites` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Invite {
    /// ID.
    pub id: InviteId,
    /// SHA-256 of the invite token.
    pub token_hash: Vec<u8>,
    /// Role granted.
    pub role: UserRole,
    /// Creating admin.
    pub created_by: Option<UserId>,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Expiry.
    pub expires: DateTime<Utc>,
    /// Use time.
    pub used_at: Option<DateTime<Utc>>,
    /// The account created with it.
    pub used_by: Option<UserId>,
}

/// An `audit_log` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AuditEntry {
    /// ID.
    pub id: AuditId,
    /// Acting user; None for the system (e.g. the purge job).
    pub actor_id: Option<UserId>,
    /// Action code, e.g. `user.approve`.
    pub action: String,
    /// Target reference, e.g. `user:<id>`.
    pub target: String,
    /// When.
    pub at: DateTime<Utc>,
}

/// A `devices` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Device {
    /// Owner.
    pub user_id: UserId,
    /// ID.
    pub id: DeviceId,
    /// Name given at login.
    pub name: String,
    /// Platform.
    pub platform: Platform,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Last request.
    pub last_seen: DateTime<Utc>,
    /// Push provider.
    pub push_provider: PushProvider,
    /// Push token (None iff provider is `none`).
    pub push_token: Option<String>,
    /// Last push registration change.
    pub push_updated: Option<DateTime<Utc>>,
    /// Whether the device schedules local reminder notifications (D27, default true).
    pub reminders_enabled: bool,
}

/// A `sessions` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Session {
    /// Owner.
    pub user_id: UserId,
    /// ID (carried in access tokens).
    pub id: SessionId,
    /// Device.
    pub device_id: DeviceId,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Absolute expiry.
    pub expires: DateTime<Utc>,
    /// Revocation time.
    pub revoked_at: Option<DateTime<Utc>>,
    /// Revocation reason.
    pub revoked_reason: Option<RevokeReason>,
    /// Export-only session of a `deletion_pending` account (D25).
    pub export_only: bool,
}

/// A `refresh_tokens` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct RefreshToken {
    /// Owner.
    pub user_id: UserId,
    /// SHA-256 of the token.
    pub token_hash: Vec<u8>,
    /// Session.
    pub session_id: SessionId,
    /// Issued at.
    pub issued: DateTime<Utc>,
    /// Expiry.
    pub expires: DateTime<Utc>,
    /// Rotation time (a second use is reuse).
    pub used_at: Option<DateTime<Utc>>,
}

/// Outcome of [`AccountsDb::use_refresh_token`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// First use: the token is now spent; issue a new one for the returned session.
    Fresh(RefreshToken),
    /// The token was already used: reuse detected; the session has been revoked.
    Reused(RefreshToken),
    /// Unknown or expired token.
    Invalid,
}

macro_rules! user_cols {
    () => {
        concat!(
            "id, username, username_normalized, display_name, password_hash, role, status, ",
            "created, updated, approved_by, approved_at, rejected_at, disabled_at, deletion_requested_by, ",
            "deletion_requested_at, deletion_at, export_downloaded_at, must_change_password"
        )
    };
}
macro_rules! user_query {
    ($tail:literal) => {
        concat!("SELECT ", user_cols!(), " FROM users ", $tail)
    };
}
macro_rules! user_update {
    ($set:literal, $where:literal) => {
        concat!(
            "UPDATE users SET ",
            $set,
            " WHERE id = $1 AND ",
            $where,
            " RETURNING ",
            user_cols!()
        )
    };
}
macro_rules! device_cols {
    () => {
        "user_id, id, name, platform, created, last_seen, push_provider, push_token, push_updated, \
         reminders_enabled"
    };
}

impl AccountsDb {
    /// Wraps a pool connected as `strata_accounts`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ---- users ------------------------------------------------------------------------------

    /// Inserts a user. A taken normalised username fails with a unique violation
    /// (`IndexError::is_unique_violation`).
    pub async fn create_user(&self, new: &NewUser, now: DateTime<Utc>) -> Result<User> {
        let approved_at = matches!(new.status, UserStatus::Active).then_some(now);
        Ok(sqlx::query_as(concat!(
            "INSERT INTO users (id, username, username_normalized, display_name, password_hash, role, ",
            "status, created, updated, approved_by, approved_at) ",
            "VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, $9, $10) RETURNING ",
            user_cols!()
        ))
        .bind(new.id)
        .bind(&new.username)
        .bind(&new.username_normalized)
        .bind(&new.display_name)
        .bind(&new.password_hash)
        .bind(new.role)
        .bind(new.status)
        .bind(now)
        .bind(new.approved_by)
        .bind(approved_at)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Looks a user up by ID.
    pub async fn user_by_id(&self, id: UserId) -> Result<Option<User>> {
        user_by_id(&mut *self.pool.acquire().await?, id).await
    }

    /// Looks a user up by normalised username.
    pub async fn user_by_username(&self, username_normalized: &str) -> Result<Option<User>> {
        Ok(
            sqlx::query_as(user_query!("WHERE username_normalized = $1"))
                .bind(username_normalized)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Lists users, optionally filtered by status, oldest first.
    pub async fn list_users(&self, status: Option<UserStatus>) -> Result<Vec<User>> {
        Ok(sqlx::query_as(user_query!(
            "WHERE ($1::text IS NULL OR status = $1) ORDER BY created, id"
        ))
        .bind(status)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Number of accounts waiting for approval (signup cap, §15).
    pub async fn count_pending(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT count(*) FROM users WHERE status = 'pending'")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// `pending` → `active`. None if the user is missing or not pending.
    pub async fn approve_user(
        &self,
        id: UserId,
        by: UserId,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'active', approved_by = $2, approved_at = $3, updated = $3",
            "status = 'pending'"
        ))
        .bind(id)
        .bind(by)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// `pending` → `rejected`. None if the user is missing or not pending.
    pub async fn reject_user(&self, id: UserId, now: DateTime<Utc>) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'rejected', rejected_at = $2, updated = $2",
            "status = 'pending'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// `active` → `disabled`. Callers also revoke sessions ([`Self::revoke_user_sessions`]).
    pub async fn disable_user(&self, id: UserId, now: DateTime<Utc>) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'disabled', disabled_at = $2, updated = $2",
            "status = 'active'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// `disabled` → `active`.
    pub async fn enable_user(&self, id: UserId, now: DateTime<Utc>) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'active', disabled_at = NULL, updated = $2",
            "status = 'disabled'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// `active`/`disabled` → `deletion_pending` with the purge at `deletion_at` (D25).
    pub async fn schedule_deletion(
        &self,
        id: UserId,
        by: UserId,
        now: DateTime<Utc>,
        deletion_at: DateTime<Utc>,
    ) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'deletion_pending', deletion_requested_by = $2, deletion_requested_at = $3, \
             deletion_at = $4, updated = $3",
            "status IN ('active', 'disabled')"
        ))
        .bind(id)
        .bind(by)
        .bind(now)
        .bind(deletion_at)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// `deletion_pending` → `active` (admin cancel).
    pub async fn cancel_deletion(&self, id: UserId, now: DateTime<Utc>) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "status = 'active', deletion_requested_by = NULL, deletion_requested_at = NULL, \
             deletion_at = NULL, updated = $2",
            "status = 'deletion_pending'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Brings the purge forward to `now` (`POST /me/confirm-deletion`).
    pub async fn confirm_deletion(&self, id: UserId, now: DateTime<Utc>) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "deletion_at = LEAST(deletion_at, $2), updated = $2",
            "status = 'deletion_pending'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Records that the user downloaded their export (admins see only this timestamp).
    pub async fn mark_export_downloaded(
        &self,
        id: UserId,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        Ok(sqlx::query_as(user_update!(
            "export_downloaded_at = $2, updated = $2",
            "status = 'deletion_pending'"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Replaces the password hash and sets whether the user must change it (true after an
    /// admin reset, false when the user chose it).
    pub async fn set_password_hash(
        &self,
        id: UserId,
        password_hash: &str,
        must_change_password: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        set_password_hash(
            &mut *self.pool.acquire().await?,
            id,
            password_hash,
            must_change_password,
            now,
        )
        .await
    }

    /// Changes the role.
    pub async fn set_role(
        &self,
        id: UserId,
        role: UserRole,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        Ok(
            sqlx::query_as(user_update!("role = $2, updated = $3", "true"))
                .bind(id)
                .bind(role)
                .bind(now)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// `deletion_pending` users whose purge time has come.
    pub async fn users_due_for_purge(&self, now: DateTime<Utc>) -> Result<Vec<UserId>> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM users WHERE status = 'deletion_pending' AND deletion_at <= $1 ORDER BY deletion_at, id",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Deletes a `deletion_pending` user row. Every user-owned row cascades with it (foreign
    /// keys to `users`), in this one statement. Returns false if not in `deletion_pending`.
    pub async fn purge_user(&self, id: UserId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM users WHERE id = $1 AND status = 'deletion_pending'")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    // ---- invites ----------------------------------------------------------------------------

    /// Stores an invite.
    pub async fn create_invite(&self, invite: &Invite) -> Result<()> {
        sqlx::query(
            "INSERT INTO invites (id, token_hash, role, created_by, created, expires, used_at, used_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(invite.id)
        .bind(&invite.token_hash)
        .bind(invite.role)
        .bind(invite.created_by)
        .bind(invite.created)
        .bind(invite.expires)
        .bind(invite.used_at)
        .bind(invite.used_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Spends an unused, unexpired invite. None if unknown, used, or expired.
    pub async fn consume_invite(
        &self,
        token_hash: &[u8],
        used_by: UserId,
        now: DateTime<Utc>,
    ) -> Result<Option<Invite>> {
        Ok(sqlx::query_as(
            "UPDATE invites SET used_at = $3, used_by = $2 \
             WHERE token_hash = $1 AND used_at IS NULL AND expires > $3 \
             RETURNING id, token_hash, role, created_by, created, expires, used_at, used_by",
        )
        .bind(token_hash)
        .bind(used_by)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?)
    }

    // ---- audit log --------------------------------------------------------------------------

    /// Appends an audit entry.
    pub async fn append_audit(&self, entry: &AuditEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, target, at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(entry.id)
        .bind(entry.actor_id)
        .bind(&entry.action)
        .bind(&entry.target)
        .bind(entry.at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Newest audit entries first.
    pub async fn list_audit(&self, limit: i64) -> Result<Vec<AuditEntry>> {
        Ok(sqlx::query_as(
            "SELECT id, actor_id, action, target, at FROM audit_log ORDER BY at DESC, id DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    // ---- devices, sessions, refresh tokens (account bridge) ---------------------------------

    /// Starts a transaction: the account writes of one login or one refresh-token rotation
    /// either all happen or none do ([`AccountsTx`]).
    pub async fn begin(&self) -> Result<AccountsTx> {
        Ok(AccountsTx {
            tx: self.pool.begin().await?,
        })
    }

    /// Creates a device at login.
    pub async fn create_device(
        &self,
        user_id: UserId,
        id: DeviceId,
        name: &str,
        platform: Platform,
        now: DateTime<Utc>,
    ) -> Result<Device> {
        create_device(
            &mut *self.pool.acquire().await?,
            user_id,
            id,
            name,
            platform,
            now,
        )
        .await
    }

    /// Creates a session for a device.
    pub async fn create_session(&self, session: &Session) -> Result<()> {
        create_session(&mut *self.pool.acquire().await?, session).await
    }

    /// Looks a session up by ID (from access-token claims).
    pub async fn session_by_id(&self, id: SessionId) -> Result<Option<Session>> {
        session_by_id(&mut *self.pool.acquire().await?, id).await
    }

    /// Revokes one session; false if unknown or already revoked.
    pub async fn revoke_session(
        &self,
        id: SessionId,
        reason: RevokeReason,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        revoke_session(&mut *self.pool.acquire().await?, id, reason, now).await
    }

    /// Revokes every live session of a user (disable, deletion scheduling); returns the IDs.
    pub async fn revoke_user_sessions(
        &self,
        user_id: UserId,
        reason: RevokeReason,
        now: DateTime<Utc>,
    ) -> Result<Vec<SessionId>> {
        Ok(sqlx::query_scalar(
            "UPDATE sessions SET revoked_at = $3, revoked_reason = $2 \
             WHERE user_id = $1 AND revoked_at IS NULL RETURNING id",
        )
        .bind(user_id)
        .bind(reason)
        .bind(now)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Revoked sessions that have not expired yet — the in-memory revocation set (§8).
    pub async fn live_revocations(&self, now: DateTime<Utc>) -> Result<Vec<SessionId>> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM sessions WHERE revoked_at IS NOT NULL AND expires > $1 ORDER BY id",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Stores a newly issued refresh token.
    pub async fn insert_refresh_token(&self, token: &RefreshToken) -> Result<()> {
        insert_refresh_token(&mut *self.pool.acquire().await?, token).await
    }

    /// Spends a refresh token (rotation) in its own transaction; see
    /// [`AccountsTx::use_refresh_token`].
    pub async fn use_refresh_token(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<RefreshOutcome> {
        let mut tx = self.begin().await?;
        let outcome = tx.use_refresh_token(token_hash, now).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}

/// One `strata_accounts` transaction over users and the account-bridge tables. Dropping it
/// without [`commit`](Self::commit) rolls everything back, so a failure part-way through a
/// login or a refresh-token rotation leaves no device, session or token behind.
#[derive(Debug)]
pub struct AccountsTx {
    tx: sqlx::Transaction<'static, sqlx::Postgres>,
}

impl AccountsTx {
    /// Commits.
    pub async fn commit(self) -> Result<()> {
        self.tx.commit().await?;
        Ok(())
    }

    /// Rolls back (same as dropping, but reports errors).
    pub async fn rollback(self) -> Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }

    /// See [`AccountsDb::user_by_id`].
    pub async fn user_by_id(&mut self, id: UserId) -> Result<Option<User>> {
        user_by_id(&mut self.tx, id).await
    }

    /// See [`AccountsDb::set_password_hash`].
    pub async fn set_password_hash(
        &mut self,
        id: UserId,
        password_hash: &str,
        must_change_password: bool,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        set_password_hash(&mut self.tx, id, password_hash, must_change_password, now).await
    }

    /// See [`AccountsDb::create_device`].
    pub async fn create_device(
        &mut self,
        user_id: UserId,
        id: DeviceId,
        name: &str,
        platform: Platform,
        now: DateTime<Utc>,
    ) -> Result<Device> {
        create_device(&mut self.tx, user_id, id, name, platform, now).await
    }

    /// Updates a device's `last_seen` (never backwards). False if absent.
    pub async fn touch_device(
        &mut self,
        user_id: UserId,
        id: DeviceId,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE devices SET last_seen = GREATEST(last_seen, $3) WHERE user_id = $1 AND id = $2",
        )
        .bind(user_id)
        .bind(id)
        .bind(now)
        .execute(&mut *self.tx)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// See [`AccountsDb::create_session`].
    pub async fn create_session(&mut self, session: &Session) -> Result<()> {
        create_session(&mut self.tx, session).await
    }

    /// See [`AccountsDb::session_by_id`].
    pub async fn session_by_id(&mut self, id: SessionId) -> Result<Option<Session>> {
        session_by_id(&mut self.tx, id).await
    }

    /// See [`AccountsDb::revoke_session`].
    pub async fn revoke_session(
        &mut self,
        id: SessionId,
        reason: RevokeReason,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        revoke_session(&mut self.tx, id, reason, now).await
    }

    /// See [`AccountsDb::insert_refresh_token`].
    pub async fn insert_refresh_token(&mut self, token: &RefreshToken) -> Result<()> {
        insert_refresh_token(&mut self.tx, token).await
    }

    /// Spends a refresh token (rotation). A second use of the same token revokes its session
    /// (`refresh_reuse`) and reports [`RefreshOutcome::Reused`]; commit to keep that
    /// revocation. Unknown, expired, or revoked-session tokens are
    /// [`RefreshOutcome::Invalid`]. The token row stays locked until the transaction ends, so
    /// a concurrent rotation of the same token waits and then sees it spent.
    pub async fn use_refresh_token(
        &mut self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<RefreshOutcome> {
        let token: Option<RefreshToken> = sqlx::query_as(
            "SELECT t.user_id, t.token_hash, t.session_id, t.issued, t.expires, t.used_at \
             FROM refresh_tokens t JOIN sessions s ON s.user_id = t.user_id AND s.id = t.session_id \
             WHERE t.token_hash = $1 AND s.revoked_at IS NULL FOR UPDATE OF t",
        )
        .bind(token_hash)
        .fetch_optional(&mut *self.tx)
        .await?;
        Ok(match token {
            None => RefreshOutcome::Invalid,
            Some(t) if t.used_at.is_some() => {
                sqlx::query(
                    "UPDATE sessions SET revoked_at = $3, revoked_reason = 'refresh_reuse' \
                     WHERE user_id = $1 AND id = $2 AND revoked_at IS NULL",
                )
                .bind(t.user_id)
                .bind(t.session_id)
                .bind(now)
                .execute(&mut *self.tx)
                .await?;
                RefreshOutcome::Reused(t)
            }
            Some(t) if t.expires <= now => RefreshOutcome::Invalid,
            Some(mut t) => {
                sqlx::query("UPDATE refresh_tokens SET used_at = $2 WHERE token_hash = $1")
                    .bind(token_hash)
                    .bind(now)
                    .execute(&mut *self.tx)
                    .await?;
                t.used_at = Some(now);
                RefreshOutcome::Fresh(t)
            }
        })
    }
}

// ---- statements shared by the pool and transaction forms -------------------------------------

async fn user_by_id(conn: &mut PgConnection, id: UserId) -> Result<Option<User>> {
    Ok(sqlx::query_as(user_query!("WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await?)
}

async fn set_password_hash(
    conn: &mut PgConnection,
    id: UserId,
    password_hash: &str,
    must_change_password: bool,
    now: DateTime<Utc>,
) -> Result<Option<User>> {
    Ok(sqlx::query_as(user_update!(
        "password_hash = $2, must_change_password = $3, updated = $4",
        "true"
    ))
    .bind(id)
    .bind(password_hash)
    .bind(must_change_password)
    .bind(now)
    .fetch_optional(conn)
    .await?)
}

async fn create_device(
    conn: &mut PgConnection,
    user_id: UserId,
    id: DeviceId,
    name: &str,
    platform: Platform,
    now: DateTime<Utc>,
) -> Result<Device> {
    Ok(sqlx::query_as(concat!(
        "INSERT INTO devices (user_id, id, name, platform, created, last_seen) \
         VALUES ($1, $2, $3, $4, $5, $5) RETURNING ",
        device_cols!()
    ))
    .bind(user_id)
    .bind(id)
    .bind(name)
    .bind(platform)
    .bind(now)
    .fetch_one(conn)
    .await?)
}

async fn create_session(conn: &mut PgConnection, session: &Session) -> Result<()> {
    sqlx::query(
        "INSERT INTO sessions (user_id, id, device_id, created, expires, revoked_at, revoked_reason, export_only) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(session.user_id)
    .bind(session.id)
    .bind(session.device_id)
    .bind(session.created)
    .bind(session.expires)
    .bind(session.revoked_at)
    .bind(session.revoked_reason)
    .bind(session.export_only)
    .execute(conn)
    .await?;
    Ok(())
}

async fn session_by_id(conn: &mut PgConnection, id: SessionId) -> Result<Option<Session>> {
    Ok(sqlx::query_as(
        "SELECT user_id, id, device_id, created, expires, revoked_at, revoked_reason, export_only \
         FROM sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

async fn revoke_session(
    conn: &mut PgConnection,
    id: SessionId,
    reason: RevokeReason,
    now: DateTime<Utc>,
) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE sessions SET revoked_at = $3, revoked_reason = $2 WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(reason)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(done.rows_affected() == 1)
}

async fn insert_refresh_token(conn: &mut PgConnection, token: &RefreshToken) -> Result<()> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, session_id, issued, expires, used_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(token.user_id)
    .bind(&token.token_hash)
    .bind(token.session_id)
    .bind(token.issued)
    .bind(token.expires)
    .bind(token.used_at)
    .execute(conn)
    .await?;
    Ok(())
}
