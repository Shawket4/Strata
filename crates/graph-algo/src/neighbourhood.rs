//! Local neighbourhoods (`GET /graph/local/{id}?depth=1..3&types=`) and the entity lens's
//! co-mention strength (§10).

use std::collections::{BTreeMap, VecDeque};

use domain::{GraphEdgeKind, GraphNodeKind};
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, NodeIx};

/// Which edges and nodes a traversal may use. Empty lists allow everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    /// Allowed edge kinds.
    pub edge_kinds: Vec<GraphEdgeKind>,
    /// Allowed node kinds (the focus is always included).
    pub node_kinds: Vec<GraphNodeKind>,
}

impl Filter {
    fn edge_ok(&self, k: GraphEdgeKind) -> bool {
        self.edge_kinds.is_empty() || self.edge_kinds.contains(&k)
    }

    fn node_ok(&self, k: GraphNodeKind) -> bool {
        self.node_kinds.is_empty() || self.node_kinds.contains(&k)
    }
}

/// A neighbourhood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Neighbourhood {
    /// The focus.
    pub focus: NodeIx,
    /// Nodes with their BFS depth (focus = 0), ordered by depth then node index.
    pub nodes: Vec<(NodeIx, u8)>,
    /// Indices of every allowed edge between included nodes, ascending.
    pub edges: Vec<u32>,
}

/// Neighbourhood errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NeighbourhoodError {
    /// Depth outside `1..=3`.
    #[error("depth must be 1..=3, got {0}")]
    InvalidDepth(u8),
    /// Unknown focus node.
    #[error("unknown node {0}")]
    UnknownNode(NodeIx),
}

/// Nodes reachable from `focus` in at most `depth` hops (edges in either direction), using
/// only allowed edges and nodes, plus every allowed edge among them.
pub fn neighbourhood(
    graph: &Graph,
    focus: NodeIx,
    depth: u8,
    filter: &Filter,
) -> Result<Neighbourhood, NeighbourhoodError> {
    if !(1..=3).contains(&depth) {
        return Err(NeighbourhoodError::InvalidDepth(depth));
    }
    if focus as usize >= graph.node_count() {
        return Err(NeighbourhoodError::UnknownNode(focus));
    }
    let mut seen: BTreeMap<NodeIx, u8> = BTreeMap::from([(focus, 0)]);
    let mut queue = VecDeque::from([focus]);
    while let Some(u) = queue.pop_front() {
        let d = seen[&u];
        if d == depth {
            continue;
        }
        let out = graph.out_edges(u).iter().map(|&e| (e, graph.edges()[e as usize].target));
        let inn = graph.in_edges(u).iter().map(|&e| (e, graph.edges()[e as usize].source));
        for (e, v) in out.chain(inn) {
            let edge = &graph.edges()[e as usize];
            if !filter.edge_ok(edge.kind) || !filter.node_ok(graph.node(v).kind) || seen.contains_key(&v) {
                continue;
            }
            seen.insert(v, d + 1);
            queue.push_back(v);
        }
    }
    let mut nodes: Vec<(NodeIx, u8)> = seen.iter().map(|(&n, &d)| (n, d)).collect();
    nodes.sort_by_key(|&(n, d)| (d, n));
    let edges = graph
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, e)| filter.edge_ok(e.kind) && seen.contains_key(&e.source) && seen.contains_key(&e.target))
        .map(|(i, _)| u32::try_from(i).unwrap_or(u32::MAX))
        .collect();
    Ok(Neighbourhood { focus, nodes, edges })
}

/// Co-mention of two entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoMention {
    /// Smaller node index.
    pub a: NodeIx,
    /// Larger node index.
    pub b: NodeIx,
    /// Number of notes mentioning both.
    pub notes: u32,
    /// Σ over those notes of `1 / (m − 1)`, where `m` is how many entities of the lens the
    /// note mentions: a note about two people ties them more than a meeting note naming ten.
    pub strength: f64,
}

/// Co-mention pairs among entities of `kinds` (via `mention` edges from notes), sorted by
/// strength (descending), then `(a, b)`.
pub fn co_mentions(graph: &Graph, kinds: &[GraphNodeKind]) -> Vec<CoMention> {
    let mut pairs: BTreeMap<(NodeIx, NodeIx), (u32, f64)> = BTreeMap::new();
    for n in 0..graph.node_count() {
        let n = NodeIx::try_from(n).unwrap_or(NodeIx::MAX);
        let mut mentioned: Vec<NodeIx> = graph
            .out_edges(n)
            .iter()
            .map(|&e| &graph.edges()[e as usize])
            .filter(|e| e.kind == GraphEdgeKind::Mention && kinds.contains(&graph.node(e.target).kind))
            .map(|e| e.target)
            .collect();
        mentioned.sort_unstable();
        mentioned.dedup();
        if mentioned.len() < 2 {
            continue;
        }
        #[expect(clippy::cast_precision_loss, reason = "mention counts are small")]
        let share = 1.0 / (mentioned.len() - 1) as f64;
        for (i, &a) in mentioned.iter().enumerate() {
            for &b in &mentioned[i + 1..] {
                let entry = pairs.entry((a, b)).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += share;
            }
        }
    }
    let mut out: Vec<CoMention> = pairs
        .into_iter()
        .map(|((a, b), (notes, strength))| CoMention { a, b, notes, strength })
        .collect();
    out.sort_by(|x, y| y.strength.total_cmp(&x.strength).then((x.a, x.b).cmp(&(y.a, y.b))));
    out
}
