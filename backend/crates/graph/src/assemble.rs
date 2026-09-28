//! Pure assembly of the graph payloads from [`GraphData`] (PLAN §7.5 Graph, §10): the global
//! graph with type filters and optional similarity edges, the entity lens, and local
//! neighbourhoods. Positions are never computed here (D3: the client core lays out).
//!
//! Tag nodes (§10 optional toggle, `include_tags`): one node per tag, compared without case
//! (`#Pricing` and `#pricing` are one tag, titled by the smallest spelling), with ID
//! `tag:<lowercase tag>`, and one `tag` edge from each note carrying it. Notes sharing a tag
//! are therefore two hops apart in a local graph.
//!
//! Output order is canonical: nodes by ID (notes by ULID, then tags by key), edges by
//! (source, target, kind), clusters by numeric ID, so equal inputs give byte-identical
//! responses.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::hash::BuildHasher;

use chrono::{DateTime, Utc};
use domain::{GraphEdgeKind, GraphNodeKind, NoteKind, RelationOrigin};
use graph_algo::{EdgeInput, Filter, GraphBuilder, co_mentions, neighbourhood};
use strata_common::NoteId;

use crate::error::{GraphError, Result};
use crate::load::{EdgeRow, GraphData, NodeRow};
use crate::query::{EdgeFilter, EdgeKind, GraphQuery, Lens, LocalQuery};

/// The ID of a response node: a note, or a tag node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeId {
    /// A note (its ULID on the wire).
    Note(NoteId),
    /// A tag, by its key ([`tag_key`]); `tag:<key>` on the wire.
    Tag(String),
}

impl NodeId {
    /// The note, if this is a note node.
    pub fn note(&self) -> Option<NoteId> {
        match self {
            Self::Note(n) => Some(*n),
            Self::Tag(_) => None,
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Note(n) => n.fmt(f),
            Self::Tag(k) => write!(f, "tag:{k}"),
        }
    }
}

impl From<NoteId> for NodeId {
    fn from(n: NoteId) -> Self {
        Self::Note(n)
    }
}

impl PartialEq<NoteId> for NodeId {
    fn eq(&self, other: &NoteId) -> bool {
        matches!(self, Self::Note(n) if n == other)
    }
}

/// The key of a tag node: tags compare without case (as in Obsidian).
pub fn tag_key(tag: &str) -> String {
    tag.to_lowercase()
}

/// A node of a graph response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    /// Note ID, or `tag:<key>`.
    pub id: NodeId,
    /// Title (a tag node's is the tag, without `#`).
    pub title: String,
    /// Kind (a note kind, or `tag`).
    pub kind: GraphNodeKind,
    /// Vault path (what a saved map's file node references); `None` for tag nodes.
    pub path: Option<String>,
    /// Stable cluster ID (`.meta/clusters.json`), if clustered.
    pub cluster_id: Option<String>,
    /// Edges of this response touching the node (in + out).
    pub degree: u32,
    /// Dominant language.
    pub lang: Option<String>,
    /// Last update (`None` for tag nodes).
    pub updated: Option<DateTime<Utc>>,
    /// Short AI summary for hover.
    pub summary: Option<String>,
    /// BFS depth from the focus (local graphs only; the focus is 0).
    pub depth: Option<u8>,
}

/// An edge of a graph response.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeView {
    /// Source node.
    pub source: NodeId,
    /// Target node (a tag node for `tag` edges).
    pub target: NodeId,
    /// Kind.
    pub kind: EdgeKind,
    /// Provenance (`None` for derived co-mention edges).
    pub by: Option<RelationOrigin>,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// AI one-line reason (relations).
    pub reason: Option<String>,
    /// Cosine similarity (`similarity`) or co-mention strength (`co-mention`).
    pub weight: Option<f64>,
    /// Notes mentioning both entities (`co-mention`).
    pub notes: Option<u32>,
}

/// A cluster present in a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterView {
    /// Stable ID.
    pub id: String,
    /// Name (AI or user).
    pub name: String,
    /// Members among the response's nodes.
    pub size: u32,
}

