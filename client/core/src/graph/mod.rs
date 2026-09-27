//! Local graph (PLAN §12.1 `graph/`): the typed graph built from the cached notes, links,
//! relations and custody fields, with the shared `graph-algo` crate (L16) doing the work —
//! neighbourhoods and the radial layout for the local mind map (D4), and the force-directed
//! layout for the global map (D3), warm-started from positions cached in the account database
//! so the map stays stable between openings.

use std::collections::HashMap;

use domain::{
    CustodyEdge, EntityRelationType, GraphEdgeKind, GraphNodeKind, MentionType, RelationOrigin,
    RelationType,
};
use graph_algo::layout::{ForceConfig, ForceLayout, Point, RadialConfig, radial_layout};
use graph_algo::neighbourhood::{Filter, neighbourhood};
use graph_algo::weighted::{EdgeWeights, WeightedGraph};
use graph_algo::{EdgeInput, Graph, GraphBuilder};
use rusqlite::{Connection, params};

use crate::error::{CoreError, CoreResult};
use crate::view::model::{ClusterLabel, GlobalGraphView, GraphEdge, GraphNode, LocalGraphView};

/// Steps of the force layout per global-map build (warm starts converge in few steps).
pub const FORCE_STEPS: usize = 300;

struct Loaded {
    graph: Graph,
    titles: HashMap<String, String>,
}

fn edge_kind(rel: &str) -> Option<GraphEdgeKind> {
    if let Ok(t) = rel.parse::<RelationType>() {
        return Some(GraphEdgeKind::Relation(t));
    }
    if let Ok(m) = rel.parse::<MentionType>() {
        return Some(match m {
            MentionType::Concepts => GraphEdgeKind::Concept,
            _ => GraphEdgeKind::Mention,
        });
    }
    rel.parse::<EntityRelationType>()
        .ok()
        .map(GraphEdgeKind::Entity)
}

fn gb_err(e: &graph_algo::GraphError) -> CoreError {
    CoreError::Internal(format!("graph: {e}"))
}

/// Builds the typed graph of every live note.
fn load(conn: &Connection) -> CoreResult<Loaded> {
    let mut b = GraphBuilder::new();
    let mut titles = HashMap::new();
    {
        let mut st =
            conn.prepare("SELECT id, title, kind FROM notes WHERE deleted = 0 ORDER BY id")?;
        let rows: Vec<(String, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, title, kind) in rows {
            let kind = kind
                .parse::<domain::NoteKind>()
                .map_or(GraphNodeKind::Note, GraphNodeKind::from);
            b.add_node(&id, kind).map_err(|e| gb_err(&e))?;
            titles.insert(id, title);
        }
    }
    let mut edges: Vec<EdgeInput> = Vec::new();
    {
        let mut st = conn.prepare(
            "SELECT DISTINCT l.note_id, l.dst_id, l.kind FROM links l
             WHERE l.dst_id IS NOT NULL AND l.dst_id != l.note_id ORDER BY 1, 2, 3",
        )?;
        let rows: Vec<(String, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (src, dst, kind) in rows {
            let k = if kind == "embed" {
                GraphEdgeKind::Embed
            } else {
                GraphEdgeKind::Link
            };
            edges.push(EdgeInput::user(&src, &dst, k));
        }
    }
    {
        let mut st = conn.prepare(
            "SELECT r.src_id, r.dst_id, r.rel_type, m.by, m.confidence FROM relations r
             LEFT JOIN relation_meta m
               ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
             WHERE r.dst_id IS NOT NULL AND r.dst_id != r.src_id ORDER BY 1, 2, 3",
        )?;
        let rows: Vec<(String, String, String, Option<String>, Option<f64>)> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        for (src, dst, rel, by, confidence) in rows {
            let Some(kind) = edge_kind(&rel) else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation)] // confidences are in [0, 1]
            let confidence = confidence.map(|c| c as f32);
            edges.push(EdgeInput {
                source: src,
                target: dst,
                kind,
                by: by
                    .and_then(|b| b.parse::<RelationOrigin>().ok())
                    .unwrap_or(RelationOrigin::User),
                confidence,
            });
        }
    }
    {
        let mut st = conn.prepare(
            "SELECT note_id, location_id, holder_id, last_holder_id FROM documents ORDER BY note_id",
        )?;
        let rows: Vec<(String, Option<String>, Option<String>, Option<String>)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        for (doc, loc, holder, last) in rows {
            for (target, edge) in [
                (loc, CustodyEdge::Location),
                (holder, CustodyEdge::Holder),
                (last, CustodyEdge::LastHolder),
            ] {
                if let Some(t) = target {
                    edges.push(EdgeInput::user(&doc, &t, GraphEdgeKind::Custody(edge)));
                }
            }
        }
        let mut st = conn.prepare(
            "SELECT note_id, parent_id FROM places WHERE parent_id IS NOT NULL ORDER BY note_id",
        )?;
        let rows: Vec<(String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (place, parent) in rows {
            edges.push(EdgeInput::user(&place, &parent, GraphEdgeKind::PartOfPlace));
        }
    }
    for e in &edges {
        // Links to notes deleted locally but still cached are skipped.
        if titles.contains_key(&e.source) && titles.contains_key(&e.target) {
            b.add_edge(e).map_err(|e| gb_err(&e))?;
        }
    }
    Ok(Loaded {
        graph: b.build(),
        titles,
    })
}

