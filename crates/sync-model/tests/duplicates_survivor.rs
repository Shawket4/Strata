//! The survivor of an accepted `duplicates` suggestion (PLAN §9.7 "propose `duplicates`
//! merges"): the rule both the server (which merges) and the device (which says what accepting
//! does) apply.

use chrono::{DateTime, Utc};
use dedupe::MatchLevel;
use pretty_assertions::assert_eq;
use sync_model::suggestions::{DuplicateItem, DuplicatesPayload, DuplicatesSurvivor};
use ulid::Ulid;

fn item(id: u128, item: &str, kind: &str) -> DuplicateItem {
    DuplicateItem {
        id: Ulid::from(id),
        item: item.to_owned(),
        snippet: None,
        kind: kind.to_owned(),
        title: "x".to_owned(),
        match_level: MatchLevel::Semantic,
        score: 0.93,
    }
}

fn pair(a: DuplicateItem, b: DuplicateItem) -> DuplicatesPayload {
    DuplicatesPayload { a, b, reason: None }
}

fn at(s: &str) -> Option<DateTime<Utc>> {
    Some(s.parse().unwrap_or_default())
}

fn note(n: u128) -> DuplicateItem {
    item(n, &Ulid::from(n).to_string(), "note")
}

#[test]
fn the_note_created_first_survives() {
    let p = pair(note(1), note(2));
    assert!(!p.is_task_pair());
    assert_eq!(
        p.survivor(at("2026-09-20T08:00:00Z"), at("2026-09-10T08:00:00Z")),
        DuplicatesSurvivor::B
    );
    assert_eq!(
        p.survivor(at("2026-09-10T08:00:00Z"), at("2026-09-20T08:00:00Z")),
        DuplicatesSurvivor::A
    );
    // Equal times: the smaller ID.
    let same = at("2026-09-10T08:00:00Z");
    assert_eq!(p.survivor(same, same), DuplicatesSurvivor::A);
    assert_eq!(pair(note(2), note(1)).survivor(same, same), DuplicatesSurvivor::B);
    // A note without `created` counts as the oldest.
    assert_eq!(p.survivor(same, None), DuplicatesSurvivor::B);
    // Entities follow the same rule.
    let e = pair(item(5, &Ulid::from(5).to_string(), "person"), note(4));
    assert_eq!(e.survivor(same, same), DuplicatesSurvivor::B);
}

#[test]
fn task_pairs_keep_the_line_whose_id_sorts_first() {
    let p = pair(item(1, "t-b2", "task"), item(1, "t-a1", "task"));
    assert!(p.is_task_pair());
    assert_eq!(p.survivor(None, None), DuplicatesSurvivor::B);
    let p = pair(item(1, "t-a1", "task"), item(1, "t-b2", "task"));
    assert_eq!(
        p.survivor(at("2030-01-01T00:00:00Z"), None),
        DuplicatesSurvivor::A,
        "times do not matter for tasks"
    );
}
