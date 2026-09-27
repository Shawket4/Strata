//! # vault-format
//!
//! The Strata vault format: Obsidian-compatible markdown notes with YAML frontmatter,
//! wikilinks, tags, block IDs, AI-owned sections, Obsidian Tasks lines, custody events,
//! sidecar metadata (`.meta/`) and JSON Canvas layouts.
//!
//! This crate is shared by the backend and the client core so the two never disagree on the
//! vault format (PLAN L16). It is pure: no I/O, no async, no clock, no randomness. Callers pass
//! in everything that varies (dates, IDs, vault paths, time zones).
//!
//! The authoritative description of the rules implemented here is `docs/VAULT_FORMAT.md`.
//!
//! ## Byte offsets
//!
//! Every span returned by this crate is a byte range into the string that was passed in
//! (UTF-8, so ranges always fall on `char` boundaries). Body spans are relative to the body;
//! add [`Document::body_offset`] to get file offsets.
//!
//! ## Module map
//!
//! - [`document`]: split a file into frontmatter and body; render back byte-exactly.
//! - [`frontmatter`]: order- and byte-preserving YAML properties with typed access to the known keys.
//! - [`wikilink`], [`body`]: links, embeds, tags, headings, blocks and block IDs in a body.
//! - [`resolve`], [`rewrite`]: Obsidian link resolution and rename/move rewriting.
//! - [`filename`]: forbidden characters, sanitisation, titles.
//! - [`blocks`]: appending block IDs (the only permitted automated body edit).
//! - [`sections`]: AI-owned `## ` sections and their validation.
//! - [`custody`]: document custody events.
//! - [`tasks`]: Obsidian Tasks lines, the recurrence grammar, RRULEs and completion.
//! - [`sidecar`], [`clusters`], [`canvas`]: JSON files on disk.

/// The shared vocabulary (PLAN L16) this crate's frontmatter, sidecar and task types use.
pub use domain;

pub mod blocks;
pub mod body;
pub mod canvas;
pub mod clusters;
pub mod custody;
pub mod document;
pub mod filename;
pub mod frontmatter;
pub mod line;
pub mod resolve;
pub mod rewrite;
pub mod sections;
pub mod sidecar;
pub mod tasks;
pub mod wikilink;

pub use body::{BodyAnalysis, analyze};
pub use document::Document;
pub use frontmatter::{Frontmatter, FrontmatterError, KnownKey, Open, PropertyValue, RelationKey};
pub use line::LineEnding;
pub use resolve::{PathIndex, Resolution};
pub use wikilink::{Anchor, WikiLink};
