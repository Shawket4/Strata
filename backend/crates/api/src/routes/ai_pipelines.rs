//! The AI pipelines' endpoints (PLAN §7.5 AI, People & companies, Suggestions; §9.3, §9.8):
//!
//! - `GET /ai-decisions` — the activity feed of AI decisions (every decision has an ID and a
//!   source, §9.8);
//! - `POST /ai-decisions/{id}/repoint`, `/retype`, `/reject` — correct an applied decision
//!   (or answer its pending suggestion) in one `user:` commit; a repointed entity link stores a
//!   disambiguation hint and the rejected link is never re-added;
//! - `POST /suggestions/{id}/accept-with-edits` — accept an AI suggestion with edits
//!   (filing title/tags/folder, the entity to link or the new entity's name and aliases, the
//!   custody participant, task fields);
//! - `POST /entities/{id}/refresh` — force an entity-insights refresh;
//! - `POST /notes/{id}/relink` — force a linking run;
//! - `GET`/`PUT /ai/settings` — the user's auto-file setting (§9.3).
//!
//! The payload DTOs of AI suggestions (`filing`, `entity_link`, `custody`, `task`,
//! `correction`) are defined here and used by `routes::inbox`.

use actix_web::http::StatusCode;
use actix_web::{Responder, web};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::{DecisionId, NoteId, SuggestionId};
use strata_index::types::DecisionKind;
use strata_vault::VaultService;
use strata_vault::ops::ai_decide::{
    self as decide, CorrectionPayload, CustodyPayload, CustodyTarget, DecisionFix, DecisionView,
    EntityLinkPayload, FilingPayload, FixAction, TaskPayload,
};
use ulid::Ulid;
use utoipa::ToSchema;
use vault_format::RelationKey;

use crate::ai::AiApi;
use crate::auth::Authenticated;
use crate::routes::inbox::Suggestion;
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem, ProblemType};

/// One AI decision (§9.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AiDecision {
    /// Decision ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// `relation`, `entity_mention`, `concept`, `custody_event`, `task_suggestion`,
    /// `filing`, `correction`.
    pub kind: String,
    /// The note it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub source_note_id: Option<Ulid>,
    /// That note's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_title: Option<String>,
    /// The block stating it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_block_id: Option<String>,
    /// `note`, `entity`, `concept`, `document`, `task`, `folder`, `decision`.
    pub target_type: String,
    /// The target (a note ID for notes and entities).
    pub target_id: String,
    /// The target's title, when it is a note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// One line for the activity feed.
    pub summary: String,
    /// Model confidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Relation key or custody event type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub rel_type: Option<String>,
    /// The mention text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention: Option<String>,
    /// The suggestion it became (not applied automatically).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub suggestion_id: Option<Ulid>,
    /// That suggestion's status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_status: Option<String>,
    /// The commit that applied it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// When.
    pub created: DateTime<Utc>,
    /// When it was corrected (repointed, retyped or rejected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverted_at: Option<DateTime<Utc>>,
}

fn kind_str(k: DecisionKind) -> &'static str {
    k.as_str()
}

impl From<DecisionView> for AiDecision {
    fn from(v: DecisionView) -> Self {
        let r = v.row;
        Self {
            id: r.id.as_ulid(),
            kind: kind_str(r.kind).to_owned(),
            source_note_id: r.source_note_id.map(|n| n.as_ulid()),
            source_title: v.source_title,
            source_block_id: r.source_block_id,
            target_type: r.target_type,
            target_id: r.target_id,
            target_name: v.target_name,
            summary: r.summary,
            confidence: r.confidence,
            rel_type: r.rel_type,
            mention: r.mention,
            suggestion_id: r.suggestion_id.map(|s| s.as_ulid()),
            suggestion_status: v.suggestion_status.map(|s| s.as_str().to_owned()),
            commit: r.git_commit,
            created: r.created,
            reverted_at: r.reverted_at,
        }
    }
}

/// The activity feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AiDecisions {
    /// Newest first.
    pub items: Vec<AiDecision>,
}

/// `GET /ai-decisions` query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DecisionsQuery {
    /// How many (1–500, default 50).
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `POST /ai-decisions/{id}/repoint`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RepointRequest {
    /// The entity or note it should point at.
    #[schema(value_type = String, format = "ulid")]
    pub target_id: Ulid,
    /// A disambiguation hint to remember (default: derived from the mention and its note,
    /// e.g. `"Ahmed" in "Acme call" = Ahmed Fathy`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// `POST /ai-decisions/{id}/retype`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RetypeRequest {
    /// The new relation type (`related`, `part-of`, `supports`, …).
    #[serde(rename = "type")]
    pub new_type: String,
}

