//! Injectable time source (ENGINEERING.md: never call `Utc::now()` in logic).

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};

/// A source of the current time. Logic takes `&dyn Clock` (or `Arc<dyn Clock>`) instead of
/// calling `Utc::now()` so tests can control time exactly.
pub trait Clock: Send + Sync + fmt::Debug {
    /// The current instant in UTC.
    fn now(&self) -> DateTime<Utc>;
}

/// The real wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A manually driven clock for tests. Clones share the same instant, so a test can hand a clone
/// to the code under test and keep one to advance time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl FakeClock {
    /// A fake clock frozen at `start`.
    pub fn new(start: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    /// A fake clock frozen at `2026-09-27T12:00:00Z`, the conventional start for tests.
    pub fn at_default_epoch() -> Self {
        Self::new(default_test_epoch())
    }

    /// Moves the clock to `instant` (may go backwards; tests decide).
    pub fn set(&self, instant: DateTime<Utc>) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = instant;
    }

    /// Advances the clock by `by` and returns the new instant.
    pub fn advance(&self, by: Duration) -> DateTime<Utc> {
        let mut guard = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *guard += by;
        *guard
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `2026-09-27T12:00:00Z` — the fixed instant [`FakeClock::at_default_epoch`] starts at.
pub fn default_test_epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_510_400, 0).unwrap_or(DateTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_epoch_is_the_documented_instant() {
        assert_eq!(
            default_test_epoch().to_rfc3339(),
            "2026-09-27T12:00:00+00:00"
        );
    }

    #[test]
    fn fake_clock_is_frozen_until_advanced_and_clones_share_time() {
        let clock = FakeClock::at_default_epoch();
        let handle = clock.clone();
        assert_eq!(clock.now(), default_test_epoch());
        assert_eq!(clock.now(), default_test_epoch());
        let after = handle.advance(Duration::minutes(90));
        assert_eq!(after.to_rfc3339(), "2026-09-27T13:30:00+00:00");
        assert_eq!(clock.now(), after);
        clock.set(default_test_epoch());
        assert_eq!(handle.now(), default_test_epoch());
    }

    #[test]
    fn system_clock_is_after_the_test_epoch_origin() {
        assert!(SystemClock.now() > DateTime::UNIX_EPOCH);
    }
}
