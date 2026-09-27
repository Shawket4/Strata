//! Benchmarks at the §16.7 scale: 10k nodes, 40k edges (a planted-partition graph, so the
//! clustering and layout see realistic community structure).
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)] // bench harness

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use graph_algo::layout::{ForceConfig, ForceLayout};
use graph_algo::rng::Rng;
use graph_algo::{LeidenConfig, WeightedGraph, leiden};

const NODES: usize = 10_000;
const EDGES: usize = 40_000;

/// 100 planted communities of 100 nodes; 85% of edges inside a community.
fn planted() -> WeightedGraph {
    let mut rng = Rng::new(2026);
    let size = 100;
    let edges: Vec<(u32, u32, f64)> = (0..EDGES)
        .map(|_| {
            let u = rng.below(NODES);
            let v = if rng.next_f64() < 0.85 {
                (u / size) * size + rng.below(size)
            } else {
                rng.below(NODES)
            };
            let w = 1.0 + f64::from(u32::try_from(rng.below(3)).unwrap());
            (u32::try_from(u).unwrap(), u32::try_from(v).unwrap(), w)
        })
        .collect();
    WeightedGraph::from_edges(NODES, edges)
}

fn benches(c: &mut Criterion) {
    let graph = planted();
    let mut group = c.benchmark_group("graph_10k_40k");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("force_step", |b| {
        b.iter_batched_ref(
            || ForceLayout::new(graph.clone(), ForceConfig::default()),
            |layout| {
                black_box(layout.step());
            },
            BatchSize::LargeInput,
        );
    });
    group.bench_function("leiden", |b| {
        b.iter(|| black_box(leiden(&graph, &LeidenConfig::default())));
    });
    group.bench_function("projection_build", |b| {
        b.iter(|| black_box(planted()));
    });
    group.finish();
}

criterion_group!(graph_benches, benches);
criterion_main!(graph_benches);
