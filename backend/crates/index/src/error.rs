//! Errors of the index crate.

use std::borrow::Cow;

/// SQLSTATE `insufficient_privilege`: missing grant **or** an RLS `WITH CHECK` violation.
pub const SQLSTATE_INSUFFICIENT_PRIVILEGE: &str = "42501";
/// SQLSTATE `unique_violation`.
pub const SQLSTATE_UNIQUE_VIOLATION: &str = "23505";
/// SQLSTATE `foreign_key_violation`.
pub const SQLSTATE_FOREIGN_KEY_VIOLATION: &str = "23503";
/// SQLSTATE `check_violation`.
pub const SQLSTATE_CHECK_VIOLATION: &str = "23514";

/// Errors returned by the data layer.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// Any database error (connection, SQL, constraint, permission).
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    /// Applying migrations failed.
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// `set_config('strata.user_id', …)` did not take effect (should be impossible).
    #[error("user scope was not applied to the transaction")]
    ScopeNotApplied,
    /// The client's sync epoch is stale; it must re-bootstrap (HTTP 410).
    #[error("sync epoch changed (current epoch {current})")]
    EpochChanged {
        /// The user's current epoch.
        current: i32,
    },
    /// An identifier passed to a bootstrap helper is unusable.
    #[error("invalid SQL identifier `{0}`")]
    InvalidIdentifier(String),
    /// A caller-supplied argument is out of range.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl IndexError {
    /// The PostgreSQL SQLSTATE, if this is a database error.
    pub fn sqlstate(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Db(sqlx::Error::Database(e)) => e.code(),
            _ => None,
        }
    }

    /// The server's error message, if this is a database error.
    pub fn db_message(&self) -> Option<&str> {
        match self {
            Self::Db(sqlx::Error::Database(e)) => Some(e.message()),
            _ => None,
        }
    }

    /// The violated constraint, if reported.
    pub fn constraint(&self) -> Option<&str> {
        match self {
            Self::Db(sqlx::Error::Database(e)) => e.constraint(),
            _ => None,
        }
    }

    /// True for SQLSTATE 42501 caused by a row-level security policy (`WITH CHECK` failure).
    pub fn is_rls_violation(&self) -> bool {
        self.sqlstate().as_deref() == Some(SQLSTATE_INSUFFICIENT_PRIVILEGE)
            && self
                .db_message()
                .is_some_and(|m| m.contains("row-level security policy"))
    }

    /// True for SQLSTATE 42501 caused by a missing grant.
    pub fn is_permission_denied(&self) -> bool {
        self.sqlstate().as_deref() == Some(SQLSTATE_INSUFFICIENT_PRIVILEGE)
            && self
                .db_message()
                .is_some_and(|m| m.starts_with("permission denied"))
    }

    /// True for a unique-constraint violation.
    pub fn is_unique_violation(&self) -> bool {
        self.sqlstate().as_deref() == Some(SQLSTATE_UNIQUE_VIOLATION)
    }
}

/// Result alias for the index crate.
pub type Result<T, E = IndexError> = std::result::Result<T, E>;
