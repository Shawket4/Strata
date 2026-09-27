//! Sync operations pushed by the client core (`POST /sync/push`, PLAN §7.5, §12.3).
//!
//! A [`SyncOp`] is the envelope `{op_id, entity_id, base_version, op}` where `op` is the
//! adjacently tagged [`Op`] `{kind, payload}`. The outbox row stores the same parts
//! separately (`kind`, `entity_id`, `base_version`, `payload`): use [`Op::kind`],
//! [`Op::payload_bytes`] and [`Op::from_parts`] to move between the two.
//!
//! Rules every op obeys (checked by [`SyncOp::validate`]):
//! - `entity_id` is derived from the payload ([`Op::entity_id`]): the note, capture,
//!   suggestion, entity, document, place, task block ID, relation *source* note or device.
//! - `base_version` is required exactly for the ops that edit existing content
//!   ([`OpKind::requires_base_version`]): note update/move/delete, entity/document/place
//!   patches (the entity note's version) and task edits (the version of the task **line**,
//!   `Version::of_text(line)`, so edits to other lines of the same note never conflict).
//! - `force` exists on creates that run the duplicate check ([`OpKind::accepts_force`]);
//!   captures never do, because a capture is never refused (principle 5).
//! - Creates carry client-generated ULIDs (task block IDs `t-<ulid>`), so offline-created
//!   items keep their IDs.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime};
use domain::{CopyKind, CustodyEventType, NoteKind, Priority};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use vault_format::RelationKey;

use crate::Version;

/// A pushed operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncOp {
    /// Idempotency key: replays return the stored result.
    pub op_id: Ulid,
    /// The entity the op addresses (see [`Op::entity_id`]).
    pub entity_id: String,
    /// The version the client edited, for ops that edit existing content.
    pub base_version: Option<Version>,
    /// The operation.
    pub op: Op,
}

/// Why an op envelope is inconsistent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    /// `entity_id` does not match the payload.
    #[error("entity_id {got:?} does not match the payload ({expected:?})")]
    EntityIdMismatch {
        /// Derived from the payload.
        expected: String,
        /// Sent.
        got: String,
    },
    /// The op edits existing content but has no `base_version`.
    #[error("`{0}` requires a base_version")]
    MissingBaseVersion(OpKind),
    /// The op does not take a `base_version`.
    #[error("`{0}` does not take a base_version")]
    UnexpectedBaseVersion(OpKind),
    /// The kind string is unknown.
    #[error("unknown op kind {0:?}")]
    UnknownKind(String),
    /// The payload does not decode as the kind's payload.
    #[error("invalid `{kind}` payload: {reason}")]
    InvalidPayload {
        /// Kind.
        kind: OpKind,
        /// Decoder message.
        reason: String,
    },
}

impl SyncOp {
    /// Builds an envelope, deriving `entity_id` from the payload.
    pub fn new(op_id: Ulid, base_version: Option<Version>, op: Op) -> Self {
        Self {
            op_id,
            entity_id: op.entity_id(),
            base_version,
            op,
        }
    }

    /// Checks `entity_id` and `base_version` against the op kind.
    pub fn validate(&self) -> Result<(), OpError> {
        let expected = self.op.entity_id();
        if expected != self.entity_id {
            return Err(OpError::EntityIdMismatch {
                expected,
                got: self.entity_id.clone(),
            });
        }
        let kind = self.op.kind();
        match (kind.requires_base_version(), &self.base_version) {
            (true, None) => Err(OpError::MissingBaseVersion(kind)),
            (false, Some(_)) => Err(OpError::UnexpectedBaseVersion(kind)),
            _ => Ok(()),
        }
    }
}

macro_rules! ops {
    ($(
        $(#[$doc:meta])*
        $variant:ident($payload:ty) => $name:literal, base: $base:literal, force: $force:literal;
    )+) => {
        /// Every operation, adjacently tagged: `{kind: "note.update", payload: {…}}`.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "payload")]
        pub enum Op {
            $(
                $(#[$doc])*
                #[serde(rename = $name)]
                $variant($payload),
            )+
        }

        /// The op kind alone (outbox `kind` column).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum OpKind {
            $(
                $(#[$doc])*
                $variant,
            )+
        }

        impl OpKind {
            /// Every kind, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire spelling (`note.update`).
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $name,)+ }
            }

            /// Whether the op edits existing content and must carry `base_version`.
            pub const fn requires_base_version(self) -> bool {
                match self { $(Self::$variant => $base,)+ }
            }

            /// Whether the op is a create that runs the duplicate check and takes `force`.
            pub const fn accepts_force(self) -> bool {
                match self { $(Self::$variant => $force,)+ }
            }
        }

        impl FromStr for OpKind {
            type Err = OpError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($name => Ok(Self::$variant),)+
                    _ => Err(OpError::UnknownKind(s.to_owned())),
                }
            }
        }

        impl Op {
            /// The kind of this op.
            pub const fn kind(&self) -> OpKind {
                match self { $(Self::$variant(_) => OpKind::$variant,)+ }
            }

            /// The payload alone as named-map MessagePack (outbox `payload` column).
            pub fn payload_bytes(&self) -> Vec<u8> {
                let encoded = match self {
                    $(Self::$variant(p) => rmp_serde::to_vec_named(p),)+
                };
                encoded.expect("op payloads are plain data and always encode")
            }

            /// Rebuilds an op from the outbox columns.
            pub fn from_parts(kind: OpKind, payload: &[u8]) -> Result<Self, OpError> {
                let invalid = |e: rmp_serde::decode::Error| OpError::InvalidPayload {
                    kind,
                    reason: e.to_string(),
                };
                match kind {
                    $(OpKind::$variant => rmp_serde::from_slice(payload).map(Self::$variant).map_err(invalid),)+
                }
            }
        }
    };
}

