//! Leiden community detection (D10; Traag, Waltman & van Eck 2019) optimising modularity
//! with a resolution parameter γ on the undirected weighted projection.
//!
//! Quality: `Q = 1/(2m) Σ_ij [A_ij − γ k_i k_j / (2m)] δ(c_i, c_j)`.
//!
//! Each pass runs the three Leiden phases:
//! 1. **Fast local moving**: nodes are visited from a queue (initially in random order); a
//!    node moves to the neighbouring (or an empty) community with the largest positive gain,
//!    and its neighbours outside that community are queued again.
//! 2. **Refinement**: within every community, starting from singletons, well-connected
//!    nodes are merged into well-connected refined communities, chosen at random with
//!    probability ∝ `exp(gain / θ)` among non-negative gains. This is what guarantees that
//!    every community is connected.
//! 3. **Aggregation**: refined communities become nodes; the partition from phase 1 is
//!    their initial partition.
//!
//! Passes repeat until local moving leaves every aggregate node on its own. Whole Leiden
//! runs repeat from the previous result until the partition is stable (or
//! [`LeidenConfig::max_runs`]). All randomness comes from the seeded [`Rng`], so the result is
//! a function of the graph and the seed.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::weighted::WeightedGraph;

/// Leiden parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LeidenConfig {
    /// Resolution γ (1.0 = standard modularity; higher → smaller communities).
    pub resolution: f64,
    /// Seed for every random choice.
    pub seed: u64,
    /// Randomness θ of the refinement phase (on the raw edge-weight scale).
    pub randomness: f64,
    /// Maximum number of full Leiden runs (each starts from the previous partition).
    pub max_runs: usize,
}

impl Default for LeidenConfig {
    fn default() -> Self {
        Self {
            resolution: 1.0,
            seed: 0,
            randomness: 0.01,
            max_runs: 10,
        }
    }
}

/// A partition of the nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Partition {
    /// Community of every node. Communities are numbered `0..count` in order of their
    /// smallest node index.
    pub membership: Vec<u32>,
    /// Number of communities.
    pub count: usize,
    /// Modularity (with the configured resolution).
    pub quality: f64,
}

impl Partition {
    /// Members of each community, in node order.
    pub fn communities(&self) -> Vec<Vec<u32>> {
        let mut out = vec![Vec::new(); self.count];
        for (node, &c) in self.membership.iter().enumerate() {
            out[c as usize].push(u32::try_from(node).unwrap_or(u32::MAX));
        }
        out
    }
}

/// Modularity of `membership` with resolution `gamma`.
pub fn modularity(graph: &WeightedGraph, membership: &[u32], gamma: f64) -> f64 {
    let m = graph.total_weight();
    if m <= 0.0 {
        return 0.0;
    }
    let count = membership
        .iter()
        .map(|&c| c as usize + 1)
        .max()
        .unwrap_or(0);
    let mut internal = vec![0.0; count];
    let mut total = vec![0.0; count];
    for u in 0..graph.node_count() {
        let uu = u32::try_from(u).unwrap_or(u32::MAX);
        let cu = membership[u] as usize;
        total[cu] += graph.strength(uu);
        for (v, w) in graph.neighbors(uu) {
            if membership[v as usize] as usize == cu {
                internal[cu] += w;
            }
        }
    }
    internal
        .iter()
        .zip(&total)
        .map(|(&e, &k)| e / (2.0 * m) - gamma * (k / (2.0 * m)).powi(2))
        .sum()
}

/// One level of the aggregation hierarchy.
struct Level {
    adj: Vec<Vec<(u32, f64)>>,
    self_loop: Vec<f64>,
    k: Vec<f64>,
}

impl Level {
    fn from_graph(g: &WeightedGraph) -> Self {
        let n = g.node_count();
        let adj = (0..n)
            .map(|u| g.neighbors(u32::try_from(u).unwrap_or(u32::MAX)).collect())
            .collect();
        let k = (0..n)
            .map(|u| g.strength(u32::try_from(u).unwrap_or(u32::MAX)))
            .collect();
        Self {
            adj,
            self_loop: vec![0.0; n],
            k,
        }
    }

    fn len(&self) -> usize {
        self.k.len()
    }
}

/// Scratch accumulator of weights per community.
struct Acc {
    w: Vec<f64>,
    touched: Vec<u32>,
    seen: Vec<bool>,
}

impl Acc {
    fn new(n: usize) -> Self {
        Self {
            w: vec![0.0; n],
            touched: Vec::new(),
            seen: vec![false; n],
        }
    }

    fn add(&mut self, c: u32, w: f64) {
        let i = c as usize;
        if !self.seen[i] {
            self.seen[i] = true;
            self.touched.push(c);
        }
        self.w[i] += w;
    }

