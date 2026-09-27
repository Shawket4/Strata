//! The sync model shared by the backend and the Rust client core (PLAN L16, §7.5 Sync,
//! §12.3–12.5, D19).
//!
//! - [`ops`]: every pushed operation ([`SyncOp`], [`Op`]) with its payload, idempotency key,
//!   entity ID, base version and `force` flag.
//! - [`results`]: per-op results (`applied`, `conflict`, `duplicate`, `rejected`) and the
//!   push request/response.
//! - [`changes`]: change records (upserts and tombstones), bootstrap and changes pages, and
//!   the client's [`SyncCursor`].
//! - [`merge()`]: the 3-way merge of note documents (key-by-key frontmatter, diff3 body, atomic
//!   task lines) and the D19 update decision.
//! - [`apply`]: pure op application rules (relations, patches, custody, task edits).
//! - [`settings`]: how stored user settings become setting records, and which records a
//!   change touches.
//! - [`Version`]: content-hash versions.
//!
//! These are internal models: the API layer maps them to its `OpenAPI` DTOs. They serialise
//! as named-map `MessagePack` (`rmp_serde::to_vec_named`) and only change additively.
//! Everything is pure: no I/O, no async, no clock.

pub mod apply;
pub mod changes;
pub mod merge;
pub mod ops;
pub mod results;
pub mod settings;
mod version;

pub use changes::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, CursorError, EntityType, Record, SyncCursor,
};
pub use merge::{
    AutoResolution, AutoResolved, Choice, ConflictHunk, ConflictKind, Conflicted, Location,
    MergeOutcome, ResolveError, UpdateDecision, decide_update, merge, merge_text_only,
};
pub use ops::{Op, OpError, OpKind, SyncOp};
pub use results::{ConflictResolution, OpOutcome, OpResult, Problem, PushRequest, PushResponse};
pub use version::{InvalidVersion, Version};
