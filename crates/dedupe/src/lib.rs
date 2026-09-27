//! Exact and near duplicate detection shared by the backend and the client core (PLAN §9.7,
//! L16).
//!
//! Both sides generate *candidates* cheaply from their own store — the backend with SQL
//! (`dedupe_keys` equality and `pg_trgm` `similarity()`), the client core from its local
//! `SQLite` cache — and then call [`check`] on the new item and those candidates for the final
//! decision. Because the decision, the keys and the scores all come from this crate, an
//! offline check on the device and the server's re-check on push always agree.
//!
//! # Flow
//!
//! 1. Describe the new item as an [`Item`] (constructors per kind: [`Item::note`],
//!    [`Item::task`], [`Item::entity`], …).
//! 2. [`CandidateQuery::for_item`] returns the keys to look up: exact keys (equality), trigram
//!    texts with a similarity floor, phonetic keys (entities only) and the compatible stored
//!    kinds. Stored items keep their [`DedupeKeys`] (from [`Item::keys`]) in `dedupe_keys`.
//! 3. [`check`] scores every candidate, drops keep-both pairs ([`KeepBothSet`]) and returns
//!    the ranked [`DuplicateCandidate`]s — the payload of the API's `duplicate_candidates`
//!    problem and of the client's offline prompt — plus the borderline semantic candidates
//!    that still need one LLM confirmation.
//!
//! # Levels (§9.7)
//!
//! | Level | Rule |
//! |---|---|
//! | [`MatchLevel::Exact`] | equal exact key ([`keys`] module: `text-normalize` dedupe key, light English plural folding, task recurrence + linked entities) |
//! | [`MatchLevel::Near`] | `pg_trgm` similarity of the prepared texts ≥ the per-kind near threshold (the stricter of the two kinds), or — for people, companies and aliases — an equal transliteration key confirmed by spelling similarity |
//! | [`MatchLevel::Semantic`] | embedding cosine supplied by the caller ≥ the confirmed threshold, or ≥ the candidate threshold and confirmed by an LLM call |
//!
//! Everything here is pure and deterministic: no I/O, no clock, no randomness.

#![cfg_attr(test, allow(clippy::float_cmp))] // tests assert exact scores

mod check;
mod item;
pub mod keys;
mod score;

pub use check::{
    CheckOutcome, DuplicateCandidate, DuplicatePair, Existing, KeepBoth, KeepBothSet,
    SemanticEvidence, Thresholds, check, compatible_kinds, sweep,
};
pub use domain::{DedupeKind, DedupeThresholds, MatchLevel};
pub use item::Item;
pub use keys::{CandidateQuery, DedupeKeys};
pub use score::{
    PHONETIC_CROSS_SCRIPT_SCORE, PHONETIC_MIN_KEY_LEN, PHONETIC_MIN_SPELLING_SIMILARITY, PairScore,
    edit_similarity, score_pair,
};
