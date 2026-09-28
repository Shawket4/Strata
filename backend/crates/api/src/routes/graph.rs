//! Graph endpoints (PLAN §7.5 Graph, §9.6, §10): the global graph with type filters,
//! optional similarity edges, optional tag nodes and the entity lens, local neighbourhoods,
//! the manual re-clustering trigger and the user's cluster rename. Payloads carry no
//! positions (D3: the client core lays out).
//!
//! Handlers take the caller's scope and the [`GraphApi`] registered by the composition root;
//! every read runs in the caller's scope, so another user's note ID is `404`.

use actix_web::http::StatusCode;
use actix_web::http::header::RETRY_AFTER;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError, web};
use chrono::{DateTime, Utc};
use domain::RelationOrigin;
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_graph::assemble::{self, NodeId, SimilarityStatus as GStatus};
use strata_graph::query::{self, EdgeFilter, GraphQuery, Lens, LocalQuery, NodeFilter};
use ulid::Ulid;
use utoipa::{OpenApi, ToSchema};

use crate::auth::Authenticated;
use crate::events::EventBus;
use crate::graph::{BusClusterEvents, GraphApi, OrGraphProblem};
use crate::wire::{MsgPack, Problem, ProblemType};

/// Kind of a graph node (§10): a note kind, or `tag` (with `include_tags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    /// An ordinary note.
    Note,
    /// A concept note.
    Concept,
    /// A person.
    Person,
    /// A company.
    Company,
    /// A document.
    Document,
    /// A place.
    Place,
    /// A tag (`include_tags=true`).
    Tag,
}

impl GraphNodeKind {
    fn of(kind: domain::GraphNodeKind) -> Result<Self, Problem> {
        Ok(match kind {
            domain::GraphNodeKind::Note => Self::Note,
            domain::GraphNodeKind::Concept => Self::Concept,
            domain::GraphNodeKind::Person => Self::Person,
            domain::GraphNodeKind::Company => Self::Company,
            domain::GraphNodeKind::Document => Self::Document,
            domain::GraphNodeKind::Place => Self::Place,
            domain::GraphNodeKind::Tag => Self::Tag,
            other @ (domain::GraphNodeKind::Attachment | domain::GraphNodeKind::Cluster) => {
                return Err(Problem::internal(&format!("{other} nodes are never returned")));
            }
        })
    }
}

/// A node: a live note, or a tag (`include_tags=true`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct GraphNode {
    /// Note ID (ULID), or `tag:<tag>` for a tag node (the tag in lowercase; tags compare
    /// without case).
    pub id: String,
    /// Title (a tag node's is the tag, without `#`).
    pub title: String,
    /// Kind.
    pub kind: GraphNodeKind,
    /// Vault path (absent on tag nodes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Stable cluster ID, if clustered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,
    /// Edges of this response touching the node (in + out).
    pub degree: u32,
    /// Dominant language (`ar`, `en`, `mixed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Last update (absent on tag nodes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,
    /// Short AI summary (hover), at most 200 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Hops from the focus (local graphs only; the focus is 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
}

/// An edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct GraphEdge {
    /// Source node ID.
    pub source: String,
    /// Target node ID (`tag:<tag>` for `tag` edges).
    pub target: String,
    /// `link`, `embed`, `relation:<type>`, `similarity`, `concept`, `mention`,
    /// `entity:<type>`, `custody:<location|holder|last-holder>`, `part-of-place`,
    /// `document:copy-of`, `tag` (note → tag node), or `co-mention` (entity lens).
    pub kind: String,
    /// Provenance (`user` / `ai`); absent on co-mention edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// AI confidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// AI one-line reason (relations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Cosine similarity (`similarity`) or co-mention strength (`co-mention`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    /// Notes mentioning both entities (`co-mention`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<u32>,
}

/// A cluster of the returned nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GraphCluster {
    /// Stable ID.
    pub id: String,
    /// Name.
    pub name: String,
    /// Members among the returned nodes.
    pub size: u32,
}

