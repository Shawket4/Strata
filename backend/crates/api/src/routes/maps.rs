//! Saved mind-map layouts (PLAN §6.8, §7.5 `GET/PUT /maps`): JSON Canvas files in `maps/`.
//! The `.canvas` file is JSON on disk (Obsidian's format) and travels as a string inside
//! the MessagePack envelope, like a note's markdown. A map's ID is its file name without
//! `.canvas`. See `strata_graph::maps` for validation and storage.

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, Responder, web};
use serde::{Deserialize, Serialize};
use strata_graph::maps::{self, MapSummary as GSummary, MapView};
use strata_vault::VaultService;
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
use crate::graph::{GraphApi, OrGraphProblem};
use crate::routes::graph::graph_api;
use crate::wire::{MsgPack, MsgPackConfig, Problem};

/// A saved map in a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MapSummary {
    /// ID (the file name without `.canvas`).
    pub id: String,
    /// Vault path (`maps/<id>.canvas`).
    pub path: String,
    /// Version (the value for `If-Match`).
    pub version: String,
    /// Node count (absent when the file is not a valid canvas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<u32>,
    /// Edge count (absent when the file is not a valid canvas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edges: Option<u32>,
}

/// The saved maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MapList {
    /// Maps, by ID.
    pub maps: Vec<MapSummary>,
}

/// A file node of a map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MapFile {
    /// Canvas node ID.
    pub node_id: String,
    /// Referenced vault path.
    pub path: String,
    /// The note at that path, when it is a live note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub note_id: Option<Ulid>,
}

/// A saved map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Map {
    /// ID.
    pub id: String,
    /// Vault path.
    pub path: String,
    /// Version (the value for `If-Match`).
    pub version: String,
    /// The `.canvas` file: JSON Canvas 1.0 text as stored (Obsidian's layout).
    pub content: String,
    /// File nodes in canvas order, resolved to notes.
    pub files: Vec<MapFile>,
}

/// `PUT /maps/{id}` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PutMapRequest {
    /// JSON Canvas 1.0 text. File nodes must reference files of the caller's vault.
    pub content: String,
}

fn wire(v: MapView) -> Map {
    Map {
        id: v.id,
        path: v.path,
        version: v.version,
        content: v.content,
        files: v
            .files
            .into_iter()
            .map(|f| MapFile {
                node_id: f.node_id,
                path: f.path,
                note_id: f.note_id.map(|n| n.as_ulid()),
            })
            .collect(),
    }
}

fn summary(s: GSummary) -> MapSummary {
    MapSummary {
        id: s.id,
        path: s.path,
        version: s.version,
        nodes: s.nodes,
        edges: s.edges,
    }
}

/// The caller's saved maps.
#[utoipa::path(
    get, path = "/maps", tag = "maps", operation_id = "list_maps",
    responses((status = 200, description = "Every `maps/*.canvas`, by ID.", body = MapList)),
)]
pub async fn list_maps(
    auth: Authenticated,
    vault: web::Data<VaultService>,
) -> Result<MsgPack<MapList>, Problem> {
    let maps = maps::list(&vault, auth.scope()).await.or_graph_problem()?;
    Ok(MsgPack(MapList {
        maps: maps.into_iter().map(summary).collect(),
    }))
}

/// One saved map.
#[utoipa::path(
    get, path = "/maps/{id}", tag = "maps", operation_id = "get_map",
    params(("id" = String, Path, description = "Map ID (file name without `.canvas`).")),
    responses((status = 200, description = "The map.", body = Map)),
)]
pub async fn get_map(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    api: Option<web::Data<GraphApi>>,
    id: web::Path<String>,
) -> Result<MsgPack<Map>, Problem> {
    let api = graph_api(api.as_ref())?;
    let view = maps::get(&vault, api.graph.db(), auth.scope(), &id)
        .await
        .or_graph_problem()?;
    Ok(MsgPack(wire(view)))
}

/// Save a map: create it (no `If-Match`) or replace it (`If-Match`: its current version).
#[utoipa::path(
    put, path = "/maps/{id}", tag = "maps", operation_id = "put_map",
    params(
        ("id" = String, Path, description = "Map ID (file name without `.canvas`)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "The map's current version; required to replace an existing map, absent to create one."),
    ),
    request_body = PutMapRequest,
    responses(
        (status = 200, description = "Replaced; one `user: save map maps/<id>.canvas` commit.", body = Map),
        (status = 201, description = "Created; one `user: save map maps/<id>.canvas` commit.", body = Map),
        (status = 409, description = "`version_conflict` with `current_version`: the map exists (send `If-Match`) or changed.", body = Problem),
    ),
)]
pub async fn put_map(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    api: Option<web::Data<GraphApi>>,
    req: HttpRequest,
    id: web::Path<String>,
    body: MsgPack<PutMapRequest>,
) -> Result<impl Responder, Problem> {
    let api = graph_api(api.as_ref())?;
    let if_match = crate::vault::if_match(req.headers())?;
    let (view, created) = maps::put(
        &vault,
        api.graph.db(),
        auth.scope(),
        &id,
        &body.into_inner().content,
        if_match,
    )
    .await
    .or_graph_problem()?;
    Ok(MsgPack(wire(view)).customize().with_status(if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }))
}

/// Mounts the map routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    let limit = MsgPackConfig::default().with_body_limit(4 * 1024 * 1024);
    cfg.route("/maps", web::get().to(list_maps)).service(
        web::resource("/maps/{id}")
            .app_data(limit)
            .route(web::get().to(get_map))
            .route(web::put().to(put_map)),
    );
}
