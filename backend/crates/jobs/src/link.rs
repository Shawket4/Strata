//! The `link` job (PLAN §9.2, §9.4): debounced 30 s after the last edit of a note (enqueued by
//! the `embed` job), skipped when the note's version equals its sidecar's `last_linked_hash`.
//! One `linking` call returns relations, concepts, entity mentions, entity relations, custody
//! statements and task suggestions; [`crate::pipeline::Planner`] turns them into one change
//! set, written as one `ai: link <path>` commit.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use strata_ai::outputs::Linking;
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService, Embedder};
use strata_common::{Clock, IdGenerator, NoteId};
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai_apply::AiApplied;

use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::pipeline::{
    self, BlockInput, CandidateInput, ConceptInput, Directory, EntityInput, Extraction,
    PendingKeys, Planner, RejectedInput, SourceNote,
};
use crate::thresholds::AiThresholds;

/// Output token limit of one linking call.
const MAX_TOKENS: u32 = 4000;

/// Parameters of a `link` job (`MessagePack`; an empty payload is the default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkParams {
    /// Run even if the note was linked at this version (`POST /notes/{id}/relink`).
    #[serde(default)]
    pub force: bool,
}

impl LinkParams {
    /// Decodes a job payload (empty = default).
    pub fn decode(payload: &[u8]) -> Self {
        if payload.is_empty() {
            return Self::default();
        }
        rmp_serde::from_slice(payload).unwrap_or_default()
    }
}

/// The note in a `linking` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkNote {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// `created` (RFC 3339, user's time zone).
    pub created: String,
    /// Blocks.
    pub blocks: Vec<BlockInput>,
}

/// The `linking` prompt input (field order is the prompt's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkInput {
    /// The note.
    pub note: LinkNote,
    /// Candidate notes.
    pub candidates: Vec<CandidateInput>,
    /// Concepts.
    pub concepts: Vec<ConceptInput>,
    /// Entities.
    pub entities: Vec<EntityInput>,
    /// Rejections.
    pub rejected: Vec<RejectedInput>,
}

/// Everything the linking and filing jobs need.
#[derive(Debug, Clone)]
pub struct PipelineDeps {
    /// Database.
    pub db: AppDb,
    /// Vault.
    pub vault: VaultService,
    /// AI.
    pub ai: Arc<AiService>,
    /// Embedder (candidate notes and entities by similarity).
    pub embedder: Option<Arc<dyn Embedder>>,
    /// IDs.
    pub ids: Arc<dyn IdGenerator>,
    /// Clock.
    pub clock: Arc<dyn Clock>,
    /// Thresholds.
    pub thresholds: AiThresholds,
}

impl PipelineDeps {
    /// The embedding model ID (vectors of other models are ignored).
    pub fn model(&self) -> Option<String> {
        self.embedder.as_ref().map(|e| e.model_id().to_owned())
    }

    /// The prompt context for `note`: candidates, directory, offered entities.
    pub async fn context(
        &self,
        scope: &UserScope,
        note: &SourceNote,
    ) -> Result<(Vec<CandidateInput>, Directory, Vec<EntityInput>), JobError> {
        let model = self.model();
        let candidates =
            pipeline::candidates(&self.db, &self.vault, scope, model.as_deref(), note).await?;
        let dir = Directory::load(&self.db, scope).await?;
        let extra = pipeline::linked_entities(&self.db, scope, note.id, model.as_deref()).await?;
        let text = format!("{}\n{}", note.title, note.body());
        let offered = dir.entity_inputs(&dir.select(&text, &extra));
        Ok((candidates, dir, offered))
    }

    /// Plans the change set of `extraction` for `note`.
    #[allow(clippy::too_many_arguments)] // the plan's inputs are explicit
    pub async fn plan(
        &self,
        ctx: &JobContext,
        job: &str,
        note: &SourceNote,
        dir: &Directory,
        candidates: &[CandidateInput],
        offered: &[EntityInput],
        extraction: &Extraction,
        model: String,
    ) -> Result<strata_vault::ops::ai_apply::AiChangeSet, JobError> {
        let events = pipeline::document_events(&self.vault, &ctx.scope, dir).await?;
        let pending =
            PendingKeys::from_views(&self.vault.note_suggestions(&ctx.scope, note.id).await?);
        let linked_before = pipeline::linked_mentions(note, dir);
        let mut p = Planner::new(
            job,
            Some(ctx.job.id),
            note,
            dir,
            candidates,
            offered,
            self.thresholds,
            model,
            self.ids.as_ref(),
            self.clock.now(),
            &events,
            pending,
        );
        let titles = pipeline::candidate_titles(candidates);
        p.relations(
            &self.db,
            &ctx.scope,
            &self.vault,
            &extraction.relations,
            &titles,
        )
        .await?;
        p.concepts(&extraction.concepts);
        p.mentions(&extraction.mentions);
        p.entity_relations(&extraction.entity_relations);
        p.custody(
            &extraction.custody,
            &pipeline::nickname_set(&extraction.mentions),
        );
        p.tasks(&extraction.tasks);
        p.stale();
        Ok(p.finish(&linked_before))
    }
}

