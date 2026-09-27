//! The frontmatter keys Strata understands, their value shapes and the canonical key order.
//!
//! Property keys (`id`, `title`, `role`, …) are the vault format's own schema. Relation keys
//! come from the shared `domain` vocabulary (PLAN L16): [`domain::RelationType`],
//! [`domain::MentionType`], [`domain::EntityRelationType`] and
//! [`domain::DocumentRelationType`], so their spellings are defined in exactly one place.

use std::fmt;
use std::str::FromStr;

use domain::{DocumentRelationType, EntityRelationType, MentionType, RelationType};
use serde::{Deserialize, Serialize};

/// The shape a known key's value takes when Strata writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueShape {
    /// A single scalar string.
    Text,
    /// A single scalar that holds one wikilink (`"[[Target]]"`) or is empty.
    Link,
    /// A flat list of strings.
    List,
    /// A flat list of wikilink strings (a relation key).
    LinkList,
}

macro_rules! property_keys {
    ($( $(#[$doc:meta])* $variant:ident = $name:literal : $shape:ident ),+ $(,)?) => {
        /// Every frontmatter key with a meaning in Strata (PLAN §6.4, §6.7, §6.12). The
        /// canonical order is: the property keys in declaration order, then every relation key
        /// in [`RelationKey::all`] order.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum KnownKey {
            $( $(#[$doc])* $variant, )+
            /// A relation list (`related`, `people`, `works-at`, `copy-of`, …).
            Relation(RelationKey),
        }

        impl KnownKey {
            /// The non-relation keys in canonical order.
            pub const PROPERTIES: &'static [KnownKey] = &[$(KnownKey::$variant),+];

            /// The YAML key.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(KnownKey::$variant => $name,)+
                    KnownKey::Relation(r) => r.as_str(),
                }
            }

            /// The value shape Strata writes for this key.
            pub fn shape(self) -> ValueShape {
                match self {
                    $(KnownKey::$variant => ValueShape::$shape,)+
                    KnownKey::Relation(_) => ValueShape::LinkList,
                }
            }

            /// Looks a key up by its YAML name (exact, case-sensitive, as Obsidian does).
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(KnownKey::$variant),)+
                    other => other.parse().ok().map(KnownKey::Relation),
                }
            }
        }
    };
}

property_keys! {
    /// ULID assigned by the backend; never changes.
    Id = "id": Text,
    /// Note kind (`concept`, `person`, `company`, `document`, `place`).
    Kind = "kind": Text,
    /// Optional display title.
    Title = "title": Text,
    /// Alternative names.
    Aliases = "aliases": List,
    /// Frontmatter tags.
    Tags = "tags": List,
    /// Creation timestamp (RFC 3339).
    Created = "created": Text,
    /// Last update timestamp (RFC 3339).
    Updated = "updated": Text,
    /// Raw source attachment of a capture.
    Source = "source": Link,
    /// Dominant language (`ar`, `en`, `mixed`).
    Lang = "lang": Text,
    /// Person: role (user-editable).
    Role = "role": Text,
    /// Company: industry.
    Industry = "industry": Text,
    /// Company: website (user-entered only).
    Website = "website": Text,
    /// Person: phone (user-entered only).
    Phone = "phone": Text,
    /// Person: email (user-entered only).
    Email = "email": Text,
    /// Place: address (user-entered only).
    Address = "address": Text,
    /// Document: type (`contract`, `id`, `licence`, …).
    DocType = "doc-type": Text,
    /// Document: copy kind (`original`, `certified copy`, `copy`, `digital`).
    Copy = "copy": Text,
    /// Document: the place it is in now.
    Location = "location": Link,
    /// Document: the person holding it now.
    Holder = "holder": Link,
    /// Document: the person who last had it.
    LastHolder = "last-holder": Link,
    /// Document: expiry date (`YYYY-MM-DD`).
    Expires = "expires": Text,
    /// Document: status (`stored`, `checked-out`, …).
    Status = "status": Text,
}

impl KnownKey {
    /// All known keys in canonical order.
    pub fn all() -> impl Iterator<Item = KnownKey> {
        Self::PROPERTIES
            .iter()
            .copied()
            .chain(RelationKey::all().map(KnownKey::Relation))
    }

    /// Position in the canonical order (0-based).
    pub fn rank(self) -> usize {
        match self {
            Self::Relation(r) => Self::PROPERTIES.len() + r.index(),
            p => Self::PROPERTIES.iter().position(|k| *k == p).unwrap_or(0),
        }
    }

    /// The relation this key stores, if it is a relation key.
    pub fn relation(self) -> Option<RelationKey> {
        match self {
            Self::Relation(r) => Some(r),
            _ => None,
        }
    }

    /// Whether values of this key are wikilinks that follow renames.
    pub fn holds_links(self) -> bool {
        matches!(self.shape(), ValueShape::Link | ValueShape::LinkList)
    }
}

impl fmt::Display for KnownKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<RelationKey> for KnownKey {
    fn from(r: RelationKey) -> Self {
        Self::Relation(r)
    }
}