/// What happened to similarity edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimilarityStatus {
    /// Not requested.
    Off,
    /// Computed for every note with a vector.
    Complete,
    /// Computed for the most recently updated notes only (bounded cost).
    Truncated,
    /// Requested but no embedding model is configured (principle 6: the graph still works).
    Unavailable,
}

/// A graph response.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphView {
    /// Nodes.
    pub nodes: Vec<NodeView>,
    /// Edges.
    pub edges: Vec<EdgeView>,
    /// Clusters of the returned nodes.
    pub clusters: Vec<ClusterView>,
    /// Similarity edges.
    pub similarity: SimilarityStatus,
}

/// An ephemeral similarity pair (never stored, §9.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimilarPair {
    /// Source (the note whose neighbour list contained the target; the smaller ID for pairs
    /// found both ways).
    pub source: NoteId,
    /// Target.
    pub target: NoteId,
    /// Cosine similarity.
    pub score: f64,
}

fn typed_view(e: &EdgeRow) -> EdgeView {
    EdgeView {
        source: e.source.into(),
        target: e.target.into(),
        kind: EdgeKind::Typed(e.kind),
        by: Some(e.by),
        confidence: e.confidence,
        reason: e.reason.clone(),
        weight: None,
        notes: None,
    }
}

fn similarity_view(p: &SimilarPair) -> EdgeView {
    EdgeView {
        source: p.source.into(),
        target: p.target.into(),
        kind: EdgeKind::Typed(GraphEdgeKind::Similarity),
        by: Some(RelationOrigin::Ai),
        confidence: None,
        reason: None,
        weight: Some(p.score),
        notes: None,
    }
}

fn sort_edges(edges: &mut [EdgeView]) {
    edges.sort_by(|a, b| (&a.source, &a.target, a.kind).cmp(&(&b.source, &b.target, b.kind)));
}

/// A node of the graph a request sees: a live note, or a tag node.
#[derive(Debug, Clone)]
struct Node<'a> {
    id: NodeId,
    kind: GraphNodeKind,
    title: String,
    row: Option<&'a NodeRow>,
}

/// Every node and typed edge a request may return: the notes and edges of `data`, plus tag
/// nodes and note → tag edges from `data.tags` when `with_tags`. Nodes are in ID order
/// (notes, then tags), which is also their index in a graph built from them.
#[derive(Debug)]
struct Universe<'a> {
    nodes: Vec<Node<'a>>,
    edges: Vec<EdgeView>,
}

impl<'a> Universe<'a> {
    fn new(data: &'a GraphData, with_tags: bool) -> Self {
        let mut nodes: Vec<Node<'a>> = data
            .nodes
            .iter()
            .map(|n| Node {
                id: n.id.into(),
                kind: n.kind.into(),
                title: n.title.clone(),
                row: Some(n),
            })
            .collect();
        let mut edges: Vec<EdgeView> = data.edges.iter().map(typed_view).collect();
        if with_tags {
            // key → spellings; (note, key) pairs once each.
            let mut spellings: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
            let mut pairs: BTreeSet<(NoteId, String)> = BTreeSet::new();
            for (note, tag) in &data.tags {
                let key = tag_key(tag);
                spellings.entry(key.clone()).or_default().insert(tag.as_str());
                pairs.insert((*note, key));
            }
            nodes.extend(spellings.into_iter().map(|(key, names)| Node {
                title: names.first().map_or_else(|| key.clone(), |t| (*t).to_owned()),
                id: NodeId::Tag(key),
                kind: GraphNodeKind::Tag,
                row: None,
            }));
            edges.extend(pairs.into_iter().map(|(note, key)| EdgeView {
                source: note.into(),
                target: NodeId::Tag(key),
                kind: EdgeKind::Typed(GraphEdgeKind::Tag),
                by: Some(RelationOrigin::User),
                confidence: None,
                reason: None,
                weight: None,
                notes: None,
            }));
        }
        Self { nodes, edges }
    }

    fn get(&self, id: &NodeId) -> Option<&Node<'a>> {
        self.nodes
            .binary_search_by(|n| n.id.cmp(id))
            .ok()
            .map(|i| &self.nodes[i])
    }
}

