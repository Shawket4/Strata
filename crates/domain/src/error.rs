//! Parse errors.

/// A string that is not a valid value of a domain enum.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {what}: {value:?}")]
pub struct ParseError {
    /// Human-readable name of the expected type, e.g. `"relation type"`.
    pub what: &'static str,
    /// The rejected input.
    pub value: String,
}

impl ParseError {
    pub(crate) fn new(what: &'static str, value: &str) -> Self {
        Self {
            what,
            value: value.to_owned(),
        }
    }
}
