//! `scheduled_notifications`: what this device asked the OS to schedule (§12.5b).

use rusqlite::{Connection, params};

use crate::error::CoreResult;

/// A notification this device has asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledRow {
    /// Stable 31-bit ID.
    pub id: i32,
    /// `<task id>@<remind_at>`.
    pub key: String,
    /// Task block ID.
    pub task_id: String,
    /// Reminder wall-clock time as written (`YYYY-MM-DD HH:MM`).
    pub remind_at: String,
    /// Exact UTC instant (RFC 3339).
    pub fire_at: String,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// `requested` | `scheduled` | `failed` | `shown`.
    pub state: String,
    /// Last platform result.
    pub last_result: Option<String>,
}

/// Every row, ordered by fire time then ID.
pub fn all(conn: &Connection) -> CoreResult<Vec<ScheduledRow>> {
    let mut st = conn.prepare(
        "SELECT id, key, task_id, remind_at, fire_at, title, body, state, last_result
         FROM scheduled_notifications ORDER BY fire_at, id",
    )?;
    Ok(st
        .query_map([], |r| {
            Ok(ScheduledRow {
                id: r.get(0)?,
                key: r.get(1)?,
                task_id: r.get(2)?,
                remind_at: r.get(3)?,
                fire_at: r.get(4)?,
                title: r.get(5)?,
                body: r.get(6)?,
                state: r.get(7)?,
                last_result: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

/// Inserts or replaces a row.
pub fn put(conn: &Connection, row: &ScheduledRow, now: &str) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO scheduled_notifications (id, key, task_id, remind_at, fire_at,
             title, body, state, last_result, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.id,
            row.key,
            row.task_id,
            row.remind_at,
            row.fire_at,
            row.title,
            row.body,
            row.state,
            row.last_result,
            now
        ],
    )?;
    Ok(())
}

/// Deletes a row.
pub fn delete(conn: &Connection, id: i32) -> CoreResult<()> {
    conn.execute("DELETE FROM scheduled_notifications WHERE id = ?1", [id])?;
    Ok(())
}

/// Records a platform result. Returns whether a row was updated.
pub fn set_result(conn: &Connection, id: i32, state: &str, result: &str, now: &str) -> CoreResult<bool> {
    Ok(conn.execute(
        "UPDATE scheduled_notifications SET state = ?2, last_result = ?3, updated_at = ?4
         WHERE id = ?1",
        params![id, state, result, now],
    )? > 0)
}
