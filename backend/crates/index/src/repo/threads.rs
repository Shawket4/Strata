//! A note's AI follow-up thread (`note_threads`, derived from `.meta/threads/<id>.json`).

use strata_common::NoteId;

use crate::error::Result;
use crate::scope::ScopedTx;

/// A `note_threads` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ThreadRow {
    /// The note.
    pub note_id: NoteId,
    /// The file's JSON.
    pub thread: String,
    /// Content hash of `thread`.
    pub version: String,
}

/// The thread of `note`.
pub async fn get(tx: &mut ScopedTx, note: NoteId) -> Result<Option<ThreadRow>> {
    Ok(
        sqlx::query_as("SELECT note_id, thread, version FROM note_threads WHERE note_id = $1")
            .bind(note)
            .fetch_optional(tx.conn())
            .await?,
    )
}

/// Stores the thread of `note`. Returns whether the row changed (a new version).
pub async fn put(tx: &mut ScopedTx, note: NoteId, thread: &str, version: &str) -> Result<bool> {
    let n = sqlx::query(
        "INSERT INTO note_threads (user_id, note_id, thread, version) \
         VALUES (strata_current_user(), $1, $2, $3) \
         ON CONFLICT (user_id, note_id) DO UPDATE \
           SET thread = excluded.thread, version = excluded.version \
           WHERE note_threads.version <> excluded.version",
    )
    .bind(note)
    .bind(thread)
    .bind(version)
    .execute(tx.conn())
    .await?
    .rows_affected();
    Ok(n > 0)
}

/// Removes the thread of `note`. Returns whether a row was removed.
pub async fn delete(tx: &mut ScopedTx, note: NoteId) -> Result<bool> {
    Ok(sqlx::query("DELETE FROM note_threads WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?
        .rows_affected()
        > 0)
}

/// Every thread's note and version (reindex compares them with the files).
pub async fn versions(tx: &mut ScopedTx) -> Result<Vec<(NoteId, String)>> {
    Ok(
        sqlx::query_as("SELECT note_id, version FROM note_threads ORDER BY note_id")
            .fetch_all(tx.conn())
            .await?,
    )
}

/// Threads after `after` (by note), at most `limit` (bootstrap paging).
pub async fn page(tx: &mut ScopedTx, after: Option<NoteId>, limit: i64) -> Result<Vec<ThreadRow>> {
    Ok(sqlx::query_as(
        "SELECT note_id, thread, version FROM note_threads \
         WHERE $1::uuid IS NULL OR note_id > $1 ORDER BY note_id LIMIT $2",
    )
    .bind(after)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}
