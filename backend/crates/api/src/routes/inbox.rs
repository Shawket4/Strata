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
use strata_vault::ops::suggestions::{Payload, SuggestionView};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
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
    /// A kind this version does not describe (MessagePack as stored).
    Opaque {
        /// The raw payload.
        data: Binary,
    },
}

/// Who wrote a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplyAuthorDto {
    /// The user.
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

impl From<SuggestionView> for Suggestion {
    fn from(v: SuggestionView) -> Self {
        let payload = match v.payload() {
            Payload::Duplicate(d) => SuggestionPayload::Duplicate {
                candidates: d
                    .candidates
                    .iter()
                    .filter_map(|c| {
                        Some(DuplicateCandidate {
                            id: c.id.parse().ok()?,
                            kind: c.kind.clone(),
                            title: c.title.clone(),
                            snippet: c.snippet.clone(),
                            match_level: if c.match_level == "exact" {
                                MatchLevel::Exact
                            } else {
                                MatchLevel::Near
                            },
                            score: c.score,
                        })
                    })
                    .collect(),
            },
            Payload::Opaque(data) => SuggestionPayload::Opaque { data: Binary(data) },
        };
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
    responses((status = 201, description = "Saved; one `user: capture <path>` commit.", body = Capture)),
)]
pub async fn capture(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CaptureRequest>,
) -> Result<impl Responder, Problem> {
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
