//! Capture, inbox and suggestions (PLAN §6.9, §7.5 Inbox & capture, Suggestions, §9.7).
//!
//! `POST /capture` is never refused as a duplicate (principle 5): the capture is saved (and
//! committed) first; likely duplicates become a `duplicate` suggestion on the inbox note.

use actix_web::http::StatusCode;
use actix_web::{Responder, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::SuggestionId;
use strata_index::types::{ReplyAuthor, SuggestionStatus as Status};
use strata_vault::VaultService;
use strata_vault::ops::suggestions::SuggestionView;
use sync_model::suggestions as shared;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::{AuthState, Authenticated};
use crate::routes::ai_pipelines::{CorrectionFixDto, CustodyTargetDto, HintDto};
use crate::routes::notes::Note;
use crate::vault::{OrProblem, candidate};
use crate::wire::{Binary, DuplicateCandidate, MatchLevel, MsgPack, MsgPackConfig, Problem};

/// `POST /capture`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CaptureRequest {
    /// The captured text (saved verbatim as the note body).
    pub text: String,
}

/// A saved capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Capture {
    /// The inbox note (`inbox/YYYY-MM-DD-HHmmss.md`).
    pub note: Note,
    /// Items it resembles (it was saved anyway).
    pub duplicates: Vec<DuplicateCandidate>,
    /// The `duplicate` suggestion created for them, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub suggestion_id: Option<Ulid>,
}

/// Suggestion status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    /// Waiting for the user.
    #[default]
    Pending,
    /// Accepted.
    Accepted,
    /// Rejected.
    Rejected,
    /// Replaced by a re-proposal after a reply.
    Superseded,
}

impl From<Status> for SuggestionStatus {
    fn from(s: Status) -> Self {
        match s {
            Status::Pending => Self::Pending,
            Status::Accepted => Self::Accepted,
            Status::Rejected => Self::Rejected,
            Status::Superseded => Self::Superseded,
        }
    }
}

impl From<SuggestionStatus> for Status {
    fn from(s: SuggestionStatus) -> Self {
        match s {
            SuggestionStatus::Pending => Self::Pending,
            SuggestionStatus::Accepted => Self::Accepted,
            SuggestionStatus::Rejected => Self::Rejected,
            SuggestionStatus::Superseded => Self::Superseded,
        }
    }
}

/// What a suggestion proposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionPayload {
    /// The note resembles existing items; accepting keeps both.
    Duplicate {
        /// The items it resembles.
        candidates: Vec<DuplicateCandidate>,
    },
    /// Two stored items the nightly sweep found to be duplicates (`duplicates`, PLAN §9.7);
    /// rejecting keeps both for good. Never merged automatically.
    Duplicates {
        /// The first item.
        a: Box<DuplicateCandidate>,
        /// The second item.
        b: Box<DuplicateCandidate>,
        /// Why the model confirmed a borderline pair.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A filing proposal for an inbox capture (`filing`, §9.3): accepting (optionally with
    /// edits) titles, tags and moves the capture in one commit.
    Filing {
        /// The AI decision.
        #[schema(value_type = String, format = "ulid")]
        decision_id: Ulid,
        /// Title (file name).
        title: String,
        /// Tags to add.
        tags: Vec<String>,
        /// Destination folder.
        folder: String,
    },
    /// Link a mention to an entity, or create it (`entity_link`, §6.7, D13 = b): accepting
    /// adds the mention to the entity's aliases.
    EntityLink {
        /// The AI decision.
        #[schema(value_type = String, format = "ulid")]
        decision_id: Ulid,
        /// The mention as written.
        mention: String,
        /// `person`, `company`, `document`, `place`.
        entity_kind: String,
        /// The note mentioning it.
        #[schema(value_type = String, format = "ulid")]
        source_note: Ulid,
        /// The block stating it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        /// The proposed entity (absent: create a new one).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<String>, format = "ulid")]
        proposed: Option<Ulid>,
        /// Entities that fit.
        #[schema(value_type = Vec<String>)]
        candidates: Vec<Ulid>,
        /// A nickname or kinship term (never created automatically).
        is_nickname: bool,
        /// Model confidence.
        confidence: f64,
        /// `ambiguous`, `nickname`, `new`, `low_confidence`, `reply`.
        reason: String,
    },
    /// A custody event not applied automatically (`custody`, §6.12, D30).
    Custody {
        /// The AI decision.
        #[schema(value_type = String, format = "ulid")]
        decision_id: Ulid,
        /// The note stating it.
        #[schema(value_type = String, format = "ulid")]
        source_note: Ulid,
        /// The block stating it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        /// Event type (`stored-at`, …).
        event: String,
        /// Date (resolved).
        date: chrono::NaiveDate,
        /// The document.
        document: Box<CustodyTargetDto>,
        /// The place.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        place: Option<Box<CustodyTargetDto>>,
        /// The enclosing place mention.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        place_part_of: Option<String>,
        /// The person.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        person: Option<Box<CustodyTargetDto>>,
        /// The third party.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counterparty: Option<Box<CustodyTargetDto>>,
        /// Model confidence.
        confidence: f64,
        /// `low_confidence`, `ambiguous`, `unknown`, `conflict`, `reply`.
        reason: String,
        /// The span stating it.
        quote: String,
    },
    /// A task proposed from a note (`task`, §6.11): accepting writes the line.
    Task {
        /// The AI decision.
        #[schema(value_type = String, format = "ulid")]
        decision_id: Ulid,
        /// The note it came from.
        #[schema(value_type = String, format = "ulid")]
        source_note: Ulid,
        /// The block stating it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        /// Title.
        title: String,
        /// Due date.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        due: Option<chrono::NaiveDate>,
        /// Recurrence phrase (Tasks plugin language).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recurrence: Option<String>,
        /// Reminders (local times).
        #[schema(value_type = Vec<String>)]
        reminders: Vec<chrono::NaiveDateTime>,
        /// Entities it concerns.
        #[schema(value_type = Vec<String>)]
        entities: Vec<Ulid>,
        /// Model confidence.
        confidence: f64,
    },
    /// A correction in words not applied automatically (`correction`, §9.8).
    Correction {
        /// The correction decision.
        #[schema(value_type = String, format = "ulid")]
        decision_id: Ulid,
        /// The user's words.
        message: String,
        /// Proposed fixes.
        fixes: Vec<CorrectionFixDto>,
        /// Hints to remember.
        hints: Vec<HintDto>,
        /// The model's question when ambiguous.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question: Option<String>,
    },
    /// A kind this version does not describe (MessagePack as stored).
    Opaque {
        /// The raw payload.
        data: Binary,
    },
}

