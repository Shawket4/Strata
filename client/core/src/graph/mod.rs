//! Local graph (PLAN §12.1 `graph/`): the typed graph built from the cached notes, links,
//! relations and custody fields, with the shared `graph-algo` crate (L16) doing the work —
//! neighbourhoods and the radial layout for the local mind map (D4), and the force-directed
//! layout for the global map (D3), warm-started from positions cached in the account database
//! so the map stays stable between openings.

// SQL rows are read into tuples right where the query is written.
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

use domain::{
    CustodyEdge, DocumentRelationType, GraphEdgeKind, GraphNodeKind as DKind, NoteKind,
    RelationOrigin,
};
use graph_algo::layout::{ForceConfig, ForceLayout, Point, RadialConfig, radial_layout};
use graph_algo::neighbourhood::{Filter, neighbourhood};
use graph_algo::weighted::{EdgeWeights, WeightedGraph};
use graph_algo::{EdgeInput, Graph, GraphBuilder};
use rusqlite::{Connection, params};

use crate::error::{CoreError, CoreResult};
use crate::format::direction::dir_of;
use crate::format::labels::{self, Lang};
use crate::view::ViewCtx;
use crate::view::model::{
    Availability, ClusterLabel, GlobalGraphView, GraphEdge, GraphFilter, GraphLens, GraphNode,
    GraphNodeKind, GraphPoint, KindCount, LocalGraphView,
};

/// Steps of the force layout per global-map build (warm starts converge in few steps).
pub const FORCE_STEPS: usize = 300;

struct Loaded {
    graph: Graph,
    titles: HashMap<String, String>,
    paths: HashMap<String, String>,
    summaries: HashMap<String, String>,
    updated: HashMap<String, String>,
    reasons: HashMap<(String, String, String), String>,
}

/// Degree from which a node is a hub (label always shown).
pub const HUB_DEGREE: u32 = 6;

/// Label priority by degree: 0 hubs, 1 well connected, 2 connected, 3 isolated.
pub fn label_rank(degree: u32) -> u32 {
    match degree {
        d if d >= HUB_DEGREE => 0,
        3..=5 => 1,
        1..=2 => 2,
        _ => 3,
    }
}

fn gb_err(e: &graph_algo::GraphError) -> CoreError {
    CoreError::Internal(format!("graph: {e}"))
}

/// The view kind of a graph node kind.
fn view_kind(k: DKind) -> GraphNodeKind {
    match k {
        DKind::Concept => GraphNodeKind::Concept,
        DKind::Person => GraphNodeKind::Person,
        DKind::Company => GraphNodeKind::Company,
        DKind::Document => GraphNodeKind::Document,
        DKind::Place => GraphNodeKind::Place,
        DKind::Tag => GraphNodeKind::Tag,
        DKind::Note | DKind::Attachment | DKind::Cluster => GraphNodeKind::Note,
    }
}

/// The ID of a tag node (as the server's graph: the tag in lowercase).
pub fn tag_node_id(tag: &str) -> String {
    format!("tag:{}", tag.trim_start_matches('#').to_lowercase())
}

