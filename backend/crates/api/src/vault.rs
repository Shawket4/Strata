//! The vault store behind the API (PLAN §7.2–§7.5): the [`VaultProvisioner`] implementation
//! of `strata_vault::VaultService`, the mapping of vault errors to problem details, the
//! vault routes and their part of the OpenAPI contract ([`VaultApiDoc`], merged into
//! [`crate::openapi::ApiDoc`]).
//!
//! Handlers take the caller's [`crate::auth::Authenticated`] scope and a
//! `web::Data<VaultService>` registered by the composition root.

use std::io;
use std::path::PathBuf;

use actix_web::http::header::HeaderMap;
use actix_web::web;
use futures_util::future::BoxFuture;
use strata_common::UserId;
use strata_vault::{Candidate, MatchLevel as VMatch, VaultError, VaultService};
use utoipa::OpenApi;

use crate::auth::VaultProvisioner;
use crate::routes;
use crate::wire::{DuplicateCandidate, MatchLevel, Problem, ProblemFieldError, ProblemType};

impl VaultProvisioner for VaultService {
    fn provision(&self, user: UserId) -> BoxFuture<'_, io::Result<bool>> {
        Box::pin(VaultService::provision(self, user))
    }

    fn deprovision(&self, user: UserId) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(VaultService::deprovision(self, user))
    }

    fn vault_dir(&self, user: UserId) -> PathBuf {
        VaultService::vault_dir(self, user)
    }
}

/// A duplicate candidate on the wire.
pub fn candidate(c: &Candidate) -> DuplicateCandidate {
    DuplicateCandidate {
        id: c.id,
        kind: c.kind.clone(),
        title: c.title.clone(),
        snippet: c.snippet.clone(),
        match_level: match c.level {
            VMatch::Exact => MatchLevel::Exact,
            VMatch::Near => MatchLevel::Near,
        },
        score: c.score,
    }
}

/// The problem for a vault error.
pub fn problem(err: &VaultError) -> Problem {
    match err {
        VaultError::VersionConflict { current } => Problem::version_conflict(current.clone()),
        VaultError::Duplicate(candidates) => {
            Problem::duplicate_candidates(candidates.iter().map(candidate).collect())
        }
        VaultError::AiUnavailable => Problem::new(ProblemType::AiUnavailable).with_detail(
            "semantic and hybrid search need the AI subsystem, which is not available yet; use mode=keyword",
        ),
        other => Problem::from_domain(other),
    }
}

/// Converts vault results into handler results.
pub trait OrProblem<T> {
    /// Maps the error to problem details.
    fn or_problem(self) -> Result<T, Problem>;
}

impl<T> OrProblem<T> for Result<T, VaultError> {
    fn or_problem(self) -> Result<T, Problem> {
        self.map_err(|e| problem(&e))
    }
}

impl<T> OrProblem<T> for Result<T, strata_index::IndexError> {
    fn or_problem(self) -> Result<T, Problem> {
        self.map_err(|e| problem(&VaultError::Index(e)))
    }
}

/// The `If-Match` header (quotes of an entity-tag are stripped).
pub fn if_match(headers: &HeaderMap) -> Result<Option<String>, Problem> {
    let Some(value) = headers.get(actix_web::http::header::IF_MATCH) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| {
        invalid_parameter("If-Match", "invalid_header", "If-Match is not visible ASCII")
    })?;
    let v = value.trim().trim_start_matches("W/").trim_matches('"');
    if v.is_empty() {
        return Err(invalid_parameter("If-Match", "invalid_header", "If-Match is empty"));
    }
    Ok(Some(v.to_owned()))
}

/// The `If-Match` header, required.
pub fn required_if_match(headers: &HeaderMap) -> Result<String, Problem> {
    if_match(headers)?.ok_or_else(|| {
        invalid_parameter("If-Match", "missing_header", "If-Match with the current version is required")
    })
}

/// `422 invalid_parameter` for one parameter.
pub fn invalid_parameter(name: &str, code: &str, message: &str) -> Problem {
    Problem::new(ProblemType::InvalidParameter)
        .with_detail(message)
        .with_error(ProblemFieldError {
            code: code.to_owned(),
            pointer: Some(name.to_owned()),
            message: message.to_owned(),
        })
}

/// Mounts every vault route (relative to `/api/v1`).
pub fn configure(cfg: &mut web::ServiceConfig) {
    routes::notes::configure(cfg);
    routes::search::configure(cfg);
    routes::inbox::configure(cfg);
    routes::relations::configure(cfg);
    routes::entities::configure(cfg);
    routes::documents::configure(cfg);
    routes::tasks::configure(cfg);
    routes::vault_ops::configure(cfg);
}

/// The vault part of the contract (merged into the production document).
#[derive(Debug, OpenApi)]
#[openapi(
    paths(
        routes::notes::tree,
        routes::notes::create_note,
        routes::notes::get_note,
        routes::notes::get_note_by_path,
        routes::notes::update_note,
        routes::notes::move_note,
        routes::notes::delete_note,
        routes::notes::restore_note,
        routes::notes::purge_note,
        routes::notes::backlinks,
        routes::notes::note_history,
        routes::notes::note_revision,
        routes::notes::revert_note,
        routes::notes::revert_commit,
        routes::search::search,
        routes::inbox::capture,
        routes::inbox::inbox,
        routes::inbox::list_suggestions,
        routes::inbox::accept_suggestion,
        routes::inbox::reject_suggestion,
        routes::inbox::reply_suggestion,
        routes::relations::add_relation,
        routes::relations::retype_relation,
        routes::relations::remove_relation,
        routes::entities::list_entities,
        routes::entities::create_entity,
        routes::entities::get_entity,
        routes::entities::patch_entity,
        routes::entities::merge_entity,
        routes::entities::entity_notes,
        routes::entities::entity_documents,
        routes::documents::list_documents,
        routes::documents::create_document,
        routes::documents::get_document,
        routes::documents::patch_document,
        routes::documents::add_custody_event,
        routes::documents::list_places,
        routes::documents::create_place,
        routes::documents::get_place,
        routes::documents::patch_place,
        routes::tasks::list_tasks,
        routes::tasks::create_task,
        routes::tasks::patch_task,
        routes::tasks::complete_task,
        routes::tasks::cancel_task,
        routes::tasks::reopen_task,
        routes::vault_ops::export_vault,
        routes::vault_ops::import_vault,
        routes::vault_ops::integrity,
    ),
    tags(
        (name = "notes", description = "Notes, the vault tree, backlinks, history and revert (PLAN §7.5 Notes)."),
        (name = "search", description = "Keyword search; semantic and hybrid need the AI subsystem."),
        (name = "inbox", description = "Capture, inbox and suggestions."),
        (name = "relations", description = "Typed relations between notes."),
        (name = "entities", description = "People, companies, documents and places."),
        (name = "tasks", description = "Tasks as Obsidian Tasks checklist lines."),
        (name = "vault", description = "Export, import and integrity."),
    )
)]
pub struct VaultApiDoc;
