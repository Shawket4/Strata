//! Contract schemas of the sync endpoints (PLAN §7.5 Sync, L13, L21).
//!
//! The handlers encode and decode the shared `sync-model` types directly, so the device and
//! the server can never disagree about a payload (L16). These types mirror them field for
//! field for the OpenAPI contract and the generated client; `tests` below proves, for a value
//! of every variant, that a `sync-model` value encodes to bytes that decode into the mirror
//! and re-encode to the same bytes (and back). Names are prefixed `Sync` in the contract.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::routes::documents::{CopyKind, CustodyEventKind};
use crate::routes::inbox::{ReplyAuthorDto, SuggestionStatus};
use crate::routes::notes::NoteKind;
use crate::wire::Binary;

/// A local wall-clock date-time without offset (`2026-10-01T09:00:00`).
pub type LocalDateTime = String;

/// Task priority (`normal` = no signifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncPriority {
    /// 🔺
    Highest,
    /// ⏫
    High,
    /// 🔼
    Medium,
    /// No signifier.
    Normal,
    /// 🔽
    Low,
    /// ⏬
    Lowest,
}

/// `note.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncNoteCreate {
    /// Client-generated note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Vault path.
    pub path: String,
    /// Full file content.
    pub content: String,
    /// When it was created on the device (UTC; required): the new note's `created` and
    /// `updated`.
    pub created: DateTime<Utc>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `note.update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncNoteUpdate {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// New full content.
    pub content: String,
}

/// `note.move`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncNoteMove {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// New vault path.
    pub new_path: String,
}

/// `note.delete`, `relink.request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncNoteRef {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
}

/// `capture`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncCapture {
    /// Client-generated ID of the inbox note.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Captured text.
    pub text: String,
    /// Capture time on the device (UTC; names the inbox file).
    pub created: DateTime<Utc>,
}

/// `relation.add`, `relation.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncRelationRef {
    /// Source note.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target note.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Relation key (`related`, `works-at`, …).
    #[serde(rename = "type")]
    pub relation: String,
}

/// `relation.retype`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncRelationRetype {
    /// Source note.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target note.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Current type.
    #[serde(rename = "type")]
    pub relation: String,
    /// New type.
    pub new_type: String,
}

/// Edits applied when accepting a suggestion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionEdits {
    /// Filing: title.
    pub title: Option<String>,
    /// Filing: tags.
    pub tags: Option<Vec<String>>,
    /// Filing: folder.
    pub folder: Option<String>,
    /// Entity link / correction target.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub target_id: Option<Ulid>,
    /// Aliases to add.
    pub aliases: Option<Vec<String>>,
    /// Task text.
    pub text: Option<String>,
    /// Task due date.
    pub due: Option<NaiveDate>,
    /// Task recurrence.
    pub recurrence: Option<String>,
    /// Task reminders (local date-times).
    #[schema(value_type = Option<Vec<String>>)]
    pub reminders: Option<Vec<LocalDateTime>>,
}

/// `suggestion.accept`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionAccept {
    /// Suggestion ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Optional edits.
    pub edits: Option<SyncSuggestionEdits>,
    /// When the user accepted on the device (UTC): `created`/`updated` of notes the
    /// acceptance creates.
    pub created: DateTime<Utc>,
}

/// `suggestion.reject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionReject {
    /// Suggestion ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Optional reason.
    pub reason: Option<String>,
}

/// `suggestion.reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionReply {
    /// Suggestion ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Client-generated reply ID.
    #[schema(value_type = String, format = "ulid")]
    pub reply_id: Ulid,
    /// Text.
    pub text: String,
}

/// `entity.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncEntityCreate {
    /// Client-generated ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Kind (`person`, `company`, `concept`).
    pub kind: NoteKind,
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// User fields.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// When it was created on the device (UTC; required): the new note's `created` and
    /// `updated`.
    pub created: DateTime<Utc>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `entity.patch`, `document.patch`, `place.patch`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncEntityPatch {
    /// Entity note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Fields to set.
    #[serde(default)]
    pub set: BTreeMap<String, String>,
    /// Fields to remove.
    #[serde(default)]
    pub unset: Vec<String>,
    /// Aliases to add.
    #[serde(default)]
    pub add_aliases: Vec<String>,
    /// Aliases to remove.
    #[serde(default)]
    pub remove_aliases: Vec<String>,
    /// List values to set, each replacing the key's whole value (`tags`, `aliases`, several
    /// phone numbers); an empty list removes the key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set_lists: BTreeMap<String, Vec<String>>,
}

