//! ULID identifiers: typed newtypes (stored as Postgres `uuid`) and injectable generators.
//!
//! A ULID and a UUID are both 128 bits; the conversion is a byte-for-byte copy, so ordering by
//! the `uuid` column equals ordering by ULID (time first).

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration as StdDuration, SystemTime};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;
use uuid::Uuid;

use crate::clock::Clock;

/// Error returned when a string is not a valid ULID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid ULID `{input}`")]
pub struct IdParseError {
    /// The rejected input (IDs are not secret).
    pub input: String,
}

/// Parses a canonical 26-character Crockford base32 ULID.
pub fn parse_ulid(s: &str) -> Result<Ulid, IdParseError> {
    Ulid::from_string(s).map_err(|_| IdParseError {
        input: s.to_owned(),
    })
}

macro_rules! ulid_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Ulid);

        impl $name {
            /// Wraps an existing ULID.
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// Converts from the `uuid` representation used in Postgres.
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(Ulid::from(uuid))
            }

            /// Draws a fresh ID from `ids`.
            pub fn generate(ids: &dyn IdGenerator) -> Self {
                Self(ids.next_ulid())
            }

            /// The underlying ULID.
            pub const fn as_ulid(&self) -> Ulid {
                self.0
            }

            /// The `uuid` representation stored in Postgres.
            pub fn as_uuid(&self) -> Uuid {
                Uuid::from(self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                parse_ulid(s).map(Self)
            }
        }

        impl From<Ulid> for $name {
            fn from(value: Ulid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Ulid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::from_uuid(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.as_uuid()
            }
        }

        /// Serialised as the 26-character ULID string (PLAN §7.7: IDs are ULID strings).
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
                parse_ulid(&s).map(Self).map_err(serde::de::Error::custom)
            }
        }

        #[cfg(feature = "sqlx")]
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <Uuid as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        #[cfg(feature = "sqlx")]
        impl sqlx::postgres::PgHasArrayType for $name {
            fn array_type_info() -> sqlx::postgres::PgTypeInfo {
                <Uuid as sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }

        #[cfg(feature = "sqlx")]
        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <Uuid as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_uuid(), buf)
            }
        }

        #[cfg(feature = "sqlx")]
        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                <Uuid as sqlx::Decode<'r, sqlx::Postgres>>::decode(value).map(Self::from_uuid)
            }
        }
    };
}

ulid_newtype!(
    /// An account (`users.id`).
    UserId
);
ulid_newtype!(
    /// A signed-in app installation (`devices.id`).
    DeviceId
);
ulid_newtype!(
    /// A device session (`sessions.id`).
    SessionId
);
ulid_newtype!(
    /// A vault note (`notes.id`, the frontmatter `id`). Entities, places and documents are notes.
    NoteId
);
ulid_newtype!(
    /// A queued job (`jobs.id`).
    JobId
);
ulid_newtype!(
    /// An AI or system suggestion (`suggestions.id`).
    SuggestionId
);
ulid_newtype!(
    /// A reply in a suggestion thread (`suggestion_replies.id`, PLAN §9.8).
    ReplyId
);
ulid_newtype!(
    /// A recorded AI decision (`ai_decisions.id`, PLAN §9.8).
    DecisionId
);
ulid_newtype!(
    /// An account invite (`invites.id`).
    InviteId
);
ulid_newtype!(
    /// An audit-log entry (`audit_log.id`).
    AuditId
);
ulid_newtype!(
    /// A retrieval chunk (`chunks.id`).
    ChunkId
);
ulid_newtype!(
    /// A document custody event (`custody_events.id`).
    CustodyEventId
);
ulid_newtype!(
    /// A client-generated sync operation ID, the idempotency key (`idempotency.op_id`).
    OpId
);
ulid_newtype!(
    /// A disambiguation hint (`disambiguation_hints.id`, PLAN §9.8).
    HintId
);
ulid_newtype!(
    /// A reminder delivery record (`notification_log.id`).
    NotificationId
);

/// A source of fresh ULIDs. Production uses [`SystemIdGenerator`]; tests use
/// [`SequentialIdGenerator`] so IDs are predictable.
pub trait IdGenerator: Send + Sync + fmt::Debug {
    /// Returns a new ULID, strictly greater than any previously returned by this generator.
    fn next_ulid(&self) -> Ulid;
}

/// Monotonic ULIDs whose timestamp comes from an injected [`Clock`] and whose random part comes
/// from the OS RNG.
pub struct SystemIdGenerator {
    clock: Arc<dyn Clock>,
    inner: Mutex<ulid::Generator>,
}

