//! Per-note sidecar metadata: `.meta/notes/<id>.json` (PLAN §6.5).
//!
//! JSON on disk, pretty-printed with two-space indentation and a trailing newline. Unknown
//! fields are kept (in `extra`) so older code never drops data written by newer code.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use domain::RelationType;

use crate::frontmatter::RelationKey;

/// Who created an edge (`ai` | `user`; shared vocabulary, PLAN L16).
pub use domain::RelationOrigin as By;

/// Provenance of one relation stored in the note's frontmatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SidecarRelation {
    /// Relation type.
    #[serde(rename = "type")]
    pub kind: RelationKey,
    /// Target note ID.
    pub target_id: Ulid,
    /// Who added it.
    pub by: By,
    /// AI confidence 0–1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// AI reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `provider/model` that proposed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// When it was added.
    pub created: DateTime<FixedOffset>,
}

/// A relation the user rejected; the AI must never re-add it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedRelation {
    /// Rejected type.
    #[serde(rename = "type")]
    pub kind: RelationKey,
    /// Target note ID.
    pub target_id: Ulid,
    /// When it was rejected.
    pub at: DateTime<FixedOffset>,
}

/// A "keep both" decision for a duplicate pair (PLAN §9.7), mirrored here so it survives
/// index rebuilds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeepBoth {
    /// The other item's note ID.
    pub other_id: Ulid,
    /// When the user chose to keep both.
    pub at: DateTime<FixedOffset>,
}

/// `.meta/notes/<id>.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteSidecar {
    /// The note ID (matches frontmatter `id` and the file name).
    pub id: Ulid,
    /// One-paragraph AI summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Relation provenance.
    #[serde(default)]
    pub relations: Vec<SidecarRelation>,
    /// Rejected relations.
    #[serde(default)]
    pub rejected: Vec<RejectedRelation>,
    /// Duplicate pairs the user kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_both: Vec<KeepBoth>,
    /// `sha256:<hex>` of the note content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Content hash at the last linking run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_linked_hash: Option<String>,
    /// Fields this version does not know.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl NoteSidecar {
    /// An empty sidecar for `id`.
    pub fn new(id: Ulid) -> Self {
        Self {
            id,
            summary: None,
            relations: Vec::new(),
            rejected: Vec::new(),
            keep_both: Vec::new(),
            content_hash: None,
            last_linked_hash: None,
            extra: BTreeMap::new(),
        }
    }

    /// The vault path of the sidecar for `id`.
    pub fn path_for(id: Ulid) -> String {
        format!(".meta/notes/{id}.json")
    }

    /// Parses the JSON file.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialises to the on-disk form.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        to_pretty_json(self)
    }

    /// Whether the AI may add `kind` to `target`: not if that triple was rejected, and not as
    /// `related` if any type to that target was rejected (PLAN §6.5).
    pub fn is_blocked(&self, kind: RelationKey, target: Ulid) -> bool {
        self.rejected.iter().any(|r| {
            r.target_id == target
                && (r.kind == kind || kind == RelationKey::Note(RelationType::Related))
        })
    }
}

pub(crate) fn to_pretty_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string_pretty(value)?;
    s.push('\n');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{
  "id": "01J8ZK3M4X7Q9W2E5R6T8Y0V1H",
  "summary": "One-paragraph AI summary used for linking and graph hover.",
  "relations": [
    {
      "type": "contradicts",
      "target_id": "01J8ZK9M4X7Q9W2E5R6T8Y0V1H",
      "by": "ai",
      "confidence": 0.72,
      "reason": "States a flat 10% discount, while target caps discounts at 5%.",
      "model": "anthropic/claude",
      "created": "2026-09-27T15:12:00+03:00"
    }
  ],
  "rejected": [
    {
      "type": "related",
      "target_id": "01J8ZKBM4X7Q9W2E5R6T8Y0V1H",
      "at": "2026-09-27T16:00:00+03:00"
    }
  ],
  "content_hash": "sha256:abc",
  "last_linked_hash": "sha256:def",
  "future_field": {
    "x": 1
  }
}
"#;

    #[test]
    fn round_trips_byte_exact() {
        let s = NoteSidecar::from_json(EXAMPLE);
        assert!(s.is_ok(), "{s:?}");
        let s = s.unwrap_or_else(|_| NoteSidecar::new(Ulid::nil()));
        assert_eq!(
            s.relations[0].kind,
            RelationKey::Note(RelationType::Contradicts)
        );
        assert_eq!(s.relations[0].confidence, Some(0.72));
        assert_eq!(s.extra.len(), 1);
        assert_eq!(s.to_json().ok().as_deref(), Some(EXAMPLE));
        assert_eq!(
            NoteSidecar::path_for(s.id),
            ".meta/notes/01J8ZK3M4X7Q9W2E5R6T8Y0V1H.json"
        );
    }

    #[test]
    fn rejection_rules() {
        let s = NoteSidecar::from_json(EXAMPLE).unwrap_or_else(|_| NoteSidecar::new(Ulid::nil()));
        let rejected = s.rejected[0].target_id;
        assert!(s.is_blocked(RelationKey::Note(RelationType::Related), rejected));
        assert!(!s.is_blocked(RelationKey::Note(RelationType::Supports), rejected));
        let mut s2 = s.clone();
        s2.rejected[0].kind = RelationKey::Note(RelationType::Supports);
        assert!(s2.is_blocked(RelationKey::Note(RelationType::Supports), rejected));
        assert!(s2.is_blocked(RelationKey::Note(RelationType::Related), rejected));
        assert!(!s2.is_blocked(RelationKey::Note(RelationType::Related), s.id));
    }

    #[test]
    fn minimal_and_invalid() {
        let s = NoteSidecar::from_json(r#"{"id":"01J8ZK3M4X7Q9W2E5R6T8Y0V1H"}"#);
        assert_eq!(s.ok().map(|s| s.relations.len()), Some(0));
        assert!(NoteSidecar::from_json(r#"{"id":"nope"}"#).is_err());
        assert!(
            NoteSidecar::from_json(
                r#"{"id":"01J8ZK3M4X7Q9W2E5R6T8Y0V1H","relations":[{"type":"likes"}]}"#
            )
            .is_err()
        );
        let empty = NoteSidecar::new(Ulid::nil()).to_json().unwrap_or_default();
        assert_eq!(
            empty,
            "{\n  \"id\": \"00000000000000000000000000\",\n  \"relations\": [],\n  \"rejected\": []\n}\n"
        );
    }
}
