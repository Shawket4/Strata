//! Injectable time (never `Utc::now()` in logic, CLAUDE.md).

use std::fmt;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};

/// A source of the current instant.
pub trait Clock: Send + Sync + fmt::Debug {
    /// The current instant.
    fn now(&self) -> DateTime<Utc>;
}

/// The system clock (production).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock {}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A clock that only moves when told to (tests).
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl FakeClock {
    /// A clock stopped at `now`.
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    /// A clock stopped at an RFC 3339 instant.
    ///
    /// # Panics
    /// If `rfc3339` does not parse (a test fixture error).
    pub fn at(rfc3339: &str) -> Self {
        let now = DateTime::parse_from_rfc3339(rfc3339).map_or_else(
            |e| panic!("bad fixture instant {rfc3339}: {e}"),
            |d| d.with_timezone(&Utc),
        );
        Self::new(now)
    }

    /// Moves the clock to `now`.
    pub fn set(&self, now: DateTime<Utc>) {
        *lock(&self.now) = now;
    }

    /// Moves the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        let mut guard = lock(&self.now);
        *guard += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *lock(&self.now)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_moves_only_when_told() {
        let c = FakeClock::at("2026-09-27T10:00:00Z");
        assert_eq!(c.now().to_rfc3339(), "2026-09-27T10:00:00+00:00");
        c.advance(Duration::minutes(90));
        assert_eq!(c.now().to_rfc3339(), "2026-09-27T11:30:00+00:00");
        c.set(DateTime::UNIX_EPOCH);
        assert_eq!(c.now(), DateTime::UNIX_EPOCH);
    }
}
