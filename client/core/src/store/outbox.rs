//! The outbox (§12.2): pending mutations, pushed in `ord` order, surviving restarts.
//!
//! Status machine: `pending` → `inflight` (pushed, awaiting results) → `done` | `conflict` |
//! `duplicate` | `rejected`. An `inflight` op found at startup was interrupted mid-push and goes
//! back to `pending`; re-pushing it with the same `op_id` is safe (server idempotency).
//! `conflict` and `duplicate` ops are **live** (their local effect stays visible) until the user
//! resolves them.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{CoreError, CoreResult};
use crate::store::{from_msgpack, to_msgpack};
use crate::sync::model::{OpKind, OpPayload, PushOp};

/// Outbox op status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpStatus {
    /// Waiting to be pushed.
    Pending,
    /// Pushed; results not recorded yet.
    Inflight,
    /// Applied by the server.
    Done,
    /// Overlapping edit (D19), awaiting the user.
    Conflict,
    /// Looks like an existing item, awaiting the user.
    Duplicate,
    /// Refused by the server; rolled back.
    Rejected,
}

impl OpStatus {
    /// Column spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Inflight => "inflight",
            Self::Done => "done",
            Self::Conflict => "conflict",
            Self::Duplicate => "duplicate",
            Self::Rejected => "rejected",
        }
    }

    fn parse(s: &str) -> CoreResult<Self> {
        Ok(match s {
            "pending" => Self::Pending,
            "inflight" => Self::Inflight,
            "done" => Self::Done,
            "conflict" => Self::Conflict,
            "duplicate" => Self::Duplicate,
            "rejected" => Self::Rejected,
            other => return Err(CoreError::Storage(format!("bad outbox status {other}"))),
        })
    }

    /// Whether the op's local effect is part of the current state.
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Inflight | Self::Conflict | Self::Duplicate
        )
    }
}

/// One outbox row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxOp {
    /// Idempotency key.
    pub op_id: String,
    /// Push order.
    pub ord: i64,
    /// Server entity.
    pub entity_id: String,
    /// Local entity the op changes (`note:<id>` / `suggestion:<id>`).
    pub local_entity: String,
    /// Base version.
    pub base_version: Option<String>,
    /// Payload.
    pub payload: OpPayload,
    /// Status.
    pub status: OpStatus,
    /// Push attempts.
    pub attempts: u32,
    /// Created at (RFC 3339 UTC).
    pub created: String,
}

impl OutboxOp {
    /// The kind.
    pub fn kind(&self) -> OpKind {
        self.payload.kind()
    }

    /// The op as pushed.
    pub fn to_push(&self) -> PushOp {
        PushOp {
            op_id: self.op_id.clone(),
            kind: self.kind(),
            entity_id: self.entity_id.clone(),
            base_version: self.base_version.clone(),
            payload: self.payload.clone(),
        }
    }
}

const COLUMNS: &str =
    "op_id, ord, entity_id, local_entity, base_version, payload, status, attempts, created";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(OutboxOp, Vec<u8>, String)> {
    Ok((
        OutboxOp {
            op_id: r.get(0)?,
            ord: r.get(1)?,
            entity_id: r.get(2)?,
            local_entity: r.get(3)?,
            base_version: r.get(4)?,
            payload: OpPayload::NoteDelete,
            status: OpStatus::Pending,
            attempts: r.get(7)?,
            created: r.get(8)?,
        },
        r.get(5)?,
        r.get(6)?,
    ))
}

fn finish(parts: (OutboxOp, Vec<u8>, String)) -> CoreResult<OutboxOp> {
    let (mut op, payload, status) = parts;
    op.payload = from_msgpack(&payload)?;
    op.status = OpStatus::parse(&status)?;
    Ok(op)
}

fn query(conn: &Connection, where_sql: &str, p: impl rusqlite::Params) -> CoreResult<Vec<OutboxOp>> {
    let mut st = conn.prepare(&format!("SELECT {COLUMNS} FROM outbox {where_sql}"))?;
    let raw = st.query_map(p, row)?.collect::<Result<Vec<_>, _>>()?;
    raw.into_iter().map(finish).collect()
}

