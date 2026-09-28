//! Pure assembly of the graph payloads from [`GraphData`] (PLAN §7.5 Graph, §10): the global
//! graph with type filters and optional similarity edges, the entity lens, and local
//! neighbourhoods. Positions are never computed here (D3: the client core lays out).
//!
//! Output order is canonical: nodes by ID, edges by (source, target, kind), clusters by
//! numeric ID, so equal inputs give byte-identical responses.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::BuildHasher;

use chrono::{DateTime, Utc};
use domain::{GraphEdgeKind, GraphNodeKind, NoteKind, RelationOrigin};
use graph_algo::{EdgeInput, Filter, GraphBuilder, co_mentions, neighbourhood};
use strata_common::NoteId;

use crate::error::{GraphError, Result};
use crate::load::{EdgeRow, GraphData};
use crate::query::{EdgeFilter, EdgeKind, GraphQuery, Lens, LocalQuery};

/// A node of a graph response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    /// Note ID.
    pub id: NoteId,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Vault path (what a saved map's file node references).
    pub path: String,
    /// Stable cluster ID (`.meta/clusters.json`), if clustered.
    pub cluster_id: Option<String>,
    /// Edges of this response touching the node (in + out).
    pub degree: u32,
    /// Dominant language.
    pub lang: Option<String>,
    /// Last update.
    pub updated: DateTime<Utc>,
    /// Short AI summary for hover.
    pub summary: Option<String>,
    /// BFS depth from the focus (local graphs only; the focus is 0).
    pub depth: Option<u8>,
}

/// An edge of a graph response.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeView {
    /// Source note.
    pub source: NoteId,
    /// Target note.
    pub target: NoteId,
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
        source: e.source,
        target: e.target,
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
        source: p.source,
        target: p.target,
        kind: EdgeKind::Typed(GraphEdgeKind::Similarity),
        by: Some(RelationOrigin::Ai),
        confidence: None,
        reason: None,
        weight: Some(p.score),
        notes: None,
    }
}

fn sort_edges(edges: &mut [EdgeView]) {
    edges.sort_by(|a, b| (a.source, a.target, a.kind).cmp(&(b.source, b.target, b.kind)));
}

