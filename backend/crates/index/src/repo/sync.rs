//! Change log, sync epochs, and push idempotency (§7.5 sync, §12.4).

use chrono::{DateTime, Utc};
use strata_common::{DeviceId, OpId};

use crate::error::{IndexError, Result};
use crate::scope::ScopedTx;
use crate::types::ChangeOp;

/// A user's sync position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct SyncPosition {
    /// Current epoch (starts at 1).
    pub epoch: i32,
    /// Highest assigned seq (0 before the first change).
    pub last_seq: i64,
}

/// Input for [`append_change`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChange<'a> {
    /// `note`, `entity`, `relation`, `task`, …
    pub entity_type: &'a str,
    /// Entity ID (ULID string or task block ID).
    pub entity_id: &'a str,
    /// Upsert or delete (tombstone).
    pub op: ChangeOp,
    /// New version, if the entity is versioned.
    pub version: Option<&'a str>,
    /// Commit time.
    pub at: DateTime<Utc>,
}

/// A `change_log` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Change {
    /// Sequence number, gapless and commit-ordered per user.
    pub seq: i64,
    /// Epoch it was written in.
    pub epoch: i32,
    /// Entity type.
    pub entity_type: String,
    /// Entity ID.
    pub entity_id: String,
    /// Operation.
    pub op: ChangeOp,
    /// Version.
    pub version: Option<String>,
    /// Commit time.
    pub at: DateTime<Utc>,
}

/// A stored push result.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct IdempotencyRecord {
    /// Client op ID.
    pub op_id: OpId,
    /// Pushing device.
    pub device_id: DeviceId,
    /// MessagePack result.
    pub result: Vec<u8>,
    /// Stored at.
    pub created: DateTime<Utc>,
}

/// Appends a change. The per-user counter row is locked until commit, so concurrent appends
/// for one user serialise and seq order equals commit order (a reader never skips a row).
pub async fn append_change(tx: &mut ScopedTx, change: &NewChange<'_>) -> Result<Change> {
    let pos: SyncPosition = sqlx::query_as(
        "INSERT INTO sync_epochs AS s (user_id, epoch, last_seq, updated) \
         VALUES (strata_current_user(), 1, 1, $1) \
         ON CONFLICT (user_id) DO UPDATE SET last_seq = s.last_seq + 1, updated = EXCLUDED.updated \
         RETURNING epoch, last_seq",
    )
    .bind(change.at)
    .fetch_one(tx.conn())
    .await?;
    Ok(sqlx::query_as(
        "INSERT INTO change_log (user_id, seq, epoch, entity_type, entity_id, op, version, at) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7) \
         RETURNING seq, epoch, entity_type, entity_id, op, version, at",
    )
    .bind(pos.last_seq)
    .bind(pos.epoch)
    .bind(change.entity_type)
    .bind(change.entity_id)
    .bind(change.op)
    .bind(change.version)
    .bind(change.at)
    .fetch_one(tx.conn())
    .await?)
}

/// The user's current position (epoch 1, seq 0 before any change).
pub async fn sync_position(tx: &mut ScopedTx) -> Result<SyncPosition> {
    let pos: Option<SyncPosition> = sqlx::query_as("SELECT epoch, last_seq FROM sync_epochs")
        .fetch_optional(tx.conn())
        .await?;
    Ok(pos.unwrap_or(SyncPosition {
        epoch: 1,
        last_seq: 0,
    }))
}

/// Changes with `seq > since` in ascending order, at most `limit`. Fails with
/// [`IndexError::EpochChanged`] if `epoch` is not the current one (client re-bootstraps).
pub async fn changes_since(
    tx: &mut ScopedTx,
    epoch: i32,
    since: i64,
    limit: i64,
) -> Result<Vec<Change>> {
    let current = sync_position(tx).await?;
    if current.epoch != epoch {
        return Err(IndexError::EpochChanged {
            current: current.epoch,
        });
    }
    Ok(sqlx::query_as(
        "SELECT seq, epoch, entity_type, entity_id, op, version, at FROM change_log \
         WHERE epoch = $1 AND seq > $2 ORDER BY seq LIMIT $3",
    )
    .bind(epoch)
    .bind(since)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

/// Starts a new epoch after a rebuild that cannot preserve seq: bumps the epoch, keeps seq
/// monotonic, and drops the old epoch's log (clients must re-bootstrap anyway).
pub async fn bump_epoch(tx: &mut ScopedTx, now: DateTime<Utc>) -> Result<SyncPosition> {
    let pos: SyncPosition = sqlx::query_as(
        "INSERT INTO sync_epochs AS s (user_id, epoch, last_seq, updated) \
         VALUES (strata_current_user(), 2, 0, $1) \
         ON CONFLICT (user_id) DO UPDATE SET epoch = s.epoch + 1, updated = EXCLUDED.updated \
         RETURNING epoch, last_seq",
    )
    .bind(now)
    .fetch_one(tx.conn())
    .await?;
    sqlx::query("DELETE FROM change_log WHERE epoch < $1")
        .bind(pos.epoch)
        .execute(tx.conn())
        .await?;
    Ok(pos)
}

/// The stored result for `op_id`, if the op was already applied.
pub async fn idempotency_get(tx: &mut ScopedTx, op_id: OpId) -> Result<Option<IdempotencyRecord>> {
    Ok(sqlx::query_as(
        "SELECT op_id, device_id, result, created FROM idempotency WHERE op_id = $1",
    )
    .bind(op_id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Stores the result for `op_id` unless one exists; returns the record that is stored (the
/// earlier one wins, so a replay always sees the original result).
pub async fn idempotency_put(
    tx: &mut ScopedTx,
    op_id: OpId,
    device_id: DeviceId,
    result: &[u8],
    now: DateTime<Utc>,
) -> Result<IdempotencyRecord> {
    let inserted: Option<IdempotencyRecord> = sqlx::query_as(
        "INSERT INTO idempotency (user_id, op_id, device_id, result, created) \
         VALUES (strata_current_user(), $1, $2, $3, $4) ON CONFLICT (user_id, op_id) DO NOTHING \
         RETURNING op_id, device_id, result, created",
    )
    .bind(op_id)
    .bind(device_id)
    .bind(result)
    .bind(now)
    .fetch_optional(tx.conn())
    .await?;
    match inserted {
        Some(r) => Ok(r),
        None => idempotency_get(tx, op_id)
            .await?
            .ok_or_else(|| IndexError::InvalidArgument("idempotency record vanished".into())),
    }
}

/// Deletes idempotency records created before `before`; returns how many.
pub async fn idempotency_purge_before(tx: &mut ScopedTx, before: DateTime<Utc>) -> Result<u64> {
    Ok(sqlx::query("DELETE FROM idempotency WHERE created < $1")
        .bind(before)
        .execute(tx.conn())
        .await?
        .rows_affected())
}
