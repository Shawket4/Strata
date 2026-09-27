//! Errors of the graph component, each mapped to one problem type at the API edge.

use strata_index::IndexError;
use strata_vault::VaultError;

/// One problem found in a saved map (`422 invalid_body`, one field error each).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapIssue {
    /// Stable code (`invalid_canvas`, `duplicate_id`, `dangling_edge`, `bad_color`,
    /// `bad_subpath`, `bad_size`, `invalid_file`, `unknown_file`).
    pub code: &'static str,
    /// Human-readable message (IDs and vault paths only, never note content).
    pub message: String,
}

/// Graph errors.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// Nonexistent, or outside the caller's scope (principle 7).
    #[error("not found")]
    NotFound,
    /// A query parameter is invalid.
    #[error("invalid parameter {name}: {message}")]
    InvalidParameter {
        /// Parameter.
        name: &'static str,
        /// Stable code.
        code: &'static str,
        /// Message.
        message: String,
    },
    /// A map is not a valid JSON Canvas for this vault.
    #[error("invalid map")]
    InvalidMap(Vec<MapIssue>),
    /// Vault store.
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// Index.
    #[error(transparent)]
    Index(#[from] IndexError),
    /// Similarity edges could not be computed.
    #[error("similarity: {0}")]
    Similarity(String),
}

impl From<sqlx::Error> for GraphError {
    fn from(e: sqlx::Error) -> Self {
        Self::Index(IndexError::from(e))
    }
}

/// Result alias.
pub type Result<T, E = GraphError> = std::result::Result<T, E>;
