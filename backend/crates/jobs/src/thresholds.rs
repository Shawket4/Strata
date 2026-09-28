//! Thresholds from the server configuration (PLAN §9.4, §6.12, §9.7): the confidence at or
//! above which AI relations and entity links (`thresholds.relation`) and custody events
//! (`thresholds.custody`) are applied automatically, and the per-kind duplicate thresholds
//! (`thresholds.dedupe.<kind>.near` and `.semantic`).

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

/// Duplicate thresholds from `thresholds.dedupe` (§9.7), per kind: the configured `near`
/// (trigram) level, and the configured `semantic` level as the cosine at or above which an
/// item is a semantic candidate (`SemanticThresholds::candidate`); the level at which no LLM
/// confirmation is needed (`confirmed`) stays the tested default, raised to the candidate
/// level when the configuration asks for more. A missing `semantic` keeps the default; kinds
/// without a semantic level (aliases) keep none; unknown kinds are ignored. The configuration
/// defaults are the `domain` defaults, so an unchanged configuration yields exactly those.
pub fn dedupe_thresholds(config: &BTreeMap<String, DedupeThreshold>) -> dedupe::Thresholds {
    let mut t = dedupe::Thresholds::new();
    for (kind, c) in config {
        let Ok(k) = kind.parse::<DedupeKind>() else {
            continue;
        };
        let d = DedupeThresholds::default_for(k);
        let semantic = d.semantic.map(|sem| match c.semantic {
            Some(v) => {
                let candidate = unit(v);
                SemanticThresholds {
                    candidate,
                    confirmed: sem.confirmed.max(candidate),
                }
            }
            None => sem,
        });
        t = t.with(
            k,
            DedupeThresholds {
                near: unit(c.near),
                semantic,
            },
        );
    }
    t
}

/// The configured near (trigram) level per kind, for the vault's create check
/// (`strata_vault::VaultConfig::near_thresholds`).
pub fn near_thresholds(config: &BTreeMap<String, DedupeThreshold>) -> BTreeMap<String, f32> {
    config
        .iter()
        .filter(|(kind, _)| kind.parse::<DedupeKind>().is_ok())
        .map(|(kind, c)| (kind.clone(), unit(c.near)))
        .collect()
}

#[allow(clippy::cast_possible_truncation)] // thresholds are in 0..=1
fn unit(v: f64) -> f32 {
    v.clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_configuration_yields_the_domain_defaults() {
        let config = strata_common::Config::default();
        let t = dedupe_thresholds(&config.thresholds.dedupe);
        for kind in DedupeKind::ALL {
            assert_eq!(t.get(*kind), DedupeThresholds::default_for(*kind), "{kind}");
        }
        let near = near_thresholds(&config.thresholds.dedupe);
        assert_eq!(
            near.into_iter().collect::<Vec<_>>(),
            DedupeKind::ALL
                .iter()
                .map(|k| (
                    k.as_str().to_owned(),
                    DedupeThresholds::default_for(*k).near
                ))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn configured_levels_replace_the_near_and_candidate_thresholds() {
        let config: BTreeMap<String, DedupeThreshold> = [
            (
                "note".to_owned(),
                DedupeThreshold {
                    near: 0.1,
                    semantic: Some(0.8),
                },
            ),
            (
                "task".to_owned(),
                DedupeThreshold {
                    near: 0.6,
                    semantic: Some(0.99),
                },
            ),
            (
                "person".to_owned(),
                DedupeThreshold {
                    near: 0.45,
                    semantic: None,
                },
            ),
            (
                "alias".to_owned(),
                DedupeThreshold {
                    near: 0.8,
                    semantic: Some(0.5),
                },
            ),
            (
                "bogus".to_owned(),
                DedupeThreshold {
                    near: 0.8,
                    semantic: Some(0.5),
                },
            ),
        ]
        .into();
        let t = dedupe_thresholds(&config);
        assert_eq!(
            t.get(DedupeKind::Note),
            DedupeThresholds {
                near: 0.1,
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
        // No semantic level configured: the default one.
        assert_eq!(
            t.get(DedupeKind::Person),
            DedupeThresholds {
                near: 0.45,
                semantic: Some(SemanticThresholds {
                    candidate: 0.90,
                    confirmed: 0.97
                })
            }
        );
        // Aliases have no semantic level, whatever the configuration says.
        assert_eq!(
            t.get(DedupeKind::Alias),
            DedupeThresholds {
                near: 0.8,
                semantic: None
            }
        );
        assert_eq!(
            t.get(DedupeKind::Company),
            DedupeThresholds::default_for(DedupeKind::Company)
        );
        assert_eq!(
            near_thresholds(&config),
            [
                ("alias".to_owned(), 0.8),
                ("note".to_owned(), 0.1),
                ("person".to_owned(), 0.45),
                ("task".to_owned(), 0.6),
            ]
            .into()
        );
        assert_eq!(
            AiThresholds::default(),
            AiThresholds {
                relation: 0.7,
                custody: 0.85
            }
        );
    }
}
