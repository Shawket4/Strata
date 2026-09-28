//! # item-render
//!
//! How new vault items are rendered: the path and the markdown of a new capture, entity,
//! document, place or concept note, the ID/timestamp stamp every new note gets, conflict-copy
//! names and the task line a suggestion proposes.
//!
//! The backend (when it writes) and the client core (when it applies an op optimistically,
//! PLAN §12.3) both call these functions, so for the same input they produce the same bytes
//! (PLAN L16). The note formats themselves live in `vault-format`; the op payloads in
//! `sync-model`; this crate only composes them. Pure: no I/O, no clock, no randomness —
//! callers pass the IDs, timestamps and the paths already taken.
//!
//! ## Module map
//!
//! - [`paths`]: inbox paths of captures, entity paths, free names next to taken ones,
//!   conflict-copy names, inbox membership.
//! - [`note`]: the `id`/`created`/`updated` stamp (UTC), new notes, `with_id`, list cleaning.
//! - [`capture`]: the markdown of a new capture (§6.9).
//! - [`entity`]: the skeletons of entity, document, place (§6.7, §6.12) and concept (§6.6)
//!   notes, and the op payload → skeleton conversions of `entity/document/place.create`.
//! - [`task`]: the task line a task suggestion proposes (§6.11), the month-heading date and
//!   the home-note stamp of a new task. Task *creation* is
//!   `sync_model::apply::apply_task_create`.

pub mod capture;
pub mod entity;
mod error;
pub mod note;
pub mod paths;
pub mod task;

pub use error::RenderError;
