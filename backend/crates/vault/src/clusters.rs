//! The `clusters` / `cluster_names` rows derived from `.meta/clusters.json` (PLAN §6.5,
//! §7.4), shared by the graph component's writes of that file and by `stratad reindex`
//! (which reloads them from the file, so a rebuilt index has clusters immediately).
//!
//! [`snapshot`] reads the current rows; [`replace`] writes the rows of a file and appends a
//! `change_log` row (`cluster_assignment` per note, `cluster_name` per cluster, upsert or
//! delete) for exactly the differences from a snapshot, so `/sync/changes` serves them as
//! the `sync-model` cluster records and an unchanged state logs nothing.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::repo::sync::{self as sync_repo, NewChange};
use strata_index::types::ChangeOp;
use vault_format::clusters::Clusters;

use crate::error::Result;

/// The scope's cluster rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClusterRows {
    /// Cluster of each note.
    pub assignment: BTreeMap<NoteId, i64>,
    /// Name of each cluster.
    pub names: BTreeMap<i64, String>,
}

impl ClusterRows {
    /// The rows `file` describes (members as listed; notes missing from the index are
    /// dropped by [`replace`]).
    pub fn of_file(file: &Clusters) -> Self {
        Self {
            assignment: file
                .clusters
                .iter()
                .flat_map(|c| {
                    c.notes
                        .iter()
                        .map(move |n| (NoteId::from_ulid(*n), i64::from(c.id)))
                })
                .collect(),
            names: file
                .clusters
                .iter()
                .map(|c| (i64::from(c.id), c.name.clone()))
                .collect(),
        }
    }
}

/// The scope's current rows.
pub async fn snapshot(tx: &mut ScopedTx) -> Result<ClusterRows> {
    let assignment = sqlx::query_as::<_, (NoteId, i64)>("SELECT note_id, cluster_id FROM clusters")
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .collect();
    let names = sqlx::query_as::<_, (i64, String)>("SELECT cluster_id, name FROM cluster_names")
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .collect();
    Ok(ClusterRows { assignment, names })
}

/// Replaces the scope's rows with `new` and logs the differences from `old` (the rows
/// before this transaction changed them) at `now`. Notes that are not in `notes` are
/// skipped (the foreign key needs them). Returns the number of change-log rows.
pub async fn replace(
    tx: &mut ScopedTx,
    old: &ClusterRows,
    new: &ClusterRows,
    now: DateTime<Utc>,
) -> Result<usize> {
    sqlx::query("DELETE FROM clusters")
        .execute(tx.conn())
        .await?;
    let (notes, ids): (Vec<NoteId>, Vec<i64>) =
        new.assignment.iter().map(|(n, c)| (*n, *c)).unzip();
    sqlx::query(
        "INSERT INTO clusters (user_id, note_id, cluster_id) \
         SELECT strata_current_user(), u.note_id, u.cluster_id \
         FROM unnest($1::uuid[], $2::bigint[]) AS u(note_id, cluster_id) \
         JOIN notes n ON n.user_id = strata_current_user() AND n.id = u.note_id",
    )
    .bind(&notes)
    .bind(&ids)
    .execute(tx.conn())
    .await?;
    sqlx::query("DELETE FROM cluster_names")
        .execute(tx.conn())
        .await?;
    let (cids, cnames): (Vec<i64>, Vec<String>) =
        new.names.iter().map(|(i, n)| (*i, n.clone())).unzip();
    sqlx::query(
        "INSERT INTO cluster_names (user_id, cluster_id, name) \
         SELECT strata_current_user(), u.cluster_id, u.name \
         FROM unnest($1::bigint[], $2::text[]) AS u(cluster_id, name)",
    )
    .bind(&cids)
    .bind(&cnames)
    .execute(tx.conn())
    .await?;

    let stored = snapshot(tx).await?;
    let mut changes: Vec<(&'static str, String, ChangeOp)> = Vec::new();
    for (note, c) in &stored.assignment {
        if old.assignment.get(note) != Some(c) {
            changes.push(("cluster_assignment", note.to_string(), ChangeOp::Upsert));
        }
    }
    for note in old
        .assignment
        .keys()
        .filter(|n| !stored.assignment.contains_key(n))
    {
        changes.push(("cluster_assignment", note.to_string(), ChangeOp::Delete));
    }
    for (id, name) in &stored.names {
        if old.names.get(id) != Some(name) {
            changes.push(("cluster_name", id.to_string(), ChangeOp::Upsert));
        }
    }
    for id in old.names.keys().filter(|i| !stored.names.contains_key(i)) {
        changes.push(("cluster_name", id.to_string(), ChangeOp::Delete));
    }
    let n = changes.len();
    for (entity_type, entity_id, op) in changes {
        sync_repo::append_change(
            tx,
            &NewChange {
                entity_type,
                entity_id: &entity_id,
                op,
                version: None,
                at: now,
            },
        )
        .await?;
    }
    Ok(n)
}
