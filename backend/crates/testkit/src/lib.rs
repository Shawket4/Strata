//! Test-only harness (PLAN §7.1 `testkit/`, §16.1): a fresh PostgreSQL database per test cloned
//! from a migrated template, pools for every role, user builders, temp data roots, and
//! re-exports of the deterministic clock and ID generator.
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

mod db;
mod error;
mod fixtures;

pub use db::{
    ADMIN_URL_ENV, DEFAULT_ADMIN_URL, ROLE_PASSWORD_ENV, TEMPLATE_PREFIX, TEST_DB_PREFIX, TestDb,
    admin_url, template_name,
};
pub use error::TestkitError;
pub use fixtures::{TempDataRoot, TestUser};
pub use strata_common::{FakeClock, SequentialIdGenerator, clock::default_test_epoch};
