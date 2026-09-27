//! Tests for the helper methods and constants beyond string round trips.

use domain::{
    AccountStatus, CUSTODY_CONFIDENCE_THRESHOLD, DedupeKind, DedupeThresholds, EntityRelationType,
    GraphNodeKind, MatchLevel, MentionType, NOTE_RELATION_KEYS, NoteKind, Priority,
    RELATION_CONFIDENCE_THRESHOLD, RelationType, SemanticThresholds,
};

#[test]
fn relation_frontmatter_keys() {
    let keys: Vec<&str> = RelationType::ALL
        .iter()
        .map(|t| t.frontmatter_key())
        .collect();
    assert_eq!(
        keys,
        [
            "related",
            "part-of",
            "supports",
            "contradicts",
            "follows-up",
            "duplicates"
        ]
    );
    assert_eq!(&NOTE_RELATION_KEYS[..6], keys.as_slice());
    assert_eq!(
        &NOTE_RELATION_KEYS[6..],
        ["concepts", "people", "companies"]
    );
    let mentions: Vec<&str> = MentionType::ALL
        .iter()
        .map(|t| t.frontmatter_key())
        .collect();
    assert_eq!(&NOTE_RELATION_KEYS[6..], mentions.as_slice());
    let targets: Vec<NoteKind> = MentionType::ALL.iter().map(|t| t.target_kind()).collect();
    assert_eq!(
        targets,
        [NoteKind::Concept, NoteKind::Person, NoteKind::Company]
    );
    assert_eq!(
        RelationType::from_frontmatter_key("follows-up"),
        Some(RelationType::FollowsUp)
    );
    assert_eq!(RelationType::from_frontmatter_key("concepts"), None);
    assert_eq!(RelationType::from_frontmatter_key("tags"), None);
}

#[test]
fn only_duplicates_is_never_auto_applied() {
    let manual: Vec<RelationType> = RelationType::ALL
        .iter()
        .copied()
        .filter(|t| !t.is_auto_applicable())
        .collect();
    assert_eq!(manual, [RelationType::Duplicates]);
}

#[test]
fn entity_relation_subject_kinds() {
    let person: Vec<&str> = EntityRelationType::ALL
        .iter()
        .filter(|t| t.subject_kind() == NoteKind::Person)
        .map(|t| t.frontmatter_key())
        .collect();
    let company: Vec<&str> = EntityRelationType::ALL
        .iter()
        .filter(|t| t.subject_kind() == NoteKind::Company)
        .map(|t| t.frontmatter_key())
        .collect();
    assert_eq!(
        person,
        [
            "works-at",
            "worked-at",
            "reports-to",
            "knows",
            "introduced-by"
        ]
    );
    assert_eq!(
        company,
        [
            "client-of",
            "supplier-of",
            "partner-of",
            "competitor-of",
            "subsidiary-of"
        ]
    );
    assert_eq!(
        EntityRelationType::from_frontmatter_key("works-at"),
        Some(EntityRelationType::WorksAt)
    );
    assert_eq!(EntityRelationType::from_frontmatter_key("related"), None);
}

#[test]
fn note_kind_entities_and_folders() {
    let table: Vec<(NoteKind, bool, &str, GraphNodeKind)> = NoteKind::ALL
        .iter()
        .map(|k| {
            (
                *k,
                k.is_entity(),
                k.default_folder(),
                GraphNodeKind::from(*k),
            )
        })
        .collect();
    assert_eq!(
        table,
        [
            (NoteKind::Note, false, "notes", GraphNodeKind::Note),
            (NoteKind::Concept, false, "concepts", GraphNodeKind::Concept),
            (NoteKind::Person, true, "people", GraphNodeKind::Person),
            (NoteKind::Company, true, "companies", GraphNodeKind::Company),
            (
                NoteKind::Document,
                true,
                "documents",
                GraphNodeKind::Document
            ),
            (NoteKind::Place, true, "places", GraphNodeKind::Place),
        ]
    );
}

#[test]
fn priority_signifiers_and_order() {
    let table: Vec<(Priority, Option<&str>)> =
        Priority::ALL.iter().map(|p| (*p, p.signifier())).collect();
    assert_eq!(
        table,
        [
            (Priority::Highest, Some("🔺")),
            (Priority::High, Some("⏫")),
            (Priority::Medium, Some("🔼")),
            (Priority::Normal, None),
            (Priority::Low, Some("🔽")),
            (Priority::Lowest, Some("⏬")),
        ]
    );
    for p in Priority::ALL {
        if let Some(s) = p.signifier() {
            assert_eq!(Priority::from_signifier(s), Some(*p));
        }
    }
    assert_eq!(Priority::from_signifier("📅"), None);
    assert_eq!(Priority::from_signifier(""), None);
    let mut shuffled = vec![
        Priority::Low,
        Priority::Highest,
        Priority::Normal,
        Priority::High,
    ];
    shuffled.sort();
    assert_eq!(
        shuffled,
        [
            Priority::Highest,
            Priority::High,
            Priority::Normal,
            Priority::Low
        ]
    );
}

