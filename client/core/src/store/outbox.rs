//! The outbox (§12.2): pending mutations, pushed in `ord` order, surviving restarts.
//!
//! Status machine: `pending` → `inflight` (pushed, awaiting results) → `done` | `conflict` |
//! `duplicate` | `rejected`. An `inflight` op found at startup was interrupted mid-push and goes
//! back to `pending`; re-pushing it with the same `op_id` is safe (server idempotency).
//! `conflict` and `duplicate` ops are **live** (their local effect stays visible) until the user
//! resolves them.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{CoreError, CoreResult};
use crate::store::parse_ulid;
use crate::sync::model::{Op, OpKind, SyncOp, Version};

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
    pub base_version: Option<Version>,
    /// The op (kind + payload).
    pub op: Op,
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
        self.op.kind()
    }

    /// The op as pushed.
    pub fn to_push(&self) -> CoreResult<SyncOp> {
        Ok(SyncOp {
            op_id: parse_ulid(&self.op_id)?,
            entity_id: self.entity_id.clone(),
            base_version: self.base_version.clone(),
            op: self.op.clone(),
        })
    }
}

const COLUMNS: &str =
    "op_id, ord, entity_id, local_entity, base_version, kind, payload, status, attempts, created";

struct RawRow {
    op_id: String,
    ord: i64,
    entity_id: String,
    local_entity: String,
    base_version: Option<String>,
    kind: String,
    payload: Vec<u8>,
    status: String,
    attempts: u32,
    created: String,
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawRow> {
    Ok(RawRow {
        op_id: r.get(0)?,
        ord: r.get(1)?,
        entity_id: r.get(2)?,
        local_entity: r.get(3)?,
        base_version: r.get(4)?,
        kind: r.get(5)?,
        payload: r.get(6)?,
        status: r.get(7)?,
        attempts: r.get(8)?,
        created: r.get(9)?,
    })
}

fn finish(raw: RawRow) -> CoreResult<OutboxOp> {
    let bad = |e: String| CoreError::Storage(format!("outbox op {}: {e}", raw.op_id));
    let kind: OpKind = raw.kind.parse().map_err(|e: sync_model::OpError| bad(e.to_string()))?;
    let op = Op::from_parts(kind, &raw.payload).map_err(|e| bad(e.to_string()))?;
    let base_version = raw
        .base_version
        .as_deref()
        .map(str::parse::<Version>)
        .transpose()
        .map_err(|e| bad(e.to_string()))?;
    Ok(OutboxOp {
        status: OpStatus::parse(&raw.status)?,
        op_id: raw.op_id,
        ord: raw.ord,
        entity_id: raw.entity_id,
        local_entity: raw.local_entity,
        base_version,
        op,
        attempts: raw.attempts,
        created: raw.created,
    })
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
    /// Local entity.
    pub local_entity: &'a str,
    /// Base version.
    pub base_version: Option<&'a Version>,
    /// The op.
    pub op: &'a Op,
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
            op.op.kind().as_str(),
            op.op.entity_id(),
            op.local_entity,
            op.base_version.map(Version::as_str),
            op.op.payload_bytes()?,
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
