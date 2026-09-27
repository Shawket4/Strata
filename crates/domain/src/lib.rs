//! Shared Strata domain vocabulary (PLAN §5.1): the enums and constants that the backend, the
//! client core and the vault format all agree on. These are **not** API DTOs; DTOs live in the
//! generated API crate and reuse these types.
//!
//! Every enum is a closed set of strings exactly as the plan spells them in frontmatter, the
//! database and on the wire. Each one offers `ALL`, `NAMES`, `as_str()`, `Display`, `FromStr`
//! (exact, case-sensitive) and string-based serde, generated from one declaration so the
//! spellings cannot drift apart.

mod macros;

mod account;
mod dedupe;
mod document;
mod error;
mod graph;
mod note;
mod relation;
mod task;
mod thresholds;

pub use account::{AccountStatus, Role};
pub use dedupe::{DedupeKind, MatchLevel};
pub use document::{CopyKind, CustodyEventType, DocType, DocumentRelationType, DocumentStatus};
pub use error::ParseError;
pub use graph::{CustodyEdge, GraphEdgeKind, GraphNodeKind};
pub use note::{Lang, NoteKind};
pub use relation::{
    EntityRelationType, MentionType, NOTE_RELATION_KEYS, RelationOrigin, RelationType,
};
pub use task::{Priority, TaskStatus};
pub use thresholds::{
    CUSTODY_CONFIDENCE_THRESHOLD, DedupeThresholds, RELATION_CONFIDENCE_THRESHOLD,
    SemanticThresholds,
};
