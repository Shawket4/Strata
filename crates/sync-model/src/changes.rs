//! Pulled data: change records, bootstrap and changes pages, and the client's cursor
//! (`GET /sync/bootstrap`, `GET /sync/changes`, §7.4 `change_log`, §12.4).
//!
//! Records carry **full payloads**. Everything derivable from note content — tags, links,
//! frontmatter relations, entity/document/place fields, custody state, tasks — is not sent
//! separately: the client core derives it from [`NoteRecord::content`] with `vault-format`,
//! exactly as the server's indexer does. Records exist only for state that is not in the
//! note text (relation provenance, rejections, suggestions, clusters, settings, keep-both).

use chrono::{DateTime, FixedOffset};
use dedupe::KeepBoth;
use domain::{NoteKind, RelationOrigin};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use vault_format::RelationKey;

use crate::Version;

macro_rules! entity_types {
    ($($(#[$doc:meta])* $variant:ident => $name:literal,)+) => {
        /// The kind of record a change is about (`change_log.entity_type`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum EntityType {
            $($(#[$doc])* #[serde(rename = $name)] $variant,)+
        }

        impl EntityType {
            /// Every type.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Wire and database spelling.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $name,)+ }
            }
        }
    };
}

entity_types! {
    /// A note (any kind: plain, concept, entity, document, place, task home).
    Note => "note",
    /// A relation with provenance (`relations`).
    Relation => "relation",
    /// A rejected edge (`rejected`).
    Rejected => "rejected",
    /// A suggestion.
    Suggestion => "suggestion",
    /// A note's cluster assignment.
    ClusterAssignment => "cluster_assignment",
    /// A cluster's name.
    ClusterName => "cluster_name",
    /// A user setting.
    Setting => "setting",
    /// A per-device setting.
    DeviceSetting => "device_setting",
    /// A keep-both pair (§9.7).
    KeepBoth => "keep_both",
}

/// A note with its full content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteRecord {
    /// Note ID.
    pub id: Ulid,
    /// Vault path.
    pub path: String,
    /// Full file content.
    pub content: String,
    /// Content hash.
    pub version: Version,
    /// Kind.
    pub kind: NoteKind,
    /// AI summary from the sidecar (graph hover), if any.
    pub summary: Option<String>,
}

/// A relation with provenance (§6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationRecord {
    /// Source note.
    pub src_id: Ulid,
    /// Target note.
    pub dst_id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub relation: RelationKey,
    /// Who added it.
    pub by: RelationOrigin,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// AI reason.
    pub reason: Option<String>,
    /// When it was added.
    pub created: Option<DateTime<FixedOffset>>,
}

impl RelationRecord {
    /// The record's `entity_id`: `<src>:<type>:<dst>`.
    pub fn key(&self) -> String {
        relation_key(self.src_id, self.relation, self.dst_id)
    }
}

/// The `entity_id` of a relation or rejection: `<src>:<type>:<dst>`.
pub fn relation_key(src: Ulid, relation: RelationKey, dst: Ulid) -> String {
    format!("{src}:{relation}:{dst}")
}

/// A rejected edge; the AI never re-adds it (§6.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedRecord {
    /// Source note.
    pub src_id: Ulid,
    /// Target note.
    pub dst_id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub relation: RelationKey,
    /// When.
    pub at: DateTime<FixedOffset>,
}

/// Suggestion status (`suggestions.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    /// Waiting for the user.
    Pending,
    /// Accepted.
    Accepted,
    /// Rejected.
    Rejected,
    /// Replaced by a re-proposal after a reply (§9.8).
    Superseded,
}

/// A reply in a suggestion thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionReplyRecord {
    /// Reply ID.
    pub id: Ulid,
    /// Text.
    pub text: String,
    /// When.
    pub at: DateTime<FixedOffset>,
}

/// A suggestion (payload kept as the stored MessagePack blob; its schema belongs to the
/// suggestion kind).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionRecord {
    /// Suggestion ID.
    pub id: Ulid,
    /// Note it concerns.
    pub note_id: Option<Ulid>,
    /// Suggestion kind (`filing`, `entity_link`, `task`, `custody`, `duplicate`, …).
    pub kind: String,
    /// Status.
    pub status: SuggestionStatus,
    /// Kind-specific payload (MessagePack).
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    /// When it was created.
    pub created: DateTime<FixedOffset>,
    /// Reply thread, oldest first.
    #[serde(default)]
    pub replies: Vec<SuggestionReplyRecord>,
}

