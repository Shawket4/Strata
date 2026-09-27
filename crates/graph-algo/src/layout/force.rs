//! Force-directed layout for the global map (D3 = a: positions computed in the Rust core and
//! streamed to the renderer).
//!
//! Fruchterman–Reingold forces — attraction `w · d² / k` along weighted edges, repulsion
//! `k² / d` between all pairs, and a weak spring towards the origin — with the all-pairs
//! repulsion approximated by a **Barnes–Hut quadtree** (`O(n log n)` per step, good for
//! ~10k nodes). Each [`ForceLayout::step`] moves every node along its net force by at most
//! the current temperature, then cools; callers stream the positions after each step.
//!
//! Deterministic: the initial placement comes from the seeded [`Rng`], the tree is built in
//! node order, and no step depends on hash order or time. A **warm start**
//! ([`ForceLayout::warm`]) keeps previous positions, places new nodes next to their placed
//! neighbours, and starts cooler, so an incremental update barely moves the map.

#![allow(clippy::many_single_char_names, clippy::similar_names)] // notation of the formulas (k, n, r, t, dx/dy)

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::weighted::WeightedGraph;

/// A position.
pub type Point = [f64; 2];

/// Layout parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ForceConfig {
    /// Seed of the initial placement.
    pub seed: u64,
    /// Ideal edge length `k`.
    pub ideal_length: f64,
    /// Barnes–Hut opening angle θ (0 = exact).
    pub theta: f64,
    /// Strength of the spring towards the origin.
    pub gravity: f64,
    /// Starting temperature as a multiple of `k` (cold starts).
    pub initial_temperature: f64,
    /// Starting temperature as a multiple of `k` for warm starts.
    pub warm_temperature: f64,
    /// Temperature factor per step.
    pub cooling: f64,
    /// Temperature floor as a multiple of `k`.
    pub min_temperature: f64,
}

impl Default for ForceConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            ideal_length: 30.0,
            theta: 0.9,
            gravity: 0.1,
            initial_temperature: 10.0,
            warm_temperature: 0.25,
            cooling: 0.97,
            min_temperature: 0.01,
        }
    }
}

/// A running layout.
#[derive(Debug, Clone)]
pub struct ForceLayout {
    graph: WeightedGraph,
    config: ForceConfig,
    pos: Vec<Point>,
    temperature: f64,
    steps: usize,
    last_max_move: f64,
    bound: f64,
}

const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
const MAX_DEPTH: u32 = 48;

fn index_f64(i: usize) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "node counts are far below 2^52")]
    let f = i as f64;
    f
}

impl ForceLayout {
    fn radius(n: usize, k: f64) -> f64 {
        k * index_f64(n.max(1)).sqrt()
    }

    /// A cold start: nodes placed uniformly at random in a disc.
    pub fn new(graph: WeightedGraph, config: ForceConfig) -> Self {
        let n = graph.node_count();
        let k = config.ideal_length;
        let r = Self::radius(n, k);
        let mut rng = Rng::new(config.seed);
        let pos = (0..n)
            .map(|_| {
                let a = rng.next_f64() * std::f64::consts::TAU;
                let d = r * rng.next_f64().sqrt();
                [d * a.cos(), d * a.sin()]
            })
            .collect();
        Self::with_positions(graph, config, pos, config.initial_temperature * k)
    }

    /// A warm start: `previous[i]` is node `i`'s last position, if it had one. New nodes go
    /// to the centroid of their placed neighbours (plus a small seeded offset), or to a
    /// random point when none is placed.
    pub fn warm(graph: WeightedGraph, previous: &[Option<Point>], config: ForceConfig) -> Self {
        let n = graph.node_count();
        let k = config.ideal_length;
        let r = Self::radius(n, k);
        let mut rng = Rng::new(config.seed);
        let mut pos: Vec<Option<Point>> =
            (0..n).map(|i| previous.get(i).copied().flatten()).collect();
        // Place new nodes in index order so that chains of new nodes still land together.
        for i in 0..n {
            if pos[i].is_some() {
                continue;
            }
            let u = u32::try_from(i).unwrap_or(u32::MAX);
            let placed: Vec<Point> = graph
                .neighbors(u)
                .filter_map(|(v, _)| pos[v as usize])
                .collect();
            let a = rng.next_f64() * std::f64::consts::TAU;
            pos[i] = Some(if placed.is_empty() {
                let d = r * rng.next_f64().sqrt();
                [d * a.cos(), d * a.sin()]
            } else {
                let c = index_f64(placed.len());
                let (sx, sy) = placed
                    .iter()
                    .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
                [sx / c + 0.5 * k * a.cos(), sy / c + 0.5 * k * a.sin()]
            });
        }
        let pos = pos.into_iter().map(|p| p.unwrap_or([0.0, 0.0])).collect();
        Self::with_positions(graph, config, pos, config.warm_temperature * k)
    }