/// Builds the typed graph of every live note — the same nodes and edge kinds as the server's
/// graph (`domain::GraphEdgeKind::of_relation` maps stored relations, L16): body links and
/// embeds, relations, mentions, concepts, entity relations, custody fields, place nesting,
/// document copies, and, with `include_tags`, tag nodes.
#[allow(clippy::too_many_lines)] // one query per source table
fn load(conn: &Connection, include_tags: bool) -> CoreResult<Loaded> {
    let mut b = GraphBuilder::new();
    let mut titles = HashMap::new();
    let mut paths = HashMap::new();
    let mut summaries = HashMap::new();
    let mut updated = HashMap::new();
    let mut reasons = HashMap::new();
    let mut kinds: HashMap<String, NoteKind> = HashMap::new();
    {
        let mut st = conn.prepare(
            "SELECT id, title, kind, summary, local_updated_at, path FROM notes
             WHERE deleted = 0 ORDER BY id",
        )?;
        let rows: Vec<(String, String, String, Option<String>, String, String)> = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        for (id, title, kind, summary, at, path) in rows {
            let kind = kind.parse::<NoteKind>().unwrap_or(NoteKind::Note);
            b.add_node(&id, DKind::from(kind)).map_err(|e| gb_err(&e))?;
            kinds.insert(id.clone(), kind);
            if let Some(s) = summary {
                summaries.insert(id.clone(), s);
            }
            updated.insert(id.clone(), at);
            paths.insert(id.clone(), path);
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
            "SELECT r.src_id, r.dst_id, r.rel_type, m.by, m.confidence, m.reason FROM relations r
             LEFT JOIN relation_meta m
               ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
             WHERE r.dst_id IS NOT NULL AND r.dst_id != r.src_id ORDER BY 1, 2, 3",
        )?;
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<f64>,
            Option<String>,
        )> = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        for (src, dst, rel, by, confidence, reason) in rows {
            let (Some(sk), Some(dk)) = (kinds.get(&src), kinds.get(&dst)) else {
                continue;
            };
            let Some(kind) = GraphEdgeKind::of_relation(&rel, *sk, *dk) else {
                continue;
            };
            if let Some(reason) = reason {
                reasons.insert((src.clone(), kind.to_string(), dst.clone()), reason);
            }
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
            "SELECT note_id, location_id, holder_id, last_holder_id, copy_of_id FROM documents
             ORDER BY note_id",
        )?;
        let rows: Vec<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        for (doc, loc, holder, last, copy_of) in rows {
            for (target, edge) in [
                (loc, GraphEdgeKind::Custody(CustodyEdge::Location)),
                (holder, GraphEdgeKind::Custody(CustodyEdge::Holder)),
                (last, GraphEdgeKind::Custody(CustodyEdge::LastHolder)),
                (
                    copy_of,
                    GraphEdgeKind::Document(DocumentRelationType::CopyOf),
                ),
            ] {
                if let Some(t) = target {
                    edges.push(EdgeInput::user(&doc, &t, edge));
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
    if include_tags {
        let mut st = conn.prepare(
            "SELECT DISTINCT t.note_id, t.tag FROM tags t JOIN notes n ON n.id = t.note_id
             WHERE n.deleted = 0 ORDER BY 2, 1",
        )?;
        let rows: Vec<(String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (note, tag) in rows {
            let id = tag_node_id(&tag);
            if !titles.contains_key(&id) {
                b.add_node(&id, DKind::Tag).map_err(|e| gb_err(&e))?;
                titles.insert(id.clone(), tag.trim_start_matches('#').to_lowercase());
            }
            edges.push(EdgeInput::user(&note, &id, GraphEdgeKind::Tag));
        }
    }
    // One edge per (source, target, kind): place nesting is both a `part-of` relation and
    // the places index, and must appear once.
    let mut seen = std::collections::HashSet::new();
    for e in &edges {
        // Links to notes deleted locally but still cached are skipped.
        if titles.contains_key(&e.source)
            && titles.contains_key(&e.target)
            && seen.insert((e.source.clone(), e.target.clone(), e.kind))
        {
            b.add_edge(e).map_err(|e| gb_err(&e))?;
        }
    }
    Ok(Loaded {
        graph: b.build(),
        titles,
        paths,
        summaries,
        updated,
        reasons,
    })
}

/// The family of an edge kind for filters (`relation:works-at` → `relation`).
pub fn edge_family(kind: &str) -> &str {
    kind.split_once(':').map_or(kind, |(f, _)| f)
}

/// Whether `kind` is selected by `kinds` (a family or the full kind; empty selects all).
pub fn edge_kind_selected(kinds: &[String], kind: &str) -> bool {
    kinds.is_empty() || kinds.iter().any(|k| k == edge_family(kind) || k == kind)
}

/// Keeps the edges whose kind `kinds` selects (the maps' edge-type toggles).
pub fn retain_edge_kinds(edges: &mut Vec<GraphEdge>, kinds: &[String]) {
    edges.retain(|e| edge_kind_selected(kinds, &e.kind));
}

fn rel_type_of(kind: &str) -> Option<String> {
    match kind.split_once(':') {
        Some(("relation" | "entity" | "document", t)) => Some(t.to_owned()),
        _ => None,
    }
}

fn edge_view(loaded: &Loaded, i: u32, lang: Lang) -> GraphEdge {
    let g = &loaded.graph;
    let e = &g.edges()[i as usize];
    let src = g.node(e.source).key.clone();
    let dst = g.node(e.target).key.clone();
    let kind = e.kind.to_string();
    let rel_type = rel_type_of(&kind);
    GraphEdge {
        id: format!("{src}|{kind}|{dst}"),
        label: labels::relation_label(rel_type.as_deref().unwrap_or(edge_family(&kind)), lang),
        reason: loaded
            .reasons
            .get(&(src.clone(), kind.clone(), dst.clone()))
            .cloned(),
        rel_type,
        by: Some(e.by.as_str().to_owned()),
        confidence: e.confidence.map(f64::from),
        src,
        dst,
        kind,
    }
}

fn degree(g: &Graph, ix: u32) -> u32 {
    u32::try_from(g.degree(ix)).unwrap_or(u32::MAX)
}

#[allow(clippy::many_single_char_names)] // x and y are the layout's names
fn node_view(
    loaded: &Loaded,
    ctx: &ViewCtx,
    ix: u32,
    depth: u8,
    cluster_id: Option<String>,
    (x, y): (f64, f64),
) -> GraphNode {
    let g = &loaded.graph;
    let n = g.node(ix);
    let title = loaded.titles.get(&n.key).cloned().unwrap_or_default();
    let d = degree(g, ix);
    let labels = ctx.labels();
    let updated = loaded
        .updated
        .get(&n.key)
        .map(|u| crate::view::build::ts(u));
    GraphNode {
        id: n.key.clone(),
        title_dir: dir_of(&title),
        title,
        kind: view_kind(n.kind),
        path: loaded.paths.get(&n.key).cloned(),
        updated,
        depth,
        cluster_id,
        degree: d,
        x,
        y,
        summary: loaded.summaries.get(&n.key).cloned(),
        updated_label: updated
            .map(|t| labels.date_in_list(labels.local(t).date()))
            .unwrap_or_default(),
        label_rank: label_rank(d),
        is_hub: d >= HUB_DEGREE,
    }
}

/// The neighbourhood of `id` up to `depth` (1–3), laid out radially.
pub fn local_graph(
    conn: &Connection,
    ctx: &ViewCtx,
    id: &str,
    depth: u8,
) -> CoreResult<LocalGraphView> {
    let loaded = load(conn, false)?;
    let g = &loaded.graph;
    let lang = ctx.lang;
    let Some(focus) = g.index_of(id) else {
        return Ok(LocalGraphView {
            center: id.to_owned(),
            found: false,
            depth: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            relation_count: 0,
            ai_relation_count: 0,
            relation_label: String::new(),
            summary: None,
            save_layout: Availability::NotAllowed,
            propose_relation: Availability::NotYetAvailable,
        });
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
            node_view(
                &loaded,
                ctx,
                ix,
                d,
                None,
                pos.get(&ix).copied().unwrap_or((0.0, 0.0)),
            )
        })
        .collect();
    let edges: Vec<GraphEdge> = hood
        .edges
        .iter()
        .map(|&i| edge_view(&loaded, i, lang))
        .collect();
    let touching: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| (e.src == id || e.dst == id) && e.rel_type.is_some())
        .collect();
    let relation_count = u32::try_from(touching.len()).unwrap_or(u32::MAX);
    let ai_relation_count = u32::try_from(
        touching
            .iter()
            .filter(|e| e.by.as_deref() == Some("ai"))
            .count(),
    )
    .unwrap_or(u32::MAX);
    let relation_label = match lang {
        Lang::En => format!(
            "{} · {ai_relation_count} by AI",
            labels::RELATIONS.of(i64::from(relation_count), lang)
        ),
        Lang::Ar => format!(
            "{} · {ai_relation_count} بواسطة الذكاء الاصطناعي",
            labels::RELATIONS.of(i64::from(relation_count), lang)
        ),
    };
    Ok(LocalGraphView {
        center: id.to_owned(),
        found: true,
        depth,
        nodes,
        edges,
        relation_count,
        ai_relation_count,
        relation_label,
        summary: loaded.summaries.get(id).cloned(),
        save_layout: if ctx.connectivity == crate::view::model::Connectivity::Offline {
            Availability::Offline
        } else {
            Availability::Available
        },
        // Proposing a relation type with AI needs an endpoint that does not exist yet.
        propose_relation: Availability::NotYetAvailable,
    })
}

/// A JSON Canvas (PLAN §6.8) of a mind-map layout: one file node per placed note (by its
/// vault path, positions rounded), and the local graph's edges between placed notes, labelled
/// with their relation type.
pub fn layout_canvas(
    conn: &Connection,
    ctx: &ViewCtx,
    center: &str,
    positions: &[crate::view::model::NodePosition],
) -> CoreResult<String> {
    use vault_format::canvas::{Canvas, Edge, Node, NodeKind};
    let view = local_graph(conn, ctx, center, 3)?;
    if !view.found {
        return Err(CoreError::not_found("note"));
    }
    let mut canvas = Canvas::default();
    let mut placed = std::collections::HashSet::new();
    for p in positions {
        let Some(path) = crate::store::notes::current(conn, &p.id)?.map(|n| n.path) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)] // layout units are small
        canvas.nodes.push(Node {
            id: p.id.clone(),
            kind: NodeKind::File {
                file: path,
                subpath: None,
            },
            x: p.x.round() as i64,
            y: p.y.round() as i64,
            width: 250,
            height: 60,
            color: None,
            extra: serde_json::Map::default(),
        });
        placed.insert(p.id.clone());
    }
    for (i, e) in view
        .edges
        .iter()
        .filter(|e| placed.contains(&e.src) && placed.contains(&e.dst))
        .enumerate()
    {
        canvas.edges.push(Edge {
            id: format!("e{i}"),
            from_node: e.src.clone(),
            from_side: None,
            from_end: None,
            to_node: e.dst.clone(),
            to_side: None,
            to_end: None,
            color: None,
            label: e.rel_type.clone(),
            extra: serde_json::Map::default(),
        });
    }
    Ok(canvas.to_json())
}