/// A new op to append.
#[derive(Debug, Clone)]
pub struct NewOp<'a> {
    /// Idempotency key.
    pub op_id: &'a str,
    /// Server entity.
    pub entity_id: &'a str,
    /// Local entity.
    pub local_entity: &'a str,
    /// Base version.
    pub base_version: Option<&'a str>,
    /// Payload.
    pub payload: &'a OpPayload,
    /// Now (RFC 3339 UTC).
    pub created: &'a str,
}

/// Appends a `pending` op after every existing op.
pub fn append(conn: &Connection, op: &NewOp<'_>) -> CoreResult<i64> {
    let ord: i64 = conn.query_row("SELECT COALESCE(MAX(ord), 0) + 1 FROM outbox", [], |r| {
        r.get(0)
    })?;
    conn.execute(
        "INSERT INTO outbox (op_id, ord, kind, entity_id, local_entity, base_version, payload,
                             status, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)",
        params![
            op.op_id,
            ord,
            op.payload.kind().as_str(),
            op.entity_id,
            op.local_entity,
            op.base_version,
            to_msgpack(op.payload)?,
            op.created
        ],
    )?;
    Ok(ord)
}

/// An op by ID.
pub fn get(conn: &Connection, op_id: &str) -> CoreResult<Option<OutboxOp>> {
    let raw = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM outbox WHERE op_id = ?1"),
            [op_id],
            row,
        )
        .optional()?;
    raw.map(finish).transpose()
}

/// Every op, in order.
pub fn all(conn: &Connection) -> CoreResult<Vec<OutboxOp>> {
    query(conn, "ORDER BY ord", [])
}

/// Live ops of a local entity, in order.
pub fn live_for(conn: &Connection, local_entity: &str) -> CoreResult<Vec<OutboxOp>> {
    query(
        conn,
        "WHERE local_entity = ?1 AND status IN ('pending', 'inflight', 'conflict', 'duplicate')
         ORDER BY ord",
        [local_entity],
    )
}

/// Every live op, in order.
pub fn live(conn: &Connection) -> CoreResult<Vec<OutboxOp>> {
    query(
        conn,
        "WHERE status IN ('pending', 'inflight', 'conflict', 'duplicate') ORDER BY ord",
        [],
    )
}

/// The next push batch: pending ops in order, at most `limit`, skipping ops on local entities
/// blocked by an unresolved conflict or duplicate (their later ops would only fail too).
pub fn next_batch(conn: &Connection, limit: usize) -> CoreResult<Vec<OutboxOp>> {
    let mut ops = query(
        conn,
        "WHERE status = 'pending'
           AND local_entity NOT IN (SELECT local_entity FROM outbox
                                    WHERE status IN ('conflict', 'duplicate'))
         ORDER BY ord",
        [],
    )?;
    ops.truncate(limit);
    Ok(ops)
}

/// Number of ops not yet synced (pending, inflight, conflict, duplicate).
pub fn unsynced_count(conn: &Connection) -> CoreResult<u32> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM outbox WHERE status IN ('pending', 'inflight', 'conflict', 'duplicate')",
        [],
        |r| r.get(0),
    )?)
}

/// Sets an op's status (and counts an attempt when it becomes `inflight`).
pub fn set_status(conn: &Connection, op_id: &str, status: OpStatus) -> CoreResult<()> {
    let bump = i32::from(status == OpStatus::Inflight);
    conn.execute(
        "UPDATE outbox SET status = ?2, attempts = attempts + ?3 WHERE op_id = ?1",
        params![op_id, status.as_str(), bump],
    )?;
    Ok(())
}

/// Records a transport error on inflight ops and puts them back to `pending`.
pub fn requeue_inflight(conn: &Connection, error: Option<&str>) -> CoreResult<usize> {
    Ok(conn.execute(
        "UPDATE outbox SET status = 'pending', last_error = COALESCE(?1, last_error)
         WHERE status = 'inflight'",
        params![error],
    )?)
}

/// Deletes `done` and `rejected` ops (compaction after results are recorded; a rejection's
/// details stay in `rejections`).
pub fn compact(conn: &Connection) -> CoreResult<usize> {
    Ok(conn.execute(
        "DELETE FROM outbox WHERE status IN ('done', 'rejected')",
        [],
    )?)
}

/// Deletes an op.
pub fn delete(conn: &Connection, op_id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM outbox WHERE op_id = ?1", [op_id])?;
    Ok(())
}
