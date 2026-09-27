//! Graph algorithms shared by the backend and the client core (PLAN L16, §10, D10, D3/D4).
//!
//! - [`graph`]: the typed graph (node kinds from [`domain::GraphNodeKind`], typed edges from
//!   [`domain::GraphEdgeKind`] with provenance), built from edge lists.
//! - [`weighted`]: the undirected weighted projection (user links > high-confidence AI
//!   relations > low-confidence).
//! - [`neighbourhood`]: depth 1–3 neighbourhoods with type filters, and co-mention strength
//!   for the entity lens.
//! - [`leiden`]: Leiden community detection with a resolution parameter and a seed;
//!   [`matching`]: stable cluster IDs across runs by member overlap.
//! - [`layout`]: Barnes–Hut force-directed layout with a streaming `step()` API and warm
//!   starts; radial layout with crossing reduction.
//!
//! Everything is pure and deterministic: the same input and seed give the same output on
//! every platform ([`rng::Rng`] is implemented here so sequences never change with a
//! dependency upgrade).

pub mod graph;
pub mod layout;
pub mod leiden;
pub mod matching;
pub mod neighbourhood;
pub mod rng;
pub mod weighted;

pub use graph::{Edge, EdgeInput, Graph, GraphBuilder, GraphError, Node, NodeIx};
pub use leiden::{LeidenConfig, Partition, leiden, leiden_from, modularity};
pub use matching::{ClusterMatching, MatchConfig, MatchedCluster, PreviousCluster, match_clusters};
pub use neighbourhood::{
    CoMention, Filter, Neighbourhood, NeighbourhoodError, co_mentions, neighbourhood,
};
pub use weighted::{EdgeWeights, WeightedGraph};
