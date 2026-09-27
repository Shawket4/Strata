//! The undirected weighted projection used by clustering and the force layout (§10:
//! "weights: user links > high-confidence AI relations > low-confidence").

use domain::{GraphEdgeKind, RelationOrigin};
use serde::{Deserialize, Serialize};

use crate::graph::{Edge, Graph};

/// How typed edges become undirected weights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeWeights {
    /// Body links, embeds and every edge the user made (or that has no provenance).
    pub user: f64,
    /// AI edges with confidence at or above [`EdgeWeights::high_confidence`].
    pub ai_high: f64,
    /// Other AI edges.
    pub ai_low: f64,
    /// Confidence at which an AI edge counts as high confidence.
    pub high_confidence: f32,
    /// Include ephemeral similarity edges (off by default: §10 clusters links + relations).
    pub include_similarity: bool,
}

impl Default for EdgeWeights {
    fn default() -> Self {
        Self {
            user: 3.0,
            ai_high: 2.0,
            ai_low: 1.0,
            high_confidence: 0.85,
            include_similarity: false,
        }
    }
}

impl EdgeWeights {
    /// The weight of one edge; `None` for excluded edges.
    pub fn weight(&self, e: &Edge) -> Option<f64> {
        if e.kind == GraphEdgeKind::Similarity && !self.include_similarity {
            return None;
        }
        Some(match e.by {
            RelationOrigin::User => self.user,
            RelationOrigin::Ai if e.confidence.unwrap_or(0.0) >= self.high_confidence => {
                self.ai_high
            }
            RelationOrigin::Ai => self.ai_low,
        })
    }
}

/// An undirected weighted graph in compressed form. Parallel edges are summed; self-loops
/// are dropped (they carry no information for clustering or layout).
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedGraph {
    offsets: Vec<usize>,
    neighbors: Vec<u32>,
    weights: Vec<f64>,
    strength: Vec<f64>,
    total: f64,
}

impl WeightedGraph {
    /// Builds from `(u, v, w)` triples over `n` nodes (`u != v`, `w > 0`; others ignored).
    pub fn from_edges(n: usize, edges: impl IntoIterator<Item = (u32, u32, f64)>) -> Self {
        let mut pairs: Vec<(u32, u32, f64)> = Vec::new();
        for (u, v, w) in edges {
            if u == v || w <= 0.0 || !w.is_finite() || u as usize >= n || v as usize >= n {
                continue;
            }
            pairs.push((u, v, w));
            pairs.push((v, u, w));
        }
        pairs.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let mut merged: Vec<(u32, u32, f64)> = Vec::with_capacity(pairs.len());
        for (u, v, w) in pairs {
            match merged.last_mut() {
                Some(last) if last.0 == u && last.1 == v => last.2 += w,
                _ => merged.push((u, v, w)),
            }
        }
        let mut offsets = vec![0_usize; n + 1];
        for &(u, _, _) in &merged {
            offsets[u as usize + 1] += 1;
        }
        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }
        let neighbors = merged.iter().map(|e| e.1).collect();
        let weights: Vec<f64> = merged.iter().map(|e| e.2).collect();
        let mut strength = vec![0.0; n];
        for &(u, _, w) in &merged {
            strength[u as usize] += w;
        }
        let total = weights.iter().sum::<f64>() / 2.0;
        Self {
            offsets,
            neighbors,
            weights,
            strength,
            total,
        }
    }

    /// The projection of a typed graph.
    pub fn project(graph: &Graph, weights: &EdgeWeights) -> Self {
        Self::from_edges(
            graph.node_count(),
            graph
                .edges()
                .iter()
                .filter_map(|e| weights.weight(e).map(|w| (e.source, e.target, w))),
        )
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.strength.len()
    }

    /// Neighbours of `u` with weights, sorted by neighbour.
    pub fn neighbors(&self, u: u32) -> impl Iterator<Item = (u32, f64)> + '_ {
        let r = self.offsets[u as usize]..self.offsets[u as usize + 1];
        self.neighbors[r.clone()]
            .iter()
            .copied()
            .zip(self.weights[r].iter().copied())
    }

    /// Weighted degree of `u`.
    pub fn strength(&self, u: u32) -> f64 {
        self.strength[u as usize]
    }

    /// Total edge weight `m` (each undirected edge once).
    pub fn total_weight(&self) -> f64 {
        self.total
    }

    /// Undirected edges `(u, v, w)` with `u < v`.
    pub fn edges(&self) -> impl Iterator<Item = (u32, u32, f64)> + '_ {
        (0..self.node_count()).flat_map(move |u| {
            let u = u32::try_from(u).unwrap_or(u32::MAX);
            self.neighbors(u)
                .filter(move |&(v, _)| u < v)
                .map(move |(v, w)| (u, v, w))
        })
    }
}
