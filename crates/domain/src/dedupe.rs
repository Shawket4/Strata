//! Duplicate detection vocabulary (§9.7, §7.4 `dedupe_keys`).

use crate::macros::string_enum;

string_enum! {
    /// Kind of item a duplicate check runs on (`dedupe_keys.kind`). Every create endpoint
    /// runs the check (§7.5), so every creatable kind is listed.
    pub enum DedupeKind("dedupe kind") {
        /// A note created with `POST /notes`.
        Note => "note",
        /// An inbox capture (never blocked, only flagged).
        Capture => "capture",
        /// A task line.
        Task => "task",
        /// A person entity.
        Person => "person",
        /// A company entity.
        Company => "company",
        /// A concept note.
        Concept => "concept",
        /// An entity alias.
        Alias => "alias",
        /// A document entity.
        Document => "document",
        /// A place entity.
        Place => "place",
    }
}

string_enum! {
    /// How a duplicate candidate matched. Ordered by strength of evidence
    /// (`Exact < Near < Semantic` in declaration order).
    pub enum MatchLevel("match level") {
        /// Normalised key equal.
        Exact => "exact",
        /// Trigram similarity above the per-kind threshold.
        Near => "near",
        /// Embedding cosine above the per-kind threshold.
        Semantic => "semantic",
    }
}
