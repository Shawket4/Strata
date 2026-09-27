//! Entities, aliases (exact + trigram), mentions, nested places, documents, custody, hints.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use chrono::{Duration, NaiveDate};
use pretty_assertions::assert_eq;
use strata_common::{Clock, CustodyEventId, DecisionId, HintId, NoteId, UserId};
use strata_index::ScopedTx;
use strata_index::repo::entities::{
    self, AliasMatch, CustodyEvent, Document, Entity, Hint, Mention,
};
use strata_index::repo::notes::{self, Note};
use strata_index::types::{By, CustodyType, DocCopy, DocStatus, EntityKind, NoteKind};
use strata_testkit::{TestDb, TestUser};

async fn entity(db: &TestDb, tx: &mut ScopedTx, kind: EntityKind, name: &str) -> NoteId {
    let t = db.clock.now();
    let id = NoteId::generate(db.ids.as_ref());
    let note_kind = match kind {
        EntityKind::Person => NoteKind::Person,
        EntityKind::Company => NoteKind::Company,
        EntityKind::Document => NoteKind::Document,
        EntityKind::Place => NoteKind::Place,
    };
    notes::upsert_note(
        tx,
        &Note {
            id,
            path: format!("{kind}/{name}.md"),
            title: name.into(),
            kind: note_kind,
            lang: None,
            created: t,
            updated: t,
            content_hash: "h".into(),
            word_count: 0,
            trashed: false,
        },
    )
    .await
    .expect("note");
    entities::upsert_entity(
        tx,
        &Entity {
            note_id: id,
            kind,
            display_name: name.into(),
            role: None,
            industry: None,
        },
    )
    .await
    .expect("entity");
    id
}

async fn user(db: &TestDb, name: &str) -> UserId {
    TestUser::new(name).create(db).await.expect("user").id
}

#[tokio::test]
async fn entity_crud_and_listing() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let samir = entity(&db, &mut tx, EntityKind::Person, "Ahmed Samir").await;
    let fathy = entity(&db, &mut tx, EntityKind::Person, "Ahmed Fathy").await;
    let acme = entity(&db, &mut tx, EntityKind::Company, "Acme").await;
    let updated = Entity {
        note_id: samir,
        kind: EntityKind::Person,
        display_name: "Ahmed Samir".into(),
        role: Some("Ops manager".into()),
        industry: None,
    };
    entities::upsert_entity(&mut tx, &updated)
        .await
        .expect("update");
    assert_eq!(
        entities::get_entity(&mut tx, samir).await.expect("get"),
        Some(updated)
    );
    let people: Vec<NoteId> = entities::list_entities(&mut tx, EntityKind::Person)
        .await
        .expect("list")
        .into_iter()
        .map(|e| e.note_id)
        .collect();
    assert_eq!(people, vec![fathy, samir]);
    assert_eq!(
        entities::list_entities(&mut tx, EntityKind::Company)
            .await
            .expect("list")
            .len(),
        1
    );
    assert_eq!(
        entities::get_entity(&mut tx, NoteId::generate(db.ids.as_ref()))
            .await
            .expect("missing"),
        None
    );
    let _ = acme;
}

#[tokio::test]
async fn alias_lookup_exact_and_fuzzy_across_scripts() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let samir = entity(&db, &mut tx, EntityKind::Person, "Ahmed Samir").await;
    let watanya = entity(&db, &mut tx, EntityKind::Company, "Watanya").await;
    entities::replace_entity_aliases(
        &mut tx,
        samir,
        &[
            ("أحمد سمير".into(), "احمد سمير".into()),
            ("Ahmed S.".into(), "ahmed s".into()),
        ],
    )
    .await
    .expect("aliases");
    entities::replace_entity_aliases(
        &mut tx,
        watanya,
        &[
            ("وطنية".into(), "وطنيه".into()),
            ("Watanya".into(), "watanya".into()),
        ],
    )
    .await
    .expect("aliases");

    assert_eq!(
        entities::entities_by_alias(&mut tx, "احمد سمير")
            .await
            .expect("exact"),
        vec![AliasMatch {
            entity_id: samir,
            alias: "أحمد سمير".into(),
            score: 1.0
        }]
    );
    assert_eq!(
        entities::entities_by_alias(&mut tx, "nobody")
            .await
            .expect("none"),
        vec![]
    );

    let fuzzy = entities::entities_by_alias_similarity(&mut tx, "watanyah", 0.4, 5)
        .await
        .expect("fuzzy");
    assert_eq!(
        fuzzy
            .iter()
            .map(|m| (m.entity_id, m.alias.as_str()))
            .collect::<Vec<_>>(),
        vec![(watanya, "Watanya")]
    );
    assert!(
        fuzzy[0].score > 0.5 && fuzzy[0].score < 1.0,
        "{}",
        fuzzy[0].score
    );
    assert_eq!(
        entities::entities_by_alias_similarity(&mut tx, "zzzz", 0.3, 5)
            .await
            .expect("none"),
        vec![]
    );
    let err = entities::entities_by_alias_similarity(&mut tx, "x", 1.5, 5)
        .await
        .expect_err("range");
    assert_eq!(
        err.to_string(),
        "invalid argument: trigram threshold 1.5 outside 0..=1"
    );

    // Replacing aliases drops the old ones.
    entities::replace_entity_aliases(&mut tx, samir, &[("Samir".into(), "samir".into())])
        .await
        .expect("replace");
    assert_eq!(
        entities::entities_by_alias(&mut tx, "ahmed s")
            .await
            .expect("gone"),
        vec![]
    );
    tx.commit().await.expect("commit");

    // Another user's aliases never match.
    let b = user(&db, "bob").await;
    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(
        entities::entities_by_alias(&mut tx, "samir")
            .await
            .expect("foreign"),
        vec![]
    );
    assert_eq!(
        entities::entities_by_alias_similarity(&mut tx, "watanya", 0.3, 5)
            .await
            .expect("foreign"),
        vec![]
    );
}

