//! Error types shared across backend crates.

use std::path::PathBuf;

/// Configuration loading and validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The config file could not be read.
    #[error("cannot read config file {path}: {message}")]
    Read {
        /// File path.
        path: PathBuf,
        /// OS error text.
        message: String,
    },
    /// TOML syntax error, unknown key, or a value of the wrong type.
    #[error("invalid config: {0}")]
    Parse(String),
    /// An environment override could not be applied.
    #[error("invalid environment override {name}: {message}")]
    Env {
        /// Variable name.
        name: String,
        /// What was expected.
        message: String,
    },
    /// A value is out of range or inconsistent.
    #[error("invalid config: {0}")]
    Invalid(String),
}
