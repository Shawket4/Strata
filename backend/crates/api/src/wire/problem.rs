//! RFC 7807 problem details carried as `application/problem+msgpack` (PLAN §7.5).
//!
//! This is the wire representation. The catalogue of problem types (slug, title, status) is
//! `strata_common::ProblemType` — the only list — and domain errors reach the wire through
//! `strata_common::DomainError` and [`Problem::from_domain`].

use std::fmt;

use actix_web::http::StatusCode;
use actix_web::http::header::ContentType;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use utoipa::ToSchema;

use strata_common::DomainError;

use super::{DecodeError, PROBLEM_MSGPACK, encode};

pub use strata_common::ProblemType;

/// The HTTP status of a problem type.
pub fn status_code(kind: ProblemType) -> StatusCode {
    StatusCode::from_u16(kind.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// RFC 7807 problem details: the error type of every handler.
///
/// Kept small (the rarely used extension members sit behind one allocation) so
/// `Result<T, Problem>` stays cheap. On the wire it is the flat [`ProblemDetails`] map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "ProblemDetails", from = "ProblemDetails")]
pub struct Problem {
    /// Problem type slug (wire field `type`).
    pub problem_type: String,
    /// Fixed summary of the problem type.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Occurrence-specific explanation. Never contains user content.
    pub detail: Option<String>,
    extensions: Option<Box<ProblemExtensions>>,
}

/// Optional RFC 7807 members and Strata's typed extension members.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProblemExtensions {
    /// URI reference identifying the occurrence.
    pub instance: Option<String>,
    /// `invalid_body` / `invalid_parameter`: what is wrong, and where.
    pub errors: Vec<ProblemFieldError>,
    /// `duplicate_candidates`: the existing items the create resembles.
    pub candidates: Vec<DuplicateCandidate>,
    /// `version_conflict`: the server's current version of the resource.
    pub current_version: Option<String>,
}

/// Wire shape of [`Problem`] (component schema `Problem`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(
    as = Problem,
    description = "RFC 7807 problem details, encoded as `application/problem+msgpack`. \
                   `type` is a problem slug (e.g. `not_found`, `duplicate_candidates`); \
                   clients treat unknown slugs by `status`."
)]
pub struct ProblemDetails {
    /// Problem type slug.
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Fixed summary of the problem type.
    pub title: String,
    /// HTTP status code.
    #[schema(minimum = 100, maximum = 599)]
    pub status: u16,
    /// Occurrence-specific explanation. Never contains user content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// URI reference identifying the occurrence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// `invalid_body` / `invalid_parameter`: what is wrong, and where.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ProblemFieldError>,
    /// `duplicate_candidates`: the existing items the create resembles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<DuplicateCandidate>,
    /// `version_conflict`: the server's current version of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
}

impl From<Problem> for ProblemDetails {
    fn from(p: Problem) -> Self {
        let ext = p.extensions.map(|e| *e).unwrap_or_default();
        Self {
            problem_type: p.problem_type,
            title: p.title,
            status: p.status,
            detail: p.detail,
            instance: ext.instance,
            errors: ext.errors,
            candidates: ext.candidates,
            current_version: ext.current_version,
        }
    }
}

impl From<ProblemDetails> for Problem {
    fn from(d: ProblemDetails) -> Self {
        let ext = ProblemExtensions {
            instance: d.instance,
            errors: d.errors,
            candidates: d.candidates,
            current_version: d.current_version,
        };
        Self {
            problem_type: d.problem_type,
            title: d.title,
            status: d.status,
            detail: d.detail,
            extensions: (ext != ProblemExtensions::default()).then(|| Box::new(ext)),
        }
    }
}

impl utoipa::PartialSchema for Problem {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <ProblemDetails as utoipa::PartialSchema>::schema()
    }
}

impl ToSchema for Problem {
    fn name() -> std::borrow::Cow<'static, str> {
        <ProblemDetails as ToSchema>::name()
    }

    fn schemas(
        schemas: &mut Vec<(
            String,
            utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
        )>,
    ) {
        <ProblemDetails as ToSchema>::schemas(schemas);
    }
}

