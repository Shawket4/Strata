//! Demo router proving the wire machinery end to end (feature `test-support` only).
//!
//! Covers what real endpoints will need: a MessagePack body with a per-route limit, a tagged
//! enum, bytes, timestamps, ULIDs, a `201`, a `204`, path/query/header parameters, `404`,
//! `409 duplicate_candidates`, `409 version_conflict`, a secured operation and a resumable
//! WebSocket stream.

use std::collections::BTreeMap;
use std::sync::Mutex;

use actix_web::http::StatusCode;
use actix_web::http::header::AUTHORIZATION;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use chrono::{DateTime, TimeZone, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;
use utoipa::{IntoParams, OpenApi, ToSchema};

use crate::app::{self, api_v1};
use crate::openapi::{ApiDoc, StreamOperation, build};
use crate::wire::ws::{self, Frame, ReplayBuffer, ResumeQuery, WsConfig};
use crate::wire::{
    Binary, DuplicateCandidate, MatchLevel, MsgPack, MsgPackConfig, Problem, ProblemType,
};

/// Body limit of `create_widget`, small so tests can exceed it.
pub const CREATE_BODY_LIMIT: usize = 4096;

/// The only accepted demo bearer token.
pub const GOOD_TOKEN: &str = "good-token";

/// Shape of a widget (internally tagged enum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    /// A circle.
    Circle {
        /// Radius.
        radius: f64,
    },
    /// A rectangle.
    Rect {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
    },
}

/// Create request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CreateWidget {
    /// Name; must be unique unless `force`.
    pub name: String,
    /// Shape.
    pub shape: Shape,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Opaque bytes.
    pub blob: Binary,
    /// Optional due time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<DateTime<Utc>>,
    /// Create even when a widget of the same name exists.
    #[serde(default)]
    pub force: bool,
}

/// A stored widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Widget {
    /// ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Shape.
    pub shape: Shape,
    /// Tags (only with `verbose=true` on reads).
    pub tags: Vec<String>,
    /// Opaque bytes.
    pub blob: Binary,
    /// Optional due time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<DateTime<Utc>>,
    /// Version for `If-Match`.
    pub version: String,
    /// Creation time.
    pub created: DateTime<Utc>,
}

/// Rename request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct RenameWidget {
    /// New name.
    pub name: String,
}

/// Query of `get_widget`.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetWidgetQuery {
    /// Include tags.
    pub verbose: Option<bool>,
}

/// The authenticated caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct WhoAmI {
    /// Subject.
    pub subject: String,
}

/// Item of the `demo_ticks` stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DemoTick {
    /// Counter.
    pub n: u64,
    /// Label.
    pub label: String,
}

#[derive(Debug, Clone, Copy)]
enum StreamScript {
    CutAfter(usize),
    HoldAfter(usize),
}

/// Server state. Deterministic: IDs and timestamps derive from a counter.
#[derive(Debug)]
pub struct DemoState {
    widgets: Mutex<(u64, BTreeMap<Ulid, Widget>)>,
    ticks: Mutex<ReplayBuffer<DemoTick>>,
    next_stream: Mutex<Option<StreamScript>>,
    /// Stream connection settings.
    pub ws: WsConfig,
}

impl DemoState {
    /// State with `ticks` stream items `1..=ticks` retained in a buffer of `capacity`.
    pub fn new(ticks: u64, capacity: usize) -> Self {
        let mut buffer = ReplayBuffer::new(capacity);
        for n in 1..=ticks {
            buffer.push(DemoTick {
                n,
                label: format!("tick {n}"),
            });
        }
        Self {
            widgets: Mutex::new((0, BTreeMap::new())),
            ticks: Mutex::new(buffer),
            next_stream: Mutex::new(None),
            ws: WsConfig::default(),
        }
    }

    /// Makes the next stream connection close (without `end`) after `frames` frames.
    pub fn cut_next_stream_after(&self, frames: usize) {
        self.script_next_stream(StreamScript::CutAfter(frames));
    }

    /// Makes the next stream connection send `frames` frames, then stay open silently.
    pub fn hold_next_stream_after(&self, frames: usize) {
        self.script_next_stream(StreamScript::HoldAfter(frames));
    }

