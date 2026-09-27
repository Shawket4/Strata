//! Testkit errors.

/// Errors setting up or tearing down test fixtures.
#[derive(Debug, thiserror::Error)]
pub enum TestkitError {
    /// Database error.
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    /// Index-crate error (bootstrap, migrations, repositories).
    #[error("index: {0}")]
    Index(#[from] strata_index::IndexError),
    /// Filesystem error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Anything else.
    #[error("{0}")]
    Setup(String),
}
