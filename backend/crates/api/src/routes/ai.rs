//! AI endpoints (PLAN §7.5 AI, §9.5): `POST /ask` + the `GET /ask/{id}` answer stream
//! (D24), `POST /ask/{id}/save` ("save as note"), and `GET /ai/status`.
//!
//! Ask frames (`AskFrame`, one per WebSocket binary message in the D24 envelope): `tokens`
//! (a batch of answer text, in order), then one `citation` per cited source (in order of
//! first appearance, resolved to the note and the block that now carries the cited ID), then
//! `done` with the final answer text (unknown citations removed), then the `end` control
//! frame. A failure ends the stream with an `error` frame (`ai_paused`, `ai_unavailable`).

use actix_web::{HttpRequest, HttpResponse, Responder, web};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use strata_ai::{AiCaller, AiError, PauseReason};
use strata_common::NoteId;
use strata_jobs::ask::{AskError, Citation, note_content};
use strata_vault::VaultService;
use strata_vault::ops::notes::CreateNote;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::ai::{AiApi, AskEnd};
use crate::auth::{AuthState, Authenticated};
use crate::openapi::StreamOperation;
use crate::routes::notes::Note;
use crate::vault::OrProblem;
use crate::wire::ws::{self, ResumeQuery, WsConfig};
use crate::wire::{MsgPack, Problem, ProblemFieldError, ProblemType};

/// The Ask answer stream in the contract.
pub const ASK_STREAM: StreamOperation = StreamOperation {
    path: "/ask/{id}",
    operation_id: "ask_stream",
    tag: "ai",
    summary: "The answer of an Ask started with `POST /ask`: token batches, then citations \
              resolved to notes and blocks, then `done`; resumable with `resume_from`.",
    payload: "AskFrame",
    secured: true,
};

/// `POST /ask`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AskRequest {
    /// The question (Arabic, English or mixed).
    pub question: String,
    /// Only use notes under this folder (vault path prefix, e.g. `notes/clients`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// An Ask that started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AskStarted {
    /// Answer ID: stream it from `GET /ask/{id}`.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
}

/// One frame of an Ask answer. Internally tagged by `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum AskFrame {
    /// The next batch of answer text.
    #[serde(rename = "tokens")]
    Tokens {
        /// Text.
        text: String,
    },
    /// A citation of the answer, resolved for navigation.
    #[serde(rename = "citation")]
    Citation {
        /// 1-based order of first appearance in the answer.
        #[schema(minimum = 1)]
        index: u32,
        /// The citation as written in the answer (`Note#^block`).
        #[serde(rename = "ref")]
        reference: String,
        /// The cited note.
        #[schema(value_type = String, format = "ulid")]
        note_id: Ulid,
        /// Its path.
        path: String,
        /// Its title.
        title: String,
        /// The cited block's ID (absent: the note as a whole).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        /// The wikilink target that resolves in the vault (`Note#^block` or `Note`).
        target: String,
    },
    /// The answer is complete.
    #[serde(rename = "done")]
    Done {
        /// The final answer: markdown with `[[target]]` citations.
        answer: String,
    },
}

impl AskFrame {
    /// The frame of a resolved citation.
    pub fn from_citation(c: &Citation) -> Self {
        Self::Citation {
            index: c.index,
            reference: c.reference.clone(),
            note_id: c.note_id.as_ulid(),
            path: c.path.clone(),
            title: c.title.clone(),
            block_id: c.block_id.clone(),
            target: c.target.clone(),
        }
    }
}

/// `POST /ask/{id}/save`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct SaveAskRequest {
    /// Note title (file name in `notes/`); default: the question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// When the user saved it on the device (UTC; required): the note's `created` and
    /// `updated`. More than `max_future_skew_secs` ahead of the server's clock is
    /// `422 created_in_future`.
    pub created: chrono::DateTime<chrono::Utc>,
    /// Create even if it looks like a duplicate.
    #[serde(default)]
    pub force: bool,
}

/// A pause of AI work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiPauseDto {
    /// `user_budget`, `global_budget`, `provider_usage_limit`, `provider_rate_limit`.
    pub reason: String,
    /// When it ends, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,
}

