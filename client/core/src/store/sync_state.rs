//! `sync_state` (§12.2): epoch, cursor, bootstrap progress, last push/pull, failures.

use rusqlite::{Connection, params};

use crate::error::CoreResult;

/// The sync state row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncState {
    /// The server epoch the cache belongs to (`None` before the first bootstrap).
    pub epoch: Option<u64>,
    /// Last applied change seq.
    pub cursor_seq: u64,
    /// Cursor of the next bootstrap page while bootstrapping.
    pub bootstrap_cursor: Option<String>,
    /// The cache holds a full snapshot.
    pub bootstrap_complete: bool,
    /// Bootstrap pages applied so far (progress).
    pub bootstrap_pages: u32,
    /// Last successful pull (RFC 3339 UTC).
    pub last_pull_at: Option<String>,
    /// Last successful push.
    pub last_push_at: Option<String>,
    /// Consecutive failed sync cycles (backoff input).
    pub consecutive_failures: u32,
    /// Last error message key.
    pub last_error: Option<String>,
}

fn to_u64(v: Option<i64>) -> Option<u64> {
    v.and_then(|x| u64::try_from(x).ok())
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Reads the state.
pub fn get(conn: &Connection) -> CoreResult<SyncState> {
    Ok(conn.query_row(
        "SELECT epoch, cursor_seq, bootstrap_cursor, bootstrap_complete, bootstrap_pages,
                last_pull_at, last_push_at, consecutive_failures, last_error
         FROM sync_state WHERE singleton = 1",
        [],
        |r| {
            Ok(SyncState {
                epoch: to_u64(r.get(0)?),
                cursor_seq: to_u64(r.get(1)?).unwrap_or(0),
                bootstrap_cursor: r.get(2)?,
                bootstrap_complete: r.get(3)?,
                bootstrap_pages: r.get(4)?,
                last_pull_at: r.get(5)?,
                last_push_at: r.get(6)?,
                consecutive_failures: r.get(7)?,
                last_error: r.get(8)?,
            })
        },
    )?)
}

/// Writes the state.
pub fn put(conn: &Connection, s: &SyncState) -> CoreResult<()> {
    conn.execute(
        "UPDATE sync_state SET epoch = ?1, cursor_seq = ?2, bootstrap_cursor = ?3,
                bootstrap_complete = ?4, bootstrap_pages = ?5, last_pull_at = ?6,
                last_push_at = ?7, consecutive_failures = ?8, last_error = ?9
         WHERE singleton = 1",
        params![
            s.epoch.map(to_i64),
            to_i64(s.cursor_seq),
            s.bootstrap_cursor,
            s.bootstrap_complete,
            s.bootstrap_pages,
            s.last_pull_at,
            s.last_push_at,
            s.consecutive_failures,
            s.last_error
        ],
    )?;
    Ok(())
}

/// Updates the state with `f`.
pub fn update(conn: &Connection, f: impl FnOnce(&mut SyncState)) -> CoreResult<SyncState> {
    let mut s = get(conn)?;
    f(&mut s);
    put(conn, &s)?;
    Ok(s)
}