/// What happened to similarity edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SimilarityStatus {
    /// Not requested.
    Off,
    /// Included for every note with an embedding.
    Complete,
    /// Included for the most recently updated notes only (bounded cost).
    Truncated,
    /// Requested, but no embedding model is configured.
    Unavailable,
}

/// A graph (no positions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Graph {
    /// Nodes, by ID.
    pub nodes: Vec<GraphNode>,
    /// Edges, by source, target, kind.
    pub edges: Vec<GraphEdge>,
    /// Clusters of the returned nodes, by ID.
    pub clusters: Vec<GraphCluster>,
    /// Similarity edges.
    pub similarity: SimilarityStatus,
}

/// `GET /graph` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GraphParams {
    /// Edge kinds, comma-separated: `link`, `embed`, `relation` or `relation:<type>`,
    /// `similarity`, `concept`, `mention`, `entity` or `entity:<type>`, `custody` or
    /// `custody:<location|holder|last-holder>`, `part-of-place`, `document` or
    /// `document:copy-of`, `tag`, `co-mention` (lens). Empty: all.
    #[serde(default)]
    pub types: Option<String>,
    /// Node kinds, comma-separated (`note`, `concept`, `person`, `company`, `document`,
    /// `place`, `tag`). Empty: all. Ignored with a lens.
    #[serde(default)]
    pub kinds: Option<String>,
    /// Add similarity edges (top-n per note above a floor, never stored).
    #[serde(default)]
    pub include_similarity: Option<bool>,
    /// Entity-centred graph: `people` or `companies`.
    #[serde(default)]
    pub lens: Option<String>,
    /// Add tag nodes (`tag:<tag>`) and note → tag edges. Ignored with a lens.
    #[serde(default)]
    pub include_tags: Option<bool>,
}

/// `GET /graph/local/{id}` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LocalParams {
    /// Hops from the focus, 1–3 (default 1).
    #[serde(default)]
    #[param(minimum = 1, maximum = 3)]
    pub depth: Option<u8>,
    /// Edge kinds (as for `GET /graph`).
    #[serde(default)]
    pub types: Option<String>,
    /// Node kinds (as for `GET /graph`); the focus is always included.
    #[serde(default)]
    pub kinds: Option<String>,
    /// Add the focus's similarity edges.
    #[serde(default)]
    pub include_similarity: Option<bool>,
    /// Add tag nodes and note → tag edges (notes sharing a tag are two hops apart).
    #[serde(default)]
    pub include_tags: Option<bool>,
}

/// `PATCH /graph/clusters/{id}` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RenameClusterRequest {
    /// The new name (whitespace runs collapse; 1–100 characters).
    pub name: String,
}

/// A queued re-clustering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ReclusterAccepted {
    /// The queued `cluster` job.
    #[schema(value_type = String, format = "ulid")]
    pub job_id: Ulid,
    /// When it may run.
    pub run_after: DateTime<Utc>,
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn node_id(id: &NodeId) -> String {
    id.to_string()
}

/// The wire form of a graph.
pub fn wire(view: assemble::GraphView) -> Result<Graph, Problem> {
    Ok(Graph {
        nodes: view
            .nodes
            .into_iter()
            .map(|n| {
                Ok(GraphNode {
                    id: node_id(&n.id),
                    title: n.title,
                    kind: GraphNodeKind::of(n.kind)?,
                    path: n.path,
                    cluster_id: n.cluster_id,
                    degree: n.degree,
                    lang: n.lang,
                    updated: n.updated,
                    summary: n.summary,
                    depth: n.depth,
                })
            })
            .collect::<Result<_, Problem>>()?,
        edges: view
            .edges
            .into_iter()
            .map(|e| GraphEdge {
                source: node_id(&e.source),
                target: node_id(&e.target),
                kind: e.kind.to_string(),
                by: e.by.map(|b| match b {
                    RelationOrigin::User => "user".to_owned(),
                    RelationOrigin::Ai => "ai".to_owned(),
                }),
                confidence: e.confidence.map(|c| round6(f64::from(c))),
                reason: e.reason,
                weight: e.weight.map(round6),
                notes: e.notes,
            })
            .collect(),
        clusters: view
            .clusters
            .into_iter()
            .map(|c| GraphCluster {
                id: c.id,
                name: c.name,
                size: c.size,
            })
            .collect(),
        similarity: match view.similarity {
            GStatus::Off => SimilarityStatus::Off,
            GStatus::Complete => SimilarityStatus::Complete,
            GStatus::Truncated => SimilarityStatus::Truncated,
            GStatus::Unavailable => SimilarityStatus::Unavailable,
        },
    })
}

