//! Vault operations (PLAN §6.10, §7.5 Vault ops): export and import (zip bodies) and the
//! integrity warnings raised by reconciliation (§7.3).

use actix_web::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_vault::VaultService;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
use crate::routes::me::ZipArchive;
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem, ProblemType, ZIP};

/// Largest accepted import body (compressed).
pub const IMPORT_BODY_LIMIT: usize = 256 * 1024 * 1024;

/// Result of an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ImportReport {
    /// The import commit (revert it with `POST /commits/{commit}/revert` to undo the
    /// import); absent if nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Vault paths written, sorted.
    pub imported: Vec<String>,
    /// Notes that were given an ID (or task block IDs).
    pub ids_assigned: Vec<String>,
    /// Entries skipped (hidden files such as `.obsidian/`, unsafe names), sorted.
    pub skipped: Vec<String>,
}

/// One integrity warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct IntegrityWarning {
    /// Warning ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// `temp_file_removed`, `uncommitted_changes`, `id_assigned`, `out_of_band_edit`,
    /// `missing_file`, `sidecar_repaired`, `orphan_sidecar_removed` or `index_repaired`.
    pub kind: String,
    /// Vault path concerned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What happened (content-free).
    pub detail: String,
    /// When.
    pub created: DateTime<Utc>,
}

/// Integrity warnings, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Integrity {
    /// Warnings (at most 500).
    pub warnings: Vec<IntegrityWarning>,
}

/// The vault as a zip (with `.meta/`, without `.git/`, plus `.obsidian/app.json`).
#[utoipa::path(
    get, path = "/export", tag = "vault", operation_id = "export_vault",
    responses(
        (status = 200, description = "The vault; the same vault always exports to the same bytes.", content_type = "application/zip", body = inline(ZipArchive)),
    ),
)]
pub async fn export_vault(
    auth: Authenticated,
    vault: web::Data<VaultService>,
) -> Result<HttpResponse, Problem> {
    let bytes = vault.export(auth.scope()).await.or_problem()?;
    Ok(HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, ZIP))
        .insert_header((
            CONTENT_DISPOSITION,
            "attachment; filename=\"strata-vault.zip\"",
        ))
        .body(bytes))
}

/// Import an Obsidian vault zip in one revertible commit.
#[utoipa::path(
    post, path = "/import", tag = "vault", operation_id = "import_vault",
    request_body(content = inline(ZipArchive), content_type = "application/zip", description = "A zip of an Obsidian vault."),
    responses(
        (status = 200, description = "Imported; one `user: import <n> files` commit.", body = ImportReport),
        (status = 413, description = "`payload_too_large`: the archive, an entry, or their total is too large.", body = Problem),
        (status = 422, description = "`invalid_archive`: not a zip, or an entry is a symlink, absolute, or contains `..`.", body = Problem),
    ),
)]
pub async fn import_vault(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    body: web::Bytes,
) -> Result<MsgPack<ImportReport>, Problem> {
    let is_zip = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<mime::Mime>().ok())
        .is_some_and(|m| m.essence_str().eq_ignore_ascii_case(ZIP));
    if !is_zip {
        return Err(Problem::new(ProblemType::UnsupportedMediaType)
            .with_detail(format!("import bodies must be {ZIP}")));
    }
    let r = vault
        .import(auth.scope(), body.to_vec())
        .await
        .or_problem()?;
    Ok(MsgPack(ImportReport {
        commit: r.commit,
        imported: r.imported,
        ids_assigned: r.ids_assigned,
        skipped: r.skipped,
    }))
}

/// Integrity warnings raised by reconciliation, newest first.
#[utoipa::path(
    get, path = "/integrity", tag = "vault", operation_id = "get_integrity",
    responses((status = 200, description = "Warnings.", body = Integrity)),
)]
pub async fn integrity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
) -> Result<MsgPack<Integrity>, Problem> {
    let w = vault.integrity(auth.scope(), 500).await.or_problem()?;
    Ok(MsgPack(Integrity {
        warnings: w
            .into_iter()
            .map(|w| IntegrityWarning {
                id: Ulid::from(w.id),
                kind: w.kind,
                path: w.path,
                detail: w.detail,
                created: w.created,
            })
            .collect(),
    }))
}

/// Mounts the vault-ops routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/export", web::get().to(export_vault))
        .service(
            web::resource("/import")
                .app_data(web::PayloadConfig::new(IMPORT_BODY_LIMIT))
                .route(web::post().to(import_vault)),
        )
        .route("/integrity", web::get().to(integrity));
}
