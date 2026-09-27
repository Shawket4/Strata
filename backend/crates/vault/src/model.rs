//! Values returned by the vault service (mapped to API DTOs by `strata-api`).

use chrono::{DateTime, NaiveDate, Utc};
use domain::NoteKind;
use strata_common::NoteId;
use vault_format::{Document, PropertyValue};

use crate::error::Candidate;

/// One frontmatter property, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Key.
    pub key: String,
    /// Value.
    pub value: PropertyValue,
}

/// A note with its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteView {
    /// ID.
    pub id: NoteId,
    /// Vault path (`.trash/…` while deleted).
    pub path: String,
    /// Display title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// The whole file.
    pub content: String,
    /// Version (content hash) for `If-Match`.
    pub version: String,
    /// Parsed frontmatter properties in file order (empty when the frontmatter is not
    /// readable).
    pub properties: Vec<Property>,
    /// Frontmatter `created`.
    pub created: DateTime<Utc>,
    /// Frontmatter `updated`.
    pub updated: DateTime<Utc>,
    /// In the trash.
    pub trashed: bool,
}

impl NoteView {
    /// Builds the view of the note at `path` from its text.
    pub fn from_text(
        id: NoteId,
        path: &str,
        text: String,
        created: DateTime<Utc>,
        updated: DateTime<Utc>,
        trashed: bool,
    ) -> Self {
        let doc = Document::parse(&text);
        let properties = doc
            .frontmatter()
            .filter(|f| f.error().is_none())
            .map(|f| {
                f.keys()
                    .filter_map(|k| {
                        f.get(k).map(|v| Property {
                            key: k.to_owned(),
                            value: v.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            id,
            path: path.to_owned(),
            title: crate::derive::title_of(path, &doc),
            kind: crate::derive::kind_of(&doc),
            version: crate::fsio::version_of(text.as_bytes()),
            content: text,
            properties,
            created,
            updated,
            trashed,
        }
    }
}

/// One entry of the vault tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEntry {
    /// A folder.
    Folder {
        /// Path.
        path: String,
    },
    /// A note.
    Note {
        /// Path.
        path: String,
        /// ID.
        id: NoteId,
        /// Title.
        title: String,
        /// Kind.
        kind: NoteKind,
        /// Last update.
        updated: DateTime<Utc>,
    },
    /// Any other file (attachment, canvas).
    File {
        /// Path.
        path: String,
    },
}

/// A note that links to or relates to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Backlink {
    /// Source note.
    pub source_id: NoteId,
    /// Source path.
    pub source_path: String,
    /// Source title.
    pub source_title: String,
    /// `link`, `embed`, or the relation type.
    pub kind: String,
    /// Relation provenance (`user` / `ai`), for relations.
    pub by: Option<String>,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// Heading anchor, for body links.
    pub anchor: Option<String>,
    /// Block anchor, for body links.
    pub block_id: Option<String>,
}

/// A group of backlinks of one kind.
#[derive(Debug, Clone, PartialEq)]
pub struct BacklinkGroup {
    /// `link`, `embed` or a relation type.
    pub kind: String,
    /// Entries, by source path.
    pub items: Vec<Backlink>,
}

/// One revision of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// Commit ID.
    pub commit: String,
    /// Commit message.
    pub message: String,
    /// `user`, `ai` or `system`.
    pub author: String,
    /// Commit time.
    pub at: DateTime<Utc>,
    /// The note's path in that commit.
    pub path: String,
    /// `added`, `modified`, `renamed` or `deleted`.
    pub change: String,
}

/// A note at a revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteAtRevision {
    /// Commit ID.
    pub commit: String,
    /// Path in that commit.
    pub path: String,
    /// Content in that commit.
    pub content: String,
    /// Version (hash) of that content.
    pub version: String,
}

/// A keyword search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// Note.
    pub id: NoteId,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Rank.
    pub score: f32,
    /// The first body line matching the query.
    pub snippet: Option<String>,
}

/// The result of a capture.
#[derive(Debug, Clone, PartialEq)]
pub struct Captured {
    /// The inbox note.
    pub note: NoteView,
    /// Likely duplicates (the capture was saved anyway).
    pub duplicates: Vec<Candidate>,
    /// The duplicate suggestion created, if any.
    pub suggestion: Option<strata_common::SuggestionId>,
}

/// Documents nested in a place and the place hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceTree {
    /// Ancestors, nearest first.
    pub ancestors: Vec<NoteId>,
    /// Direct children.
    pub children: Vec<NoteId>,
    /// Documents here or in any nested place.
    pub documents: Vec<NoteId>,
}

/// Task views of `GET /tasks`.
pub use strata_index::repo::vault::TaskView;

/// A task and its reminders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    /// The task row.
    pub task: strata_index::repo::tasks::Task,
    /// Note path.
    pub note_path: String,
    /// The raw line.
    pub line: String,
    /// The task's version (hash of its line) for `If-Match`.
    pub version: String,
    /// Reminder instants.
    pub reminders: Vec<DateTime<Utc>>,
}

/// An import result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// The import commit (revert it to undo the import).
    pub commit: Option<String>,
    /// Files written (sorted).
    pub imported: Vec<String>,
    /// Notes that were given an ID.
    pub ids_assigned: Vec<String>,
    /// Entries skipped (hidden or unsupported), sorted.
    pub skipped: Vec<String>,
}

/// A day in the user's time zone (for task views).
pub type Day = NaiveDate;