/// Who wrote a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplyAuthorDto {
    /// The user.
    #[default]
    User,
    /// The AI.
    Ai,
}

/// One reply in a suggestion thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SuggestionReply {
    /// Reply ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Author.
    pub author: ReplyAuthorDto,
    /// Text.
    pub body: String,
    /// When.
    pub created: DateTime<Utc>,
}

/// A suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Suggestion {
    /// Suggestion ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// The note it concerns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub note_id: Option<Ulid>,
    /// Kind (`duplicate`; AI kinds from Phase 4).
    pub kind: String,
    /// Status.
    pub status: SuggestionStatus,
    /// What it proposes.
    pub payload: SuggestionPayload,
    /// Thread.
    pub replies: Vec<SuggestionReply>,
    /// Created.
    pub created: DateTime<Utc>,
    /// Last change.
    pub updated: DateTime<Utc>,
    /// Accepted/rejected at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<DateTime<Utc>>,
}

const fn level_of(level: dedupe::MatchLevel) -> MatchLevel {
    match level {
        dedupe::MatchLevel::Exact => MatchLevel::Exact,
        dedupe::MatchLevel::Near => MatchLevel::Near,
        dedupe::MatchLevel::Semantic => MatchLevel::Semantic,
    }
}

fn candidate_of(c: shared::DuplicateItem) -> DuplicateCandidate {
    DuplicateCandidate {
        id: c.id,
        kind: c.kind,
        title: c.title,
        snippet: c.snippet,
        match_level: level_of(c.match_level),
        score: c.score,
    }
}

fn target_of(t: shared::CustodyTarget) -> Box<CustodyTargetDto> {
    Box::new(CustodyTargetDto {
        mention: t.mention,
        id: t.id,
        candidates: t.candidates,
    })
}

impl SuggestionPayload {
    /// The contract's view of the stored payload of a suggestion of `kind` (the shared
    /// `sync_model::suggestions` types, L16). Kinds the contract does not describe
    /// (`conflict`, kinds from a newer server) and undecodable bytes are `opaque`.
    pub fn from_stored(kind: &str, bytes: &[u8]) -> Self {
        shared::SuggestionPayload::decode(kind, bytes)
            .ok()
            .and_then(Self::from_shared)
            .unwrap_or_else(|| Self::Opaque {
                data: Binary(bytes.to_vec()),
            })
    }

