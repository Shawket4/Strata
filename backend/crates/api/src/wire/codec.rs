//! Bounded MessagePack encode/decode shared by the extractor, responders and WebSocket frames.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_path_to_error::Segment;

use super::{DecodeLimits, Violation, scan};

/// Why a MessagePack document could not be decoded into the expected type.
///
/// Messages never contain payload content (PLAN §15): serde messages are reduced to the
/// expectation derived from the target type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The bytes are not one well-formed MessagePack value within limits.
    #[error(transparent)]
    Structure(#[from] Violation),
    /// The value does not match the expected type.
    #[error("{message}")]
    Schema {
        /// JSON Pointer (RFC 6901) to the offending location; empty for the root.
        pointer: String,
        /// Content-free description of the mismatch.
        message: String,
    },
}

impl DecodeError {
    /// Stable machine-readable code (see `docs/WIRE_FORMAT.md`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Structure(v) => v.code(),
            Self::Schema { .. } => "schema_mismatch",
        }
    }

    /// JSON Pointer of the offending location, if known.
    pub fn pointer(&self) -> Option<&str> {
        match self {
            Self::Structure(_) => None,
            Self::Schema { pointer, .. } => Some(pointer),
        }
    }
}

/// Encodes `value` as MessagePack with structs as maps keyed by field name.
pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    rmp_serde::to_vec_named(value)
}

/// Decodes exactly one MessagePack value of type `T`, enforcing `limits` first.
///
/// Unknown map keys are ignored (unless `T` opts into `deny_unknown_fields`), so older
/// decoders accept newer payloads.
pub fn decode<T: DeserializeOwned>(bytes: &[u8], limits: &DecodeLimits) -> Result<T, DecodeError> {
    scan(bytes, limits)?;
    let mut de = rmp_serde::Deserializer::from_read_ref(bytes);
    // The scan already bounded depth; keep rmp-serde's own guard aligned with it.
    de.set_max_depth(limits.max_depth.saturating_add(1));
    serde_path_to_error::deserialize(&mut de).map_err(|err| {
        let pointer = pointer(err.path());
        let message = sanitize(err.inner());
        DecodeError::Schema { pointer, message }
    })
}

fn pointer(path: &serde_path_to_error::Path) -> String {
    let mut out = String::new();
    for segment in path.iter() {
        match segment {
            Segment::Seq { index } => {
                out.push('/');
                out.push_str(&index.to_string());
            }
            Segment::Map { key } => {
                out.push('/');
                out.push_str(&key.replace('~', "~0").replace('/', "~1"));
            }
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    out
}

/// Reduces a decode error to a message that cannot contain payload content.
fn sanitize(err: &rmp_serde::decode::Error) -> String {
    use rmp_serde::decode::Error as E;
    match err {
        E::Syntax(msg) | E::Uncategorized(msg) => sanitize_serde_message(msg),
        E::TypeMismatch(_) => "invalid type".to_owned(),
        E::OutOfRange => "number out of range".to_owned(),
        E::LengthMismatch(_) => "invalid length".to_owned(),
        E::Utf8Error(_) => "string is not valid UTF-8".to_owned(),
        E::DepthLimitExceeded => "nesting too deep".to_owned(),
        E::InvalidMarkerRead(_) | E::InvalidDataRead(_) => "truncated value".to_owned(),
    }
}

/// Keeps only type-derived parts of serde's standard messages.
///
/// `invalid type: string "secret", expected u32` → `invalid type, expected u32`;
/// `missing field `x`` is kept (the name comes from the target type); anything else becomes
/// a generic message.
fn sanitize_serde_message(msg: &str) -> String {
    const PREFIXES: [&str; 5] = [
        "invalid type",
        "invalid value",
        "invalid length",
        "unknown variant",
        "unknown field",
    ];
    for prefix in PREFIXES {
        if msg.starts_with(prefix) {
            return match msg.find(", expected ") {
                Some(idx) => format!("{prefix}{}", &msg[idx..]),
                None => prefix.to_owned(),
            };
        }
    }
    if msg.starts_with("missing field `") || msg.starts_with("duplicate field `") {
        return msg.to_owned();
    }
    "invalid value".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_messages_are_stripped_of_content() {
        let cases = [
            (
                "invalid type: string \"s3cret\", expected u32",
                "invalid type, expected u32",
            ),
            (
                "invalid value: integer `-4`, expected u8",
                "invalid value, expected u8",
            ),
            (
                "unknown variant `hexagon`, expected `circle` or `rect`",
                "unknown variant, expected `circle` or `rect`",
            ),
            ("missing field `name`", "missing field `name`"),
            ("premature end of input: s3cret", "invalid value"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize_serde_message(input), expected);
        }
    }
}
