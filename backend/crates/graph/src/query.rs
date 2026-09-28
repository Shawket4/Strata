//! The graph endpoints' filters (`types=`, `kinds=`, `lens=`, `depth=`), parsed from their
//! comma-separated query forms.
//!
//! `types` selects **edge** kinds by their §10 names: `link`, `embed`, `relation` (every
//! relation type) or `relation:<type>`, `similarity`, `concept`, `mention`, `entity` (every
//! entity relation) or `entity:<type>`, `custody` (all three) or
//! `custody:<location|holder|last-holder>`, `part-of-place`, `document` (every document
//! relation) or `document:<type>` (`document:copy-of`), `tag` (note → tag, with
//! `include_tags`), and `co-mention` (entity lens only). `kinds` selects **node** kinds
//! (`note`, `concept`, `person`, `company`, `document`, `place`, and `tag` with
//! `include_tags`); it is a separate parameter because `concept`, `document` and `tag` name
//! both a node and an edge kind. An absent or empty list allows everything.

use std::collections::BTreeSet;
use std::fmt;

use domain::{
    CustodyEdge, DocumentRelationType, EntityRelationType, GraphEdgeKind, GraphNodeKind,
    RelationType,
};

use crate::error::{GraphError, Result};

/// An edge kind as returned on the wire: a §10 typed kind, or the entity lens's derived
/// co-mention edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeKind {
    /// A typed edge.
    Typed(GraphEdgeKind),
    /// Two entities mentioned by the same notes (entity lens).
    CoMention,
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Typed(k) => k.fmt(f),
            Self::CoMention => f.write_str("co-mention"),
        }
    }
}

/// Allowed edge kinds (`None` = all).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdgeFilter(Option<BTreeSet<EdgeKind>>);

impl EdgeFilter {
    /// Everything.
    pub fn all() -> Self {
        Self(None)
    }

    /// Exactly these kinds.
    pub fn only(kinds: impl IntoIterator<Item = EdgeKind>) -> Self {
        Self(Some(kinds.into_iter().collect()))
    }

    /// Parses the `types` parameter.
    pub fn parse(types: Option<&str>) -> Result<Self> {
        let Some(list) = types.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(Self::all());
        };
        let mut out = BTreeSet::new();
        for item in list.split(',').map(str::trim) {
            let expanded: Vec<EdgeKind> = match item {
                "relation" => RelationType::ALL
                    .iter()
                    .map(|t| EdgeKind::Typed(GraphEdgeKind::Relation(*t)))
                    .collect(),
                "entity" => EntityRelationType::ALL
                    .iter()
                    .map(|t| EdgeKind::Typed(GraphEdgeKind::Entity(*t)))
                    .collect(),
                "custody" => CustodyEdge::ALL
                    .iter()
                    .map(|c| EdgeKind::Typed(GraphEdgeKind::Custody(*c)))
                    .collect(),
                "document" => DocumentRelationType::ALL
                    .iter()
                    .map(|t| EdgeKind::Typed(GraphEdgeKind::Document(*t)))
                    .collect(),
                "co-mention" => vec![EdgeKind::CoMention],
                other => match other.parse::<GraphEdgeKind>() {
                    Ok(k) => vec![EdgeKind::Typed(k)],
                    Err(_) => {
                        return Err(GraphError::InvalidParameter {
                            name: "types",
                            code: "unknown_edge_type",
                            message: format!("unknown edge type `{other}`"),
                        });
                    }
                },
            };
            out.extend(expanded);
        }
        Ok(Self(Some(out)))
    }

    /// Whether `kind` is allowed.
    pub fn allows(&self, kind: EdgeKind) -> bool {
        self.0.as_ref().is_none_or(|s| s.contains(&kind))
    }

    /// The typed kinds for `graph_algo::Filter` (empty = all).
    pub fn typed(&self) -> Vec<GraphEdgeKind> {
        self.0
            .iter()
            .flatten()
            .filter_map(|k| match k {
                EdgeKind::Typed(t) => Some(*t),
                EdgeKind::CoMention => None,
            })
            .collect()
    }

    /// Whether every kind is allowed.
    pub fn is_all(&self) -> bool {
        self.0.is_none()
    }
}

