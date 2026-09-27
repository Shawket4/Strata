//! Error types. Messages never contain note content, prompts, model output or stderr text
//! (PLAN §15: content never in logs or errors); they carry kinds, counts and paths only.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Why AI work is paused. Paused work waits and resumes; it never fails (PLAN §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    /// The user's daily token or cost cap is reached.
    UserBudget,
    /// The global daily token or cost cap is reached.
    GlobalBudget,
    /// The `claude -p` subscription hit a usage limit (shared with interactive use).
    ProviderUsageLimit,
    /// The API provider kept answering `429` after all retries.
    ProviderRateLimit,
}

/// A provider call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider cannot serve calls until `until` (unknown: retry later).
    #[error("provider paused ({reason:?}) until {until:?}")]
    Paused {
        /// Why.
        reason: PauseReason,
        /// When calls may resume, if known.
        until: Option<DateTime<Utc>>,
    },
    /// The call exceeded its time limit; the process or request was cancelled.
    #[error("provider call timed out after {0:?}")]
    Timeout(Duration),
    /// Credentials are missing, invalid or not allowed.
    #[error("provider authentication failed")]
    Auth,
    /// The provider rejected the request as invalid (not retryable). Carries the error type.
    #[error("provider rejected the request: {0}")]
    Rejected(String),
    /// The provider is unreachable or failing (after retries). Carries a content-free cause.
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    /// The output hit the token limit before it was complete.
    #[error("model output was truncated at the token limit")]
    Truncated,
    /// The model declined the request.
    #[error("model refused the request")]
    Refused,
    /// The provider answered in a shape this code does not understand.
    #[error("malformed provider response: {0}")]
    Protocol(String),
    /// The fake provider has no fixture for this input (tests; record one at `path`).
    #[error(
        "no fixture for prompt {prompt_id} v{version} with input hash {input_hash}; record it at {path}"
    )]
    MissingFixture {
        /// Prompt id.
        prompt_id: String,
        /// Prompt version.
        version: u32,
        /// SHA-256 (hex) of the request input.
        input_hash: String,
        /// Where the fixture file is expected.
        path: String,
    },
}

/// A failure of the AI layer as seen by jobs and handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AiError {
    /// Work must wait (budget or provider limit); reschedule at `until` (or later if unknown).
    #[error("AI paused ({reason:?}) until {until:?}")]
    Paused {
        /// Why.
        reason: PauseReason,
        /// When work may resume, if known.
        until: Option<DateTime<Utc>>,
    },
    /// AI is disabled for this user (principle 6: everything else keeps working).
    #[error("AI is disabled for this user")]
    Disabled,
    /// The provider configured for this user is not set up in this process.
    #[error("provider {0} is not configured")]
    ProviderNotConfigured(&'static str),
    /// The provider failed.
    #[error(transparent)]
    Provider(ProviderError),
    /// The model's output never matched the schema (first attempt plus retries).
    #[error(
        "output of prompt {prompt_id} v{version} failed schema validation after {attempts} attempts: {errors:?}"
    )]
    InvalidOutput {
        /// Prompt id.
        prompt_id: String,
        /// Prompt version.
        version: u32,
        /// Attempts made (1 + retries).
        attempts: u32,
        /// Content-free violations of the last attempt (`<instance path>: <kind>`).
        errors: Vec<String>,
    },
    /// The validated output did not deserialize into the expected Rust type.
    #[error("output of prompt {prompt_id} v{version} does not fit its type: {message}")]
    OutputType {
        /// Prompt id.
        prompt_id: String,
        /// Prompt version.
        version: u32,
        /// Content-free serde message (path and expectation).
        message: String,
    },
    /// No prompt with this id/version is registered, or it has no schema.
    #[error("unknown prompt {0}")]
    UnknownPrompt(String),
    /// The request's schema is not a valid JSON schema.
    #[error("invalid output schema: {0}")]
    InvalidSchema(String),
    /// Usage accounting failed (database).
    #[error("usage store error: {0}")]
    Store(String),
    /// Invalid configuration.
    #[error("invalid AI configuration: {0}")]
    Config(String),
}

impl AiError {
    /// Whether the work should wait and resume rather than fail.
    pub fn is_pause(&self) -> bool {
        matches!(self, Self::Paused { .. })
    }
}

impl From<ProviderError> for AiError {
    fn from(e: ProviderError) -> Self {
        match e {
            ProviderError::Paused { reason, until } => Self::Paused { reason, until },
            other => Self::Provider(other),
        }
    }
}

impl From<strata_index::IndexError> for AiError {
    fn from(e: strata_index::IndexError) -> Self {
        Self::Store(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_pause_becomes_ai_pause_and_other_errors_wrap() {
        let until = DateTime::from_timestamp(1_790_553_600, 0).expect("valid");
        let e: AiError = ProviderError::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(until),
        }
        .into();
        assert_eq!(
            e,
            AiError::Paused {
                reason: PauseReason::ProviderUsageLimit,
                until: Some(until)
            }
        );
        assert!(e.is_pause());
        let e: AiError = ProviderError::Auth.into();
        assert_eq!(e, AiError::Provider(ProviderError::Auth));
        assert!(!e.is_pause());
        assert_eq!(e.to_string(), "provider authentication failed");
    }
}
