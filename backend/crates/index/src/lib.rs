//! Postgres index and application state (PLAN §7.1 `index/`, §7.4 schema, §5.2 RLS).
//!
//! - [`bootstrap`]: superuser setup — the three roles and per-database grants.
//! - [`MIGRATOR`] / [`migrate`]: forward-only migrations, run as `strata_owner`.
//! - [`scope`]: [`AppDb`], [`ScopeIssuer`], [`UserScope`], [`ScopedTx`] — the only way to touch
//!   user-owned rows.
//! - [`repo`]: repositories over user-owned tables, all taking `&mut ScopedTx`.
//! - [`accounts`]: [`AccountsDb`] over the global and account-bridge tables.
//! - [`schema_audit`]: the schema-enumeration check behind the CI isolation test.

pub mod accounts;
pub mod bootstrap;
pub mod error;
pub mod pool;
pub mod repo;
pub mod schema_audit;
pub mod scope;
pub mod types;

pub use accounts::AccountsDb;
pub use error::{IndexError, Result};
pub use scope::{AppDb, ScopeIssuer, ScopedTx, UserScope};

/// The embedded migrations (`backend/crates/index/migrations/`).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Applies pending migrations. `owner` must be connected as `strata_owner` (after
/// [`bootstrap::prepare_database`]). Idempotent: already-applied migrations are skipped.
pub async fn migrate(owner: &sqlx::PgPool) -> Result<()> {
    MIGRATOR.run(owner).await?;
    Ok(())
}
