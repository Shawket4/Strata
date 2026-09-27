//! The frontmatter keys Strata understands, their value shapes and the canonical key order.

use std::fmt;
use std::str::FromStr;

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

macro_rules! known_keys {
    ($( $(#[$doc:meta])* $variant:ident = $name:literal : $shape:ident ),+ $(,)?) => {
        /// Every frontmatter key with a meaning in Strata (PLAN §6.4, §6.7, §6.12), in
        /// canonical order: the declaration order below is the order Strata writes keys in.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum KnownKey {
            $( $(#[$doc])* $variant, )+
        }

        impl KnownKey {
            /// All known keys in canonical order.
            pub const ALL: &'static [KnownKey] = &[$(KnownKey::$variant),+];

            /// The YAML key.
            pub fn as_str(self) -> &'static str {
                match self { $(KnownKey::$variant => $name,)+ }
            }

            /// The value shape Strata writes for this key.
            pub fn shape(self) -> ValueShape {
                match self { $(KnownKey::$variant => ValueShape::$shape,)+ }
            }

            /// Looks a key up by its YAML name (exact, case-sensitive, as Obsidian does).
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($name => Some(KnownKey::$variant),)+ _ => None }
            }
        }
    };
}

known_keys! {
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
    /// Relation: related.
    Related = "related": LinkList,
    /// Relation: part of (also place nesting).
    PartOf = "part-of": LinkList,
    /// Relation: supports.
    Supports = "supports": LinkList,
    /// Relation: contradicts.
    Contradicts = "contradicts": LinkList,
    /// Relation: follows up.
    FollowsUp = "follows-up": LinkList,
    /// Relation: duplicates.
    Duplicates = "duplicates": LinkList,
    /// Relation: concepts this note is about.
    Concepts = "concepts": LinkList,
    /// Relation: people this note is about or meaningfully involves.
    People = "people": LinkList,
    /// Relation: companies this note is about or meaningfully involves.
    Companies = "companies": LinkList,
    /// Person relation: works at.
    WorksAt = "works-at": LinkList,
    /// Person relation: worked at.
    WorkedAt = "worked-at": LinkList,
    /// Person relation: reports to.
    ReportsTo = "reports-to": LinkList,
    /// Person relation: knows.
    Knows = "knows": LinkList,
    /// Person relation: introduced by.
    IntroducedBy = "introduced-by": LinkList,
    /// Company relation: client of.
    ClientOf = "client-of": LinkList,
    /// Company relation: supplier of.
    SupplierOf = "supplier-of": LinkList,
    /// Company relation: partner of.
    PartnerOf = "partner-of": LinkList,
    /// Company relation: competitor of.
    CompetitorOf = "competitor-of": LinkList,
    /// Company relation: subsidiary of.
    SubsidiaryOf = "subsidiary-of": LinkList,
    /// Document relation: copy of another document note.
    CopyOf = "copy-of": LinkList,
}

impl KnownKey {
    /// Position in the canonical order (0-based).
    pub fn rank(self) -> usize {
        self as usize
    }

    /// The relation this key stores, if it is a relation key.
    pub fn relation(self) -> Option<RelationKey> {
        RelationKey::ALL.iter().copied().find(|r| r.key() == self)
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

/// Error returned when a string names no known relation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown relation type `{0}`")]
pub struct UnknownRelation(pub String);

macro_rules! relation_keys {
    ($( $(#[$doc:meta])* $variant:ident ),+ $(,)?) => {
        /// A relation type stored as a flat wikilink list in frontmatter (and referenced by ID
        /// in the sidecar). Declaration order is the canonical order.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum RelationKey {
            $( $(#[$doc])* $variant, )+
        }

        impl RelationKey {
            /// All relation keys in canonical order.
            pub const ALL: &'static [RelationKey] = &[$(RelationKey::$variant),+];

            /// The frontmatter key that stores this relation.
            pub fn key(self) -> KnownKey {
                match self { $(RelationKey::$variant => KnownKey::$variant,)+ }
            }
        }
    };
}

relation_keys! {
    /// `related`
    Related,
    /// `part-of`
    PartOf,
    /// `supports`
    Supports,
    /// `contradicts`
    Contradicts,
    /// `follows-up`
    FollowsUp,
    /// `duplicates`
    Duplicates,
    /// `concepts`
    Concepts,
    /// `people`
    People,
    /// `companies`
    Companies,
    /// `works-at`
    WorksAt,
    /// `worked-at`
    WorkedAt,
    /// `reports-to`
    ReportsTo,
    /// `knows`
    Knows,
    /// `introduced-by`
    IntroducedBy,
    /// `client-of`
    ClientOf,
    /// `supplier-of`
    SupplierOf,
    /// `partner-of`
    PartnerOf,
    /// `competitor-of`
    CompetitorOf,
    /// `subsidiary-of`
    SubsidiaryOf,
    /// `copy-of`
    CopyOf,
}

impl RelationKey {
    /// The kebab-case name used in frontmatter and in the sidecar `type` field.
    pub fn as_str(self) -> &'static str {
        self.key().as_str()
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
        KnownKey::from_name(s)
            .and_then(KnownKey::relation)
            .ok_or_else(|| UnknownRelation(s.to_owned()))
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
        let names: Vec<_> = KnownKey::ALL.iter().take(9).map(|k| k.as_str()).collect();
        assert_eq!(
            names,
            [
                "id", "kind", "title", "aliases", "tags", "created", "updated", "source", "lang"
            ]
        );
        let rel: Vec<_> = RelationKey::ALL
            .iter()
            .take(9)
            .map(|k| k.as_str())
            .collect();
        assert_eq!(
            rel,
            [
                "related",
                "part-of",
                "supports",
                "contradicts",
                "follows-up",
                "duplicates",
                "concepts",
                "people",
                "companies"
            ]
        );
    }

    #[test]
    fn relation_lookup_round_trips() {
        for r in RelationKey::ALL {
            assert_eq!(r.as_str().parse::<RelationKey>(), Ok(*r));
            assert_eq!(r.key().relation(), Some(*r));
            assert!(r.key().holds_links());
        }
        assert_eq!(
            "title".parse::<RelationKey>(),
            Err(UnknownRelation("title".into()))
        );
        assert_eq!(KnownKey::Title.relation(), None);
        assert_eq!(KnownKey::from_name("Title"), None);
    }

    #[test]
    fn relation_serde_is_kebab_string() {
        let json = serde_json::to_string(&RelationKey::FollowsUp).unwrap_or_default();
        assert_eq!(json, "\"follows-up\"");
        let back: Result<RelationKey, _> = serde_json::from_str("\"client-of\"");
        assert_eq!(back.ok(), Some(RelationKey::ClientOf));
        assert!(serde_json::from_str::<RelationKey>("\"nope\"").is_err());
    }
}
