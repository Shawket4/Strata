//! Sync ops, push results and change records (PLAN §7.5 "Sync").
//!
//! **Seam:** these types belong in the shared `sync-model` crate (L16), which is being built
//! concurrently and is still a placeholder. They mirror PLAN §7.5 exactly (op kinds, per-op
//! results, change records) so that switching to `sync-model` is a type swap: the op kind
//! strings are the PLAN's, and every payload carries what both sides need to apply the op
//! identically (client-generated IDs, dates, new block IDs).

use serde::{Deserialize, Serialize};

/// The kind of an outbox op (the PLAN's `kind` strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpKind {
    /// `note.create`
    #[serde(rename = "note.create")]
    NoteCreate,
    /// `note.update`
    #[serde(rename = "note.update")]
    NoteUpdate,
    /// `note.move`
    #[serde(rename = "note.move")]
    NoteMove,
    /// `note.delete`
    #[serde(rename = "note.delete")]
    NoteDelete,
    /// `capture`
    #[serde(rename = "capture")]
    Capture,
    /// `relation.add`
    #[serde(rename = "relation.add")]
    RelationAdd,
    /// `relation.remove`
    #[serde(rename = "relation.remove")]
    RelationRemove,
    /// `relation.retype`
    #[serde(rename = "relation.retype")]
    RelationRetype,
    /// `suggestion.accept`
    #[serde(rename = "suggestion.accept")]
    SuggestionAccept,
    /// `suggestion.reject`
    #[serde(rename = "suggestion.reject")]
    SuggestionReject,
    /// `entity.create`
    #[serde(rename = "entity.create")]
    EntityCreate,
    /// `relink.request`
    #[serde(rename = "relink.request")]
    RelinkRequest,
    /// `task.create`
    #[serde(rename = "task.create")]
    TaskCreate,
    /// `task.update`
    #[serde(rename = "task.update")]
    TaskUpdate,
    /// `task.complete`
    #[serde(rename = "task.complete")]
    TaskComplete,
    /// `task.cancel`
    #[serde(rename = "task.cancel")]
    TaskCancel,
    /// `task.reopen`
    #[serde(rename = "task.reopen")]
    TaskReopen,
    /// `task.delete`
    #[serde(rename = "task.delete")]
    TaskDelete,
}

impl OpKind {
    /// The PLAN spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoteCreate => "note.create",
            Self::NoteUpdate => "note.update",
            Self::NoteMove => "note.move",
            Self::NoteDelete => "note.delete",
            Self::Capture => "capture",
            Self::RelationAdd => "relation.add",
            Self::RelationRemove => "relation.remove",
            Self::RelationRetype => "relation.retype",
            Self::SuggestionAccept => "suggestion.accept",
            Self::SuggestionReject => "suggestion.reject",
            Self::EntityCreate => "entity.create",
            Self::RelinkRequest => "relink.request",
            Self::TaskCreate => "task.create",
            Self::TaskUpdate => "task.update",
            Self::TaskComplete => "task.complete",
            Self::TaskCancel => "task.cancel",
            Self::TaskReopen => "task.reopen",
            Self::TaskDelete => "task.delete",
        }
    }

    /// Parses a PLAN spelling.
    pub fn parse(s: &str) -> Option<Self> {
        ALL_KINDS.iter().copied().find(|k| k.as_str() == s)
    }

    /// Whether the op creates an item (duplicate-checked by the server, §9.7).
    pub fn is_create(self) -> bool {
        matches!(
            self,
            Self::NoteCreate | Self::Capture | Self::EntityCreate | Self::TaskCreate
        )
    }
}