ops! {
    /// Create a note (client-generated ULID).
    NoteCreate(NoteCreate) => "note.create", base: false, force: true;
    /// Replace a note's content.
    NoteUpdate(NoteUpdate) => "note.update", base: true, force: false;
    /// Move/rename a note (inbound links are rewritten by the server).
    NoteMove(NoteMove) => "note.move", base: true, force: false;
    /// Soft-delete a note.
    NoteDelete(NoteRef) => "note.delete", base: true, force: false;
    /// Capture into `inbox/` (never refused; duplicates ride on the inbox suggestion).
    Capture(Capture) => "capture", base: false, force: false;
    /// Add a relation (by: user).
    RelationAdd(RelationRef) => "relation.add", base: false, force: false;
    /// Remove a relation (recorded in `rejected` if it was by: ai).
    RelationRemove(RelationRef) => "relation.remove", base: false, force: false;
    /// Change a relation's type.
    RelationRetype(RelationRetype) => "relation.retype", base: false, force: false;
    /// Accept a suggestion, optionally with edits.
    SuggestionAccept(SuggestionAccept) => "suggestion.accept", base: false, force: false;
    /// Reject a suggestion.
    SuggestionReject(SuggestionReject) => "suggestion.reject", base: false, force: false;
    /// Reply to a suggestion (the AI re-proposes, §9.8).
    SuggestionReply(SuggestionReply) => "suggestion.reply", base: false, force: false;
    /// Create a person, company or concept.
    EntityCreate(EntityCreate) => "entity.create", base: false, force: true;
    /// Edit an entity's user fields and aliases.
    EntityPatch(EntityPatch) => "entity.patch", base: true, force: false;
    /// Merge an entity into another.
    EntityMerge(EntityMerge) => "entity.merge", base: false, force: false;
    /// Create a document.
    DocumentCreate(DocumentCreate) => "document.create", base: false, force: true;
    /// Edit a document's user fields.
    DocumentPatch(EntityPatch) => "document.patch", base: true, force: false;
    /// Record a manual custody event.
    DocumentCustody(DocumentCustody) => "document.custody", base: false, force: false;
    /// Create a place.
    PlaceCreate(PlaceCreate) => "place.create", base: false, force: true;
    /// Edit a place's user fields.
    PlacePatch(EntityPatch) => "place.patch", base: true, force: false;
    /// Create a task line.
    TaskCreate(TaskCreate) => "task.create", base: false, force: true;
    /// Edit a task line.
    TaskUpdate(TaskUpdate) => "task.update", base: true, force: false;
    /// Complete a task (recurring: also writes the next occurrence).
    TaskComplete(TaskComplete) => "task.complete", base: true, force: false;
    /// Cancel a task.
    TaskCancel(TaskCancel) => "task.cancel", base: true, force: false;
    /// Reopen a done or cancelled task.
    TaskReopen(TaskRef) => "task.reopen", base: true, force: false;
    /// Delete a task line.
    TaskDelete(TaskRef) => "task.delete", base: true, force: false;
    /// Queue a relink job for a note.
    RelinkRequest(NoteRef) => "relink.request", base: false, force: false;
    /// Change this device's settings.
    DeviceSettings(DeviceSettings) => "device.settings", base: false, force: false;
}

