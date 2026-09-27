//! Relations (PLAN §7.5 Relations, §6.4, §6.5): add (by: user), retype, remove. Removing or
//! retyping an AI edge records a rejection (sidecar + `rejected`), so the AI never re-adds
//! it.

use actix_web::http::StatusCode;
use actix_web::{Responder, web};
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_vault::VaultService;
use ulid::Ulid;
use utoipa::ToSchema;
use vault_format::RelationKey;

use crate::auth::Authenticated;
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem, ProblemFieldError, ProblemType};

/// Identifies one edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RelationRef {
    /// Source note (the edge is stored in its frontmatter).
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target note.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Relation type: `related`, `part-of`, `supports`, `contradicts`, `follows-up`,
    /// `duplicates`, `concepts`, `people`, `companies`, an entity relation (`works-at`, …) or
    /// `copy-of`.
    #[serde(rename = "type")]
    pub rel_type: String,
}

/// `PATCH /relations`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RetypeRelation {
    /// Source note.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target note.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Current type.
    #[serde(rename = "type")]
    pub rel_type: String,
    /// New type.
    pub new_type: String,
}

/// An edge after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RelationResult {
    /// Source note.
    #[schema(value_type = String, format = "ulid")]
    pub src_id: Ulid,
    /// Target note.
    #[schema(value_type = String, format = "ulid")]
    pub dst_id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub rel_type: String,
    /// `POST`: false if it already existed. `DELETE`: true if it was an AI edge and is now
    /// recorded as rejected.
    pub changed: bool,
}

fn relation_key(pointer: &str, value: &str) -> Result<RelationKey, Problem> {
    value.parse::<RelationKey>().map_err(|_| {
        let message = "unknown relation type";
        Problem::new(ProblemType::InvalidBody)
            .with_detail(message)
            .with_error(ProblemFieldError {
                code: "unknown_relation_type".to_owned(),
                pointer: Some(pointer.to_owned()),
                message: message.to_owned(),
            })
    })
}

/// Add a user edge (no-op if it exists).
#[utoipa::path(
    post, path = "/relations", tag = "relations", operation_id = "add_relation",
    request_body = RelationRef,
    responses(
        (status = 201, description = "Added; one `user: relation add <path>` commit.", body = RelationResult),
        (status = 404, description = "`not_found`: a note is not in the caller's vault.", body = Problem),
    ),
)]
pub async fn add_relation(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<RelationRef>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let rel = relation_key("/type", &b.rel_type)?;
    let added = vault
        .add_relation(
            auth.scope(),
            NoteId::from_ulid(b.src_id),
            NoteId::from_ulid(b.dst_id),
            rel,
        )
        .await
        .or_problem()?;
    Ok(MsgPack(RelationResult {
        src_id: b.src_id,
        dst_id: b.dst_id,
        rel_type: rel.as_str().to_owned(),
        changed: added,
    })
    .customize()
    .with_status(StatusCode::CREATED))
}

/// Change an edge's type (a retyped AI edge is rejected under its old type).
#[utoipa::path(
    patch, path = "/relations", tag = "relations", operation_id = "retype_relation",
    request_body = RetypeRelation,
    responses(
        (status = 200, description = "Retyped; one `user: relation retype <path>` commit.", body = RelationResult),
        (status = 404, description = "`not_found`: no such edge in the caller's vault.", body = Problem),
    ),
)]
pub async fn retype_relation(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<RetypeRelation>,
) -> Result<MsgPack<RelationResult>, Problem> {
    let b = body.into_inner();
    let rel = relation_key("/type", &b.rel_type)?;
    let new_rel = relation_key("/new_type", &b.new_type)?;
    vault
        .retype_relation(
            auth.scope(),
            NoteId::from_ulid(b.src_id),
            NoteId::from_ulid(b.dst_id),
            rel,
            new_rel,
        )
        .await
        .or_problem()?;
    Ok(MsgPack(RelationResult {
        src_id: b.src_id,
        dst_id: b.dst_id,
        rel_type: new_rel.as_str().to_owned(),
        changed: true,
    }))
}

/// Remove an edge; an AI edge is recorded as rejected and never re-added.
#[utoipa::path(
    delete, path = "/relations", tag = "relations", operation_id = "remove_relation",
    request_body = RelationRef,
    responses(
        (status = 200, description = "Removed; one `user: relation remove <path>` commit.", body = RelationResult),
        (status = 404, description = "`not_found`: no such edge in the caller's vault.", body = Problem),
    ),
)]
pub async fn remove_relation(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<RelationRef>,
) -> Result<MsgPack<RelationResult>, Problem> {
    let b = body.into_inner();
    let rel = relation_key("/type", &b.rel_type)?;
    let was_ai = vault
        .remove_relation(
            auth.scope(),
            NoteId::from_ulid(b.src_id),
            NoteId::from_ulid(b.dst_id),
            rel,
        )
        .await
        .or_problem()?;
    Ok(MsgPack(RelationResult {
        src_id: b.src_id,
        dst_id: b.dst_id,
        rel_type: rel.as_str().to_owned(),
        changed: was_ai,
    }))
}

/// Mounts the relation routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/relations")
            .route(web::post().to(add_relation))
            .route(web::patch().to(retype_relation))
            .route(web::delete().to(remove_relation)),
    );
}