/// One validation or decoding failure inside a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProblemFieldError {
    /// Stable machine-readable code (e.g. `trailing_bytes`, `schema_mismatch`).
    pub code: String,
    /// JSON Pointer (RFC 6901) into the decoded body, or the parameter name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    /// Content-free explanation.
    pub message: String,
}

/// How a duplicate candidate matched (§9.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchLevel {
    /// Same normalised key.
    Exact,
    /// Trigram / fuzzy match.
    Near,
    /// Embedding similarity.
    Semantic,
}

/// An existing item a create request resembles (`409 duplicate_candidates`, §7.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DuplicateCandidate {
    /// ID of the existing item.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Item kind (`note`, `capture`, `task`, `entity`, `concept`, `alias`, ...).
    pub kind: String,
    /// Display title.
    pub title: String,
    /// Short excerpt around the match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// How it matched.
    pub match_level: MatchLevel,
    /// Similarity in `[0, 1]`.
    #[schema(minimum = 0.0, maximum = 1.0)]
    pub score: f64,
}

impl Problem {
    /// A problem of `kind` with its fixed title and status and no extensions.
    pub fn new(kind: ProblemType) -> Self {
        Self {
            problem_type: kind.slug().to_owned(),
            title: kind.title().to_owned(),
            status: kind.status(),
            detail: None,
            extensions: None,
        }
    }

    /// Optional members; `None` when all are empty.
    pub fn extensions(&self) -> Option<&ProblemExtensions> {
        self.extensions.as_deref()
    }

    fn extensions_mut(&mut self) -> &mut ProblemExtensions {
        self.extensions.get_or_insert_with(Box::default)
    }

    /// `instance`, if set.
    pub fn instance(&self) -> Option<&str> {
        self.extensions().and_then(|e| e.instance.as_deref())
    }

    /// `errors` (empty unless `invalid_body` / `invalid_parameter`).
    pub fn errors(&self) -> &[ProblemFieldError] {
        self.extensions().map_or(&[], |e| &e.errors)
    }

    /// `candidates` (empty unless `duplicate_candidates`).
    pub fn candidates(&self) -> &[DuplicateCandidate] {
        self.extensions().map_or(&[], |e| &e.candidates)
    }

    /// `current_version` (only for `version_conflict`).
    pub fn current_version(&self) -> Option<&str> {
        self.extensions().and_then(|e| e.current_version.as_deref())
    }

    /// Sets `instance`.
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.extensions_mut().instance = Some(instance.into());
        self
    }

    /// Adds an occurrence-specific explanation. Must not contain user content.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Adds a field error.
    #[must_use]
    pub fn with_error(mut self, error: ProblemFieldError) -> Self {
        self.extensions_mut().errors.push(error);
        self
    }

    /// `422 invalid_body` for a body that failed decoding.
    pub fn invalid_body(err: &DecodeError) -> Self {
        let message = err.to_string();
        Self::new(ProblemType::InvalidBody)
            .with_detail(message.clone())
            .with_error(ProblemFieldError {
                code: err.code().to_owned(),
                pointer: err.pointer().map(str::to_owned),
                message,
            })
    }

    /// `409 duplicate_candidates` carrying the candidates.
    pub fn duplicate_candidates(candidates: Vec<DuplicateCandidate>) -> Self {
        let mut p = Self::new(ProblemType::DuplicateCandidates)
            .with_detail("resend with force = true to create it anyway");
        p.extensions_mut().candidates = candidates;
        p
    }

    /// `409 version_conflict` carrying the server's current version.
    pub fn version_conflict(current_version: impl Into<String>) -> Self {
        let mut p = Self::new(ProblemType::VersionConflict);
        p.extensions_mut().current_version = Some(current_version.into());
        p
    }

    /// The problem for a domain error. Server errors (`5xx`) are logged here with their cause
    /// and answered without any detail; other errors carry only their content-free
    /// [`DomainError::public_detail`].
    pub fn from_domain<E: DomainError + ?Sized>(err: &E) -> Self {
        let kind = err.problem_type();
        if kind.is_server_error() {
            tracing::error!(error = %err, problem = kind.slug(), "request failed");
            return Self::new(kind);
        }
        let problem = Self::new(kind);
        match err.public_detail() {
            Some(detail) => problem.with_detail(detail),
            None => problem,
        }
    }

    /// `500 internal` for an unexpected error; the cause is logged, never returned.
    pub fn internal(cause: &dyn std::fmt::Display) -> Self {
        tracing::error!(error = %cause, "internal error");
        Self::new(ProblemType::Internal)
    }

    /// The known type of this problem, if its slug is known.
    pub fn kind(&self) -> Option<ProblemType> {
        ProblemType::from_slug(&self.problem_type)
    }

    /// The problem encoded as MessagePack.
    pub fn to_msgpack(&self) -> Vec<u8> {
        // A Problem contains only strings, integers, floats and vectors of those: encoding into
        // a Vec cannot fail.
        encode(self).unwrap_or_default()
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<ProblemType> for Problem {
    fn from(kind: ProblemType) -> Self {
        Self::new(kind)
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status, self.problem_type, self.title)
    }
}

