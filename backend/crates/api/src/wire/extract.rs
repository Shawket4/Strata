//! `MsgPack<T>`: the request extractor.

use std::ops::{Deref, DerefMut};

use actix_web::dev::Payload;
use actix_web::http::header::{CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::{FromRequest, HttpRequest, web};
use bytes::BytesMut;
use futures_util::StreamExt;
use futures_util::future::LocalBoxFuture;
use serde::de::DeserializeOwned;

use super::{DecodeLimits, MSGPACK, Problem, ProblemFieldError, ProblemType, decode};

/// A MessagePack body (request extractor) or response (see the `Responder` impl).
///
/// As an extractor it requires `Content-Type: application/vnd.msgpack` (else `415`), no
/// `Content-Encoding` (else `415`), a body within the route's [`MsgPackConfig::body_limit`]
/// (else `413`), and a body that decodes within [`DecodeLimits`] into `T` (else `422
/// invalid_body`). Every failure is `application/problem+msgpack`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MsgPack<T>(pub T);

impl<T> MsgPack<T> {
    /// Unwraps the value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for MsgPack<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for MsgPack<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// Per-route body configuration. Register with `.app_data(MsgPackConfig { .. })` on an
/// `App`, scope or resource; the innermost wins. Defaults apply when none is registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgPackConfig {
    /// Maximum body size in bytes.
    pub body_limit: usize,
    /// Structural decode limits.
    pub limits: DecodeLimits,
}

impl MsgPackConfig {
    /// Default body limit (256 KiB).
    pub const DEFAULT_BODY_LIMIT: usize = 256 * 1024;

    /// Same limits with a different body size.
    #[must_use]
    pub fn with_body_limit(mut self, body_limit: usize) -> Self {
        self.body_limit = body_limit;
        self
    }

    fn from_req(req: &HttpRequest) -> Self {
        req.app_data::<Self>()
            .copied()
            .or_else(|| req.app_data::<web::Data<Self>>().map(|d| *d.get_ref()))
            .unwrap_or_default()
    }
}

impl Default for MsgPackConfig {
    fn default() -> Self {
        Self {
            body_limit: Self::DEFAULT_BODY_LIMIT,
            limits: DecodeLimits::default(),
        }
    }
}

impl<T: DeserializeOwned + 'static> FromRequest for MsgPack<T> {
    type Error = Problem;
    type Future = LocalBoxFuture<'static, Result<Self, Problem>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let config = MsgPackConfig::from_req(req);
        let precheck = precheck(req, &config);
        let mut payload = payload.take();
        Box::pin(async move {
            precheck?;
            let mut body = BytesMut::new();
            while let Some(chunk) = payload.next().await {
                let chunk = chunk.map_err(|_| {
                    Problem::new(ProblemType::InvalidBody)
                        .with_detail("the request body could not be read")
                        .with_error(ProblemFieldError {
                            code: "incomplete_body".to_owned(),
                            pointer: None,
                            message: "the request body could not be read".to_owned(),
                        })
                })?;
                if body.len() + chunk.len() > config.body_limit {
                    return Err(too_large(config.body_limit));
                }
                body.extend_from_slice(&chunk);
            }
            decode(&body, &config.limits)
                .map(MsgPack)
                .map_err(|err| Problem::invalid_body(&err))
        })
    }
}

fn too_large(limit: usize) -> Problem {
    Problem::new(ProblemType::PayloadTooLarge)
        .with_detail(format!("the body limit for this route is {limit} bytes"))
}

/// Header checks done before reading the body.
fn precheck(req: &HttpRequest, config: &MsgPackConfig) -> Result<(), Problem> {
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<mime::Mime>().ok());
    let is_msgpack = content_type
        .as_ref()
        .is_some_and(|m| m.essence_str().eq_ignore_ascii_case(MSGPACK));
    if !is_msgpack {
        return Err(Problem::new(ProblemType::UnsupportedMediaType)
            .with_detail(format!("request bodies must be {MSGPACK}")));
    }
    let encoded = req
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !v.trim().eq_ignore_ascii_case("identity"));
    if encoded {
        return Err(Problem::new(ProblemType::UnsupportedMediaType)
            .with_detail("content-encoded request bodies are not accepted"));
    }
    let declared = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|len| len > config.body_limit) {
        return Err(too_large(config.body_limit));
    }
    Ok(())
}
