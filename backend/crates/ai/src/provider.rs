//! The provider trait (PLAN §9.1).

use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::{PauseReason, ProviderError};
use crate::request::{ChatRequest, JsonCompletion, JsonRequest, TokenStream};

/// An LLM backend. Implementations do transport only: routing, budgets and schema validation
/// live in [`crate::AiService`].
///
/// `complete_json` returns the parsed value together with its token usage and model, because
/// the budget guard must account every call (the plan's sketch returns the bare value).
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync + fmt::Debug {
    /// Stable provider name recorded in `ai_usage.provider` (`claude_cli`, `anthropic_api`, …).
    fn name(&self) -> &'static str;

    /// The configured model (`default` when the provider picks it).
    fn model(&self) -> &str;

    /// Current health, for `GET /ai/status`.
    fn health(&self) -> ProviderHealth;

    /// One structured call: the reply is parsed as JSON (not yet validated).
    async fn complete_json(&self, req: JsonRequest) -> Result<JsonCompletion, ProviderError>;

    /// One streamed free-text call.
    async fn stream(&self, req: ChatRequest) -> Result<TokenStream, ProviderError>;
}

/// Provider health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderHealth {
    /// Current state.
    pub state: HealthState,
    /// Kind of the most recent failure, if the last call failed (content-free).
    pub last_error: Option<String>,
    /// When the most recent failure happened.
    pub last_error_at: Option<DateTime<Utc>>,
}

impl ProviderHealth {
    /// A healthy provider with no recorded failure.
    pub fn ready() -> Self {
        Self {
            state: HealthState::Ready,
            last_error: None,
            last_error_at: None,
        }
    }
}

/// Provider state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HealthState {
    /// Accepting calls.
    Ready,
    /// Not accepting calls until `until` (usage limit).
    Paused {
        /// Why.
        reason: PauseReason,
        /// When calls resume, if known.
        until: Option<DateTime<Utc>>,
    },
    /// The last call failed for a reason other than a pause; calls are still attempted.
    Degraded,
}

/// Records failures for [`ProviderHealth`] (shared by the real providers).
#[derive(Debug, Default)]
pub(crate) struct HealthTracker {
    inner: std::sync::Mutex<TrackerState>,
}

#[derive(Debug, Default)]
struct TrackerState {
    paused: Option<(PauseReason, Option<DateTime<Utc>>)>,
    last_error: Option<(String, DateTime<Utc>)>,
    failing: bool,
}

impl HealthTracker {
    fn lock(&self) -> std::sync::MutexGuard<'_, TrackerState> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The active pause at `now`, if any (expired pauses are cleared).
    pub(crate) fn active_pause(
        &self,
        now: DateTime<Utc>,
    ) -> Option<(PauseReason, Option<DateTime<Utc>>)> {
        let mut s = self.lock();
        match s.paused {
            Some((_, Some(until))) if until <= now => {
                s.paused = None;
                None
            }
            other => other,
        }
    }

    pub(crate) fn success(&self) {
        let mut s = self.lock();
        s.failing = false;
        s.paused = None;
    }

    pub(crate) fn failure(&self, err: &ProviderError, now: DateTime<Utc>) {
        let mut s = self.lock();
        if let ProviderError::Paused { reason, until } = err {
            s.paused = Some((*reason, *until));
        }
        s.failing = true;
        s.last_error = Some((error_kind(err).to_owned(), now));
    }

    pub(crate) fn snapshot(&self, now: DateTime<Utc>) -> ProviderHealth {
        let pause = self.active_pause(now);
        let s = self.lock();
        let state = match pause {
            Some((reason, until)) => HealthState::Paused { reason, until },
            None if s.failing => HealthState::Degraded,
            None => HealthState::Ready,
        };
        ProviderHealth {
            state,
            last_error: s.last_error.as_ref().map(|(k, _)| k.clone()),
            last_error_at: s.last_error.as_ref().map(|(_, at)| *at),
        }
    }
}

/// Content-free kind of a provider error.
pub fn error_kind(err: &ProviderError) -> &'static str {
    match err {
        ProviderError::Paused { .. } => "paused",
        ProviderError::Timeout(_) => "timeout",
        ProviderError::Auth => "auth",
        ProviderError::Rejected(_) => "rejected",
        ProviderError::Unavailable(_) => "unavailable",
        ProviderError::Truncated => "truncated",
        ProviderError::Refused => "refused",
        ProviderError::Protocol(_) => "protocol",
        ProviderError::MissingFixture { .. } => "missing_fixture",
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn tracker_reports_pause_until_it_expires_then_degraded_until_success() {
        let t = HealthTracker::default();
        let now = DateTime::from_timestamp(1_790_510_400, 0).expect("valid");
        assert_eq!(t.snapshot(now), ProviderHealth::ready());

        let until = now + Duration::hours(2);
        t.failure(
            &ProviderError::Paused {
                reason: PauseReason::ProviderUsageLimit,
                until: Some(until),
            },
            now,
        );
        assert_eq!(
            t.snapshot(now + Duration::hours(1)),
            ProviderHealth {
                state: HealthState::Paused {
                    reason: PauseReason::ProviderUsageLimit,
                    until: Some(until)
                },
                last_error: Some("paused".into()),
                last_error_at: Some(now),
            }
        );
        let later = now + Duration::hours(2);
        assert_eq!(t.active_pause(later), None);
        assert_eq!(t.snapshot(later).state, HealthState::Degraded);
        t.success();
        assert_eq!(t.snapshot(later).state, HealthState::Ready);
        assert_eq!(t.snapshot(later).last_error, Some("paused".into()));
    }
}
