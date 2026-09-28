//! Error types shared across backend crates.

use std::path::PathBuf;

/// Configuration loading and validation errors. Messages name the variable and never contain
/// a configured value that could be a secret (database URLs); see `config::env`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The env file could not be read.
    #[error("cannot read env file {}: {message}", path.display())]
    Read {
        /// File path.
        path: PathBuf,
        /// OS error text.
        message: String,
    },
    /// A line of the env file is not `NAME=value` (the line itself is not repeated: it may
    /// hold a secret).
    #[error(
        "env file {}, line {line}: expected NAME=value; quote a value that contains spaces, \
         `#`, `$` or quotes in single quotes (NAME='value')",
        path.display()
    )]
    Syntax {
        /// File path.
        path: PathBuf,
        /// 1-based line number.
        line: usize,
    },
    /// The env file sets a variable twice.
    #[error("env file {}: {name} is set more than once", path.display())]
    Duplicate {
        /// File path.
        path: PathBuf,
        /// Variable name.
        name: String,
    },
    /// A variable that is not a setting (in the env file, or a `STRATA_…__…` name in the
    /// environment).
    #[error(
        "unknown variable {name} in {origin}{}",
        hint.as_deref().map(|h| format!("; {h}")).unwrap_or_default()
    )]
    Unknown {
        /// Variable name.
        name: String,
        /// `the environment` or the env file path.
        origin: String,
        /// A correction, if one is likely.
        hint: Option<String>,
    },
    /// A variable's value does not parse as its type.
    #[error("invalid {name}: {message}")]
    Value {
        /// Variable name.
        name: String,
        /// What was expected (never the value itself).
        message: String,
    },
    /// A value is out of range or inconsistent with another.
    #[error("invalid config: {0}")]
    Invalid(String),
}
