//! `device_settings`: per-device settings of the account (§12.5b "per device").

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;

/// Reminders on this device (`true`/`false`, default on).
pub const REMINDERS_ENABLED: &str = "reminders_enabled";
/// Last notification permission state reported by the platform (`granted` / `denied` /
/// `platform_limit`).
pub const NOTIFICATION_PERMISSION: &str = "notification_permission";
/// Time used for date-only reminders (`HH:MM`, default `09:00`).
pub const DEFAULT_REMINDER_TIME: &str = "default_reminder_time";

/// Reads a setting.
pub fn get(conn: &Connection, key: &str) -> CoreResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM device_settings WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?)
}

/// Writes a setting. Returns whether it changed.
pub fn set(conn: &Connection, key: &str, value: &str) -> CoreResult<bool> {
    if get(conn, key)?.as_deref() == Some(value) {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO device_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(true)
}

/// Whether reminders are on for this device.
pub fn reminders_enabled(conn: &Connection) -> CoreResult<bool> {
    Ok(get(conn, REMINDERS_ENABLED)?.as_deref() != Some("false"))
}
