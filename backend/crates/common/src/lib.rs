//! Shared backend foundations: configuration, injectable clock and ID generation, typed IDs,
//! error types, and RFC 7807 problem details (PLAN §7.1 `common/`).

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::float_cmp,
        clippy::field_reassign_with_default
    )
)]

pub mod clock;
pub mod config;
pub mod error;
pub mod id;
pub mod problem;

pub use clock::{Clock, FakeClock, SystemClock};
pub use config::Config;
pub use error::ConfigError;
pub use id::{
    AuditId, ChunkId, CustodyEventId, DecisionId, DeviceId, HintId, IdGenerator, IdParseError,
    InviteId, JobId, NoteId, NotificationId, OpId, ReplyId, SequentialIdGenerator, SessionId,
    SuggestionId, SystemIdGenerator, UserId,
};
pub use problem::{DomainError, ProblemType};