/// Every op kind.
pub const ALL_KINDS: &[OpKind] = &[
    OpKind::NoteCreate,
    OpKind::NoteUpdate,
    OpKind::NoteMove,
    OpKind::NoteDelete,
    OpKind::Capture,
    OpKind::RelationAdd,
    OpKind::RelationRemove,
    OpKind::RelationRetype,
    OpKind::SuggestionAccept,
    OpKind::SuggestionReject,
    OpKind::EntityCreate,
    OpKind::RelinkRequest,
    OpKind::TaskCreate,
    OpKind::TaskUpdate,
    OpKind::TaskComplete,
    OpKind::TaskCancel,
    OpKind::TaskReopen,
    OpKind::TaskDelete,
];

/// The payload of an op. The variant determines the [`OpKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpPayload {
    /// Create a note at `path` with `content` (the server keeps the client ID).
    NoteCreate {
        /// Vault path.
        path: String,
        /// Markdown as typed (the `id` property is added on apply).
        content: String,
        /// Skip the duplicate check (§9.7 "Create anyway").
        force: bool,
    },
    /// Replace a note's content.
    NoteUpdate {
        /// New markdown.
        content: String,
    },
    /// Move/rename a note (inbound links are rewritten server-side).
    NoteMove {
        /// New vault path.
        new_path: String,
    },
    /// Soft-delete a note.
    NoteDelete,
    /// Capture text into `inbox/` (never refused, §9.7).
    Capture {
        /// The capture text.
        text: String,
        /// Inbox path (`inbox/YYYY-MM-DD-HHmmss.md`, §6.9).
        path: String,
        /// Creation time, RFC 3339 with the user's offset.
        created: String,
    },
    /// Add a relation from the entity note to `dst_path`.
    RelationAdd {
        /// Relation type (`related`, `works-at`, …).
        rel_type: String,
        /// Target note ID.
        dst_id: String,
        /// Target as a link path (how the frontmatter refers to it).
        dst_link: String,
    },
    /// Remove a relation.
    RelationRemove {
        /// Relation type.
        rel_type: String,
        /// Target note ID.
        dst_id: String,
        /// Target link path.
        dst_link: String,
    },
    /// Change a relation's type.
    RelationRetype {
        /// Current type.
        rel_type: String,
        /// New type.
        new_type: String,
        /// Target note ID.
        dst_id: String,
        /// Target link path.
        dst_link: String,
    },
    /// Accept a suggestion.
    SuggestionAccept,
    /// Reject a suggestion.
    SuggestionReject,
    /// Create a person/company/document/place note.
    EntityCreate {
        /// Kind (`person`, `company`, `document`, `place`).
        kind: String,
        /// Display name (file name).
        name: String,
        /// Aliases.
        aliases: Vec<String>,
        /// Vault path chosen locally (`people/<name>.md`, …).
        path: String,
        /// Skip the duplicate check.
        force: bool,
    },
    /// Ask the server to re-run linking for a note.
    RelinkRequest,
    /// Create a task line.
    TaskCreate {
        /// The note to add it to. For a task without a home note this is the ID of
        /// `tasks/Tasks.md`; when that note does not exist yet the client generates the ID and
        /// sets `create_home`, so the server creates the task home under the client's ID.
        note_id: String,
        /// Create `tasks/Tasks.md` with `note_id` first.
        create_home: bool,
        /// The rendered task line (with its `^t-…` block ID).
        line: String,
        /// Skip the duplicate check.
        force: bool,
    },
    /// Replace a task line.
    TaskUpdate {
        /// The new line (same block ID).
        line: String,
    },
    /// Complete a task (recurring tasks also get their next occurrence).
    TaskComplete {
        /// Completion date (`YYYY-MM-DD`, user's timezone).
        done_date: String,
        /// Block ID of the next occurrence (recurring tasks).
        next_task_id: Option<String>,
    },
    /// Cancel a task.
    TaskCancel {
        /// Cancellation date.
        date: String,
    },
    /// Reopen a done/cancelled task.
    TaskReopen,
    /// Delete a task line.
    TaskDelete,
}

