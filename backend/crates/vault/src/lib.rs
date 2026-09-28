//! Strata vault store (PLAN §7.1 `vault/`, §7.2, §7.3).
//!
//! Each user's vault is a directory of Obsidian-compatible markdown (plus `.meta/` sidecars
//! and `.trash/`) that is also a git repository. [`VaultService`] owns all of them:
//!
//! - one **single-writer actor** per user serialises writes ([`store`]); reads are
//!   concurrent;
//! - every write is **atomic** on disk ([`fsio`]) and **one git commit** ([`git`]) with a
//!   `user:`/`ai:`/`system:` message;
//! - optimistic concurrency by **content-hash versions**;
//! - moves/renames **rewrite every inbound link and relation** in the same commit;
//! - after each write the **index is updated synchronously** ([`derive`], [`indexer`]) in the
//!   user's scoped transaction, the change log is appended and AI jobs are enqueued;
//! - **reconciliation** ([`reconcile`]) repairs crashes and out-of-band edits and can rebuild
//!   a user's derived rows from the vault (with the cluster rows of `.meta/clusters.json`,
//!   [`clusters`]);
//! - a write is **all-or-nothing** across crashes ([`journal`]), and a pushed op's result is
//!   stored in the write's own transaction and commit ([`receipt`]);
//! - duplicate checks on create ([`dup`]), export/import ([`archive`]).
//!
//! The design is written up in `docs/ARCHITECTURE.md`, section "Vault store".

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(test, allow(clippy::expect_used, clippy::float_cmp))]
// Vault notes are `.md` exactly as Obsidian writes them; link resolution is case-insensitive
// separately (vault-format `PathIndex`).
#![allow(clippy::case_sensitive_file_extension_comparisons)]

pub mod archive;
pub mod clusters;
pub mod derive;
pub mod diff;
pub mod dup;
pub mod error;
pub mod events;
pub mod fsio;
pub mod git;
pub mod indexer;
pub mod journal;
pub mod model;
pub mod ops;
pub mod paths;
pub mod prepare;
pub mod receipt;
pub mod reconcile;
pub mod revert;
pub mod semantic;
pub mod state;
pub mod store;

pub use error::{Candidate, MatchLevel, Result, VaultError};
pub use events::{CommitListener, Committed};
pub use receipt::{AfterWrite, OpReceipt, ResultHook};
pub use store::{Author, Core, CrashPoint, ImportLimits, VaultConfig, VaultService};
