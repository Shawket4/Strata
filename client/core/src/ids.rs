//! Injectable ULID generation (client-generated IDs survive offline creation, §7.5).

use std::fmt;
use std::sync::Mutex;

use ulid::Ulid;

use crate::clock::Clock;

/// A source of new ULIDs.
pub trait IdGenerator: Send + Sync + fmt::Debug {
    /// A new ULID.
    fn ulid(&self) -> Ulid;
}

/// Monotonic ULIDs from a clock and the OS random source (production).
pub struct UlidGenerator {
    inner: Mutex<ulid::Generator>,
    clock: std::sync::Arc<dyn Clock>,
}

impl fmt::Debug for UlidGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UlidGenerator").finish_non_exhaustive()
    }
}

impl UlidGenerator {
    /// A generator stamping IDs with `clock`'s time.
    pub fn new(clock: std::sync::Arc<dyn Clock>) -> Self {
        Self {
            inner: Mutex::new(ulid::Generator::new()),
            clock,
        }
    }
}

impl IdGenerator for UlidGenerator {
    fn ulid(&self) -> Ulid {
        let now: std::time::SystemTime = self.clock.now().into();
        let mut g = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Overflow within one millisecond is astronomically unlikely; fall back to a fresh ULID.
        g.generate_from_datetime(now)
            .unwrap_or_else(|_| Ulid::from_datetime(now))
    }
}

/// Deterministic, strictly increasing ULIDs: timestamp part fixed, random part a counter
/// (tests).
#[derive(Debug)]
pub struct SeqIds {
    base_ms: u64,
    next: Mutex<u128>,
}

impl SeqIds {
    /// IDs `base_ms`/1, `base_ms`/2, ….
    pub fn new(base_ms: u64) -> Self {
        Self {
            base_ms,
            next: Mutex::new(1),
        }
    }
}

impl IdGenerator for SeqIds {
    fn ulid(&self) -> Ulid {
        let mut n = self
            .next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = Ulid::from_parts(self.base_ms, *n);
        *n += 1;
        id
    }
}

/// A task block ID (`t-<ulid>` lower-case, §6.11) from a ULID (the shared rule).
pub use item_render::task::task_block_id;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_ids_are_deterministic_and_ordered() {
        let g = SeqIds::new(1_790_000_000_000);
        let a = g.ulid();
        let b = g.ulid();
        assert_eq!(a.to_string(), "01M3250V000000000000000001");
        assert_eq!(b.to_string(), "01M3250V000000000000000002");
        assert!(a < b);
        assert_eq!(task_block_id(a), "t-01m3250v000000000000000001");
    }

    #[test]
    fn ulid_generator_uses_the_clock() {
        let clock = std::sync::Arc::new(crate::clock::FakeClock::at("2026-09-27T10:00:00Z"));
        let g = UlidGenerator::new(clock);
        let a = g.ulid();
        let b = g.ulid();
        assert_eq!(a.timestamp_ms(), 1_790_503_200_000);
        assert!(a < b);
    }
}