/// `entity.merge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncEntityMerge {
    /// The entity merged away.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// The survivor.
    #[schema(value_type = String, format = "ulid")]
    pub into_id: Ulid,
}

/// `document.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncDocumentCreate {
    /// Client-generated ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// `doc-type`.
    pub doc_type: Option<String>,
    /// `copy`.
    pub copy: Option<CopyKind>,
    /// `copy-of`.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub copy_of: Option<Ulid>,
    /// Companies the document concerns.
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub companies: Vec<Ulid>,
    /// People the document concerns.
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub people: Vec<Ulid>,
    /// `expires`.
    pub expires: Option<NaiveDate>,
    /// When it was created on the device (UTC; required): the new note's `created` and
    /// `updated`.
    pub created: DateTime<Utc>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `document.custody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncDocumentCustody {
    /// Document ID.
    #[schema(value_type = String, format = "ulid")]
    pub document_id: Ulid,
    /// Event type.
    #[serde(rename = "type")]
    pub event: CustodyEventKind,
    /// Event date.
    pub at: NaiveDate,
    /// Place.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub place_id: Option<Ulid>,
    /// Person.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub person_id: Option<Ulid>,
    /// Third party.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub counterparty_id: Option<Ulid>,
    /// The user's note on the event (one line), written last on the custody line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `place.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncPlaceCreate {
    /// Client-generated ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Containing place.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
    /// Address.
    pub address: Option<String>,
    /// When it was created on the device (UTC; required): the new note's `created` and
    /// `updated`.
    pub created: DateTime<Utc>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `task.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncTaskCreate {
    /// Client-generated block ID (`t-<ulid>`).
    pub id: String,
    /// Home note (`tasks/Tasks.md` when absent).
    #[schema(value_type = Option<String>, format = "ulid")]
    pub note_id: Option<Ulid>,
    /// Description.
    pub text: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    pub start: Option<NaiveDate>,
    /// Recurrence phrase.
    pub recurrence: Option<String>,
    /// Reminders (local date-times).
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub reminders: Vec<LocalDateTime>,
    /// Priority.
    pub priority: Option<SyncPriority>,
    /// When it was created on the device (UTC; required): its date in the user's time zone
    /// picks the month heading of `tasks/Tasks.md`; a `tasks/Tasks.md` it makes gets it as
    /// `created`/`updated`.
    pub created: DateTime<Utc>,
    /// The ID a `tasks/Tasks.md` this create makes gets (the device's ID for it).
    #[serde(default)]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub home_id: Option<Ulid>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// Patch helper: absent = unchanged, `nil` = clear.
mod patch {
    pub use sync_model::ops::patch_field::{deserialize, serialize};
}

/// `task.update` (absent = unchanged, `nil` = clear).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncTaskUpdate {
    /// Block ID.
    pub id: String,
    /// New description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 📅 (`nil` clears).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    #[schema(value_type = Option<NaiveDate>)]
    pub due: Option<Option<NaiveDate>>,
    /// ⏳ (`nil` clears).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    #[schema(value_type = Option<NaiveDate>)]
    pub scheduled: Option<Option<NaiveDate>>,
    /// 🛫 (`nil` clears).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    #[schema(value_type = Option<NaiveDate>)]
    pub start: Option<Option<NaiveDate>>,
    /// 🔁 (`nil` clears).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    #[schema(value_type = Option<String>)]
    pub recurrence: Option<Option<String>>,
    /// Reminders (replace all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Vec<String>>)]
    pub reminders: Option<Vec<LocalDateTime>>,
    /// Priority (`nil` = normal).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    #[schema(value_type = Option<SyncPriority>)]
    pub priority: Option<Option<SyncPriority>>,
}

/// `task.complete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncTaskComplete {
    /// Block ID.
    pub id: String,
    /// ✅ date.
    pub done: NaiveDate,
    /// Recurring: the next occurrence's client-generated block ID.
    pub next_id: Option<String>,
}

/// `task.cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncTaskCancel {
    /// Block ID.
    pub id: String,
    /// ❌ date.
    pub date: NaiveDate,
}

