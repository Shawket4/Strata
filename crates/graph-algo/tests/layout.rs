//! Layout invariants: no NaN, bounded, reproducible, energy decreases, warm starts, radial
//! geometry and crossing reduction.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)] // test helpers; exact values

use std::f64::consts::TAU;

use domain::{GraphEdgeKind as E, GraphNodeKind as K};
use graph_algo::layout::{ForceConfig, ForceLayout, RadialConfig, edge_crossings, radial_layout};
use graph_algo::rng::Rng;
use graph_algo::{EdgeInput, Filter, Graph, WeightedGraph, neighbourhood};
use proptest::prelude::*;

fn random_graph(seed: u64, n: usize, m: usize) -> WeightedGraph {
    let mut rng = Rng::new(seed);
    let edges: Vec<(u32, u32, f64)> = (0..m)
        .map(|_| {
            (
                u32::try_from(rng.below(n)).unwrap(),
                u32::try_from(rng.below(n)).unwrap(),
                1.0,
            )
        })
        .collect();
    WeightedGraph::from_edges(n, edges)
}

fn assert_sane(layout: &ForceLayout) {
    for p in layout.positions() {
        assert!(p[0].is_finite() && p[1].is_finite(), "{p:?}");
        assert!(
            p[0].hypot(p[1]) <= layout.bound() + 1e-9,
            "{p:?} outside {}",
            layout.bound()
        );
    }
}

#[test]
fn reproducible_for_a_seed() {
    let cfg = ForceConfig {
        seed: 11,
        ..ForceConfig::default()
    };
    let mut a = ForceLayout::new(random_graph(1, 200, 500), cfg);
    let mut b = ForceLayout::new(random_graph(1, 200, 500), cfg);
    let pa: Vec<[f64; 2]> = a.run(50).to_vec();
    let pb: Vec<[f64; 2]> = b.run(50).to_vec();
    assert_eq!(pa, pb, "bit-identical");
    let mut c = ForceLayout::new(random_graph(1, 200, 500), ForceConfig { seed: 12, ..cfg });
    assert_ne!(c.run(50).to_vec(), pa);
}

#[test]
fn energy_decreases_and_stays_bounded() {
    let mut layout = ForceLayout::new(random_graph(2, 150, 300), ForceConfig::default());
    let e0 = layout.energy();
    let mut prev = e0;
    for round in 0..6 {
        layout.run(40);
        assert_sane(&layout);
        let e = layout.energy();
        assert!(e < prev, "round {round}: {e} >= {prev}");
        prev = e;
    }
    assert!(layout.is_cool() || layout.temperature() < 1.0);
    assert_eq!(layout.steps(), 240);
}

#[test]
fn exact_and_barnes_hut_agree_closely() {
    let g = random_graph(3, 120, 240);
    let mut exact = ForceLayout::new(
        g.clone(),
        ForceConfig {
            theta: 0.0,
            ..ForceConfig::default()
        },
    );
    let mut approx = ForceLayout::new(g, ForceConfig::default());
    exact.run(100);
    approx.run(100);
    let (ee, ea) = (exact.energy(), approx.energy());
    assert!((ee - ea).abs() / ee.abs() < 0.05, "{ee} vs {ea}");
}

#[test]
fn coincident_and_degenerate_inputs() {
    let g = random_graph(4, 30, 40);
    let previous = vec![Some([5.0, 5.0]); 30];
    let mut layout = ForceLayout::warm(g, &previous, ForceConfig::default());
    layout.run(100);
    assert_sane(&layout);
    let ps = layout.positions();
    let distinct = ps.iter().filter(|p| **p != ps[0]).count();
    assert_eq!(distinct, 29, "every node was pushed apart");
    // Empty, single-node and edgeless graphs.
    assert!(
        ForceLayout::new(WeightedGraph::from_edges(0, []), ForceConfig::default())
            .step()
            .is_empty()
    );
    let mut one = ForceLayout::new(WeightedGraph::from_edges(1, []), ForceConfig::default());
    one.run(10);
    assert_sane(&one);
    let mut lonely = ForceLayout::new(WeightedGraph::from_edges(20, []), ForceConfig::default());
    lonely.run(200);
    assert_sane(&lonely);
}