/// The `link` job.
#[derive(Debug, Clone)]
pub struct LinkHandler {
    deps: PipelineDeps,
}

impl LinkHandler {
    /// The handler.
    pub fn new(deps: PipelineDeps) -> Self {
        Self { deps }
    }
}

/// Whether `path` is an inbox capture.
pub fn in_inbox(path: &str) -> bool {
    item_render::paths::is_inbox_path(path)
}

#[async_trait::async_trait]
impl JobHandler for LinkHandler {
    fn kind(&self) -> &'static str {
        pipeline::LINK
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(id) = ctx.job.note_id else {
            return Err(JobError::Fatal("link job without a note".into()));
        };
        let params = LinkParams::decode(&ctx.job.payload);
        let d = &self.deps;
        let Some(note) = SourceNote::load(&d.vault, &ctx.scope, id).await? else {
            return Ok(());
        };
        if note.kind != domain::NoteKind::Note {
            return Ok(());
        }
        // Captures are linked by their `file_inbox` job (one call, not two).
        if in_inbox(&note.path) {
            return Ok(());
        }
        if !params.force && note.sidecar.last_linked_hash.as_deref() == Some(&note.version) {
            return Ok(());
        }
        let (candidates, dir, offered) = d.context(&ctx.scope, &note).await?;
        let input = LinkInput {
            note: LinkNote {
                id: id.to_string(),
                title: note.title.clone(),
                created: note.created_rfc3339(),
                blocks: note.blocks.clone(),
            },
            candidates: candidates.clone(),
            concepts: dir.concept_inputs(),
            entities: offered.clone(),
            rejected: note.rejected_input(),
        };
        let prompt = prompts::latest(ids::LINKING)
            .ok_or_else(|| JobError::Fatal("linking prompt missing".into()))?;
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        let out = match d
            .ai
            .complete::<Linking>(caller, prompt, &input, MAX_TOKENS)
            .await
        {
            Ok(o) => o,
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let model = format!("{}/{}", out.provider, out.model);
        let l = out.value;
        let extraction = Extraction {
            relations: l.relations,
            concepts: l.concepts,
            mentions: l.mentions,
            entity_relations: l.entity_relations,
            custody: l.custody,
            tasks: l.tasks,
        };
        let set = d
            .plan(
                &ctx,
                pipeline::LINK,
                &note,
                &dir,
                &candidates,
                &offered,
                &extraction,
                model,
            )
            .await?;
        match d.vault.ai_apply(&ctx.scope, set).await? {
            AiApplied::Done { .. } | AiApplied::Stale => Ok(()),
        }
    }
}

/// The job a forced relink of the note at `path` runs: `file_inbox` for an inbox capture
/// (its filing job links it), `link` otherwise.
pub fn relink_kind(path: &str) -> &'static str {
    if in_inbox(path) {
        pipeline::FILE_INBOX
    } else {
        pipeline::LINK
    }
}

/// Enqueues a forced `kind` job (`link` or `file_inbox`, see [`relink_kind`]) for `note` now
/// (`POST /notes/{id}/relink`).
pub async fn enqueue_forced(
    db: &AppDb,
    scope: &UserScope,
    ids: &dyn IdGenerator,
    kind: &str,
    note: NoteId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<strata_common::JobId, JobError> {
    let mut tx = db.begin(scope).await?;
    let job = enqueue_forced_in(&mut tx, ids, kind, note, now).await?;
    tx.commit().await?;
    Ok(job)
}

/// [`enqueue_forced`] inside the caller's transaction (the sync push stores the op's result in
/// the same transaction). A queued job of `kind` for the note becomes the forced run.
pub async fn enqueue_forced_in(
    tx: &mut strata_index::ScopedTx,
    ids: &dyn IdGenerator,
    kind: &str,
    note: NoteId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<strata_common::JobId, JobError> {
    let job = crate::repo::enqueue(
        tx,
        &strata_index::repo::jobs::NewJob {
            id: strata_common::JobId::generate(ids),
            kind: kind.to_owned(),
            note_id: Some(note),
            payload: pipeline::rmp(&LinkParams { force: true })?,
            run_after: now,
            max_attempts: 5,
            dedupe_key: Some(note.to_string()),
        },
        now,
    )
    .await?;
    Ok(job.id)
}