/// The registered [`GraphApi`] (a composition error when missing).
pub fn graph_api(api: Option<&web::Data<GraphApi>>) -> Result<&GraphApi, Problem> {
    api.map(|a| a.as_ref())
        .ok_or_else(|| Problem::internal(&"graph API not registered"))
}

/// The caller's graph.
#[utoipa::path(
    get, path = "/graph", tag = "graph", operation_id = "get_graph",
    params(GraphParams),
    responses(
        (status = 200, description = "Nodes, edges and clusters (no positions).", body = Graph),
    ),
)]
pub async fn get_graph(
    auth: Authenticated,
    api: Option<web::Data<GraphApi>>,
    q: web::Query<GraphParams>,
) -> Result<MsgPack<Graph>, Problem> {
    let api = graph_api(api.as_ref())?;
    let query = GraphQuery {
        edges: EdgeFilter::parse(q.types.as_deref()).or_graph_problem()?,
        nodes: NodeFilter::parse(q.kinds.as_deref()).or_graph_problem()?,
        include_similarity: q.include_similarity.unwrap_or(false),
        lens: Lens::parse(q.lens.as_deref()).or_graph_problem()?,
        include_tags: q.include_tags.unwrap_or(false),
    };
    let view = api
        .graph
        .graph(auth.scope(), &query)
        .await
        .or_graph_problem()?;
    Ok(MsgPack(wire(view)?))
}

/// The neighbourhood of a note.
#[utoipa::path(
    get, path = "/graph/local/{id}", tag = "graph", operation_id = "get_local_graph",
    params(("id" = Ulid, Path, description = "Focus note ID (ULID)."), LocalParams),
    responses(
        (status = 200, description = "The focus and everything within `depth` hops over the allowed edges.", body = Graph),
    ),
)]
pub async fn get_local_graph(
    auth: Authenticated,
    api: Option<web::Data<GraphApi>>,
    id: web::Path<Ulid>,
    q: web::Query<LocalParams>,
) -> Result<MsgPack<Graph>, Problem> {
    let api = graph_api(api.as_ref())?;
    let query = LocalQuery {
        depth: query::depth(q.depth).or_graph_problem()?,
        edges: EdgeFilter::parse(q.types.as_deref()).or_graph_problem()?,
        nodes: NodeFilter::parse(q.kinds.as_deref()).or_graph_problem()?,
        include_similarity: q.include_similarity.unwrap_or(false),
        include_tags: q.include_tags.unwrap_or(false),
    };
    let view = api
        .graph
        .local(auth.scope(), NoteId::from_ulid(*id), &query)
        .await
        .or_graph_problem()?;
    Ok(MsgPack(wire(view)?))
}

