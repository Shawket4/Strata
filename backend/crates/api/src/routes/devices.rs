//! `GET /devices`, `PATCH /devices/{id}`, `DELETE /devices/{id}` (PLAN §7.5, §8, D27).
//!
//! Everything runs in the caller's `UserScope`: row-level security makes another user's
//! device ID indistinguishable from a missing one (`404`, principle 7).
//!
//! `reminders_enabled` (D27: whether this device schedules local reminder notifications) is
//! kept in the user's `settings` table under `device.<id>.reminders_enabled` until the
//! `devices` table carries it; it defaults to `true` and is removed with the device.

use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::DeviceId;
use strata_index::accounts::Device as DeviceRow;
use strata_index::repo::{devices, settings};
use strata_index::ScopedTx;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::service::check_device_name;
use crate::auth::{AccountError, AuthState, Authenticated};
use crate::routes::auth::DevicePlatform;
use crate::wire::{MsgPack, MsgPackConfig, Problem};

/// A signed-in device of the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Device {
    /// Device ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name given at login or by `PATCH`.
    pub name: String,
    /// Platform.
    pub platform: DevicePlatform,
    /// First login.
    pub created: DateTime<Utc>,
    /// Last token refresh.
    pub last_seen: DateTime<Utc>,
    /// This is the device making the request.
    pub current: bool,
    /// Whether this device schedules local reminder notifications (D27).
    pub reminders_enabled: bool,
}

/// `PATCH /devices/{id}` body; absent fields are unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdateDevice {
    /// New name (1–100 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Turn local reminder notifications on this device on or off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminders_enabled: Option<bool>,
}

fn reminders_key(id: DeviceId) -> String {
    format!("device.{id}.reminders_enabled")
}

async fn reminders_enabled(tx: &mut ScopedTx, id: DeviceId) -> Result<bool, AccountError> {
    Ok(settings::get_setting(tx, &reminders_key(id))
        .await?
        .and_then(|b| rmp_serde::from_slice::<bool>(&b).ok())
        .unwrap_or(true))
}

async fn view(
    tx: &mut ScopedTx,
    row: DeviceRow,
    current: DeviceId,
) -> Result<Device, AccountError> {
    Ok(Device {
        id: row.id.as_ulid(),
        reminders_enabled: reminders_enabled(tx, row.id).await?,
        current: row.id == current,
        name: row.name,
        platform: row.platform.into(),
        created: row.created,
        last_seen: row.last_seen,
    })
}

/// The caller's devices, most recently seen first.
#[utoipa::path(
    get,
    path = "/devices",
    tag = "devices",
    operation_id = "list_devices",
    responses((status = 200, description = "Every device signed in to the caller's account.", body = Vec<Device>)),
)]
pub async fn list_devices(
    state: web::Data<AuthState>,
    auth: Authenticated,
) -> Result<MsgPack<Vec<Device>>, AccountError> {
    let mut tx = state.app_db.begin(auth.scope()).await?;
    let rows = devices::list_devices(&mut tx).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(view(&mut tx, row, auth.device).await?);
    }
    tx.commit().await?;
    Ok(MsgPack(out))
}

/// Rename a device or switch its reminders.
#[utoipa::path(
    patch,
    path = "/devices/{id}",
    tag = "devices",
    operation_id = "update_device",
    params(("id" = Ulid, Path, description = "Device ID (ULID).")),
    request_body = UpdateDevice,
    responses(
        (status = 200, description = "The updated device.", body = Device),
        (status = 422, description = "`invalid_body`: code `invalid_device_name`, or a decoding error.", body = Problem),
    ),
)]
pub async fn update_device(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
    body: MsgPack<UpdateDevice>,
) -> Result<MsgPack<Device>, AccountError> {
    let id = DeviceId::from_ulid(id.into_inner());
    let body = body.into_inner();
    let name = body.name.as_deref().map(check_device_name).transpose()?;
    let mut tx = state.app_db.begin(auth.scope()).await?;
    if devices::get_device(&mut tx, id).await?.is_none() {
        return Err(AccountError::NotFound);
    }
    if let Some(name) = name {
        sqlx::query("UPDATE devices SET name = $2 WHERE id = $1")
            .bind(id)
            .bind(name)
            .execute(tx.conn())
            .await?;
    }
    if let Some(enabled) = body.reminders_enabled {
        let value =
            rmp_serde::to_vec(&enabled).map_err(|e| AccountError::Internal(e.to_string()))?;
        settings::put_setting(&mut tx, &reminders_key(id), &value, state.clock.now()).await?;
    }
    let row = devices::get_device(&mut tx, id)
        .await?
        .ok_or(AccountError::NotFound)?;
    let device = view(&mut tx, row, auth.device).await?;
    tx.commit().await?;
    Ok(MsgPack(device))
}

/// Remove a device: its sessions end immediately (removing the current device signs out).
#[utoipa::path(
    delete,
    path = "/devices/{id}",
    tag = "devices",
    operation_id = "delete_device",
    params(("id" = Ulid, Path, description = "Device ID (ULID).")),
    responses((status = 204, description = "Removed; its tokens stop working immediately.")),
)]
pub async fn delete_device(
    state: web::Data<AuthState>,
    auth: Authenticated,
    id: web::Path<Ulid>,
) -> Result<HttpResponse, AccountError> {
    let id = DeviceId::from_ulid(id.into_inner());
    let mut tx = state.app_db.begin(auth.scope()).await?;
    // Revoke in memory before the rows go (fail closed).
    let sessions: Vec<strata_common::SessionId> =
        sqlx::query_scalar("SELECT id FROM sessions WHERE device_id = $1")
            .bind(id)
            .fetch_all(tx.conn())
            .await?;
    state.revocations.revoke_sessions(sessions);
    let removed = devices::delete_device(&mut tx, id)
        .await?
        .ok_or(AccountError::NotFound)?;
    state.revocations.revoke_sessions(removed);
    sqlx::query("DELETE FROM settings WHERE key = $1")
        .bind(reminders_key(id))
        .execute(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Mounts the device routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/devices", web::get().to(list_devices)).service(
        web::resource("/devices/{id}")
            .app_data(MsgPackConfig::default().with_body_limit(8 * 1024))
            .route(web::patch().to(update_device))
            .route(web::delete().to(delete_device)),
    );
}
