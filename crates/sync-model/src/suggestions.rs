//! Suggestion payloads (PLAN §7.4 `suggestions.payload`, §6.7, §6.11, §6.12, §9.3, §9.7,
//! §9.8, D19): the schema of every suggestion kind the server creates, shared by the backend
//! (which writes them) and the client core (which decodes them from
//! [`SuggestionRecord`](crate::changes::SuggestionRecord)s for the inbox).
//!
//! A suggestion's payload is stored and synced as the named-map `MessagePack` of the kind's
//! struct (the kind itself is the record's `kind`, not part of the bytes):
//!
//! | kind | struct | created by |
//! |---|---|---|
//! | `duplicate` | [`DuplicatePayload`] | a capture/create that resembles existing items (§9.7) |
//! | `duplicates` | [`DuplicatesPayload`] | the nightly sweep and linking (§9.7) |
//! | `filing` | [`FilingPayload`] | inbox filing (§9.3) |
//! | `entity_link` | [`EntityLinkPayload`] | linking/filing: link a mention or create the entity (§6.7) |
//! | `custody` | [`CustodyPayload`] | a custody event not applied automatically (§6.12, D30) |
//! | `task` | [`TaskPayload`] | a task proposed from a note (§6.11) |
//! | `correction` | [`CorrectionPayload`] | a correction in words not applied automatically (§9.8) |
//! | `conflict` | [`ConflictPayload`] | a pushed `note.update` kept as a conflict copy (D19) |
//!
//! [`SuggestionPayload`] is the typed union: [`SuggestionPayload::decode`] reads stored bytes
//! of a kind, [`SuggestionPayload::to_bytes`] writes them. Serialised on its own it is an
//! internally tagged enum (`{type: "filing", …fields}`, named map). Reply threads are part of
//! the record ([`SuggestionReplyRecord`](crate::changes::SuggestionReplyRecord)), not of the
//! payload. Structs only change additively; optional fields decode as absent when missing.

use chrono::{NaiveDate, NaiveDateTime};
use dedupe::MatchLevel;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Suggestion kinds (`suggestions.kind`).
pub mod kinds {
    /// A new item resembles existing items; accepting keeps both (§9.7).
    pub const DUPLICATE: &str = "duplicate";
    /// Two stored items are duplicates; accepting merges them, rejecting keeps both (§9.7).
    pub const DUPLICATES: &str = "duplicates";
    /// Filing proposal of an inbox capture (§9.3).
    pub const FILING: &str = "filing";
    /// Link a mention to an entity or create it (§6.7, D13 = b).
    pub const ENTITY_LINK: &str = "entity_link";
    /// A custody event below the threshold, ambiguous or conflicting (§6.12, D30).
    pub const CUSTODY: &str = "custody";
    /// A task proposed from a note (§6.11).
    pub const TASK: &str = "task";
    /// A correction in words that was not applied automatically (§9.8).
    pub const CORRECTION: &str = "correction";
    /// A pushed edit kept as a conflict copy (D19).
    pub const CONFLICT: &str = "conflict";
    /// Every kind.
    pub const ALL: &[&str] = &[
        DUPLICATE,
        DUPLICATES,
        FILING,
        ENTITY_LINK,
        CUSTODY,
        TASK,
        CORRECTION,
        CONFLICT,
    ];
}

/// One item of a duplicate payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateItem {
    /// The item's note (for a task: the note holding the task line).
    pub id: Ulid,
    /// Stored item ID (note ULID or task block ID).
    pub item: String,
    /// Snippet.
    #[serde(default)]
    pub snippet: Option<String>,
    /// Item kind (`note`, `capture`, `task`, `person`, `company`, `concept`, …).
    pub kind: String,
    /// Title.
    pub title: String,
    /// How it matched.
    pub match_level: MatchLevel,
    /// Score of that level (`1.0` exact, similarity for near, cosine for semantic).
    pub score: f64,
}

