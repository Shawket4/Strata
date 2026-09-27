//! Graph model vocabulary (§10).

use std::fmt;
use std::str::FromStr;

use crate::macros::string_enum;
use crate::{EntityRelationType, NoteKind, ParseError, RelationType};

string_enum! {
    /// Node kind in the graph API (§10).
    pub enum GraphNodeKind("graph node kind") {
        /// Plain note.
        Note => "note",
        /// Concept note.
        Concept => "concept",
        /// Person entity.
        Person => "person",
        /// Company entity.
        Company => "company",
        /// Document entity.
        Document => "document",
        /// Place entity.
        Place => "place",
        /// Attachment (deferred, §18).
        Attachment => "attachment",
        /// Tag (optional toggle).
        Tag => "tag",
        /// Virtual cluster node used for region labels.
        Cluster => "cluster",
    }
}

impl From<NoteKind> for GraphNodeKind {
    fn from(kind: NoteKind) -> Self {
        match kind {
            NoteKind::Note => Self::Note,
            NoteKind::Concept => Self::Concept,
            NoteKind::Person => Self::Person,
            NoteKind::Company => Self::Company,
            NoteKind::Document => Self::Document,
            NoteKind::Place => Self::Place,
        }
    }
}

string_enum! {
    /// Which document frontmatter field a custody edge comes from (document → place/person).
    pub enum CustodyEdge("custody edge") {
        /// `location:` → place.
        Location => "location",
        /// `holder:` → person.
        Holder => "holder",
        /// `last-holder:` → person.
        LastHolder => "last-holder",
    }
}

/// Edge kind in the graph API (§10). Parameterised kinds are written `prefix:value`
/// (`relation:contradicts`, `entity:works-at`, `custody:last-holder`), and serialise as that
/// single string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GraphEdgeKind {
    /// Body wikilink.
    Link,
    /// Body embed.
    Embed,
    /// Typed note relation.
    Relation(RelationType),
    /// Ephemeral embedding-similarity edge (§9.6).
    Similarity,
    /// Note → concept (`concepts:`).
    Concept,
    /// Note → person/company (`people:` / `companies:`).
    Mention,
    /// Entity-to-entity relation.
    Entity(EntityRelationType),
    /// Document → place/person custody field.
    Custody(CustodyEdge),
    /// Place → containing place (`part-of` on a place).
    PartOfPlace,
}

impl GraphEdgeKind {
    /// Every edge kind, including every parameterised variant.
    pub fn all() -> Vec<Self> {
        let mut all = vec![Self::Link, Self::Embed];
        all.extend(RelationType::ALL.iter().copied().map(Self::Relation));
        all.extend([Self::Similarity, Self::Concept, Self::Mention]);
        all.extend(EntityRelationType::ALL.iter().copied().map(Self::Entity));
        all.extend(CustodyEdge::ALL.iter().copied().map(Self::Custody));
        all.push(Self::PartOfPlace);
        all
    }
}

impl fmt::Display for GraphEdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link => f.write_str("link"),
            Self::Embed => f.write_str("embed"),
            Self::Relation(t) => write!(f, "relation:{t}"),
            Self::Similarity => f.write_str("similarity"),
            Self::Concept => f.write_str("concept"),
            Self::Mention => f.write_str("mention"),
            Self::Entity(t) => write!(f, "entity:{t}"),
            Self::Custody(c) => write!(f, "custody:{c}"),
            Self::PartOfPlace => f.write_str("part-of-place"),
        }
    }
}

impl FromStr for GraphEdgeKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseError::new("graph edge kind", s);
        if let Some((prefix, value)) = s.split_once(':') {
            return match prefix {
                "relation" => value.parse().map(Self::Relation).map_err(|_| err()),
                "entity" => value.parse().map(Self::Entity).map_err(|_| err()),
                "custody" => value.parse().map(Self::Custody).map_err(|_| err()),
                _ => Err(err()),
            };
        }
        match s {
            "link" => Ok(Self::Link),
            "embed" => Ok(Self::Embed),
            "similarity" => Ok(Self::Similarity),
            "concept" => Ok(Self::Concept),
            "mention" => Ok(Self::Mention),
            "part-of-place" => Ok(Self::PartOfPlace),
            _ => Err(err()),
        }
    }
}

impl serde::Serialize for GraphEdgeKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for GraphEdgeKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = GraphEdgeKind;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a graph edge kind string")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<GraphEdgeKind, E> {
                v.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