    fn with_positions(
        graph: WeightedGraph,
        config: ForceConfig,
        pos: Vec<Point>,
        temperature: f64,
    ) -> Self {
        let n = graph.node_count();
        let k = config.ideal_length;
        let bound = 10.0 * Self::radius(n, k) + 10.0 * k;
        let mut layout = Self {
            graph,
            config,
            pos,
            temperature,
            steps: 0,
            last_max_move: 0.0,
            bound,
        };
        for i in 0..layout.pos.len() {
            layout.pos[i] = layout.clamp(layout.pos[i]);
        }
        layout
    }

    fn clamp(&self, p: Point) -> Point {
        if !p[0].is_finite() || !p[1].is_finite() {
            return [0.0, 0.0];
        }
        let d = p[0].hypot(p[1]);
        if d > self.bound {
            [p[0] / d * self.bound, p[1] / d * self.bound]
        } else {
            p
        }
    }

    /// Current positions (node order).
    pub fn positions(&self) -> &[Point] {
        &self.pos
    }

    /// Current temperature (maximum move of the next step).
    pub fn temperature(&self) -> f64 {
        self.temperature
    }

    /// Steps taken.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Largest move of the last step.
    pub fn last_max_move(&self) -> f64 {
        self.last_max_move
    }

    /// Every position lies within this distance of the origin.
    pub fn bound(&self) -> f64 {
        self.bound
    }

    /// Whether the temperature reached its floor (further steps only polish).
    pub fn is_cool(&self) -> bool {
        self.temperature <= self.config.min_temperature * self.config.ideal_length + 1e-12
    }

    /// Runs one step and returns the new positions.
    pub fn step(&mut self) -> &[Point] {
        let n = self.pos.len();
        let k = self.config.ideal_length;
        let tree = QuadTree::build(&self.pos);
        let mut disp = vec![[0.0_f64; 2]; n];
        for (i, d) in disp.iter_mut().enumerate() {
            *d = tree.repulsion(i, &self.pos, k * k, self.config.theta);
            let p = self.pos[i];
            d[0] -= self.config.gravity * p[0];
            d[1] -= self.config.gravity * p[1];
        }
        for (u, v, w) in self.graph.edges() {
            let (a, b) = (self.pos[u as usize], self.pos[v as usize]);
            let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
            let dist = dx.hypot(dy);
            if dist <= 0.0 {
                continue;
            }
            let f = w * dist / k; // |F| = w d² / k, times the unit vector dx / d
            disp[u as usize][0] -= dx * f;
            disp[u as usize][1] -= dy * f;
            disp[v as usize][0] += dx * f;
            disp[v as usize][1] += dy * f;
        }
        let t = self.temperature;
        let mut max_move: f64 = 0.0;
        for (i, &[fx, fy]) in disp.iter().enumerate() {
            let len = fx.hypot(fy);
            if len <= 0.0 || !len.is_finite() {
                continue;
            }
            let step = len.min(t);
            max_move = max_move.max(step);
            let p = [
                self.pos[i][0] + fx / len * step,
                self.pos[i][1] + fy / len * step,
            ];
            self.pos[i] = self.clamp(p);
        }
        self.last_max_move = max_move;
        self.temperature = (t * self.config.cooling).max(self.config.min_temperature * k);
        self.steps += 1;
        &self.pos
    }

    /// Runs `steps` steps.
    pub fn run(&mut self, steps: usize) -> &[Point] {
        for _ in 0..steps {
            self.step();
        }
        &self.pos
    }

    /// The layout energy whose negative gradient is the (exact) force field:
    /// `Σ_edges w d³ / (3k) − Σ_pairs k² ln d + Σ_nodes gravity |p|² / 2`. `O(n²)`; for tests
    /// and diagnostics.
    pub fn energy(&self) -> f64 {
        let k = self.config.ideal_length;
        let mut e = 0.0;
        for (u, v, w) in self.graph.edges() {
            let (a, b) = (self.pos[u as usize], self.pos[v as usize]);
            e += w * (a[0] - b[0]).hypot(a[1] - b[1]).powi(3) / (3.0 * k);
        }
        for i in 0..self.pos.len() {
            for j in i + 1..self.pos.len() {
                let d = (self.pos[i][0] - self.pos[j][0]).hypot(self.pos[i][1] - self.pos[j][1]);
                e -= k * k * d.max(1e-9).ln();
            }
            e += self.config.gravity * (self.pos[i][0].powi(2) + self.pos[i][1].powi(2)) / 2.0;
        }
        e
    }
}

/// Barnes–Hut quadtree over positions (unit masses).
struct QuadTree {
    cells: Vec<Cell>,
}

#[derive(Clone)]
struct Cell {
    cx: f64,
    cy: f64,
    half: f64,
    mass: f64,
    count: u32,
    sx: f64,
    sy: f64,
    children: [u32; 4],
    body: u32,
    leaf: bool,
}