/// `duplicate`: the suggestion's note resembles existing items (found when it was saved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicatePayload {
    /// The items it resembles, strongest first.
    pub candidates: Vec<DuplicateItem>,
}

/// `duplicates`: two stored items that are duplicates. Never merged automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicatesPayload {
    /// The first item (its note is the suggestion's note).
    pub a: DuplicateItem,
    /// The second item.
    pub b: DuplicateItem,
    /// The model's one-sentence reason when a borderline score was confirmed.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Which item of a `duplicates` pair accepting the suggestion keeps (§9.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatesSurvivor {
    /// [`DuplicatesPayload::a`] survives.
    A,
    /// [`DuplicatesPayload::b`] survives.
    B,
}

impl DuplicatesPayload {
    /// Whether the pair is two task lines (accepting cancels one line instead of merging).
    pub fn is_task_pair(&self) -> bool {
        self.a.kind == "task"
    }

    /// The item accepting keeps, the same on the server (which merges) and the device (which
    /// says so before the user accepts). Task pairs keep the line whose block ID sorts first
    /// and cancel the other. Notes and entities keep the one created first (`created_a`,
    /// `created_b`: each note's `created`; a note without one counts as the oldest), ties by
    /// ID; the other is merged into it.
    pub fn survivor(
        &self,
        created_a: Option<chrono::DateTime<chrono::Utc>>,
        created_b: Option<chrono::DateTime<chrono::Utc>>,
    ) -> DuplicatesSurvivor {
        let keeps_a = if self.is_task_pair() {
            self.a.item <= self.b.item
        } else {
            let id = |d: &DuplicateItem| d.item.parse::<Ulid>().unwrap_or(d.id);
            (created_a, id(&self.a)) <= (created_b, id(&self.b))
        };
        if keeps_a {
            DuplicatesSurvivor::A
        } else {
            DuplicatesSurvivor::B
        }
    }
}

/// `filing`: title, tags and folder for an inbox capture. Accepting (optionally with `title`,
/// `tags`, `folder` edits) applies them and moves the capture in one commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilingPayload {
    /// The AI decision.
    pub decision_id: Ulid,
    /// Proposed title (file name).
    pub title: String,
    /// Proposed tags.
    pub tags: Vec<String>,
    /// Proposed folder.
    pub folder: String,
}

/// `entity_link`: link a mention to an entity, or create it. Accepting adds the mention (and
/// `aliases` edits) to the entity's aliases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityLinkPayload {
    /// The AI decision.
    pub decision_id: Ulid,
    /// The mention as written.
    pub mention: String,
    /// Entity kind: `person`, `company`, `document` or `place`.
    pub kind: String,
    /// The note that mentions it.
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default)]
    pub block_id: Option<String>,
    /// The entity proposed (absent: create a new one).
    #[serde(default)]
    pub proposed: Option<Ulid>,
    /// Plausible entities (ambiguous).
    #[serde(default)]
    pub candidates: Vec<Ulid>,
    /// A nickname or kinship term (never created automatically).
    pub is_nickname: bool,
    /// Model confidence.
    pub confidence: f64,
    /// Why it is a suggestion: `ambiguous`, `nickname`, `new`, `low_confidence`, `reply`.
    pub reason: String,
}

/// One participant of a custody suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodyTarget {
    /// Mention as written.
    pub mention: String,
    /// Resolved entity.
    #[serde(default)]
    pub id: Option<Ulid>,
    /// Plausible entities when ambiguous.
    #[serde(default)]
    pub candidates: Vec<Ulid>,
}

