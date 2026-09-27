//! Radial layout for the local mind map (D4): the focus at the centre, one ring per BFS
//! depth, each node's angular wedge proportional to the number of leaves below it in the
//! BFS tree, and children ordered to reduce edge crossings.
//!
//! Crossing reduction is a circular barycenter heuristic: level by level, each parent's
//! children are sorted by the circular mean of their own angle and the angles of their
//! neighbours other than the parent (including the node itself keeps mutually connected
//! siblings from swapping past each other). Sweeps repeat [`RadialConfig::sweeps`] times.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::graph::{Graph, NodeIx};
use crate::neighbourhood::Neighbourhood;

/// Radial layout parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RadialConfig {
    /// Distance between rings.
    pub ring_spacing: f64,
    /// Crossing-reduction sweeps (0 = children in node order).
    pub sweeps: usize,
}

impl Default for RadialConfig {
    fn default() -> Self {
        Self {
            ring_spacing: 180.0,
            sweeps: 3,
        }
    }
}

/// A placed node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadialNode {
    /// Node.
    pub node: NodeIx,
    /// Ring (0 = focus).
    pub depth: u8,
    /// BFS-tree parent (`None` for the focus).
    pub parent: Option<NodeIx>,
    /// Angle in radians, `0..2π`.
    pub angle: f64,
    /// Start of the node's wedge.
    pub wedge_start: f64,
    /// Width of the node's wedge.
    pub wedge: f64,
    /// Position.
    pub x: f64,
    /// Position.
    pub y: f64,
}

struct Tree {
    nodes: Vec<NodeIx>,
    depth: BTreeMap<NodeIx, u8>,
    parent: BTreeMap<NodeIx, NodeIx>,
    children: BTreeMap<NodeIx, Vec<NodeIx>>,
    adj: BTreeMap<NodeIx, BTreeSet<NodeIx>>,
    leaves: BTreeMap<NodeIx, usize>,
}

fn build_tree(graph: &Graph, hood: &Neighbourhood) -> Tree {
    let depth: BTreeMap<NodeIx, u8> = hood.nodes.iter().copied().collect();
    let mut adj: BTreeMap<NodeIx, BTreeSet<NodeIx>> = depth.keys().map(|&n| (n, BTreeSet::new())).collect();
    for &e in &hood.edges {
        let edge = &graph.edges()[e as usize];
        if edge.source != edge.target {
            adj.entry(edge.source).or_default().insert(edge.target);
            adj.entry(edge.target).or_default().insert(edge.source);
        }
    }
    let mut parent = BTreeMap::new();
    let mut children: BTreeMap<NodeIx, Vec<NodeIx>> = depth.keys().map(|&n| (n, Vec::new())).collect();
    let nodes: Vec<NodeIx> = hood.nodes.iter().map(|&(n, _)| n).collect();
    for &(n, d) in &hood.nodes {
        if d == 0 {
            continue;
        }
        // Parent: the lowest-index neighbour one ring in.
        if let Some(&p) = adj[&n].iter().find(|m| depth.get(m) == Some(&(d - 1))) {
            parent.insert(n, p);
            children.entry(p).or_default().push(n);
        }
    }
    let mut leaves = BTreeMap::new();
    for &n in nodes.iter().rev() {
        let l = children[&n].iter().map(|c| leaves[c]).sum::<usize>().max(1);
        leaves.insert(n, l);
    }
    Tree {
        nodes,
        depth,
        parent,
        children,
        adj,
        leaves,
    }
}

fn norm(a: f64) -> f64 {
    a.rem_euclid(TAU)
}

fn count_f64(n: usize) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "subtree sizes are small")]
    let f = n as f64;
    f
}

/// Assigns wedges top-down from the current child orders.
fn assign(tree: &Tree, focus: NodeIx, wedges: &mut BTreeMap<NodeIx, (f64, f64)>) {
    wedges.insert(focus, (0.0, TAU));
    for &n in &tree.nodes {
        let (start, width) = wedges[&n];
        let total = count_f64(tree.leaves[&n]);
        let mut at = start;
        for c in &tree.children[&n] {
            let w = width * count_f64(tree.leaves[c]) / total;
            wedges.insert(*c, (at, w));
            at += w;
        }
    }
}

