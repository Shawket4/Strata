//! Note relations (§6.4, §9.4) and entity-to-entity relations (§6.7).

use crate::macros::string_enum;
use crate::note::NoteKind;

string_enum! {
    /// Typed, directed note-to-note relation (§6.4). The string is also the frontmatter key
    /// the relation is stored under on the source note.
    pub enum RelationType("relation type") {
        /// `related`
        Related => "related",
        /// `part-of`
        PartOf => "part-of",
        /// `supports`
        Supports => "supports",
        /// `contradicts`
        Contradicts => "contradicts",
        /// `follows-up`
        FollowsUp => "follows-up",
        /// `duplicates` — never auto-applied by AI; always a suggestion (§9.4).
        Duplicates => "duplicates",
    }
}

impl RelationType {
    /// The frontmatter key holding this relation's wikilink list.
    pub const fn frontmatter_key(self) -> &'static str {
        self.as_str()
    }

    /// Parses a frontmatter key; `None` for keys that are not relation types.
    pub fn from_frontmatter_key(key: &str) -> Option<Self> {
        key.parse().ok()
    }

    /// Whether the AI may apply this relation automatically above the relation threshold
    /// (§9.4 step 3: `duplicates` never is).
    pub const fn is_auto_applicable(self) -> bool {
        !matches!(self, Self::Duplicates)
    }
}

/// Every note relation key in the canonical frontmatter order (§6.4): the six
/// [`RelationType`] keys, then `concepts`, `people`, `companies`.
pub const NOTE_RELATION_KEYS: [&str; 9] = [
    "related",
    "part-of",
    "supports",
    "contradicts",
    "follows-up",
    "duplicates",
    "concepts",
    "people",
    "companies",
];

string_enum! {
    /// Entity-to-entity relation (§6.7), stored as a frontmatter key on the subject entity.
    pub enum EntityRelationType("entity relation type") {
        /// Person → company, current.
        WorksAt => "works-at",
        /// Person → company, past.
        WorkedAt => "worked-at",
        /// Person → person.
        ReportsTo => "reports-to",
        /// Person → person.
        Knows => "knows",
        /// Person → person.
        IntroducedBy => "introduced-by",
        /// Company → company.
        ClientOf => "client-of",
        /// Company → company.
        SupplierOf => "supplier-of",
        /// Company → company.
        PartnerOf => "partner-of",
        /// Company → company.
        CompetitorOf => "competitor-of",
        /// Company → company.
        SubsidiaryOf => "subsidiary-of",
    }
}

impl EntityRelationType {
    /// The frontmatter key holding this relation's wikilink list.
    pub const fn frontmatter_key(self) -> &'static str {
        self.as_str()
    }

    /// Parses a frontmatter key; `None` for keys that are not entity relation types.
    pub fn from_frontmatter_key(key: &str) -> Option<Self> {
        key.parse().ok()
    }

    /// The kind of entity that carries this key (§6.7: person keys vs company keys).
    pub const fn subject_kind(self) -> NoteKind {
        match self {
            Self::WorksAt | Self::WorkedAt | Self::ReportsTo | Self::Knows | Self::IntroducedBy => {
                NoteKind::Person
            }
            Self::ClientOf
            | Self::SupplierOf
            | Self::PartnerOf
            | Self::CompetitorOf
            | Self::SubsidiaryOf => NoteKind::Company,
        }
    }
}

string_enum! {
    /// Who created a relation, link or custody event (`by` in sidecar provenance and the
    /// index, §6.5, §7.4). Entries without provenance count as [`RelationOrigin::User`].
    pub enum RelationOrigin("relation origin") {
        /// Added by the user (or by hand without provenance).
        User => "user",
        /// Added by an AI job.
        Ai => "ai",
    }
}