/// Node kinds the graph endpoints return: the note kinds and `tag` (with `include_tags`).
/// `attachment` (deferred) and the virtual `cluster` are never nodes of a response.
pub const NODE_KINDS: [GraphNodeKind; 7] = [
    GraphNodeKind::Note,
    GraphNodeKind::Concept,
    GraphNodeKind::Person,
    GraphNodeKind::Company,
    GraphNodeKind::Document,
    GraphNodeKind::Place,
    GraphNodeKind::Tag,
];

/// Allowed node kinds (`None` = all).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeFilter(Option<BTreeSet<GraphNodeKind>>);

impl NodeFilter {
    /// Everything.
    pub fn all() -> Self {
        Self(None)
    }

    /// Parses the `kinds` parameter.
    pub fn parse(kinds: Option<&str>) -> Result<Self> {
        let Some(list) = kinds.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(Self::all());
        };
        let mut out = BTreeSet::new();
        for item in list.split(',').map(str::trim) {
            let kind = item
                .parse::<GraphNodeKind>()
                .ok()
                .filter(|k| NODE_KINDS.contains(k))
                .ok_or_else(|| GraphError::InvalidParameter {
                    name: "kinds",
                    code: "unknown_node_kind",
                    message: format!("unknown node kind `{item}`"),
                })?;
            out.insert(kind);
        }
        Ok(Self(Some(out)))
    }

    /// Whether `kind` is allowed.
    pub fn allows(&self, kind: impl Into<GraphNodeKind>) -> bool {
        let kind = kind.into();
        self.0.as_ref().is_none_or(|s| s.contains(&kind))
    }

    /// The kinds for `graph_algo::Filter` (empty = all).
    pub fn graph_kinds(&self) -> Vec<GraphNodeKind> {
        self.0.iter().flatten().copied().collect()
    }
}

/// The entity lens (`lens=people|companies`, §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lens {
    /// People and the ties between them.
    People,
    /// Companies and the ties between them.
    Companies,
}

impl Lens {
    /// Parses the `lens` parameter.
    pub fn parse(lens: Option<&str>) -> Result<Option<Self>> {
        match lens.map(str::trim) {
            None | Some("") => Ok(None),
            Some("people") => Ok(Some(Self::People)),
            Some("companies") => Ok(Some(Self::Companies)),
            Some(other) => Err(GraphError::InvalidParameter {
                name: "lens",
                code: "unknown_lens",
                message: format!("unknown lens `{other}`; use people or companies"),
            }),
        }
    }

    /// The entity kind at the centre of the lens.
    pub fn kind(self) -> domain::NoteKind {
        match self {
            Self::People => domain::NoteKind::Person,
            Self::Companies => domain::NoteKind::Company,
        }
    }
}

/// A validated neighbourhood depth (`1..=3`).
pub fn depth(depth: Option<u8>) -> Result<u8> {
    match depth.unwrap_or(1) {
        d @ 1..=3 => Ok(d),
        _ => Err(GraphError::InvalidParameter {
            name: "depth",
            code: "invalid_depth",
            message: "depth must be 1, 2 or 3".into(),
        }),
    }
}

/// `GET /graph` parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphQuery {
    /// Edge kinds.
    pub edges: EdgeFilter,
    /// Node kinds.
    pub nodes: NodeFilter,
    /// Add similarity edges (§9.6).
    pub include_similarity: bool,
    /// Entity lens.
    pub lens: Option<Lens>,
    /// Add tag nodes and note → tag edges (§10 optional toggle; ignored with a lens).
    pub include_tags: bool,
}

