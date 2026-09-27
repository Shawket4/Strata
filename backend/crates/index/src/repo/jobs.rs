//! The per-user job queue (§9.2): enqueue with optional debounce, claim with
//! `FOR UPDATE SKIP LOCKED`, complete, fail with retry, and recover stale claims.
//!
//! Workers find users with runnable jobs through [`crate::scope::AppDb::due_job_users`] (the
//! global, content-free `job_wakeups` hints kept current by a trigger), open a scoped
//! transaction for one user, claim, and commit; the claim marks the job `running`, so the
//! actual work happens outside any transaction and ends with [`complete`] or [`fail`].

use chrono::{DateTime, Utc};
use strata_common::{JobId, NoteId};

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::JobStatus;

/// A `jobs` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Job {
    /// ID.
    pub id: JobId,
    /// Kind (`embed`, `link`, `file_inbox`, …).
    pub kind: String,
    /// Note the job is about.
    pub note_id: Option<NoteId>,
    /// `MessagePack` parameters.
    pub payload: Vec<u8>,
    /// Status.
    pub status: JobStatus,
    /// Claims so far.
    pub attempts: i32,
    /// Claims allowed before the job fails permanently.
    pub max_attempts: i32,
    /// Not claimable before this time.
    pub run_after: DateTime<Utc>,
    /// Claim time while running.
    pub locked_at: Option<DateTime<Utc>>,
    /// Last failure message.
    pub last_error: Option<String>,
    /// Debounce key.
    pub dedupe_key: Option<String>,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Last change.
    pub updated: DateTime<Utc>,
}

/// Input for [`enqueue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewJob {
    /// ID for a new row (unused when a debounced job is updated instead).
    pub id: JobId,
    /// Kind.
    pub kind: String,
    /// Note.
    pub note_id: Option<NoteId>,
    /// `MessagePack` parameters.
    pub payload: Vec<u8>,
    /// Earliest run time.
    pub run_after: DateTime<Utc>,
    /// Attempt budget (≥ 1).
    pub max_attempts: i32,
    /// With a key, an existing *queued* job of the same kind and key is updated (new payload
    /// and `run_after`) instead of adding a second one — the debounce of §9.2.
    pub dedupe_key: Option<String>,
}

const COLS: &str = "id, kind, note_id, payload, status, attempts, max_attempts, run_after, \
    locked_at, last_error, dedupe_key, created, updated";

/// Enqueues a job (or re-times the queued job with the same debounce key); returns the row.
pub async fn enqueue(tx: &mut ScopedTx, job: &NewJob, now: DateTime<Utc>) -> Result<Job> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO jobs (user_id, id, kind, note_id, payload, status, attempts, max_attempts, \
           run_after, dedupe_key, created, updated) \
         VALUES (strata_current_user(), $1, $2, $3, $4, 'queued', 0, $5, $6, $7, $8, $8) \
         ON CONFLICT (user_id, kind, dedupe_key) WHERE status = 'queued' AND dedupe_key IS NOT NULL \
         DO UPDATE SET payload = EXCLUDED.payload, run_after = EXCLUDED.run_after, \
           note_id = EXCLUDED.note_id, updated = EXCLUDED.updated \
         RETURNING {COLS}"
    )))
    .bind(job.id)
    .bind(&job.kind)
    .bind(job.note_id)
    .bind(&job.payload)
    .bind(job.max_attempts)
    .bind(job.run_after)
    .bind(&job.dedupe_key)
    .bind(now)
    .fetch_one(tx.conn())
    .await?)
}

/// Claims the next runnable job of the scoped user (earliest `run_after`, then ID), skipping
/// rows locked by concurrent claimers. The job becomes `running` with `attempts + 1`.
pub async fn claim_next(tx: &mut ScopedTx, now: DateTime<Utc>) -> Result<Option<Job>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, locked_at = $1, updated = $1 \
         WHERE (user_id, id) = ( \
           SELECT user_id, id FROM jobs WHERE status = 'queued' AND run_after <= $1 \
           ORDER BY run_after, id FOR UPDATE SKIP LOCKED LIMIT 1) \
         RETURNING {COLS}"
    )))
    .bind(now)
    .fetch_optional(tx.conn())
    .await?)
}

