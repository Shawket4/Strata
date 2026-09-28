//! The sync model: ops, results, change records and pages come from the shared `sync-model`
//! crate (L16), so the device and the server agree on every byte. This module re-exports them
//! and adds the one client-side piece: decoding a suggestion's payload (shared schema) for display,
//! keeping kinds this client does not know.

pub use sync_model::changes::{
    NoteRecord, RejectedRecord, RelationRecord, ReplyAuthor, SuggestionRecord,
    SuggestionReplyRecord, SuggestionStatus,
};
pub use sync_model::ops;
pub use sync_model::suggestions;
pub use sync_model::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, ConflictResolution, EntityType, Op, OpKind,
    OpOutcome, OpResult, Problem, Record, SyncCursor, SyncOp, Version,
};

/// What a suggestion proposes, decoded from [`SuggestionRecord::payload`] for the inbox with
/// the shared schema ([`sync_model::suggestions`], L16). Payloads of a kind this client does
/// not know, or that do not match their kind's schema, are shown as [`Self::Other`] with the
/// record's `kind`, never dropped.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // decoded one at a time for a view build
pub enum DecodedPayload {
    /// A known kind.
    Known(sync_model::SuggestionPayload),
    /// A kind this client cannot show.
    Other {
        /// The record's kind string.
        kind: String,
    },
}

impl DecodedPayload {
    /// Decodes a record's payload by its kind.
    pub fn decode(kind: &str, bytes: &[u8]) -> Self {
        match sync_model::SuggestionPayload::decode(kind, bytes) {
            Ok(p) => Self::Known(p),
            Err(_) => Self::Other {
                kind: kind.to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sync_model::suggestions::{SuggestionPayload, TaskPayload};

    #[test]
    fn undecodable_payloads_are_kept_as_other() {
        assert_eq!(
            DecodedPayload::decode("filing", &[0xc0]),
            DecodedPayload::Other {
                kind: "filing".into()
            }
        );
        assert_eq!(
            DecodedPayload::decode("hunch", &[0x80]),
            DecodedPayload::Other {
                kind: "hunch".into()
            }
        );
        let task = SuggestionPayload::Task(TaskPayload {
            decision_id: ulid::Ulid::from_parts(1, 1),
            source_note: ulid::Ulid::from_parts(1, 2),
            block_id: None,
            title: "Call Ahmed".into(),
            due: None,
            recurrence: None,
            reminders: Vec::new(),
            entities: Vec::new(),
            confidence: 0.8,
        });
        assert_eq!(
            DecodedPayload::decode("task", &task.to_bytes()),
            DecodedPayload::Known(task)
        );
    }
}
