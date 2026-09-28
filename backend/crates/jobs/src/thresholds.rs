//! Thresholds from the server configuration (PLAN §9.4, §6.12, §9.7): the confidence at or
//! above which AI relations and entity links (`thresholds.relation`) and custody events
//! (`thresholds.custody`) are applied automatically, and the per-kind semantic duplicate
//! thresholds (`thresholds.dedupe.<kind>.semantic`).

use std::collections::BTreeMap;

use dedupe::DedupeKind;
use domain::{DedupeThresholds, SemanticThresholds};
use strata_common::config::{DedupeThreshold, Thresholds};

/// Confidence thresholds of the AI pipelines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiThresholds {
    /// Relations, concepts, entity links and corrections are applied at or above this.
    pub relation: f64,
    /// Custody events are applied at or above this (stricter, D30).
    pub custody: f64,
}

impl Default for AiThresholds {
    /// The PLAN defaults: 0.7 and 0.85.
    fn default() -> Self {
        Self {
            relation: 0.7,
            custody: 0.85,
        }
    }
}

impl From<&Thresholds> for AiThresholds {
    fn from(t: &Thresholds) -> Self {
        Self {
            relation: t.relation,
            custody: t.custody,
        }
    }
}

/// Duplicate thresholds with the configured **semantic** level per kind: a configured
/// `semantic` value is the cosine at or above which an item is a semantic candidate
/// (`SemanticThresholds::candidate`); the level at which no LLM confirmation is needed
/// (`confirmed`) stays the tested default, raised to the candidate level when the
/// configuration asks for more. Near (trigram) thresholds keep the defaults tuned in `domain`
/// (the vault applies its own `near` overrides on create). Unknown kinds and kinds without a
/// semantic level (aliases) are ignored.
pub fn dedupe_thresholds(config: &BTreeMap<String, DedupeThreshold>) -> dedupe::Thresholds {
    let mut t = dedupe::Thresholds::new();
    for (kind, c) in config {
        let Ok(k) = kind.parse::<DedupeKind>() else {
            continue;
        };
        let d = DedupeThresholds::default_for(k);
        let Some(sem) = d.semantic else { continue };
        #[allow(clippy::cast_possible_truncation)] // thresholds are in 0..=1
        let candidate = c.semantic.clamp(0.0, 1.0) as f32;
        t = t.with(
            k,
            DedupeThresholds {
                near: d.near,
                semantic: Some(SemanticThresholds {
                    candidate,
                    confirmed: sem.confirmed.max(candidate),
                }),
            },
        );
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_semantic_levels_replace_the_candidate_threshold() {
        let config: BTreeMap<String, DedupeThreshold> = [
            ("note".to_owned(), DedupeThreshold { near: 0.1, semantic: 0.8 }),
            ("task".to_owned(), DedupeThreshold { near: 0.6, semantic: 0.99 }),
            ("alias".to_owned(), DedupeThreshold { near: 0.8, semantic: 0.5 }),
            ("bogus".to_owned(), DedupeThreshold { near: 0.8, semantic: 0.5 }),
        ]
        .into();
        let t = dedupe_thresholds(&config);
        assert_eq!(
            t.get(DedupeKind::Note),
            DedupeThresholds {
                near: 0.6,
                semantic: Some(SemanticThresholds {
                    candidate: 0.8,
                    confirmed: 0.96
                })
            }
        );
        assert_eq!(
            t.get(DedupeKind::Task),
            DedupeThresholds {
                near: 0.6,
                semantic: Some(SemanticThresholds {
                    candidate: 0.99,
                    confirmed: 0.99
                })
            }
        );
        assert_eq!(t.get(DedupeKind::Alias), DedupeThresholds::default_for(DedupeKind::Alias));
        assert_eq!(
            t.get(DedupeKind::Person),
            DedupeThresholds::default_for(DedupeKind::Person)
        );
        assert_eq!(AiThresholds::default(), AiThresholds { relation: 0.7, custody: 0.85 });
    }
}
