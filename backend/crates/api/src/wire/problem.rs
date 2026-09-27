//! RFC 7807 problem details carried as `application/problem+msgpack` (PLAN §7.5).
//!
//! This is the wire representation. When `strata-common::problem` lands, domain errors map
//! into [`Problem`]; the wire shape stays defined here, next to the codec and the contract.

use std::fmt;

use actix_web::http::StatusCode;
use actix_web::http::header::ContentType;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use utoipa::ToSchema;

use super::{DecodeError, PROBLEM_MSGPACK, encode};

/// Problem types known to the API. The wire field `type` carries [`ProblemType::slug`];
/// clients must treat unknown slugs as generic problems of the given `status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProblemType {
    /// 422: the body failed decoding or validation; see `errors`.
    InvalidBody,
    /// 422: a query or header parameter is invalid.
    InvalidParameter,
    /// 415: the request body is not `application/msgpack` (or is content-encoded).
    UnsupportedMediaType,
    /// 413: the body exceeds the route's limit.
    PayloadTooLarge,
    /// 406: the `Accept` header excludes MessagePack.
    NotAcceptable,
    /// 404: the resource does not exist in the caller's scope.
    NotFound,
    /// 404: no route matches the path.
    RouteNotFound,
    /// 405: the route exists but not for this method.
    MethodNotAllowed,
    /// 400: the request is malformed at the HTTP level.
    BadRequest,
    /// 401: missing, invalid or expired access token.
    Unauthorized,
    /// 403: authenticated but not allowed.
    Forbidden,
    /// 409: `If-Match` does not match; `current_version` carries the server's version.
    VersionConflict,
    /// 409: a create looks like a duplicate; `candidates` lists the matches (§9.7).
    DuplicateCandidates,
    /// 403: the account awaits admin approval.
    AccountPending,
    /// 403: the account is disabled.
    AccountDisabled,
    /// 403: the account is scheduled for deletion (export-only session).
    AccountDeletionPending,
    /// 410: the sync epoch changed; the client must re-bootstrap.
    EpochChanged,
    /// 429: rate limited.
    RateLimited,
    /// 500: unexpected server error (details are logged, never returned).
    Internal,
}

impl ProblemType {
    /// Every known type, for exhaustive tests and documentation.
    pub const ALL: [Self; 19] = [
        Self::InvalidBody,
        Self::InvalidParameter,
        Self::UnsupportedMediaType,
        Self::PayloadTooLarge,
        Self::NotAcceptable,
        Self::NotFound,
        Self::RouteNotFound,
        Self::MethodNotAllowed,
        Self::BadRequest,
        Self::Unauthorized,
        Self::Forbidden,
        Self::VersionConflict,
        Self::DuplicateCandidates,
        Self::AccountPending,
        Self::AccountDisabled,
        Self::AccountDeletionPending,
        Self::EpochChanged,
        Self::RateLimited,
        Self::Internal,
    ];

    /// The value of the wire field `type`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::InvalidBody => "invalid_body",
            Self::InvalidParameter => "invalid_parameter",
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::PayloadTooLarge => "payload_too_large",
            Self::NotAcceptable => "not_acceptable",
            Self::NotFound => "not_found",
            Self::RouteNotFound => "route_not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::BadRequest => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::VersionConflict => "version_conflict",
            Self::DuplicateCandidates => "duplicate_candidates",
            Self::AccountPending => "account_pending",
            Self::AccountDisabled => "account_disabled",
            Self::AccountDeletionPending => "account_deletion_pending",
            Self::EpochChanged => "epoch_changed",
            Self::RateLimited => "rate_limited",
            Self::Internal => "internal",
        }
    }

    /// HTTP status of this problem type.
    pub fn status(self) -> StatusCode {
        match self {
            Self::InvalidBody | Self::InvalidParameter => StatusCode::UNPROCESSABLE_ENTITY,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            Self::NotFound | Self::RouteNotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden
            | Self::AccountPending
            | Self::AccountDisabled
            | Self::AccountDeletionPending => StatusCode::FORBIDDEN,
            Self::VersionConflict | Self::DuplicateCandidates => StatusCode::CONFLICT,
            Self::EpochChanged => StatusCode::GONE,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Short, fixed, human-readable summary (RFC 7807 `title`).
    pub fn title(self) -> &'static str {
        match self {
            Self::InvalidBody => "Request body is invalid",
            Self::InvalidParameter => "Request parameter is invalid",
            Self::UnsupportedMediaType => "Unsupported media type",
            Self::PayloadTooLarge => "Request body too large",
            Self::NotAcceptable => "Response media type not acceptable",
            Self::NotFound => "Not found",
            Self::RouteNotFound => "No such route",
            Self::MethodNotAllowed => "Method not allowed",
            Self::BadRequest => "Bad request",
            Self::Unauthorized => "Authentication required",
            Self::Forbidden => "Forbidden",
            Self::VersionConflict => "Version conflict",
            Self::DuplicateCandidates => "Possible duplicate",
            Self::AccountPending => "Account awaiting approval",
            Self::AccountDisabled => "Account disabled",
            Self::AccountDeletionPending => "Account scheduled for deletion",
            Self::EpochChanged => "Sync epoch changed",
            Self::RateLimited => "Too many requests",
            Self::Internal => "Internal server error",
        }
    }

    /// Looks a slug up.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.slug() == slug)
    }
}

/// RFC 7807 problem details. Extension members are optional and typed per problem type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(
    description = "RFC 7807 problem details, encoded as `application/problem+msgpack`. \
                   `type` is a problem slug (e.g. `not_found`, `duplicate_candidates`); \
                   clients treat unknown slugs by `status`."
)]
pub struct Problem {
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
            status: kind.status().as_u16(),
            detail: None,
            instance: None,
            errors: Vec::new(),
            candidates: Vec::new(),
            current_version: None,
        }
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
        self.errors.push(error);
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
        p.candidates = candidates;
        p
    }

    /// `409 version_conflict` carrying the server's current version.
    pub fn version_conflict(current_version: impl Into<String>) -> Self {
        let mut p = Self::new(ProblemType::VersionConflict);
        p.current_version = Some(current_version.into());
        p
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
    fn slugs_round_trip_and_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in ProblemType::ALL {
            assert_eq!(ProblemType::from_slug(kind.slug()), Some(kind));
            assert!(seen.insert(kind.slug()), "duplicate slug {}", kind.slug());
            assert_eq!(Problem::new(kind).status, kind.status().as_u16());
        }
        assert_eq!(ProblemType::from_slug("nope"), None);
    }
}
