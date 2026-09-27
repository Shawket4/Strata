//! Deterministic Arabic/Latin text normalisation shared by the backend and the client core
//! (PLAN L16, §7.4, §9.7).
//!
//! Every function here is pure: the same input always yields the same output, on every
//! platform, with no locale or environment dependence. The backend uses these functions when
//! building the full-text index, alias tables and duplicate keys, and the client core uses the
//! very same code for its offline search and duplicate checks, so both sides always agree.
//!
//! | Function | Used for |
//! |---|---|
//! | [`normalize_for_search`] | full-text index and query text, trigram input |
//! | [`dedupe_key`] | exact duplicate detection (§9.7 "Exact") |
//! | [`transliteration_key`] | coarse Arabic↔Latin alias *candidate* matching |
//! | [`trigrams`] / [`trigram_similarity`] | near-duplicate detection, identical to `pg_trgm` |
//!
//! See each module for the precise rules and the documented design decisions.

mod dedupe;
mod normalize;
mod translit;
mod trigram;

pub use dedupe::{ARABIC_STOPWORDS, ENGLISH_STOPWORDS, dedupe_key};
pub use normalize::normalize_for_search;
pub use translit::transliteration_key;
pub use trigram::{raw_similarity, raw_trigrams, trigram_similarity, trigrams};