fn edge_views(g: &Graph, which: impl Iterator<Item = u32>) -> Vec<GraphEdge> {
    which
        .map(|i| {
            let e = &g.edges()[i as usize];
            GraphEdge {
                src: g.node(e.source).key.clone(),
                dst: g.node(e.target).key.clone(),
                kind: e.kind.to_string(),
                by: Some(e.by.as_str().to_owned()),
                confidence: e.confidence.map(f64::from),
            }
        })
        .collect()
}

fn degree(g: &Graph, ix: u32) -> u32 {
    u32::try_from(g.degree(ix)).unwrap_or(u32::MAX)
}

/// The neighbourhood of `id` up to `depth` (1–3), laid out radially.
pub fn local_graph(conn: &Connection, id: &str, depth: u8) -> CoreResult<LocalGraphView> {
    let loaded = load(conn)?;
    let g = &loaded.graph;
    let Some(focus) = g.index_of(id) else {
        return Ok(LocalGraphView::NotFound { id: id.to_owned() });
    };
    let depth = depth.clamp(1, 3);
    let hood = neighbourhood(g, focus, depth, &Filter::default())
        .map_err(|e| CoreError::invalid("depth", &e.to_string()))?;
    let placed = radial_layout(g, &hood, &RadialConfig::default());
    let pos: HashMap<u32, (f64, f64)> = placed.iter().map(|p| (p.node, (p.x, p.y))).collect();
    let nodes = hood
        .nodes
        .iter()
        .map(|&(ix, d)| {
            let n = g.node(ix);
            let (x, y) = pos.get(&ix).copied().unwrap_or((0.0, 0.0));
            GraphNode {
                id: n.key.clone(),
                title: loaded.titles.get(&n.key).cloned().unwrap_or_default(),
                kind: n.kind.as_str().to_owned(),
                depth: d,
                cluster_id: None,
                degree: degree(g, ix),
                x,
                y,
            }
        })
        .collect();
    Ok(LocalGraphView::Ready {
        center: id.to_owned(),
        depth,
        nodes,
        edges: edge_views(g, hood.edges.iter().copied()),
    })
}

/// The global map: every note, force-directed positions warm-started from (and saved to) the
/// position cache.
pub fn global_graph(conn: &Connection) -> CoreResult<GlobalGraphView> {
    let loaded = load(conn)?;
    let g = &loaded.graph;
    let cached: HashMap<String, Point> = {
        let mut st = conn.prepare("SELECT note_id, x, y FROM graph_positions")?;
        st.query_map([], |r| Ok((r.get::<_, String>(0)?, [r.get(1)?, r.get(2)?])))?
            .collect::<Result<_, _>>()?
    };
    let previous: Vec<Option<Point>> = g
        .nodes()
        .iter()
        .map(|n| cached.get(&n.key).copied())
        .collect();
    let weighted = WeightedGraph::project(g, &EdgeWeights::default());
    let mut layout = if previous.iter().any(Option::is_some) {
        ForceLayout::warm(weighted, &previous, ForceConfig::default())
    } else {
        ForceLayout::new(weighted, ForceConfig::default())
    };
    let positions = layout.run(FORCE_STEPS).to_vec();
    conn.execute("DELETE FROM graph_positions", [])?;
    for (n, p) in g.nodes().iter().zip(&positions) {
        conn.execute(
            "INSERT INTO graph_positions (note_id, x, y) VALUES (?1, ?2, ?3)",
            params![n.key, p[0], p[1]],
        )?;
    }
    let clusters_of: HashMap<String, String> = {
        let mut st = conn.prepare("SELECT note_id, cluster_id FROM clusters")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let nodes: Vec<GraphNode> = g
        .nodes()
        .iter()
        .zip(&positions)
        .enumerate()
        .map(|(i, (n, p))| GraphNode {
            id: n.key.clone(),
            title: loaded.titles.get(&n.key).cloned().unwrap_or_default(),
            kind: n.kind.as_str().to_owned(),
            depth: 0,
            cluster_id: clusters_of.get(&n.key).cloned(),
            degree: degree(g, u32::try_from(i).unwrap_or(u32::MAX)),
            x: p[0],
            y: p[1],
        })
        .collect();
    let mut clusters: Vec<ClusterLabel> = {
        let mut st = conn.prepare(
            "SELECT n.cluster_id, n.name, COUNT(c.note_id) FROM cluster_names n
             LEFT JOIN clusters c ON c.cluster_id = n.cluster_id GROUP BY n.cluster_id, n.name",
        )?;
        st.query_map([], |r| {
            Ok(ClusterLabel {
                id: r.get(0)?,
                name: r.get(1)?,
                size: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?
    };
    clusters.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    let edge_count = u32::try_from(g.edge_count()).unwrap_or(u32::MAX);
    Ok(GlobalGraphView {
        nodes,
        edges: edge_views(g, 0..edge_count),
        clusters,
    })
}
