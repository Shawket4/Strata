//! `AiStatus` with the parts only the job side knows (PLAN §7.5 `GET /ai/status`): the
//! user's queue depth, failed jobs a retry would run again, and how many of their notes are embedded with the current model
//! (backfill progress, §9.1b).

use strata_ai::{AiCaller, AiError, AiService, AiStatus, EmbeddingProgress};
use strata_index::AppDb;

use crate::repo;
use crate::vectors;

/// The status of `caller`, including queue depth and embedding progress.
pub async fn ai_status(ai: &AiService, db: &AppDb, caller: &AiCaller) -> Result<AiStatus, AiError> {
    let mut status = ai.status(caller).await?;
    let mut tx = db.begin(&caller.scope).await?;
    status.queue_depth = Some(
        repo::queue_depth(&mut tx)
            .await
            .map_err(|e| AiError::Store(e.to_string()))?,
    );
    status.failed_jobs = Some(
        strata_index::repo::jobs::retryable_failed(&mut tx, false)
            .await
            .map_err(|e| AiError::Store(e.to_string()))?,
    );
    if let Some(e) = ai.embedder() {
        let (embedded, total) = vectors::coverage(&mut tx, e.model_id())
            .await
            .map_err(|e| AiError::Store(e.to_string()))?;
        status.embedding_progress = Some(EmbeddingProgress { embedded, total });
    }
    tx.commit().await?;
    Ok(status)
}

/// Queues the caller's failed jobs again (the app's "Retry failed jobs", owner decision
/// 2026-09-28); returns how many.
pub async fn retry_failed(
    db: &AppDb,
    caller: &AiCaller,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<u64, AiError> {
    let mut tx = db.begin(&caller.scope).await?;
    let n = strata_index::repo::jobs::requeue_failed(&mut tx, false, None, now)
        .await
        .map_err(|e| AiError::Store(e.to_string()))?;
    tx.commit().await?;
    Ok(n)
}
