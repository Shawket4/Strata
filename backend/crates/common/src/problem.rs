//! RFC 7807 problem details (PLAN §7.5 "Errors"). Encoded as MessagePack on the wire
//! (`application/problem+msgpack`); this type is serde-only and format-agnostic.
//!
//! Problem type URIs are URNs of the form `urn:strata:problem:<code>`: stable, not resolvable,
//! and independent of the (not yet chosen) public domain.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Prefix of every Strata problem type URI.
pub const PROBLEM_TYPE_PREFIX: &str = "urn:strata:problem:";

/// A known problem type: machine code, human title, and the HTTP status it is sent with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProblemType {
    /// Snake-case machine code, e.g. `duplicate_candidates`.
    pub code: &'static str,
    /// Short, stable, human-readable summary (RFC 7807 `title`).
    pub title: &'static str,
    /// HTTP status code.
    pub status: u16,
}

impl ProblemType {
    /// The `type` URI, e.g. `urn:strata:problem:not_found`.
    pub fn uri(&self) -> String {
        format!("{PROBLEM_TYPE_PREFIX}{}", self.code)
    }

    /// Looks up a known problem type by its URI.
    pub fn from_uri(uri: &str) -> Option<Self> {
        let code = uri.strip_prefix(PROBLEM_TYPE_PREFIX)?;
        ALL.iter().copied().find(|t| t.code == code)
    }
}

