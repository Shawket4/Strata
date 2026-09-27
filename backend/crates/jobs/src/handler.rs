//! What a job kind implements, and how a run ends.

use chrono::{DateTime, Utc};
use strata_ai::{AiError, PauseReason};
use strata_index::UserScope;
use strata_index::repo::jobs::Job;

/// The resource a job kind uses; the runner limits each class separately (PLAN §9.1, §9.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobClass {
    /// Local embeddings: one at a time, low priority, never during a `claude -p` call.
    Embed,
    /// LLM calls: subject to budgets, provider pauses and the daily job limit.
    Llm,
    /// Database/vault work only.
    Light,
}

/// Everything a handler gets for one run.
#[derive(Debug, Clone)]
pub struct JobContext {
    /// The claimed row (`attempts` already counts this run).
    pub job: Job,
    /// The job owner's scope (every query and vault write runs in it, principle 7).
    pub scope: UserScope,
    /// The owner's username (provider routing, D23).
    pub username: String,
    /// When the run started (injected clock).
    pub now: DateTime<Utc>,
}

/// Why a run did not complete.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum JobError {
    /// Wait and resume, never fail (budget reached, provider usage limit, PLAN §9.1). The job
    /// goes back to the queue for `until` (or the runner's default pause) without spending
    /// an attempt.
    #[error("paused ({reason:?}) until {until:?}")]
    Paused {
        /// Why.
        reason: PauseReason,
        /// When work may resume, if known.
        until: Option<DateTime<Utc>>,
    },
    /// A transient failure: retried with exponential backoff until attempts run out.
    #[error("retryable failure: {0}")]
    Retry(String),
    /// A permanent failure: the job fails now.
    #[error("permanent failure: {0}")]
    Fatal(String),
}

impl From<AiError> for JobError {
    /// Pauses wait; invalid output and provider trouble are retried; a refusal, an auth
    /// problem or a broken prompt fails at once. Messages are content-free (PLAN §15).
    fn from(e: AiError) -> Self {
        use strata_ai::ProviderError as P;
        match e {
            AiError::Paused { reason, until } => Self::Paused { reason, until },
            AiError::Provider(P::Auth | P::Rejected(_) | P::Refused)
            | AiError::UnknownPrompt(_)
            | AiError::InvalidSchema(_)
            | AiError::Config(_) => Self::Fatal(e.to_string()),
            other => Self::Retry(other.to_string()),
        }
    }
}

impl From<strata_index::IndexError> for JobError {
    fn from(e: strata_index::IndexError) -> Self {
        Self::Retry(format!("database: {e}"))
    }
}

impl From<strata_vault::VaultError> for JobError {
    fn from(e: strata_vault::VaultError) -> Self {
        Self::Retry(format!("vault: {e}"))
    }
}

impl From<sqlx::Error> for JobError {
    fn from(e: sqlx::Error) -> Self {
        Self::Retry(format!("database: {e}"))
    }
}

/// One job kind (`embed`, `summarize`, `dedupe`, …).
#[async_trait::async_trait]
pub trait JobHandler: Send + Sync + std::fmt::Debug {
    /// The `jobs.kind` it runs.
    fn kind(&self) -> &'static str;

    /// Its resource class.
    fn class(&self) -> JobClass;

    /// Jobs of this kind running at once across all users.
    fn max_concurrency(&self) -> usize {
        1
    }

    /// Runs one job.
    async fn run(&self, ctx: JobContext) -> Result<(), JobError>;
}

#[cfg(test)]
mod tests {
    use strata_ai::ProviderError;

    use super::*;

    #[test]
    fn ai_errors_map_to_pause_retry_or_failure() {
        let until = "2026-09-28T00:00:00Z".parse().ok();
        assert_eq!(
            JobError::from(AiError::Paused {
                reason: PauseReason::UserBudget,
                until
            }),
            JobError::Paused {
                reason: PauseReason::UserBudget,
                until
            }
        );
        assert_eq!(
            JobError::from(AiError::Provider(ProviderError::Auth)),
            JobError::Fatal("provider authentication failed".into())
        );
        assert_eq!(
            JobError::from(AiError::Provider(ProviderError::Unavailable("x".into()))),
            JobError::Retry("provider unavailable: x".into())
        );
        assert!(matches!(
            JobError::from(AiError::InvalidOutput {
                prompt_id: "summary".into(),
                version: 1,
                attempts: 3,
                errors: vec![]
            }),
            JobError::Retry(_)
        ));
    }
}