impl fmt::Display for OpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for OpKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for OpKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Op {
    /// The entity this op addresses (`SyncOp::entity_id`).
    pub fn entity_id(&self) -> String {
        match self {
            Self::NoteCreate(p) => p.id.to_string(),
            Self::NoteUpdate(p) => p.id.to_string(),
            Self::NoteMove(p) => p.id.to_string(),
            Self::NoteDelete(p) | Self::RelinkRequest(p) => p.id.to_string(),
            Self::Capture(p) => p.id.to_string(),
            Self::RelationAdd(p) | Self::RelationRemove(p) => p.src_id.to_string(),
            Self::RelationRetype(p) => p.src_id.to_string(),
            Self::SuggestionAccept(p) => p.id.to_string(),
            Self::SuggestionReject(p) => p.id.to_string(),
            Self::SuggestionReply(p) => p.id.to_string(),
            Self::EntityCreate(p) => p.id.to_string(),
            Self::EntityPatch(p) | Self::DocumentPatch(p) | Self::PlacePatch(p) => p.id.to_string(),
            Self::EntityMerge(p) => p.id.to_string(),
            Self::DocumentCreate(p) => p.id.to_string(),
            Self::DocumentCustody(p) => p.document_id.to_string(),
            Self::PlaceCreate(p) => p.id.to_string(),
            Self::TaskCreate(p) => p.id.clone(),
            Self::TaskUpdate(p) => p.id.clone(),
            Self::TaskComplete(p) => p.id.clone(),
            Self::TaskCancel(p) => p.id.clone(),
            Self::TaskReopen(p) | Self::TaskDelete(p) => p.id.clone(),
            Self::DeviceSettings(p) => p.device_id.to_string(),
        }
    }

    /// `force` of a create op; `false` for every other op.
    pub const fn force(&self) -> bool {
        match self {
            Self::NoteCreate(p) => p.force,
            Self::EntityCreate(p) => p.force,
            Self::DocumentCreate(p) => p.force,
            Self::PlaceCreate(p) => p.force,
            Self::TaskCreate(p) => p.force,
            _ => false,
        }
    }
}

/// Serde helper for patch fields: absent = unchanged, `nil` = clear, value = set.
pub mod patch_field {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serialises `Some(None)` as `nil` and `Some(Some(v))` as `v` (`None` is skipped).
    #[expect(clippy::ref_option, reason = "serde `with` helpers receive `&Option<T>`")]
    pub fn serialize<T: Serialize, S: Serializer>(
        value: &Option<Option<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    /// Reads a present field as `Some(value-or-None)`.
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

/// `note.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteCreate {
    /// Client-generated note ID.
    pub id: Ulid,
    /// Vault path (`notes/Pricing experiments.md`).
    pub path: String,
    /// Full file content.
    pub content: String,
    /// Create even if duplicates exist (records keep-both pairs).
    #[serde(default)]
    pub force: bool,
}

/// `note.update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteUpdate {
    /// Note ID.
    pub id: Ulid,
    /// New full file content.
    pub content: String,
}

/// `note.move`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMove {
    /// Note ID.
    pub id: Ulid,
    /// New vault path.
    pub new_path: String,
}

/// An op that only names a note (`note.delete`, `relink.request`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteRef {
    /// Note ID.
    pub id: Ulid,
}

/// `capture`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    /// Client-generated ID of the inbox note.
    pub id: Ulid,
    /// Captured text.
    pub text: String,
    /// When it was captured on the device (names the inbox file, §6.9).
    pub created: DateTime<FixedOffset>,
}

/// `relation.add` / `relation.remove`: the edge `src --type--> dst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationRef {
    /// Source note (the relation is stored on it).
    pub src_id: Ulid,
    /// Target note.
    pub dst_id: Ulid,
    /// Relation key.
    #[serde(rename = "type")]
    pub relation: RelationKey,
}

/// `relation.retype`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationRetype {
    /// Source note.
    pub src_id: Ulid,
    /// Target note.
    pub dst_id: Ulid,
    /// Current type.
    #[serde(rename = "type")]
    pub relation: RelationKey,
    /// New type.
    pub new_type: RelationKey,
}

/// User edits applied when accepting a suggestion (every field optional).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionEdits {
    /// Filing: title.
    pub title: Option<String>,
    /// Filing: tags.
    pub tags: Option<Vec<String>>,
    /// Filing: destination folder.
    pub folder: Option<String>,
    /// Entity link / correction: the entity (or note) to point at instead.
    pub target_id: Option<Ulid>,
    /// Entity link: mention spellings to add as aliases (both scripts, §6.7).
    pub aliases: Option<Vec<String>>,
    /// Task: text.
    pub text: Option<String>,
    /// Task: due date.
    pub due: Option<NaiveDate>,
    /// Task: recurrence phrase.
    pub recurrence: Option<String>,
    /// Task: reminders.
    pub reminders: Option<Vec<NaiveDateTime>>,
}

/// `suggestion.accept`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionAccept {
    /// Suggestion ID.
    pub id: Ulid,
    /// Optional edits.
    pub edits: Option<SuggestionEdits>,
}