/// Builds the node views of `ids` (in ID order) with degrees counted over `edges`.
fn finish<S: BuildHasher>(
    data: &GraphData,
    ids: &BTreeMap<NoteId, Option<u8>>,
    mut edges: Vec<EdgeView>,
    summaries: &HashMap<NoteId, String, S>,
    similarity: SimilarityStatus,
) -> GraphView {
    sort_edges(&mut edges);
    let mut degree: HashMap<NoteId, u32> = HashMap::new();
    for e in &edges {
        *degree.entry(e.source).or_default() += 1;
        *degree.entry(e.target).or_default() += 1;
    }
    let pos = data.positions();
    let mut sizes: BTreeMap<i64, u32> = BTreeMap::new();
    let nodes: Vec<NodeView> = ids
        .iter()
        .filter_map(|(id, depth)| pos.get(id).map(|&i| (&data.nodes[i], *depth)))
        .map(|(n, depth)| {
            let cluster = data.clusters.get(&n.id).copied();
            if let Some(c) = cluster {
                *sizes.entry(c).or_default() += 1;
            }
            NodeView {
                id: n.id,
                title: n.title.clone(),
                kind: n.kind,
                path: n.path.clone(),
                cluster_id: cluster.map(|c| c.to_string()),
                degree: degree.get(&n.id).copied().unwrap_or(0),
                lang: n.lang.clone(),
                updated: n.updated,
                summary: summaries.get(&n.id).cloned(),
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

/// `GET /graph` without a lens: every live note of an allowed kind, every allowed edge
/// between them, similarity edges when given.
pub fn global<S: BuildHasher>(
    data: &GraphData,
    query: &GraphQuery,
    similar: &[SimilarPair],
    similarity: SimilarityStatus,
    summaries: &HashMap<NoteId, String, S>,
) -> GraphView {
    let ids: BTreeMap<NoteId, Option<u8>> = data
        .nodes
        .iter()
        .filter(|n| query.nodes.allows(n.kind))
        .map(|n| (n.id, None))
        .collect();
    let mut edges: Vec<EdgeView> = data
        .edges
        .iter()
        .filter(|e| {
            query.edges.allows(EdgeKind::Typed(e.kind))
                && ids.contains_key(&e.source)
                && ids.contains_key(&e.target)
        })
        .map(typed_view)
        .collect();
    if query.include_similarity
        && query
            .edges
            .allows(EdgeKind::Typed(GraphEdgeKind::Similarity))
    {
        edges.extend(
            similar
                .iter()
                .filter(|p| ids.contains_key(&p.source) && ids.contains_key(&p.target))
                .map(similarity_view),
        );
    }
    finish(data, &ids, edges, summaries, similarity)
}

/// `GET /graph?lens=people|companies`: the lens's entities as nodes; edges are entity
/// relations between them and co-mention strength (`graph_algo::co_mentions`: per note
/// mentioning `m` of them, each pair gains `1/(m−1)`).
pub fn lens<S: BuildHasher>(
    data: &GraphData,
    lens: Lens,
    edges_filter: &EdgeFilter,
    summaries: &HashMap<NoteId, String, S>,
) -> GraphView {
    let kind = lens.kind();
    let ids: BTreeMap<NoteId, Option<u8>> = data
        .nodes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| (n.id, None))
        .collect();
    let mut edges: Vec<EdgeView> = data
        .edges
        .iter()
        .filter(|e| {
            matches!(e.kind, GraphEdgeKind::Entity(_))
                && edges_filter.allows(EdgeKind::Typed(e.kind))
                && ids.contains_key(&e.source)
                && ids.contains_key(&e.target)
        })
        .map(typed_view)
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
                source: a,
                target: b,
                kind: EdgeKind::CoMention,
                by: None,
                confidence: None,
                reason: None,
                weight: Some(c.strength),
                notes: Some(c.notes),
            });
        }
    }
    finish(data, &ids, edges, summaries, SimilarityStatus::Off)
}

/// `GET /graph/local/{id}`: the focus and everything within `depth` hops over allowed edges
/// (either direction) through allowed nodes, plus every allowed edge among them
/// (`graph_algo::neighbourhood`). `similar` are the focus's similarity edges, if requested.
pub fn local<S: BuildHasher>(
    data: &GraphData,
    focus: NoteId,
    query: &LocalQuery,
    similar: &[SimilarPair],
    similarity: SimilarityStatus,
    summaries: &HashMap<NoteId, String, S>,
) -> Result<GraphView> {
    let mut b = GraphBuilder::new();
    for n in &data.nodes {
        let _ = b.add_node(&n.id.to_string(), n.kind.into());
    }
    // Edge index in the graph → the view it came from.
    let mut views: Vec<EdgeView> = Vec::with_capacity(data.edges.len() + similar.len());
    for e in &data.edges {
        if b.add_edge(&EdgeInput {
            source: e.source.to_string(),
            target: e.target.to_string(),
            kind: e.kind,
            by: e.by,
            confidence: e.confidence,
        })
        .is_ok()
        {
            views.push(typed_view(e));
        }
    }
    for p in similar {
        if b.add_edge(&EdgeInput::ai(
            &p.source.to_string(),
            &p.target.to_string(),
            GraphEdgeKind::Similarity,
            0.0,
        ))
        .is_ok()
        {
            views.push(similarity_view(p));
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
    let ids: BTreeMap<NoteId, Option<u8>> = hood
        .nodes
        .iter()
        .map(|&(ix, d)| (data.nodes[ix as usize].id, Some(d)))
        .collect();
    let edges: Vec<EdgeView> = hood
        .edges
        .iter()
        .map(|&i| views[i as usize].clone())
        .collect();
    Ok(finish(data, &ids, edges, summaries, similarity))
}

/// The node IDs of `data` as a set.
pub fn node_ids(data: &GraphData) -> BTreeSet<NoteId> {
    data.nodes.iter().map(|n| n.id).collect()
}