/// The corrected decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CorrectionResult {
    /// The decision after the correction (`reverted_at` set).
    pub decision: AiDecision,
    /// The `user:` commit (absent when only database rows changed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// Edits applied when accepting an AI suggestion (every field optional; the ones that do not
/// apply to the suggestion's kind are ignored).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct SuggestionEditsDto {
    /// Filing: title; entity link: the new entity's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Filing: tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Filing: destination folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Entity link, custody, correction: the entity (or note) to use instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub target_id: Option<Ulid>,
    /// Entity link: spellings to add as aliases (both scripts, §6.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    /// Task: text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Task: due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// Task: recurrence phrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Task: reminders (local times).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Vec<String>>)]
    pub reminders: Option<Vec<NaiveDateTime>>,
}

impl From<SuggestionEditsDto> for sync_model::ops::SuggestionEdits {
    fn from(e: SuggestionEditsDto) -> Self {
        Self {
            title: e.title,
            tags: e.tags,
            folder: e.folder,
            target_id: e.target_id,
            aliases: e.aliases,
            text: e.text,
            due: e.due,
            recurrence: e.recurrence,
            reminders: e.reminders,
        }
    }
}

/// `POST /suggestions/{id}/accept-with-edits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AcceptWithEditsRequest {
    /// The edits.
    pub edits: SuggestionEditsDto,
}

/// A queued job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JobQueued {
    /// Job ID (`job.completed` / `job.failed` events carry it).
    #[schema(value_type = String, format = "ulid")]
    pub job_id: Ulid,
}

/// The user's AI settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AiSettings {
    /// Apply filing proposals (title, tags, move) automatically (§9.3; default off).
    pub auto_file: bool,
}

// ---- suggestion payload DTOs (used by `routes::inbox`) ---------------------------------------

/// A filing proposal (`filing`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FilingProposal {
    /// The decision.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// Title.
    pub title: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Destination folder.
    pub folder: String,
}

/// A link-or-create proposal (`entity_link`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct EntityLinkProposal {
    /// The decision.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// The mention as written.
    pub mention: String,
    /// `person`, `company`, `document`, `place`.
    pub entity_kind: String,
    /// The note mentioning it.
    #[schema(value_type = String, format = "ulid")]
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    /// The proposed entity (absent: create a new one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub proposed: Option<Ulid>,
    /// Entities that fit.
    #[schema(value_type = Vec<String>)]
    pub candidates: Vec<Ulid>,
    /// A nickname or kinship term.
    pub is_nickname: bool,
    /// Model confidence.
    pub confidence: f64,
    /// `ambiguous`, `nickname`, `new`, `low_confidence`, `reply`.
    pub reason: String,
}

/// A custody participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CustodyTargetDto {
    /// Mention.
    pub mention: String,
    /// Resolved entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub id: Option<Ulid>,
    /// Entities that fit.
    #[schema(value_type = Vec<String>)]
    pub candidates: Vec<Ulid>,
}

impl From<CustodyTarget> for CustodyTargetDto {
    fn from(t: CustodyTarget) -> Self {
        Self {
            mention: t.mention,
            id: t.id.map(|i| i.as_ulid()),
            candidates: t.candidates.into_iter().map(|c| c.as_ulid()).collect(),
        }
    }
}

/// A custody event proposal (`custody`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CustodyProposal {
    /// The decision.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// The note stating it.
    #[schema(value_type = String, format = "ulid")]
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    /// Event type (`stored-at`, …).
    pub event: String,
    /// Date.
    pub date: NaiveDate,
    /// Document.
    pub document: CustodyTargetDto,
    /// Place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<CustodyTargetDto>,
    /// Enclosing place mention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_part_of: Option<String>,
    /// Person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<CustodyTargetDto>,
    /// Third party.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterparty: Option<CustodyTargetDto>,
    /// Model confidence.
    pub confidence: f64,
    /// `low_confidence`, `ambiguous`, `unknown`, `conflict`, `reply`.
    pub reason: String,
    /// The span stating it.
    pub quote: String,
}

