//! In-process sliding-window rate limiter (PLAN §8, §15: login per IP and per username;
//! signup per IP and globally; capture and ask per user).
//!
//! A limiter allows at most `max` events per key in any window of `window` length. It keeps
//! the timestamps of the admitted events per key, so the check is exact rather than
//! bucketed, and it reads time from the injected [`Clock`] (fake time in tests). State lives
//! in this process only; see `docs/ARCHITECTURE.md` "Auth" for the single-process design.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};
use strata_common::Clock;
use strata_common::config::RateLimit;

/// Keys are pruned when the map grows past this many entries.
const PRUNE_THRESHOLD: usize = 10_000;

/// A rejected event: retry after this many seconds (at least 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limited {
    /// Seconds until the oldest counted event leaves the window.
    pub retry_after_secs: u64,
}

/// One sliding-window limit over string keys.
#[derive(Debug)]
pub struct RateLimiter {
    max: usize,
    window: Duration,
    clock: Arc<dyn Clock>,
    events: Mutex<HashMap<String, VecDeque<DateTime<Utc>>>>,
}

impl RateLimiter {
    /// A limiter for `limit`.
    pub fn new(limit: RateLimit, clock: Arc<dyn Clock>) -> Self {
        Self {
            max: usize::try_from(limit.max).unwrap_or(usize::MAX),
            window: Duration::seconds(i64::from(limit.window_secs)),
            clock,
            events: Mutex::new(HashMap::new()),
        }
    }

    /// Counts one event for `key` if it fits in the window; otherwise reports when to retry
    /// (a rejected event is not counted).
    pub fn check(&self, key: &str) -> Result<(), Limited> {
        let now = self.clock.now();
        let mut events = self.events.lock().unwrap_or_else(PoisonError::into_inner);
        if events.len() > PRUNE_THRESHOLD {
            let horizon = now - self.window;
            events.retain(|_, q| q.back().is_some_and(|t| *t > horizon));
        }
        let queue = events.entry(key.to_owned()).or_default();
        while queue.front().is_some_and(|t| *t <= now - self.window) {
            queue.pop_front();
        }
        if queue.len() < self.max {
            queue.push_back(now);
            return Ok(());
        }
        let oldest = queue.front().copied().unwrap_or(now);
        let wait = (oldest + self.window - now).num_seconds().max(1);
        Err(Limited {
            retry_after_secs: u64::try_from(wait).unwrap_or(1),
        })
    }
}

/// The limiters of the account endpoints and the per-user capture/ask limits.
#[derive(Debug)]
pub struct AuthLimiters {
    /// Login attempts per client IP.
    pub login_ip: RateLimiter,
    /// Login attempts per username key.
    pub login_user: RateLimiter,
    /// Sign-ups per client IP.
    pub signup_ip: RateLimiter,
    /// Sign-ups overall.
    pub signup_global: RateLimiter,
    /// `POST /capture` per user.
    pub capture_user: RateLimiter,
    /// `POST /ask` per user.
    pub ask_user: RateLimiter,
}

impl AuthLimiters {
    /// Limiters for the configured limits.
    pub fn new(limits: &strata_common::config::RateLimits, clock: &Arc<dyn Clock>) -> Self {
        Self {
            login_ip: RateLimiter::new(limits.login_per_ip, clock.clone()),
            login_user: RateLimiter::new(limits.login_per_username, clock.clone()),
            signup_ip: RateLimiter::new(limits.signup_per_ip, clock.clone()),
            signup_global: RateLimiter::new(limits.signup_global, clock.clone()),
            capture_user: RateLimiter::new(limits.capture_per_user, clock.clone()),
            ask_user: RateLimiter::new(limits.ask_per_user, clock.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_common::FakeClock;

    fn limiter(max: u32, window_secs: u32, clock: &FakeClock) -> RateLimiter {
        RateLimiter::new(RateLimit { max, window_secs }, Arc::new(clock.clone()))
    }

    #[test]
    fn allows_max_events_per_window_then_reports_retry_after() {
        let clock = FakeClock::at_default_epoch();
        let l = limiter(3, 60, &clock);
        for _ in 0..3 {
            assert_eq!(l.check("k"), Ok(()));
            clock.advance(Duration::seconds(10));
        }
        // Events at t=0, 10, 20; now t=30: the first leaves the window at t=60.
        assert_eq!(
            l.check("k"),
            Err(Limited {
                retry_after_secs: 30
            })
        );
        assert_eq!(l.check("other"), Ok(()));
        clock.advance(Duration::seconds(30));
        assert_eq!(l.check("k"), Ok(()));
        assert_eq!(
            l.check("k"),
            Err(Limited {
                retry_after_secs: 10
            })
        );
    }

    #[test]
    fn rejected_events_are_not_counted() {
        let clock = FakeClock::at_default_epoch();
        let l = limiter(1, 60, &clock);
        assert_eq!(l.check("k"), Ok(()));
        for _ in 0..5 {
            assert!(l.check("k").is_err());
        }
        clock.advance(Duration::seconds(60));
        assert_eq!(l.check("k"), Ok(()));
    }

    #[test]
    fn retry_after_is_at_least_one_second() {
        let clock = FakeClock::at_default_epoch();
        let l = limiter(1, 1, &clock);
        assert_eq!(l.check("k"), Ok(()));
        assert_eq!(
            l.check("k"),
            Err(Limited {
                retry_after_secs: 1
            })
        );
    }

    #[test]
    fn stale_keys_are_pruned() {
        let clock = FakeClock::at_default_epoch();
        let l = limiter(1, 1, &clock);
        for i in 0..=PRUNE_THRESHOLD {
            assert_eq!(l.check(&i.to_string()), Ok(()));
        }
        clock.advance(Duration::seconds(5));
        assert_eq!(l.check("fresh"), Ok(()));
        assert_eq!(l.events.lock().expect("lock").len(), 1);
    }
}