/// Convex hull (Andrew's monotone chain), counter-clockwise, without collinear points.
fn convex_hull(mut pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Padding of cluster regions around their members (layout units).
pub const REGION_PAD: f64 = 24.0;

fn region(members: &[(f64, f64)]) -> (f64, f64, Vec<GraphPoint>, f64) {
    #[allow(clippy::cast_precision_loss)] // member counts are small
    let n = members.len().max(1) as f64;
    let cx = members.iter().map(|p| p.0).sum::<f64>() / n;
    let cy = members.iter().map(|p| p.1).sum::<f64>() / n;
    let radius = members
        .iter()
        .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt())
        .fold(0.0_f64, f64::max)
        + REGION_PAD;
    let hull = convex_hull(members.to_vec())
        .into_iter()
        .map(|(x, y)| {
            let (dx, dy) = (x - cx, y - cy);
            let len = (dx * dx + dy * dy).sqrt();
            if len == 0.0 {
                GraphPoint { x, y }
            } else {
                GraphPoint {
                    x: x + dx / len * REGION_PAD,
                    y: y + dy / len * REGION_PAD,
                }
            }
        })
        .collect();
    (cx, cy, hull, radius)
}

/// The global map with every note (lens Notes, no filter).
pub fn global_graph(conn: &Connection, ctx: &ViewCtx) -> CoreResult<GlobalGraphView> {
    global_graph_filtered(
        conn,
        ctx,
        &GraphFilter {
            edge_kinds: Vec::new(),
            node_kinds: Vec::new(),
            similarity: false,
            cluster: None,
            lens: GraphLens::Notes,
            focus: None,
            include_tags: false,
        },
    )
}

