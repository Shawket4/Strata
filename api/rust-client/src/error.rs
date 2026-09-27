//! Client errors, with problem details decoded into typed variants.

use std::fmt;

use crate::types::{DuplicateCandidate, Problem};

/// A problem-details response, classified by its `type` slug.
#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// `404 not_found` (also everything outside the caller's scope).
    NotFound(Problem),
    /// `401 unauthorized` after the one refresh attempt.
    Unauthorized(Problem),
    /// `403 forbidden`.
    Forbidden(Problem),
    /// `403 account_pending`.
    AccountPending(Problem),
    /// `403 account_disabled`.
    AccountDisabled(Problem),
    /// `403 account_deletion_pending` (export-only session).
    AccountDeletionPending(Problem),
    /// `409 version_conflict`.
    VersionConflict {
        /// The server's current version, when sent.
        current_version: Option<String>,
        /// The full problem.
        problem: Problem,
    },
    /// `409 duplicate_candidates`: resend with `force` to create anyway.
    DuplicateCandidates {
        /// Existing items the create resembles.
        candidates: Vec<DuplicateCandidate>,
        /// The full problem.
        problem: Problem,
    },
    /// `422 invalid_body`.
    InvalidBody(Problem),
    /// `422 invalid_parameter`.
    InvalidParameter(Problem),
    /// `410 epoch_changed`: re-bootstrap sync.
    EpochChanged(Problem),
    /// `429 rate_limited`.
    RateLimited(Problem),
    /// Any other (or unknown) problem type; use `status`.
    Other(Problem),
}

impl ApiError {
    /// Classifies a decoded problem by its `type`.
    pub fn from_problem(problem: Problem) -> Self {
        match problem.type_.as_str() {
            "not_found" => Self::NotFound(problem),
            "unauthorized" => Self::Unauthorized(problem),
            "forbidden" => Self::Forbidden(problem),
            "account_pending" => Self::AccountPending(problem),
            "account_disabled" => Self::AccountDisabled(problem),
            "account_deletion_pending" => Self::AccountDeletionPending(problem),
            "version_conflict" => Self::VersionConflict {
                current_version: problem.current_version.clone(),
                problem,
            },
            "duplicate_candidates" => Self::DuplicateCandidates {
                candidates: problem.candidates.clone(),
                problem,
            },
            "invalid_body" => Self::InvalidBody(problem),
            "invalid_parameter" => Self::InvalidParameter(problem),
            "epoch_changed" => Self::EpochChanged(problem),
            "rate_limited" => Self::RateLimited(problem),
            _ => Self::Other(problem),
        }
    }

    /// The underlying problem.
    pub fn problem(&self) -> &Problem {
        match self {
            Self::NotFound(p)
            | Self::Unauthorized(p)
            | Self::Forbidden(p)
            | Self::AccountPending(p)
            | Self::AccountDisabled(p)
            | Self::AccountDeletionPending(p)
            | Self::InvalidBody(p)
            | Self::InvalidParameter(p)
            | Self::EpochChanged(p)
            | Self::RateLimited(p)
            | Self::Other(p)
            | Self::VersionConflict { problem: p, .. }
            | Self::DuplicateCandidates { problem: p, .. } => p,
        }
    }

    /// HTTP status.
    pub fn status(&self) -> u32 {
        self.problem().status
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p = self.problem();
        write!(f, "{} {}: {}", p.status, p.type_, p.title)
    }
}

/// Everything a client call can fail with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server answered with problem details (boxed to keep `Result`s small).
    #[error("{0}")]
    Api(Box<ApiError>),
    /// Network / HTTP failure.
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    /// WebSocket failure.
    #[error("websocket: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// The base URL or a built URL is invalid.
    #[error("invalid URL: {0}")]
    Url(String),
    /// A parameter cannot be sent (e.g. a `..` path segment).
    #[error("invalid parameter: {0}")]
    Param(String),
    /// The request body could not be encoded.
    #[error("cannot encode the request of {operation}: {message}")]
    Encode {
        /// Operation id.
        operation: &'static str,
        /// Cause.
        message: String,
    },
    /// A response or frame did not decode into the contract type.
    #[error("cannot decode the response of {operation}: {message}")]
    Decode {
        /// Operation id.
        operation: &'static str,
        /// Cause.
        message: String,
    },
    /// A response that is neither the documented success nor problem details.
    #[error("unexpected {status} response ({content_type:?}) from {operation}")]
    UnexpectedResponse {
        /// Operation id.
        operation: &'static str,
        /// HTTP status.
        status: u16,
        /// Content type, if any.
        content_type: Option<String>,
    },
    /// The token provider failed.
    #[error("token provider: {0}")]
    Auth(String),
    /// A stream ended because reconnecting was disabled or gave up.
    #[error("stream {operation} disconnected after {attempts} reconnect attempts")]
    StreamClosed {
        /// Operation id.
        operation: &'static str,
        /// Reconnect attempts made.
        attempts: u32,
    },
}

impl Error {
    /// The typed problem, if the server sent one.
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Self::Api(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        Self::Api(Box::new(e))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(e))
    }
}
