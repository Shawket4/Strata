//! The `file_inbox` job (PLAN §9.2, §9.3): one `inbox_filing` call on a capture proposes a
//! title, tags and a destination folder, and extracts what linking extracts (relations,
//! concepts, people, companies, custody, tasks). Relations, concepts, entities and custody are
//! applied like linking; title, tags and the move are applied only when the user's `auto_file`
//! setting is on, otherwise they become a `filing` suggestion. A capture that corrects an
//! earlier AI decision also queues the correction job (§9.8). One `ai: file_inbox <path>`
//! commit.

use serde::Serialize;
use strata_ai::outputs::{InboxFiling, MentionKind};
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError};
use strata_common::{DecisionId, JobId, SuggestionId};
use strata_index::repo::jobs::NewJob;
use strata_index::types::DecisionKind;
use strata_index::{AppDb, UserScope};
use strata_vault::model::TreeEntry;
use strata_vault::ops::ai_apply::{AiApplied, Filing, NewDecision, NewSuggestion, SYSTEM_FOLDERS};
use strata_vault::ops::ai_decide as decide;
use sync_model::suggestions::FilingPayload;

use crate::correct::{self, CorrectParams};
use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::link::{LinkParams, PipelineDeps, in_inbox};
use crate::pipeline::{
    self, BlockInput, CandidateInput, ConceptInput, EntityInput, Extraction, RejectedInput,
    SourceNote,
};

/// Output token limit of one filing call.
const MAX_TOKENS: u32 = 4000;

/// The user setting that applies filing proposals automatically (§9.3; default off).
pub const AUTO_FILE_SETTING: &str = "auto_file";

/// The capture in an `inbox_filing` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilingNote {
    /// ID.
    pub id: String,
    /// `created` (RFC 3339, user's time zone).
    pub created: String,
    /// Blocks.
    pub blocks: Vec<BlockInput>,
}

/// The `inbox_filing` prompt input (field order is the prompt's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilingInput {
    /// The capture.
    pub note: FilingNote,
    /// Existing folders.
    pub folders: Vec<String>,
    /// Most used tags.
    pub top_tags: Vec<String>,
    /// Candidate notes.
    pub candidates: Vec<CandidateInput>,
    /// Concepts.
    pub concepts: Vec<ConceptInput>,
    /// Entities.
    pub entities: Vec<EntityInput>,
    /// Rejections.
    pub rejected: Vec<RejectedInput>,
}

/// The `file_inbox` job.
#[derive(Debug, Clone)]
pub struct FileInboxHandler {
    deps: PipelineDeps,
}

impl FileInboxHandler {
    /// The handler.
    pub fn new(deps: PipelineDeps) -> Self {
        Self { deps }
    }
}

/// Whether the user turned auto-file on (a `MessagePack` boolean setting).
pub async fn auto_file(db: &AppDb, scope: &UserScope) -> Result<bool, JobError> {
    let mut tx = db.begin(scope).await?;
    let v = strata_index::repo::settings::get_setting(&mut tx, AUTO_FILE_SETTING).await?;
    tx.commit().await?;
    Ok(v.and_then(|b| rmp_serde::from_slice::<bool>(&b).ok())
        .unwrap_or(false))
}

/// The folders a capture may be filed into (user folders; `notes` always).
pub fn filing_folders(tree: &[TreeEntry]) -> Vec<String> {
    let mut out: Vec<String> = tree
        .iter()
        .filter_map(|e| match e {
            TreeEntry::Folder { path } => Some(path.clone()),
            _ => None,
        })
        .filter(|p| {
            let top = p.split('/').next().unwrap_or_default();
            !p.starts_with('.') && !SYSTEM_FOLDERS.contains(&top) && top != "tasks"
        })
        .collect();
    if !out.iter().any(|p| p == "notes") {
        out.push("notes".to_owned());
    }
    out.sort();
    out.dedup();
    out
}