/// Error returned when a string names no known relation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown relation type `{0}`")]
pub struct UnknownRelation(pub String);

/// A relation stored as a flat wikilink list in frontmatter (and referenced by ID in the
/// sidecar `type` field). Each variant wraps the shared `domain` enum that defines its
/// spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationKey {
    /// Typed note-to-note relation (`related`, `part-of`, …).
    Note(RelationType),
    /// About/involves links (`concepts`, `people`, `companies`).
    Mention(MentionType),
    /// Entity-to-entity relation (`works-at`, `client-of`, …).
    Entity(EntityRelationType),
    /// Document-to-document relation (`copy-of`).
    Document(DocumentRelationType),
}

impl RelationKey {
    /// All relation keys in canonical order: note relations, mentions, entity relations,
    /// document relations (each in its `domain` declaration order).
    pub fn all() -> impl Iterator<Item = RelationKey> {
        RelationType::ALL
            .iter()
            .copied()
            .map(Self::Note)
            .chain(MentionType::ALL.iter().copied().map(Self::Mention))
            .chain(EntityRelationType::ALL.iter().copied().map(Self::Entity))
            .chain(
                DocumentRelationType::ALL
                    .iter()
                    .copied()
                    .map(Self::Document),
            )
    }

    fn index(self) -> usize {
        Self::all().position(|r| r == self).unwrap_or(0)
    }

    /// The kebab-case name used in frontmatter and in the sidecar `type` field.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note(t) => t.as_str(),
            Self::Mention(t) => t.as_str(),
            Self::Entity(t) => t.as_str(),
            Self::Document(t) => t.as_str(),
        }
    }

    /// The frontmatter key that stores this relation.
    pub fn key(self) -> KnownKey {
        KnownKey::Relation(self)
    }
}

impl From<RelationType> for RelationKey {
    fn from(t: RelationType) -> Self {
        Self::Note(t)
    }
}

impl From<MentionType> for RelationKey {
    fn from(t: MentionType) -> Self {
        Self::Mention(t)
    }
}

impl From<EntityRelationType> for RelationKey {
    fn from(t: EntityRelationType) -> Self {
        Self::Entity(t)
    }
}

impl From<DocumentRelationType> for RelationKey {
    fn from(t: DocumentRelationType) -> Self {
        Self::Document(t)
    }
}

impl fmt::Display for RelationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RelationKey {
    type Err = UnknownRelation;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse()
            .map(Self::Note)
            .or_else(|_| s.parse().map(Self::Mention))
            .or_else(|_| s.parse().map(Self::Entity))
            .or_else(|_| s.parse().map(Self::Document))
            .map_err(|_| UnknownRelation(s.to_owned()))
    }
}

impl Serialize for RelationKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelationKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_starts_as_specified() {
        let names: Vec<_> = KnownKey::all().take(9).map(KnownKey::as_str).collect();
        assert_eq!(
            names,
            [
                "id", "kind", "title", "aliases", "tags", "created", "updated", "source", "lang"
            ]
        );
        let rel: Vec<_> = RelationKey::all()
            .take(9)
            .map(RelationKey::as_str)
            .collect();
        assert_eq!(rel, domain::NOTE_RELATION_KEYS);
        let all: Vec<_> = KnownKey::all().map(KnownKey::as_str).collect();
        assert_eq!(all.len(), 42);
        assert_eq!(all[21..23], ["status", "related"]);
        assert_eq!(all.last(), Some(&"copy-of"));
        for (i, k) in KnownKey::all().enumerate() {
            assert_eq!(k.rank(), i);
            assert_eq!(KnownKey::from_name(k.as_str()), Some(k));
        }
    }

    #[test]
    fn relation_lookup_round_trips() {
        for r in RelationKey::all() {
            assert_eq!(r.as_str().parse::<RelationKey>(), Ok(r));
            assert_eq!(r.key().relation(), Some(r));
            assert!(r.key().holds_links());
        }
        assert_eq!(
            "title".parse::<RelationKey>(),
            Err(UnknownRelation("title".into()))
        );
        assert_eq!(KnownKey::Title.relation(), None);
        assert_eq!(KnownKey::from_name("Title"), None);
        assert_eq!(
            RelationKey::from(EntityRelationType::WorksAt).key(),
            KnownKey::Relation(RelationKey::Entity(EntityRelationType::WorksAt))
        );
    }

    #[test]
    fn relation_serde_is_kebab_string() {
        let json =
            serde_json::to_string(&RelationKey::Note(RelationType::FollowsUp)).unwrap_or_default();
        assert_eq!(json, "\"follows-up\"");
        let back: Result<RelationKey, _> = serde_json::from_str("\"client-of\"");
        assert_eq!(
            back.ok(),
            Some(RelationKey::Entity(EntityRelationType::ClientOf))
        );
        let back: Result<RelationKey, _> = serde_json::from_str("\"copy-of\"");
        assert_eq!(
            back.ok(),
            Some(RelationKey::Document(DocumentRelationType::CopyOf))
        );
        assert!(serde_json::from_str::<RelationKey>("\"nope\"").is_err());
    }
}
