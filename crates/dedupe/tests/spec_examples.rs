//! The duplicate examples from PLAN §9.7 / §16 and the rules around them.

use dedupe::{
    CandidateQuery, CheckOutcome, DedupeKind, DedupeThresholds, DuplicateCandidate, Existing,
    Item, KeepBoth, KeepBothSet, MatchLevel, SemanticEvidence, Thresholds, check, score_pair,
    sweep,
};
use pretty_assertions::assert_eq;

fn run(new: &Item, existing: &[Item]) -> CheckOutcome {
    let existing: Vec<Existing> = existing.iter().cloned().map(Existing::from).collect();
    check(new, &existing, &Thresholds::default(), &KeepBothSet::new())
}

fn candidate(id: &str, kind: DedupeKind, title: &str, level: MatchLevel, score: f32) -> DuplicateCandidate {
    DuplicateCandidate {
        id: id.to_owned(),
        kind,
        title: title.to_owned(),
        snippet: None,
        level,
        score,
    }
}

fn watanya_task() -> Item {
    Item::task(
        Some("t-01j9a2"),
        "Make Watanya's ETA invoice",
        Some("FREQ=MONTHLY;BYMONTHDAY=1"),
        &["Watanya"],
    )
    .with_snippet("monthly · next Thu 1 Oct")
}

#[test]
fn capture_remind_me_matches_monthly_task_as_near() {
    let new = Item::capture(Some("01CAPTURE"), "remind me to make watanya's invoice");
    let out = run(&new, &[watanya_task()]);
    assert_eq!(
        out.candidates,
        [DuplicateCandidate {
            id: "t-01j9a2".into(),
            kind: DedupeKind::Task,
            title: "Make Watanya's ETA invoice".into(),
            snippet: Some("monthly · next Thu 1 Oct".into()),
            level: MatchLevel::Near,
            score: 0.84,
        }]
    );
    assert!(out.candidates[0].score >= DedupeThresholds::default_for(DedupeKind::Task).near);
    assert!(out.is_duplicate());
}

#[test]
fn new_task_same_text_other_schedule_is_near_not_exact() {
    let new = Item::task(Some("t-new"), "Make Watanya ETA invoice", Some("FREQ=WEEKLY"), &["Watanya"]);
    let out = run(&new, &[watanya_task()]);
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.candidates[0].level, MatchLevel::Near);
    assert_eq!(out.candidates[0].score, 1.0);

    let same = Item::task(
        Some("t-new"),
        "make watanya's ETA invoice",
        Some("RRULE:BYMONTHDAY=1;FREQ=MONTHLY"),
        &["watanya"],
    );
    let out = run(&same, &[watanya_task()]);
    assert_eq!(out.candidates[0].level, MatchLevel::Exact);
}

#[test]
fn pricing_experiment_plural_is_exact() {
    let new = Item::note(Some("01NEW"), "Pricing experiment");
    let existing = Item::note(Some("01OLD"), "Pricing experiments");
    let out = run(&new, &[existing]);
    assert_eq!(
        out.candidates,
        [candidate("01OLD", DedupeKind::Note, "Pricing experiments", MatchLevel::Exact, 1.0)]
    );
}

#[test]
fn reworded_note_titles_are_exact() {
    let new = Item::note(Some("01NEW"), "ETA invoice for Watanya");
    let existing = Item::note(Some("01OLD"), "Watanya's ETA invoice");
    assert_eq!(run(&new, &[existing]).candidates[0].level, MatchLevel::Exact);
}

#[test]
fn ahmed_sameer_matches_ahmed_samir_alias_as_near() {
    let existing = Item::entity(
        DedupeKind::Person,
        Some("01AHMED"),
        "Ahmed Samir",
        &["أحمد سمير", "A. Samir"],
    );
    // A new person: trigrams already pass (0.5625 ≥ 0.5); the phonetic score is higher.
    let new = Item::entity(DedupeKind::Person, Some("01NEWP"), "Ahmed Sameer", &[]);
    let out = run(&new, std::slice::from_ref(&existing));
    assert_eq!(
        out.candidates,
        [candidate("01AHMED", DedupeKind::Person, "Ahmed Samir", MatchLevel::Near, 1.0 - 2.0 / 12.0)]
    );
    let pair = score_pair(&new, &existing);
    assert_eq!((pair.exact, pair.trigram, pair.phonetic), (false, 0.5625, Some(1.0 - 2.0 / 12.0)));

    // "Ahmad Sameer": trigrams alone would not match (0.316 < 0.5); the equal
    // transliteration key plus spelling similarity 0.75 decides.
    let new = Item::entity(DedupeKind::Person, Some("01NEWP"), "Ahmad Sameer", &[]);
    let pair = score_pair(&new, &existing);
    assert_eq!((pair.exact, pair.trigram, pair.phonetic), (false, 0.315_789_46, Some(0.75)));
    assert_eq!(
        run(&new, std::slice::from_ref(&existing)).candidates,
        [candidate("01AHMED", DedupeKind::Person, "Ahmed Samir", MatchLevel::Near, 0.75)]
    );

    // A new alias on another entity.
    let alias = Item::alias(Some("01OTHER"), "Ahmed Sameer");
    assert_eq!(run(&alias, std::slice::from_ref(&existing)).candidates[0].level, MatchLevel::Near);
}

