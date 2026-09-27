//! The items duplicate detection compares.

use serde::{Deserialize, Serialize};

use crate::DedupeKind;
use crate::keys::{self, DedupeKeys};

/// One item to check (the new item) or one stored candidate.
///
/// Use the per-kind constructors; they fill the facets each kind needs. Fields are public
/// so callers can build items from database rows directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// What the item is.
    pub kind: DedupeKind,
    /// Stable ID (note ULID, task block ID `t-…`, entity note ULID). A new item that has no
    /// ID yet uses `None`; it then can never match a keep-both pair.
    pub id: Option<String>,
    /// Title shown in the prompt (note title, task text, entity name).
    pub title: String,
    /// The text compared: note title / capture text / task description / entity name.
    pub text: String,
    /// Further names that identify the item (entity and concept aliases). Every name is
    /// compared like [`Item::text`].
    pub aliases: Vec<String>,
    /// Tasks: the compiled recurrence rule (RFC 5545 RRULE) — part of the exact key.
    pub rrule: Option<String>,
    /// Tasks: linked entities (wikilink targets or IDs) — part of the exact key.
    pub entities: Vec<String>,
    /// Optional snippet shown with a candidate (first line of a note, task due date, …).
    pub snippet: Option<String>,
}

impl Item {
    fn base(kind: DedupeKind, id: Option<&str>, text: &str) -> Self {
        Self {
            kind,
            id: id.map(str::to_owned),
            title: text.to_owned(),
            text: text.to_owned(),
            aliases: Vec::new(),
            rrule: None,
            entities: Vec::new(),
            snippet: None,
        }
    }

    /// A note, compared by title.
    pub fn note(id: Option<&str>, title: &str) -> Self {
        Self::base(DedupeKind::Note, id, title)
    }

    /// An inbox capture, compared by its text (task-intent preambles such as "remind me to"
    /// are ignored, see [`keys::prepare_text`]).
    pub fn capture(id: Option<&str>, text: &str) -> Self {
        Self::base(DedupeKind::Capture, id, text)
    }

    /// A concept, compared by name and aliases.
    pub fn concept(id: Option<&str>, name: &str, aliases: &[&str]) -> Self {
        Self::entity(DedupeKind::Concept, id, name, aliases)
    }

    /// A task line: description text (without signifiers), compiled RRULE and linked
    /// entities.
    pub fn task(id: Option<&str>, text: &str, rrule: Option<&str>, entities: &[&str]) -> Self {
        let mut item = Self::base(DedupeKind::Task, id, text);
        item.rrule = rrule.map(str::to_owned);
        item.entities = entities.iter().map(|&e| e.to_owned()).collect();
        item
    }

    /// A named item with aliases: person, company, concept, document or place. `kind` should
    /// be one of those; other kinds are accepted and compared by name only.
    pub fn entity(kind: DedupeKind, id: Option<&str>, name: &str, aliases: &[&str]) -> Self {
        let mut item = Self::base(kind, id, name);
        item.aliases = aliases.iter().map(|&a| a.to_owned()).collect();
        item
    }

    /// A new alias being added to an entity; compared against every entity name and alias.
    pub fn alias(id: Option<&str>, alias: &str) -> Self {
        Self::base(DedupeKind::Alias, id, alias)
    }

    /// Sets the snippet shown in the prompt.
    #[must_use]
    pub fn with_snippet(mut self, snippet: &str) -> Self {
        self.snippet = Some(snippet.to_owned());
        self
    }

    /// Every name compared: [`Item::text`] then the non-empty aliases, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.text.as_str())
            .chain(self.aliases.iter().map(String::as_str))
            .filter(|n| !n.trim().is_empty())
    }

    /// The keys the backend stores in `dedupe_keys` (and the client in its local table).
    pub fn keys(&self) -> DedupeKeys {
        keys::keys_for(self)
    }
}