impl OpPayload {
    /// The kind this payload belongs to.
    pub fn kind(&self) -> OpKind {
        match self {
            Self::NoteCreate { .. } => OpKind::NoteCreate,
            Self::NoteUpdate { .. } => OpKind::NoteUpdate,
            Self::NoteMove { .. } => OpKind::NoteMove,
            Self::NoteDelete => OpKind::NoteDelete,
            Self::Capture { .. } => OpKind::Capture,
            Self::RelationAdd { .. } => OpKind::RelationAdd,
            Self::RelationRemove { .. } => OpKind::RelationRemove,
            Self::RelationRetype { .. } => OpKind::RelationRetype,
            Self::SuggestionAccept => OpKind::SuggestionAccept,
            Self::SuggestionReject => OpKind::SuggestionReject,
            Self::EntityCreate { .. } => OpKind::EntityCreate,
            Self::RelinkRequest => OpKind::RelinkRequest,
            Self::TaskCreate { .. } => OpKind::TaskCreate,
            Self::TaskUpdate { .. } => OpKind::TaskUpdate,
            Self::TaskComplete { .. } => OpKind::TaskComplete,
            Self::TaskCancel { .. } => OpKind::TaskCancel,
            Self::TaskReopen => OpKind::TaskReopen,
            Self::TaskDelete => OpKind::TaskDelete,
        }
    }

    /// The same create op with the duplicate check skipped (`force: true`).
    #[must_use]
    pub fn forced(&self) -> Self {
        let mut p = self.clone();
        match &mut p {
            Self::NoteCreate { force, .. }
            | Self::EntityCreate { force, .. }
            | Self::TaskCreate { force, .. } => *force = true,
            _ => {}
        }
        p
    }
}

/// One op as pushed (`POST /sync/push` `ops[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushOp {
    /// Idempotency key (client ULID).
    pub op_id: String,
    /// PLAN kind string.
    pub kind: OpKind,
    /// The server entity the op addresses (note, task block, suggestion).
    pub entity_id: String,
    /// The server version the op was made against.
    pub base_version: Option<String>,
    /// Payload.
    pub payload: OpPayload,
}

/// An existing item a create resembles (`duplicate{candidates}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// ID of the existing item.
    pub id: String,
    /// Item kind (`note`, `task`, `person`, …).
    pub kind: String,
    /// Display title.
    pub title: String,
    /// Excerpt around the match.
    pub snippet: Option<String>,
    /// `exact` | `near` | `semantic`.
    pub match_level: String,
    /// Similarity in `[0, 1]`.
    pub score: f64,
}

/// The per-op result of a push.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpResult {
    /// Applied; the entity's new version.
    Applied {
        /// New version of the entity (`None` for ops without a versioned entity).
        new_version: Option<String>,
    },
    /// The edit overlaps a newer server edit (D19); nothing applied.
    Conflict {
        /// The server's current version.
        server_version: String,
        /// The server's current content, when the server sends it.
        server_content: Option<String>,
        /// How the server handled it (e.g. `needs_user`).
        resolution: String,
    },
    /// A create looks like existing items; resend with `force` to create anyway.
    Duplicate {
        /// The candidates.
        candidates: Vec<Candidate>,
    },
    /// Refused (problem details); the optimistic change is rolled back.
    Rejected {
        /// Problem type slug.
        problem_type: String,
        /// HTTP status of the problem.
        status: u16,
    },
}

/// The result of one pushed op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpOutcome {
    /// The op.
    pub op_id: String,
    /// Its result.
    pub result: OpResult,
}

/// A suggestion as the server describes it.
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
        candidates: Vec<Candidate>,
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
    /// A kind this client does not know yet.
    Other {
        /// The server's kind string.
        kind: String,
    },
}