/// `custody`: a custody event not applied automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustodyPayload {
    /// The AI decision.
    pub decision_id: Ulid,
    /// The note stating it.
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default)]
    pub block_id: Option<String>,
    /// Event type (`stored-at`, `handed-to`, …).
    pub event: String,
    /// Resolved date.
    pub date: NaiveDate,
    /// The document.
    pub document: CustodyTarget,
    /// The place.
    #[serde(default)]
    pub place: Option<CustodyTarget>,
    /// The enclosing place mention.
    #[serde(default)]
    pub place_part_of: Option<String>,
    /// The person.
    #[serde(default)]
    pub person: Option<CustodyTarget>,
    /// The third party.
    #[serde(default)]
    pub counterparty: Option<CustodyTarget>,
    /// Model confidence.
    pub confidence: f64,
    /// Why it is a suggestion: `low_confidence`, `ambiguous`, `unknown`, `conflict`, `reply`.
    pub reason: String,
    /// The span stating it.
    pub quote: String,
}

/// `task`: a task proposed from a note. Accepting writes the line (edits: `text`, `due`,
/// `recurrence`, `reminders`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskPayload {
    /// The AI decision.
    pub decision_id: Ulid,
    /// The note it came from.
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default)]
    pub block_id: Option<String>,
    /// Title.
    pub title: String,
    /// Due date.
    #[serde(default)]
    pub due: Option<NaiveDate>,
    /// Recurrence phrase (Tasks plugin language).
    #[serde(default)]
    pub recurrence: Option<String>,
    /// Reminder times (the user's time zone).
    #[serde(default)]
    pub reminders: Vec<NaiveDateTime>,
    /// Entities the task concerns (linked in the line).
    #[serde(default)]
    pub entities: Vec<Ulid>,
    /// Model confidence.
    pub confidence: f64,
}

/// One proposed fix of a correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionFix {
    /// The decision to fix.
    pub decision_id: Ulid,
    /// `repoint`, `retype` or `reject`.
    pub action: String,
    /// New target (`repoint`).
    #[serde(default)]
    pub new_target: Option<Ulid>,
    /// New relation type (`retype`).
    #[serde(default)]
    pub new_type: Option<String>,
    /// Model confidence.
    pub confidence: f64,
    /// One sentence.
    pub reason: String,
}

/// A disambiguation hint a correction stores (§9.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionHint {
    /// The entity it describes.
    pub entity: Ulid,
    /// The hint ("Ahmed at Acme = Ahmed Samir").
    pub text: String,
}

/// `correction`: a correction in words not applied automatically. Accepting applies the fixes
/// and stores the hints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionPayload {
    /// The correction decision.
    pub decision_id: Ulid,
    /// The user's words.
    pub message: String,
    /// Proposed fixes.
    #[serde(default)]
    pub fixes: Vec<CorrectionFix>,
    /// Hints to remember.
    #[serde(default)]
    pub hints: Vec<CorrectionHint>,
    /// The model's question when the reference is ambiguous.
    #[serde(default)]
    pub question: Option<String>,
}

/// `conflict`: the server kept its version of the suggestion's note and saved the device's
/// edit as a conflict copy (D19). The suggestion's ID is the op's ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictPayload {
    /// The op that conflicted.
    pub op_id: Ulid,
    /// The conflict copy note.
    pub copy_id: Ulid,
    /// Its path.
    pub copy_path: String,
    /// The version the device edited.
    pub base_version: String,
    /// The server's version that was kept.
    pub server_version: String,
    /// Number of conflicting hunks.
    pub hunks: u32,
}

/// A suggestion's payload, typed by its kind. As a value on its own it serialises as an
/// internally tagged named map (`{type: "<kind>", …}`); as stored in a suggestion it is the
/// untagged struct ([`Self::to_bytes`], [`Self::decode`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // decoded one at a time for display; plain variants match simply
pub enum SuggestionPayload {
    /// `duplicate`.
    Duplicate(DuplicatePayload),
    /// `duplicates`.
    Duplicates(DuplicatesPayload),
    /// `filing`.
    Filing(FilingPayload),
    /// `entity_link`.
    EntityLink(EntityLinkPayload),
    /// `custody`.
    Custody(CustodyPayload),
    /// `task`.
    Task(TaskPayload),
    /// `correction`.
    Correction(CorrectionPayload),
    /// `conflict`.
    Conflict(ConflictPayload),
}

