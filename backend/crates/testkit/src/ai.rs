//! AI test support: the fixture-replaying LLM provider (PLAN §9.1) and scopes for tests that
//! never touch the database.

use strata_common::UserId;
use strata_index::{AppDb, UserScope};

pub use strata_ai::fake::{FakeLlmProvider, Fixture};

use crate::db::DEFAULT_ADMIN_URL;
use crate::error::TestkitError;

/// A [`UserScope`] for `user` in tests that need one but no database (e.g. AI tests with the
/// in-memory usage store). It is minted from a lazily connecting pool that is never used; any
/// query through it would fail to connect. Call inside a Tokio runtime.
pub fn detached_scope(user: UserId) -> Result<UserScope, TestkitError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_lazy(DEFAULT_ADMIN_URL)?;
    let (_db, issuer) = AppDb::new(pool);
    Ok(issuer.issue(user))
}