/// The most used tags, most used first (ties by name), at most 20.
pub async fn top_tags(db: &AppDb, scope: &UserScope) -> Result<Vec<String>, JobError> {
    let mut tx = db.begin(scope).await?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT t.tag FROM tags t JOIN notes n ON n.user_id = t.user_id AND n.id = t.note_id \
         WHERE NOT n.trashed GROUP BY t.tag ORDER BY count(*) DESC, t.tag LIMIT 20",
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

#[async_trait::async_trait]
impl JobHandler for FileInboxHandler {
    fn kind(&self) -> &'static str {
        pipeline::FILE_INBOX
    }

    fn interactive(&self) -> bool {
        true
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    #[allow(clippy::too_many_lines)] // one linear pass: context, call, plan, filing, write
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(id) = ctx.job.note_id else {
            return Err(JobError::Fatal("file_inbox job without a note".into()));
        };
        let params = LinkParams::decode(&ctx.job.payload);
        let d = &self.deps;
        let Some(note) = SourceNote::load(&d.vault, &ctx.scope, id).await? else {
            return Ok(());
        };
        if note.kind != domain::NoteKind::Note || !in_inbox(&note.path) {
            return Ok(());
        }
        let filed = note
            .sidecar
            .extra
            .get(strata_vault::ops::ai_apply::FILED_KEY)
            .and_then(|v| v.as_str().map(str::to_owned));
        if !params.force && filed.as_deref() == Some(&note.version) {
            return Ok(());
        }
        let (candidates, dir, offered) = d.context(&ctx.scope, &note).await?;
        let folders = filing_folders(&d.vault.tree(&ctx.scope).await?);
        let input = FilingInput {
            note: FilingNote {
                id: id.to_string(),
                created: note.created_rfc3339(),
                blocks: note.blocks.clone(),
            },
            folders: folders.clone(),
            top_tags: top_tags(&d.db, &ctx.scope).await?,
            candidates: candidates.clone(),
            concepts: dir.concept_inputs(),
            entities: offered.clone(),
            rejected: note.rejected_input(),
        };
        let prompt = prompts::latest(ids::INBOX_FILING)
            .ok_or_else(|| JobError::Fatal("inbox_filing prompt missing".into()))?;
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        let out = match d
            .ai
            .complete::<InboxFiling>(caller, prompt, &input, MAX_TOKENS)
            .await
        {
            Ok(o) => o,
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let model = format!("{}/{}", out.provider, out.model);
        let f = out.value;
        let mentions = f
            .people
            .iter()
            .map(|m| m.to_mention(MentionKind::Person))
            .chain(
                f.companies
                    .iter()
                    .map(|m| m.to_mention(MentionKind::Company)),
            )
            .collect();
        let extraction = Extraction {
            relations: f.relations.clone(),
            concepts: f.concepts.clone(),
            mentions,
            entity_relations: Vec::new(),
            custody: f.custody.clone(),
            tasks: f.tasks.clone(),
        };
        let mut set = d
            .plan(
                &ctx,
                pipeline::FILE_INBOX,
                &note,
                &dir,
                &candidates,
                &offered,
                &extraction,
                model,
            )
            .await?;
        set.mark_filed = true;
        let folder = if folders.contains(&f.destination_folder) {
            f.destination_folder.clone()
        } else {
            "notes".to_owned()
        };
        let title = vault_format::filename::sanitize_file_name(f.title.trim());
        let title = if title.trim().is_empty() {
            note.title.clone()
        } else {
            f.title.trim().to_owned()
        };
        let tags: Vec<String> = f
            .tags
            .iter()
            .map(|t| t.trim().trim_start_matches('#').to_lowercase())
            .filter(|t| vault_format::body::is_valid_tag(t))
            .take(5)
            .collect();
        let decision_id = DecisionId::generate(d.ids.as_ref());
        let mut decision = NewDecision {
            id: decision_id,
            kind: DecisionKind::Filing,
            source_note: Some(id),
            source_block: None,
            target_type: "folder".into(),
            target_id: format!("{folder}/{title}.md"),
            summary: format!("file as {folder}/{title} [{}]", tags.join(", ")),
            confidence: None,
            rel_type: None,
            mention: None,
            detail: Vec::new(),
            suggestion: None,
        };
        if auto_file(&d.db, &ctx.scope).await? {
            set.filing = Some(Filing {
                title,
                tags,
                folder,
            });
        } else {
            let pending = d.vault.note_suggestions(&ctx.scope, id).await?;
            if !pending
                .iter()
                .any(|s| s.suggestion.kind == decide::KIND_FILING)
            {
                let sid = SuggestionId::generate(d.ids.as_ref());
                set.suggestions.push(NewSuggestion {
                    id: sid,
                    note: Some(id),
                    kind: decide::KIND_FILING.to_owned(),
                    payload: pipeline::rmp(&FilingPayload {
                        decision_id: decision_id.as_ulid(),
                        title,
                        tags,
                        folder,
                    })?,
                });
                decision.suggestion = Some(sid);
            }
        }
        set.decisions.push(decision);
        if f.is_correction {
            let now = d.clock.now();
            set.jobs.push(NewJob {
                id: JobId::generate(d.ids.as_ref()),
                kind: correct::CORRECT.to_owned(),
                note_id: Some(id),
                payload: pipeline::rmp(&CorrectParams {
                    message: note.body().trim().to_owned(),
                    created: note.created_rfc3339(),
                    source: Some(id),
                })?,
                run_after: now,
                max_attempts: 5,
                dedupe_key: Some(id.to_string()),
            });
        }
        match d.vault.ai_apply(&ctx.scope, set).await? {
            AiApplied::Done { .. } | AiApplied::Stale => Ok(()),
        }
    }
}
