//! `GET /search` (PLAN §7.5 Search): keyword search over Arabic/Latin-normalised text
//! (Postgres full-text); `semantic` and `hybrid` need the AI subsystem (Phase 4) and answer
//! `503 ai_unavailable`.

use actix_web::web;
use serde::{Deserialize, Serialize};
use strata_vault::VaultService;
use strata_vault::ops::read::SearchMode as VMode;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
use crate::routes::notes::NoteKind;
use crate::wire::{MsgPack, Problem};

/// Search mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// Full-text over normalised text (works without AI).
    #[default]
    Keyword,
    /// Embedding similarity (needs the AI subsystem).
    Semantic,
    /// Keyword and semantic fused (needs the AI subsystem).
    Hybrid,
}

/// `GET /search` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// Query text (web-search syntax: words, `"phrases"`, `-exclusions`, `or`).
    pub q: String,
    /// Mode (default `keyword`).
    #[serde(default)]
    pub mode: Option<SearchMode>,
    /// Maximum hits (1–100, default 20).
    #[serde(default)]
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
}

/// One hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    /// Note.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Rank (higher is better).
    pub score: f64,
    /// The first body line matching the query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Search results, best first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SearchResults {
    /// Hits.
    pub hits: Vec<SearchHit>,
}

/// Search the vault.
#[utoipa::path(
    get, path = "/search", tag = "search", operation_id = "search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Hits, best first.", body = SearchResults),
        (status = 503, description = "`ai_unavailable`: semantic and hybrid need the AI subsystem.", body = Problem),
    ),
)]
pub async fn search(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<SearchQuery>,
) -> Result<MsgPack<SearchResults>, Problem> {
    let mode = match q.mode.unwrap_or_default() {
        SearchMode::Keyword => VMode::Keyword,
        SearchMode::Semantic => VMode::Semantic,
        SearchMode::Hybrid => VMode::Hybrid,
    };
    let hits = vault
        .search(auth.scope(), &q.q, mode, q.limit.unwrap_or(20))
        .await
        .map_err(|e| match e {
            strata_vault::VaultError::Invalid(reason) => {
                crate::vault::invalid_parameter("q", "empty_query", &reason)
            }
            other => crate::vault::problem(&other),
        })?;
    Ok(MsgPack(SearchResults {
        hits: hits
            .into_iter()
            .map(|h| SearchHit {
                id: h.id.as_ulid(),
                path: h.path,
                title: h.title,
                kind: h.kind.into(),
                score: f64::from(h.score),
                snippet: h.snippet,
            })
            .collect(),
    }))
}

/// Mounts the search route.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/search", web::get().to(search));
}
