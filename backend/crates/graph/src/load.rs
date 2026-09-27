//! Reads one user's graph from the index (PLAN §10): live notes as nodes, and every typed
//! edge between live notes. Runs in the caller's [`ScopedTx`], so row-level security limits
//! every row to the scope's user (principle 7); nothing here takes a user ID.
//!
//! Edge sources:
//! - `links` (resolved body wikilinks and embeds) → `link` / `embed`, one edge per
//!   (source, target, kind);
//! - `relations` (frontmatter keys with sidecar provenance): note relation types →
//!   `relation:<type>` (`part-of` between two places → `part-of-place`), `concepts` →
//!   `concept`, `people`/`companies` → `mention`, entity relation types → `entity:<type>`;
//!   `copy-of` has no graph edge kind in §10 and is not returned;
//! - `documents` (the state computed from the newest custody event) →
//!   `custody:location|holder|last-holder`, with that event's provenance.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use domain::{
    CustodyEdge, EntityRelationType, GraphEdgeKind, MentionType, NoteKind, RelationOrigin,
    RelationType,
};
use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::types::{By, LinkKind};

use crate::error::Result;

/// A live note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    /// ID.
    pub id: NoteId,
    /// Display title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Vault path.
    pub path: String,
    /// Dominant language (`ar`, `en`, `mixed`), if detected.
    pub lang: Option<String>,
    /// Last update.
    pub updated: DateTime<Utc>,
}

/// A typed, directed edge between live notes.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRow {
    /// Source.
    pub source: NoteId,
    /// Target.
    pub target: NoteId,
    /// Kind.
    pub kind: GraphEdgeKind,
    /// Provenance.
    pub by: RelationOrigin,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// AI reason (relations).
    pub reason: Option<String>,
}

/// One user's graph as stored in the index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphData {
    /// Live notes, by ID.
    pub nodes: Vec<NodeRow>,
    /// Edges, sorted by (source, target, kind), unique per (source, target, kind).
    pub edges: Vec<EdgeRow>,
    /// Cluster of each clustered note.
    pub clusters: BTreeMap<NoteId, i64>,
    /// Cluster names.
    pub cluster_names: BTreeMap<i64, String>,
}

impl GraphData {
    /// Index of every node by ID (positions in [`GraphData::nodes`]).
    pub fn positions(&self) -> BTreeMap<NoteId, usize> {
        self.nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect()
    }
}

fn origin(by: By) -> RelationOrigin {
    match by {
        By::User => RelationOrigin::User,
        By::Ai => RelationOrigin::Ai,
    }
}

/// The graph edge kind of a stored relation type between notes of the given kinds; `None`
/// for types without a graph edge (`copy-of`) and unknown types.
pub fn relation_edge_kind(rel_type: &str, src: NoteKind, dst: NoteKind) -> Option<GraphEdgeKind> {
    if let Ok(t) = rel_type.parse::<RelationType>() {
        if t == RelationType::PartOf && src == NoteKind::Place && dst == NoteKind::Place {
            return Some(GraphEdgeKind::PartOfPlace);
        }
        return Some(GraphEdgeKind::Relation(t));
    }
    if let Ok(m) = rel_type.parse::<MentionType>() {
        return Some(match m {
            MentionType::Concepts => GraphEdgeKind::Concept,
            MentionType::People | MentionType::Companies => GraphEdgeKind::Mention,
        });
    }
    rel_type
        .parse::<EntityRelationType>()
        .ok()
        .map(GraphEdgeKind::Entity)
}

type NoteTuple = (NoteId, String, String, String, Option<String>, DateTime<Utc>);
type RelationTuple = (
    NoteId,
    NoteId,
    String,
    By,
    Option<f32>,
    Option<String>,
    String,
    String,
);
type CustodyTuple = (
    NoteId,
    Option<NoteId>,
    Option<NoteId>,
    Option<NoteId>,
    Option<By>,
    Option<f32>,
);