#[test]
fn arabic_spelling_of_a_person_matches_cross_script() {
    let existing = Item::entity(DedupeKind::Person, Some("01AHMED"), "Ahmed Samir", &[]);
    let new = Item::entity(DedupeKind::Person, Some("01NEWP"), "أحمد سمير", &[]);
    let out = run(&new, &[existing]);
    assert_eq!(
        out.candidates,
        [candidate("01AHMED", DedupeKind::Person, "Ahmed Samir", MatchLevel::Near, 0.75)]
    );
}

#[test]
fn distinct_people_do_not_match() {
    let existing = Item::entity(DedupeKind::Person, Some("01AHMED"), "Ahmed Samir", &["أحمد سمير"]);
    for name in ["Ahmed Fathy", "أحمد فتحي", "Ali", "Ahmad Samar Hassan"] {
        let new = Item::entity(DedupeKind::Person, Some("01NEWP"), name, &[]);
        assert_eq!(run(&new, std::slice::from_ref(&existing)).candidates, [], "{name}");
    }
}

#[test]
fn nested_place_is_not_a_duplicate() {
    let existing = Item::entity(DedupeKind::Place, Some("01OFFICE"), "Nasr City office", &["مكتب مدينة نصر"]);
    let new = Item::entity(DedupeKind::Place, Some("01SAFE"), "Safe — Nasr City office", &[]);
    let out = run(&new, std::slice::from_ref(&existing));
    assert_eq!(out, CheckOutcome::default());
    // The trigram score is high (0.77) but below the stricter place threshold (0.8).
    let pair = score_pair(&new, &existing);
    assert_eq!(pair.trigram, 0.772_727_25);
    assert!(!pair.exact);
}

#[test]
fn place_spelling_variant_is_a_duplicate() {
    let existing = Item::entity(DedupeKind::Place, Some("01OFFICE"), "Nasr City office", &[]);
    let new = Item::entity(DedupeKind::Place, Some("01NEW"), "Nasr-City Office!", &[]);
    assert_eq!(run(&new, &[existing]).candidates[0].level, MatchLevel::Exact);
}

#[test]
fn documents_are_stricter_than_notes() {
    let existing = Item::entity(DedupeKind::Document, Some("01DOC"), "Watanya contract", &["عقد وطنية"]);
    // 0.74 would be a near duplicate between notes (0.6) but not between documents (0.8).
    let lease = Item::entity(DedupeKind::Document, Some("01LEASE"), "Watanya lease contract", &[]);
    assert_eq!(run(&lease, std::slice::from_ref(&existing)).candidates, []);
    let as_notes = Item::note(Some("01LEASE"), "Watanya lease contract");
    assert_eq!(run(&as_notes, &[Item::note(Some("01DOC"), "Watanya contract")]).candidates[0].level, MatchLevel::Near);
    let arabic = Item::entity(DedupeKind::Document, Some("01NEW"), "عقد وطنيه", &[]);
    assert_eq!(run(&arabic, &[existing]).candidates[0].level, MatchLevel::Exact);
}

#[test]
fn arabic_and_english_company_variants() {
    let watanya = Item::entity(DedupeKind::Company, Some("01WAT"), "Watanya", &["وطنية", "ووتانيا"]);
    // Ta marbuta / ha spelling of a configured alias → exact.
    let new = Item::entity(DedupeKind::Company, Some("01NEW"), "وطنيه", &[]);
    assert_eq!(run(&new, std::slice::from_ref(&watanya)).candidates[0].level, MatchLevel::Exact);
    // Hamza/tashkeel variants → exact.
    let new = Item::entity(DedupeKind::Company, Some("01NEW"), "وَطَنِيَّة", &[]);
    assert_eq!(run(&new, std::slice::from_ref(&watanya)).candidates[0].level, MatchLevel::Exact);
    // Latin variant spelling → near via trigram threshold? "Watania" scores 0.45 < 0.5; the
    // transliteration key `tn` is too short to propose it, so it is not flagged.
    let new = Item::entity(DedupeKind::Company, Some("01NEW"), "Watania", &[]);
    assert_eq!(run(&new, std::slice::from_ref(&watanya)).candidates, []);
    // Unrelated company.
    let new = Item::entity(DedupeKind::Company, Some("01NEW"), "Petrol Arrows", &[]);
    assert_eq!(run(&new, &[watanya]).candidates, []);
}