/// A task proposal (`task`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TaskProposal {
    /// The decision.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// The note it came from.
    #[schema(value_type = String, format = "ulid")]
    pub source_note: Ulid,
    /// The block stating it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    /// Title.
    pub title: String,
    /// Due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// Recurrence phrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Reminders (local times).
    #[schema(value_type = Vec<String>)]
    pub reminders: Vec<NaiveDateTime>,
    /// Entities it concerns.
    #[schema(value_type = Vec<String>)]
    pub entities: Vec<Ulid>,
    /// Model confidence.
    pub confidence: f64,
}

/// One proposed fix of a correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CorrectionFixDto {
    /// The decision to fix.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// `repoint`, `retype`, `reject`.
    pub action: String,
    /// New target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub new_target: Option<Ulid>,
    /// New type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_type: Option<String>,
    /// Model confidence.
    pub confidence: f64,
    /// One sentence.
    pub reason: String,
}

/// A hint of a correction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HintDto {
    /// Entity.
    #[schema(value_type = String, format = "ulid")]
    pub entity: Ulid,
    /// Text.
    pub text: String,
}

/// A correction in words that was not applied automatically (`correction`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CorrectionProposal {
    /// The correction decision.
    #[schema(value_type = String, format = "ulid")]
    pub decision_id: Ulid,
    /// The user's words.
    pub message: String,
    /// Proposed fixes.
    pub fixes: Vec<CorrectionFixDto>,
    /// Hints to remember.
    pub hints: Vec<HintDto>,
    /// The model's question when ambiguous.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

/// The typed payload of an AI suggestion of `kind`, if it is one.
pub fn ai_payload(kind: &str, bytes: &[u8]) -> Option<AiPayload> {
    let ids = |v: Vec<NoteId>| v.into_iter().map(|n| n.as_ulid()).collect::<Vec<_>>();
    match kind {
        decide::KIND_FILING => rmp_serde::from_slice::<FilingPayload>(bytes).ok().map(|p| {
            AiPayload::Filing(FilingProposal {
                decision_id: p.decision_id.as_ulid(),
                title: p.title,
                tags: p.tags,
                folder: p.folder,
            })
        }),
        decide::KIND_ENTITY_LINK => {
            rmp_serde::from_slice::<EntityLinkPayload>(bytes).ok().map(|p| {
                AiPayload::EntityLink(EntityLinkProposal {
                    decision_id: p.decision_id.as_ulid(),
                    mention: p.mention,
                    entity_kind: p.kind,
                    source_note: p.source_note.as_ulid(),
                    block_id: p.block_id,
                    proposed: p.proposed.map(|n| n.as_ulid()),
                    candidates: ids(p.candidates),
                    is_nickname: p.is_nickname,
                    confidence: p.confidence,
                    reason: p.reason,
                })
            })
        }
        decide::KIND_CUSTODY => rmp_serde::from_slice::<CustodyPayload>(bytes).ok().map(|p| {
            AiPayload::Custody(CustodyProposal {
                decision_id: p.decision_id.as_ulid(),
                source_note: p.source_note.as_ulid(),
                block_id: p.block_id,
                event: p.event,
                date: p.date,
                document: p.document.into(),
                place: p.place.map(Into::into),
                place_part_of: p.place_part_of,
                person: p.person.map(Into::into),
                counterparty: p.counterparty.map(Into::into),
                confidence: p.confidence,
                reason: p.reason,
                quote: p.quote,
            })
        }),
        decide::KIND_TASK => rmp_serde::from_slice::<TaskPayload>(bytes).ok().map(|p| {
            AiPayload::Task(TaskProposal {
                decision_id: p.decision_id.as_ulid(),
                source_note: p.source_note.as_ulid(),
                block_id: p.block_id,
                title: p.title,
                due: p.due,
                recurrence: p.recurrence,
                reminders: p.reminders,
                entities: ids(p.entities),
                confidence: p.confidence,
            })
        }),
        decide::KIND_CORRECTION => {
            rmp_serde::from_slice::<CorrectionPayload>(bytes).ok().map(|p| {
                AiPayload::Correction(CorrectionProposal {
                    decision_id: p.decision_id.as_ulid(),
                    message: p.message,
                    fixes: p
                        .fixes
                        .into_iter()
                        .map(|f| CorrectionFixDto {
                            decision_id: f.decision_id.as_ulid(),
                            action: f.action,
                            new_target: f.new_target.map(|n| n.as_ulid()),
                            new_type: f.new_type,
                            confidence: f.confidence,
                            reason: f.reason,
                        })
                        .collect(),
                    hints: p
                        .hints
                        .into_iter()
                        .map(|h| HintDto {
                            entity: h.entity.as_ulid(),
                            text: h.text,
                        })
                        .collect(),
                    question: p.question,
                })
            })
        }
        _ => None,
    }
}

