//! Queue queries the runner needs beyond `strata_index::repo::jobs`: claiming only the kinds
//! this process runs (and has capacity for), putting a paused job back without spending an
//! attempt, and queue statistics. Every function takes a [`ScopedTx`] (principle 7).

use chrono::{DateTime, Utc};
use strata_index::ScopedTx;
use strata_index::repo::jobs::{Job, NewJob};

use crate::handler::JobError;

const COLS: &str = "id, kind, note_id, payload, status, attempts, max_attempts, run_after, \
    locked_at, last_error, dedupe_key, created, updated";

/// Claims the scoped user's next runnable job among `kinds` (those of `first` before the
/// rest, then earliest `run_after`, then ID), skipping rows other claimers hold
/// (`FOR UPDATE SKIP LOCKED`, PLAN §9.2).
pub async fn claim_of_kinds(
    tx: &mut ScopedTx,
    now: DateTime<Utc>,
    kinds: &[String],
    first: &[String],
) -> Result<Option<Job>, JobError> {
    if kinds.is_empty() {
        return Ok(None);
    }
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, locked_at = $1, updated = $1 \
         WHERE (user_id, id) = ( \
           SELECT user_id, id FROM jobs WHERE status = 'queued' AND run_after <= $1 \
             AND kind = ANY($2) \
           ORDER BY kind = ANY($3) DESC, run_after, id FOR UPDATE SKIP LOCKED LIMIT 1) \
         RETURNING {COLS}"
    )))
    .bind(now)
    .bind(kinds)
    .bind(first)
    .fetch_optional(tx.conn())
    .await?)
}

/// Whether a queued job of `kinds` is due (runnable if capacity allowed).
pub async fn any_due(
    tx: &mut ScopedTx,
    now: DateTime<Utc>,
    kinds: &[String],
) -> Result<bool, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM jobs WHERE status = 'queued' AND run_after <= $1 \
         AND kind = ANY($2))",
    )
    .bind(now)
    .bind(kinds)
    .fetch_one(tx.conn())
    .await?)
}

/// The earliest `run_after` among queued jobs of `kinds`.
pub async fn next_due(
    tx: &mut ScopedTx,
    kinds: &[String],
) -> Result<Option<DateTime<Utc>>, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT min(run_after) FROM jobs WHERE status = 'queued' AND kind = ANY($1)",
    )
    .bind(kinds)
    .fetch_one(tx.conn())
    .await?)
}

/// Puts a running job back in the queue for `run_after` without spending the attempt its
/// claim counted (a pause is not a failure, PLAN §9.1). False if it is not running.
pub async fn release_paused(
    tx: &mut ScopedTx,
    id: strata_common::JobId,
    run_after: DateTime<Utc>,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<bool, JobError> {
    let done = sqlx::query(
        "UPDATE jobs SET status = 'queued', attempts = GREATEST(attempts - 1, 0), \
           locked_at = NULL, run_after = $2, last_error = $3, updated = $4 \
         WHERE id = $1 AND status = 'running'",
    )
    .bind(id)
    .bind(run_after)
    .bind(reason)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Queued and running jobs of the scoped user (`AiStatus.queue_depth`).
pub async fn queue_depth(tx: &mut ScopedTx) -> Result<u64, JobError> {
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE status IN ('queued', 'running')")
            .fetch_one(tx.conn())
            .await?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// Queued or running jobs of `kind` of the scoped user.
pub async fn pending_of_kind(tx: &mut ScopedTx, kind: &str) -> Result<u64, JobError> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = $1 AND status IN ('queued', 'running')",
    )
    .bind(kind)
    .fetch_one(tx.conn())
    .await?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// The scoped user's username (`users` is global; `strata_app` may read this column).
pub async fn username(tx: &mut ScopedTx) -> Result<Option<String>, JobError> {
    Ok(
        sqlx::query_scalar("SELECT username FROM users WHERE id = strata_current_user()")
            .fetch_optional(tx.conn())
            .await?,
    )
}

/// Enqueues `job` (see `strata_index::repo::jobs::enqueue`: a queued job with the same kind
/// and debounce key is re-timed instead).
pub async fn enqueue(tx: &mut ScopedTx, job: &NewJob, now: DateTime<Utc>) -> Result<Job, JobError> {
    Ok(strata_index::repo::jobs::enqueue(tx, job, now).await?)
}

/// Enqueues `kind` for `note` debounced by the note ID, runnable at `run_after`.
pub async fn enqueue_for_note(
    tx: &mut ScopedTx,
    ids: &dyn strata_common::IdGenerator,
    kind: &str,
    note: strata_common::NoteId,
    run_after: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Job, JobError> {
    enqueue(
        tx,
        &NewJob {
            id: strata_common::JobId::generate(ids),
            kind: kind.to_owned(),
            note_id: Some(note),
            payload: Vec::new(),
            run_after,
            max_attempts: 5,
            dedupe_key: Some(note.to_string()),
        },
        now,
    )
    .await
}

/// Whether a queued or running job of `kind` exists for the scoped user.
pub async fn exists_pending(tx: &mut ScopedTx, kind: &str) -> Result<bool, JobError> {
    Ok(pending_of_kind(tx, kind).await? > 0)
}
