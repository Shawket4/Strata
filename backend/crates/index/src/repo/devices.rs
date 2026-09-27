//! The app-side view of the scoped user's devices and sessions (`GET /devices`,
//! `DELETE /devices/{id}`, `PUT /devices/{id}/push`, logout). Devices and sessions are created
//! by the accounts layer at login.

use chrono::{DateTime, Utc};
use strata_common::{DeviceId, SessionId};

use crate::accounts::{Device, Session};
use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{PushProvider, RevokeReason};

const DEVICE_COLS: &str =
    "user_id, id, name, platform, created, last_seen, push_provider, push_token, push_updated";

/// The user's devices, most recently seen first.
pub async fn list_devices(tx: &mut ScopedTx) -> Result<Vec<Device>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {DEVICE_COLS} FROM devices ORDER BY last_seen DESC, id"
    )))
    .fetch_all(tx.conn())
    .await?)
}

/// A device by ID.
pub async fn get_device(tx: &mut ScopedTx, id: DeviceId) -> Result<Option<Device>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {DEVICE_COLS} FROM devices WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Registers (or with `PushProvider::None`, clears) a device's push token. False if absent.
pub async fn set_push(
    tx: &mut ScopedTx,
    id: DeviceId,
    provider: PushProvider,
    token: Option<&str>,
    now: DateTime<Utc>,
) -> Result<bool> {
    let token = if provider == PushProvider::None { None } else { token };
    let done = sqlx::query(
        "UPDATE devices SET push_provider = $2, push_token = $3, push_updated = $4 WHERE id = $1",
    )
    .bind(id)
    .bind(provider)
    .bind(token)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Updates `last_seen`. False if absent.
pub async fn touch_device(tx: &mut ScopedTx, id: DeviceId, now: DateTime<Utc>) -> Result<bool> {
    let done = sqlx::query("UPDATE devices SET last_seen = GREATEST(last_seen, $2) WHERE id = $1")
        .bind(id)
        .bind(now)
        .execute(tx.conn())
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Removes a device; its sessions and refresh tokens go with it. Returns the removed device's
/// session IDs (for the in-memory revocation set), or None if the device is absent.
pub async fn delete_device(tx: &mut ScopedTx, id: DeviceId) -> Result<Option<Vec<SessionId>>> {
    let sessions: Vec<SessionId> =
        sqlx::query_scalar("SELECT id FROM sessions WHERE device_id = $1 ORDER BY id")
            .bind(id)
            .fetch_all(tx.conn())
            .await?;
    let done = sqlx::query("DELETE FROM devices WHERE id = $1")
        .bind(id)
        .execute(tx.conn())
        .await?;
    Ok((done.rows_affected() == 1).then_some(sessions))
}

/// The user's sessions, newest first.
pub async fn list_sessions(tx: &mut ScopedTx) -> Result<Vec<Session>> {
    Ok(sqlx::query_as(
        "SELECT user_id, id, device_id, created, expires, revoked_at, revoked_reason, export_only \
         FROM sessions ORDER BY created DESC, id DESC",
    )
    .fetch_all(tx.conn())
    .await?)
}

/// Revokes one of the user's sessions (logout). False if absent or already revoked.
pub async fn revoke_session(
    tx: &mut ScopedTx,
    id: SessionId,
    reason: RevokeReason,
    now: DateTime<Utc>,
) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE sessions SET revoked_at = $3, revoked_reason = $2 WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(reason)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}