/// Builds the node views of `ids` (in ID order) with degrees counted over `edges`.
fn finish<S: BuildHasher>(
    data: &GraphData,
    universe: &Universe<'_>,
    ids: &BTreeMap<NodeId, Option<u8>>,
    mut edges: Vec<EdgeView>,
    summaries: &HashMap<NoteId, String, S>,
    similarity: SimilarityStatus,
) -> GraphView {
    sort_edges(&mut edges);
    let mut degree: HashMap<&NodeId, u32> = HashMap::new();
    for e in &edges {
        *degree.entry(&e.source).or_default() += 1;
        *degree.entry(&e.target).or_default() += 1;
    }
    let mut sizes: BTreeMap<i64, u32> = BTreeMap::new();
    let nodes: Vec<NodeView> = ids
        .iter()
        .filter_map(|(id, depth)| universe.get(id).map(|n| (n, *depth)))
        .map(|(n, depth)| {
            let note = n.id.note();
            let cluster = note.and_then(|id| data.clusters.get(&id).copied());
            if let Some(c) = cluster {
                *sizes.entry(c).or_default() += 1;
            }
            NodeView {
                id: n.id.clone(),
                title: n.title.clone(),
                kind: n.kind,
                path: n.row.map(|r| r.path.clone()),
                cluster_id: cluster.map(|c| c.to_string()),
                degree: degree.get(&n.id).copied().unwrap_or(0),
                lang: n.row.and_then(|r| r.lang.clone()),
                updated: n.row.map(|r| r.updated),
                summary: note.and_then(|id| summaries.get(&id).cloned()),
                depth,
            }
        })
        .collect();
    let clusters = sizes
        .into_iter()
        .map(|(id, size)| ClusterView {
            id: id.to_string(),
            name: data
                .cluster_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("Cluster {id}")),
            size,
        })
        .collect();
    GraphView {
        nodes,
        edges,
        clusters,
        similarity,
    }
}

/// `GET /graph` without a lens: every live note (and tag node, with `include_tags`) of an
/// allowed kind, every allowed edge between them, similarity edges when given.
pub fn global<S: BuildHasher>(
    data: &GraphData,
    query: &GraphQuery,
    similar: &[SimilarPair],
    similarity: SimilarityStatus,
    summaries: &HashMap<NoteId, String, S>,
) -> GraphView {
    let universe = Universe::new(data, query.include_tags);
    let ids: BTreeMap<NodeId, Option<u8>> = universe
        .nodes
        .iter()
        .filter(|n| query.nodes.allows(n.kind))
        .map(|n| (n.id.clone(), None))
        .collect();
    let mut edges: Vec<EdgeView> = universe
        .edges
        .iter()
        .filter(|e| {
            query.edges.allows(e.kind) && ids.contains_key(&e.source) && ids.contains_key(&e.target)
        })
        .cloned()
        .collect();
    if query.include_similarity
        && query
            .edges
            .allows(EdgeKind::Typed(GraphEdgeKind::Similarity))
    {
        edges.extend(
            similar
                .iter()
                .map(similarity_view)
                .filter(|e| ids.contains_key(&e.source) && ids.contains_key(&e.target)),
        );
    }
    finish(data, &universe, &ids, edges, summaries, similarity)
}