/// A decoded AI suggestion payload.
#[derive(Debug, Clone, PartialEq)]
pub enum AiPayload {
    /// `filing`.
    Filing(FilingProposal),
    /// `entity_link`.
    EntityLink(EntityLinkProposal),
    /// `custody`.
    Custody(CustodyProposal),
    /// `task`.
    Task(TaskProposal),
    /// `correction`.
    Correction(CorrectionProposal),
}

// ---- handlers ---------------------------------------------------------------------------------

fn ai_api(api: Option<&web::Data<AiApi>>) -> Result<&AiApi, Problem> {
    api.map(web::Data::get_ref).ok_or_else(|| {
        crate::routes::ai::unavailable_problem("the AI subsystem is not configured on this server")
    })
}

fn job_problem(e: &strata_jobs::JobError) -> Problem {
    tracing::error!(error = %e, "queueing an AI job failed");
    Problem::new(ProblemType::Internal)
}

/// The recent AI decisions, newest first (the activity feed, §9.8).
#[utoipa::path(
    get, path = "/ai-decisions", tag = "ai", operation_id = "list_ai_decisions",
    params(DecisionsQuery),
    responses((status = 200, description = "Decisions, newest first.", body = AiDecisions)),
)]
pub async fn list_decisions(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<DecisionsQuery>,
) -> Result<MsgPack<AiDecisions>, Problem> {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let items = vault
        .ai_decisions(auth.scope(), limit)
        .await
        .or_problem()?
        .into_iter()
        .map(AiDecision::from)
        .collect();
    Ok(MsgPack(AiDecisions { items }))
}

async fn correct(
    auth: &Authenticated,
    vault: &VaultService,
    id: Ulid,
    action: FixAction,
    hint: Option<String>,
) -> Result<MsgPack<CorrectionResult>, Problem> {
    let decision = DecisionId::from_ulid(id);
    let applied = vault
        .correct_decisions(
            auth.scope(),
            vec![DecisionFix {
                decision,
                action,
                hint,
            }],
            Vec::new(),
        )
        .await
        .or_problem()?;
    let commit = match applied {
        strata_vault::ops::ai_apply::AiApplied::Done { commit, .. } => commit,
        strata_vault::ops::ai_apply::AiApplied::Stale => None,
    };
    let view = vault.ai_decision(auth.scope(), decision).await.or_problem()?;
    Ok(MsgPack(CorrectionResult {
        decision: view.into(),
        commit,
    }))
}

/// Repoint an AI link, mention or pending entity-link suggestion ("this Ahmed is Ahmed
/// Fathy"): the old link is removed and recorded as rejected, the new one added, and a
/// disambiguation hint stored on the entity — one `user: repoint` commit.
#[utoipa::path(
    post, path = "/ai-decisions/{id}/repoint", tag = "ai", operation_id = "repoint_ai_decision",
    params(("id" = Ulid, Path, description = "Decision ID (ULID).")),
    request_body = RepointRequest,
    responses(
        (status = 200, description = "Corrected.", body = CorrectionResult),
        (status = 404, description = "`not_found`: no such decision or target.", body = Problem),
        (status = 422, description = "`invalid_body`: already corrected, a target of another kind, or a decision that cannot be repointed.", body = Problem),
    ),
)]
pub async fn repoint(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<RepointRequest>,
) -> Result<MsgPack<CorrectionResult>, Problem> {
    let b = body.into_inner();
    correct(
        &auth,
        &vault,
        *id,
        FixAction::Repoint(NoteId::from_ulid(b.target_id)),
        b.hint.filter(|h| !h.trim().is_empty()),
    )
    .await
}