fn kind_counts<'a>(kinds: impl Iterator<Item = &'a str>, lang: Lang, node: bool) -> Vec<KindCount> {
    let mut m: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for k in kinds {
        *m.entry(k.to_owned()).or_default() += 1;
    }
    m.into_iter()
        .map(|(kind, count)| KindCount {
            label: if node {
                labels::kind_label(&kind, lang)
            } else {
                labels::edge_kind_label(&kind, lang)
            },
            kind,
            count,
        })
        .collect()
}

/// The global map: force-directed positions of the whole graph (warm-started from, and saved
/// to, the position cache, so filters never move nodes), then the lens and the filters applied
/// here (L15: the painter only paints).
#[allow(clippy::too_many_lines)] // one pass per filter step
pub fn global_graph_filtered(
    conn: &Connection,
    ctx: &ViewCtx,
    filter: &GraphFilter,
) -> CoreResult<GlobalGraphView> {
    let loaded = load(conn, filter.include_tags)?;
    let g = &loaded.graph;
    let lang = ctx.lang;
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
    // Lens: which nodes and edges exist before filtering.
    let lens_kind = match filter.lens {
        GraphLens::Notes => None,
        GraphLens::People => Some(DKind::Person),
        GraphLens::Companies => Some(DKind::Company),
    };
    let in_lens = |ix: u32| lens_kind.is_none_or(|k| g.node(ix).kind == k);
    let mut all_nodes: Vec<GraphNode> = (0..u32::try_from(g.node_count()).unwrap_or(0))
        .filter(|&ix| in_lens(ix))
        .map(|ix| {
            let p = positions[ix as usize];
            node_view(
                &loaded,
                ctx,
                ix,
                0,
                clusters_of.get(&g.node(ix).key).cloned(),
                (p[0], p[1]),
            )
        })
        .collect();
    let mut all_edges: Vec<GraphEdge> = (0..u32::try_from(g.edge_count()).unwrap_or(0))
        .filter(|&i| {
            let e = &g.edges()[i as usize];
            in_lens(e.source)
                && in_lens(e.target)
                && (lens_kind.is_none() || matches!(e.kind, GraphEdgeKind::Entity(_)))
        })
        .map(|i| edge_view(&loaded, i, lang))
        .collect();
    if let Some(k) = lens_kind {
        for c in graph_algo::neighbourhood::co_mentions(g, &[k]) {
            let (a, b) = (g.node(c.a).key.clone(), g.node(c.b).key.clone());
            all_edges.push(GraphEdge {
                id: format!("{a}|co-mention|{b}"),
                label: match lang {
                    Lang::En => format!(
                        "co-mentioned in {}",
                        labels::NOTES.of(i64::from(c.notes), lang)
                    ),
                    Lang::Ar => {
                        format!("ذُكرا معًا في {}", labels::NOTES.of(i64::from(c.notes), lang))
                    }
                },
                src: a,
                dst: b,
                kind: "co-mention".to_owned(),
                by: None,
                confidence: Some(c.strength),
                rel_type: None,
                reason: None,
            });
        }
    }
    let similarity = crate::store::cache::get::<Vec<crate::net::SimilarityEdge>>(
        conn,
        crate::store::cache::SIMILARITY,
    )?
    .map(|(e, _)| e);
    if let Some(sim) = &similarity {
        let present: std::collections::HashSet<&str> =
            all_nodes.iter().map(|n| n.id.as_str()).collect();
        for e in sim {
            if present.contains(e.src.as_str()) && present.contains(e.dst.as_str()) {
                all_edges.push(GraphEdge {
                    id: format!("{}|similarity|{}", e.src, e.dst),
                    label: labels::relation_label("similarity", lang),
                    src: e.src.clone(),
                    dst: e.dst.clone(),
                    kind: "similarity".to_owned(),
                    by: Some("ai".to_owned()),
                    confidence: e.score,
                    rel_type: None,
                    reason: None,
                });
            }
        }
    }
    let node_counts = kind_counts(all_nodes.iter().map(|n| n.kind.as_str()), lang, true);
    let edge_counts = kind_counts(all_edges.iter().map(|e| edge_family(&e.kind)), lang, false);
    // Filters.
    all_nodes.retain(|n| {
        (filter.node_kinds.is_empty() || filter.node_kinds.iter().any(|k| k == n.kind.as_str()))
            && filter
                .cluster
                .as_ref()
                .is_none_or(|c| n.cluster_id.as_ref() == Some(c))
    });
    let kept: std::collections::HashSet<String> = all_nodes.iter().map(|n| n.id.clone()).collect();
    all_edges.retain(|e| {
        kept.contains(&e.src)
            && kept.contains(&e.dst)
            && edge_kind_selected(&filter.edge_kinds, &e.kind)
            && (filter.similarity || e.kind != "similarity")
    });
    let neighbours = match &filter.focus {
        Some(f) => {
            let mut v: Vec<String> = all_edges
                .iter()
                .filter_map(|e| {
                    if &e.src == f {
                        Some(e.dst.clone())
                    } else if &e.dst == f {
                        Some(e.src.clone())
                    } else {
                        None
                    }
                })
                .collect();
            v.sort();
            v.dedup();
            v
        }
        None => Vec::new(),
    };
    let mut clusters: Vec<ClusterLabel> = {
        let mut st = conn.prepare("SELECT cluster_id, name FROM cluster_names")?;
        let names: Vec<(String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        names
            .into_iter()
            .filter_map(|(id, name)| {
                let members: Vec<(f64, f64)> = all_nodes
                    .iter()
                    .filter(|n| n.cluster_id.as_ref() == Some(&id))
                    .map(|n| (n.x, n.y))
                    .collect();
                if members.is_empty() {
                    return None;
                }
                let (x, y, hull, radius) = region(&members);
                Some(ClusterLabel {
                    size: u32::try_from(members.len()).unwrap_or(u32::MAX),
                    id,
                    name,
                    x,
                    y,
                    hull,
                    radius,
                })
            })
            .collect()
    };
    clusters.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    Ok(GlobalGraphView {
        nodes: all_nodes,
        edges: all_edges,
        clusters,
        filter: filter.clone(),
        edge_counts,
        node_counts,
        neighbours,
        similarity: if similarity.is_some() {
            Availability::Available
        } else if ctx.connectivity == crate::view::model::Connectivity::Offline {
            Availability::Offline
        } else {
            // Fetched on demand (`refresh_similarity`).
            Availability::Available
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_and_ranks() {
        let hull = convex_hull(vec![
            (0.0, 0.0),
            (2.0, 0.0),
            (1.0, 1.0),
            (2.0, 2.0),
            (0.0, 2.0),
        ]);
        assert_eq!(hull, vec![(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
        assert_eq!(
            [0, 1, 2, 3, 5, 6, 40].map(label_rank),
            [3, 2, 2, 1, 1, 0, 0]
        );
        assert_eq!(edge_family("relation:works-at"), "relation");
        assert_eq!(edge_family("link"), "link");
        let (x, y, h, r) = region(&[(0.0, 0.0), (10.0, 0.0)]);
        assert_eq!((x, y), (5.0, 0.0));
        assert_eq!(r, 5.0 + REGION_PAD);
        assert_eq!(h.len(), 2);
        assert_eq!(
            h[0],
            GraphPoint {
                x: -REGION_PAD,
                y: 0.0
            }
        );
    }
}
