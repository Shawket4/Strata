//! Assembly of the `/api/v1` scope with the wire conventions applied.

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::middleware::{ErrorHandlers, from_fn};
use actix_web::{Error, Scope, web};

use crate::health;
use crate::wire::negotiate::{problemize, require_msgpack};
use crate::wire::{Problem, ProblemFieldError, ProblemType};

/// URL prefix of every API route.
pub const API_PREFIX: &str = "/api/v1";

/// Mounts the complete API (all production routes) on an `App`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(api_v1(routes));
}

/// Every production route, relative to [`API_PREFIX`]. Endpoint authors add theirs here and
/// list the handler in [`crate::openapi::ApiDoc`].
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health::health));
    crate::routes::auth::configure(cfg);
    crate::routes::me::configure(cfg);
    crate::routes::devices::configure(cfg);
    crate::routes::admin::configure(cfg);
    crate::vault::configure(cfg);
    crate::routes::sync::configure(cfg);
    crate::routes::events::configure(cfg);
    crate::routes::ai::configure(cfg);
    crate::routes::ai_pipelines::configure(cfg);
    crate::routes::graph::configure(cfg);
}

/// The `/api/v1` scope with `routes` mounted and the wire conventions applied:
/// - `406` unless `Accept` allows MessagePack;
/// - bearer tokens are authenticated (`crate::auth::middleware::authenticate`);
/// - every non-problem error response (unknown route, wrong method, ...) becomes problem
///   details;
/// - malformed path segments are `404 not_found`, malformed query strings `422
///   invalid_parameter`.
pub fn api_v1(
    routes: impl FnOnce(&mut web::ServiceConfig),
) -> Scope<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = Error,
        InitError = (),
    >,
> {
    web::scope(API_PREFIX)
        .app_data(
            web::PathConfig::default()
                .error_handler(|_, _| Problem::new(ProblemType::NotFound).into()),
        )
        .app_data(web::QueryConfig::default().error_handler(|err, _| {
            let message = match &err {
                actix_web::error::QueryPayloadError::Deserialize(e) => {
                    sanitize_query_error(&e.to_string())
                }
                _ => "invalid query string".to_owned(),
            };
            Problem::new(ProblemType::InvalidParameter)
                .with_detail(message.clone())
                .with_error(ProblemFieldError {
                    code: "invalid_query".to_owned(),
                    pointer: None,
                    message,
                })
                .into()
        }))
        .configure(routes)
        .default_service(web::to(|| async {
            Problem::new(ProblemType::RouteNotFound)
        }))
        .wrap(from_fn(crate::auth::middleware::authenticate))
        .wrap(from_fn(require_msgpack))
        .wrap(ErrorHandlers::new().default_handler(problemize))
}

/// Keeps query errors free of the offending value.
fn sanitize_query_error(msg: &str) -> String {
    if msg.starts_with("missing field `") {
        return msg.to_owned();
    }
    match msg.find(", expected ") {
        Some(idx) => format!("invalid value{}", &msg[idx..]),
        None => "invalid query string".to_owned(),
    }
}