/// A note's cluster (`clusters`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterAssignmentRecord {
    /// Note.
    pub note_id: Ulid,
    /// Cluster.
    pub cluster_id: String,
}

/// A cluster's name (`cluster_names`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNameRecord {
    /// Cluster.
    pub cluster_id: String,
    /// Name.
    pub name: String,
}

/// A user setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingRecord {
    /// Key.
    pub key: String,
    /// Value (setting-specific text).
    pub value: String,
}

/// A per-device setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSettingRecord {
    /// Device.
    pub device_id: Ulid,
    /// Key.
    pub key: String,
    /// Value.
    pub value: String,
}

/// A full record, adjacently tagged by its entity type: `{type: "note", data: {…}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Record {
    /// Note.
    Note(NoteRecord),
    /// Relation.
    Relation(RelationRecord),
    /// Rejected edge.
    Rejected(RejectedRecord),
    /// Suggestion.
    Suggestion(SuggestionRecord),
    /// Cluster assignment.
    ClusterAssignment(ClusterAssignmentRecord),
    /// Cluster name.
    ClusterName(ClusterNameRecord),
    /// User setting.
    Setting(SettingRecord),
    /// Device setting.
    DeviceSetting(DeviceSettingRecord),
    /// Keep-both pair.
    KeepBoth(KeepBoth),
}

impl Record {
    /// The entity type.
    pub const fn entity_type(&self) -> EntityType {
        match self {
            Self::Note(_) => EntityType::Note,
            Self::Relation(_) => EntityType::Relation,
            Self::Rejected(_) => EntityType::Rejected,
            Self::Suggestion(_) => EntityType::Suggestion,
            Self::ClusterAssignment(_) => EntityType::ClusterAssignment,
            Self::ClusterName(_) => EntityType::ClusterName,
            Self::Setting(_) => EntityType::Setting,
            Self::DeviceSetting(_) => EntityType::DeviceSetting,
            Self::KeepBoth(_) => EntityType::KeepBoth,
        }
    }

    /// The entity ID (`change_log.entity_id`).
    pub fn entity_id(&self) -> String {
        match self {
            Self::Note(r) => r.id.to_string(),
            Self::Relation(r) => r.key(),
            Self::Rejected(r) => relation_key(r.src_id, r.relation, r.dst_id),
            Self::Suggestion(r) => r.id.to_string(),
            Self::ClusterAssignment(r) => r.note_id.to_string(),
            Self::ClusterName(r) => r.cluster_id.clone(),
            Self::Setting(r) => r.key.clone(),
            Self::DeviceSetting(r) => format!("{}:{}", r.device_id, r.key),
            Self::KeepBoth(r) => format!("{}:{}:{}", r.kind, r.a_id, r.b_id),
        }
    }

    /// The record's version: notes only.
    pub fn version(&self) -> Option<&Version> {
        match self {
            Self::Note(r) => Some(&r.version),
            _ => None,
        }
    }
}

/// Upsert or tombstone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    /// The record now is this.
    Upsert {
        /// Full record.
        record: Record,
    },
    /// The record was deleted.
    Delete,
}

/// One `change_log` entry with its payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// Position in the user's log (per epoch, gapless, commit order).
    pub seq: u64,
    /// Epoch the seq belongs to.
    pub epoch: u64,
    /// Entity type.
    pub entity_type: EntityType,
    /// Entity ID.
    pub entity_id: String,
    /// Version after the change (notes), `None` otherwise and for deletes.
    pub version: Option<Version>,
    /// Upsert or delete.
    pub change: Change,
}

/// An inconsistent change record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    /// Type or ID differs from the payload.
    #[error("change {seq}: header ({header}) does not match the record ({record})")]
    HeaderMismatch {
        /// Seq.
        seq: u64,
        /// Header `type/id`.
        header: String,
        /// Record `type/id`.
        record: String,
    },
    /// Version differs from the payload's.
    #[error("change {seq}: version does not match the record")]
    VersionMismatch {
        /// Seq.
        seq: u64,
    },
}