/// Retype an AI relation (the old type is recorded as rejected).
#[utoipa::path(
    post, path = "/ai-decisions/{id}/retype", tag = "ai", operation_id = "retype_ai_decision",
    params(("id" = Ulid, Path, description = "Decision ID (ULID).")),
    request_body = RetypeRequest,
    responses(
        (status = 200, description = "Corrected.", body = CorrectionResult),
        (status = 404, description = "`not_found`: no such decision.", body = Problem),
        (status = 422, description = "`invalid_body`: unknown type, the same type, or not a relation.", body = Problem),
    ),
)]
pub async fn retype(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<RetypeRequest>,
) -> Result<MsgPack<CorrectionResult>, Problem> {
    let t = body.into_inner().new_type;
    let rel = RelationKey::all()
        .find(|k| k.as_str() == t)
        .ok_or_else(|| Problem::new(ProblemType::InvalidBody).with_detail("unknown relation type"))?;
    correct(&auth, &vault, *id, FixAction::Retype(rel), None).await
}

/// Reject an AI decision: the link, mention or custody event is removed and never re-added;
/// a pending suggestion is rejected.
#[utoipa::path(
    post, path = "/ai-decisions/{id}/reject", tag = "ai", operation_id = "reject_ai_decision",
    params(("id" = Ulid, Path, description = "Decision ID (ULID).")),
    responses(
        (status = 200, description = "Corrected.", body = CorrectionResult),
        (status = 404, description = "`not_found`: no such decision.", body = Problem),
        (status = 422, description = "`invalid_body`: already corrected.", body = Problem),
    ),
)]
pub async fn reject(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<CorrectionResult>, Problem> {
    correct(&auth, &vault, *id, FixAction::Reject, None).await
}

/// Accept an AI suggestion with edits (filing: title/tags/folder; entity link: the entity
/// or the new entity's name and aliases; custody: the participant; tasks: text, due,
/// recurrence, reminders) — one commit.
#[utoipa::path(
    post, path = "/suggestions/{id}/accept-with-edits", tag = "inbox",
    operation_id = "accept_suggestion_with_edits",
    params(("id" = Ulid, Path, description = "Suggestion ID (ULID).")),
    request_body = AcceptWithEditsRequest,
    responses(
        (status = 200, description = "Accepted.", body = Suggestion),
        (status = 404, description = "`not_found`: no such suggestion or target.", body = Problem),
        (status = 422, description = "`invalid_body`: already decided, edits not supported for the kind, or an invalid edit.", body = Problem),
    ),
)]
pub async fn accept_with_edits(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<AcceptWithEditsRequest>,
) -> Result<MsgPack<Suggestion>, Problem> {
    let edits: sync_model::ops::SuggestionEdits = body.into_inner().edits.into();
    let v = vault
        .decide_suggestion_with(
            auth.scope(),
            SuggestionId::from_ulid(*id),
            true,
            Some(edits),
        )
        .await
        .or_problem()?;
    Ok(MsgPack(v.into()))
}

/// Refresh an entity's AI sections now (Summary, Insights, Open items, Timeline).
#[utoipa::path(
    post, path = "/entities/{id}/refresh", tag = "entities", operation_id = "refresh_entity",
    params(("id" = Ulid, Path, description = "Entity ID (ULID).")),
    responses(
        (status = 202, description = "Queued; `job.completed` follows.", body = JobQueued),
        (status = 404, description = "`not_found`: no such entity.", body = Problem),
        (status = 503, description = "`ai_unavailable`.", body = Problem),
    ),
)]
pub async fn refresh_entity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    api: Option<web::Data<AiApi>>,
    id: web::Path<Ulid>,
) -> Result<impl Responder, Problem> {
    let note = NoteId::from_ulid(*id);
    let view = vault.note(auth.scope(), note).await.or_problem()?;
    if view.trashed || strata_vault::ops::entities::entity_kind(view.kind).is_none() {
        return Err(Problem::new(ProblemType::NotFound));
    }
    let api = ai_api(api.as_ref())?;
    let job = strata_jobs::insights::enqueue_refresh(
        &api.db,
        auth.scope(),
        api.ids.as_ref(),
        note,
        api.clock.now(),
    )
    .await
    .map_err(|e| job_problem(&e))?;
    Ok(MsgPack(JobQueued {
        job_id: job.as_ulid(),
    })
    .customize()
    .with_status(StatusCode::ACCEPTED))
}

