//! Note kinds and languages (§6.4, §6.6, §6.7, §6.12).

use crate::macros::string_enum;

string_enum! {
    /// What a note represents. Stored in the index (`notes.kind`); in frontmatter, `kind:` is
    /// written only for non-plain notes (a note without `kind` is [`NoteKind::Note`]).
    ///
    /// Tasks are checklist lines inside notes (§6.11), so there is no task kind; the task home
    /// `tasks/Tasks.md` is a plain note.
    pub enum NoteKind("note kind") {
        /// A plain user note, capture or digest.
        Note => "note",
        /// AI-maintained concept note in `concepts/` (§6.6).
        Concept => "concept",
        /// Person entity in `people/` (§6.7).
        Person => "person",
        /// Company entity in `companies/` (§6.7).
        Company => "company",
        /// Document entity in `documents/` (§6.12).
        Document => "document",
        /// Place entity in `places/` (§6.12).
        Place => "place",
    }
}

impl NoteKind {
    /// Whether this kind is a first-class entity with a directory page (§6.7, §6.12).
    pub const fn is_entity(self) -> bool {
        matches!(
            self,
            Self::Person | Self::Company | Self::Document | Self::Place
        )
    }

    /// The default vault folder for new notes of this kind (§6.1), without trailing slash.
    pub const fn default_folder(self) -> &'static str {
        match self {
            Self::Note => "notes",
            Self::Concept => "concepts",
            Self::Person => "people",
            Self::Company => "companies",
            Self::Document => "documents",
            Self::Place => "places",
        }
    }
}

string_enum! {
    /// Dominant content language detected by AI (`lang:` frontmatter, §6.4).
    pub enum Lang("language") {
        /// Arabic.
        Ar => "ar",
        /// English.
        En => "en",
        /// Mixed Arabic and English.
        Mixed => "mixed",
    }
}