impl ChangeRecord {
    /// An upsert whose header is derived from `record`.
    pub fn upsert(seq: u64, epoch: u64, record: Record) -> Self {
        Self {
            seq,
            epoch,
            entity_type: record.entity_type(),
            entity_id: record.entity_id(),
            version: record.version().cloned(),
            change: Change::Upsert { record },
        }
    }

    /// A tombstone.
    pub fn delete(seq: u64, epoch: u64, entity_type: EntityType, entity_id: &str) -> Self {
        Self {
            seq,
            epoch,
            entity_type,
            entity_id: entity_id.to_owned(),
            version: None,
            change: Change::Delete,
        }
    }

    /// Checks the header against the payload.
    pub fn validate(&self) -> Result<(), RecordError> {
        let Change::Upsert { record } = &self.change else {
            return Ok(());
        };
        if record.entity_type() != self.entity_type || record.entity_id() != self.entity_id {
            return Err(RecordError::HeaderMismatch {
                seq: self.seq,
                header: format!("{}/{}", self.entity_type.as_str(), self.entity_id),
                record: format!("{}/{}", record.entity_type().as_str(), record.entity_id()),
            });
        }
        if record.version() != self.version.as_ref() {
            return Err(RecordError::VersionMismatch { seq: self.seq });
        }
        Ok(())
    }
}

/// One page of `GET /sync/bootstrap`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapPage {
    /// Epoch of the snapshot.
    pub epoch: u64,
    /// Log position the snapshot corresponds to; pull changes after it.
    pub seq: u64,
    /// Records.
    pub records: Vec<Record>,
    /// Cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// One page of `GET /sync/changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangesPage {
    /// Current epoch.
    pub epoch: u64,
    /// Changes in seq order.
    pub changes: Vec<ChangeRecord>,
    /// Seq to pass as `since` next time.
    pub next_seq: u64,
    /// Whether more changes are waiting.
    pub has_more: bool,
}

/// Where a client is in the log (`sync_state.epoch`, `cursor_seq`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyncCursor {
    /// Epoch.
    pub epoch: u64,
    /// Last applied seq.
    pub seq: u64,
}

/// Why a changes page cannot be applied at a cursor.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CursorError {
    /// The server bumped the epoch (a rebuild): re-bootstrap (`410`).
    #[error("epoch changed from {expected} to {got}: re-bootstrap")]
    EpochChanged {
        /// Client epoch.
        expected: u64,
        /// Server epoch.
        got: u64,
    },
    /// A change is not after the previous one.
    #[error("change seq {seq} is not after {after}")]
    OutOfOrder {
        /// Offending seq.
        seq: u64,
        /// Previous seq.
        after: u64,
    },
    /// `next_seq` is behind the last change.
    #[error("next_seq {next_seq} is before the last change {last}")]
    NextSeqBehind {
        /// Sent `next_seq`.
        next_seq: u64,
        /// Last change seq.
        last: u64,
    },
    /// A record is inconsistent.
    #[error(transparent)]
    Record(#[from] RecordError),
}

impl SyncCursor {
    /// The cursor after a completed bootstrap.
    pub const fn after_bootstrap(page: &BootstrapPage) -> Self {
        Self {
            epoch: page.epoch,
            seq: page.seq,
        }
    }

    /// Validates `page` against this cursor and returns the cursor after applying it. The
    /// page is rejected as a whole (nothing to apply) on any error.
    pub fn advance(self, page: &ChangesPage) -> Result<Self, CursorError> {
        let mut last = self.seq;
        for change in &page.changes {
            if change.epoch != self.epoch {
                return Err(CursorError::EpochChanged {
                    expected: self.epoch,
                    got: change.epoch,
                });
            }
            if change.seq <= last {
                return Err(CursorError::OutOfOrder {
                    seq: change.seq,
                    after: last,
                });
            }
            change.validate()?;
            last = change.seq;
        }
        if page.epoch != self.epoch {
            return Err(CursorError::EpochChanged {
                expected: self.epoch,
                got: page.epoch,
            });
        }
        if page.next_seq < last {
            return Err(CursorError::NextSeqBehind {
                next_seq: page.next_seq,
                last,
            });
        }
        Ok(Self {
            epoch: self.epoch,
            seq: page.next_seq.max(self.seq),
        })
    }
}
