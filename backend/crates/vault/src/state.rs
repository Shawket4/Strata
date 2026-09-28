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

/// Live notes by path. Read-only through `Deref`; changed only through [`VaultState`], which
/// keeps its path index and link map in step.
#[derive(Debug, Clone, Default)]
pub struct Notes(BTreeMap<String, NoteMeta>);

impl std::ops::Deref for Notes {
    type Target = BTreeMap<String, NoteMeta>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Live attachments. Read-only through `Deref`; changed only through [`VaultState`].
#[derive(Debug, Clone, Default)]
pub struct Attachments(BTreeSet<String>);

impl std::ops::Deref for Attachments {
    type Target = BTreeSet<String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Files and notes of a vault.
///
/// Besides the files it maintains, across writes, the path index used for link resolution
/// and the reverse link map (link name → notes linking to it), so a write costs the same
/// whatever the size of the vault instead of rebuilding either from every file.
#[derive(Debug, Clone, Default)]
pub struct VaultState {
    /// Live notes by path.
    pub notes: Notes,
    /// Live attachments (content files that are not notes), for link resolution.
    pub attachments: Attachments,
    /// Live note path by ID.
    pub by_id: HashMap<NoteId, String>,
    /// Trashed notes by trash path.
    pub trash: BTreeMap<String, NoteMeta>,
    /// Trash path by ID.
    pub trash_by_id: HashMap<NoteId, String>,
    /// Every live content file (notes and attachments).
    index: PathIndex,
    /// Lower-cased link name → live notes whose `link_names` hold it.
    linkers: HashMap<String, BTreeSet<NoteId>>,
}

impl VaultState {
    /// The path index over every live content file.
    pub fn path_index(&self) -> &PathIndex {
        &self.index
    }

    fn link(&mut self, id: NoteId, names: &BTreeSet<String>) {
        for n in names {
            self.linkers.entry(n.clone()).or_default().insert(id);
        }
    }

    fn unlink(&mut self, id: NoteId, names: &BTreeSet<String>) {
        for n in names {
            if let Some(ids) = self.linkers.get_mut(n) {
                ids.remove(&id);
                if ids.is_empty() {
                    self.linkers.remove(n);
                }
            }
        }
    }

    fn drop_live(&mut self, path: &str) -> Option<NoteMeta> {
        let meta = self.notes.0.remove(path)?;
        self.unlink(meta.id, &meta.link_names);
        if !self.attachments.contains(path) {
            self.index.remove(path);
        }
        Some(meta)
    }

    /// Inserts or replaces a live note.
    pub fn put_note(&mut self, path: &str, meta: NoteMeta) {
        if let Some(old) = self.by_id.insert(meta.id, path.to_owned())
            && old != path
        {
            self.drop_live(&old);
        }
        self.drop_live(path);
        self.link(meta.id, &meta.link_names);
        self.index.insert(path);
        self.notes.0.insert(path.to_owned(), meta);
    }

    /// Removes a live note by path.
    pub fn remove_note(&mut self, path: &str) -> Option<NoteMeta> {
        let meta = self.drop_live(path)?;
        self.by_id.remove(&meta.id);
        Some(meta)
    }

    /// Replaces the link names of the live note at `path` (no-op for other paths).
    pub fn set_link_names(&mut self, path: &str, names: BTreeSet<String>) {
        let Some(old) = self.notes.0.get(path).map(|m| (m.id, m.link_names.clone())) else {
            return;
        };
        self.unlink(old.0, &old.1);
        self.link(old.0, &names);
        if let Some(m) = self.notes.0.get_mut(path) {
            m.link_names = names;
        }
    }

    /// Adds a live attachment.
    pub fn add_attachment(&mut self, path: &str) {
        self.attachments.0.insert(path.to_owned());
        self.index.insert(path);
    }

    /// Removes a live attachment; returns whether it was there.
    pub fn remove_attachment(&mut self, path: &str) -> bool {
        let was = self.attachments.0.remove(path);
        if was && !self.notes.contains_key(path) {
            self.index.remove(path);
        }
        was
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
        names
            .iter()
            .filter_map(|n| self.linkers.get(n))
            .flatten()
            .copied()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(n: u128, names: &[&str]) -> NoteMeta {
        NoteMeta {
            id: NoteId::from_ulid(ulid::Ulid(n)),
            kind: NoteKind::Note,
            version: String::new(),
            link_names: names.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn ids(ns: &[u128]) -> BTreeSet<NoteId> {
        ns.iter()
            .map(|n| NoteId::from_ulid(ulid::Ulid(*n)))
            .collect()
    }

    fn names(ns: &[&str]) -> BTreeSet<String> {
        ns.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn index_and_link_map_follow_every_change() {
        let mut s = VaultState::default();
        s.put_note("notes/A.md", meta(1, &["b", "scan.pdf"]));
        s.put_note("notes/B.md", meta(2, &["a"]));
        s.add_attachment("files/scan.pdf");
        assert_eq!(s.linking_to(&names(&["b", "a"])), ids(&[1, 2]));
        // Move: the old path leaves the index and the note keeps its links.
        s.put_note("notes/sub/A2.md", meta(1, &["b"]));
        s.set_link_names("notes/B.md", names(&["a2"]));
        assert_eq!(s.linking_to(&names(&["a"])), ids(&[]));
        assert_eq!(s.linking_to(&names(&["a2"])), ids(&[2]));
        assert_eq!(s.linking_to(&names(&["scan.pdf"])), ids(&[]));
        assert!(s.remove_attachment("files/scan.pdf"));
        assert!(!s.remove_attachment("files/scan.pdf"));
        assert_eq!(
            s.remove_note("notes/B.md").map(|m| m.id),
            ids(&[2]).pop_first()
        );
        assert_eq!(s.linking_to(&names(&["a2"])), ids(&[]));
        assert_eq!(s.linking_to(&names(&["b"])), ids(&[1]));
        assert_eq!(s.path_index(), &PathIndex::new(["notes/sub/A2.md"]));
        assert_eq!(s.by_id.len(), 1);
        assert_eq!(s.notes.keys().collect::<Vec<_>>(), vec!["notes/sub/A2.md"]);
    }
}