#[test]
fn arabic_capture_matches_arabic_task() {
    let task = Item::task(Some("t-ar"), "اعمل فاتورة وطنية", Some("FREQ=MONTHLY;BYMONTHDAY=1"), &["Watanya"]);
    let new = Item::capture(Some("01CAP"), "فكّرني أعمل فاتورة وطنيّة");
    let out = run(&new, &[task]);
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.candidates[0].level, MatchLevel::Near);
    assert_eq!(out.candidates[0].score, 1.0);
}

#[test]
fn keep_both_suppresses_in_either_order_and_kind() {
    let new = Item::capture(Some("01CAPTURE"), "remind me to make watanya's invoice");
    let existing = vec![Existing::from(watanya_task())];
    let flagged = check(&new, &existing, &Thresholds::default(), &KeepBothSet::new());
    let pairs = KeepBoth::for_forced_create(DedupeKind::Capture, "01CAPTURE", &flagged.candidates);
    assert_eq!(
        pairs,
        [KeepBoth {
            kind: DedupeKind::Capture,
            a_id: "01CAPTURE".into(),
            b_id: "t-01j9a2".into()
        }]
    );
    let set: KeepBothSet = pairs.into_iter().collect();
    let out = check(&new, &existing, &Thresholds::default(), &set);
    assert_eq!(
        out,
        CheckOutcome {
            candidates: vec![],
            needs_confirmation: vec![],
            suppressed: vec!["t-01j9a2".into()],
        }
    );
    // Recorded under the task's kind, reversed order: still suppressed.
    let set: KeepBothSet = [KeepBoth::new(DedupeKind::Task, "t-01j9a2", "01CAPTURE")].into_iter().collect();
    assert_eq!(check(&new, &existing, &Thresholds::default(), &set).candidates, []);
    // Recorded under an unrelated kind: not suppressed.
    let set: KeepBothSet = [KeepBoth::new(DedupeKind::Note, "t-01j9a2", "01CAPTURE")].into_iter().collect();
    assert_eq!(check(&new, &existing, &Thresholds::default(), &set).candidates.len(), 1);
    // A new item without an ID can never be suppressed.
    let anon = Item::capture(None, "remind me to make watanya's invoice");
    let set: KeepBothSet = [KeepBoth::new(DedupeKind::Capture, "t-01j9a2", "01CAPTURE")].into_iter().collect();
    assert_eq!(check(&anon, &existing, &Thresholds::default(), &set).candidates.len(), 1);
}

#[test]
fn ranking_orders_by_level_then_score_then_id() {
    let new = Item::note(Some("01NEW"), "Churn notes");
    let existing = vec![
        Existing::from(Item::note(Some("01D"), "Churn note")),            // exact (plural)
        Existing::from(Item::note(Some("01C"), "Churn noted")),           // near
        Existing::from(Item::note(Some("01B"), "notes on churn")),        // exact (stopword, order)
        Existing::from(Item::note(Some("01A"), "Churn notes draft")),     // near, lower
        Existing {
            item: Item::note(Some("01E"), "Why customers leave"),
            semantic: Some(SemanticEvidence { cosine: 0.97, llm_confirmed: None }),
        },
        Existing {
            item: Item::note(Some("01F"), "Customer attrition"),
            semantic: Some(SemanticEvidence { cosine: 0.93, llm_confirmed: Some(true) }),
        },
        Existing {
            item: Item::note(Some("01G"), "Retention ideas"),
            semantic: Some(SemanticEvidence { cosine: 0.93, llm_confirmed: None }),
        },
        Existing {
            item: Item::note(Some("01H"), "Loyalty ideas"),
            semantic: Some(SemanticEvidence { cosine: 0.93, llm_confirmed: Some(false) }),
        },
        Existing::from(Item::note(Some("01I"), "Pricing")),               // nothing
        Existing::from(Item::entity(DedupeKind::Concept, Some("01J"), "Churn notes", &[])), // incompatible kind
        Existing::from(Item::note(Some("01NEW"), "Churn notes")),          // itself
        Existing::from(Item::note(None, "Churn notes")),                   // no id
    ];
    let out = check(&new, &existing, &Thresholds::default(), &KeepBothSet::new());
    let got: Vec<(&str, MatchLevel)> = out.candidates.iter().map(|c| (c.id.as_str(), c.level)).collect();
    assert_eq!(
        got,
        [
            ("01B", MatchLevel::Exact),
            ("01D", MatchLevel::Exact),
            ("01C", MatchLevel::Near),
            ("01A", MatchLevel::Near),
            ("01E", MatchLevel::Semantic),
            ("01F", MatchLevel::Semantic),
        ]
    );
    assert!(out.candidates[2].score > out.candidates[3].score);
    assert_eq!(out.candidates[4].score, 0.97);
    assert_eq!(out.needs_confirmation, ["01G"]);
}