/// `GET /graph?lens=people|companies`: the lens's entities as nodes; edges are entity
/// relations between them and co-mention strength (`graph_algo::co_mentions`: per note
/// mentioning `m` of them, each pair gains `1/(m−1)`). Tag nodes are never part of a lens.
pub fn lens<S: BuildHasher>(
    data: &GraphData,
    lens: Lens,
    edges_filter: &EdgeFilter,
    summaries: &HashMap<NoteId, String, S>,
) -> GraphView {
    let universe = Universe::new(data, false);
    let kind = lens.kind();
    let ids: BTreeMap<NodeId, Option<u8>> = data
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| (n.id.into(), None))
        .collect();
    let mut edges: Vec<EdgeView> = universe
        .edges
        .iter()
        .filter(|e| {
            matches!(e.kind, EdgeKind::Typed(GraphEdgeKind::Entity(_)))
                && edges_filter.allows(e.kind)
                && ids.contains_key(&e.source)
                && ids.contains_key(&e.target)
        })
        .cloned()
        .collect();
    if edges_filter.allows(EdgeKind::CoMention) {
        // Node indices follow ID order, so a co-mention's (a, b) is (smaller, larger) ID.
        let mut b = GraphBuilder::new();
        for n in &data.nodes {
            let _ = b.add_node(&n.id.to_string(), n.kind.into());
        }
        for e in data
            .edges
            .iter()
            .filter(|e| e.kind == GraphEdgeKind::Mention)
        {
            let _ = b.add_edge(&EdgeInput::user(
                &e.source.to_string(),
                &e.target.to_string(),
                GraphEdgeKind::Mention,
            ));
        }
        let g = b.build();
        let lens_kind: GraphNodeKind = kind.into();
        for c in co_mentions(&g, &[lens_kind]) {
            let (a, b) = (data.nodes[c.a as usize].id, data.nodes[c.b as usize].id);
            edges.push(EdgeView {
                source: a.into(),
                target: b.into(),
                kind: EdgeKind::CoMention,
                by: None,
                confidence: None,
                reason: None,
                weight: Some(c.strength),
                notes: Some(c.notes),
            });
        }
    }
    finish(data, &universe, &ids, edges, summaries, SimilarityStatus::Off)
}

/// `GET /graph/local/{id}`: the focus and everything within `depth` hops over allowed edges
/// (either direction) through allowed nodes, plus every allowed edge among them
/// (`graph_algo::neighbourhood`). With `include_tags`, tag nodes take part like any other
/// node. `similar` are the focus's similarity edges, if requested.
pub fn local<S: BuildHasher>(
    data: &GraphData,
    focus: NoteId,
    query: &LocalQuery,
    similar: &[SimilarPair],
    similarity: SimilarityStatus,
    summaries: &HashMap<NoteId, String, S>,
) -> Result<GraphView> {
    let universe = Universe::new(data, query.include_tags);
    let mut b = GraphBuilder::new();
    for n in &universe.nodes {
        let _ = b.add_node(&n.id.to_string(), n.kind);
    }
    // Edge index in the graph → the view it came from.
    let mut views: Vec<EdgeView> = Vec::with_capacity(universe.edges.len() + similar.len());
    for e in universe.edges.iter().cloned().chain(similar.iter().map(similarity_view)) {
        let EdgeKind::Typed(kind) = e.kind else {
            continue;
        };
        if b.add_edge(&EdgeInput {
            source: e.source.to_string(),
            target: e.target.to_string(),
            kind,
            by: e.by.unwrap_or(RelationOrigin::Ai),
            confidence: e.confidence,
        })
        .is_ok()
        {
            views.push(e);
        }
    }
    let g = b.build();
    let focus_ix = g.index_of(&focus.to_string()).ok_or(GraphError::NotFound)?;
    let filter = Filter {
        edge_kinds: query.edges.typed(),
        node_kinds: query.nodes.graph_kinds(),
    };
    // A filter that names only `co-mention` allows no typed edge at all.
    let nothing = !query.edges.is_all() && filter.edge_kinds.is_empty();
    let hood = if nothing {
        graph_algo::Neighbourhood {
            focus: focus_ix,
            nodes: vec![(focus_ix, 0)],
            edges: Vec::new(),
        }
    } else {
        neighbourhood(&g, focus_ix, query.depth, &filter).map_err(|e| {
            GraphError::InvalidParameter {
                name: "depth",
                code: "invalid_depth",
                message: e.to_string(),
            }
        })?
    };
    let ids: BTreeMap<NodeId, Option<u8>> = hood
        .nodes
        .iter()
        .map(|&(ix, d)| (universe.nodes[ix as usize].id.clone(), Some(d)))
        .collect();
    let edges: Vec<EdgeView> = hood
        .edges
        .iter()
        .map(|&i| views[i as usize].clone())
        .collect();
    Ok(finish(data, &universe, &ids, edges, summaries, similarity))
}

/// The node IDs of `data` as a set.
pub fn node_ids(data: &GraphData) -> BTreeSet<NoteId> {
    data.nodes.iter().map(|n| n.id).collect()
}
