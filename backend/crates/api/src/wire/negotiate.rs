//! Content negotiation and the problem catch-all.
//!
//! - [`require_msgpack`] (middleware, via `actix_web::middleware::from_fn`): `406
//!   not_acceptable` when `Accept` excludes `application/msgpack`.
//! - [`problemize`] (an `ErrorHandlers` default handler): turns any error response that is not
//!   already problem details (actix routing 404/405, handshake errors, ...) into one, so
//!   nothing but MessagePack ever leaves the API (L21).

use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::http::header::{ACCEPT, CONTENT_TYPE, HeaderMap};
use actix_web::middleware::{ErrorHandlerResponse, Next};
use actix_web::{Error, ResponseError};

use super::{MSGPACK, PROBLEM_MSGPACK, Problem, ProblemType};

/// Whether an `Accept` header (absent = anything) allows `application/msgpack`.
///
/// The most specific matching media range decides (`application/msgpack` over
/// `application/*` over `*/*`); `q=0` excludes. Unparseable ranges are ignored.
pub fn accepts_msgpack(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(ACCEPT).peekable();
    if values.peek().is_none() {
        return true;
    }
    let (want_type, want_sub) = MSGPACK.split_once('/').unwrap_or((MSGPACK, ""));
    // (specificity, q) of the best match so far.
    let mut best: Option<(u8, f32)> = None;
    for value in values {
        let Ok(value) = value.to_str() else { continue };
        for range in value.split(',') {
            let mut parts = range.split(';');
            let media = parts.next().unwrap_or("").trim();
            let Some((ty, sub)) = media.split_once('/') else {
                continue;
            };
            let specificity =
                if ty.eq_ignore_ascii_case(want_type) && sub.eq_ignore_ascii_case(want_sub) {
                    3
                } else if ty.eq_ignore_ascii_case(want_type) && sub == "*" {
                    2
                } else if ty == "*" && sub == "*" {
                    1
                } else {
                    continue;
                };
            let q = parts
                .filter_map(|p| {
                    p.trim()
                        .strip_prefix("q=")
                        .or_else(|| p.trim().strip_prefix("Q="))
                })
                .find_map(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            if best.is_none_or(|(s, _)| specificity > s) {
                best = Some((specificity, q));
            }
        }
    }
    best.is_some_and(|(_, q)| q > 0.0)
}

/// Middleware: rejects requests whose `Accept` excludes MessagePack with `406`.
pub async fn require_msgpack(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<EitherBody<impl MessageBody>>, Error> {
    if accepts_msgpack(req.headers()) {
        next.call(req)
            .await
            .map(ServiceResponse::map_into_left_body)
    } else {
        let problem = Problem::new(ProblemType::NotAcceptable)
            .with_detail(format!("responses are {MSGPACK}"));
        Ok(req
            .into_response(problem.error_response())
            .map_into_right_body())
    }
}

/// `ErrorHandlers` default handler: replaces non-problem error bodies with problem details.
pub fn problemize<B>(res: ServiceResponse<B>) -> actix_web::Result<ErrorHandlerResponse<B>> {
    let is_problem = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case(PROBLEM_MSGPACK));
    if is_problem {
        return Ok(ErrorHandlerResponse::Response(res.map_into_left_body()));
    }
    let status = res.status();
    let problem = problem_for_status(status);
    let (req, _) = res.into_parts();
    let res = ServiceResponse::new(req, problem.error_response());
    Ok(ErrorHandlerResponse::Response(res.map_into_right_body()))
}

/// The problem used for a bare error status.
pub fn problem_for_status(status: StatusCode) -> Problem {
    let kind = match status.as_u16() {
        401 => ProblemType::Unauthorized,
        403 => ProblemType::Forbidden,
        404 => ProblemType::RouteNotFound,
        405 => ProblemType::MethodNotAllowed,
        406 => ProblemType::NotAcceptable,
        413 => ProblemType::PayloadTooLarge,
        415 => ProblemType::UnsupportedMediaType,
        429 => ProblemType::RateLimited,
        s if s >= 500 => ProblemType::Internal,
        _ => ProblemType::BadRequest,
    };
    let mut problem = Problem::new(kind);
    if kind == ProblemType::BadRequest {
        problem.status = status.as_u16();
    }
    problem
}

#[cfg(test)]
mod tests {
    use actix_web::http::header::HeaderValue;

    use super::*;

    fn with_accept(values: &[&str]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for v in values {
            h.append(ACCEPT, HeaderValue::from_str(v).expect("valid header"));
        }
        h
    }

    #[test]
    fn accept_header_matrix() {
        let cases: [(&[&str], bool); 12] = [
            (&[], true),
            (&["*/*"], true),
            (&["application/*"], true),
            (&["application/msgpack"], true),
            (&["Application/MsgPack"], true),
            (&["application/json"], false),
            (&["text/html, application/json;q=0.9"], false),
            (&["application/json", "application/msgpack;q=0.1"], true),
            (&["*/*, application/msgpack;q=0"], false),
            (&["application/msgpack;q=0, */*"], false),
            (&["application/*;q=0, application/msgpack"], true),
            (&["garbage"], false),
        ];
        for (values, expected) in cases {
            assert_eq!(
                accepts_msgpack(&with_accept(values)),
                expected,
                "{values:?}"
            );
        }
    }
}