#[tokio::test]
async fn mentions_keep_first_seen_and_advance_last_seen() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let samir = entity(&db, &mut tx, EntityKind::Person, "Samir").await;
    let n = entity(&db, &mut tx, EntityKind::Company, "Call").await; // any note will do
    entities::record_mention(&mut tx, samir, n, "", t0 + Duration::days(1))
        .await
        .expect("m1");
    entities::record_mention(&mut tx, samir, n, "", t0)
        .await
        .expect("earlier");
    entities::record_mention(&mut tx, samir, n, "", t0 + Duration::days(3))
        .await
        .expect("later");
    entities::record_mention(&mut tx, samir, n, "a1b2", t0)
        .await
        .expect("block");
    assert_eq!(
        entities::mentions_of(&mut tx, samir)
            .await
            .expect("mentions"),
        vec![
            Mention {
                entity_id: samir,
                note_id: n,
                block_id: String::new(),
                first_seen: t0,
                last_seen: t0 + Duration::days(3)
            },
            Mention {
                entity_id: samir,
                note_id: n,
                block_id: "a1b2".into(),
                first_seen: t0,
                last_seen: t0
            },
        ]
    );
}

#[tokio::test]
async fn nested_places_documents_and_custody() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let office = entity(&db, &mut tx, EntityKind::Place, "Nasr City office").await;
    let safe = entity(&db, &mut tx, EntityKind::Place, "Safe — Nasr City office").await;
    let drawer = entity(&db, &mut tx, EntityKind::Place, "Drawer").await;
    let home = entity(&db, &mut tx, EntityKind::Place, "Home").await;
    entities::upsert_place(&mut tx, office, None)
        .await
        .expect("office");
    entities::upsert_place(&mut tx, safe, Some(office))
        .await
        .expect("safe");
    entities::upsert_place(&mut tx, drawer, Some(safe))
        .await
        .expect("drawer");
    entities::upsert_place(&mut tx, home, None)
        .await
        .expect("home");
    let mut expected = vec![office, safe, drawer];
    expected.sort();
    assert_eq!(
        entities::place_subtree(&mut tx, office)
            .await
            .expect("subtree"),
        expected
    );
    assert_eq!(
        entities::place_subtree(&mut tx, drawer)
            .await
            .expect("leaf"),
        vec![drawer]
    );
    assert_eq!(
        entities::place_parent(&mut tx, safe).await.expect("parent"),
        Some(Some(office))
    );
    assert_eq!(
        entities::place_parent(&mut tx, NoteId::generate(db.ids.as_ref()))
            .await
            .expect("unknown"),
        None
    );

    let shady = entity(&db, &mut tx, EntityKind::Person, "Shady").await;
    let contract = entity(&db, &mut tx, EntityKind::Document, "Watanya contract").await;
    let copy = entity(
        &db,
        &mut tx,
        EntityKind::Document,
        "Watanya contract (copy)",
    )
    .await;
    let id_card = entity(&db, &mut tx, EntityKind::Document, "ID").await;
    let doc = Document {
        note_id: contract,
        doc_type: "contract".into(),
        copy: DocCopy::Original,
        copy_of: None,
        location_id: Some(safe),
        holder_id: None,
        last_holder_id: Some(shady),
        status: DocStatus::Stored,
        expires: Some(NaiveDate::from_ymd_opt(2027, 3, 31).expect("date")),
    };
    entities::upsert_document(&mut tx, &doc).await.expect("doc");
    entities::upsert_document(
        &mut tx,
        &Document {
            note_id: copy,
            copy: DocCopy::Copy,
            copy_of: Some(contract),
            location_id: Some(drawer),
            holder_id: Some(shady),
            status: DocStatus::CheckedOut,
            expires: None,
            ..doc.clone()
        },
    )
    .await
    .expect("copy");
    entities::upsert_document(
        &mut tx,
        &Document {
            note_id: id_card,
            doc_type: "id".into(),
            location_id: Some(home),
            expires: Some(NaiveDate::from_ymd_opt(2026, 10, 1).expect("date")),
            ..doc.clone()
        },
    )
    .await
    .expect("id");
    assert_eq!(
        entities::get_document(&mut tx, contract)
            .await
            .expect("get"),
        Some(doc.clone())
    );

    let mut in_office = vec![contract, copy];
    in_office.sort();
    assert_eq!(
        entities::documents_in_place(&mut tx, office)
            .await
            .expect("office"),
        in_office
    );
    assert_eq!(
        entities::documents_in_place(&mut tx, home)
            .await
            .expect("home"),
        vec![id_card]
    );
    assert_eq!(
        entities::documents_held_by(&mut tx, shady)
            .await
            .expect("held"),
        vec![copy]
    );
    let expiring: Vec<NoteId> = entities::documents_expiring_before(
        &mut tx,
        NaiveDate::from_ymd_opt(2027, 12, 31).expect("date"),
    )
    .await
    .expect("expiring")
    .into_iter()
    .map(|d| d.note_id)
    .collect();
    assert_eq!(expiring, vec![id_card, contract]);

    let older = CustodyEvent {
        id: CustodyEventId::generate(db.ids.as_ref()),
        document_id: contract,
        event_type: CustodyType::HandedTo,
        at: t0 - Duration::days(7),
        place_id: None,
        person_id: Some(shady),
        counterparty_id: None,
        by: By::User,
        confidence: None,
        source_note_id: None,
        source_block_id: None,
        created: t0,
    };
    let newer = CustodyEvent {
        id: CustodyEventId::generate(db.ids.as_ref()),
        event_type: CustodyType::ReturnedBy,
        at: t0 - Duration::days(1),
        place_id: Some(safe),
        by: By::Ai,
        confidence: Some(0.9),
        source_note_id: Some(shady),
        source_block_id: Some("c1d2".into()),
        ..older.clone()
    };
    entities::add_custody_event(&mut tx, &older)
        .await
        .expect("older");
    entities::add_custody_event(&mut tx, &newer)
        .await
        .expect("newer");
    assert_eq!(
        entities::custody_history(&mut tx, contract)
            .await
            .expect("history"),
        vec![newer, older]
    );

    // Removing a place unsets (not deletes) the documents located there, and re-parents nothing.
    notes::delete_note(&mut tx, drawer)
        .await
        .expect("delete drawer");
    let moved = entities::get_document(&mut tx, copy)
        .await
        .expect("get")
        .expect("copy survives");
    assert_eq!(moved.location_id, None);
}