/// `suggestion.reject`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionReject {
    /// Suggestion ID.
    pub id: Ulid,
    /// Optional reason.
    pub reason: Option<String>,
}

/// `suggestion.reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionReply {
    /// Suggestion ID.
    pub id: Ulid,
    /// Client-generated reply ID.
    pub reply_id: Ulid,
    /// Reply text ("no, the Petrol Arrows one").
    pub text: String,
}

/// `entity.create` (person, company or concept).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityCreate {
    /// Client-generated ID.
    pub id: Ulid,
    /// Kind (`person`, `company`, `concept`).
    pub kind: NoteKind,
    /// Name (file name).
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// User fields (`role`, `industry`, `phone`, …).
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `entity.patch`, `document.patch`, `place.patch`: user fields and aliases.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityPatch {
    /// Entity note ID.
    pub id: Ulid,
    /// Fields to set (scalar text).
    #[serde(default)]
    pub set: BTreeMap<String, String>,
    /// Fields to remove.
    #[serde(default)]
    pub unset: Vec<String>,
    /// Aliases to add (kept once).
    #[serde(default)]
    pub add_aliases: Vec<String>,
    /// Aliases to remove.
    #[serde(default)]
    pub remove_aliases: Vec<String>,
}

/// `entity.merge`: `id` (the loser) into `into_id` (the survivor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityMerge {
    /// The entity merged away.
    pub id: Ulid,
    /// The survivor.
    pub into_id: Ulid,
}

/// `document.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentCreate {
    /// Client-generated ID.
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// `doc-type` (free text allowed).
    pub doc_type: Option<String>,
    /// `copy`.
    pub copy: Option<CopyKind>,
    /// `copy-of` another document.
    pub copy_of: Option<Ulid>,
    /// Companies the document concerns.
    #[serde(default)]
    pub companies: Vec<Ulid>,
    /// People the document concerns.
    #[serde(default)]
    pub people: Vec<Ulid>,
    /// `expires`.
    pub expires: Option<NaiveDate>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `document.custody`: a manual custody event (§6.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentCustody {
    /// Document ID.
    pub document_id: Ulid,
    /// Event type.
    #[serde(rename = "type")]
    pub event: CustodyEventType,
    /// Event date (resolved, never relative).
    pub at: NaiveDate,
    /// Place involved.
    pub place_id: Option<Ulid>,
    /// Person involved.
    pub person_id: Option<Ulid>,
    /// Third party involved.
    pub counterparty_id: Option<Ulid>,
}

/// `place.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceCreate {
    /// Client-generated ID.
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Containing place (`part-of`).
    pub parent_id: Option<Ulid>,
    /// Address (user-entered only).
    pub address: Option<String>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `task.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreate {
    /// Client-generated block ID (`t-<ulid>`).
    pub id: String,
    /// Home note; `None` = `tasks/Tasks.md`.
    pub note_id: Option<Ulid>,
    /// Description.
    pub text: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    pub start: Option<NaiveDate>,
    /// Recurrence phrase (kept verbatim).
    pub recurrence: Option<String>,
    /// Reminders (local wall-clock times).
    #[serde(default)]
    pub reminders: Vec<NaiveDateTime>,
    /// Priority.
    pub priority: Option<Priority>,
    /// Create even if duplicates exist.
    #[serde(default)]
    pub force: bool,
}

/// `task.update`: absent fields are unchanged, `nil` clears.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskUpdate {
    /// Block ID.
    pub id: String,
    /// New description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 📅
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch_field")]
    pub due: Option<Option<NaiveDate>>,
    /// ⏳
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch_field")]
    pub scheduled: Option<Option<NaiveDate>>,
    /// 🛫
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch_field")]
    pub start: Option<Option<NaiveDate>>,
    /// 🔁
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch_field")]
    pub recurrence: Option<Option<String>>,
    /// Reminders (replaces all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminders: Option<Vec<NaiveDateTime>>,
    /// Priority (`nil` = normal).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch_field")]
    pub priority: Option<Option<Priority>>,
}

/// `task.complete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskComplete {
    /// Block ID.
    pub id: String,
    /// ✅ date.
    pub done: NaiveDate,
    /// Recurring tasks: client-generated block ID of the next occurrence, so the device and
    /// the server write the same line.
    pub next_id: Option<String>,
}

/// `task.cancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCancel {
    /// Block ID.
    pub id: String,
    /// ❌ date.
    pub date: NaiveDate,
}

/// An op that only names a task (`task.reopen`, `task.delete`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRef {
    /// Block ID.
    pub id: String,
}

/// `device.settings`: per-device settings (§12.5b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSettings {
    /// The device.
    pub device_id: Ulid,
    /// Reminders on/off for this device (`None` = unchanged).
    pub reminders_enabled: Option<bool>,
}