impl std::error::Error for Problem {}

impl ResponseError for Problem {
    fn status_code(&self) -> StatusCode {
        Problem::status_code(self)
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(Problem::status_code(self))
            .insert_header(ContentType(problem_mime()))
            .body(self.to_msgpack())
    }
}

impl Responder for Problem {
    type Body = actix_web::body::BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse {
        self.error_response()
    }
}

fn problem_mime() -> mime::Mime {
    // Invariant: PROBLEM_MSGPACK is a valid media type literal.
    PROBLEM_MSGPACK
        .parse()
        .unwrap_or(mime::APPLICATION_OCTET_STREAM)
}

/// Every `errors[].code` the wire layer can emit for undecodable bodies.
pub fn decode_error_codes() -> [&'static str; 12] {
    [
        "empty_body",
        "truncated",
        "trailing_bytes",
        "depth_exceeded",
        "string_too_long",
        "binary_too_long",
        "array_too_long",
        "map_too_long",
        "invalid_utf8",
        "extension_not_allowed",
        "invalid_marker",
        "schema_mismatch",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalogue_type_builds_a_problem_with_its_status() {
        for kind in ProblemType::ALL.iter().copied() {
            let problem = Problem::new(kind);
            assert_eq!(problem.status, kind.status());
            assert_eq!(problem.problem_type, kind.slug());
            assert_eq!(problem.title, kind.title());
            assert_eq!(problem.kind(), Some(kind));
            assert_eq!(status_code(kind).as_u16(), kind.status());
        }
    }

    #[derive(Debug, thiserror::Error)]
    enum Sample {
        #[error("taken")]
        Taken,
        #[error("database exploded: secret")]
        Db,
    }

    impl DomainError for Sample {
        fn problem_type(&self) -> ProblemType {
            match self {
                Self::Taken => ProblemType::UsernameTaken,
                Self::Db => ProblemType::Internal,
            }
        }

        fn public_detail(&self) -> Option<std::borrow::Cow<'static, str>> {
            Some("detail".into())
        }
    }

    #[test]
    fn domain_errors_map_to_problems_without_leaking_server_causes() {
        assert_eq!(
            Problem::from_domain(&Sample::Taken),
            Problem::new(ProblemType::UsernameTaken).with_detail("detail")
        );
        assert_eq!(
            Problem::from_domain(&Sample::Db),
            Problem::new(ProblemType::Internal)
        );
    }

    #[test]
    fn problem_stays_small_and_extensions_normalise() {
        assert!(std::mem::size_of::<Problem>() <= 96);
        let plain = Problem::new(ProblemType::NotFound);
        let round: Problem = ProblemDetails::from(plain.clone()).into();
        assert_eq!(round, plain);
        assert_eq!(round.extensions(), None);
        let conflict = Problem::version_conflict("v2");
        assert_eq!(conflict.current_version(), Some("v2"));
        assert_eq!(conflict.errors(), &[]);
    }
}
