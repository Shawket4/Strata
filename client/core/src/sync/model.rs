//! The sync model: ops, results, change records and pages come from the shared `sync-model`
//! crate (L16), so the device and the server agree on every byte. This module re-exports them
//! and adds the one client-side piece: decoding a suggestion's opaque payload for display.

use serde::{Deserialize, Serialize};

pub use sync_model::changes::{
    NoteRecord, RejectedRecord, RelationRecord, SuggestionRecord, SuggestionStatus,
};
pub use sync_model::ops;
pub use sync_model::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, ConflictResolution, EntityType, Op, OpKind,
    OpOutcome, OpResult, Problem, Record, SyncCursor, SyncOp, Version,
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
        /// Applied automatically (above the custody threshold, D30).
        #[serde(default)]
        auto_applied: bool,
        /// Documents the event may be about when ambiguous (id, title).
        #[serde(default)]
        document_choices: Vec<(String, String)>,
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
    /// The server's `duplicate` suggestion on a capture (§9.7; accepting keeps both).
    DuplicateOf {
        /// Candidates.
        candidates: Vec<ServerCandidate>,
    },
    /// The nightly sweep's `duplicates` pair (rejecting keeps both for good).
    Duplicates {
        /// First item (its note is the suggestion's note).
        a: ServerCandidate,
        /// Second item.
        b: ServerCandidate,
        /// Why the model confirmed a borderline pair.
        reason: Option<String>,
    },
    /// The server kept its version and saved the device's edit as a conflict copy (D19).
    Conflict {
        /// The op that conflicted.
        op_id: String,
        /// The conflict copy note.
        copy_id: String,
        /// Its path.
        copy_path: String,
        /// Conflicting hunks.
        hunks: u32,
    },
    /// A kind this client cannot show.
    Other {
        /// The record's kind string.
        kind: String,
    },
}

/// A candidate in the server's duplicate payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerCandidate {
    /// Existing item's note ID.
    pub id: String,
    /// Stored item ID (note ULID or task block ID).
    pub item: String,
    /// Snippet.
    pub snippet: Option<String>,
    /// Kind.
    pub kind: String,
    /// Title.
    pub title: String,
    /// `exact` | `near` | `semantic`.
    pub match_level: String,
    /// Score.
    pub score: f64,
}

#[derive(Deserialize)]
struct DuplicateOfWire {
    candidates: Vec<ServerCandidate>,
}

#[derive(Deserialize)]
struct DuplicatesWire {
    a: ServerCandidate,
    b: ServerCandidate,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct ConflictWire {
    op_id: String,
    copy_id: String,
    copy_path: String,
    hunks: u32,
}

impl SuggestionPayload {
    /// Decodes a record's payload: the server's own kinds (`duplicate`, `duplicates`,
    /// `conflict`) by their stored shapes, AI kinds by their tagged shape; anything else is
    /// [`SuggestionPayload::Other`], never dropped.
    pub fn decode(kind: &str, bytes: &[u8]) -> Self {
        let server = match kind {
            "duplicate" => rmp_serde::from_slice::<DuplicateOfWire>(bytes)
                .ok()
                .map(|d| Self::DuplicateOf {
                    candidates: d.candidates,
                }),
            "duplicates" => rmp_serde::from_slice::<DuplicatesWire>(bytes)
                .ok()
                .map(|d| Self::Duplicates {
                    a: d.a,
                    b: d.b,
                    reason: d.reason,
                }),
            "conflict" => rmp_serde::from_slice::<ConflictWire>(bytes)
                .ok()
                .map(|c| Self::Conflict {
                    op_id: c.op_id,
                    copy_id: c.copy_id,
                    copy_path: c.copy_path,
                    hunks: c.hunks,
                }),
            _ => None,
        };
        server
            .or_else(|| rmp_serde::from_slice(bytes).ok())
            .unwrap_or_else(|| Self::Other {
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
