//! Exact bytes of every item this crate renders (PLAN §6.4, §6.6, §6.7, §6.9, §6.12, D19).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate};
use domain::{CopyKind, NoteKind};
use item_render::entity::{
    ENTITY_BODY, EntitySpec, concept_body, document_fields, document_relations, place_fields,
    skeleton,
};
use item_render::paths::{capture_path, conflict_copy_path, entity_path};
use pretty_assertions::assert_eq;
use sync_model::ops::{DocumentCreate, EntityCreate, PlaceCreate};
use ulid::Ulid;
use vault_format::RelationKey;

fn id(s: &str) -> Ulid {
    Ulid::from_string(s).unwrap()
}

const ID: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1H";

#[test]
fn person_with_everything() {
    let at = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").unwrap();
    let spec = EntitySpec {
        aliases: vec![
            " أحمد سمير ".into(),
            "#Ahmed S.".into(),
            "أحمد سمير".into(),
            " ".into(),
        ],
        tags: vec!["#client".into(), "client".into()],
        fields: BTreeMap::from([
            ("role".to_owned(), "Operations manager".to_owned()),
            ("email".to_owned(), "a@example.com".to_owned()),
        ]),
        relations: vec![
            (
                RelationKey::Entity(domain::EntityRelationType::WorksAt),
                "Acme Logistics".into(),
            ),
            (
                RelationKey::Entity(domain::EntityRelationType::WorksAt),
                "acme logistics".into(),
            ),
        ],
        ..EntitySpec::new(NoteKind::Person, "  Ahmed Samir ")
    };
    assert_eq!(spec.stem(), "Ahmed Samir");
    assert_eq!(
        spec.render(id(ID), Some(&at), Some(&at)).unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: person\naliases: [أحمد سمير, Ahmed S.]\n\
         tags: [client]\ncreated: 2026-09-27T14:32:00+03:00\nupdated: 2026-09-27T14:32:00+03:00\n\
         role: Operations manager\nemail: a@example.com\nworks-at: [\"[[Acme Logistics]]\"]\n---\n## Notes\n"
    );
    // Without the server's clock (the device) the timestamps are left out.
    assert_eq!(
        EntitySpec::new(NoteKind::Company, "Acme")
            .render(id(ID), None, None)
            .unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: company\n---\n## Notes\n"
    );
}

#[test]
fn a_name_that_is_not_a_file_name_is_kept_as_title() {
    let spec = EntitySpec::new(NoteKind::Company, "Q3: plan / review");
    assert_eq!(spec.stem(), "Q3 - plan - review");
    assert_eq!(
        spec.render(id(ID), None, None).unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: company\ntitle: \"Q3: plan / review\"\n---\n## Notes\n"
    );
}

#[test]
fn ops_become_specs() {
    let entity = EntityCreate {
        id: id(ID),
        kind: NoteKind::Person,
        name: "Shady".into(),
        aliases: vec!["شادي".into()],
        fields: BTreeMap::from([("phone".to_owned(), "+20 100".to_owned())]),
        force: false,
    };
    assert_eq!(
        EntitySpec::from_entity_create(&entity)
            .render(entity.id, None, None)
            .unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: person\naliases: [شادي]\nphone: +20 100\n---\n## Notes\n"
    );

    let original = id("01J8ZK3M4X7Q9W2E5R6T8Y0V1A");
    let company = id("01J8ZK3M4X7Q9W2E5R6T8Y0V1B");
    let person = id("01J8ZK3M4X7Q9W2E5R6T8Y0V1C");
    let doc = DocumentCreate {
        id: id(ID),
        name: "Watanya contract (copy)".into(),
        aliases: Vec::new(),
        doc_type: Some("contract".into()),
        copy: Some(CopyKind::CertifiedCopy),
        copy_of: Some(original),
        companies: vec![company],
        people: vec![person],
        expires: NaiveDate::from_ymd_opt(2027, 3, 31),
        force: false,
    };
    assert_eq!(
        document_fields(&doc),
        BTreeMap::from([
            ("copy".to_owned(), "certified copy".to_owned()),
            ("doc-type".to_owned(), "contract".to_owned()),
            ("expires".to_owned(), "2027-03-31".to_owned()),
        ])
    );
    let rels = document_relations(&doc);
    assert_eq!(
        rels.iter()
            .map(|(r, i)| (r.as_str(), *i))
            .collect::<Vec<_>>(),
        vec![
            ("copy-of", original),
            ("companies", company),
            ("people", person)
        ]
    );
    let texts = vec![
        (rels[0].0, "Watanya contract".to_owned()),
        (rels[1].0, "Watanya".to_owned()),
        (rels[2].0, "people/Shady".to_owned()),
    ];
    assert_eq!(
        EntitySpec::from_document_create(&doc, texts)
            .render(doc.id, None, None)
            .unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: document\ndoc-type: contract\ncopy: certified copy\n\
         expires: 2027-03-31\npeople: [\"[[people/Shady]]\"]\ncompanies: [\"[[Watanya]]\"]\n\
         copy-of: [\"[[Watanya contract]]\"]\n---\n## Notes\n"
    );

    let place = PlaceCreate {
        id: id(ID),
        name: "Safe — Nasr City office".into(),
        aliases: vec!["الخزنة".into()],
        parent_id: Some(original),
        address: Some(String::new()),
        force: false,
    };
    assert_eq!(
        place_fields(&place),
        BTreeMap::from([("address".to_owned(), String::new())])
    );
    assert_eq!(
        EntitySpec::from_place_create(&place, Some("Nasr City office".into()))
            .render(place.id, None, None)
            .unwrap(),
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: place\naliases: [الخزنة]\naddress: \"\"\n\
         part-of: [\"[[Nasr City office]]\"]\n---\n## Notes\n"
    );
}

#[test]
fn concept_and_ai_skeletons() {
    assert_eq!(
        concept_body(Some("  Loyalty\n\n  programs  ")),
        "## Summary\nLoyalty programs\n"
    );
    assert_eq!(concept_body(Some("## Heading")), "## Summary\nHeading\n");
    assert_eq!(concept_body(Some(" \n ")), "## Summary\n");
    assert_eq!(concept_body(None), "## Summary\n");
    let doc = skeleton(
        NoteKind::Concept,
        " Loyalty ",
        "Loyalty 2",
        Vec::new(),
        &concept_body(Some("Why customers return.")),
    )
    .unwrap();
    assert_eq!(
        doc.render(),
        "---\nkind: concept\ntitle: Loyalty\n---\n## Summary\nWhy customers return.\n"
    );
    let doc = skeleton(
        NoteKind::Person,
        "Ahmed",
        "Ahmed",
        vec!["أحمد".into()],
        ENTITY_BODY,
    )
    .unwrap();
    assert_eq!(
        doc.render(),
        "---\nkind: person\naliases: [أحمد]\n---\n## Notes\n"
    );
}

#[test]
fn paths() {
    let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:05+03:00").unwrap();
    assert_eq!(capture_path(&created, []), "inbox/2026-09-27-143205.md");
    assert_eq!(
        capture_path(
            &created,
            [
                "inbox/2026-09-27-143205.md",
                "inbox/2026-09-27-143205 2.md",
                "notes/2026-09-27-143205 3.md"
            ]
        ),
        "inbox/2026-09-27-143205 3.md"
    );
    assert_eq!(
        entity_path(NoteKind::Person, " أحمد سمير ", ["people/أحمد سمير.md"]),
        "people/أحمد سمير 2.md"
    );
    assert_eq!(
        entity_path(NoteKind::Document, "عقد وطنية: نسخة", []),
        "documents/عقد وطنية - نسخة.md"
    );
    assert_eq!(
        entity_path(NoteKind::Place, "***", []),
        "places/Untitled.md"
    );
    assert_eq!(
        entity_path(NoteKind::Concept, "CON", []),
        "concepts/CON_.md"
    );
    let at = NaiveDate::from_ymd_opt(2026, 9, 27)
        .unwrap()
        .and_hms_opt(9, 5, 7)
        .unwrap();
    assert_eq!(
        conflict_copy_path("notes/Pricing.md", at, 1),
        "notes/Pricing (conflict 2026-09-27 090507).md"
    );
    assert_eq!(
        conflict_copy_path("notes/Pricing.md", at, 3),
        "notes/Pricing (conflict 2026-09-27 090507 3).md"
    );
}