/// Loads the scope's graph.
#[allow(clippy::too_many_lines)] // one query per edge source, in order
pub async fn load(tx: &mut ScopedTx) -> Result<GraphData> {
    let rows: Vec<NoteTuple> = sqlx::query_as(
        "SELECT id, title, kind, path, lang, updated FROM notes WHERE NOT trashed ORDER BY id",
    )
    .fetch_all(tx.conn())
    .await?;
    let nodes: Vec<NodeRow> = rows
        .into_iter()
        .map(|(id, title, kind, path, lang, updated)| NodeRow {
            id,
            title,
            kind: kind.parse().unwrap_or(NoteKind::Note),
            path,
            lang,
            updated,
        })
        .collect();
    let live: BTreeSet<NoteId> = nodes.iter().map(|n| n.id).collect();

    let mut edges: BTreeMap<(NoteId, NoteId, GraphEdgeKind), EdgeRow> = BTreeMap::new();
    let mut put = |e: EdgeRow| {
        edges.entry((e.source, e.target, e.kind)).or_insert(e);
    };

    let links: Vec<(NoteId, NoteId, LinkKind)> = sqlx::query_as(
        "SELECT DISTINCT l.src_id, l.dst_id, l.kind FROM links l \
         JOIN notes s ON s.user_id = l.user_id AND s.id = l.src_id AND NOT s.trashed \
         JOIN notes d ON d.user_id = l.user_id AND d.id = l.dst_id AND NOT d.trashed \
         WHERE l.dst_id IS NOT NULL AND l.src_id <> l.dst_id",
    )
    .fetch_all(tx.conn())
    .await?;
    for (source, target, kind) in links {
        put(EdgeRow {
            source,
            target,
            kind: match kind {
                LinkKind::Link => GraphEdgeKind::Link,
                LinkKind::Embed => GraphEdgeKind::Embed,
            },
            by: RelationOrigin::User,
            confidence: None,
            reason: None,
        });
    }

    let relations: Vec<RelationTuple> = sqlx::query_as(
        "SELECT r.src_id, r.dst_id, r.type, r.by, r.confidence, r.reason, s.kind, d.kind \
         FROM relations r \
         JOIN notes s ON s.user_id = r.user_id AND s.id = r.src_id AND NOT s.trashed \
         JOIN notes d ON d.user_id = r.user_id AND d.id = r.dst_id AND NOT d.trashed",
    )
    .fetch_all(tx.conn())
    .await?;
    for (source, target, rel_type, by, confidence, reason, sk, dk) in relations {
        let (sk, dk) = (
            sk.parse().unwrap_or(NoteKind::Note),
            dk.parse().unwrap_or(NoteKind::Note),
        );
        let Some(kind) = relation_edge_kind(&rel_type, sk, dk) else {
            continue;
        };
        put(EdgeRow {
            source,
            target,
            kind,
            by: origin(by),
            confidence,
            reason,
        });
    }

    let custody: Vec<CustodyTuple> = sqlx::query_as(
        "SELECT d.note_id, d.location_id, d.holder_id, d.last_holder_id, e.by, e.confidence \
         FROM documents d \
         JOIN notes n ON n.user_id = d.user_id AND n.id = d.note_id AND NOT n.trashed \
         LEFT JOIN LATERAL ( \
           SELECT c.by, c.confidence FROM custody_events c \
           WHERE c.user_id = d.user_id AND c.document_id = d.note_id \
           ORDER BY c.at DESC, c.id DESC LIMIT 1) e ON true",
    )
    .fetch_all(tx.conn())
    .await?;
    for (doc, location, holder, last_holder, by, confidence) in custody {
        for (target, which) in [
            (location, CustodyEdge::Location),
            (holder, CustodyEdge::Holder),
            (last_holder, CustodyEdge::LastHolder),
        ] {
            let Some(target) = target.filter(|t| live.contains(t) && *t != doc) else {
                continue;
            };
            put(EdgeRow {
                source: doc,
                target,
                kind: GraphEdgeKind::Custody(which),
                by: by.map_or(RelationOrigin::User, origin),
                confidence,
                reason: None,
            });
        }
    }

    let clusters: Vec<(NoteId, i64)> = sqlx::query_as(
        "SELECT c.note_id, c.cluster_id FROM clusters c \
         JOIN notes n ON n.user_id = c.user_id AND n.id = c.note_id AND NOT n.trashed",
    )
    .fetch_all(tx.conn())
    .await?;
    let cluster_names: Vec<(i64, String)> =
        sqlx::query_as("SELECT cluster_id, name FROM cluster_names")
            .fetch_all(tx.conn())
            .await?;

    Ok(GraphData {
        nodes,
        edges: edges.into_values().collect(),
        clusters: clusters.into_iter().collect(),
        cluster_names: cluster_names.into_iter().collect(),
    })
}

/// A string setting of the scope's free-form preferences (`PATCH /me` `preferences`).
pub async fn preference(tx: &mut ScopedTx, key: &str) -> Result<Option<String>> {
    let raw = strata_index::repo::settings::get_setting(tx, "preferences").await?;
    let prefs: BTreeMap<String, String> = raw
        .and_then(|b| rmp_serde::from_slice(&b).ok())
        .unwrap_or_default();
    Ok(prefs.get(key).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_types_map_to_the_edge_kinds_of_section_10() {
        use NoteKind::{Company, Concept, Document, Note, Person, Place};
        let cases = [
            (
                "related",
                Note,
                Note,
                Some(GraphEdgeKind::Relation(RelationType::Related)),
            ),
            (
                "part-of",
                Note,
                Note,
                Some(GraphEdgeKind::Relation(RelationType::PartOf)),
            ),
            ("part-of", Place, Place, Some(GraphEdgeKind::PartOfPlace)),
            (
                "part-of",
                Place,
                Note,
                Some(GraphEdgeKind::Relation(RelationType::PartOf)),
            ),
            ("concepts", Note, Concept, Some(GraphEdgeKind::Concept)),
            ("people", Note, Person, Some(GraphEdgeKind::Mention)),
            ("companies", Document, Company, Some(GraphEdgeKind::Mention)),
            (
                "works-at",
                Person,
                Company,
                Some(GraphEdgeKind::Entity(EntityRelationType::WorksAt)),
            ),
            ("copy-of", Document, Document, None),
            ("nonsense", Note, Note, None),
        ];
        for (t, s, d, want) in cases {
            assert_eq!(relation_edge_kind(t, s, d), want, "{t}");
        }
    }
}
