//! The sync model: ops, results, change records and pages come from the shared `sync-model`
//! crate (L16), so the device and the server agree on every byte. This module re-exports them
//! and adds the one client-side piece: decoding a suggestion's opaque payload for display.

use serde::{Deserialize, Serialize};

pub use sync_model::changes::{
    NoteRecord, RejectedRecord, RelationRecord, SuggestionRecord, SuggestionStatus,
};
pub use sync_model::ops;
pub use sync_model::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, ConflictResolution, EntityType, Op,
    OpKind, OpOutcome, OpResult, Problem, Record, SyncCursor, SyncOp, Version,
};

/// What a suggestion proposes, decoded from [`SuggestionRecord::payload`] for the inbox.
///
/// The server stores suggestion payloads as opaque `MessagePack` (§7.4 `suggestions.payload`)
/// and `sync-model` carries them as bytes; their schema is not shared yet. Payloads that do not
/// decode as one of these shapes are shown as [`SuggestionPayload::Other`] with the record's
/// `kind`, never dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionPayload {
    /// AI filing of an inbox capture (§9.3).
    Filing {
        /// Proposed title.
        title: String,
        /// Proposed folder.
        folder: String,
        /// Proposed tags.
        tags: Vec<String>,
    },
    /// A mention to link to an existing entity or create one (§6.7 nicknames).
    EntityLinkOrCreate {
        /// The mention text (e.g. "بابا").
        mention: String,
        /// Candidate entities (id, title).
        candidates: Vec<(String, String)>,
    },
    /// A custody event below the confidence threshold (§6.12, D30).
    Custody {
        /// Document note ID (if resolved).
        document_id: Option<String>,
        /// The proposed custody line.
        line: String,
        /// Confidence.
        confidence: f64,
    },
    /// The capture resembles existing items (§9.7).
    Duplicate {
        /// Candidates.
        candidates: Vec<dedupe::DuplicateCandidate>,
    },
    /// A low-confidence relation.
    Relation {
        /// Target note ID.
        dst_id: String,
        /// Relation type.
        rel_type: String,
        /// Confidence.
        confidence: f64,
        /// One-line reason.
        reason: String,
    },
    /// A task proposed from a capture.
    Task {
        /// The proposed task line.
        line: String,
    },
    /// A kind this client cannot show.
    Other {
        /// The record's kind string.
        kind: String,
    },
}

impl SuggestionPayload {
    /// Decodes a record's payload (unknown shapes become [`SuggestionPayload::Other`]).
    pub fn decode(kind: &str, bytes: &[u8]) -> Self {
        rmp_serde::from_slice(bytes).unwrap_or_else(|_| Self::Other {
            kind: kind.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undecodable_payloads_are_kept_as_other() {
        assert_eq!(
            SuggestionPayload::decode("filing", &[0xc0]),
            SuggestionPayload::Other {
                kind: "filing".into()
            }
        );
        let bytes = rmp_serde::to_vec_named(&SuggestionPayload::Task {
            line: "- [ ] x".into(),
        })
        .expect("encodes");
        assert_eq!(
            SuggestionPayload::decode("task", &bytes),
            SuggestionPayload::Task {
                line: "- [ ] x".into()
            }
        );
    }
}