/// The provider serving the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiProviderDto {
    /// `claude_cli` or `anthropic_api`.
    pub name: String,
    /// Model.
    pub model: String,
    /// `ready`, `paused` or `degraded`.
    pub state: String,
    /// Kind of the last failure (content-free).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Usage totals of one day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiUsageDto {
    /// Calls.
    pub calls: u64,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Estimated cost in micro-USD.
    pub cost_micros: u64,
}

/// Daily caps (0 = unlimited).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiLimitsDto {
    /// Tokens per user per day.
    pub per_user_daily_tokens: u64,
    /// Micro-USD per user per day.
    pub per_user_daily_cost_micros: u64,
    /// Tokens per day, all users.
    pub global_daily_tokens: u64,
    /// Micro-USD per day, all users.
    pub global_daily_cost_micros: u64,
}

/// The local embedding model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiEmbeddingsDto {
    /// Model ID stored with every vector.
    pub model_id: String,
    /// Dimensions.
    pub dims: u32,
    /// In memory now (it loads on demand and unloads when idle).
    pub loaded: bool,
    /// The caller's live notes with current vectors.
    pub embedded: u64,
    /// The caller's live notes.
    pub total: u64,
}

/// `GET /ai/status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiStatusDto {
    /// AI is enabled for the caller.
    pub enabled: bool,
    /// The provider serving the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<AiProviderDto>,
    /// The pause in effect (budget first, then provider).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<AiPauseDto>,
    /// The caller's queued and running background jobs.
    pub queue_depth: u64,
    /// Budget day.
    #[schema(value_type = String, format = "date")]
    pub day: NaiveDate,
    /// The caller's usage today.
    pub usage: AiUsageDto,
    /// Everyone's usage today.
    pub global_usage: AiUsageDto,
    /// Daily caps.
    pub limits: AiLimitsDto,
    /// The embedding model and the caller's embedding progress, when configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<AiEmbeddingsDto>,
}

fn reason_slug(r: PauseReason) -> &'static str {
    match r {
        PauseReason::UserBudget => "user_budget",
        PauseReason::GlobalBudget => "global_budget",
        PauseReason::ProviderUsageLimit => "provider_usage_limit",
        PauseReason::ProviderRateLimit => "provider_rate_limit",
    }
}

/// `503 ai_paused` with the reason and end of the pause.
pub fn paused_problem(reason: PauseReason, until: Option<DateTime<Utc>>) -> Problem {
    let detail = match until {
        Some(u) => format!("{} until {}", reason_slug(reason), u.to_rfc3339()),
        None => reason_slug(reason).to_owned(),
    };
    Problem::new(ProblemType::AiPaused).with_detail(detail)
}

/// `503 ai_unavailable` with a content-free detail.
pub fn unavailable_problem(detail: &str) -> Problem {
    Problem::new(ProblemType::AiUnavailable).with_detail(detail)
}

/// The problem of an Ask failure.
pub fn ask_problem(e: &AskError) -> Problem {
    match e {
        AskError::EmptyQuestion => Problem::new(ProblemType::InvalidBody)
            .with_detail("the question is empty")
            .with_error(ProblemFieldError {
                code: "empty".to_owned(),
                pointer: Some("/question".to_owned()),
                message: "the question is empty".to_owned(),
            }),
        AskError::Paused { reason, until } => paused_problem(*reason, *until),
        AskError::Unavailable(_) => {
            unavailable_problem("AI is disabled or not available for this account")
        }
        AskError::Internal(_) => Problem::new(ProblemType::Internal),
    }
}

fn ai_problem(e: &AiError) -> Problem {
    match e {
        AiError::Paused { reason, until } => paused_problem(*reason, *until),
        AiError::Disabled | AiError::ProviderNotConfigured(_) => {
            unavailable_problem("AI is disabled or not available for this account")
        }
        _ => Problem::new(ProblemType::Internal),
    }
}

async fn username(api: &AiApi, auth: &Authenticated) -> Result<String, Problem> {
    let mut tx = api.db.begin(auth.scope()).await.or_problem()?;
    let name = strata_jobs::repo::username(&mut tx)
        .await
        .map_err(|_| Problem::new(ProblemType::Internal))?
        .unwrap_or_default();
    tx.commit().await.or_problem()?;
    Ok(name)
}