/// Why stored payload bytes could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayloadError {
    /// A kind this version does not describe (show it generically, never drop it).
    #[error("unknown suggestion kind `{0}`")]
    UnknownKind(String),
    /// The bytes do not match the kind's schema.
    #[error("invalid `{kind}` payload: {message}")]
    Invalid {
        /// The kind.
        kind: String,
        /// The decoder's message.
        message: String,
    },
}

fn decode_as<T: serde::de::DeserializeOwned>(kind: &str, bytes: &[u8]) -> Result<T, PayloadError> {
    rmp_serde::from_slice(bytes).map_err(|e| PayloadError::Invalid {
        kind: kind.to_owned(),
        message: e.to_string(),
    })
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    // Named-map encoding of plain structs of strings, numbers, dates and lists cannot fail.
    rmp_serde::to_vec_named(value).unwrap_or_default()
}

impl SuggestionPayload {
    /// The suggestion kind (one of [`kinds`]).
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Duplicate(_) => kinds::DUPLICATE,
            Self::Duplicates(_) => kinds::DUPLICATES,
            Self::Filing(_) => kinds::FILING,
            Self::EntityLink(_) => kinds::ENTITY_LINK,
            Self::Custody(_) => kinds::CUSTODY,
            Self::Task(_) => kinds::TASK,
            Self::Correction(_) => kinds::CORRECTION,
            Self::Conflict(_) => kinds::CONFLICT,
        }
    }

    /// Decodes the stored payload of a suggestion of `kind`.
    pub fn decode(kind: &str, bytes: &[u8]) -> Result<Self, PayloadError> {
        Ok(match kind {
            kinds::DUPLICATE => Self::Duplicate(decode_as(kind, bytes)?),
            kinds::DUPLICATES => Self::Duplicates(decode_as(kind, bytes)?),
            kinds::FILING => Self::Filing(decode_as(kind, bytes)?),
            kinds::ENTITY_LINK => Self::EntityLink(decode_as(kind, bytes)?),
            kinds::CUSTODY => Self::Custody(decode_as(kind, bytes)?),
            kinds::TASK => Self::Task(decode_as(kind, bytes)?),
            kinds::CORRECTION => Self::Correction(decode_as(kind, bytes)?),
            kinds::CONFLICT => Self::Conflict(decode_as(kind, bytes)?),
            other => return Err(PayloadError::UnknownKind(other.to_owned())),
        })
    }

    /// The stored payload: the kind's struct as a named-map `MessagePack` (no tag).
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Duplicate(p) => encode(p),
            Self::Duplicates(p) => encode(p),
            Self::Filing(p) => encode(p),
            Self::EntityLink(p) => encode(p),
            Self::Custody(p) => encode(p),
            Self::Task(p) => encode(p),
            Self::Correction(p) => encode(p),
            Self::Conflict(p) => encode(p),
        }
    }
}

macro_rules! payload_from {
    ($($variant:ident($ty:ty)),+ $(,)?) => {
        $(impl From<$ty> for SuggestionPayload {
            fn from(p: $ty) -> Self {
                Self::$variant(p)
            }
        })+
    };
}

payload_from!(
    Duplicate(DuplicatePayload),
    Duplicates(DuplicatesPayload),
    Filing(FilingPayload),
    EntityLink(EntityLinkPayload),
    Custody(CustodyPayload),
    Task(TaskPayload),
    Correction(CorrectionPayload),
    Conflict(ConflictPayload),
);

impl crate::changes::SuggestionRecord {
    /// The record's payload, decoded by its kind.
    pub fn decode_payload(&self) -> Result<SuggestionPayload, PayloadError> {
        SuggestionPayload::decode(&self.kind, &self.payload)
    }
}
