//! Requeueing jobs that failed because the AI provider was down (owner decision 2026-09-28).
//!
//! A job that runs out of attempts with [`crate::JobError::Provider`] stays `failed` with its
//! `provider_failure` mark. When an LLM job then succeeds, the provider works again: the
//! runner requeues that user's marked jobs at once and records the time here; the hourly
//! scheduler pass requeues every other user's jobs marked before that time. The time lives
//! in memory: after a restart the first successful LLM job sets it again.

use std::sync::Mutex;

use chrono::{DateTime, Utc};

/// When an LLM job last succeeded in this process.
#[derive(Debug, Default)]
pub struct LlmRecovery {
    last_success: Mutex<Option<DateTime<Utc>>>,
}

impl LlmRecovery {
    /// Records a successful LLM job at `at` (never moves back).
    pub fn note_success(&self, at: DateTime<Utc>) {
        let mut last = self
            .last_success
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.is_none_or(|t| t < at) {
            *last = Some(at);
        }
    }

    /// The last successful LLM job, if any since start.
    pub fn last_success(&self) -> Option<DateTime<Utc>> {
        *self
            .last_success
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