/// A job by ID.
pub async fn get_job(tx: &mut ScopedTx, id: JobId) -> Result<Option<Job>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM jobs WHERE id = $1")))
        .bind(id)
        .fetch_optional(tx.conn())
        .await?)
}

/// Marks a running job done. False if it is not running.
pub async fn complete(tx: &mut ScopedTx, id: JobId, now: DateTime<Utc>) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE jobs SET status = 'done', locked_at = NULL, last_error = NULL, updated = $2 \
         WHERE id = $1 AND status = 'running'",
    )
    .bind(id)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Records a failure of a running job. With `retry_at` and attempts left it is re-queued for
/// then (backoff chosen by the caller); otherwise it becomes `failed`. None if not running.
pub async fn fail(
    tx: &mut ScopedTx,
    id: JobId,
    error: &str,
    retry_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<Option<Job>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE jobs SET locked_at = NULL, last_error = $2, updated = $4, \
           status = CASE WHEN $3::timestamptz IS NOT NULL AND attempts < max_attempts \
                         THEN 'queued' ELSE 'failed' END, \
           run_after = COALESCE($3, run_after) \
         WHERE id = $1 AND status = 'running' RETURNING {COLS}"
    )))
    .bind(id)
    .bind(error)
    .bind(retry_at)
    .bind(now)
    .fetch_optional(tx.conn())
    .await?)
}

/// Re-queues jobs whose claim is older than `claimed_before` (worker crashed); returns how many.
pub async fn requeue_stale(
    tx: &mut ScopedTx,
    claimed_before: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<u64> {
    Ok(sqlx::query(
        "UPDATE jobs SET status = 'queued', locked_at = NULL, run_after = $2, updated = $2, \
           last_error = 'claim expired' \
         WHERE status = 'running' AND locked_at < $1",
    )
    .bind(claimed_before)
    .bind(now)
    .execute(tx.conn())
    .await?
    .rows_affected())
}

/// Recomputes the scoped user's `job_wakeups` hint from their queued jobs (call after a claim
/// finds nothing, or after draining). Returns the new earliest `run_after`, if any.
///
/// Race-free with concurrent enqueues: the hint row is locked first, so an enqueue either
/// committed before (and is seen by the recount) or its trigger waits and then lowers the hint.
pub async fn refresh_wakeup(tx: &mut ScopedTx) -> Result<Option<DateTime<Utc>>> {
    let locked: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT run_after FROM job_wakeups WHERE user_id = strata_current_user() FOR UPDATE",
    )
    .fetch_optional(tx.conn())
    .await?;
    let earliest: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT min(run_after) FROM jobs WHERE status = 'queued'")
            .fetch_one(tx.conn())
            .await?;
    match (locked, earliest) {
        (Some(_), Some(at)) => {
            sqlx::query("UPDATE job_wakeups SET run_after = $1 WHERE user_id = strata_current_user()")
                .bind(at)
                .execute(tx.conn())
                .await?;
        }
        (Some(_), None) => {
            sqlx::query("DELETE FROM job_wakeups WHERE user_id = strata_current_user()")
                .execute(tx.conn())
                .await?;
        }
        (None, Some(at)) => {
            sqlx::query(
                "INSERT INTO job_wakeups AS w (user_id, run_after) VALUES (strata_current_user(), $1) \
                 ON CONFLICT (user_id) DO UPDATE SET run_after = LEAST(w.run_after, EXCLUDED.run_after)",
            )
            .bind(at)
            .execute(tx.conn())
            .await?;
        }
        (None, None) => {}
    }
    Ok(earliest)
}