/// Link a note again now, even if it was linked at this version.
#[utoipa::path(
    post, path = "/notes/{id}/relink", tag = "ai", operation_id = "relink_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses(
        (status = 202, description = "Queued; `job.completed` follows.", body = JobQueued),
        (status = 404, description = "`not_found`: no such note.", body = Problem),
        (status = 503, description = "`ai_unavailable`.", body = Problem),
    ),
)]
pub async fn relink(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    api: Option<web::Data<AiApi>>,
    id: web::Path<Ulid>,
) -> Result<impl Responder, Problem> {
    let note = NoteId::from_ulid(*id);
    let view = vault.note(auth.scope(), note).await.or_problem()?;
    if view.trashed {
        return Err(Problem::new(ProblemType::NotFound));
    }
    let api = ai_api(api.as_ref())?;
    let kind = if view.path.starts_with("inbox/") {
        strata_jobs::pipeline::FILE_INBOX
    } else {
        strata_jobs::pipeline::LINK
    };
    let job = strata_jobs::link::enqueue_forced(
        &api.db,
        auth.scope(),
        api.ids.as_ref(),
        kind,
        note,
        api.clock.now(),
    )
    .await
    .map_err(|e| job_problem(&e))?;
    Ok(MsgPack(JobQueued {
        job_id: job.as_ulid(),
    })
    .customize()
    .with_status(StatusCode::ACCEPTED))
}

async fn read_settings(api: &AiApi, auth: &Authenticated) -> Result<AiSettings, Problem> {
    let auto_file = strata_jobs::file_inbox::auto_file(&api.db, auth.scope())
        .await
        .map_err(|e| job_problem(&e))?;
    Ok(AiSettings { auto_file })
}

/// The user's AI settings.
#[utoipa::path(
    get, path = "/ai/settings", tag = "ai", operation_id = "get_ai_settings",
    responses(
        (status = 200, description = "Settings.", body = AiSettings),
        (status = 503, description = "`ai_unavailable`.", body = Problem),
    ),
)]
pub async fn get_settings(
    auth: Authenticated,
    api: Option<web::Data<AiApi>>,
) -> Result<MsgPack<AiSettings>, Problem> {
    let api = ai_api(api.as_ref())?;
    Ok(MsgPack(read_settings(api, &auth).await?))
}

/// Change the user's AI settings.
#[utoipa::path(
    put, path = "/ai/settings", tag = "ai", operation_id = "put_ai_settings",
    request_body = AiSettings,
    responses(
        (status = 200, description = "Saved.", body = AiSettings),
        (status = 503, description = "`ai_unavailable`.", body = Problem),
    ),
)]
pub async fn put_settings(
    auth: Authenticated,
    api: Option<web::Data<AiApi>>,
    body: MsgPack<AiSettings>,
) -> Result<MsgPack<AiSettings>, Problem> {
    let api = ai_api(api.as_ref())?;
    let value = rmp_serde::to_vec(&body.auto_file).map_err(|_| Problem::new(ProblemType::Internal))?;
    let mut tx = api.db.begin(auth.scope()).await.or_problem()?;
    strata_index::repo::settings::put_setting(
        &mut tx,
        strata_jobs::file_inbox::AUTO_FILE_SETTING,
        &value,
        api.clock.now(),
    )
    .await
    .or_problem()?;
    tx.commit().await.or_problem()?;
    Ok(MsgPack(read_settings(api, &auth).await?))
}

/// Mounts the routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/ai-decisions", web::get().to(list_decisions))
        .route("/ai-decisions/{id}/repoint", web::post().to(repoint))
        .route("/ai-decisions/{id}/retype", web::post().to(retype))
        .route("/ai-decisions/{id}/reject", web::post().to(reject))
        .route(
            "/suggestions/{id}/accept-with-edits",
            web::post().to(accept_with_edits),
        )
        .route("/entities/{id}/refresh", web::post().to(refresh_entity))
        .route("/notes/{id}/relink", web::post().to(relink))
        .route("/ai/settings", web::get().to(get_settings))
        .route("/ai/settings", web::put().to(put_settings));
}

/// The AI pipelines' part of the contract (merged into the production document).
#[derive(Debug, utoipa::OpenApi)]
#[openapi(
    paths(
        list_decisions,
        repoint,
        retype,
        reject,
        accept_with_edits,
        refresh_entity,
        relink,
        get_settings,
        put_settings
    ),
    components(schemas(
        FilingProposal,
        EntityLinkProposal,
        CustodyProposal,
        CustodyTargetDto,
        TaskProposal,
        CorrectionProposal,
        CorrectionFixDto,
        HintDto
    ))
)]
pub struct AiPipelinesApiDoc;
