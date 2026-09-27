//! `GET /events` (PLAN §7.5 Events, D24): the caller's event stream over a WebSocket, one
//! binary `MessagePack` frame per event. Authenticated on upgrade with the bearer token; see
//! [`crate::events`].

use actix_web::{HttpRequest, HttpResponse, web};

use crate::auth::{AuthState, Authenticated};
use crate::events::{self, EventBus};
use crate::openapi::StreamOperation;
use crate::wire::ws::{self, ResumeQuery, WsConfig};

/// The stream operation in the contract.
pub const STREAM: StreamOperation = StreamOperation {
    path: "/events",
    operation_id: "events",
    tag: "events",
    summary: "The caller's events (note/relation/entity/task/custody/suggestion changes, jobs, \
              clusters, integrity warnings, account closure); resumable with `resume_from`.",
    payload: "Event",
    secured: true,
};

/// Upgrades to the caller's event stream, replaying events after `resume_from`.
pub async fn events(
    req: HttpRequest,
    body: web::Payload,
    auth: Authenticated,
    state: web::Data<AuthState>,
    bus: web::Data<EventBus>,
    query: web::Query<ResumeQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let config = req
        .app_data::<web::Data<WsConfig>>()
        .map_or_else(WsConfig::default, |c| **c);
    let frames = events::connection(
        bus.into_inner(),
        state.revocations().clone(),
        auth.user_id(),
        auth.session,
        query.resume_from,
    );
    ws::start(&req, body, config, frames)
}

/// Mounts `GET /events`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/events", web::get().to(events));
}