    /// The contract's view of a shared payload (`None`: shown as `opaque`).
    fn from_shared(p: shared::SuggestionPayload) -> Option<Self> {
        use shared::SuggestionPayload as P;
        Some(match p {
            P::Duplicate(p) => Self::Duplicate {
                candidates: p.candidates.into_iter().map(candidate_of).collect(),
            },
            P::Duplicates(p) => Self::Duplicates {
                a: Box::new(candidate_of(p.a)),
                b: Box::new(candidate_of(p.b)),
                reason: p.reason,
            },
            P::Filing(p) => Self::Filing {
                decision_id: p.decision_id,
                title: p.title,
                tags: p.tags,
                folder: p.folder,
            },
            P::EntityLink(p) => Self::EntityLink {
                decision_id: p.decision_id,
                mention: p.mention,
                entity_kind: p.kind,
                source_note: p.source_note,
                block_id: p.block_id,
                proposed: p.proposed,
                candidates: p.candidates,
                is_nickname: p.is_nickname,
                confidence: p.confidence,
                reason: p.reason,
            },
            P::Custody(p) => Self::Custody {
                decision_id: p.decision_id,
                source_note: p.source_note,
                block_id: p.block_id,
                event: p.event,
                date: p.date,
                document: target_of(p.document),
                place: p.place.map(target_of),
                place_part_of: p.place_part_of,
                person: p.person.map(target_of),
                counterparty: p.counterparty.map(target_of),
                confidence: p.confidence,
                reason: p.reason,
                quote: p.quote,
            },
            P::Task(p) => Self::Task {
                decision_id: p.decision_id,
                source_note: p.source_note,
                block_id: p.block_id,
                title: p.title,
                due: p.due,
                recurrence: p.recurrence,
                reminders: p.reminders,
                entities: p.entities,
                confidence: p.confidence,
            },
            P::Correction(p) => Self::Correction {
                decision_id: p.decision_id,
                message: p.message,
                fixes: p
                    .fixes
                    .into_iter()
                    .map(|f| CorrectionFixDto {
                        decision_id: f.decision_id,
                        action: f.action,
                        new_target: f.new_target,
                        new_type: f.new_type,
                        confidence: f.confidence,
                        reason: f.reason,
                    })
                    .collect(),
                hints: p
                    .hints
                    .into_iter()
                    .map(|h| HintDto {
                        entity: h.entity,
                        text: h.text,
                    })
                    .collect(),
                question: p.question,
            },
            // The conflict copy is a note of its own; the contract shows the payload opaque.
            P::Conflict(_) => return None,
        })
    }
}

impl From<SuggestionView> for Suggestion {
    fn from(v: SuggestionView) -> Self {
        let payload = SuggestionPayload::from_stored(&v.suggestion.kind, &v.suggestion.payload);
        let s = v.suggestion;
        Self {
            id: s.id.as_ulid(),
            note_id: s.note_id.map(|n| n.as_ulid()),
            kind: s.kind,
            status: s.status.into(),
            payload,
            replies: v
                .replies
                .into_iter()
                .map(|r| SuggestionReply {
                    id: r.id.as_ulid(),
                    author: match r.author {
                        ReplyAuthor::User => ReplyAuthorDto::User,
                        ReplyAuthor::Ai => ReplyAuthorDto::Ai,
                    },
                    body: r.body,
                    created: r.created,
                })
                .collect(),
            created: s.created,
            updated: s.updated,
            decided_at: s.decided_at,
        }
    }
}

/// Suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Suggestions {
    /// Oldest first.
    pub items: Vec<Suggestion>,
}

/// One inbox note with its pending suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct InboxItem {
    /// The inbox note.
    pub note: Note,
    /// Its pending suggestions.
    pub suggestions: Vec<Suggestion>,
}

/// The inbox, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Inbox {
    /// Items.
    pub items: Vec<InboxItem>,
}

/// `GET /suggestions` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SuggestionsQuery {
    /// Status (default `pending`).
    #[serde(default)]
    pub status: Option<SuggestionStatus>,
}

/// `POST /suggestions/{id}/reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReplyRequest {
    /// The reply ("no, the Petrol Arrows one").
    pub body: String,
}

/// Capture text into the inbox (saved before anything else, never refused as a duplicate).
#[utoipa::path(
    post, path = "/capture", tag = "inbox", operation_id = "capture",
    request_body = CaptureRequest,
    responses(
        (status = 201, description = "Saved; one `user: capture <path>` commit.", body = Capture),
        (status = 429, description = "`rate_limited`: too many captures from this user (`auth.rate_limits.capture_per_user`). See `Retry-After`.", body = Problem),
    ),
)]
pub async fn capture(
    auth: Authenticated,
    state: web::Data<AuthState>,
    vault: web::Data<VaultService>,
    body: MsgPack<CaptureRequest>,
) -> Result<impl Responder, actix_web::Error> {
    state.check_capture_limit(auth.user_id())?;
    let c = vault
        .capture(auth.scope(), body.into_inner().text)
        .await
        .or_problem()?;
    Ok(MsgPack(Capture {
        note: c.note.into(),
        duplicates: c.duplicates.iter().map(candidate).collect(),
        suggestion_id: c.suggestion.map(|s| s.as_ulid()),
    })
    .customize()
    .with_status(StatusCode::CREATED))
}

