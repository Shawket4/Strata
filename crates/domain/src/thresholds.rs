//! Default thresholds (settings defaults; users may override them in settings, §11 screen 13).

use crate::DedupeKind;

/// Default minimum AI confidence for applying a note relation automatically (§9.4).
pub const RELATION_CONFIDENCE_THRESHOLD: f32 = 0.7;

/// Default minimum AI confidence for applying a custody event automatically (§6.12, D30);
/// stricter than relations.
pub const CUSTODY_CONFIDENCE_THRESHOLD: f32 = 0.85;

/// Embedding-cosine thresholds for semantic duplicates (§9.7 "Semantic").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticThresholds {
    /// At or above this cosine the item is a candidate; below [`Self::confirmed`] it is
    /// borderline and confirmed by one LLM call.
    pub candidate: f32,
    /// At or above this cosine the item is a duplicate candidate without LLM confirmation.
    pub confirmed: f32,
}

/// Per-kind duplicate thresholds (§9.7: "Thresholds are per-kind settings with tested
/// defaults").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DedupeThresholds {
    /// Minimum `pg_trgm` `similarity()` of normalised text for a near duplicate.
    pub near: f32,
    /// Semantic thresholds; `None` when semantic matching does not apply (aliases are single
    /// names, where embeddings add nothing over trigrams and transliteration keys).
    pub semantic: Option<SemanticThresholds>,
}

impl DedupeThresholds {
    /// Default thresholds for `kind`.
    ///
    /// Near thresholds were chosen from `pg_trgm` scores on normalised text (see the
    /// `text-normalize` parity table): reworded same-item titles score about 0.7–0.85
    /// (`Watanya's ETA invoice` / `ETA invoice for Watanya` = 0.83, `Churn notes` /
    /// `Churn note` = 0.77), while distinct items with shared words score about 0.2–0.35
    /// (`Petrol Arrows invoice` / `Watanya's ETA invoice` = 0.24, `Ahmed Samir` /
    /// `Ahmed Fathy` = 0.33).
    ///
    /// - Text items (note, capture, task): 0.6.
    /// - Names (person, company, concept): 0.5 — short strings score lower for one-letter
    ///   spelling variants (`Shady` / `Shadi` = 0.5, `Loyalty` / `Loyality` = 0.55).
    ///   Cross-script spellings are caught by alias transliteration keys, not trigrams.
    /// - Alias: 0.6.
    /// - Document, place: 0.8 — nested places and copies legitimately share most of their
    ///   name (`Nasr City office` / `Safe — Nasr City office` = 0.77 must not match).
    ///
    /// Semantic (cosine of the 384-dim granite embeddings): candidate 0.90 / confirmed 0.96
    /// for notes and captures, 0.88 / 0.95 for tasks and concepts (short texts embed less
    /// distinctly), 0.90 / 0.97 for people and companies, 0.92 / 0.97 for documents and
    /// places (a wrong merge of an entity is costly, so auto-confirmation is stricter).
    pub const fn default_for(kind: DedupeKind) -> Self {
        const fn t(near: f32, candidate: f32, confirmed: f32) -> DedupeThresholds {
            DedupeThresholds {
                near,
                semantic: Some(SemanticThresholds {
                    candidate,
                    confirmed,
                }),
            }
        }
        match kind {
            DedupeKind::Note | DedupeKind::Capture => t(0.6, 0.90, 0.96),
            DedupeKind::Task => t(0.6, 0.88, 0.95),
            DedupeKind::Concept => t(0.5, 0.88, 0.95),
            DedupeKind::Person | DedupeKind::Company => t(0.5, 0.90, 0.97),
            DedupeKind::Document | DedupeKind::Place => t(0.8, 0.92, 0.97),
            DedupeKind::Alias => Self {
                near: 0.6,
                semantic: None,
            },
        }
    }
}
