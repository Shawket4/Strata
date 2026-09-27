//! `UserScope` and scoped transactions (PLAN §5.2).
//!
//! The chain of custody for a user ID is:
//!
//! 1. [`AppDb::new`] wraps the `strata_app` pool and returns the one [`ScopeIssuer`]. The
//!    composition root (`stratad`) hands the issuer only to the auth middleware and the job
//!    runner.
//! 2. The auth middleware resolves the session **once** and calls [`ScopeIssuer::issue`] to mint
//!    a [`UserScope`]. Nothing else can construct a `UserScope` (private field, no public
//!    constructor), so handlers can never scope to a user ID taken from a request.
//! 3. [`AppDb::begin`] turns a `UserScope` into a [`ScopedTx`]: a transaction on which
//!    `SELECT set_config('strata.user_id', $1, true)` has run. The setting is transaction-local,
//!    so it vanishes at commit/rollback and a pooled connection can never carry it into the
//!    next checkout.
//! 4. Every repository function takes `&mut ScopedTx` — never a raw user ID. Row-level security
//!    then makes PostgreSQL itself enforce the scope.

use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use strata_common::UserId;

use crate::error::{IndexError, Result};

/// Capability to mint [`UserScope`]s. Only obtainable from [`AppDb::new`].
#[derive(Debug, Clone)]
pub struct ScopeIssuer {
    _sealed: (),
}

impl ScopeIssuer {
    /// Mints a scope for an authenticated user (auth middleware) or a job's owner (job runner).
    pub fn issue(&self, user_id: UserId) -> UserScope {
        UserScope { user_id }
    }
}

/// Proof that the holder acts for exactly one authenticated user. Minted by [`ScopeIssuer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserScope {
    user_id: UserId,
}

impl UserScope {
    /// The scoped user.
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

/// The `strata_app` database handle. It can open transactions only for a [`UserScope`].
#[derive(Debug, Clone)]
pub struct AppDb {
    pool: PgPool,
}

impl AppDb {
    /// Wraps a pool connected as `strata_app`; returns the handle and its scope issuer.
    pub fn new(pool: PgPool) -> (Self, ScopeIssuer) {
        (Self { pool }, ScopeIssuer { _sealed: () })
    }

    /// Begins a transaction scoped to `scope`.
    pub async fn begin(&self, scope: &UserScope) -> Result<ScopedTx> {
        let mut tx = self.pool.begin().await?;
        let user = scope.user_id.as_uuid().to_string();
        let applied: Option<String> =
            sqlx::query_scalar("SELECT set_config('strata.user_id', $1, true)")
                .bind(&user)
                .fetch_one(&mut *tx)
                .await?;
        if applied.as_deref() != Some(user.as_str()) {
            return Err(IndexError::ScopeNotApplied);
        }
        Ok(ScopedTx {
            tx,
            user_id: scope.user_id,
        })
    }

    /// Users with queued jobs whose earliest `run_after` is at or before `now`, oldest first
    /// (reads the content-free `job_wakeups` hints; see `repo::jobs`).
    pub async fn due_job_users(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        limit: i64,
    ) -> Result<Vec<UserId>> {
        Ok(sqlx::query_scalar(
            "SELECT user_id FROM job_wakeups WHERE run_after <= $1 ORDER BY run_after, user_id LIMIT $2",
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }
}

/// A transaction scoped to one user. Dropping it without [`commit`](Self::commit) rolls back.
#[derive(Debug)]
pub struct ScopedTx {
    tx: Transaction<'static, Postgres>,
    user_id: UserId,
}

impl ScopedTx {
    /// The user this transaction is scoped to.
    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// The underlying connection, for queries not covered by a repository function. RLS still
    /// applies. Never run `SET strata.user_id` (session-level) on it.
    pub fn conn(&mut self) -> &mut PgConnection {
        &mut self.tx
    }

    /// Commits the transaction; the scope ends with it.
    pub async fn commit(self) -> Result<()> {
        Ok(self.tx.commit().await?)
    }

    /// Rolls the transaction back; the scope ends with it.
    pub async fn rollback(self) -> Result<()> {
        Ok(self.tx.rollback().await?)
    }
}