#[tokio::test]
async fn place_cycles_do_not_hang_subtree_queries() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let p1 = entity(&db, &mut tx, EntityKind::Place, "p1").await;
    let p2 = entity(&db, &mut tx, EntityKind::Place, "p2").await;
    entities::upsert_place(&mut tx, p1, None).await.expect("p1");
    entities::upsert_place(&mut tx, p2, Some(p1))
        .await
        .expect("p2");
    entities::upsert_place(&mut tx, p1, Some(p2))
        .await
        .expect("cycle");
    let mut both = vec![p1, p2];
    both.sort();
    assert_eq!(
        entities::place_subtree(&mut tx, p1).await.expect("subtree"),
        both
    );
}

#[tokio::test]
async fn disambiguation_hints_by_entity() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let samir = entity(&db, &mut tx, EntityKind::Person, "Ahmed Samir").await;
    let fathy = entity(&db, &mut tx, EntityKind::Person, "Ahmed Fathy").await;
    let h1 = Hint {
        id: HintId::generate(db.ids.as_ref()),
        entity_id: samir,
        hint: "Ahmed at Acme = Ahmed Samir".into(),
        source_decision_id: Some(DecisionId::generate(db.ids.as_ref())),
        created: t0,
    };
    let h2 = Hint {
        id: HintId::generate(db.ids.as_ref()),
        entity_id: fathy,
        hint: "Ahmed at Petrol Arrows = Ahmed Fathy".into(),
        source_decision_id: None,
        created: t0 + Duration::seconds(1),
    };
    entities::add_hint(&mut tx, &h1).await.expect("h1");
    entities::add_hint(&mut tx, &h2).await.expect("h2");
    assert_eq!(
        entities::hints_for(&mut tx, &[samir, fathy])
            .await
            .expect("both"),
        vec![h1.clone(), h2]
    );
    assert_eq!(
        entities::hints_for(&mut tx, &[samir]).await.expect("one"),
        vec![h1]
    );
}
