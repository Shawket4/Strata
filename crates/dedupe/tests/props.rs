//! Properties of the duplicate decision.

use dedupe::{DedupeKind, Existing, Item, KeepBothSet, MatchLevel, Thresholds, check, score_pair};
use proptest::prelude::*;

fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z]{1,8}",
        "[ابتثجحخدذرزسشصضطظعغفقكلمنهوي]{1,6}",
        Just("Watanya's".to_owned()),
        Just("وطنية".to_owned()),
        Just("the".to_owned()),
    ]
}

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(word(), 1..5).prop_map(|w| w.join(" "))
}

fn kind() -> impl Strategy<Value = DedupeKind> {
    prop::sample::select(DedupeKind::ALL.to_vec())
}

fn item(id: &'static str) -> impl Strategy<Value = Item> {
    (kind(), text(), prop::collection::vec(text(), 0..3), prop::option::of("FREQ=(DAILY|WEEKLY)"))
        .prop_map(move |(kind, text, aliases, rrule)| Item {
            kind,
            id: Some(id.to_owned()),
            title: text.clone(),
            text,
            aliases,
            rrule,
            entities: vec![],
            snippet: None,
        })
}

proptest! {
    #[test]
    fn score_is_symmetric(a in item("a"), b in item("b")) {
        prop_assert_eq!(score_pair(&a, &b), score_pair(&b, &a));
    }

    #[test]
    fn decision_is_symmetric_for_same_kind(k in kind(), x in text(), y in text()) {
        let a = Item { kind: k, ..Item::note(Some("a"), &x) };
        let b = Item { kind: k, ..Item::note(Some("b"), &y) };
        let ab = check(&a, &[Existing::from(b.clone())], &Thresholds::default(), &KeepBothSet::new());
        let ba = check(&b, &[Existing::from(a)], &Thresholds::default(), &KeepBothSet::new());
        let ab: Vec<_> = ab.candidates.iter().map(|c| (c.level, c.score)).collect();
        let ba: Vec<_> = ba.candidates.iter().map(|c| (c.level, c.score)).collect();
        prop_assert_eq!(ab, ba);
    }

    #[test]
    fn case_and_diacritics_never_matter(k in kind(), x in text()) {
        let a = Item { kind: k, ..Item::note(Some("a"), &x) };
        let b = Item { kind: k, ..Item::note(Some("b"), &format!("  {}! ", x.to_uppercase())) };
        let out = check(&a, &[Existing::from(b)], &Thresholds::default(), &KeepBothSet::new());
        prop_assert_eq!(out.candidates.len(), 1);
        prop_assert_eq!(out.candidates[0].level, MatchLevel::Exact);
        prop_assert_eq!(out.candidates[0].score, 1.0);
    }

    #[test]
    fn scores_are_bounded_and_ranked(new in item("new"), others in prop::collection::vec(item("x"), 0..6)) {
        let existing: Vec<Existing> = others
            .into_iter()
            .enumerate()
            .map(|(i, mut it)| { it.id = Some(format!("id{i}")); Existing::from(it) })
            .collect();
        let out = check(&new, &existing, &Thresholds::default(), &KeepBothSet::new());
        for c in &out.candidates {
            prop_assert!((0.0..=1.0).contains(&c.score));
            prop_assert!(c.id != "new");
        }
        for w in out.candidates.windows(2) {
            prop_assert!(w[0].level < w[1].level || (w[0].level == w[1].level && w[0].score >= w[1].score));
        }
    }
}
