//! Leiden on known graphs, the connectivity guarantee, determinism.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::many_single_char_names
)] // test helpers; exact values

use std::collections::VecDeque;

use domain::{GraphEdgeKind, GraphNodeKind, RelationType};
use graph_algo::rng::Rng;
use graph_algo::{
    EdgeInput, EdgeWeights, Graph, LeidenConfig, WeightedGraph, leiden, leiden_from, modularity,
};
use pretty_assertions::assert_eq;
use proptest::prelude::*;

fn wg(n: usize, edges: &[(u32, u32)]) -> WeightedGraph {
    WeightedGraph::from_edges(n, edges.iter().map(|&(u, v)| (u, v, 1.0)))
}

fn clique(offset: u32, size: u32) -> Vec<(u32, u32)> {
    let mut e = Vec::new();
    for i in 0..size {
        for j in i + 1..size {
            e.push((offset + i, offset + j));
        }
    }
    e
}

/// Zachary's karate club (34 members, 78 ties), 1-indexed as published.
const KARATE: &[(u32, &[u32])] = &[
    (1, &[2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 18, 20, 22, 32]),
    (2, &[3, 4, 8, 14, 18, 20, 22, 31]),
    (3, &[4, 8, 9, 10, 14, 28, 29, 33]),
    (4, &[8, 13, 14]),
    (5, &[7, 11]),
    (6, &[7, 11, 17]),
    (7, &[17]),
    (9, &[31, 33, 34]),
    (10, &[34]),
    (14, &[34]),
    (15, &[33, 34]),
    (16, &[33, 34]),
    (19, &[33, 34]),
    (20, &[34]),
    (21, &[33, 34]),
    (23, &[33, 34]),
    (24, &[26, 28, 30, 33, 34]),
    (25, &[26, 28, 32]),
    (26, &[32]),
    (27, &[30, 34]),
    (28, &[34]),
    (29, &[32, 34]),
    (30, &[33, 34]),
    (31, &[33, 34]),
    (32, &[33, 34]),
    (33, &[34]),
];

fn karate() -> WeightedGraph {
    let edges: Vec<(u32, u32)> = KARATE
        .iter()
        .flat_map(|&(u, vs)| vs.iter().map(move |&v| (u - 1, v - 1)))
        .collect();
    assert_eq!(edges.len(), 78);
    wg(34, &edges)
}

/// Every community induces a connected subgraph.
fn communities_connected(g: &WeightedGraph, membership: &[u32]) -> bool {
    let n = g.node_count();
    let mut seen = vec![false; n];
    let mut comms_seen = std::collections::BTreeSet::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let c = membership[start];
        if !comms_seen.insert(c) {
            return false; // a second component of the same community
        }
        let mut q = VecDeque::from([start]);
        seen[start] = true;
        while let Some(u) = q.pop_front() {
            for (v, _) in g.neighbors(u32::try_from(u).unwrap()) {
                let v = v as usize;
                if !seen[v] && membership[v] == c {
                    seen[v] = true;
                    q.push_back(v);
                }
            }
        }
    }
    true
}

#[test]
fn two_cliques_joined_by_a_bridge() {
    let mut edges = clique(0, 5);
    edges.extend(clique(5, 5));
    edges.push((4, 5));
    let g = wg(10, &edges);
    let p = leiden(&g, &LeidenConfig::default());
    assert_eq!(p.membership, [0, 0, 0, 0, 0, 1, 1, 1, 1, 1]);
    assert_eq!(p.count, 2);
    // Q = 2 · (20/42 − (21/42)²) = 0.452…
    let expected = 2.0 * (20.0 / 42.0 - (21.0_f64 / 42.0).powi(2));
    assert!((p.quality - expected).abs() < 1e-12, "{}", p.quality);
    assert_eq!(p.quality, modularity(&g, &p.membership, 1.0));
}

#[test]
fn karate_club_modularity_above_bound_and_connected() {
    let g = karate();
    for seed in 0..20 {
        let p = leiden(
            &g,
            &LeidenConfig {
                seed,
                ..LeidenConfig::default()
            },
        );
        // The optimum is 0.4198 (4 communities); Louvain typically finds ≈ 0.4188.
        assert!(p.quality > 0.415, "seed {seed}: Q = {}", p.quality);
        assert!(
            (3..=5).contains(&p.count),
            "seed {seed}: {} communities",
            p.count
        );
        assert!(communities_connected(&g, &p.membership), "seed {seed}");
    }
}

#[test]
fn same_seed_same_partition() {
    let g = karate();
    let cfg = LeidenConfig {
        seed: 7,
        ..LeidenConfig::default()
    };
    let a = leiden(&g, &cfg);
    let b = leiden(&g, &cfg);
    assert_eq!(a, b);
}