macro_rules! problem_types {
    ($($(#[$m:meta])* $name:ident = ($code:literal, $status:literal, $title:literal);)*) => {
        $( $(#[$m])* pub const $name: ProblemType = ProblemType { code: $code, title: $title, status: $status }; )*
        /// Every known problem type.
        pub const ALL: &[ProblemType] = &[$($name),*];
    };
}

problem_types! {
    /// Anything outside the caller's scope or nonexistent (principle 7: never 403 for foreign IDs).
    NOT_FOUND = ("not_found", 404, "Not found");
    /// `If-Match` / base version is stale (PLAN §7.2).
    VERSION_CONFLICT = ("version_conflict", 409, "Version conflict");
    /// A create matched existing items (PLAN §9.7); candidates ride in the `candidates` extension.
    DUPLICATE_CANDIDATES = ("duplicate_candidates", 409, "Possible duplicate");
    /// A note/file name is not Obsidian-safe (PLAN §6.2).
    INVALID_NAME = ("invalid_name", 422, "Invalid name");
    /// The request body failed validation or decoding limits (PLAN §7.7).
    VALIDATION_FAILED = ("validation_failed", 422, "Validation failed");
    /// Missing, expired, or revoked credentials.
    UNAUTHORIZED = ("unauthorized", 401, "Unauthorized");
    /// Authenticated but not allowed (e.g. non-admin on admin routes).
    FORBIDDEN = ("forbidden", 403, "Forbidden");
    /// Signed up, waiting for admin approval (D22).
    ACCOUNT_PENDING = ("account_pending", 403, "Account pending approval");
    /// Sign-up was rejected by an admin.
    ACCOUNT_REJECTED = ("account_rejected", 403, "Account rejected");
    /// The account was disabled by an admin.
    ACCOUNT_DISABLED = ("account_disabled", 403, "Account disabled");
    /// The account is scheduled for deletion; only export endpoints work (D25).
    ACCOUNT_DELETION_PENDING = ("account_deletion_pending", 403, "Account deletion pending");
    /// The sync epoch changed; the client must re-bootstrap (PLAN §7.5 sync).
    EPOCH_CHANGED = ("epoch_changed", 410, "Sync epoch changed");
    /// Too many requests (login, signup, capture, ask).
    RATE_LIMITED = ("rate_limited", 429, "Too many requests");
    /// Body exceeds the route's size limit.
    PAYLOAD_TOO_LARGE = ("payload_too_large", 413, "Payload too large");
    /// Unexpected server error; details are logged, never returned.
    INTERNAL = ("internal", 500, "Internal server error");
}

/// An RFC 7807 problem details object. Extension members are flattened next to the standard
/// members, exactly as the RFC specifies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    /// Problem type URI (`urn:strata:problem:<code>`).
    #[serde(rename = "type")]
    pub type_uri: String,
    /// Short summary of the problem type.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Explanation specific to this occurrence. Never contains note content (PLAN §15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// URI reference identifying this occurrence (e.g. the request path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// Extension members (e.g. `candidates`, `current_version`).
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Problem {
    /// A problem of a known type with no detail.
    pub fn new(kind: ProblemType) -> Self {
        Self {
            type_uri: kind.uri(),
            title: kind.title.to_owned(),
            status: kind.status,
            detail: None,
            instance: None,
            extensions: BTreeMap::new(),
        }
    }

    /// Sets `detail`.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets `instance`.
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Adds an extension member. Standard member names (`type`, `title`, `status`, `detail`,
    /// `instance`) are rejected so an extension can never shadow them.
    pub fn with_extension<T: Serialize>(
        mut self,
        key: &str,
        value: &T,
    ) -> Result<Self, ProblemError> {
        if matches!(key, "type" | "title" | "status" | "detail" | "instance") {
            return Err(ProblemError::ReservedMember(key.to_owned()));
        }
        let value = serde_json::to_value(value).map_err(|e| ProblemError::Encode(e.to_string()))?;
        self.extensions.insert(key.to_owned(), value);
        Ok(self)
    }

    /// The known problem type, if `type_uri` is one of [`ALL`].
    pub fn problem_type(&self) -> Option<ProblemType> {
        ProblemType::from_uri(&self.type_uri)
    }

    /// True if this problem is of type `kind`.
    pub fn is(&self, kind: ProblemType) -> bool {
        self.type_uri == kind.uri()
    }
}

impl From<ProblemType> for Problem {
    fn from(kind: ProblemType) -> Self {
        Self::new(kind)
    }
}

/// Errors building a [`Problem`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProblemError {
    /// The extension key collides with a standard RFC 7807 member.
    #[error("`{0}` is a reserved problem member")]
    ReservedMember(String),
    /// The extension value could not be represented.
    #[error("cannot encode problem extension: {0}")]
    Encode(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn known_types_have_exact_uris_and_statuses() {
        let table: Vec<(String, u16)> = ALL.iter().map(|t| (t.uri(), t.status)).collect();
        assert_eq!(
            table,
            vec![
                ("urn:strata:problem:not_found".into(), 404),
                ("urn:strata:problem:version_conflict".into(), 409),
                ("urn:strata:problem:duplicate_candidates".into(), 409),
                ("urn:strata:problem:invalid_name".into(), 422),
                ("urn:strata:problem:validation_failed".into(), 422),
                ("urn:strata:problem:unauthorized".into(), 401),
                ("urn:strata:problem:forbidden".into(), 403),
                ("urn:strata:problem:account_pending".into(), 403),
                ("urn:strata:problem:account_rejected".into(), 403),
                ("urn:strata:problem:account_disabled".into(), 403),
                ("urn:strata:problem:account_deletion_pending".into(), 403),
                ("urn:strata:problem:epoch_changed".into(), 410),
                ("urn:strata:problem:rate_limited".into(), 429),
                ("urn:strata:problem:payload_too_large".into(), 413),
                ("urn:strata:problem:internal".into(), 500),
            ]
        );
    }

    #[test]
    fn from_uri_round_trips_and_rejects_unknown() {
        for kind in ALL {
            assert_eq!(ProblemType::from_uri(&kind.uri()), Some(*kind));
        }
        assert_eq!(ProblemType::from_uri("urn:strata:problem:nope"), None);
        assert_eq!(ProblemType::from_uri("about:blank"), None);
    }

    #[test]
    fn serialises_with_flattened_extensions() {
        let problem = Problem::new(DUPLICATE_CANDIDATES)
            .with_detail("1 existing item matches")
            .with_extension("candidates", &vec![json!({"id": "01M3HBS0G00000000000000001", "score": 1.0})])
            .expect("valid extension");
        let value = serde_json::to_value(&problem).expect("serialise");
        assert_eq!(
            value,
            json!({
                "type": "urn:strata:problem:duplicate_candidates",
                "title": "Possible duplicate",
                "status": 409,
                "detail": "1 existing item matches",
                "candidates": [{"id": "01M3HBS0G00000000000000001", "score": 1.0}]
            })
        );
        let back: Problem = serde_json::from_value(value).expect("deserialise");
        assert_eq!(back, problem);
        assert!(back.is(DUPLICATE_CANDIDATES));
        assert_eq!(back.problem_type(), Some(DUPLICATE_CANDIDATES));
    }

    #[test]
    fn minimal_problem_omits_optional_members() {
        let value = serde_json::to_value(Problem::from(NOT_FOUND)).expect("serialise");
        assert_eq!(
            value,
            json!({"type": "urn:strata:problem:not_found", "title": "Not found", "status": 404})
        );
    }

    #[test]
    fn reserved_extension_keys_are_rejected() {
        let err = Problem::new(INVALID_NAME)
            .with_extension("status", &500)
            .expect_err("reserved");
        assert_eq!(err, ProblemError::ReservedMember("status".into()));
    }
}
