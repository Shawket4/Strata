//! Strata graph component (PLAN §7.1 `graph/`, §6.5, §6.8, §7.5 Graph, §9.2, §9.6, §10,
//! D3, D10).
//!
//! - [`load`]: one user's graph from the index (live notes; links, relations, mentions,
//!   concepts, entity relations, custody, place nesting), always inside the caller's scoped
//!   transaction (principle 7).
//! - [`assemble`]: the global graph with type filters, optional similarity edges and
//!   optional tag nodes, the
//!   entity lens (`lens=people|companies`, co-mention strength), and local neighbourhoods
//!   (depth 1–3), built on `graph-algo`. No positions: layouts are computed by the client
//!   core (D3).
//! - [`similarity`]: the [`similarity::SimilaritySource`] seam for §9.6 edges and its
//!   implementation over stored note vectors.
//! - [`maps`]: saved JSON Canvas layouts in `maps/` (list, read, validated write).
//! - [`cluster`]: the `cluster` job (Leiden, stable IDs, AI names, `.meta/clusters.json`,
//!   tables, change log, `cluster.updated`) and the user's cluster rename.
//!
//! Design notes are in `docs/ARCHITECTURE.md`, section "Graph".

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(test, allow(clippy::expect_used, clippy::float_cmp))]

pub mod assemble;
pub mod cluster;
pub mod error;
pub mod load;
pub mod maps;
pub mod query;
pub mod service;
pub mod similarity;
pub mod summaries;

pub use error::{GraphError, MapIssue, Result};
pub use service::GraphService;