fn angle_of(wedges: &BTreeMap<NodeIx, (f64, f64)>, n: NodeIx) -> f64 {
    let (s, w) = wedges[&n];
    s + w / 2.0
}

/// Lays out a neighbourhood radially.
pub fn radial_layout(graph: &Graph, hood: &Neighbourhood, config: &RadialConfig) -> Vec<RadialNode> {
    let mut tree = build_tree(graph, hood);
    let focus = hood.focus;
    let mut wedges = BTreeMap::new();
    assign(&tree, focus, &mut wedges);
    let max_depth = tree.depth.values().copied().max().unwrap_or(0);
    for _ in 0..config.sweeps {
        for d in 0..max_depth {
            let parents: Vec<NodeIx> = tree.nodes.iter().copied().filter(|n| tree.depth[n] == d).collect();
            for p in parents {
                let (start, _) = wedges[&p];
                let mut keyed: Vec<(f64, f64, NodeIx)> = tree.children[&p]
                    .iter()
                    .map(|&c| {
                        let own = angle_of(&wedges, c);
                        let (mut sx, mut sy) = (own.cos(), own.sin());
                        for &m in &tree.adj[&c] {
                            if Some(&m) != tree.parent.get(&c) {
                                let a = angle_of(&wedges, m);
                                sx += a.cos();
                                sy += a.sin();
                            }
                        }
                        let bary = if sx.hypot(sy) < 1e-9 { own } else { sy.atan2(sx) };
                        // Compare relative to the parent's wedge start to avoid wrap-around.
                        (norm(bary - start), norm(own - start), c)
                    })
                    .collect();
                keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
                if let Some(ch) = tree.children.get_mut(&p) {
                    *ch = keyed.into_iter().map(|k| k.2).collect();
                }
                assign(&tree, focus, &mut wedges);
            }
        }
    }
    tree.nodes
        .iter()
        .map(|&n| {
            let (wedge_start, wedge) = wedges[&n];
            let depth = tree.depth[&n];
            let angle = if depth == 0 { 0.0 } else { norm(wedge_start + wedge / 2.0) };
            let r = f64::from(depth) * config.ring_spacing;
            RadialNode {
                node: n,
                depth,
                parent: tree.parent.get(&n).copied(),
                angle,
                wedge_start,
                wedge,
                x: r * angle.cos(),
                y: r * angle.sin(),
            }
        })
        .collect()
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Number of proper crossings between the straight edges of `hood` in `layout` (edges that
/// share an endpoint do not count).
pub fn edge_crossings(graph: &Graph, hood: &Neighbourhood, layout: &[RadialNode]) -> usize {
    let at: BTreeMap<NodeIx, (f64, f64)> = layout.iter().map(|n| (n.node, (n.x, n.y))).collect();
    let mut segs: BTreeSet<(NodeIx, NodeIx)> = BTreeSet::new();
    for &e in &hood.edges {
        let edge = &graph.edges()[e as usize];
        if edge.source != edge.target {
            segs.insert((edge.source.min(edge.target), edge.source.max(edge.target)));
        }
    }
    let segs: Vec<(NodeIx, NodeIx)> = segs.into_iter().collect();
    let mut count = 0;
    for (i, &(a, b)) in segs.iter().enumerate() {
        for &(c, d) in &segs[i + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            let (pa, pb, pc, pd) = (at[&a], at[&b], at[&c], at[&d]);
            let (d1, d2) = (cross(pc, pd, pa), cross(pc, pd, pb));
            let (d3, d4) = (cross(pa, pb, pc), cross(pa, pb, pd));
            if d1 * d2 < -1e-9 && d3 * d4 < -1e-9 {
                count += 1;
            }
        }
    }
    count
}
