//! `AiStatus` with the parts only the job side knows (PLAN §7.5 `GET /ai/status`): the
//! user's queue depth and how many of their notes are embedded with the current model
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
    if let Some(e) = ai.embedder() {
        let (embedded, total) = vectors::coverage(&mut tx, e.model_id())
            .await
            .map_err(|e| AiError::Store(e.to_string()))?;
        status.embedding_progress = Some(EmbeddingProgress { embedded, total });
    }
    tx.commit().await?;
    Ok(status)
}