#[test]
fn account_sign_in() {
    let allowed: Vec<AccountStatus> = AccountStatus::ALL
        .iter()
        .copied()
        .filter(|s| s.can_sign_in())
        .collect();
    assert_eq!(
        allowed,
        [AccountStatus::Active, AccountStatus::DeletionPending]
    );
}

#[test]
fn match_level_order() {
    assert!(MatchLevel::Exact < MatchLevel::Near && MatchLevel::Near < MatchLevel::Semantic);
}

#[test]
fn confidence_thresholds() {
    assert_eq!(RELATION_CONFIDENCE_THRESHOLD.to_bits(), 0.7_f32.to_bits());
    assert_eq!(CUSTODY_CONFIDENCE_THRESHOLD.to_bits(), 0.85_f32.to_bits());
}

#[test]
fn dedupe_threshold_defaults() {
    let s = |candidate, confirmed| {
        Some(SemanticThresholds {
            candidate,
            confirmed,
        })
    };
    let table: Vec<(DedupeKind, DedupeThresholds)> = DedupeKind::ALL
        .iter()
        .map(|k| (*k, DedupeThresholds::default_for(*k)))
        .collect();
    assert_eq!(
        table,
        [
            (
                DedupeKind::Note,
                DedupeThresholds {
                    near: 0.6,
                    semantic: s(0.90, 0.96)
                }
            ),
            (
                DedupeKind::Capture,
                DedupeThresholds {
                    near: 0.6,
                    semantic: s(0.90, 0.96)
                }
            ),
            (
                DedupeKind::Task,
                DedupeThresholds {
                    near: 0.6,
                    semantic: s(0.88, 0.95)
                }
            ),
            (
                DedupeKind::Person,
                DedupeThresholds {
                    near: 0.5,
                    semantic: s(0.90, 0.97)
                }
            ),
            (
                DedupeKind::Company,
                DedupeThresholds {
                    near: 0.5,
                    semantic: s(0.90, 0.97)
                }
            ),
            (
                DedupeKind::Concept,
                DedupeThresholds {
                    near: 0.5,
                    semantic: s(0.88, 0.95)
                }
            ),
            (
                DedupeKind::Alias,
                DedupeThresholds {
                    near: 0.6,
                    semantic: None
                }
            ),
            (
                DedupeKind::Document,
                DedupeThresholds {
                    near: 0.8,
                    semantic: s(0.92, 0.97)
                }
            ),
            (
                DedupeKind::Place,
                DedupeThresholds {
                    near: 0.8,
                    semantic: s(0.92, 0.97)
                }
            ),
        ]
    );
    for (_, t) in &table {
        assert!(t.near > 0.0 && t.near <= 1.0);
        if let Some(sem) = t.semantic {
            assert!(sem.candidate < sem.confirmed && sem.confirmed <= 1.0);
        }
    }
}

/// The near thresholds are calibrated against real `pg_trgm` scores (see the parity table in
/// `text-normalize`); these pairs pin the intended side of each threshold.
#[test]
fn near_thresholds_separate_calibration_pairs() {
    use text_normalize::trigram_similarity;
    let near = |k| DedupeThresholds::default_for(k).near;
    let cases: &[(DedupeKind, &str, &str, bool)] = &[
        (
            DedupeKind::Task,
            "Watanya's ETA invoice",
            "ETA invoice for Watanya",
            true,
        ),
        (
            DedupeKind::Task,
            "Petrol Arrows invoice",
            "Watanya's ETA invoice",
            false,
        ),
        (DedupeKind::Note, "Churn notes", "Churn note", true),
        (
            DedupeKind::Note,
            "Pricing experiments",
            "pricing tests",
            false,
        ),
        (DedupeKind::Person, "Shady", "Shadi", true),
        (DedupeKind::Person, "Ahmed Samir", "Ahmed Fathy", false),
        (DedupeKind::Concept, "Loyalty", "Loyality", true),
        (DedupeKind::Company, "Acme Logistics", "Acme Logistic", true),
        (
            DedupeKind::Place,
            "Nasr City office",
            "Safe — Nasr City office",
            false,
        ),
        (DedupeKind::Place, "مكتب مدينة نصر", "مكتب مدينه نصر", true),
        (DedupeKind::Document, "عقد وطنية", "عقد شركة وطنية", false),
    ];
    for (kind, a, b, flagged) in cases {
        let score = trigram_similarity(a, b);
        assert_eq!(
            score >= near(*kind),
            *flagged,
            "{kind} {a:?} vs {b:?}: {score}"
        );
    }
}
