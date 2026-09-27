//! The local 3-way merge preview of a conflict (D19).
//!
//! **Seam:** the 3-way merge is shared logic and belongs to `sync-model` (L16: never
//! implemented on one side only). Until `sync-model` exposes it, [`PendingMerge`] reports that
//! no preview is available and the conflict screen shows base / local / server side by side.

use std::fmt;

/// A merge preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePreview {
    /// The merged text (with conflict markers where hunks overlap).
    pub merged: String,
    /// No overlapping hunks.
    pub clean: bool,
}

/// Computes merge previews.
pub trait NoteMerger: Send + Sync + fmt::Debug {
    /// Merges `local` and `server`, both edited from `base`. `None`: no preview available.
    fn merge(&self, base: &str, local: &str, server: &str) -> Option<MergePreview>;
}

/// The merger used until `sync-model` provides one: no preview.
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingMerge;

impl NoteMerger for PendingMerge {
    fn merge(&self, _base: &str, _local: &str, _server: &str) -> Option<MergePreview> {
        None
    }
}
