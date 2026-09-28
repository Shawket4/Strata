//! Test-only harness (PLAN §7.1 `testkit/`, §16.1): a fresh Postgres database per test cloned
//! from a migrated template, pools for every role, user builders, temp data roots, and
//! re-exports of the deterministic clock and ID generator, and a synthetic vault generator for
//! the performance suite (§16.7).
//!
//! ```no_run
//! # async fn demo() -> Result<(), strata_testkit::TestkitError> {
//! let db = strata_testkit::TestDb::new().await?;
//! let alice = strata_testkit::TestUser::new("alice").create(&db).await?;
//! let mut tx = db.begin(alice.id).await?;
//! // … repository calls with &mut tx …
//! tx.commit().await?;
//! # Ok(()) }
//! ```

mod ai;
mod db;
mod error;
mod fixtures;
mod synthetic;

pub use ai::{FakeLlmProvider, Fixture, detached_scope};
pub use db::{
    ADMIN_URL_ENV, DEFAULT_ADMIN_URL, ROLE_PASSWORD_ENV, TEMPLATE_PREFIX, TEST_DB_PREFIX, TestDb,
    admin_url, template_name,
};
pub use error::TestkitError;
pub use fixtures::{TempDataRoot, TestUser};
pub use strata_common::{FakeClock, SequentialIdGenerator, clock::default_test_epoch};
pub use synthetic::{SyntheticConfig, SyntheticCounts, SyntheticFile, SyntheticVault};