const NONE: u32 = u32::MAX;

impl Cell {
    fn new(cx: f64, cy: f64, half: f64) -> Self {
        Self {
            cx,
            cy,
            half,
            mass: 0.0,
            count: 0,
            sx: 0.0,
            sy: 0.0,
            children: [NONE; 4],
            body: NONE,
            leaf: true,
        }
    }

    fn quadrant(&self, p: Point) -> usize {
        usize::from(p[0] >= self.cx) + 2 * usize::from(p[1] >= self.cy)
    }
}

impl QuadTree {
    fn build(pos: &[Point]) -> Self {
        let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in pos {
            minx = minx.min(p[0]);
            miny = miny.min(p[1]);
            maxx = maxx.max(p[0]);
            maxy = maxy.max(p[1]);
        }
        let half = ((maxx - minx).max(maxy - miny) / 2.0).max(1e-6) * 1.000_001;
        let mut tree = Self {
            cells: vec![Cell::new(
                f64::midpoint(minx, maxx),
                f64::midpoint(miny, maxy),
                half,
            )],
        };
        if pos.is_empty() {
            return tree;
        }
        for (i, &p) in pos.iter().enumerate() {
            tree.insert(u32::try_from(i).unwrap_or(NONE), p, pos);
        }
        tree
    }

    fn child(&mut self, cell: usize, q: usize) -> usize {
        let existing = self.cells[cell].children[q];
        if existing != NONE {
            return existing as usize;
        }
        let c = &self.cells[cell];
        let h = c.half / 2.0;
        let cx = if q & 1 == 1 { c.cx + h } else { c.cx - h };
        let cy = if q & 2 == 2 { c.cy + h } else { c.cy - h };
        self.cells.push(Cell::new(cx, cy, h));
        let ix = self.cells.len() - 1;
        self.cells[cell].children[q] = u32::try_from(ix).unwrap_or(NONE);
        ix
    }

    fn insert(&mut self, body: u32, p: Point, pos: &[Point]) {
        let mut cell = 0;
        let mut depth = 0;
        loop {
            let c = &mut self.cells[cell];
            c.mass += 1.0;
            c.count += 1;
            c.sx += p[0];
            c.sy += p[1];
            if c.leaf {
                if c.body == NONE && c.count == 1 {
                    c.body = body;
                    return;
                }
                if depth >= MAX_DEPTH {
                    return; // coincident bucket: mass accumulates in this leaf
                }
                // Split: push the resident body down one level.
                let resident = c.body;
                c.leaf = false;
                c.body = NONE;
                if resident != NONE {
                    let rp = pos[resident as usize];
                    let q = self.cells[cell].quadrant(rp);
                    let ch = self.child(cell, q);
                    let rc = &mut self.cells[ch];
                    rc.mass += 1.0;
                    rc.count += 1;
                    rc.sx += rp[0];
                    rc.sy += rp[1];
                    rc.body = resident;
                }
            }
            let q = self.cells[cell].quadrant(p);
            cell = self.child(cell, q);
            depth += 1;
        }
    }

    /// Repulsive displacement on body `i`: `Σ k² m (p_i − c) / d²`.
    fn repulsion(&self, i: usize, pos: &[Point], k2: f64, theta: f64) -> Point {
        let p = pos[i];
        let me = u32::try_from(i).unwrap_or(NONE);
        let mut f = [0.0, 0.0];
        let mut stack = vec![0_usize];
        while let Some(ci) = stack.pop() {
            let c = &self.cells[ci];
            if c.mass <= 0.0 {
                continue;
            }
            let (cx, cy) = (c.sx / c.mass, c.sy / c.mass);
            let (dx, dy) = (p[0] - cx, p[1] - cy);
            let d2 = dx * dx + dy * dy;
            if c.leaf {
                let mut mass = c.mass;
                if c.body == me || (d2 < 1e-18 && c.body != NONE) {
                    // Exclude ourselves (a coincident bucket holds us plus duplicates).
                    mass -= 1.0;
                }
                if mass <= 0.0 {
                    continue;
                }
                if d2 < 1e-18 {
                    // Coincident: push apart in a direction fixed by the node index.
                    let a = GOLDEN_ANGLE * index_f64(i);
                    f[0] += a.cos() * k2 * mass * 1e-3;
                    f[1] += a.sin() * k2 * mass * 1e-3;
                    continue;
                }
                f[0] += dx * k2 * mass / d2;
                f[1] += dy * k2 * mass / d2;
                continue;
            }
            let size = 2.0 * c.half;
            if d2 > 0.0 && size * size < theta * theta * d2 {
                f[0] += dx * k2 * c.mass / d2;
                f[1] += dy * k2 * c.mass / d2;
                continue;
            }
            for &ch in &c.children {
                if ch != NONE {
                    stack.push(ch as usize);
                }
            }
        }
        f
    }
}
