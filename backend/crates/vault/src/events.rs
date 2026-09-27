//! Commit notices: what one committed write changed, for the per-user event stream (PLAN
//! §7.5 Events) and anything else that reacts to vault changes.
//!
//! The writer builds a [`Committed`] while it re-derives the index (it knows every note's
//! path and version before and after, and diffs the derived relation, task and custody rows),
//! and hands it to the registered [`CommitListener`] only **after** the scoped transaction
//! committed, on the user's actor. Notices of one user therefore arrive in commit order and
//! never describe a write that was rolled back.

use std::sync::Arc;

use domain::NoteKind;
use strata_common::{NoteId, SuggestionId, UserId};

/// How a note changed in a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteChange {
    /// A live note appeared (created, captured, imported, restored).
    Created,
    /// A live note's content changed at the same path.
    Updated,
    /// A live note has a new path (its content may have changed too).
    Moved,
    /// A live note went to the trash or was removed.
    Deleted,
}

/// One note in a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEvent {
    /// Note ID.
    pub id: NoteId,
    /// Kind (after the change; before it for deletes).
    pub kind: NoteKind,
    /// What happened.
    pub change: NoteChange,
    /// Current path (last live path for deletes).
    pub path: String,
    /// Previous path (moves).
    pub old_path: Option<String>,
    /// New version (`None` for deletes).
    pub version: Option<String>,
}

/// A relation that appeared or disappeared (index `relations`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationEvent {
    /// Source note.
    pub src: NoteId,
    /// Relation type (`related`, `works-at`, …).
    pub rel: String,
    /// Target note.
    pub dst: NoteId,
    /// Added (`true`) or removed.
    pub added: bool,
}

/// A task line that changed or disappeared.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskEvent {
    /// Block ID (`t-…`).
    pub id: String,
    /// The note holding (or last holding) the line.
    pub note_id: NoteId,
    /// Version of the line now (`None` when the task is gone).
    pub version: Option<String>,
}

/// A document whose custody history changed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustodyEvent {
    /// Document note.
    pub document_id: NoteId,
    /// Its version now.
    pub version: Option<String>,
}

/// A suggestion that was created or changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionEvent {
    /// Suggestion ID.
    pub id: SuggestionId,
    /// Note it concerns.
    pub note_id: Option<NoteId>,
    /// Kind (`duplicate`, `conflict`, …).
    pub kind: String,
    /// Status after the change (`pending`, `accepted`, …).
    pub status: String,
    /// Whether it was just created.
    pub created: bool,
}

/// Everything one committed write changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Committed {
    /// The vault's user.
    pub user: Option<UserId>,
    /// Operation label from the commit message (`create`, `merge`, `task complete`, …;
    /// empty for suggestion-only changes).
    pub op: String,
    /// Notes, in ID order.
    pub notes: Vec<NoteEvent>,
    /// `(loser, survivor)` of an entity merge.
    pub merged: Option<(NoteId, NoteId)>,
    /// Relations added or removed, sorted.
    pub relations: Vec<RelationEvent>,
    /// Task lines changed, sorted by ID.
    pub tasks: Vec<TaskEvent>,
    /// Documents whose custody changed, sorted.
    pub custody: Vec<CustodyEvent>,
    /// Suggestions created or changed.
    pub suggestions: Vec<SuggestionEvent>,
    /// Integrity warnings recorded (reconciliation).
    pub integrity_warnings: u32,
}

impl Committed {
    /// Whether the notice carries nothing.
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
            && self.merged.is_none()
            && self.relations.is_empty()
            && self.tasks.is_empty()
            && self.custody.is_empty()
            && self.suggestions.is_empty()
            && self.integrity_warnings == 0
    }

    pub(crate) fn absorb(&mut self, other: Self) {
        self.notes.extend(other.notes);
        if other.merged.is_some() {
            self.merged = other.merged;
        }
        self.relations.extend(other.relations);
        self.tasks.extend(other.tasks);
        self.custody.extend(other.custody);
        self.suggestions.extend(other.suggestions);
        self.integrity_warnings += other.integrity_warnings;
    }
}

/// Receives commit notices. Called on the user's writer actor right after the commit, so it
/// must not block (hand the notice to a channel or an in-memory buffer).
pub trait CommitListener: Send + Sync {
    /// One committed write of `user`.
    fn committed(&self, user: UserId, notice: &Committed);
}

/// The registered listener slot.
pub(crate) type ListenerSlot = std::sync::RwLock<Option<Arc<dyn CommitListener>>>;

/// The operation label of a commit message (`user: task complete tasks/Tasks.md` →
/// `task complete` needs the op; we take everything between the prefix and the subject, which
/// the writer passes separately, so this helper only strips the prefix).
pub(crate) fn op_label(message: &str) -> String {
    let rest = message
        .split_once(": ")
        .map_or(message, |(_, rest)| rest)
        .trim();
    // Subjects are paths; ops are one or two lowercase words before them.
    let mut words = Vec::new();
    for w in rest.split(' ') {
        if !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c == '-') && words.len() < 2
        {
            words.push(w);
        } else {
            break;
        }
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::op_label;

    #[test]
    fn labels_come_from_commit_messages() {
        assert_eq!(op_label("user: create notes/A.md"), "create");
        assert_eq!(op_label("user: task complete tasks/Tasks.md"), "task complete");
        assert_eq!(
            op_label("user: merge people/A.md -> people/B.md"),
            "merge"
        );
        assert_eq!(op_label("ai: link notes/a b.md"), "link");
        assert_eq!(op_label("system: recovered changes"), "recovered changes");
    }
}
