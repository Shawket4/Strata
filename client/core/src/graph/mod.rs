//! Local graph (PLAN §12.1 `graph/`): a note's neighbourhood from the cached links and
//! relations, for the local mind map (D4) and the note's mini-graph.
//!
//! Depth-1 neighbourhoods are a plain query over the local tables. Deeper neighbourhoods,
//! clustering and layouts are shared algorithms that belong to `graph-algo` (L16), which is
//! still a placeholder: `layout` is therefore reported as not yet available.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};

use crate::error::CoreResult;
use crate::view::model::{Availability, GraphEdge, GraphNode, LocalGraphView};

/// Depth-1 neighbourhood of a note from cached links and relations.
pub fn local_graph(conn: &Connection, id: &str) -> CoreResult<LocalGraphView> {
    type EdgeRow = (String, String, String, Option<String>, Option<f64>);
    let edges: Vec<EdgeRow> = {
        let mut st = conn.prepare(
            "SELECT DISTINCT l.note_id, l.dst_id, l.kind, NULL, NULL FROM links l
               JOIN notes a ON a.id = l.note_id JOIN notes b ON b.id = l.dst_id
               WHERE (l.note_id = ?1 OR l.dst_id = ?1) AND l.dst_id IS NOT NULL
                 AND a.deleted = 0 AND b.deleted = 0
             UNION
             SELECT r.src_id, r.dst_id, 'relation:' || r.rel_type, COALESCE(m.by, 'user'), m.confidence
               FROM relations r
               JOIN notes a ON a.id = r.src_id JOIN notes b ON b.id = r.dst_id
               LEFT JOIN relation_meta m ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
               WHERE (r.src_id = ?1 OR r.dst_id = ?1) AND r.dst_id IS NOT NULL
                 AND a.deleted = 0 AND b.deleted = 0
             ORDER BY 1, 2, 3",
        )?;
        st.query_map([id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut node_ids: Vec<String> = edges
        .iter()
        .flat_map(|e| [e.0.clone(), e.1.clone()])
        .filter(|n| n != id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let node = |nid: &str| -> CoreResult<Option<GraphNode>> {
        Ok(conn
            .query_row(
                "SELECT id, title, kind FROM notes WHERE id = ?1 AND deleted = 0",
                [nid],
                |r| {
                    Ok(GraphNode {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        kind: r.get(2)?,
                    })
                },
            )
            .optional()?)
    };
    let mut nodes = Vec::new();
    nodes.extend(node(id)?);
    let mut others = Vec::new();
    for n in node_ids.drain(..) {
        others.extend(node(&n)?);
    }
    others.sort_by(|a, b| (&a.title, &a.id).cmp(&(&b.title, &b.id)));
    nodes.extend(others);
    Ok(LocalGraphView {
        center: id.to_owned(),
        nodes,
        edges: edges
            .into_iter()
            .map(|(src, dst, kind, by, confidence)| GraphEdge {
                src,
                dst,
                kind,
                by,
                confidence,
            })
            .collect(),
        layout: Availability::NotYetAvailable {
            feature: "graph_layout".to_owned(),
        },
    })
}

