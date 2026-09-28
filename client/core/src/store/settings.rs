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

/// Snooze length in minutes (default 15).
pub const SNOOZE_MINUTES: &str = "snooze_minutes";
/// Quiet hours on (`true`/`false`).
pub const QUIET_ENABLED: &str = "quiet_enabled";
/// Quiet hours start (`HH:MM`, default `22:00`).
pub const QUIET_FROM: &str = "quiet_from";
/// Quiet hours end (`HH:MM`, default `07:00`).
pub const QUIET_UNTIL: &str = "quiet_until";

/// Default snooze length.
pub const DEFAULT_SNOOZE_MINUTES: u32 = 15;

/// Snooze length.
pub fn snooze_minutes(conn: &Connection) -> CoreResult<u32> {
    Ok(get(conn, SNOOZE_MINUTES)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SNOOZE_MINUTES))
}

/// Quiet hours: `(enabled, from, until)` as `HH:MM`.
pub fn quiet_hours(conn: &Connection) -> CoreResult<(bool, String, String)> {
    Ok((
        get(conn, QUIET_ENABLED)?.as_deref() == Some("true"),
        get(conn, QUIET_FROM)?.unwrap_or_else(|| "22:00".to_owned()),
        get(conn, QUIET_UNTIL)?.unwrap_or_else(|| "07:00".to_owned()),
    ))
}

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