fn ai_api(api: Option<&web::Data<AiApi>>) -> Result<&AiApi, Problem> {
    api.map(web::Data::get_ref)
        .ok_or_else(|| unavailable_problem("the AI subsystem is not configured on this server"))
}

/// Ask a question about your notes (hybrid retrieval, cited answer).
#[utoipa::path(
    post, path = "/ask", tag = "ai", operation_id = "ask",
    request_body = AskRequest,
    responses(
        (status = 200, description = "Started; stream the answer from `GET /ask/{id}`.", body = AskStarted),
        (status = 429, description = "`rate_limited`: too many questions from this user (`auth.rate_limits.ask_per_user`). See `Retry-After`.", body = Problem),
        (status = 503, description = "`ai_unavailable` (AI disabled or not configured) or `ai_paused` (budget reached, provider usage limit; `detail` says until when).", body = Problem),
    ),
)]
pub async fn ask(
    auth: Authenticated,
    state: web::Data<AuthState>,
    api: Option<web::Data<AiApi>>,
    body: MsgPack<AskRequest>,
) -> Result<MsgPack<AskStarted>, actix_web::Error> {
    let api = ai_api(api.as_ref())?;
    state.check_ask_limit(auth.user_id())?;
    let req = body.into_inner();
    let name = username(api, &auth).await?;
    let run = api
        .ask
        .start(*auth.scope(), name, &req.question, req.scope.as_deref())
        .await
        .map_err(|e| ask_problem(&e))?;
    let id = api.ids.next_ulid();
    let entry = api
        .asks
        .insert(id, auth.user_id(), req.question.trim(), api.clock.now());
    let clock = api.clock.clone();
    tokio::spawn(async move {
        run.run(move |event| entry.push(event, clock.now())).await;
    });
    // A question that corrects an earlier AI decision also goes to the correction job (§9.8).
    let now = api.clock.now();
    if let Err(e) = strata_jobs::correct::enqueue_for_message(
        &api.db,
        auth.scope(),
        api.ids.as_ref(),
        &req.question,
        now.fixed_offset(),
        now,
    )
    .await
    {
        tracing::warn!(error = %e, "queueing a correction from Ask failed");
    }
    Ok(MsgPack(AskStarted { id }))
}

/// Streams an Ask answer (WebSocket, D24).
pub async fn ask_stream(
    req: HttpRequest,
    body: web::Payload,
    auth: Authenticated,
    api: Option<web::Data<AiApi>>,
    id: web::Path<Ulid>,
    query: web::Query<ResumeQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let api = ai_api(api.as_ref())?;
    let entry = api
        .asks
        .get(*id, auth.user_id())
        .ok_or_else(|| Problem::new(ProblemType::NotFound))?;
    let config = req
        .app_data::<web::Data<WsConfig>>()
        .map_or_else(WsConfig::default, |c| ***c);
    ws::start(&req, body, config, entry.frames(query.resume_from))
}

/// Saves an answer as a note in `notes/` with its citations as links.
#[utoipa::path(
    post, path = "/ask/{id}/save", tag = "ai", operation_id = "save_ask",
    params(("id" = Ulid, Path, description = "Answer ID.")),
    request_body = SaveAskRequest,
    responses(
        (status = 201, description = "Created (waits for the answer to finish); one `user: create <path>` commit.", body = Note),
        (status = 409, description = "`duplicate_candidates` (resend with `force`) or `path_taken`.", body = Problem),
        (status = 422, description = "`invalid_name` (title) or `invalid_body`.", body = Problem),
    ),
)]
pub async fn save_ask(
    auth: Authenticated,
    api: Option<web::Data<AiApi>>,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<SaveAskRequest>,
) -> Result<impl Responder, Problem> {
    let api = ai_api(api.as_ref())?;
    let entry = api
        .asks
        .get(*id, auth.user_id())
        .ok_or_else(|| Problem::new(ProblemType::NotFound))?;
    let (answer, citations) = match entry.finished().await {
        AskEnd::Done { answer, citations } => (answer, citations),
        AskEnd::Failed(_) => {
            return Err(Problem::new(ProblemType::NotFound)
                .with_detail("the answer failed; there is nothing to save"));
        }
    };
    let req = body.into_inner();
    let title = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| entry.question());
    let name: String = vault_format::filename::sanitize_file_name(title)
        .chars()
        .take(80)
        .collect();
    let name = name.trim().to_owned();
    let content = note_content(entry.question(), &answer, &citations);
    let note = vault
        .create_note(
            auth.scope(),
            CreateNote {
                path: format!("notes/{name}.md"),
                content,
                created: req.created,
                id: None,
                force: req.force,
            },
        )
        .await
        .or_problem()?;
    Ok(MsgPack(Note::from(note))
        .customize()
        .with_status(actix_web::http::StatusCode::CREATED))
}