/// `GET /graph/local/{id}` parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalQuery {
    /// Depth `1..=3`.
    pub depth: u8,
    /// Edge kinds.
    pub edges: EdgeFilter,
    /// Node kinds (the focus is always included).
    pub nodes: NodeFilter,
    /// Add the focus's similarity edges.
    pub include_similarity: bool,
    /// Add tag nodes and note → tag edges (notes sharing a tag are two hops apart).
    pub include_tags: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_expand_families_and_reject_unknown_names() {
        let f = EdgeFilter::parse(Some("link, custody,relation:contradicts,co-mention"))
            .expect("valid");
        assert_eq!(
            f,
            EdgeFilter::only([
                EdgeKind::Typed(GraphEdgeKind::Link),
                EdgeKind::Typed(GraphEdgeKind::Relation(RelationType::Contradicts)),
                EdgeKind::Typed(GraphEdgeKind::Custody(CustodyEdge::Location)),
                EdgeKind::Typed(GraphEdgeKind::Custody(CustodyEdge::Holder)),
                EdgeKind::Typed(GraphEdgeKind::Custody(CustodyEdge::LastHolder)),
                EdgeKind::CoMention,
            ])
        );
        assert_eq!(
            EdgeFilter::parse(Some(" ")).expect("empty"),
            EdgeFilter::all()
        );
        assert_eq!(
            EdgeFilter::parse(Some("relation"))
                .expect("ok")
                .typed()
                .len(),
            6
        );
        assert_eq!(
            EdgeFilter::parse(Some("entity")).expect("ok").typed().len(),
            10
        );
        assert_eq!(
            EdgeFilter::parse(Some("document,tag")).expect("ok"),
            EdgeFilter::only([
                EdgeKind::Typed(GraphEdgeKind::Document(DocumentRelationType::CopyOf)),
                EdgeKind::Typed(GraphEdgeKind::Tag),
            ])
        );
        assert_eq!(
            EdgeFilter::parse(Some("document:copy-of")).expect("ok"),
            EdgeFilter::only([EdgeKind::Typed(GraphEdgeKind::Document(
                DocumentRelationType::CopyOf
            ))])
        );
        assert_eq!(
            EdgeFilter::parse(Some("copy-of"))
                .expect_err("family prefix required")
                .to_string(),
            "invalid parameter types: unknown edge type `copy-of`"
        );
        let err = EdgeFilter::parse(Some("link,relation:likes")).expect_err("unknown");
        assert_eq!(
            err.to_string(),
            "invalid parameter types: unknown edge type `relation:likes`"
        );
    }

    #[test]
    fn kinds_lens_and_depth() {
        use domain::NoteKind;
        let n = NodeFilter::parse(Some("person,place")).expect("ok");
        assert!(n.allows(NoteKind::Person) && n.allows(NoteKind::Place));
        assert!(!n.allows(NoteKind::Note) && !n.allows(GraphNodeKind::Tag));
        let t = NodeFilter::parse(Some("tag,note")).expect("ok");
        assert_eq!(
            t.graph_kinds(),
            vec![GraphNodeKind::Note, GraphNodeKind::Tag]
        );
        for bad in ["cluster", "attachment", "tags"] {
            assert_eq!(
                NodeFilter::parse(Some(bad))
                    .expect_err("not a response node kind")
                    .to_string(),
                format!("invalid parameter kinds: unknown node kind `{bad}`")
            );
        }
        assert_eq!(Lens::parse(Some("people")).expect("ok"), Some(Lens::People));
        assert_eq!(Lens::parse(None).expect("ok"), None);
        assert!(Lens::parse(Some("places")).is_err());
        assert_eq!(depth(None).expect("default"), 1);
        assert_eq!(depth(Some(3)).expect("ok"), 3);
        assert!(depth(Some(0)).is_err() && depth(Some(4)).is_err());
    }
}
