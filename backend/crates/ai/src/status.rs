//! `AiStatus`: the snapshot behind `GET /ai/status` (PLAN §7.5, §9.1): provider health, pause
//! state, queue depth, and today's usage against the budget. The API crate maps it to its DTO.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::budget::{BudgetLimits, UsageTotals};
use crate::error::PauseReason;
use crate::provider::ProviderHealth;

/// A pause in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PauseInfo {
    /// Why.
    pub reason: PauseReason,
    /// When work resumes, if known.
    pub until: Option<DateTime<Utc>>,
}

/// The provider serving the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderStatus {
    /// Provider name (`claude_cli`, `anthropic_api`).
    pub name: String,
    /// Configured model.
    pub model: String,
    /// Health.
    pub health: ProviderHealth,
}

/// Today's usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageStatus {
    /// Budget day.
    pub day: NaiveDate,
    /// This user.
    pub user: UsageTotals,
    /// All users.
    pub global: UsageTotals,
}

/// Local embedding model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddingsStatus {
    /// Model ID stored with every vector.
    pub model_id: String,
    /// Vector dimensions.
    pub dims: usize,
    /// Whether the model is in memory now (it loads on demand and unloads when idle, §9.1b).
    pub loaded: bool,
}

/// Progress of embedding the user's notes with the current model (first import, model
/// change: a resumable background backfill, §9.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EmbeddingProgress {
    /// Live notes whose vectors are current (same model, same note version).
    pub embedded: u64,
    /// Live notes.
    pub total: u64,
}

/// Status for one user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiStatus {
    /// Whether AI is enabled for this user.
    pub enabled: bool,
    /// The provider serving this user (`None` when disabled or not configured).
    pub provider: Option<ProviderStatus>,
    /// The pause in effect: a budget pause first, else the provider's usage-limit pause.
    pub paused: Option<PauseInfo>,
    /// Queued AI jobs for this user; filled in by the job runner (`None` until it exists).
    pub queue_depth: Option<u64>,
    /// Failed jobs a retry would run again; filled in by the job runner (`None` until it
    /// exists).
    pub failed_jobs: Option<u64>,
    /// Today's usage.
    pub usage: UsageStatus,
    /// Budget caps (0 = unlimited).
    pub limits: BudgetLimits,
    /// The embedding model, when one is configured.
    pub embeddings: Option<EmbeddingsStatus>,
    /// Embedding coverage of the user's notes; filled in by the job runner (`None` until it
    /// exists or without an embedding model).
    pub embedding_progress: Option<EmbeddingProgress>,
}