/// `task.reopen`, `task.delete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncTaskRef {
    /// Block ID.
    pub id: String,
}

/// `device.settings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncDeviceSettings {
    /// The device.
    #[schema(value_type = String, format = "ulid")]
    pub device_id: Ulid,
    /// Reminders on/off (`nil` = unchanged).
    pub reminders_enabled: Option<bool>,
}

/// An operation: `{kind, payload}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum SyncOperation {
    /// Create a note.
    #[serde(rename = "note.create")]
    NoteCreate(SyncNoteCreate),
    /// Replace a note's content (3-way merged when stale, D19).
    #[serde(rename = "note.update")]
    NoteUpdate(SyncNoteUpdate),
    /// Move/rename a note.
    #[serde(rename = "note.move")]
    NoteMove(SyncNoteMove),
    /// Soft-delete a note.
    #[serde(rename = "note.delete")]
    NoteDelete(SyncNoteRef),
    /// Capture into the inbox.
    #[serde(rename = "capture")]
    Capture(SyncCapture),
    /// Add a relation.
    #[serde(rename = "relation.add")]
    RelationAdd(SyncRelationRef),
    /// Remove a relation.
    #[serde(rename = "relation.remove")]
    RelationRemove(SyncRelationRef),
    /// Retype a relation.
    #[serde(rename = "relation.retype")]
    RelationRetype(SyncRelationRetype),
    /// Accept a suggestion.
    #[serde(rename = "suggestion.accept")]
    SuggestionAccept(SyncSuggestionAccept),
    /// Reject a suggestion.
    #[serde(rename = "suggestion.reject")]
    SuggestionReject(SyncSuggestionReject),
    /// Reply to a suggestion.
    #[serde(rename = "suggestion.reply")]
    SuggestionReply(SyncSuggestionReply),
    /// Create a person, company or concept.
    #[serde(rename = "entity.create")]
    EntityCreate(SyncEntityCreate),
    /// Patch an entity.
    #[serde(rename = "entity.patch")]
    EntityPatch(SyncEntityPatch),
    /// Merge an entity into another.
    #[serde(rename = "entity.merge")]
    EntityMerge(SyncEntityMerge),
    /// Create a document.
    #[serde(rename = "document.create")]
    DocumentCreate(SyncDocumentCreate),
    /// Patch a document.
    #[serde(rename = "document.patch")]
    DocumentPatch(SyncEntityPatch),
    /// Record a custody event.
    #[serde(rename = "document.custody")]
    DocumentCustody(SyncDocumentCustody),
    /// Create a place.
    #[serde(rename = "place.create")]
    PlaceCreate(SyncPlaceCreate),
    /// Patch a place.
    #[serde(rename = "place.patch")]
    PlacePatch(SyncEntityPatch),
    /// Create a task line.
    #[serde(rename = "task.create")]
    TaskCreate(SyncTaskCreate),
    /// Edit a task line.
    #[serde(rename = "task.update")]
    TaskUpdate(SyncTaskUpdate),
    /// Complete a task.
    #[serde(rename = "task.complete")]
    TaskComplete(SyncTaskComplete),
    /// Cancel a task.
    #[serde(rename = "task.cancel")]
    TaskCancel(SyncTaskCancel),
    /// Reopen a task.
    #[serde(rename = "task.reopen")]
    TaskReopen(SyncTaskRef),
    /// Delete a task line.
    #[serde(rename = "task.delete")]
    TaskDelete(SyncTaskRef),
    /// Queue a relink job.
    #[serde(rename = "relink.request")]
    RelinkRequest(SyncNoteRef),
    /// Change this device's settings.
    #[serde(rename = "device.settings")]
    DeviceSettings(SyncDeviceSettings),
}

