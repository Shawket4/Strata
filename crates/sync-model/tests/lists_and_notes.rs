//! `entity.patch` list values (`set_lists`) and custody notes (`document.custody.note`).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use domain::CustodyEventType;
use pretty_assertions::assert_eq;
use sync_model::apply::{
    ApplyError, clean_list_value, custody_event, entity_patch, record_custody,
};
use sync_model::ops::{DocumentCustody, EntityPatch};
use ulid::Ulid;
use vault_format::Document;

fn lists(entries: &[(&str, &[&str])]) -> EntityPatch {
    EntityPatch {
        id: Ulid::from(1_u128),
        set_lists: entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect()))
            .collect(),
        ..EntityPatch::default()
    }
}

#[test]
fn set_lists_replaces_whole_lists_in_canonical_order() {
    let mut doc = Document::parse(
        "---\nid: 01J\nkind: person\naliases: [Ahmed S.]\ntags: [old]\nphone: \"+20 1\"\n---\nx\n",
    );
    let patch = lists(&[
        ("aliases", &[" أحمد سمير ", "Ahmed S.", "أحمد سمير", ""]),
        ("tags", &["#client", "vip", "client"]),
        ("phone", &["+20 100", "+20 101"]),
        ("languages", &["ar", "en"]),
    ]);
    assert_eq!(entity_patch(doc.frontmatter_mut(), &patch), Ok(()));
    assert_eq!(
        doc.render(),
        "---\nid: 01J\nkind: person\naliases: [أحمد سمير, Ahmed S.]\ntags: [client, vip]\nphone: [+20 100, +20 101]\nlanguages: [ar, en]\n---\nx\n"
    );
    // One value of a user field is a scalar; tags and aliases stay lists.
    let mut single = Document::parse("---\nid: 01J\nkind: person\n---\nx\n");
    let patch = lists(&[("phone", &[" +20 100 ", ""]), ("tags", &["vip"])]);
    assert_eq!(entity_patch(single.frontmatter_mut(), &patch), Ok(()));
    assert_eq!(
        single.render(),
        "---\nid: 01J\nkind: person\ntags: [vip]\nphone: +20 100\n---\nx\n"
    );
    // An empty list (after cleaning) removes the key.
    let patch = lists(&[("tags", &[" ", "#"]), ("phone", &[])]);
    assert_eq!(entity_patch(doc.frontmatter_mut(), &patch), Ok(()));
    assert_eq!(
        doc.render(),
        "---\nid: 01J\nkind: person\naliases: [أحمد سمير, Ahmed S.]\nlanguages: [ar, en]\n---\nx\n"
    );
}

#[test]
fn set_lists_refuses_relations_ids_and_custody_fields() {
    let mut doc = Document::parse("---\nid: 01J\nkind: document\n---\nx\n");
    for (key, err) in [
        ("id", ApplyError::ImmutableKey("id".into())),
        ("kind", ApplyError::ImmutableKey("kind".into())),
        ("holder", ApplyError::NotPatchable("holder".into())),
        ("works-at", ApplyError::NotPatchable("works-at".into())),
        ("related", ApplyError::NotPatchable("related".into())),
    ] {
        let before = doc.render();
        let patch = lists(&[("tags", &["a"]), (key, &["[[x]]"])]);
        assert_eq!(
            entity_patch(doc.frontmatter_mut(), &patch),
            Err(err),
            "{key}"
        );
        assert_eq!(doc.render(), before, "nothing changes on error");
    }
}

#[test]
fn clean_list_value_trims_and_dedupes() {
    let v = |s: &[&str]| s.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        clean_list_value("tags", &v(&["#a", " a ", "#", "b"])),
        v(&["a", "b"])
    );
    assert_eq!(
        clean_list_value("phone", &v(&["#1", " #1 ", ""])),
        v(&["#1"])
    );
}

fn custody(note: Option<&str>) -> DocumentCustody {
    DocumentCustody {
        document_id: Ulid::from(1_u128),
        event: CustodyEventType::HandedTo,
        at: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
        place_id: None,
        person_id: Some(Ulid::from(3_u128)),
        counterparty_id: None,
        note: note.map(str::to_owned),
    }
}

fn link(id: Ulid) -> Option<String> {
    (u128::from(id) == 3).then(|| "[[Shady]]".to_owned())
}

#[test]
fn custody_notes_are_written_last_on_one_line() {
    let e = custody_event(&custody(Some("  for the\n audit ")), link, vec![]).unwrap();
    assert_eq!(e.note.as_deref(), Some("for the audit"));
    assert_eq!(
        e.to_line(),
        "- 2026-09-21 — handed-to [[Shady]] — for the audit"
    );
    let cited = custody_event(&custody(Some("x")), link, vec!["[[C]]".into()]).unwrap();
    assert_eq!(
        cited.to_line(),
        "- 2026-09-21 — handed-to [[Shady]] — [[C]] — x"
    );
    // A blank note is no note.
    let e = custody_event(&custody(Some(" \n ")), link, vec![]).unwrap();
    assert_eq!(e.to_line(), "- 2026-09-21 — handed-to [[Shady]]");
    // A note of links only would read back as citations: refused, never written.
    assert!(matches!(
        custody_event(&custody(Some("[[Safe]]")), link, vec![]),
        Err(ApplyError::InvalidCustodyEvent(_))
    ));
    let mut doc = Document::parse("---\nid: 01J\nkind: document\n---\n## Notes\n");
    let e = custody_event(&custody(Some("for the audit")), link, vec![]).unwrap();
    assert!(record_custody(&mut doc, e).is_ok());
    assert_eq!(
        doc.render(),
        "---\nid: 01J\nkind: document\nlocation: \"\"\nholder: \"[[Shady]]\"\nlast-holder: \"[[Shady]]\"\nstatus: checked-out\n---\n## Custody\n- 2026-09-21 — handed-to [[Shady]] — for the audit\n\n## Notes\n"
    );
}

#[test]
fn custody_note_is_omitted_from_the_wire_when_absent() {
    let value = serde_json::to_value(custody(None)).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "at",
            "counterparty_id",
            "document_id",
            "person_id",
            "place_id",
            "type"
        ]
    );
    let back: DocumentCustody = serde_json::from_value(value).unwrap();
    assert_eq!(back, custody(None));
    let with_note = serde_json::to_value(custody(Some("x"))).unwrap();
    assert_eq!(with_note["note"], "x");
    let empty = BTreeMap::<String, Vec<String>>::new();
    let patch = EntityPatch::default();
    assert_eq!(patch.set_lists, empty);
}
