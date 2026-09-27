//! Backoff and an injectable sleeper, so retry delays are asserted in tests without sleeping.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// Waits between retries.
#[async_trait::async_trait]
pub trait Sleeper: Send + Sync + fmt::Debug {
    /// Waits `d`.
    async fn sleep(&self, d: Duration);
}

/// Real waiting (`tokio::time::sleep`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

#[async_trait::async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, d: Duration) {
        tokio::time::sleep(d).await;
    }
}

/// Records requested delays and returns at once (tests).
#[derive(Debug, Clone, Default)]
pub struct RecordingSleeper {
    delays: Arc<Mutex<Vec<Duration>>>,
}

impl RecordingSleeper {
    /// Every delay requested so far.
    pub fn delays(&self) -> Vec<Duration> {
        self.delays
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait::async_trait]
impl Sleeper for RecordingSleeper {
    async fn sleep(&self, d: Duration) {
        self.delays
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(d);
    }
}

/// Exponential backoff: `base * 2^attempt`, capped at `max`; a server-provided `retry-after`
/// wins when present (still capped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// First delay.
    pub base: Duration,
    /// Upper bound for any delay.
    pub max: Duration,
}

impl Backoff {
    /// Delay before retry number `attempt` (0-based).
    pub fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        let exp = self
            .base
            .saturating_mul(2u32.saturating_pow(attempt.min(31)));
        retry_after.unwrap_or(exp).min(self.max)
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            base: Duration::from_secs(1),
            max: Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_caps_and_prefers_retry_after() {
        let b = Backoff::default();
        let delays: Vec<u64> = (0..8).map(|a| b.delay(a, None).as_secs()).collect();
        assert_eq!(delays, vec![1, 2, 4, 8, 16, 32, 60, 60]);
        assert_eq!(
            b.delay(0, Some(Duration::from_secs(7))),
            Duration::from_secs(7)
        );
        assert_eq!(
            b.delay(0, Some(Duration::from_secs(600))),
            Duration::from_secs(60)
        );
        assert_eq!(b.delay(u32::MAX, None), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn recording_sleeper_records_without_waiting() {
        let s = RecordingSleeper::default();
        s.sleep(Duration::from_secs(3600)).await;
        s.sleep(Duration::from_millis(5)).await;
        assert_eq!(
            s.delays(),
            vec![Duration::from_secs(3600), Duration::from_millis(5)]
        );
    }
}