#[test]
fn threshold_overrides_apply() {
    let new = Item::entity(DedupeKind::Place, Some("01SAFE"), "Safe — Nasr City office", &[]);
    let existing = Item::entity(DedupeKind::Place, Some("01OFFICE"), "Nasr City office", &[]);
    let lenient = Thresholds::new().with(
        DedupeKind::Place,
        DedupeThresholds { near: 0.7, semantic: None },
    );
    let out = check(&new, &[Existing::from(existing)], &lenient, &KeepBothSet::new());
    assert_eq!(out.candidates[0].level, MatchLevel::Near);
    assert_eq!(lenient.semantic_between(DedupeKind::Place, DedupeKind::Place), None);
    assert_eq!(lenient.near_between(DedupeKind::Place, DedupeKind::Alias), 0.7);
}

#[test]
fn candidate_query_lists_keys_and_floor() {
    let new = Item::capture(Some("01CAP"), "Remind me to make Watanya's invoice");
    let q = CandidateQuery::for_item(&new, &Thresholds::default());
    assert_eq!(
        q,
        CandidateQuery {
            kinds: vec![DedupeKind::Capture, DedupeKind::Note, DedupeKind::Task],
            exact_keys: vec!["invoice make watanya".into()],
            trigram_texts: vec!["make watanya invoice".into()],
            trigram_floor: 0.6,
            phonetic_keys: vec![],
        }
    );
    let alias = Item::alias(None, "Ahmed Sameer");
    let q = CandidateQuery::for_item(&alias, &Thresholds::default());
    // Alias (0.6) against people/companies (0.5): the stricter bound, 0.6.
    assert_eq!(q.trigram_floor, 0.6);
    assert_eq!(q.phonetic_keys, ["hmdsmr"]);
}

#[test]
fn sweep_finds_pairs_and_skips_keep_both() {
    let items = vec![
        Item::note(Some("01A"), "Pricing experiments"),
        Item::note(Some("01B"), "Pricing experiment"),
        Item::note(Some("01C"), "pricing tests"),
        Item::capture(Some("01D"), "Pricing experiments!"),
        Item::entity(DedupeKind::Concept, Some("01E"), "Pricing experiments", &[]),
    ];
    let pairs = sweep(&items, &Thresholds::default(), &KeepBothSet::new());
    let got: Vec<(&str, &str, MatchLevel)> =
        pairs.iter().map(|p| (p.a_id.as_str(), p.b_id.as_str(), p.level)).collect();
    assert_eq!(
        got,
        [
            ("01A", "01B", MatchLevel::Exact),
            ("01A", "01D", MatchLevel::Exact),
            ("01B", "01D", MatchLevel::Exact),
        ]
    );
    let keep: KeepBothSet = [KeepBoth::new(DedupeKind::Note, "01B", "01A")].into_iter().collect();
    assert_eq!(sweep(&items, &Thresholds::default(), &keep).len(), 2);
}

#[test]
fn outcome_round_trips_as_named_msgpack() {
    let new = Item::capture(Some("01CAPTURE"), "remind me to make watanya's invoice");
    let out = run(&new, &[watanya_task()]);
    let bytes = rmp_serde::to_vec_named(&out).unwrap();
    let back: CheckOutcome = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(back, out);
    let value: serde_json::Value = serde_json::to_value(&out.candidates[0]).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "id": "t-01j9a2", "kind": "task", "title": "Make Watanya's ETA invoice",
            "snippet": "monthly · next Thu 1 Oct", "level": "near", "score": 0.8399999737739563
        })
    );
}