/// Re-cluster the caller's graph now (the `cluster` job also runs nightly).
#[utoipa::path(
    post, path = "/graph/recluster", tag = "graph", operation_id = "recluster_graph",
    responses(
        (status = 202, description = "A `cluster` job is queued (a queued one is reused); `cluster.updated` follows on `/events` when clusters change.", body = ReclusterAccepted),
        (status = 429, description = "`rate_limited`: at most 3 re-clusterings per hour. See `Retry-After`.", body = Problem),
    ),
)]
pub async fn recluster_graph(
    auth: Authenticated,
    api: Option<web::Data<GraphApi>>,
    req: HttpRequest,
) -> Result<HttpResponse, Problem> {
    let api = graph_api(api.as_ref())?;
    if let Err(limited) = api.recluster.check(&auth.scope().user_id().to_string()) {
        let mut resp = ResponseError::error_response(
            &Problem::new(ProblemType::RateLimited)
                .with_detail("too many re-clusterings; try again later"),
        );
        if let Ok(v) = limited.retry_after_secs.to_string().parse() {
            resp.headers_mut().insert(RETRY_AFTER, v);
        }
        return Ok(resp);
    }
    let now = api.clock.now();
    let mut tx = api
        .graph
        .db()
        .begin(auth.scope())
        .await
        .map_err(|e| Problem::internal(&e))?;
    let job = strata_graph::cluster::enqueue(&mut tx, api.ids.as_ref(), "manual", now)
        .await
        .map_err(|e| Problem::internal(&e))?;
    tx.commit().await.map_err(|e| Problem::internal(&e))?;
    Ok(MsgPack(ReclusterAccepted {
        job_id: job.id.as_ulid(),
        run_after: job.run_after,
    })
    .customize()
    .with_status(StatusCode::ACCEPTED)
    .respond_to(&req)
    .map_into_boxed_body())
}

/// Rename a cluster. The name is the user's from now on: re-clustering keeps it.
#[utoipa::path(
    patch, path = "/graph/clusters/{id}", tag = "graph", operation_id = "rename_cluster",
    params(("id" = String, Path, description = "Stable cluster ID (as in `clusters[].id`).")),
    request_body = RenameClusterRequest,
    responses(
        (status = 200, description = "The renamed cluster (`size` = its member notes). `.meta/clusters.json` is written with `named_by: user` in one `user:` commit (nothing when the cluster already has this name from the user); `cluster.updated` follows on `/events`.", body = GraphCluster),
        (status = 404, description = "`not_found`: no such cluster.", body = Problem),
        (status = 422, description = "`invalid_body`: code `empty_name` or `name_too_long`, or a decoding error.", body = Problem),
    ),
)]
pub async fn rename_cluster(
    auth: Authenticated,
    api: Option<web::Data<GraphApi>>,
    bus: Option<web::Data<EventBus>>,
    id: web::Path<String>,
    body: MsgPack<RenameClusterRequest>,
) -> Result<MsgPack<GraphCluster>, Problem> {
    let api = graph_api(api.as_ref())?;
    let bus = bus.ok_or_else(|| Problem::internal(&"event bus not registered"))?;
    let events = BusClusterEvents(bus.into_inner());
    let renamed = strata_graph::cluster::rename(
        api.graph.vault(),
        &events,
        auth.scope(),
        &id,
        &body.into_inner().name,
        api.clock.now(),
    )
    .await
    .or_graph_problem()?;
    Ok(MsgPack(GraphCluster {
        id: renamed.id,
        name: renamed.name,
        size: renamed.size,
    }))
}

/// Mounts the graph and map routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/graph", web::get().to(get_graph))
        .route("/graph/local/{id}", web::get().to(get_local_graph))
        .route("/graph/recluster", web::post().to(recluster_graph))
        .route("/graph/clusters/{id}", web::patch().to(rename_cluster));
    crate::routes::maps::configure(cfg);
}

/// The graph part of the contract (merged into the production document).
#[derive(Debug, OpenApi)]
#[openapi(
    paths(
        get_graph,
        get_local_graph,
        recluster_graph,
        rename_cluster,
        crate::routes::maps::list_maps,
        crate::routes::maps::get_map,
        crate::routes::maps::put_map,
    ),
    tags(
        (name = "graph", description = "The knowledge graph, neighbourhoods and clustering (PLAN §10)."),
        (name = "maps", description = "Saved mind-map layouts (JSON Canvas in `maps/`, §6.8)."),
    )
)]
pub struct GraphApiDoc;
