//! `conflicts`, `duplicates` and `rejections`: push results that need the user.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{CoreError, CoreResult};
use crate::store::{from_msgpack, to_msgpack};
use crate::sync::model::{ConflictResolution, Op, OpKind};

/// A `conflict` push result (D19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRow {
    /// The conflicting op.
    pub op_id: String,
    /// The note.
    pub entity_id: String,
    /// The op.
    pub local_op: Op,
    /// The base the local edit started from.
    pub base_content: Option<String>,
    /// The local content.
    pub local_content: Option<String>,
    /// The server's version (when sent).
    pub server_version: Option<String>,
    /// The server's content (when known).
    pub server_content: Option<String>,
    /// The local 3-way merge preview.
    pub merged_preview: Option<String>,
    /// Whether the preview merged without overlapping hunks.
    pub merge_clean: Option<bool>,
    /// The full `sync-model` merge outcome (hunks for per-hunk resolution).
    pub merge_outcome: Option<sync_model::MergeOutcome>,
    /// How the server handled the edit (conflict copy / server kept).
    pub resolution: Option<ConflictResolution>,
    /// Created at.
    pub created: String,
}

/// Inserts (or replaces) a conflict.
pub fn put_conflict(conn: &Connection, c: &ConflictRow) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO conflicts (op_id, entity_id, local_payload, base_content,
             local_content, server_version, server_content, merged_preview, merge_clean,
             merge_outcome, resolution, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            c.op_id,
            c.entity_id,
            to_msgpack(&(c.local_op.kind().as_str(), c.local_op.payload_bytes()?))?,
            c.base_content,
            c.local_content,
            c.server_version,
            c.server_content,
            c.merged_preview,
            c.merge_clean,
            c.merge_outcome.as_ref().map(to_msgpack).transpose()?,
            c.resolution.as_ref().map(to_msgpack).transpose()?,
            c.created
        ],
    )?;
    Ok(())
}

struct RawConflict {
    op_id: String,
    entity_id: String,
    local_payload: Vec<u8>,
    base_content: Option<String>,
    local_content: Option<String>,
    server_version: Option<String>,
    server_content: Option<String>,
    merged_preview: Option<String>,
    merge_clean: Option<bool>,
    merge_outcome: Option<Vec<u8>>,
    resolution: Option<Vec<u8>>,
    created: String,
}

fn conflict_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawConflict> {
    Ok(RawConflict {
        op_id: r.get(0)?,
        entity_id: r.get(1)?,
        local_payload: r.get(2)?,
        base_content: r.get(3)?,
        local_content: r.get(4)?,
        server_version: r.get(5)?,
        server_content: r.get(6)?,
        merged_preview: r.get(7)?,
        merge_clean: r.get(8)?,
        merge_outcome: r.get(9)?,
        resolution: r.get(10)?,
        created: r.get(11)?,
    })
}

fn finish(raw: RawConflict) -> CoreResult<ConflictRow> {
    let (kind, payload): (String, Vec<u8>) = from_msgpack(&raw.local_payload)?;
    let kind: OpKind = kind
        .parse()
        .map_err(|e: sync_model::OpError| CoreError::Storage(e.to_string()))?;
    let local_op =
        Op::from_parts(kind, &payload).map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(ConflictRow {
        op_id: raw.op_id,
        entity_id: raw.entity_id,
        local_op,
        base_content: raw.base_content,
        local_content: raw.local_content,
        server_version: raw.server_version,
        server_content: raw.server_content,
        merged_preview: raw.merged_preview,
        merge_clean: raw.merge_clean,
        merge_outcome: raw.merge_outcome.as_deref().map(from_msgpack).transpose()?,
        resolution: raw.resolution.as_deref().map(from_msgpack).transpose()?,
        created: raw.created,
    })
}

const CONFLICT_COLUMNS: &str = "op_id, entity_id, local_payload, base_content, local_content, \
                                server_version, server_content, merged_preview, merge_clean, \
                                merge_outcome, resolution, created";

/// Every conflict, oldest first.
pub fn conflicts(conn: &Connection) -> CoreResult<Vec<ConflictRow>> {
    let mut st = conn.prepare(&format!(
        "SELECT {CONFLICT_COLUMNS} FROM conflicts ORDER BY created, op_id"
    ))?;
    let raw = st.query_map([], conflict_row)?.collect::<Result<Vec<_>, _>>()?;
    raw.into_iter().map(finish).collect()
}

/// One conflict.
pub fn conflict(conn: &Connection, op_id: &str) -> CoreResult<Option<ConflictRow>> {
    let raw = conn
        .query_row(
            &format!("SELECT {CONFLICT_COLUMNS} FROM conflicts WHERE op_id = ?1"),
            [op_id],
            conflict_row,
        )
        .optional()?;
    raw.map(finish).transpose()
}

/// Conflicts of one note.
pub fn conflicts_of(conn: &Connection, note_id: &str) -> CoreResult<Vec<ConflictRow>> {
    Ok(conflicts(conn)?
        .into_iter()
        .filter(|c| c.entity_id == note_id)
        .collect())
}

/// Deletes a conflict.
pub fn delete_conflict(conn: &Connection, op_id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM conflicts WHERE op_id = ?1", [op_id])?;
    Ok(())
}

/// Records duplicate candidates for an op.
pub fn put_duplicate(
    conn: &Connection,
    op_id: &str,
    candidates: &[dedupe::DuplicateCandidate],
    created: &str,
) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO duplicates (op_id, candidates, created) VALUES (?1, ?2, ?3)",
        params![op_id, to_msgpack(&candidates)?, created],
    )?;
    Ok(())
}

/// Every duplicate prompt: `(op_id, candidates)`, oldest first.
pub fn duplicates(conn: &Connection) -> CoreResult<Vec<(String, Vec<dedupe::DuplicateCandidate>)>> {
    let mut st = conn.prepare("SELECT op_id, candidates FROM duplicates ORDER BY created, op_id")?;
    let raw: Vec<(String, Vec<u8>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    raw.into_iter()
        .map(|(id, c)| Ok((id, from_msgpack(&c)?)))
        .collect()
}

/// Deletes a duplicate prompt.
pub fn delete_duplicate(conn: &Connection, op_id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM duplicates WHERE op_id = ?1", [op_id])?;
    Ok(())
}

/// A rejected op, kept until dismissed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionRow {
    /// The op.
    pub op_id: String,
    /// Op kind (PLAN string).
    pub kind: String,
    /// Server entity.
    pub entity_id: String,
    /// Problem type slug.
    pub problem_type: String,
    /// HTTP status.
    pub status: u16,
    /// Created at.
    pub created: String,
}

/// Records a rejection.
pub fn put_rejection(conn: &Connection, r: &RejectionRow) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO rejections (op_id, kind, entity_id, problem_type, status, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![r.op_id, r.kind, r.entity_id, r.problem_type, r.status, r.created],
    )?;
    Ok(())
}

/// Every rejection, oldest first.
pub fn rejections(conn: &Connection) -> CoreResult<Vec<RejectionRow>> {
    let mut st = conn.prepare(
        "SELECT op_id, kind, entity_id, problem_type, status, created FROM rejections
         ORDER BY created, op_id",
    )?;
    Ok(st
        .query_map([], |r| {
            Ok(RejectionRow {
                op_id: r.get(0)?,
                kind: r.get(1)?,
                entity_id: r.get(2)?,
                problem_type: r.get(3)?,
                status: r.get(4)?,
                created: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

/// Dismisses a rejection.
pub fn delete_rejection(conn: &Connection, op_id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM rejections WHERE op_id = ?1", [op_id])?;
    Ok(())
}
