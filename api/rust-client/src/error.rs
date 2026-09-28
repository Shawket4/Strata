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
    /// The TLS configuration could not be built (e.g. a malformed extra root certificate).
    #[error("tls configuration: {0}")]
    Tls(String),
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

    /// Why the server could not be reached, for [`Error::Transport`] and [`Error::WebSocket`]
    /// failures that happened before a response (`None` for every other error, including
    /// HTTP-level WebSocket handshake refusals).
    pub fn transport_kind(&self) -> Option<TransportKind> {
        match self {
            Self::Transport(e) => {
                if chain_has_tls(e) {
                    Some(TransportKind::Tls)
                } else if e.is_timeout() {
                    Some(TransportKind::Timeout)
                } else if chain_has_dns(e) {
                    Some(TransportKind::Dns)
                } else if e.is_connect() {
                    Some(TransportKind::Connect)
                } else if e.is_status() || e.is_decode() || e.is_builder() {
                    None
                } else {
                    Some(TransportKind::Network)
                }
            }
            Self::WebSocket(e) => {
                use tokio_tungstenite::tungstenite::Error as Ws;
                match e.as_ref() {
                    Ws::Tls(_) => Some(TransportKind::Tls),
                    Ws::Io(io) if io_is_tls(io) => Some(TransportKind::Tls),
                    Ws::Io(io) => Some(match io.kind() {
                        std::io::ErrorKind::TimedOut => TransportKind::Timeout,
                        std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::HostUnreachable
                        | std::io::ErrorKind::NetworkUnreachable
                        | std::io::ErrorKind::AddrNotAvailable => TransportKind::Connect,
                        _ => TransportKind::Network,
                    }),
                    Ws::ConnectionClosed | Ws::AlreadyClosed => Some(TransportKind::Network),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Why a request or handshake never got a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportKind {
    /// The TLS handshake failed (unknown issuer, expired or mismatched certificate, protocol).
    Tls,
    /// The host name did not resolve.
    Dns,
    /// The TCP connection was refused or the network is unreachable.
    Connect,
    /// The request or connection timed out.
    Timeout,
    /// Another I/O failure (connection dropped, …).
    Network,
}

impl TransportKind {
    /// Stable code (`tls`, `dns`, `connect`, `timeout`, `network`).
    pub fn code(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::Dns => "dns",
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::Network => "network",
        }
    }
}

/// Whether `e` or one of its causes is a rustls error. `std::io::Error::source` skips the
/// wrapped error, so an I/O node is looked into explicitly (tokio-rustls reports handshake
/// failures as an I/O error wrapping the `rustls::Error`).
fn chain_has_tls(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut node = Some(e);
    while let Some(err) = node {
        if err.is::<rustls::Error>() {
            return true;
        }
        if let Some(io) = err.downcast_ref::<std::io::Error>()
            && io_is_tls(io)
        {
            return true;
        }
        node = err.source();
    }
    false
}

fn io_is_tls(io: &std::io::Error) -> bool {
    io.get_ref().is_some_and(|inner| chain_has_tls(inner))
}

/// Whether one of the causes is hyper-util's "dns error" (reqwest resolves through it).
fn chain_has_dns(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut node = Some(e);
    while let Some(err) = node {
        if err.to_string() == "dns error" {
            return true;
        }
        node = err.source();
    }
    false
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