    fn clear(&mut self) {
        for &c in &self.touched {
            self.w[c as usize] = 0.0;
            self.seen[c as usize] = false;
        }
        self.touched.clear();
    }
}

fn to_u32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

const EPS: f64 = 1e-12;

/// Phase 1. Returns whether any node moved.
fn local_moving(level: &Level, comm: &mut [u32], gamma_2m: f64, rng: &mut Rng) -> bool {
    let n = level.len();
    let mut total = vec![0.0; n];
    let mut size = vec![0_usize; n];
    for v in 0..n {
        total[comm[v] as usize] += level.k[v];
        size[comm[v] as usize] += 1;
    }
    let mut empty: Vec<u32> = (0..n).filter(|&c| size[c] == 0).map(to_u32).rev().collect();
    let mut order: Vec<u32> = (0..n).map(to_u32).collect();
    rng.shuffle(&mut order);
    let mut queue: std::collections::VecDeque<u32> = order.into();
    let mut queued = vec![true; n];
    let mut acc = Acc::new(n);
    let mut moved = false;
    while let Some(v) = queue.pop_front() {
        let vi = v as usize;
        queued[vi] = false;
        let old = comm[vi];
        for &(u, w) in &level.adj[vi] {
            acc.add(comm[u as usize], w);
        }
        let kv = level.k[vi];
        total[old as usize] -= kv;
        size[old as usize] -= 1;
        let gain = |c: u32, acc: &Acc| acc.w[c as usize] - gamma_2m * kv * total[c as usize];
        let mut best = old;
        let mut best_gain = gain(old, &acc);
        for &c in &acc.touched {
            let g = gain(c, &acc);
            if g > best_gain + EPS {
                best = c;
                best_gain = g;
            }
        }
        if best_gain < -EPS
            && size[old as usize] > 0
            && let Some(c) = empty.pop()
        {
            // Alone is better than any community: move to an empty one.
            best = c;
        }
        if size[old as usize] == 0 && best != old {
            empty.push(old);
        }
        total[best as usize] += kv;
        size[best as usize] += 1;
        comm[vi] = best;
        if best != old {
            moved = true;
            for &(u, _) in &level.adj[vi] {
                let ui = u as usize;
                if !queued[ui] && comm[ui] != best {
                    queued[ui] = true;
                    queue.push_back(u);
                }
            }
        }
        acc.clear();
    }
    moved
}

/// Phase 2. Returns the refined partition (each refined community inside one community).
fn refine(level: &Level, comm: &[u32], gamma_2m: f64, theta: f64, rng: &mut Rng) -> Vec<u32> {
    let n = level.len();
    let mut refined: Vec<u32> = (0..n).map(to_u32).collect();
    let mut ref_total: Vec<f64> = level.k.clone();
    let mut ref_size = vec![1_usize; n];
    let mut comm_total = vec![0.0; n];
    let mut members: Vec<Vec<u32>> = vec![Vec::new(); n];
    for v in 0..n {
        comm_total[comm[v] as usize] += level.k[v];
        members[comm[v] as usize].push(to_u32(v));
    }
    // External weight of each refined community towards the rest of its community.
    let mut ext: Vec<f64> = (0..n)
        .map(|v| {
            level.adj[v]
                .iter()
                .filter(|&&(u, _)| comm[u as usize] == comm[v])
                .map(|&(_, w)| w)
                .sum()
        })
        .collect();
    let mut acc = Acc::new(n);
    for (c, nodes) in members.iter_mut().enumerate() {
        if nodes.len() < 2 {
            continue;
        }
        let kc = comm_total[c];
        rng.shuffle(nodes);
        for &v in nodes.iter() {
            let vi = v as usize;
            if ref_size[refined[vi] as usize] != 1 {
                continue;
            }
            let kv = level.k[vi];
            if ext[vi] < gamma_2m * kv * (kc - kv) - EPS {
                continue; // v is not well connected to its community
            }
            for &(u, w) in &level.adj[vi] {
                if comm[u as usize] as usize == c {
                    acc.add(refined[u as usize], w);
                }
            }
            let own = refined[vi];
            let mut candidates: Vec<(u32, f64)> = vec![(own, 0.0)];
            for &t in &acc.touched {
                if t == own {
                    continue;
                }
                let kt = ref_total[t as usize];
                if ext[t as usize] < gamma_2m * kt * (kc - kt) - EPS {
                    continue; // T is not well connected
                }
                let g = acc.w[t as usize] - gamma_2m * kv * kt;
                if g >= 0.0 {
                    candidates.push((t, g));
                }
            }
            let max = candidates
                .iter()
                .map(|c| c.1)
                .fold(f64::NEG_INFINITY, f64::max);
            let weights: Vec<f64> = candidates
                .iter()
                .map(|c| ((c.1 - max) / theta).exp())
                .collect();
            let sum: f64 = weights.iter().sum();
            let mut r = rng.next_f64() * sum;
            let mut chosen = candidates[candidates.len() - 1].0;
            for (cand, w) in candidates.iter().zip(&weights) {
                if r < *w {
                    chosen = cand.0;
                    break;
                }
                r -= w;
            }
            if chosen != own {
                let t = chosen as usize;
                let w_vt = acc.w[t];
                ext[t] = ext[t] + ext[vi] - 2.0 * w_vt;
                ref_total[t] += kv;
                ref_size[t] += 1;
                ref_size[own as usize] -= 1;
                ref_total[own as usize] -= kv;
                refined[vi] = chosen;
            }
            acc.clear();
        }
    }
    refined
}

