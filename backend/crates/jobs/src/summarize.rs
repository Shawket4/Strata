//! The `summarize` job (PLAN §9.2, §6.5): writes the one-paragraph `summary` into the note's
//! sidecar in one `ai: summarize <path>` commit (sidecar only; the note is never touched,
//! principle 3). Skipped when the sidecar already holds a summary of this note version, and
//! when AI is disabled for the user (principle 6). Runs after `embed`.

use std::sync::Arc;

use serde::Serialize;
use strata_ai::outputs::Summary;
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService};
use strata_vault::VaultService;
use strata_vault::ops::ai::SummaryWrite;
use vault_format::Document;

use crate::embed::SUMMARIZE;
use crate::handler::{JobClass, JobContext, JobError, JobHandler};

/// Characters of body text sent to the model (a note fits the model's context; this bounds
/// cost for very long notes).
pub const MAX_INPUT_CHARS: usize = 24_000;

/// Output token limit of one summary.
const MAX_TOKENS: u32 = 400;

#[derive(Debug, Serialize)]
struct NoteInput<'a> {
    id: String,
    title: &'a str,
    created: String,
    text: String,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct Input<'a> {
    note: NoteInput<'a>,
}

/// The `summarize` job.
#[derive(Debug, Clone)]
pub struct SummarizeHandler {
    vault: VaultService,
    ai: Arc<AiService>,
}

impl SummarizeHandler {
    /// The handler.
    pub fn new(vault: VaultService, ai: Arc<AiService>) -> Self {
        Self { vault, ai }
    }
}

/// The body of `content`, cut at `max` characters; whether it was cut.
pub fn truncated_body(content: &str, max: usize) -> (String, bool) {
    let doc = Document::parse(content);
    let body = doc.body().trim();
    match body.char_indices().nth(max) {
        Some((i, _)) => (body[..i].to_owned(), true),
        None => (body.to_owned(), false),
    }
}

#[async_trait::async_trait]
impl JobHandler for SummarizeHandler {
    fn kind(&self) -> &'static str {
        SUMMARIZE
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(note) = ctx.job.note_id else {
            return Err(JobError::Fatal("summarize job without a note".into()));
        };
        let view = match self.vault.note(&ctx.scope, note).await {
            Ok(v) if !v.trashed => v,
            Ok(_) | Err(strata_vault::VaultError::NotFound) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let sidecar = self.vault.note_sidecar(&ctx.scope, note).await?;
        if sidecar.as_ref().is_some_and(|s| {
            s.summary.is_some() && s.content_hash.as_deref() == Some(view.version.as_str())
        }) {
            return Ok(());
        }
        let (text, truncated) = truncated_body(&view.content, MAX_INPUT_CHARS);
        let input = Input {
            note: NoteInput {
                id: note.to_string(),
                title: &view.title,
                created: view.created.to_rfc3339(),
                text,
                truncated,
            },
        };
        let prompt = prompts::latest(ids::SUMMARY)
            .ok_or_else(|| JobError::Fatal("summary prompt missing".into()))?;
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        let out = match self
            .ai
            .complete::<Summary>(caller, prompt, &input, MAX_TOKENS)
            .await
        {
            Ok(out) => out,
            // AI off for this user: nothing to do (principle 6).
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        match self
            .vault
            .ai_set_summary(
                &ctx.scope,
                note,
                view.version.clone(),
                out.value.summary,
                SUMMARIZE.to_owned(),
            )
            .await?
        {
            SummaryWrite::Written | SummaryWrite::Unchanged | SummaryWrite::Stale => Ok(()),
        }
    }
}
