//! Sync endpoints (PLAN §7.5 Sync, §12.4, D19): `GET /sync/bootstrap`, `GET /sync/changes`,
//! `POST /sync/push`. The logic is in [`crate::sync`].

use actix_web::http::header::ContentType;
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use strata_vault::VaultService;
use sync_model::{BootstrapPage, ChangesPage, PushRequest};
use utoipa::IntoParams;

use crate::auth::{AuthState, Authenticated};
use crate::sync::push::{PushContext, push};
use crate::sync::wire::{
    SyncBootstrapPage, SyncChangesPage, SyncPushRequest, SyncPushResponse,
};
use crate::sync::{BootstrapCursor, SyncState, records};
use crate::wire::{MSGPACK, MsgPack, MsgPackConfig, Problem};

/// `GET /sync/bootstrap` query.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BootstrapQuery {
    /// `next_cursor` of the previous page; absent for the first page.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Records per page (default and maximum 200).
    #[serde(default)]
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// `GET /sync/changes` query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChangesQuery {
    /// The last seq applied (the bootstrap's `seq`, then each page's `next_seq`).
    #[param(minimum = 0)]
    pub since: u64,
    /// The epoch the client is on.
    #[param(minimum = 1)]
    pub epoch: u64,
    /// Changes per page (default and maximum 500).
    #[serde(default)]
    #[param(minimum = 1, maximum = 500)]
    pub limit: Option<u32>,
}

fn clamp(requested: Option<u32>, max: u32) -> u32 {
    requested.map_or(max, |l| l.clamp(1, max))
}

/// A paged snapshot of the caller's data: notes with full content and version (entities,
/// documents, places, task homes and inbox notes included), relations with provenance,
/// rejected edges, suggestions, cluster assignments and names, settings, device settings and
/// keep-both pairs. The first page captures the log position (`seq`); pull
/// `/sync/changes?since=seq` after the last page.
#[utoipa::path(
    get, path = "/sync/bootstrap", tag = "sync", operation_id = "sync_bootstrap",
    params(BootstrapQuery),
    responses(
        (status = 200, description = "One page.", body = SyncBootstrapPage),
        (status = 410, description = "`epoch_changed`: the epoch moved during the bootstrap; start again.", body = Problem),
    ),
)]
pub async fn bootstrap(
    auth: Authenticated,
    state: web::Data<AuthState>,
    vault: web::Data<VaultService>,
    sync: web::Data<SyncState>,
    query: web::Query<BootstrapQuery>,
) -> Result<MsgPack<BootstrapPage>, Problem> {
    let q = query.into_inner();
    let cursor = q.cursor.as_deref().map(BootstrapCursor::decode).transpose()?;
    let limit = clamp(q.limit, sync.config.bootstrap_page);
    Ok(MsgPack(
        records::bootstrap(&state.app_db, &vault, auth.scope(), cursor, limit).await?,
    ))
}

/// Changed records after `since` with full payloads, tombstones for deletes, and the seq to
/// continue from.
#[utoipa::path(
    get, path = "/sync/changes", tag = "sync", operation_id = "sync_changes",
    params(ChangesQuery),
    responses(
        (status = 200, description = "One page.", body = SyncChangesPage),
        (status = 410, description = "`epoch_changed`: re-bootstrap.", body = Problem),
    ),
)]
pub async fn changes(
    auth: Authenticated,
    state: web::Data<AuthState>,
    vault: web::Data<VaultService>,
    sync: web::Data<SyncState>,
    query: web::Query<ChangesQuery>,
) -> Result<MsgPack<ChangesPage>, Problem> {
    let q = query.into_inner();
    let limit = clamp(q.limit, sync.config.changes_page);
    Ok(MsgPack(
        records::changes(&state.app_db, &vault, auth.scope(), q.since, q.epoch, limit).await?,
    ))
}

/// Applies ops in order and answers one result per op: `applied{new_version, merged}`,
/// `conflict{server_version, resolution}` (D19), `duplicate{candidates}` or
/// `rejected{problem}`. Results are stored under `op_id`; a replayed op returns its stored
/// result byte for byte and is not applied again.
#[utoipa::path(
    post, path = "/sync/push", tag = "sync", operation_id = "sync_push",
    request_body = SyncPushRequest,
    responses((status = 200, description = "One result per op, in order.", body = SyncPushResponse)),
)]
pub async fn push_ops(
    auth: Authenticated,
    state: web::Data<AuthState>,
    vault: web::Data<VaultService>,
    sync: web::Data<SyncState>,
    body: MsgPack<PushRequest>,
) -> Result<HttpResponse, Problem> {
    let ctx = PushContext {
        vault: &vault,
        db: &state.app_db,
        scope: auth.scope(),
        device: auth.device,
        clock: &state.clock,
        ids: &state.ids,
        bus: sync.bus(),
        merge_history: sync.config.merge_history,
    };
    let bytes = push(&sync, &ctx, body.into_inner().ops).await?;
    Ok(HttpResponse::Ok()
        .insert_header(ContentType(MSGPACK.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM)))
        .body(bytes))
}

/// Mounts the sync routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    let limit = crate::sync::SyncConfig::default().push_body_limit;
    cfg.route("/sync/bootstrap", web::get().to(bootstrap))
        .route("/sync/changes", web::get().to(changes))
        .service(
            web::resource("/sync/push")
                .app_data(MsgPackConfig::default().with_body_limit(limit))
                .route(web::post().to(push_ops)),
        );
}