/// One pushed op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncOp {
    /// Idempotency key (ULID): a replay returns the stored result.
    #[schema(value_type = String, format = "ulid")]
    pub op_id: Ulid,
    /// The entity the op addresses (derived from the payload).
    pub entity_id: String,
    /// Version the device edited (edits of existing content only).
    pub base_version: Option<String>,
    /// The operation.
    pub op: SyncOperation,
}

/// `POST /sync/push` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncPushRequest {
    /// Ops, applied in order.
    pub ops: Vec<SyncOp>,
}

/// Why an op was rejected (RFC 7807 members).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncProblem {
    /// Problem type slug.
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Summary.
    pub title: String,
    /// The HTTP status the same failure has on the REST endpoint.
    pub status: u16,
    /// Details.
    pub detail: Option<String>,
}

/// How the server settled a conflict (D19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncConflictResolution {
    /// Overlapping edits: the server kept its version and saved the device's content as a
    /// conflict copy next to it.
    ConflictCopy {
        /// Conflict copy note ID.
        #[schema(value_type = String, format = "ulid")]
        note_id: Ulid,
        /// Its path.
        path: String,
        /// Its version.
        version: String,
    },
    /// The op cannot apply to the server's state; the server state stands.
    ServerKept {
        /// Why.
        reason: String,
    },
}

/// A duplicate candidate (§9.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncDuplicateCandidate {
    /// Existing item ID (note ULID or task block ID).
    pub id: String,
    /// Kind (`note`, `task`, `person`, …).
    pub kind: String,
    /// Title.
    pub title: String,
    /// Snippet.
    pub snippet: Option<String>,
    /// `exact`, `near` or `semantic`.
    pub level: String,
    /// Score.
    pub score: f32,
}

/// Result of one op, tagged by `status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SyncOpResult {
    /// Applied; `merged` when a stale edit was 3-way merged cleanly.
    Applied {
        /// The entity's new version.
        new_version: Option<String>,
        /// Whether a clean 3-way merge was needed.
        #[serde(default)]
        merged: bool,
    },
    /// Stale and not mergeable.
    Conflict {
        /// The server's current version.
        server_version: Option<String>,
        /// What the server did.
        resolution: SyncConflictResolution,
    },
    /// A create without `force` resembles existing items.
    Duplicate {
        /// Ranked candidates.
        candidates: Vec<SyncDuplicateCandidate>,
    },
    /// Invalid or not allowed (foreign IDs are `404 not_found`).
    Rejected {
        /// Why.
        problem: SyncProblem,
    },
}

/// Result of one op, keyed by its idempotency key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncOpOutcome {
    /// The op.
    #[schema(value_type = String, format = "ulid")]
    pub op_id: Ulid,
    /// Its result (replays return the stored bytes).
    pub result: SyncOpResult,
}

/// `POST /sync/push` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncPushResponse {
    /// One result per op, in request order.
    pub results: Vec<SyncOpOutcome>,
}

/// A note with its full content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncNoteRecord {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Vault path.
    pub path: String,
    /// Full file content (tags, links, relations, entity fields and tasks are derived from it
    /// with `vault-format`, as the server's indexer does).
    pub content: String,
    /// Version (`sha256:…`).
    pub version: String,
    /// Kind.
    pub kind: NoteKind,
    /// AI summary, if any.
    pub summary: Option<String>,
}

/// Who added a relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncRelationOrigin {
    /// The user.
    User,
    /// An AI job.
    Ai,
}

/// A relation with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncRelationRecord {
    /// Source.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub relation: String,
    /// Who added it.
    pub by: SyncRelationOrigin,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// AI reason.
    pub reason: Option<String>,
    /// When.
    pub created: Option<DateTime<FixedOffset>>,
}

/// A rejected edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncRejectedRecord {
    /// Source.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub relation: String,
    /// When.
    pub at: DateTime<FixedOffset>,
}

/// A reply in a suggestion thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionReplyRecord {
    /// Reply ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Text.
    pub text: String,
    /// When.
    pub at: DateTime<FixedOffset>,
    /// Who wrote it (absent: the user).
    #[serde(default)]
    pub author: ReplyAuthorDto,
}

