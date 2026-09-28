//! Errors of the vault store, each mapped to one problem type (PLAN §7.5 "Errors").

use std::borrow::Cow;

use strata_common::{DomainError, ProblemType};

/// A duplicate candidate returned with `409 duplicate_candidates` (PLAN §9.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The stored item ID (note ULID or task block ID), as in `dedupe_keys`.
    pub item: String,
    /// ID of the existing item (note ULID; for tasks the ULID inside `t-<ulid>`, else the
    /// containing note's ID).
    pub id: ulid::Ulid,
    /// Item kind (`note`, `capture`, `task`, `person`, …).
    pub kind: String,
    /// Display title.
    pub title: String,
    /// Short excerpt.
    pub snippet: Option<String>,
    /// `exact` or `near`. A semantic match (§9.7) is reported as `near` with
    /// [`Candidate::semantic`] set, so existing matches on [`MatchLevel`] keep compiling;
    /// the API maps it to the wire's `semantic` level.
    pub level: MatchLevel,
    /// Similarity in `[0, 1]` (the embedding cosine for semantic matches).
    pub score: f64,
    /// Whether the match came from embedding similarity (the semantic level).
    pub semantic: bool,
}

/// How a duplicate candidate matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchLevel {
    /// Same normalised key.
    Exact,
    /// Trigram similarity above the per-kind threshold.
    Near,
}

/// Errors returned by vault operations.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    /// Nonexistent, or outside the caller's scope (principle 7).
    #[error("not found")]
    NotFound,
    /// A path or file name is not Obsidian-safe (PLAN §6.2) or escapes the vault.
    #[error("invalid name: {0}")]
    InvalidName(&'static str),
    /// A request value is invalid (content-free reason).
    #[error("invalid input: {0}")]
    Invalid(Cow<'static, str>),
    /// `If-Match` is stale; carries the current version.
    #[error("version conflict")]
    VersionConflict {
        /// The server's current version.
        current: String,
    },
    /// A create resembles existing items.
    #[error("possible duplicate")]
    Duplicate(Vec<Candidate>),
    /// A note already exists at the target path.
    #[error("path already exists")]
    PathTaken,
    /// A revert conflicts with later changes.
    #[error("revert conflicts with later changes")]
    RevertConflict,
    /// The task's state does not allow the transition.
    #[error("task state conflict: {0}")]
    TaskState(Cow<'static, str>),
    /// An import archive was rejected.
    #[error("invalid archive: {0}")]
    InvalidArchive(Cow<'static, str>),
    /// An import archive (or one entry) is too large.
    #[error("archive too large: {0}")]
    ArchiveTooLarge(Cow<'static, str>),
    /// A create's device creation time is more than `max_skew_secs` ahead of the server's
    /// clock.
    #[error("created is more than {max_skew_secs} s in the future")]
    CreatedInFuture {
        /// The allowed skew (`VaultConfig::max_future_skew_secs`).
        max_skew_secs: u32,
    },
    /// Semantic and hybrid search need the AI subsystem (Phase 4).
    #[error("AI features are not available")]
    AiUnavailable,
    /// The vault actor stopped.
    #[error("vault writer unavailable")]
    WriterGone,
    /// Filesystem error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Git error.
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    /// Database error.
    #[error("index: {0}")]
    Index(#[from] strata_index::IndexError),
    /// Anything else that should never happen.
    #[error("internal: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for VaultError {
    fn from(e: sqlx::Error) -> Self {
        Self::Index(e.into())
    }
}

impl VaultError {
    /// An invalid-input error with a fixed message.
    pub fn invalid(msg: &'static str) -> Self {
        Self::Invalid(Cow::Borrowed(msg))
    }
}

impl DomainError for VaultError {
    fn problem_type(&self) -> ProblemType {
        match self {
            Self::NotFound => ProblemType::NotFound,
            Self::InvalidName(_) => ProblemType::InvalidName,
            Self::Invalid(_) => ProblemType::InvalidBody,
            Self::VersionConflict { .. } => ProblemType::VersionConflict,
            Self::Duplicate(_) => ProblemType::DuplicateCandidates,
            Self::PathTaken => ProblemType::PathTaken,
            Self::RevertConflict => ProblemType::RevertConflict,
            Self::TaskState(_) => ProblemType::TaskStateConflict,
            Self::InvalidArchive(_) => ProblemType::InvalidArchive,
            Self::ArchiveTooLarge(_) => ProblemType::PayloadTooLarge,
            Self::AiUnavailable => ProblemType::AiUnavailable,
            Self::CreatedInFuture { .. } => ProblemType::CreatedInFuture,
            Self::WriterGone | Self::Io(_) | Self::Git(_) | Self::Index(_) | Self::Internal(_) => {
                ProblemType::Internal
            }
        }
    }

    fn public_detail(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidName(reason) => Some(Cow::Borrowed(reason)),
            Self::Invalid(reason)
            | Self::TaskState(reason)
            | Self::InvalidArchive(reason)
            | Self::ArchiveTooLarge(reason) => Some(reason.clone()),
            Self::AiUnavailable => Some(Cow::Borrowed(
                "semantic and hybrid search need the AI subsystem, which is not available yet; use mode=keyword",
            )),
            Self::RevertConflict => Some(Cow::Borrowed(
                "the files changed after that commit in ways that conflict with reverting it",
            )),
            Self::PathTaken => Some(Cow::Borrowed("a note already exists at this path")),
            Self::CreatedInFuture { max_skew_secs } => Some(Cow::Owned(format!(
                "`created` is more than {max_skew_secs} seconds ahead of the server's clock; check the device's clock"
            ))),
            _ => None,
        }
    }
}

/// Result alias.
pub type Result<T, E = VaultError> = std::result::Result<T, E>;