/// Inbox notes, newest first, with their pending suggestions.
#[utoipa::path(
    get, path = "/inbox", tag = "inbox", operation_id = "get_inbox",
    responses((status = 200, description = "The inbox.", body = Inbox)),
)]
pub async fn inbox(
    auth: Authenticated,
    vault: web::Data<VaultService>,
) -> Result<MsgPack<Inbox>, Problem> {
    let items = vault.inbox(auth.scope()).await.or_problem()?;
    let out = items
        .into_iter()
        .map(|(note, suggestions)| InboxItem {
            note: note.into(),
            suggestions: suggestions.into_iter().map(Suggestion::from).collect(),
        })
        .collect();
    Ok(MsgPack(Inbox { items: out }))
}

/// Suggestions with a status (default pending), oldest first.
#[utoipa::path(
    get, path = "/suggestions", tag = "inbox", operation_id = "list_suggestions",
    params(SuggestionsQuery),
    responses((status = 200, description = "Suggestions.", body = Suggestions)),
)]
pub async fn list_suggestions(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<SuggestionsQuery>,
) -> Result<MsgPack<Suggestions>, Problem> {
    let status = q.status.unwrap_or_default().into();
    let items = vault.suggestions(auth.scope(), status).await.or_problem()?;
    Ok(MsgPack(Suggestions {
        items: items.into_iter().map(Suggestion::from).collect(),
    }))
}

/// Accept a pending suggestion (a `duplicate` suggestion: keep both).
#[utoipa::path(
    post, path = "/suggestions/{id}/accept", tag = "inbox", operation_id = "accept_suggestion",
    params(("id" = Ulid, Path, description = "Suggestion ID (ULID).")),
    responses(
        (status = 200, description = "Accepted.", body = Suggestion),
        (status = 422, description = "`invalid_body`: already decided.", body = Problem),
    ),
)]
pub async fn accept_suggestion(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Suggestion>, Problem> {
    let v = vault
        .decide_suggestion(auth.scope(), SuggestionId::from_ulid(*id), true)
        .await
        .or_problem()?;
    Ok(MsgPack(v.into()))
}

/// Reject a pending suggestion.
#[utoipa::path(
    post, path = "/suggestions/{id}/reject", tag = "inbox", operation_id = "reject_suggestion",
    params(("id" = Ulid, Path, description = "Suggestion ID (ULID).")),
    responses(
        (status = 200, description = "Rejected.", body = Suggestion),
        (status = 422, description = "`invalid_body`: already decided.", body = Problem),
    ),
)]
pub async fn reject_suggestion(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Suggestion>, Problem> {
    let v = vault
        .decide_suggestion(auth.scope(), SuggestionId::from_ulid(*id), false)
        .await
        .or_problem()?;
    Ok(MsgPack(v.into()))
}

/// Reply to a pending suggestion (the AI re-proposes once the AI subsystem exists).
#[utoipa::path(
    post, path = "/suggestions/{id}/reply", tag = "inbox", operation_id = "reply_suggestion",
    params(("id" = Ulid, Path, description = "Suggestion ID (ULID).")),
    request_body = ReplyRequest,
    responses((status = 200, description = "The suggestion with the reply.", body = Suggestion)),
)]
pub async fn reply_suggestion(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<ReplyRequest>,
) -> Result<MsgPack<Suggestion>, Problem> {
    let v = vault
        .reply_suggestion(
            auth.scope(),
            SuggestionId::from_ulid(*id),
            body.into_inner().body,
        )
        .await
        .or_problem()?;
    Ok(MsgPack(v.into()))
}

/// Mounts the capture, inbox and suggestion routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/capture")
            .app_data(MsgPackConfig::default().with_body_limit(1024 * 1024))
            .route(web::post().to(capture)),
    )
    .route("/inbox", web::get().to(inbox))
    .route("/suggestions", web::get().to(list_suggestions))
    .route(
        "/suggestions/{id}/accept",
        web::post().to(accept_suggestion),
    )
    .route(
        "/suggestions/{id}/reject",
        web::post().to(reject_suggestion),
    )
    .route("/suggestions/{id}/reply", web::post().to(reply_suggestion));
}