/// A suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSuggestionRecord {
    /// ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Note it concerns.
    #[schema(value_type = Option<String>, format = "ulid")]
    pub note_id: Option<Ulid>,
    /// Kind.
    pub kind: String,
    /// Status.
    pub status: SuggestionStatus,
    /// Kind-specific payload (`MessagePack`).
    #[schema(value_type = Binary)]
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    /// Created.
    pub created: DateTime<FixedOffset>,
    /// Thread, oldest first.
    #[serde(default)]
    pub replies: Vec<SyncSuggestionReplyRecord>,
}

/// A note's cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncClusterAssignmentRecord {
    /// Note.
    #[schema(value_type = String, format = "ulid")]
    pub note_id: Ulid,
    /// Cluster.
    pub cluster_id: String,
}

/// A cluster's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncClusterNameRecord {
    /// Cluster.
    pub cluster_id: String,
    /// Name.
    pub name: String,
}

/// A user setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncSettingRecord {
    /// Key.
    pub key: String,
    /// Value.
    pub value: String,
}

/// A per-device setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncDeviceSettingRecord {
    /// Device.
    #[schema(value_type = String, format = "ulid")]
    pub device_id: Ulid,
    /// Key (`reminders_enabled`).
    pub key: String,
    /// Value (`true`/`false`).
    pub value: String,
}

/// A keep-both pair (§9.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncKeepBoth {
    /// Kind of the force-created item.
    pub kind: String,
    /// Smaller ID.
    pub a_id: String,
    /// Larger ID.
    pub b_id: String,
}

/// A full record: `{type, data}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum SyncRecord {
    /// Note.
    Note(SyncNoteRecord),
    /// Relation.
    Relation(SyncRelationRecord),
    /// Rejected edge.
    Rejected(SyncRejectedRecord),
    /// Suggestion.
    Suggestion(SyncSuggestionRecord),
    /// Cluster assignment.
    ClusterAssignment(SyncClusterAssignmentRecord),
    /// Cluster name.
    ClusterName(SyncClusterNameRecord),
    /// User setting.
    Setting(SyncSettingRecord),
    /// Device setting.
    DeviceSetting(SyncDeviceSettingRecord),
    /// Keep-both pair.
    KeepBoth(SyncKeepBoth),
}

/// Upsert or tombstone, tagged by `op`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SyncChange {
    /// The record now is this.
    Upsert {
        /// Full record.
        record: SyncRecord,
    },
    /// The record was deleted.
    Delete,
}

/// Kind of record a change is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntityType {
    /// Note.
    Note,
    /// Relation.
    Relation,
    /// Rejected edge.
    Rejected,
    /// Suggestion.
    Suggestion,
    /// Cluster assignment.
    ClusterAssignment,
    /// Cluster name.
    ClusterName,
    /// Setting.
    Setting,
    /// Device setting.
    DeviceSetting,
    /// Keep-both pair.
    KeepBoth,
}

/// One change-log entry with its payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncChangeRecord {
    /// Position in the user's log.
    pub seq: u64,
    /// Epoch.
    pub epoch: u64,
    /// Entity type.
    pub entity_type: SyncEntityType,
    /// Entity ID (`<src>:<type>:<dst>` for relations).
    pub entity_id: String,
    /// Version after the change (notes).
    pub version: Option<String>,
    /// Upsert or delete.
    pub change: SyncChange,
}

/// One page of `GET /sync/bootstrap`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncBootstrapPage {
    /// Epoch of the snapshot.
    pub epoch: u64,
    /// Log position of the snapshot (captured on the first page): pull changes after it.
    pub seq: u64,
    /// Records.
    pub records: Vec<SyncRecord>,
    /// Cursor of the next page; `nil` on the last page.
    pub next_cursor: Option<String>,
}

/// One page of `GET /sync/changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SyncChangesPage {
    /// Current epoch.
    pub epoch: u64,
    /// Changes in seq order.
    pub changes: Vec<SyncChangeRecord>,
    /// `since` for the next call.
    pub next_seq: u64,
    /// More changes are waiting.
    pub has_more: bool,
}

#[cfg(test)]
mod tests;