#[test]
fn warm_start_keeps_the_map_and_places_new_nodes_near_neighbours() {
    let g = random_graph(5, 100, 250);
    let mut cold = ForceLayout::new(g, ForceConfig::default());
    cold.run(600);
    let settled = cold.positions().to_vec();
    // Add node 100 linked to node 7.
    let mut edges: Vec<(u32, u32, f64)> = random_graph(5, 100, 250).edges().collect();
    edges.push((100, 7, 1.0));
    let g2 = WeightedGraph::from_edges(101, edges);
    let mut previous: Vec<Option<[f64; 2]>> = settled.iter().copied().map(Some).collect();
    previous.push(None);
    let cfg = ForceConfig::default();
    let mut warm = ForceLayout::warm(g2, &previous, cfg);
    let new = warm.positions()[100];
    let anchor = settled[7];
    assert!((new[0] - anchor[0]).hypot(new[1] - anchor[1]) <= 0.5 * cfg.ideal_length + 1e-9);
    warm.run(20);
    let mut shifts: Vec<f64> = settled
        .iter()
        .zip(warm.positions())
        .map(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]))
        .collect();
    shifts.sort_by(f64::total_cmp);
    let k = cfg.ideal_length;
    // 20 steps at the warm temperature (0.25·k, cooling) bound every move…
    let budget: f64 = (0..20)
        .map(|s| cfg.warm_temperature * k * cfg.cooling.powi(s))
        .sum();
    assert!(shifts[99] <= budget + 1e-9, "{} > {budget}", shifts[99]);
    // …and the map barely moves: a typical node shifts by a small fraction of an edge.
    assert!(shifts[50] < 0.25 * k, "median shift {}", shifts[50]);
    assert!(shifts[99] < k, "max shift {}", shifts[99]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn never_nan_or_unbounded(seed in 0_u64..500, n in 1_usize..120, density in 0_usize..4) {
        let mut layout = ForceLayout::new(random_graph(seed, n, n * density), ForceConfig { seed, ..ForceConfig::default() });
        for _ in 0..30 {
            layout.step();
        }
        for p in layout.positions() {
            prop_assert!(p[0].is_finite() && p[1].is_finite());
            prop_assert!(p[0].hypot(p[1]) <= layout.bound() + 1e-9);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Radial

fn star_graph() -> Graph {
    // focus f; ring 1: a b c d; siblings a–c and b–d cross in index order; ring 2: x under a,
    // y under b.
    let nodes = [
        ("f", K::Note),
        ("a", K::Note),
        ("b", K::Note),
        ("c", K::Note),
        ("d", K::Note),
        ("x", K::Note),
        ("y", K::Note),
    ];
    let edges = vec![
        EdgeInput::user("f", "a", E::Link),
        EdgeInput::user("f", "b", E::Link),
        EdgeInput::user("f", "c", E::Link),
        EdgeInput::user("f", "d", E::Link),
        EdgeInput::user("a", "c", E::Link),
        EdgeInput::user("b", "d", E::Link),
        EdgeInput::user("a", "x", E::Link),
        EdgeInput::user("y", "b", E::Link),
    ];
    Graph::from_edge_list(nodes, edges).unwrap()
}

#[test]
fn radial_geometry() {
    let g = star_graph();
    let f = g.index_of("f").unwrap();
    let hood = neighbourhood(&g, f, 2, &Filter::default()).unwrap();
    let cfg = RadialConfig::default();
    let layout = radial_layout(&g, &hood, &cfg);
    assert_eq!(layout.len(), 7);
    assert_eq!(
        (layout[0].node, layout[0].x, layout[0].y, layout[0].depth),
        (f, 0.0, 0.0, 0)
    );
    for n in &layout {
        let r = n.x.hypot(n.y);
        assert!((r - f64::from(n.depth) * cfg.ring_spacing).abs() < 1e-9);
        assert!((0.0..TAU).contains(&n.angle));
        if let Some(p) = n.parent {
            let parent = layout.iter().find(|m| m.node == p).unwrap();
            assert_eq!(parent.depth + 1, n.depth);
            // A child's wedge lies inside its parent's wedge.
            assert!(n.wedge_start >= parent.wedge_start - 1e-9);
            assert!(n.wedge_start + n.wedge <= parent.wedge_start + parent.wedge + 1e-9);
        }
    }
    // Wedges are proportional to leaves: a and b have one leaf each like c and d.
    let ring1: Vec<f64> = layout
        .iter()
        .filter(|n| n.depth == 1)
        .map(|n| n.wedge)
        .collect();
    assert_eq!(ring1, [TAU / 4.0; 4]);
    // Deterministic.
    assert_eq!(radial_layout(&g, &hood, &cfg), layout);
}

#[test]
fn crossing_reduction_untangles_siblings() {
    let g = star_graph();
    let f = g.index_of("f").unwrap();
    let hood = neighbourhood(&g, f, 2, &Filter::default()).unwrap();
    let naive = radial_layout(
        &g,
        &hood,
        &RadialConfig {
            sweeps: 0,
            ..RadialConfig::default()
        },
    );
    let tuned = radial_layout(&g, &hood, &RadialConfig::default());
    assert_eq!(edge_crossings(&g, &hood, &naive), 1);
    assert_eq!(edge_crossings(&g, &hood, &tuned), 0);
    // a sits next to c, b next to d.
    let order: Vec<&str> = {
        let mut ring: Vec<_> = tuned.iter().filter(|n| n.depth == 1).collect();
        ring.sort_by(|p, q| p.angle.total_cmp(&q.angle));
        ring.iter().map(|n| g.node(n.node).key.as_str()).collect()
    };
    let pos = |k: &str| order.iter().position(|&o| o == k).unwrap();
    let gap = pos("a").abs_diff(pos("c"));
    assert!(
        gap == 1 || gap == 3,
        "a and c are neighbours on the ring: {order:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn radial_never_adds_crossings(seed in 0_u64..10_000, n in 3_usize..25, m in 2_usize..40) {
        let mut rng = Rng::new(seed);
        let names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
        let nodes = names.iter().map(|s| (s.as_str(), K::Note));
        let edges: Vec<EdgeInput> = (0..m)
            .map(|_| EdgeInput::user(&names[rng.below(n)], &names[rng.below(n)], E::Link))
            .collect();
        let g = Graph::from_edge_list(nodes, edges).unwrap();
        let hood = neighbourhood(&g, 0, 3, &Filter::default()).unwrap();
        let naive = radial_layout(&g, &hood, &RadialConfig { sweeps: 0, ..RadialConfig::default() });
        let tuned = radial_layout(&g, &hood, &RadialConfig::default());
        prop_assert_eq!(naive.len(), hood.nodes.len());
        for node in &tuned {
            prop_assert!(node.x.is_finite() && node.y.is_finite());
        }
        // Sweeps that do not help are undone: never worse than node order.
        prop_assert!(edge_crossings(&g, &hood, &tuned) <= edge_crossings(&g, &hood, &naive));
    }
}