#[test]
fn ring_of_cliques_and_resolution() {
    let mut edges = Vec::new();
    for c in 0..10 {
        edges.extend(clique(c * 5, 5));
        edges.push((c * 5 + 4, ((c + 1) % 10) * 5));
    }
    let g = wg(50, &edges);
    let p = leiden(&g, &LeidenConfig::default());
    assert_eq!(p.count, 10);
    let expected: Vec<u32> = (0..50).map(|i| i / 5).collect();
    assert_eq!(p.membership, expected);
    // Low resolution merges neighbouring cliques.
    let coarse = leiden(
        &g,
        &LeidenConfig {
            resolution: 0.05,
            ..LeidenConfig::default()
        },
    );
    assert!(coarse.count < 10, "{}", coarse.count);
    assert!(communities_connected(&g, &coarse.membership));
    // Very high resolution splits cliques.
    let fine = leiden(
        &g,
        &LeidenConfig {
            resolution: 20.0,
            ..LeidenConfig::default()
        },
    );
    assert!(fine.count > 10, "{}", fine.count);
    assert!(communities_connected(&g, &fine.membership));
}

#[test]
fn edgeless_and_empty_graphs() {
    let p = leiden(&wg(3, &[]), &LeidenConfig::default());
    assert_eq!((p.membership, p.count, p.quality), (vec![0, 1, 2], 3, 0.0));
    let p = leiden(&wg(0, &[]), &LeidenConfig::default());
    assert_eq!((p.membership.len(), p.count), (0, 0));
}

#[test]
fn starting_from_a_previous_partition_is_stable() {
    let g = karate();
    let cfg = LeidenConfig {
        seed: 3,
        ..LeidenConfig::default()
    };
    let first = leiden(&g, &cfg);
    let again = leiden_from(&g, Some(&first.membership), &cfg);
    assert!(again.quality >= first.quality - 1e-12);
    assert!(communities_connected(&g, &again.membership));
}

#[test]
fn projection_weights_user_links_over_ai_relations() {
    // a–b by user link, b–c by a low-confidence AI relation, c–d by user link.
    let nodes = [
        ("a", GraphNodeKind::Note),
        ("b", GraphNodeKind::Note),
        ("c", GraphNodeKind::Note),
        ("d", GraphNodeKind::Note),
    ];
    let rel = GraphEdgeKind::Relation(RelationType::Related);
    let edges = vec![
        EdgeInput::user("a", "b", GraphEdgeKind::Link),
        EdgeInput::ai("b", "c", rel, 0.72),
        EdgeInput::ai("c", "d", rel, 0.9),
        EdgeInput::user("d", "c", GraphEdgeKind::Link),
        EdgeInput::ai("a", "d", GraphEdgeKind::Similarity, 0.99),
    ];
    let g = Graph::from_edge_list(nodes, edges).unwrap();
    let w = WeightedGraph::project(&g, &EdgeWeights::default());
    let got: Vec<(u32, u32, f64)> = w.edges().collect();
    assert_eq!(got, [(0, 1, 3.0), (1, 2, 1.0), (2, 3, 5.0)]);
    assert_eq!(w.total_weight(), 9.0);
    let with_sim = WeightedGraph::project(
        &g,
        &EdgeWeights {
            include_similarity: true,
            ..EdgeWeights::default()
        },
    );
    assert_eq!(with_sim.edges().count(), 4);
}

fn random_graph(seed: u64, n: usize, m: usize) -> WeightedGraph {
    let mut rng = Rng::new(seed);
    let edges: Vec<(u32, u32, f64)> = (0..m)
        .map(|_| {
            let u = u32::try_from(rng.below(n)).unwrap();
            let v = u32::try_from(rng.below(n)).unwrap();
            (u, v, 1.0 + f64::from(u32::try_from(rng.below(3)).unwrap()))
        })
        .collect();
    WeightedGraph::from_edges(n, edges)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// The Leiden guarantee: every community is connected.
    #[test]
    fn every_community_is_connected(seed in 0_u64..1000, n in 2_usize..80, density in 1_usize..4, gamma in prop::sample::select(vec![0.5, 1.0, 2.0])) {
        let g = random_graph(seed, n, n * density);
        let p = leiden(&g, &LeidenConfig { seed, resolution: gamma, ..LeidenConfig::default() });
        prop_assert!(communities_connected(&g, &p.membership));
        prop_assert_eq!(p.membership.len(), n);
        prop_assert_eq!(p.count, p.membership.iter().map(|&c| c as usize + 1).max().unwrap_or(0));
        // Labels are in order of first appearance.
        let mut next = 0;
        for &c in &p.membership {
            prop_assert!(c <= next);
            if c == next { next += 1; }
        }
        // Never worse than all singletons.
        let singletons: Vec<u32> = (0..u32::try_from(n).unwrap()).collect();
        prop_assert!(p.quality >= modularity(&g, &singletons, gamma) - 1e-9);
    }
}

#[test]
fn recovers_a_planted_partition() {
    // 10 blocks of 30 nodes, 1500 edges, 90% inside a block.
    let mut rng = Rng::new(99);
    let edges: Vec<(u32, u32, f64)> = (0..1500)
        .map(|_| {
            let u = rng.below(300);
            let v = if rng.next_f64() < 0.9 {
                (u / 30) * 30 + rng.below(30)
            } else {
                rng.below(300)
            };
            (u32::try_from(u).unwrap(), u32::try_from(v).unwrap(), 1.0)
        })
        .collect();
    let g = WeightedGraph::from_edges(300, edges);
    let p = leiden(&g, &LeidenConfig::default());
    let expected: Vec<u32> = (0..300).map(|i| i / 30).collect();
    assert!(p.quality >= modularity(&g, &expected, 1.0) - 1e-12);
    assert_eq!(p.count, 10);
    assert_eq!(p.membership, expected);
}
