//! `conflicts`, `duplicates` and `rejections`: push results that need the user.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;
use crate::store::{from_msgpack, to_msgpack};
use crate::sync::model::{Candidate, OpPayload};

/// A `conflict` push result (D19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRow {
    /// The conflicting op.
    pub op_id: String,
    /// The note.
    pub entity_id: String,
    /// The op's payload.
    pub local_payload: OpPayload,
    /// The base the local edit started from.
    pub base_content: Option<String>,
    /// The local content.
    pub local_content: Option<String>,
    /// The server's version.
    pub server_version: String,
    /// The server's content (when known).
    pub server_content: Option<String>,
    /// The local 3-way merge preview.
    pub merged_preview: Option<String>,
    /// Whether the preview merged without overlapping hunks.
    pub merge_clean: Option<bool>,
    /// Created at.
    pub created: String,
}

/// Inserts (or replaces) a conflict.
pub fn put_conflict(conn: &Connection, c: &ConflictRow) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO conflicts (op_id, entity_id, local_payload, base_content,
             local_content, server_version, server_content, merged_preview, merge_clean, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            c.op_id,
            c.entity_id,
            to_msgpack(&c.local_payload)?,
            c.base_content,
            c.local_content,
            c.server_version,
            c.server_content,
            c.merged_preview,
            c.merge_clean,
            c.created
        ],
    )?;
    Ok(())
}

fn conflict_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(ConflictRow, Vec<u8>)> {
    Ok((
        ConflictRow {
            op_id: r.get(0)?,
            entity_id: r.get(1)?,
            local_payload: OpPayload::NoteDelete,
            base_content: r.get(3)?,
            local_content: r.get(4)?,
            server_version: r.get(5)?,
            server_content: r.get(6)?,
            merged_preview: r.get(7)?,
            merge_clean: r.get(8)?,
            created: r.get(9)?,
        },
        r.get(2)?,
    ))
}

const CONFLICT_COLUMNS: &str = "op_id, entity_id, local_payload, base_content, local_content, \
                                server_version, server_content, merged_preview, merge_clean, created";

/// Every conflict, oldest first.
pub fn conflicts(conn: &Connection) -> CoreResult<Vec<ConflictRow>> {
    let mut st = conn.prepare(&format!(
        "SELECT {CONFLICT_COLUMNS} FROM conflicts ORDER BY created, op_id"
    ))?;
    let raw = st.query_map([], conflict_row)?.collect::<Result<Vec<_>, _>>()?;
    raw.into_iter()
        .map(|(mut c, p)| {
            c.local_payload = from_msgpack(&p)?;
            Ok(c)
        })
        .collect()
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
    raw.map(|(mut c, p)| {
        c.local_payload = from_msgpack(&p)?;
        Ok(c)
    })
    .transpose()
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
    candidates: &[Candidate],
    created: &str,
) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO duplicates (op_id, candidates, created) VALUES (?1, ?2, ?3)",
        params![op_id, to_msgpack(&candidates)?, created],
    )?;
    Ok(())
}

/// Every duplicate prompt: `(op_id, candidates)`, oldest first.
pub fn duplicates(conn: &Connection) -> CoreResult<Vec<(String, Vec<Candidate>)>> {
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