/// One changed record (bootstrap page item or `changes` item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeRecord {
    /// A note's full content and version.
    NoteUpsert {
        /// Note ID.
        id: String,
        /// Vault path.
        path: String,
        /// Full markdown.
        content: String,
        /// Version (content hash).
        version: String,
    },
    /// A note was deleted (tombstone).
    NoteDelete {
        /// Note ID.
        id: String,
    },
    /// A suggestion.
    SuggestionUpsert {
        /// Suggestion ID.
        id: String,
        /// The note it is about.
        note_id: Option<String>,
        /// Payload.
        payload: SuggestionPayload,
        /// `pending` | `accepted` | `rejected` | `superseded`.
        status: String,
        /// Created at (RFC 3339).
        created: String,
    },
    /// A suggestion was removed.
    SuggestionDelete {
        /// Suggestion ID.
        id: String,
    },
    /// Provenance of a relation (sidecar).
    RelationMeta {
        /// Source note.
        src_id: String,
        /// Target note.
        dst_id: String,
        /// Type.
        rel_type: String,
        /// `ai` | `user`.
        by: String,
        /// AI confidence.
        confidence: Option<f64>,
        /// AI reason.
        reason: Option<String>,
    },
    /// A rejected edge (never re-added by AI).
    RejectedEdge {
        /// Source note.
        src_id: String,
        /// Target note.
        dst_id: String,
        /// Type.
        rel_type: String,
        /// When.
        at: String,
    },
    /// Cluster membership of a note.
    ClusterAssign {
        /// Note ID.
        note_id: String,
        /// Cluster ID.
        cluster_id: i64,
    },
    /// A cluster's name.
    ClusterName {
        /// Cluster ID.
        cluster_id: i64,
        /// Name.
        name: String,
    },
    /// This device's synced settings.
    DeviceSettings {
        /// Reminders on this device (§12.5b "per device").
        reminders_enabled: bool,
    },
}

/// One page of `GET /sync/bootstrap`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapPage {
    /// The server's epoch.
    pub epoch: u64,
    /// The seq incremental sync continues after.
    pub start_seq: u64,
    /// Records of this page.
    pub records: Vec<ChangeRecord>,
    /// Cursor of the next page (`None`: last page).
    pub next_cursor: Option<String>,
    /// Total pages, when the server knows it (progress display).
    pub total_pages: Option<u32>,
}

/// One change with its sequence number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// Seq (gapless per user).
    pub seq: u64,
    /// The record.
    pub record: ChangeRecord,
}

/// One page of `GET /sync/changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangesPage {
    /// Changes after `since`, in seq order.
    pub changes: Vec<Change>,
    /// The seq to ask from next.
    pub next_seq: u64,
    /// More changes are waiting.
    pub has_more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_their_plan_spelling() {
        for k in ALL_KINDS {
            assert_eq!(OpKind::parse(k.as_str()), Some(*k));
        }
        assert_eq!(ALL_KINDS.len(), 18);
        assert_eq!(OpKind::parse("note.rename"), None);
    }

    #[test]
    fn forced_sets_force_on_creates_only() {
        let c = OpPayload::NoteCreate {
            path: "notes/a.md".into(),
            content: "x".into(),
            force: false,
        };
        assert_eq!(
            c.forced(),
            OpPayload::NoteCreate {
                path: "notes/a.md".into(),
                content: "x".into(),
                force: true
            }
        );
        assert_eq!(OpPayload::NoteDelete.forced(), OpPayload::NoteDelete);
    }

    #[test]
    fn payload_encoding_is_a_tagged_named_map() {
        let bytes = rmp_serde::to_vec_named(&OpPayload::TaskCancel {
            date: "2026-09-27".into(),
        })
        .expect("encodes");
        // {"type": "task_cancel", "date": "2026-09-27"}
        let mut expected = vec![0x82, 0xa4];
        expected.extend_from_slice(b"type");
        expected.push(0xab);
        expected.extend_from_slice(b"task_cancel");
        expected.push(0xa4);
        expected.extend_from_slice(b"date");
        expected.push(0xaa);
        expected.extend_from_slice(b"2026-09-27");
        assert_eq!(bytes, expected);
    }
}
