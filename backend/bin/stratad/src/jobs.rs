//! Background jobs and the API's AI features, wired at `stratad serve` (PLAN §5.2, §9):
//!
//! - the job runner over the standard handlers (`embed`, `embed_backfill`, `summarize`,
//!   `dedupe`), fair across users, publishing `job.completed` / `job.failed` to the event bus;
//! - the scheduler (nightly `dedupe` at `jobs.nightly_hour` in `default_timezone`, checked
//!   hourly) and, with embeddings, one `embed_backfill` per user at start (first import and
//!   model changes);
//! - with embeddings, the semantic level of the duplicate check on create and the idle
//!   unloading of the model;
//! - [`strata_api::ai::AiApi`] for semantic/hybrid search, Ask and `GET /ai/status`.

use std::sync::Arc;
use std::time::Duration;

use strata_api::ai::AiApi;
use strata_api::events::{Event, EventBus};
use strata_common::{Clock, Config, IdGenerator, UserId};
use strata_index::types::UserStatus;
use strata_index::{AccountsDb, AppDb, ScopeIssuer};
use strata_jobs::ask::{AskConfig, AskEngine};
use strata_jobs::retrieval::Retriever;
use strata_jobs::{
    Deps, JobEvents, JobNotice, JobOutcome, Runner, RunnerConfig, Scheduler, UserDirectory,
};
use strata_vault::VaultService;
use tokio::task::JoinHandle;

use crate::ai::AiParts;
use crate::checks::StartupError;

/// Publishes job notices on the per-user event stream.
#[derive(Debug)]
pub struct BusEvents(pub Arc<EventBus>);

impl JobEvents for BusEvents {
    fn job_finished(&self, user: UserId, notice: &JobNotice) {
        let (id, kind, note_id) = (
            notice.id.as_ulid(),
            notice.kind.clone(),
            notice.note_id.map(|n| n.as_ulid()),
        );
        let event = match notice.outcome {
            JobOutcome::Completed => Event::JobCompleted { id, kind, note_id },
            JobOutcome::Failed => Event::JobFailed { id, kind, note_id },
        };
        self.0.publish(user, [event]);
    }
}

/// Active and deletion-pending users (their vaults exist and their jobs run).
#[derive(Debug, Clone)]
pub struct AccountUsers(pub AccountsDb);

#[async_trait::async_trait]
impl UserDirectory for AccountUsers {
    async fn active_users(&self) -> Result<Vec<UserId>, String> {
        let users = self.0.list_users(None).await.map_err(|e| e.to_string())?;
        Ok(users
            .into_iter()
            .filter(|u| matches!(u.status, UserStatus::Active | UserStatus::DeletionPending))
            .map(|u| u.id)
            .collect())
    }
}

/// The running background parts.
#[derive(Debug)]
pub struct Background {
    /// The runner loop.
    pub runner: strata_jobs::RunnerHandle,
    /// The hourly scheduler pass.
    pub scheduler: JoinHandle<()>,
    /// The embedder's idle reaper.
    pub reaper: Option<JoinHandle<()>>,
}

impl Background {
    /// Stops claiming, waits up to `grace` for running jobs, stops the periodic tasks.
    pub async fn shutdown(self, grace: Duration) {
        self.scheduler.abort();
        if let Some(r) = self.reaper {
            r.abort();
        }
        self.runner.shutdown(grace).await;
    }
}

/// The API's AI features over `parts`.
pub fn ai_api(
    parts: &AiParts,
    db: &AppDb,
    vault: &VaultService,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    config: &Config,
) -> Result<AiApi, StartupError> {
    let ai = Arc::new(parts.service.clone());
    let retriever = parts
        .embedder
        .clone()
        .map(|e| Retriever::new(db.clone(), e));
    let tz = config
        .default_tz()
        .map_err(|e| StartupError::Config(e.to_string()))?;
    let ask = AskEngine::new(
        db.clone(),
        vault.clone(),
        ai.clone(),
        retriever.clone(),
        ids.clone(),
        clock.clone(),
        AskConfig {
            default_tz: tz,
            ..AskConfig::default()
        },
    );
    Ok(AiApi::new(ai, db.clone(), retriever, ask, clock, ids))
}

/// Starts the runner, the scheduler and the embedder reaper; registers the semantic
/// duplicate level on the vault store.
#[allow(clippy::too_many_arguments)] // the composition root hands each part over explicitly
pub async fn start(
    config: &Config,
    parts: &AiParts,
    db: &AppDb,
    issuer: &ScopeIssuer,
    accounts: &AccountsDb,
    vault: &VaultService,
    bus: Arc<EventBus>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
) -> Result<Background, StartupError> {
    let thresholds = strata_vault::dup::thresholds(&std::collections::BTreeMap::new());
    if let Some(e) = &parts.embedder {
        vault.set_semantic(Arc::new(strata_jobs::semantic_dup::SemanticDupSource::new(
            e.clone(),
            thresholds.clone(),
        )));
    }
    let deps = Deps {
        db: db.clone(),
        vault: vault.clone(),
        ai: Arc::new(parts.service.clone()),
        embedder: parts.embedder.clone(),
        clock: clock.clone(),
        ids: ids.clone(),
        thresholds,
    };
    let runner = Runner::new(
        db.clone(),
        issuer.clone(),
        clock.clone(),
        Arc::new(BusEvents(bus)),
        RunnerConfig::from_config(config),
        strata_jobs::standard_handlers(&deps),
    );
    let users: Arc<dyn UserDirectory> = Arc::new(AccountUsers(accounts.clone()));
    let active = users.active_users().await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "listing users for the job runner failed");
        Vec::new()
    });
    runner.recover_stale(&active).await;
    let tz = config
        .default_tz()
        .map_err(|e| StartupError::Config(e.to_string()))?;
    let scheduler = Arc::new(Scheduler::new(
        db.clone(),
        issuer.clone(),
        clock,
        ids,
        users,
        tz,
        config.jobs.nightly_hour,
        strata_jobs::standard_periodic(),
    ));
    if parts.embedder.is_some() {
        let n = scheduler
            .enqueue_for_all(strata_jobs::embed::EMBED_BACKFILL, "backfill")
            .await;
        tracing::info!(users = n, "embedding backfill queued");
    }
    let scheduler_task = scheduler.spawn(Duration::from_secs(3600));
    let reaper = parts
        .lazy_embedder
        .as_ref()
        .map(|e| e.spawn_reaper(Duration::from_secs(30)));
    tracing::info!(kinds = ?runner.kinds(), "job runner started");
    Ok(Background {
        runner: runner.start_loop(),
        scheduler: scheduler_task,
        reaper,
    })
}
