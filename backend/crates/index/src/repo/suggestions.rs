//! Suggestions with threaded replies, and the AI decision log (§9.8).

use chrono::{DateTime, Utc};
use strata_common::{DecisionId, JobId, NoteId, ReplyId, SuggestionId};

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{DecisionKind, ReplyAuthor, SuggestionStatus};

/// A `suggestions` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Suggestion {
    /// ID.
    pub id: SuggestionId,
    /// Note it concerns.
    pub note_id: Option<NoteId>,
    /// Kind (`filing`, `entity_link`, `custody`, `task`, `duplicate`, …).
    pub kind: String,
    /// `MessagePack` payload.
    pub payload: Vec<u8>,
    /// Status.
    pub status: SuggestionStatus,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Last change.
    pub updated: DateTime<Utc>,
    /// Accept/reject time.
    pub decided_at: Option<DateTime<Utc>>,
}

/// A `suggestion_replies` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Reply {
    /// ID.
    pub id: ReplyId,
    /// Thread.
    pub suggestion_id: SuggestionId,
    /// Who wrote it.
    pub author: ReplyAuthor,
    /// Text.
    pub body: String,
    /// Created at.
    pub created: DateTime<Utc>,
}

/// An `ai_decisions` row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Decision {
    /// ID.
    pub id: DecisionId,
    /// Kind.
    pub kind: DecisionKind,
    /// Source note.
    pub source_note_id: Option<NoteId>,
    /// Source block.
    pub source_block_id: Option<String>,
    /// Target type (`entity`, `note`, `document`, `task`, …).
    pub target_type: String,
    /// Target ID.
    pub target_id: String,
    /// One-line description for the activity feed and correction prompts.
    pub summary: String,
    /// Confidence.
    pub confidence: Option<f32>,
    /// Producing job.
    pub job_id: Option<JobId>,
    /// Resulting suggestion, when not applied automatically.
    pub suggestion_id: Option<SuggestionId>,
    /// The `ai:` commit that applied it.
    pub git_commit: Option<String>,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Revert time.
    pub reverted_at: Option<DateTime<Utc>>,
}

const COLS: &str = "id, note_id, kind, payload, status, created, updated, decided_at";

/// Creates a pending suggestion.
pub async fn create_suggestion(
    tx: &mut ScopedTx,
    id: SuggestionId,
    note_id: Option<NoteId>,
    kind: &str,
    payload: &[u8],
    now: DateTime<Utc>,
) -> Result<Suggestion> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO suggestions (user_id, id, note_id, kind, payload, status, created, updated) \
         VALUES (strata_current_user(), $1, $2, $3, $4, 'pending', $5, $5) RETURNING {COLS}"
    )))
    .bind(id)
    .bind(note_id)
    .bind(kind)
    .bind(payload)
    .bind(now)
    .fetch_one(tx.conn())
    .await?)
}

/// A suggestion by ID.
pub async fn get_suggestion(tx: &mut ScopedTx, id: SuggestionId) -> Result<Option<Suggestion>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLS} FROM suggestions WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Suggestions with `status`, oldest first.
pub async fn list_suggestions(
    tx: &mut ScopedTx,
    status: SuggestionStatus,
) -> Result<Vec<Suggestion>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLS} FROM suggestions WHERE status = $1 ORDER BY created, id"
    )))
    .bind(status)
    .fetch_all(tx.conn())
    .await?)
}

/// Moves a *pending* suggestion to `status` (accepted, rejected, superseded). None if missing
/// or already decided.
pub async fn decide_suggestion(
    tx: &mut ScopedTx,
    id: SuggestionId,
    status: SuggestionStatus,
    now: DateTime<Utc>,
) -> Result<Option<Suggestion>> {
    if status == SuggestionStatus::Pending {
        return Err(crate::error::IndexError::InvalidArgument(
            "a suggestion cannot be decided as pending".into(),
        ));
    }
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE suggestions SET status = $2, updated = $3, \
           decided_at = CASE WHEN $2 IN ('accepted', 'rejected') THEN $3 END \
         WHERE id = $1 AND status = 'pending' RETURNING {COLS}"
    )))
    .bind(id)
    .bind(status)
    .bind(now)
    .fetch_optional(tx.conn())
    .await?)
}

/// Adds a reply to a suggestion thread.
pub async fn add_reply(tx: &mut ScopedTx, reply: &Reply) -> Result<()> {
    sqlx::query(
        "INSERT INTO suggestion_replies (user_id, suggestion_id, id, author, body, created) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5)",
    )
    .bind(reply.suggestion_id)
    .bind(reply.id)
    .bind(reply.author)
    .bind(&reply.body)
    .bind(reply.created)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A thread's replies in order.
pub async fn replies(tx: &mut ScopedTx, suggestion: SuggestionId) -> Result<Vec<Reply>> {
    Ok(sqlx::query_as(
        "SELECT id, suggestion_id, author, body, created FROM suggestion_replies \
         WHERE suggestion_id = $1 ORDER BY created, id",
    )
    .bind(suggestion)
    .fetch_all(tx.conn())
    .await?)
}

/// Records an AI decision.
pub async fn record_decision(tx: &mut ScopedTx, d: &Decision) -> Result<()> {
    sqlx::query(
        "INSERT INTO ai_decisions (user_id, id, kind, source_note_id, source_block_id, target_type, \
           target_id, summary, confidence, job_id, suggestion_id, git_commit, created, reverted_at) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(d.id)
    .bind(d.kind)
    .bind(d.source_note_id)
    .bind(&d.source_block_id)
    .bind(&d.target_type)
    .bind(&d.target_id)
    .bind(&d.summary)
    .bind(d.confidence)
    .bind(d.job_id)
    .bind(d.suggestion_id)
    .bind(&d.git_commit)
    .bind(d.created)
    .bind(d.reverted_at)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// The most recent `limit` decisions, newest first (correction-loop context).
pub async fn recent_decisions(tx: &mut ScopedTx, limit: i64) -> Result<Vec<Decision>> {
    Ok(sqlx::query_as(
        "SELECT id, kind, source_note_id, source_block_id, target_type, target_id, summary, \
           confidence, job_id, suggestion_id, git_commit, created, reverted_at \
         FROM ai_decisions ORDER BY created DESC, id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

/// Marks a decision reverted. False if missing or already reverted.
pub async fn mark_decision_reverted(
    tx: &mut ScopedTx,
    id: DecisionId,
    now: DateTime<Utc>,
) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE ai_decisions SET reverted_at = $2 WHERE id = $1 AND reverted_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}