/// Renumbers labels to `0..count` in order of first appearance.
fn compact(labels: &mut [u32]) -> usize {
    let mut map: Vec<u32> = vec![u32::MAX; labels.len().max(1)];
    let mut next = 0_u32;
    for l in labels.iter_mut() {
        let i = *l as usize;
        if i >= map.len() {
            map.resize(i + 1, u32::MAX);
        }
        if map[i] == u32::MAX {
            map[i] = next;
            next += 1;
        }
        *l = map[i];
    }
    next as usize
}

/// Phase 3.
fn aggregate(level: &Level, refined: &[u32], count: usize) -> Level {
    let mut k = vec![0.0; count];
    let mut self_loop = vec![0.0; count];
    let mut maps: Vec<std::collections::BTreeMap<u32, f64>> =
        vec![std::collections::BTreeMap::new(); count];
    for v in 0..level.len() {
        let a = refined[v];
        k[a as usize] += level.k[v];
        self_loop[a as usize] += level.self_loop[v];
        for &(u, w) in &level.adj[v] {
            let b = refined[u as usize];
            if a == b {
                // Each internal edge is seen from both ends.
                self_loop[a as usize] += w / 2.0;
            } else {
                *maps[a as usize].entry(b).or_insert(0.0) += w;
            }
        }
    }
    Level {
        adj: maps.into_iter().map(|m| m.into_iter().collect()).collect(),
        self_loop,
        k,
    }
}

fn run_once(
    graph: &WeightedGraph,
    initial: &[u32],
    config: &LeidenConfig,
    rng: &mut Rng,
) -> Vec<u32> {
    let n = graph.node_count();
    let m = graph.total_weight();
    let gamma_2m = config.resolution / (2.0 * m);
    let mut level = Level::from_graph(graph);
    let mut comm: Vec<u32> = initial.to_vec();
    compact(&mut comm);
    // node_of[original] = node index at the current level.
    let mut node_of: Vec<u32> = (0..n).map(to_u32).collect();
    loop {
        local_moving(&level, &mut comm, gamma_2m, rng);
        let communities = compact(&mut comm);
        if communities == level.len() {
            break;
        }
        let mut refined = refine(&level, &comm, gamma_2m, config.randomness, rng);
        let mut count = compact(&mut refined);
        if count == level.len() {
            // Refinement merged nothing: aggregate on the unrefined partition instead, so
            // the next level is strictly smaller and the loop terminates.
            refined.clone_from(&comm);
            count = communities;
        }
        // Initial partition of the aggregate: the community of its members.
        let mut next_comm = vec![0_u32; count];
        for v in 0..level.len() {
            next_comm[refined[v] as usize] = comm[v];
        }
        for x in &mut node_of {
            *x = refined[*x as usize];
        }
        level = aggregate(&level, &refined, count);
        comm = next_comm;
    }
    let mut membership: Vec<u32> = node_of.iter().map(|&x| comm[x as usize]).collect();
    compact(&mut membership);
    membership
}

/// Runs Leiden on `graph`.
pub fn leiden(graph: &WeightedGraph, config: &LeidenConfig) -> Partition {
    leiden_from(graph, None, config)
}

/// Runs Leiden starting from `initial` (e.g. yesterday's clusters, for stability); `None`
/// starts from singletons.
pub fn leiden_from(
    graph: &WeightedGraph,
    initial: Option<&[u32]>,
    config: &LeidenConfig,
) -> Partition {
    let n = graph.node_count();
    let mut rng = Rng::new(config.seed);
    let mut membership: Vec<u32> = match initial {
        Some(p) if p.len() == n => p.to_vec(),
        _ => (0..n).map(to_u32).collect(),
    };
    if graph.total_weight() > 0.0 {
        for _ in 0..config.max_runs.max(1) {
            let next = run_once(graph, &membership, config, &mut rng);
            let stable = next == membership;
            membership = next;
            if stable {
                break;
            }
        }
    }
    let count = compact(&mut membership);
    let quality = modularity(graph, &membership, config.resolution);
    Partition {
        membership,
        count,
        quality,
    }
}