    fn script_next_stream(&self, script: StreamScript) {
        if let Ok(mut next) = self.next_stream.lock() {
            *next = Some(script);
        }
    }
}

impl Default for DemoState {
    fn default() -> Self {
        Self::new(5, 16)
    }
}

fn internal() -> Problem {
    Problem::new(ProblemType::Internal)
}

/// Create a widget.
#[utoipa::path(
    post,
    path = "/demo/widgets",
    tag = "demo",
    operation_id = "create_widget",
    request_body = CreateWidget,
    responses(
        (status = 201, description = "Created.", body = Widget),
        (status = 409, description = "`duplicate_candidates`.", body = Problem),
    ),
)]
pub async fn create_widget(
    state: web::Data<DemoState>,
    body: MsgPack<CreateWidget>,
) -> Result<impl Responder, Problem> {
    let body = body.into_inner();
    let mut guard = state.widgets.lock().map_err(|_| internal())?;
    let (counter, widgets) = &mut *guard;
    if !body.force
        && let Some(existing) = widgets.values().find(|w| w.name == body.name)
    {
        return Err(Problem::duplicate_candidates(vec![DuplicateCandidate {
            id: existing.id,
            kind: "widget".to_owned(),
            title: existing.name.clone(),
            snippet: None,
            match_level: MatchLevel::Exact,
            score: 1.0,
        }]));
    }
    *counter += 1;
    let n = *counter;
    let created = Utc
        .timestamp_opt(1_790_000_000 + i64::try_from(n).unwrap_or(0), 0)
        .single()
        .ok_or_else(internal)?;
    let widget = Widget {
        id: Ulid::from_parts(1_790_000_000_000 + n, u128::from(n)),
        name: body.name,
        shape: body.shape,
        tags: body.tags,
        blob: body.blob,
        due: body.due,
        version: format!("v{n}.1"),
        created,
    };
    widgets.insert(widget.id, widget.clone());
    Ok(MsgPack(widget).customize().with_status(StatusCode::CREATED))
}

/// Read a widget.
#[utoipa::path(
    get,
    path = "/demo/widgets/{id}",
    tag = "demo",
    operation_id = "get_widget",
    params(("id" = String, Path, description = "Widget ID (ULID)."), GetWidgetQuery),
    responses((status = 200, description = "The widget.", body = Widget)),
)]
pub async fn get_widget(
    state: web::Data<DemoState>,
    id: web::Path<Ulid>,
    query: web::Query<GetWidgetQuery>,
) -> Result<MsgPack<Widget>, Problem> {
    let guard = state.widgets.lock().map_err(|_| internal())?;
    let mut widget = guard
        .1
        .get(&id)
        .cloned()
        .ok_or_else(|| Problem::new(ProblemType::NotFound))?;
    if !query.verbose.unwrap_or(false) {
        widget.tags.clear();
    }
    Ok(MsgPack(widget))
}

/// Rename a widget (optimistic concurrency).
#[utoipa::path(
    put,
    path = "/demo/widgets/{id}",
    tag = "demo",
    operation_id = "rename_widget",
    params(
        ("id" = String, Path, description = "Widget ID (ULID)."),
        ("If-Match" = String, Header, description = "Expected current version."),
    ),
    request_body = RenameWidget,
    responses(
        (status = 200, description = "Renamed.", body = Widget),
        (status = 409, description = "`version_conflict`.", body = Problem),
    ),
)]
pub async fn rename_widget(
    req: HttpRequest,
    state: web::Data<DemoState>,
    id: web::Path<Ulid>,
    body: MsgPack<RenameWidget>,
) -> Result<MsgPack<Widget>, Problem> {
    let if_match = req
        .headers()
        .get("if-match")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            Problem::new(ProblemType::InvalidParameter).with_detail("If-Match is required")
        })?;
    let mut guard = state.widgets.lock().map_err(|_| internal())?;
    let widget = guard
        .1
        .get_mut(&id)
        .ok_or_else(|| Problem::new(ProblemType::NotFound))?;
    if widget.version != if_match {
        return Err(Problem::version_conflict(widget.version.clone()));
    }
    let next = widget
        .version
        .rsplit_once('.')
        .and_then(|(_, n)| n.parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    widget.version = format!("{}.{next}", widget.version.split('.').next().unwrap_or("v"));
    widget.name = body.into_inner().name;
    Ok(MsgPack(widget.clone()))
}

