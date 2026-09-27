//! The typed graph (§10): nodes with a [`GraphNodeKind`], directed typed edges with
//! provenance, built from edge lists, with adjacency in both directions.

use std::collections::HashMap;

use domain::{GraphEdgeKind, GraphNodeKind, RelationOrigin};
use serde::{Deserialize, Serialize};

/// Index of a node in a [`Graph`].
pub type NodeIx = u32;

/// A node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// External ID (note ULID, tag name, cluster ID).
    pub key: String,
    /// Kind.
    pub kind: GraphNodeKind,
}

/// A directed, typed edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source node.
    pub source: NodeIx,
    /// Target node.
    pub target: NodeIx,
    /// Kind.
    pub kind: GraphEdgeKind,
    /// Who created it (body links and edges without provenance are `user`).
    pub by: RelationOrigin,
    /// AI confidence, if any.
    pub confidence: Option<f32>,
}

/// An edge by node keys, as the index or the local cache lists them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeInput {
    /// Source key.
    pub source: String,
    /// Target key.
    pub target: String,
    /// Kind.
    pub kind: GraphEdgeKind,
    /// Provenance.
    pub by: RelationOrigin,
    /// AI confidence.
    pub confidence: Option<f32>,
}

impl EdgeInput {
    /// A user edge (body link or user relation).
    pub fn user(source: &str, target: &str, kind: GraphEdgeKind) -> Self {
        Self {
            source: source.to_owned(),
            target: target.to_owned(),
            kind,
            by: RelationOrigin::User,
            confidence: None,
        }
    }

    /// An AI edge with a confidence.
    pub fn ai(source: &str, target: &str, kind: GraphEdgeKind, confidence: f32) -> Self {
        Self {
            source: source.to_owned(),
            target: target.to_owned(),
            kind,
            by: RelationOrigin::Ai,
            confidence: Some(confidence),
        }
    }
}

/// Building a graph failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    /// An edge names a node that was not added.
    #[error("edge refers to unknown node {0:?}")]
    UnknownNode(String),
    /// The same key was added with two kinds.
    #[error("node {key:?} added as both {first} and {second}")]
    KindMismatch {
        /// Key.
        key: String,
        /// First kind.
        first: GraphNodeKind,
        /// Second kind.
        second: GraphNodeKind,
    },
    /// More than `u32::MAX` nodes.
    #[error("too many nodes")]
    TooLarge,
}

/// An immutable graph with compressed adjacency (edge indices per node, both directions).
#[derive(Debug, Clone, Default)]
pub struct Graph {
    nodes: Vec<Node>,
    index: HashMap<String, NodeIx>,
    edges: Vec<Edge>,
    out_offsets: Vec<usize>,
    out_edges: Vec<u32>,
    in_offsets: Vec<usize>,
    in_edges: Vec<u32>,
}

/// Incremental builder.
#[derive(Debug, Clone, Default)]
pub struct GraphBuilder {
    nodes: Vec<Node>,
    index: HashMap<String, NodeIx>,
    edges: Vec<Edge>,
}

impl GraphBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node (idempotent for the same key and kind) and returns its index.
    pub fn add_node(&mut self, key: &str, kind: GraphNodeKind) -> Result<NodeIx, GraphError> {
        if let Some(&ix) = self.index.get(key) {
            let first = self.nodes[ix as usize].kind;
            if first != kind {
                return Err(GraphError::KindMismatch {
                    key: key.to_owned(),
                    first,
                    second: kind,
                });
            }
            return Ok(ix);
        }
        let ix = NodeIx::try_from(self.nodes.len()).map_err(|_| GraphError::TooLarge)?;
        self.nodes.push(Node {
            key: key.to_owned(),
            kind,
        });
        self.index.insert(key.to_owned(), ix);
        Ok(ix)
    }

    /// Adds an edge between existing nodes.
    pub fn add_edge(&mut self, e: &EdgeInput) -> Result<(), GraphError> {
        let find = |k: &str| {
            self.index
                .get(k)
                .copied()
                .ok_or_else(|| GraphError::UnknownNode(k.to_owned()))
        };
        let (source, target) = (find(&e.source)?, find(&e.target)?);
        self.edges.push(Edge {
            source,
            target,
            kind: e.kind,
            by: e.by,
            confidence: e.confidence,
        });
        Ok(())
    }

    /// Finishes the graph.
    pub fn build(self) -> Graph {
        let n = self.nodes.len();
        let adjacency = |key: fn(&Edge) -> NodeIx| {
            let mut offsets = vec![0_usize; n + 1];
            for e in &self.edges {
                offsets[key(e) as usize + 1] += 1;
            }
            for i in 0..n {
                offsets[i + 1] += offsets[i];
            }
            let mut fill = offsets.clone();
            let mut list = vec![0_u32; self.edges.len()];
            for (i, e) in self.edges.iter().enumerate() {
                let slot = &mut fill[key(e) as usize];
                list[*slot] = u32::try_from(i).unwrap_or(u32::MAX);
                *slot += 1;
            }
            (offsets, list)
        };
        let (out_offsets, out_edges) = adjacency(|e| e.source);
        let (in_offsets, in_edges) = adjacency(|e| e.target);
        Graph {
            nodes: self.nodes,
            index: self.index,
            edges: self.edges,
            out_offsets,
            out_edges,
            in_offsets,
            in_edges,
        }
    }
}

impl Graph {
    /// Builds a graph from nodes and edges; edges must name added nodes.
    pub fn from_edge_list<'a>(
        nodes: impl IntoIterator<Item = (&'a str, GraphNodeKind)>,
        edges: impl IntoIterator<Item = EdgeInput>,
    ) -> Result<Self, GraphError> {
        let mut b = GraphBuilder::new();
        for (key, kind) in nodes {
            b.add_node(key, kind)?;
        }
        for e in edges {
            b.add_edge(&e)?;
        }
        Ok(b.build())
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// All nodes, by index.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// All edges, by index.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// A node.
    pub fn node(&self, ix: NodeIx) -> &Node {
        &self.nodes[ix as usize]
    }

    /// The index of a key.
    pub fn index_of(&self, key: &str) -> Option<NodeIx> {
        self.index.get(key).copied()
    }

    /// Indices of edges leaving `ix`, in insertion order.
    pub fn out_edges(&self, ix: NodeIx) -> &[u32] {
        let i = ix as usize;
        &self.out_edges[self.out_offsets[i]..self.out_offsets[i + 1]]
    }

    /// Indices of edges entering `ix`, in insertion order.
    pub fn in_edges(&self, ix: NodeIx) -> &[u32] {
        let i = ix as usize;
        &self.in_edges[self.in_offsets[i]..self.in_offsets[i + 1]]
    }

    /// Degree (in + out, self-loops counted twice).
    pub fn degree(&self, ix: NodeIx) -> usize {
        self.out_edges(ix).len() + self.in_edges(ix).len()
    }
}
