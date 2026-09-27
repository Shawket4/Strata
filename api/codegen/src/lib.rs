//! Strata's client generator (D15 = c).
//!
//! Input: the OpenAPI 3.1 document written by `strata-api` (`api/openapi.json`).
//! Output: a `generated/` module for `strata-client`:
//!
//! - `types.rs`: every component schema as Rust types, via `typify` (derives `Debug`, `Clone`,
//!   `PartialEq`, serde). `Binary` maps to `serde_bytes::ByteBuf`, `format: ulid` strings to
//!   `ulid::Ulid`, `format: date-time` to `chrono::DateTime<Utc>`.
//! - `operations.rs`: one `async fn` per operation taking path/query/header parameters and a
//!   typed body, all delegating to the client core's `Client::send`.
//! - `streams.rs`: one constructor per stream operation (`x-strata-stream`) returning a
//!   `streaming::Subscription` of the generated payload type.
//! - `mod.rs`.
//!
//! Output is deterministic (components and operations in sorted order) and formatted with
//! `prettyplease` then `rustfmt`, so `cargo fmt` leaves it untouched.

mod emit;
mod format;
mod output;
mod spec;

use std::path::PathBuf;

pub use output::{GeneratedFile, check, write};

/// Generation settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Rust path of the client core (`Client`, `Request`, `Error`, `streaming`): `crate` when
    /// generating into `strata-client` itself, `::strata_client` elsewhere.
    pub core_path: String,
    /// Shown in the `@generated` header (e.g. `api/openapi.json`).
    pub source_label: String,
    /// `rustfmt.toml` to format with; `None` uses rustfmt's defaults.
    pub rustfmt_config: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            core_path: "crate".to_owned(),
            source_label: "api/openapi.json".to_owned(),
            rustfmt_config: None,
        }
    }
}

/// Why generation failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The document does not have the expected shape.
    #[error("invalid OpenAPI document: {0}")]
    Spec(String),
    /// The document uses something the generator does not support yet.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// typify rejected a schema.
    #[error("type generation failed: {0}")]
    Types(String),
    /// Generated tokens did not parse (a generator bug).
    #[error("generated code does not parse: {0}")]
    Syntax(String),
    /// rustfmt failed or is missing.
    #[error("rustfmt: {0}")]
    Rustfmt(String),
    /// Reading or writing files failed.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: std::io::Error,
    },
}

/// Generates the `generated/` module files from an OpenAPI document.
pub fn generate(doc: &serde_json::Value, options: &Options) -> Result<Vec<GeneratedFile>, Error> {
    let api = spec::Api::parse(doc)?;
    emit::emit(&api, options)
}
