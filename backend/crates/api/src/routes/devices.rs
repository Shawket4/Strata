//! `GET /devices`, `PATCH /devices/{id}`, `DELETE /devices/{id}` (PLAN §7.5, §8, D27).
//!
//! Everything runs in the caller's `UserScope`: row-level security makes another user's
//! device ID indistinguishable from a missing one (`404`, principle 7).
//!
//! `reminders_enabled` (D27: whether this device schedules local reminder notifications) is
//! the `devices.reminders_enabled` column (default `true`). A change appends the
//! `device_setting` change-log row `<device>:reminders_enabled` (removing a device, its
//! tombstone), so every device of the user pulls it from `/sync/changes`.

use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::DeviceId;
use strata_index::accounts::Device as DeviceRow;
use strata_index::repo::devices;
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

fn view(row: DeviceRow, current: DeviceId) -> Device {
    Device {
        id: row.id.as_ulid(),
        reminders_enabled: row.reminders_enabled,
        current: row.id == current,
        name: row.name,
        platform: row.platform.into(),
        created: row.created,
        last_seen: row.last_seen,
    }
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
    tx.commit().await?;
    Ok(MsgPack(
        rows.into_iter().map(|row| view(row, auth.device)).collect(),
    ))
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
        devices::rename_device(&mut tx, id, &name).await?;
    }
    if let Some(enabled) = body.reminders_enabled {
        // Logged as the `device_setting` record every device pulls (like `device.settings`).
        crate::sync::push::set_device_reminders(&mut tx, id, enabled, state.clock.now()).await?;
    }
    let row = devices::get_device(&mut tx, id)
        .await?
        .ok_or(AccountError::NotFound)?;
    tx.commit().await?;
    Ok(MsgPack(view(row, auth.device)))
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
    // Its device setting record is gone: a tombstone for the other devices.
    let entity = format!("{}:reminders_enabled", id.as_ulid());
    strata_index::repo::sync::append_change(
        &mut tx,
        &strata_index::repo::sync::NewChange {
            entity_type: "device_setting",
            entity_id: &entity,
            op: strata_index::types::ChangeOp::Delete,
            version: None,
            at: state.clock.now(),
        },
    )
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