/// AI status for the caller: provider health, pause, queue depth, usage vs budget,
/// embedding model and progress.
#[utoipa::path(
    get, path = "/ai/status", tag = "ai", operation_id = "ai_status",
    responses(
        (status = 200, description = "The caller's AI status.", body = AiStatusDto),
        (status = 503, description = "`ai_unavailable`: the AI subsystem is not configured on this server.", body = Problem),
    ),
)]
pub async fn ai_status(
    auth: Authenticated,
    api: Option<web::Data<AiApi>>,
) -> Result<MsgPack<AiStatusDto>, Problem> {
    let api = ai_api(api.as_ref())?;
    let name = username(api, &auth).await?;
    let caller = AiCaller {
        scope: *auth.scope(),
        username: name,
    };
    let s = strata_jobs::status::ai_status(&api.ai, &api.db, &caller)
        .await
        .map_err(|e| ai_problem(&e))?;
    let usage = |u: strata_ai::budget::UsageTotals| AiUsageDto {
        calls: u.calls,
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        cost_micros: u.cost_micros,
    };
    Ok(MsgPack(AiStatusDto {
        enabled: s.enabled,
        provider: s.provider.map(|p| AiProviderDto {
            name: p.name,
            model: p.model,
            state: match p.health.state {
                strata_ai::provider::HealthState::Ready => "ready",
                strata_ai::provider::HealthState::Paused { .. } => "paused",
                strata_ai::provider::HealthState::Degraded => "degraded",
            }
            .to_owned(),
            last_error: p.health.last_error,
        }),
        paused: s.paused.map(|p| AiPauseDto {
            reason: reason_slug(p.reason).to_owned(),
            until: p.until,
        }),
        queue_depth: s.queue_depth.unwrap_or(0),
        day: s.usage.day,
        usage: usage(s.usage.user),
        global_usage: usage(s.usage.global),
        limits: AiLimitsDto {
            per_user_daily_tokens: s.limits.per_user_daily_tokens,
            per_user_daily_cost_micros: s.limits.per_user_daily_cost_micros,
            global_daily_tokens: s.limits.global_daily_tokens,
            global_daily_cost_micros: s.limits.global_daily_cost_micros,
        },
        embeddings: s.embeddings.map(|e| {
            let progress = s
                .embedding_progress
                .unwrap_or(strata_ai::EmbeddingProgress {
                    embedded: 0,
                    total: 0,
                });
            AiEmbeddingsDto {
                model_id: e.model_id,
                dims: u32::try_from(e.dims).unwrap_or(u32::MAX),
                loaded: e.loaded,
                embedded: progress.embedded,
                total: progress.total,
            }
        }),
    }))
}

/// Mounts the AI routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/ask", web::post().to(ask));
    cfg.route("/ask/{id}", web::get().to(ask_stream));
    cfg.route("/ask/{id}/save", web::post().to(save_ask));
    cfg.route("/ai/status", web::get().to(ai_status));
}

/// The AI part of the contract (merged into the production document).
#[derive(Debug, utoipa::OpenApi)]
#[openapi(
    paths(ask, save_ask, ai_status),
    components(schemas(AskFrame)),
    tags((name = "ai", description = "Ask (RAG with citations) and AI status (PLAN §7.5 AI, §9.5)."))
)]
pub struct AiApiDoc;

/// Note IDs of citations (for tests and callers resolving navigation).
pub fn cited_notes(frames: &[AskFrame]) -> Vec<NoteId> {
    frames
        .iter()
        .filter_map(|f| match f {
            AskFrame::Citation { note_id, .. } => Some(NoteId::from_ulid(*note_id)),
            _ => None,
        })
        .collect()
}
