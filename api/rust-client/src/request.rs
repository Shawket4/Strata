//! Requests built by the generated operations.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;

use crate::Error;

pub use reqwest::Method;

/// Everything but RFC 3986 unreserved characters is escaped in a path segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Percent-encodes one path segment. Rejects empty, `.` and `..` segments, which URL
/// normalisation would collapse into a different path.
pub fn encode_path_segment(value: &str) -> Result<String, Error> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(Error::Param(format!(
            "path segment {value:?} is not allowed"
        )));
    }
    Ok(utf8_percent_encode(value, SEGMENT).to_string())
}

/// The string form of a scalar parameter (string, number, bool or string enum).
pub fn param_string<T: Serialize + ?Sized>(value: &T) -> Result<String, Error> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        Ok(serde_json::Value::Number(n)) => Ok(n.to_string()),
        Ok(serde_json::Value::Bool(b)) => Ok(b.to_string()),
        Ok(other) => Err(Error::Param(format!(
            "only scalar parameters are supported, got {}",
            match other {
                serde_json::Value::Array(_) => "an array",
                serde_json::Value::Object(_) => "an object",
                _ => "null",
            }
        ))),
        Err(e) => Err(Error::Param(e.to_string())),
    }
}

/// One API request (built by generated code; public so tests and hand-written transports can
/// reuse [`crate::Client::send`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) operation_id: &'static str,
    pub(crate) auth: bool,
    pub(crate) query: Vec<(&'static str, String)>,
    pub(crate) headers: Vec<(&'static str, String)>,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) content_type: Option<&'static str>,
    pub(crate) accept: Option<&'static str>,
}

impl Request {
    /// A request for `path` (absolute, already encoded, e.g. `/api/v1/health`).
    pub fn new(method: Method, path: String, operation_id: &'static str) -> Self {
        Self {
            method,
            path,
            operation_id,
            auth: false,
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
            content_type: None,
            accept: None,
        }
    }

    /// Overrides the `Accept` header (default: MessagePack).
    #[must_use]
    pub fn accept(mut self, accept: &'static str) -> Self {
        self.accept = Some(accept);
        self
    }

    /// Sends the bearer token from the client's [`crate::TokenProvider`].
    #[must_use]
    pub fn authenticated(mut self) -> Self {
        self.auth = true;
        self
    }

    /// Adds a query parameter.
    #[must_use]
    pub fn query(mut self, name: &'static str, value: String) -> Self {
        self.query.push((name, value));
        self
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, name: &'static str, value: String) -> Self {
        self.headers.push((name, value));
        self
    }

    /// Sets a MessagePack body (structs as maps with field names).
    pub fn body<T: Serialize + ?Sized>(mut self, body: &T) -> Result<Self, Error> {
        let bytes = rmp_serde::to_vec_named(body).map_err(|e| Error::Encode {
            operation: self.operation_id,
            message: e.to_string(),
        })?;
        self.body = Some(bytes);
        Ok(self)
    }

    /// Sets a raw `application/zip` body (vault import).
    #[must_use]
    pub fn zip_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self.content_type = Some(crate::ZIP);
        self
    }

    /// The operation id.
    pub fn operation_id(&self) -> &'static str {
        self.operation_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_escaped_and_dot_segments_rejected() {
        assert_eq!(
            encode_path_segment("a b/ك?#%").expect("encodes"),
            "a%20b%2F%D9%83%3F%23%25"
        );
        assert_eq!(
            encode_path_segment("01J8ZK3M4X7Q9V2B6C8D0E1F2G").expect("encodes"),
            "01J8ZK3M4X7Q9V2B6C8D0E1F2G"
        );
        for bad in ["", ".", ".."] {
            assert!(matches!(encode_path_segment(bad), Err(Error::Param(_))));
        }
    }

    #[test]
    fn scalar_parameters_are_stringified() {
        assert_eq!(param_string("x").expect("ok"), "x");
        assert_eq!(param_string(&7u32).expect("ok"), "7");
        assert_eq!(param_string(&true).expect("ok"), "true");
        assert!(matches!(param_string(&[1, 2]), Err(Error::Param(_))));
    }
}
