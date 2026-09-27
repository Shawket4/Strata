//! The writer's in-memory view of one vault: which files exist, which notes they are, and the
//! link names each note refers to (to find the notes whose links a create/move/delete can
//! re-resolve). Rebuilt from disk whenever the actor starts or reconciles.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use domain::NoteKind;
use strata_common::NoteId;
use vault_format::PathIndex;

/// What the writer knows about one note file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMeta {
    /// Frontmatter `id`.
    pub id: NoteId,
    /// Frontmatter `kind` (ordinary notes are `note`).
    pub kind: NoteKind,
    /// Content hash (`sha256:…`).
    pub version: String,
    /// Lower-cased link names this note links to (body links and frontmatter link values).
    pub link_names: BTreeSet<String>,
}

/// Files and notes of a vault.
#[derive(Debug, Clone, Default)]
pub struct VaultState {
    /// Live notes by path.
    pub notes: BTreeMap<String, NoteMeta>,
    /// Live attachments (content files that are not notes), for link resolution.
    pub attachments: BTreeSet<String>,
    /// Live note path by ID.
    pub by_id: HashMap<NoteId, String>,
    /// Trashed notes by trash path.
    pub trash: BTreeMap<String, NoteMeta>,
    /// Trash path by ID.
    pub trash_by_id: HashMap<NoteId, String>,
}

impl VaultState {
    /// A path index over every live content file.
    pub fn path_index(&self) -> PathIndex {
        PathIndex::new(
            self.notes
                .keys()
                .cloned()
                .chain(self.attachments.iter().cloned()),
        )
    }

    /// Inserts or replaces a live note.
    pub fn put_note(&mut self, path: &str, meta: NoteMeta) {
        if let Some(old) = self.by_id.insert(meta.id, path.to_owned())
            && old != path
        {
            self.notes.remove(&old);
        }
        self.notes.insert(path.to_owned(), meta);
    }

    /// Removes a live note by path.
    pub fn remove_note(&mut self, path: &str) -> Option<NoteMeta> {
        let meta = self.notes.remove(path)?;
        self.by_id.remove(&meta.id);
        Some(meta)
    }

    /// Inserts a trashed note.
    pub fn put_trash(&mut self, path: &str, meta: NoteMeta) {
        self.trash_by_id.insert(meta.id, path.to_owned());
        self.trash.insert(path.to_owned(), meta);
    }

    /// Removes a trashed note by trash path.
    pub fn remove_trash(&mut self, path: &str) -> Option<NoteMeta> {
        let meta = self.trash.remove(path)?;
        self.trash_by_id.remove(&meta.id);
        Some(meta)
    }

    /// The live note with `id`.
    pub fn note(&self, id: NoteId) -> Option<(&str, &NoteMeta)> {
        let path = self.by_id.get(&id)?;
        self.notes.get(path).map(|m| (path.as_str(), m))
    }

    /// The kind of the live note at `path`.
    pub fn kind_at(&self, path: &str) -> Option<NoteKind> {
        self.notes.get(path).map(|m| m.kind)
    }

    /// Live notes that link to any of `names` (lower-cased link names), by ID.
    pub fn linking_to(&self, names: &BTreeSet<String>) -> BTreeSet<NoteId> {
        if names.is_empty() {
            return BTreeSet::new();
        }
        self.notes
            .values()
            .filter(|m| !m.link_names.is_disjoint(names))
            .map(|m| m.id)
            .collect()
    }

    /// Whether any live or trashed note has `id`.
    pub fn contains_id(&self, id: NoteId) -> bool {
        self.by_id.contains_key(&id) || self.trash_by_id.contains_key(&id)
    }
}

/// The lower-cased link name of a vault path (`notes/A.md` → `a`, `x/scan.pdf` →
/// `scan.pdf`).
pub fn name_key(path: &str) -> String {
    vault_format::resolve::link_name(path).to_lowercase()
}
