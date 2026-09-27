//! Structured JSON logging (`tracing`) with content redaction (PLAN §8, §14, §15).
//!
//! Request logs carry the method, the path **without its query string** (queries carry
//! search terms and note paths), the status and the duration — never headers (tokens),
//! bodies (note content, passwords) or the client's query. Handlers log IDs, never content.

use std::time::Instant;

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use tracing_subscriber::EnvFilter;

/// Installs the global JSON subscriber. The filter comes from `RUST_LOG` (default `info`).
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_current_span(false)
        .with_target(true)
        .try_init();
}

/// The part of a request target that may be logged: the path, with the query dropped and
/// path segments longer than 64 bytes (possible content, e.g. a note path) elided.
pub fn loggable_path(target: &str) -> String {
    let path = target.split(['?', '#']).next().unwrap_or_default();
    path.split('/')
        .map(|segment| if segment.len() > 64 { "…" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

/// Middleware logging one line per request (see the module docs for what is left out).
pub async fn log_request(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let started = Instant::now();
    let method = req.method().to_string();
    let path = loggable_path(req.path());
    let result = next.call(req).await;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match &result {
        Ok(res) => tracing::info!(
            method = %method,
            path = %path,
            status = res.status().as_u16(),
            elapsed_ms,
            "request"
        ),
        Err(err) => tracing::warn!(
            method = %method,
            path = %path,
            status = err.as_response_error().status_code().as_u16(),
            elapsed_ms,
            "request failed"
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_and_long_segments_never_reach_the_log() {
        assert_eq!(
            loggable_path("/api/v1/search?q=secret+plans"),
            "/api/v1/search"
        );
        assert_eq!(loggable_path("/api/v1/me#x"), "/api/v1/me");
        let long = "x".repeat(65);
        assert_eq!(
            loggable_path(&format!("/api/v1/notes/{long}/history")),
            "/api/v1/notes/…/history"
        );
        assert_eq!(
            loggable_path("/api/v1/devices/01M3HBS0G00000000000000001"),
            "/api/v1/devices/01M3HBS0G00000000000000001"
        );
    }
}