/// Delete a widget.
#[utoipa::path(
    delete,
    path = "/demo/widgets/{id}",
    tag = "demo",
    operation_id = "delete_widget",
    params(("id" = String, Path, description = "Widget ID (ULID).")),
    responses((status = 204, description = "Deleted.")),
)]
pub async fn delete_widget(
    state: web::Data<DemoState>,
    id: web::Path<Ulid>,
) -> Result<HttpResponse, Problem> {
    let mut guard = state.widgets.lock().map_err(|_| internal())?;
    guard
        .1
        .remove(&id)
        .ok_or_else(|| Problem::new(ProblemType::NotFound))?;
    Ok(HttpResponse::NoContent().finish())
}

/// The authenticated caller (bearer `good-token`).
#[utoipa::path(
    get,
    path = "/demo/whoami",
    tag = "demo",
    operation_id = "whoami",
    responses((status = 200, description = "The caller.", body = WhoAmI)),
)]
pub async fn whoami(req: HttpRequest) -> Result<MsgPack<WhoAmI>, Problem> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if token == Some(GOOD_TOKEN) {
        Ok(MsgPack(WhoAmI {
            subject: "demo-user".to_owned(),
        }))
    } else {
        Err(Problem::new(ProblemType::Unauthorized))
    }
}

/// `GET /demo/ticks`: replays retained ticks after `resume_from` (0 for a fresh
/// subscription), then `end`.
pub async fn ticks(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<DemoState>,
    query: web::Query<ResumeQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (mut frames, last) = {
        let buffer = state.ticks.lock().map_err(|_| internal())?;
        (
            buffer.resume(Some(query.resume_from.unwrap_or(0))),
            buffer.last_seq(),
        )
    };
    if !matches!(frames.last(), Some(Frame::Reset { .. })) {
        frames.push(Frame::End { seq: last });
    }
    let script = state.next_stream.lock().map_err(|_| internal())?.take();
    let hold = match script {
        Some(StreamScript::CutAfter(n)) => {
            frames.truncate(n);
            false
        }
        Some(StreamScript::HoldAfter(n)) => {
            frames.truncate(n);
            true
        }
        None => false,
    };
    let frames = futures_util::stream::iter(frames);
    if hold {
        ws::start(
            &req,
            body,
            state.ws,
            frames.chain(futures_util::stream::pending()),
        )
    } else {
        ws::start(&req, body, state.ws, frames)
    }
}

/// Stream operations of the demo.
pub const STREAMS: &[StreamOperation] = &[StreamOperation {
    path: "/demo/ticks",
    operation_id: "demo_ticks",
    tag: "demo",
    summary: "Resumable stream of demo ticks.",
    payload: "DemoTick",
    secured: false,
}];

#[derive(Debug, OpenApi)]
#[openapi(
    paths(create_widget, get_widget, rename_widget, delete_widget, whoami),
    components(schemas(DemoTick)),
    tags((name = "demo", description = "Wire machinery demo (tests only)."))
)]
struct DemoDoc;

/// The demo contract: production operations plus the demo ones.
pub fn document() -> Value {
    let mut api = ApiDoc::openapi();
    api.merge(DemoDoc::openapi());
    build(&api, STREAMS)
}

/// Demo routes relative to `/api/v1`.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/demo/widgets")
            .app_data(MsgPackConfig::default().with_body_limit(CREATE_BODY_LIMIT))
            .route(web::post().to(create_widget)),
    )
    .service(
        web::resource("/demo/widgets/{id}")
            .route(web::get().to(get_widget))
            .route(web::put().to(rename_widget))
            .route(web::delete().to(delete_widget)),
    )
    .route("/demo/whoami", web::get().to(whoami))
    .route("/demo/ticks", web::get().to(ticks));
}

/// App configuration: production routes plus the demo routes, sharing `state`.
pub fn configure(
    state: web::Data<DemoState>,
) -> impl Fn(&mut web::ServiceConfig) + Send + Clone + 'static {
    move |cfg| {
        cfg.app_data(state.clone()).service(api_v1(|c| {
            app::routes(c);
            routes(c);
        }));
    }
}
