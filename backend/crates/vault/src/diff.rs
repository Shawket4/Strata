//! Before/after snapshots of the derived rows a write can change outside the note text
//! (relations with provenance, rejections, keep-both pairs) plus task and custody rows, so a
//! write can append change-log rows for them (`GET /sync/changes`, PLAN §7.4) and describe
//! them in its commit notice (`GET /events`).

use std::collections::{BTreeMap, BTreeSet};

use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::types::ChangeOp;
use uuid::Uuid;

use crate::error::Result;
use crate::events::RelationEvent;

/// Relation value that matters to clients: `by`, confidence, reason.
type RelationValue = (String, Option<f32>, Option<String>);

/// Derived rows around a set of notes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// `(src, type, dst)` → provenance, for relations from or to the notes.
    relations: BTreeMap<(Uuid, String, Uuid), RelationValue>,
    /// `(src, type, dst)` of rejections from the notes.
    rejected: BTreeSet<(Uuid, String, Uuid)>,
    /// Every keep-both pair (only when sidecars changed).
    keep_both: Option<BTreeSet<(String, String, String)>>,
    /// Task ID → (note, row hash) for tasks in the notes.
    tasks: BTreeMap<String, (Uuid, String)>,
    /// Document → custody event IDs, for the notes.
    custody: BTreeMap<Uuid, BTreeSet<Uuid>>,
}

/// What changed between two snapshots.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diff {
    /// Change-log rows: (entity type, entity ID, op).
    pub log: Vec<(&'static str, String, ChangeOp)>,
    /// Relations added/removed, sorted.
    pub relations: Vec<RelationEvent>,
    /// Tasks changed or removed: (task, note now).
    pub tasks: Vec<(String, Option<NoteId>)>,
    /// Documents whose custody events changed.
    pub custody: Vec<NoteId>,
}

impl Snapshot {
    /// Reads the rows around `ids` (keep-both pairs too when `keep_both`).
    pub async fn take(
        tx: &mut ScopedTx,
        ids: &BTreeSet<NoteId>,
        keep_both: bool,
    ) -> Result<Self> {
        let ids: Vec<Uuid> = ids.iter().map(NoteId::as_uuid).collect();
        let mut out = Self::default();
        if !ids.is_empty() {
            let rows: Vec<(Uuid, String, Uuid, String, Option<f32>, Option<String>)> =
                sqlx::query_as(
                    "SELECT src_id, type, dst_id, by, confidence, reason FROM relations \
                     WHERE src_id = ANY($1) OR dst_id = ANY($1)",
                )
                .bind(&ids)
                .fetch_all(tx.conn())
                .await?;
            for (s, t, d, by, c, r) in rows {
                out.relations.insert((s, t, d), (by, c, r));
            }
            let rows: Vec<(Uuid, String, Uuid)> =
                sqlx::query_as("SELECT src_id, type, dst_id FROM rejected WHERE src_id = ANY($1)")
                    .bind(&ids)
                    .fetch_all(tx.conn())
                    .await?;
            out.rejected = rows.into_iter().collect();
            let rows: Vec<(String, Uuid, String)> = sqlx::query_as(
                "SELECT t.id, t.note_id, md5(t::text) FROM tasks t WHERE t.note_id = ANY($1)",
            )
            .bind(&ids)
            .fetch_all(tx.conn())
            .await?;
            out.tasks = rows.into_iter().map(|(id, n, h)| (id, (n, h))).collect();
            let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT document_id, id FROM custody_events WHERE document_id = ANY($1)",
            )
            .bind(&ids)
            .fetch_all(tx.conn())
            .await?;
            for (doc, id) in rows {
                out.custody.entry(doc).or_default().insert(id);
            }
        }
        if keep_both {
            let rows: Vec<(String, String, String)> =
                sqlx::query_as("SELECT kind, a_id, b_id FROM dedupe_keep_both")
                    .fetch_all(tx.conn())
                    .await?;
            out.keep_both = Some(rows.into_iter().collect());
        }
        Ok(out)
    }

    /// The note that held `task` in this snapshot.
    pub fn task_note(&self, task: &str) -> Option<NoteId> {
        self.tasks.get(task).map(|(n, _)| NoteId::from_uuid(*n))
    }
}

fn key(s: Uuid, t: &str, d: Uuid) -> String {
    format!(
        "{}:{t}:{}",
        NoteId::from_uuid(s).as_ulid(),
        NoteId::from_uuid(d).as_ulid()
    )
}

/// The differences from `before` to `after`.
pub fn diff(before: &Snapshot, after: &Snapshot) -> Diff {
    let mut out = Diff::default();
    // Relations: upsert when new or provenance changed, delete when gone.
    let mut relation_log = Vec::new();
    for ((s, t, d), v) in &after.relations {
        match before.relations.get(&(*s, t.clone(), *d)) {
            Some(old) if old == v => {}
            Some(_) => relation_log.push((key(*s, t, *d), ChangeOp::Upsert)),
            None => {
                relation_log.push((key(*s, t, *d), ChangeOp::Upsert));
                out.relations.push(RelationEvent {
                    src: NoteId::from_uuid(*s),
                    rel: t.clone(),
                    dst: NoteId::from_uuid(*d),
                    added: true,
                });
            }
        }
    }
    for (s, t, d) in before.relations.keys() {
        if !after.relations.contains_key(&(*s, t.clone(), *d)) {
            relation_log.push((key(*s, t, *d), ChangeOp::Delete));
            out.relations.push(RelationEvent {
                src: NoteId::from_uuid(*s),
                rel: t.clone(),
                dst: NoteId::from_uuid(*d),
                added: false,
            });
        }
    }
    relation_log.sort();
    out.log
        .extend(relation_log.into_iter().map(|(k, op)| ("relation", k, op)));
    out.relations.sort();

    let mut rejected_log = Vec::new();
    for (s, t, d) in after.rejected.difference(&before.rejected) {
        rejected_log.push((key(*s, t, *d), ChangeOp::Upsert));
    }
    for (s, t, d) in before.rejected.difference(&after.rejected) {
        rejected_log.push((key(*s, t, *d), ChangeOp::Delete));
    }
    rejected_log.sort();
    out.log
        .extend(rejected_log.into_iter().map(|(k, op)| ("rejected", k, op)));

    if let (Some(b), Some(a)) = (&before.keep_both, &after.keep_both) {
        let mut kb = Vec::new();
        for (k, x, y) in a.difference(b) {
            kb.push((format!("{k}:{x}:{y}"), ChangeOp::Upsert));
        }
        for (k, x, y) in b.difference(a) {
            kb.push((format!("{k}:{x}:{y}"), ChangeOp::Delete));
        }
        kb.sort();
        out.log.extend(kb.into_iter().map(|(k, op)| ("keep_both", k, op)));
    }

    for (id, (note, hash)) in &after.tasks {
        if before.tasks.get(id).is_none_or(|(n, h)| n != note || h != hash) {
            out.tasks.push((id.clone(), Some(NoteId::from_uuid(*note))));
        }
    }
    for id in before.tasks.keys() {
        if !after.tasks.contains_key(id) {
            out.tasks.push((id.clone(), None));
        }
    }
    out.tasks.sort();

    let docs: BTreeSet<&Uuid> = before.custody.keys().chain(after.custody.keys()).collect();
    for doc in docs {
        if before.custody.get(doc) != after.custody.get(doc) {
            out.custody.push(NoteId::from_uuid(*doc));
        }
    }
    out
}