impl fmt::Debug for SystemIdGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemIdGenerator")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

impl SystemIdGenerator {
    /// A generator reading time from `clock`.
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            inner: Mutex::new(ulid::Generator::new()),
        }
    }
}

impl IdGenerator for SystemIdGenerator {
    fn next_ulid(&self) -> Ulid {
        let millis = u64::try_from(self.clock.now().timestamp_millis()).unwrap_or(0);
        let at = SystemTime::UNIX_EPOCH + StdDuration::from_millis(millis);
        let mut generator = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        // Overflow of the 80-bit random part within one millisecond is practically impossible;
        // fall back to a fresh random ULID rather than failing.
        generator
            .generate_from_datetime(at)
            .unwrap_or_else(|_| Ulid::from_datetime(at))
    }
}

/// Deterministic ULIDs for tests: timestamp fixed at `timestamp_ms`, random part counting up
/// from 1. The n-th call (1-based) returns `Ulid::from_parts(timestamp_ms, n)`.
#[derive(Debug)]
pub struct SequentialIdGenerator {
    timestamp_ms: u64,
    counter: AtomicU64,
}

impl SequentialIdGenerator {
    /// A generator with the given fixed timestamp.
    pub const fn new(timestamp_ms: u64) -> Self {
        Self {
            timestamp_ms,
            counter: AtomicU64::new(0),
        }
    }

    /// The ULID the `n`-th call (1-based) returns; handy for expected values in assertions.
    pub const fn nth(&self, n: u64) -> Ulid {
        Ulid::from_parts(self.timestamp_ms, n as u128)
    }
}

impl Default for SequentialIdGenerator {
    /// Timestamp = the test epoch `2026-09-27T12:00:00Z`.
    fn default() -> Self {
        Self::new(1_790_510_400_000)
    }
}

impl IdGenerator for SequentialIdGenerator {
    fn next_ulid(&self) -> Ulid {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        Ulid::from_parts(self.timestamp_ms, u128::from(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FakeClock;

    #[test]
    fn sequential_generator_is_deterministic() {
        let ids = SequentialIdGenerator::default();
        let a = UserId::generate(&ids);
        let b = NoteId::generate(&ids);
        assert_eq!(a.to_string(), "01M3HBS0G00000000000000001");
        assert_eq!(b.as_ulid(), ids.nth(2));
        assert_eq!(b.to_string(), "01M3HBS0G00000000000000002");
        assert!(a.as_ulid() < b.as_ulid());
    }

    #[test]
    fn ulid_uuid_round_trip_preserves_bytes_and_order() {
        let ids = SequentialIdGenerator::new(1);
        let first = UserId::generate(&ids);
        let second = UserId::generate(&ids);
        assert_eq!(first.as_uuid().to_string(), "00000000-0001-0000-0000-000000000001");
        assert_eq!(UserId::from_uuid(first.as_uuid()), first);
        assert!(first.as_uuid() < second.as_uuid());
    }

    #[test]
    fn parse_and_display_round_trip() {
        let id: DeviceId = "01M3HBS0G00000000000000002".parse().expect("valid ULID literal");
        assert_eq!(id.to_string(), "01M3HBS0G00000000000000002");
        assert_eq!(format!("{id:?}"), "DeviceId(01M3HBS0G00000000000000002)");
        assert_eq!(
            "not-a-ulid".parse::<DeviceId>(),
            Err(IdParseError {
                input: "not-a-ulid".into()
            })
        );
    }

    #[test]
    fn serde_uses_ulid_strings() {
        let id = SessionId::from_ulid(Ulid::from_parts(1_790_510_400_000, 7));
        let json = serde_json::to_string(&id).expect("serialise");
        assert_eq!(json, "\"01M3HBS0G00000000000000007\"");
        let back: SessionId = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, id);
        let err = serde_json::from_str::<SessionId>("\"nope\"").expect_err("must reject");
        assert_eq!(err.to_string(), "invalid ULID `nope`");
    }

    #[test]
    fn system_generator_uses_clock_time_and_is_monotonic() {
        let clock = FakeClock::at_default_epoch();
        let ids = SystemIdGenerator::new(Arc::new(clock));
        let a = ids.next_ulid();
        let b = ids.next_ulid();
        assert_eq!(a.timestamp_ms(), 1_790_510_400_000);
        assert_eq!(b.timestamp_ms(), 1_790_510_400_000);
        assert!(b > a);
    }
}
