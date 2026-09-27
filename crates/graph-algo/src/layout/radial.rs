//! Radial layout for the local mind map (D4): the focus at the centre, one ring per BFS
//! depth, each node's angular wedge proportional to the number of leaves below it in the
//! BFS tree, and children ordered to reduce edge crossings.
//!
//! Crossing reduction is a barycenter heuristic: level by level, each parent's children are
//! sorted by the mean of their own angle and the angles of their neighbours other than the
//! parent, all measured from the start of the parent's wedge (including the node itself keeps
//! mutually connected siblings from swapping past each other). Sweeps repeat
//! [`RadialConfig::sweeps`] times.

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
    let mut adj: BTreeMap<NodeIx, BTreeSet<NodeIx>> =
        depth.keys().map(|&n| (n, BTreeSet::new())).collect();
    for &e in &hood.edges {
        let edge = &graph.edges()[e as usize];
        if edge.source != edge.target {
            adj.entry(edge.source).or_default().insert(edge.target);
            adj.entry(edge.target).or_default().insert(edge.source);
        }
    }
    let mut parent = BTreeMap::new();
    let mut children: BTreeMap<NodeIx, Vec<NodeIx>> =
        depth.keys().map(|&n| (n, Vec::new())).collect();
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

fn positions(
    tree: &Tree,
    wedges: &BTreeMap<NodeIx, (f64, f64)>,
    spacing: f64,
) -> BTreeMap<NodeIx, (f64, f64)> {
    tree.nodes
        .iter()
        .map(|&n| {
            let depth = tree.depth[&n];
            let angle = if depth == 0 {
                0.0
            } else {
                norm(angle_of(wedges, n))
            };
            let r = f64::from(depth) * spacing;
            (n, (r * angle.cos(), r * angle.sin()))
        })
        .collect()
}

/// Lays out a neighbourhood radially. Sweeps that do not reduce the number of crossings are
/// undone, so the result never has more crossings than the initial (node-order) layout.
pub fn radial_layout(
    graph: &Graph,
    hood: &Neighbourhood,
    config: &RadialConfig,
) -> Vec<RadialNode> {
    let mut tree = build_tree(graph, hood);
    let focus = hood.focus;
    let mut wedges = BTreeMap::new();
    assign(&tree, focus, &mut wedges);
    let segs = segments(graph, hood);
    let mut best_children = tree.children.clone();
    let mut best = crossings(&positions(&tree, &wedges, config.ring_spacing), &segs);
    let max_depth = tree.depth.values().copied().max().unwrap_or(0);
    for _ in 0..config.sweeps {
        if best == 0 {
            break;
        }
        for d in 0..max_depth {
            let parents: Vec<NodeIx> = tree
                .nodes
                .iter()
                .copied()
                .filter(|n| tree.depth[n] == d)
                .collect();
            for p in parents {
                let (start, _) = wedges[&p];
                let mut keyed: Vec<(f64, f64, NodeIx)> = tree.children[&p]
                    .iter()
                    .map(|&c| {
                        // Angles relative to the parent's wedge start (no wrap-around), so
                        // antipodal neighbours do not cancel out as they would in a
                        // circular mean.
                        let own = norm(angle_of(&wedges, c) - start);
                        let mut sum = own;
                        let mut count = 1.0;
                        for &m in &tree.adj[&c] {
                            if Some(&m) != tree.parent.get(&c) {
                                sum += norm(angle_of(&wedges, m) - start);
                                count += 1.0;
                            }
                        }
                        (sum / count, own, c)
                    })
                    .collect();
                keyed.sort_by(|a, b| {
                    a.0.total_cmp(&b.0)
                        .then(a.1.total_cmp(&b.1))
                        .then(a.2.cmp(&b.2))
                });
                if let Some(ch) = tree.children.get_mut(&p) {
                    *ch = keyed.into_iter().map(|k| k.2).collect();
                }
                assign(&tree, focus, &mut wedges);
            }
        }
        let now = crossings(&positions(&tree, &wedges, config.ring_spacing), &segs);
        if now < best {
            best = now;
            best_children.clone_from(&tree.children);
        }
    }
    tree.children = best_children;
    assign(&tree, focus, &mut wedges);
    tree.nodes
        .iter()
        .map(|&n| {
            let (wedge_start, wedge) = wedges[&n];
            let depth = tree.depth[&n];
            let angle = if depth == 0 {
                0.0
            } else {
                norm(wedge_start + wedge / 2.0)
            };
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

fn segments(graph: &Graph, hood: &Neighbourhood) -> Vec<(NodeIx, NodeIx)> {
    let mut segs: BTreeSet<(NodeIx, NodeIx)> = BTreeSet::new();
    for &e in &hood.edges {
        let edge = &graph.edges()[e as usize];
        if edge.source != edge.target {
            segs.insert((edge.source.min(edge.target), edge.source.max(edge.target)));
        }
    }
    segs.into_iter().collect()
}

fn crossings(at: &BTreeMap<NodeIx, (f64, f64)>, segs: &[(NodeIx, NodeIx)]) -> usize {
    let mut count = 0;
    for (i, &(a, b)) in segs.iter().enumerate() {
        for &(c, d) in &segs[i + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            let (pa, pb, pc, pd) = (at[&a], at[&b], at[&c], at[&d]);
            let len = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
            // Orientation signs with a tolerance relative to the segment lengths, so points
            // that lie on the other segment's line (up to rounding) never count.
            let eps = 1e-9 * len(pa, pb).max(1.0) * len(pc, pd).max(1.0);
            let sign = |x: f64| {
                if x.abs() <= eps {
                    0
                } else if x > 0.0 {
                    1
                } else {
                    -1
                }
            };
            let (d1, d2) = (sign(cross(pc, pd, pa)), sign(cross(pc, pd, pb)));
            let (d3, d4) = (sign(cross(pa, pb, pc)), sign(cross(pa, pb, pd)));
            if d1 * d2 < 0 && d3 * d4 < 0 {
                count += 1;
            }
        }
    }
    count
}

/// Number of proper crossings between the straight edges of `hood` in `layout` (edges that
/// share an endpoint, or touch without crossing, do not count).
pub fn edge_crossings(graph: &Graph, hood: &Neighbourhood, layout: &[RadialNode]) -> usize {
    let at: BTreeMap<NodeIx, (f64, f64)> = layout.iter().map(|n| (n.node, (n.x, n.y))).collect();
    crossings(&at, &segments(graph, hood))
}
