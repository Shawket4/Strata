//! The graph endpoints' filters (`types=`, `kinds=`, `lens=`, `depth=`), parsed from their
//! comma-separated query forms.
//!
//! `types` selects **edge** kinds by their §10 names: `link`, `embed`, `relation` (every
//! relation type) or `relation:<type>`, `similarity`, `concept`, `mention`, `entity` (every
//! entity relation) or `entity:<type>`, `custody` (all three) or
//! `custody:<location|holder|last-holder>`, `part-of-place`, and `co-mention` (entity lens
//! only). `kinds` selects **node** kinds (`note`, `concept`, `person`, `company`,
//! `document`, `place`); it is a separate parameter because `concept` names both a node and
//! an edge kind. An absent or empty list allows everything.

use std::collections::BTreeSet;
use std::fmt;

use domain::{CustodyEdge, EntityRelationType, GraphEdgeKind, NoteKind, RelationType};

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

/// Allowed node kinds (`None` = all).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeFilter(Option<BTreeSet<NoteKind>>);

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
                .parse::<NoteKind>()
                .map_err(|_| GraphError::InvalidParameter {
                    name: "kinds",
                    code: "unknown_node_kind",
                    message: format!("unknown node kind `{item}`"),
                })?;
            out.insert(kind);
        }
        Ok(Self(Some(out)))
    }

    /// Whether `kind` is allowed.
    pub fn allows(&self, kind: NoteKind) -> bool {
        self.0.as_ref().is_none_or(|s| s.contains(&kind))
    }

    /// The kinds for `graph_algo::Filter` (empty = all).
    pub fn graph_kinds(&self) -> Vec<domain::GraphNodeKind> {
        self.0.iter().flatten().map(|k| (*k).into()).collect()
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
    pub fn kind(self) -> NoteKind {
        match self {
            Self::People => NoteKind::Person,
            Self::Companies => NoteKind::Company,
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
        let err = EdgeFilter::parse(Some("link,relation:likes")).expect_err("unknown");
        assert_eq!(
            err.to_string(),
            "invalid parameter types: unknown edge type `relation:likes`"
        );
    }

    #[test]
    fn kinds_lens_and_depth() {
        let n = NodeFilter::parse(Some("person,place")).expect("ok");
        assert!(n.allows(NoteKind::Person) && n.allows(NoteKind::Place));
        assert!(!n.allows(NoteKind::Note));
        assert!(NodeFilter::parse(Some("tag")).is_err());
        assert_eq!(Lens::parse(Some("people")).expect("ok"), Some(Lens::People));
        assert_eq!(Lens::parse(None).expect("ok"), None);
        assert!(Lens::parse(Some("places")).is_err());
        assert_eq!(depth(None).expect("default"), 1);
        assert_eq!(depth(Some(3)).expect("ok"), 3);
        assert!(depth(Some(0)).is_err() && depth(Some(4)).is_err());
    }
}
