//! Strata background jobs and AI pipelines (PLAN §5.2, §9).
//!
//! - [`runner`]: the fair job runner (round-robin across users over `job_wakeups`, claims in
//!   each user's scope with `SKIP LOCKED`, global and per-kind limits, retries with
//!   exponential backoff, pauses that wait instead of failing, graceful shutdown).
//! - [`schedule`]: periodic kinds (the nightly `dedupe` sweep) and one-off fan-out (the
//!   embedding backfill).
//! - Jobs: [`embed`] (`embed`, `embed_backfill`), [`summarize`], [`dedupe_sweep`] (`dedupe`),
//!   and the AI pipelines: [`link`] (`link`), [`file_inbox`] (`file_inbox`), [`insights`]
//!   (`entity_insights`, `entity_insights_sweep`), [`correct`] (`correct`,
//!   `suggestion_reply`), [`digest`] (`digest`); shared planning in [`pipeline`], event dates
//!   in [`dates`], configured thresholds in [`thresholds`].
//! - Retrieval: [`retrieval`] (semantic and hybrid search, similarity edges), [`ask`] (RAG with
//!   citations), [`semantic_dup`] (the semantic level of the duplicate check on create).
//! - [`status`]: `AiStatus` with queue depth and embedding progress.
//!
//! Design notes are in `docs/ARCHITECTURE.md`, section "Jobs and AI pipelines".

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(test, allow(clippy::expect_used, clippy::float_cmp))]

pub mod ask;
pub mod chunk;
pub mod correct;
pub mod dates;
pub mod dedupe_sweep;
pub mod digest;
pub mod embed;
pub mod events;
pub mod file_inbox;
pub mod handler;
pub mod insights;
pub mod link;
pub mod pipeline;
pub mod repo;
pub mod retrieval;
pub mod runner;
pub mod schedule;
pub mod semantic_dup;
pub mod status;
pub mod summarize;
pub mod thresholds;
pub mod vectors;

use std::sync::Arc;

use chrono::Weekday;
use chrono_tz::Tz;
use strata_ai::{AiService, Embedder};
use strata_common::{Clock, IdGenerator};
use strata_index::AppDb;
use strata_vault::VaultService;

pub use events::{JobEvents, JobNotice, JobOutcome, NoEvents, RecordedEvents};
pub use handler::{JobClass, JobContext, JobError, JobHandler};
pub use runner::{Runner, RunnerConfig, RunnerHandle};
pub use schedule::{Cadence, Periodic, Scheduler, StaticUsers, UserDirectory};

/// What the standard job handlers need.
#[derive(Debug, Clone)]
pub struct Deps {
    /// The `strata_app` database.
    pub db: AppDb,
    /// The vault store (reads and `ai:` commits).
    pub vault: VaultService,
    /// The AI service (routing, budgets, validation).
    pub ai: Arc<AiService>,
    /// The embedder, when embeddings are configured.
    pub embedder: Option<Arc<dyn Embedder>>,
    /// Clock.
    pub clock: Arc<dyn Clock>,
    /// ID generator.
    pub ids: Arc<dyn IdGenerator>,
    /// Duplicate thresholds.
    pub thresholds: dedupe::Thresholds,
    /// Confidence thresholds of the AI pipelines.
    pub ai_thresholds: thresholds::AiThresholds,
    /// Time zone of users without a `timezone` setting (digest weeks).
    pub default_tz: Tz,
}

impl Deps {
    /// What the linking and filing jobs need.
    pub fn pipeline(&self) -> link::PipelineDeps {
        link::PipelineDeps {
            db: self.db.clone(),
            vault: self.vault.clone(),
            ai: self.ai.clone(),
            embedder: self.embedder.clone(),
            ids: self.ids.clone(),
            clock: self.clock.clone(),
            thresholds: self.ai_thresholds,
        }
    }

    /// What the correction jobs need.
    pub fn correct(&self) -> correct::CorrectDeps {
        correct::CorrectDeps {
            db: self.db.clone(),
            vault: self.vault.clone(),
            ai: self.ai.clone(),
            ids: self.ids.clone(),
            clock: self.clock.clone(),
            thresholds: self.ai_thresholds,
        }
    }
}

/// The job kinds this build runs: `embed`, `embed_backfill`, `summarize`, `dedupe`, `link`,
/// `file_inbox`, `entity_insights`, `entity_insights_sweep`, `correct`, `suggestion_reply`,
/// `digest`.
pub fn standard_handlers(d: &Deps) -> Vec<Arc<dyn JobHandler>> {
    vec![
        Arc::new(
            embed::EmbedHandler::new(
                d.db.clone(),
                d.vault.clone(),
                d.embedder.clone(),
                d.clock.clone(),
                d.ids.clone(),
                true,
            )
            .with_link_delay(pipeline::LINK_DEBOUNCE),
        ),
        Arc::new(embed::BackfillHandler::new(
            d.db.clone(),
            d.embedder.clone(),
            d.clock.clone(),
            d.ids.clone(),
            25,
        )),
        Arc::new(summarize::SummarizeHandler::new(
            d.vault.clone(),
            d.ai.clone(),
        )),
        Arc::new(dedupe_sweep::DedupeHandler::new(
            d.db.clone(),
            d.vault.clone(),
            d.ai.clone(),
            d.embedder.clone(),
            d.thresholds.clone(),
            d.ids.clone(),
            20,
        )),
        Arc::new(link::LinkHandler::new(d.pipeline())),
        Arc::new(file_inbox::FileInboxHandler::new(d.pipeline())),
        Arc::new(insights::InsightsHandler::new(
            d.db.clone(),
            d.vault.clone(),
            d.ai.clone(),
        )),
        Arc::new(insights::SweepHandler::new(
            d.db.clone(),
            d.ids.clone(),
            d.clock.clone(),
        )),
        Arc::new(correct::CorrectHandler::new(d.correct())),
        Arc::new(correct::ReplyHandler::new(d.correct())),
        Arc::new(digest::DigestHandler::new(
            d.db.clone(),
            d.vault.clone(),
            d.ai.clone(),
            d.clock.clone(),
            d.default_tz,
        )),
    ]
}

/// The periodic kinds with the weekly digest on Mondays (see [`periodic`]).
pub fn standard_periodic() -> Vec<Periodic> {
    periodic(Weekday::Mon)
}

/// The periodic kinds: the nightly semantic duplicate sweep, the nightly entity-insights
/// sweep, and the weekly digest on `digest_day` (all at `jobs.nightly_hour`).
pub fn periodic(digest_day: Weekday) -> Vec<Periodic> {
    vec![
        Periodic {
            kind: dedupe_sweep::DEDUPE,
            cadence: Cadence::Nightly,
        },
        Periodic {
            kind: insights::SWEEP,
            cadence: Cadence::Nightly,
        },
        Periodic {
            kind: digest::DIGEST,
            cadence: Cadence::Weekly(digest_day),
        },
    ]
}
