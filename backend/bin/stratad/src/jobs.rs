//! Background jobs and the API's AI features, wired at `stratad serve` (PLAN §5.2, §9):
//!
//! - the job runner over the standard handlers (`embed`, `embed_backfill`, `summarize`,
//!   `dedupe`, and the AI pipelines `link`, `file_inbox`, `entity_insights`,
//!   `entity_insights_sweep`, `correct`, `suggestion_reply`, `digest` with the thresholds of
//!   `[thresholds]`), fair across users, publishing `job.started` (interactive kinds),
//!   `job.completed` and `job.failed` to the event bus, and woken by every vault commit so a
//!   capture or a reply is picked up at once instead of at the next poll;
//! - the scheduler (nightly `dedupe` and `entity_insights_sweep` at `jobs.nightly_hour` in
//!   `default_timezone`, the weekly `digest` on `jobs.digest_weekday`, checked hourly) and, with
//!   embeddings, one `embed_backfill` per user at start (first import and model changes);
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
    Deps, JobEvents, JobNotice, JobOutcome, JobStart, Runner, RunnerConfig, Scheduler,
    UserDirectory,
};
use strata_vault::{CommitListener, Committed, VaultService};
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

    fn job_started(&self, user: UserId, start: &JobStart) {
        self.0.publish(
            user,
            [Event::JobStarted {
                id: start.id.as_ulid(),
                kind: start.kind.clone(),
                note_id: start.note_id.map(|n| n.as_ulid()),
            }],
        );
    }
}

/// The vault's commit listener: the event bus first, then a wake-up for the job runner (the
/// commit may have queued a capture's filing or a reply's answer).
#[derive(Debug)]
pub struct BusAndRunner {
    /// The event bus.
    pub bus: Arc<EventBus>,
    /// The job runner.
    pub runner: Runner,
}

impl CommitListener for BusAndRunner {
    fn committed(&self, user: UserId, notice: &Committed) {
        self.bus.committed(user, notice);
        self.runner.wake();
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

/// The standard handlers plus the graph's `cluster` job (publishing `cluster.updated`).
pub fn handlers(deps: &Deps, bus: Arc<EventBus>) -> Vec<Arc<dyn strata_jobs::JobHandler>> {
    let mut handlers = strata_jobs::standard_handlers(deps);
    handlers.push(Arc::new(strata_graph::cluster::ClusterHandler::new(
        deps.db.clone(),
        deps.vault.clone(),
        deps.ai.clone(),
        Arc::new(strata_api::graph::BusClusterEvents(bus)),
        strata_graph::cluster::ClusterConfig::default(),
    )));
    handlers
}

/// The standard periodic kinds plus the nightly `cluster` run (§9.2), with the digest on
/// Mondays (see [`periodic_for`]).
pub fn periodic() -> Vec<strata_jobs::Periodic> {
    periodic_for(chrono::Weekday::Mon)
}

/// The periodic kinds with the weekly digest on `digest_day` (`jobs.digest_weekday`).
pub fn periodic_for(digest_day: chrono::Weekday) -> Vec<strata_jobs::Periodic> {
    let mut periodic = strata_jobs::periodic(digest_day);
    periodic.push(strata_jobs::Periodic {
        kind: strata_graph::cluster::CLUSTER,
        cadence: strata_jobs::Cadence::Nightly,
    });
    periodic
}

/// The graph endpoints' dependencies: graph reads (with similarity edges when an embedding
/// model is configured), the clock and IDs for the recluster trigger.
pub fn graph_api(
    parts: &AiParts,
    db: &AppDb,
    vault: &VaultService,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
) -> strata_api::graph::GraphApi {
    let similarity = parts.embedder.as_ref().map(|e| {
        Arc::new(strata_graph::similarity::NoteVectorSimilarity::new(
            db.clone(),
            e.model_id(),
            strata_graph::similarity::SimilarityConfig::default(),
        )) as Arc<dyn strata_graph::similarity::SimilaritySource>
    });
    strata_api::graph::GraphApi::new(
        strata_graph::GraphService::new(db.clone(), vault.clone(), similarity),
        clock,
        ids,
    )
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
    // Semantic duplicate thresholds per kind from `thresholds.dedupe` (§9.7).
    let thresholds = strata_jobs::thresholds::dedupe_thresholds(&config.thresholds.dedupe);
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
        ai_thresholds: strata_jobs::thresholds::AiThresholds::from(&config.thresholds),
        default_tz: config
            .default_tz()
            .map_err(|e| StartupError::Config(e.to_string()))?,
    };
    let runner = Runner::new(
        db.clone(),
        issuer.clone(),
        clock.clone(),
        Arc::new(BusEvents(bus.clone())),
        RunnerConfig::from_config(config),
        handlers(&deps, bus.clone()),
    );
    vault.set_listener(Arc::new(BusAndRunner {
        bus,
        runner: runner.clone(),
    }));
    let users: Arc<dyn UserDirectory> = Arc::new(AccountUsers(accounts.clone()));
    let active = users.active_users().await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "listing users for the job runner failed");
        Vec::new()
    });
    runner.recover_stale(&active).await;
    let tz = config
        .default_tz()
        .map_err(|e| StartupError::Config(e.to_string()))?;
    let scheduler = Arc::new(
        Scheduler::new(
            db.clone(),
            issuer.clone(),
            clock,
            ids,
            users,
            tz,
            config.jobs.nightly_hour,
            periodic_for(config.jobs.digest_day()),
        )
        .with_recovery(runner.recovery()),
    );
    if parts.embedder.is_some() {
        let n = scheduler
            .enqueue_for_all(strata_jobs::embed::EMBED_BACKFILL, "backfill")
            .await;
        tracing::info!(users = n, "embedding backfill queued");
    }
    let scheduler_task = scheduler.spawn(Duration::from_secs(3600));
    // Kept loaded: load now, so the first search or embedding does not wait for it.
    // Otherwise the reaper unloads it after its idle period.
    let reaper = parts.lazy_embedder.as_ref().map(|e| {
        if e.keeps_loaded() {
            let e = e.clone();
            tokio::spawn(async move {
                if let Err(err) = e.preload().await {
                    tracing::warn!(error = %err, "loading the embedding model at start failed; it loads on first use");
                }
            })
        } else {
            e.spawn_reaper(Duration::from_secs(30))
        }
    });
    tracing::info!(kinds = ?runner.kinds(), "job runner started");
    Ok(Background {
        runner: runner.start_loop(),
        scheduler: scheduler_task,
        reaper,
    })
}
