//! Small local stores of v3: cached online-only reads (`remote_cache`), the sync log, pinned
//! notes and acknowledged AI decisions.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::CoreResult;

/// Cache key of the account's devices.
pub const DEVICES: &str = "devices";
/// Cache key of the AI status.
pub const AI_STATUS: &str = "ai_status";
/// Cache key of the integrity warnings.
pub const INTEGRITY: &str = "integrity";
/// Cache key of the last downloaded export's size and note count.
pub const EXPORT: &str = "export";
/// Cache key of the AI decisions (activity feed).
pub const AI_DECISIONS: &str = "ai_decisions";
/// Cache key of the similarity edges (global map).
pub const SIMILARITY: &str = "similarity";
/// Cache key of the pending-approval count (admins).
pub const ADMIN_PENDING: &str = "admin_pending";
/// Cache key of the pending sign-up / sign-in awaiting approval (in the registry database).
pub const PENDING_APPROVAL: &str = "pending_approval";

/// Cache key of a note's history.
pub fn history_key(note_id: &str) -> String {
    format!("history:{note_id}")
}

/// Stores a value (`MessagePack`, named fields).
pub fn put<T: Serialize>(conn: &Connection, key: &str, value: &T, now: &str) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO remote_cache (key, value, fetched_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, fetched_at = excluded.fetched_at",
        params![key, crate::store::to_msgpack(value)?, now],
    )?;
    Ok(())
}

/// Reads a value and when it was fetched (undecodable values read as missing).
pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> CoreResult<Option<(T, String)>> {
    let row: Option<(Vec<u8>, String)> = conn
        .query_row(
            "SELECT value, fetched_at FROM remote_cache WHERE key = ?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.and_then(|(v, at)| rmp_serde::from_slice(&v).ok().map(|v| (v, at))))
}

/// Removes a value.
pub fn remove(conn: &Connection, key: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM remote_cache WHERE key = ?1", [key])?;
    Ok(())
}

/// Most log entries kept.
pub const LOG_LIMIT: i64 = 50;

/// Appends a sync log entry (and trims the log).
pub fn log(conn: &Connection, at: &str, kind: &str, detail: &str) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO sync_log (at, kind, detail) VALUES (?1, ?2, ?3)",
        params![at, kind, detail],
    )?;
    conn.execute(
        "DELETE FROM sync_log WHERE id NOT IN (SELECT id FROM sync_log ORDER BY id DESC LIMIT ?1)",
        [LOG_LIMIT],
    )?;
    Ok(())
}

/// Log entries, newest first: (at, kind, detail).
pub fn log_entries(conn: &Connection) -> CoreResult<Vec<(String, String, String)>> {
    let mut st = conn.prepare("SELECT at, kind, detail FROM sync_log ORDER BY id DESC")?;
    Ok(st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?)
}

/// Pinned note IDs in pin order.
pub fn pinned(conn: &Connection) -> CoreResult<Vec<String>> {
    let mut st = conn.prepare("SELECT note_id FROM pinned_notes ORDER BY ord, note_id")?;
    Ok(st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

/// Pins (at the end) or unpins a note. Returns whether anything changed.
pub fn set_pinned(conn: &Connection, note_id: &str, pinned: bool) -> CoreResult<bool> {
    if pinned {
        Ok(conn.execute(
            "INSERT OR IGNORE INTO pinned_notes (note_id, ord)
             VALUES (?1, (SELECT COALESCE(MAX(ord), 0) + 1 FROM pinned_notes))",
            [note_id],
        )? > 0)
    } else {
        Ok(conn.execute("DELETE FROM pinned_notes WHERE note_id = ?1", [note_id])? > 0)
    }
}

/// Records "Looks right" on an AI decision.
pub fn acknowledge(conn: &Connection, id: &str, now: &str) -> CoreResult<bool> {
    Ok(conn.execute(
        "INSERT OR IGNORE INTO acknowledged_suggestions (id, at) VALUES (?1, ?2)",
        params![id, now],
    )? > 0)
}
