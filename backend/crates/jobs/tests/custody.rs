//! Custody statements in linking (PLAN §6.12, D30, §16.3): the Watanya contract example in
//! English and Arabic applied automatically (cited custody lines, derived frontmatter, block
//! ID appended, one `ai:` commit); below the threshold, ambiguous ("gave the contract to
//! Shady" with two contracts) and conflicting statements become suggestions; accepting with
//! the chosen document records the event.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod common;
mod pipeline_support;

use std::collections::BTreeMap;

use common::World;
use domain::NoteKind;
use pipeline_support::{
    CREATED, assert_input, block, decisions, enqueue, ent, push, runner, suggestion_ids,
    suggestions,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_common::NoteId;
use strata_index::UserScope;
use strata_jobs::link::{LinkInput, LinkNote};
use strata_jobs::pipeline::{EntityInput, generated_block_id};
use strata_vault::ops::entities::NewEntity;

async fn entity(
    w: &World,
    s: &UserScope,
    kind: NoteKind,
    name: &str,
    aliases: &[&str],
    parent: Option<NoteId>,
) -> NoteId {
    w.vault
        .create_entity(
            s,
            NewEntity {
                kind,
                name: name.to_owned(),
                aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
                tags: vec![],
                fields: BTreeMap::new(),
                parent,
                id: None,
                force: true,
            },
        )
        .await
        .expect("entity")
        .id
}

struct Vault {
    doc: NoteId,
    office: NoteId,
    safe: NoteId,
    shady: NoteId,
}

async fn setup(w: &World, s: &UserScope) -> Vault {
    let doc = entity(w, s, NoteKind::Document, "Watanya contract", &["عقد وطنية"], None).await;
    let office = entity(w, s, NoteKind::Place, "Nasr City office", &["مكتب مدينة نصر"], None).await;
    let safe = entity(
        w,
        s,
        NoteKind::Place,
        "Safe — Nasr City office",
        &["الخزنة"],
        Some(office),
    )
    .await;
    let shady = entity(w, s, NoteKind::Person, "Shady", &["شادي"], None).await;
    Vault {
        doc,
        office,
        safe,
        shady,
    }
}

fn input(note: NoteId, title: &str, text: &str, entities: Vec<EntityInput>) -> LinkInput {
    LinkInput {
        note: LinkNote {
            id: note.to_string(),
            title: title.to_owned(),
            created: CREATED.to_owned(),
            blocks: vec![block(text)],
        },
        candidates: vec![],
        concepts: vec![],
        entities,
        rejected: vec![],
    }
}

#[allow(clippy::too_many_arguments)]
fn event(
    kind: &str,
    document: &str,
    doc: Option<NoteId>,
    place: Option<(&str, NoteId)>,
    part_of: Option<&str>,
    person: Option<(&str, NoteId)>,
    confidence: f64,
    text: &str,
    quote: &str,
) -> serde_json::Value {
    json!({
        "type": kind, "document": document, "document_existing_id": doc.map(|d| d.to_string()),
        "place": place.map(|p| p.0), "place_existing_id": place.map(|p| p.1.to_string()),
        "place_part_of": part_of,
        "person": person.map(|p| p.0), "person_existing_id": person.map(|p| p.1.to_string()),
        "counterparty": null, "date": "2026-09-27", "date_source": "created",
        "confidence": confidence, "evidence_block_id": generated_block_id(text), "quote": quote
    })
}

fn out(custody: Vec<serde_json::Value>, mentions: Vec<serde_json::Value>) -> serde_json::Value {
    json!({"relations": [], "concepts": [], "mentions": mentions, "entity_relations": [],
           "custody": custody, "tasks": []})
}

fn watanya_entities(v: &Vault, arabic_first: bool) -> Vec<EntityInput> {
    let _ = arabic_first;
    let mut safe = ent(v.safe, "place", "Safe — Nasr City office", &["الخزنة"], &[]);
    safe.part_of = Some(v.office.to_string());
    vec![
        ent(v.doc, "document", "Watanya contract", &["عقد وطنية"], &[]),
        ent(v.shady, "person", "Shady", &["شادي"], &[]),
        ent(v.office, "place", "Nasr City office", &["مكتب مدينة نصر"], &[]),
        safe,
    ]
}

async fn run_example(w: &World, a: strata_common::UserId, s: &UserScope, v: &Vault, text: &str, doc_mention: &str, safe_mention: &str, office_mention: &str, shady_mention: &str) -> NoteId {
    let note = w.create(s, "notes/Contract.md", &format!("{text}\n")).await;
    let i = input(note, "Contract", text, watanya_entities(v, false));
    push(
        w,
        ids::LINKING,
        &i,
        out(
            vec![
                event("returned-by", doc_mention, Some(v.doc), None, None, Some((shady_mention, v.shady)), 0.95, text, text),
                event("stored-at", doc_mention, Some(v.doc), Some((safe_mention, v.safe)), Some(office_mention), None, 0.95, text, text),
            ],
            vec![],
        ),
    );
    enqueue(w, a, "link", note).await;
    runner(w, &["link"]).run_until_idle().await;
    assert_input(w, ids::LINKING, &i);
    note
}

fn expected_document(doc: NoteId, cite: &str) -> String {
    format!(
        "---\nid: {doc}\nkind: document\naliases: [عقد وطنية]\ncreated: {CREATED}\nupdated: {CREATED}\nlocation: \"[[Safe — Nasr City office]]\"\nholder: \"\"\nlast-holder: \"[[Shady]]\"\nstatus: stored\n---\n## Custody\n- 2026-09-27 — stored-at [[Safe — Nasr City office]] — {cite}\n- 2026-09-27 — returned-by [[Shady]] — {cite}\n\n## Notes\n"
    )
}

#[tokio::test]
async fn the_watanya_example_in_english_is_applied_with_cited_events() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let v = setup(&w, &sa).await;
    let text = "Watanya's contract is at the Nasr City office in the safe, last with Shady.";
    let note = run_example(&w, a, &sa, &v, text, "Watanya's contract", "the safe", "the Nasr City office", "Shady").await;
    let b = generated_block_id(text);
    // The cited block got its ID (the one automated body edit); one `ai:` commit.
    assert_eq!(
        w.read(a, "notes/Contract.md"),
        format!("---\nid: {note}\ncreated: {CREATED}\nupdated: {CREATED}\n---\n{text} ^{b}\n")
    );
    assert_eq!(
        w.read(a, "documents/Watanya contract.md"),
        expected_document(v.doc, &format!("[[Contract#^{b}]]"))
    );
    assert_eq!(w.log(a)[0], "ai: link notes/Contract.md");
    assert_eq!(
        decisions(&w, a)
            .await
            .into_iter()
            .map(|d| (d.kind, d.target, d.rel, d.committed, d.suggested))
            .collect::<Vec<_>>(),
        vec![
            ("custody_event".to_owned(), v.doc.to_string(), Some("returned-by".to_owned()), true, false),
            ("custody_event".to_owned(), v.doc.to_string(), Some("stored-at".to_owned()), true, false),
        ]
    );
    assert_eq!(suggestions(&w, a).await, vec![]);
    w.finish().await;
}

#[tokio::test]
async fn the_watanya_example_in_arabic_is_applied_with_cited_events() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let v = setup(&w, &sa).await;
    let text = "عقد وطنية في الخزنة في مكتب مدينة نصر، وآخر مرة كان مع شادي.";
    run_example(&w, a, &sa, &v, text, "عقد وطنية", "الخزنة", "مكتب مدينة نصر", "شادي").await;
    let b = generated_block_id(text);
    assert_eq!(
        w.read(a, "documents/Watanya contract.md"),
        expected_document(v.doc, &format!("[[Contract#^{b}]]"))
    );
    w.finish().await;
}

#[tokio::test]
async fn ambiguous_low_confidence_and_conflicting_statements_are_suggestions() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let v = setup(&w, &sa).await;
    let acme = entity(&w, &sa, NoteKind::Document, "Acme contract", &[], None).await;
    // "gave the contract to Shady": two contracts fit.
    let text = "Gave the contract to Shady.";
    let note = w.create(&sa, "notes/Handover.md", &format!("{text}\n")).await;
    let i = input(
        note,
        "Handover",
        text,
        vec![ent(v.shady, "person", "Shady", &["شادي"], &[])],
    );
    push(
        &w,
        ids::LINKING,
        &i,
        out(
            vec![event("handed-to", "the contract", None, None, None, Some(("Shady", v.shady)), 0.95, text, text)],
            vec![json!({"text": "the contract", "kind": "document", "existing_id": null,
                        "candidate_ids": [v.doc.to_string(), acme.to_string()], "is_nickname": false,
                        "confidence": 0.5, "evidence_block_id": generated_block_id(text)})],
        ),
    );
    enqueue(&w, a, "link", note).await;
    let r = runner(&w, &["link"]);
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i);
    let before = w.read(a, "documents/Watanya contract.md");
    assert!(!before.contains("## Custody"));
    let s = suggestions(&w, a).await;
    let b = generated_block_id(text);
    assert_eq!(
        s,
        vec![(
            "custody".to_owned(),
            "pending".to_owned(),
            json!({
                "decision_id": s[0].2["decision_id"], "source_note": note.to_string(),
                "block_id": b, "event": "handed-to", "date": "2026-09-27",
                "document": {"mention": "the contract", "id": null,
                             "candidates": [v.doc.to_string(), acme.to_string()]},
                "place": null, "place_part_of": null,
                "person": {"mention": "Shady", "id": v.shady.to_string(), "candidates": []},
                "counterparty": null, "confidence": 0.95, "reason": "ambiguous",
                "quote": text
            })
        )]
    );
    // The block a suggestion cites is made citable already.
    assert_eq!(
        w.read(a, "notes/Handover.md"),
        format!("---\nid: {note}\ncreated: {CREATED}\nupdated: {CREATED}\n---\n{text} ^{b}\n")
    );
    // Accepting with the chosen document records the event (one `user:` commit).
    let sid = suggestion_ids(&w, a).await[0];
    w.vault
        .decide_suggestion_with(
            &sa,
            sid,
            true,
            Some(sync_model::ops::SuggestionEdits {
                target_id: Some(v.doc.as_ulid()),
                ..sync_model::ops::SuggestionEdits::default()
            }),
        )
        .await
        .expect("accept");
    assert_eq!(w.log(a)[0], "user: accept custody notes/Handover.md");
    assert_eq!(
        w.read(a, "documents/Watanya contract.md"),
        format!(
            "---\nid: {}\nkind: document\naliases: [عقد وطنية]\ncreated: {CREATED}\nupdated: {CREATED}\nlocation: \"\"\nholder: \"[[Shady]]\"\nlast-holder: \"[[Shady]]\"\nstatus: checked-out\n---\n## Custody\n- 2026-09-27 — handed-to [[Shady]] — [[Handover#^{b}]]\n\n## Notes\n",
            v.doc
        )
    );

    // Below the custody threshold (0.85): a suggestion even with every entity resolved.
    let text2 = "The Watanya contract is in the safe at the Nasr City office.";
    let note2 = w.create(&sa, "notes/Safe.md", &format!("{text2}\n")).await;
    let mut safe = ent(v.safe, "place", "Safe — Nasr City office", &["الخزنة"], &[]);
    safe.part_of = Some(v.office.to_string());
    let mut i2 = input(
        note2,
        "Safe",
        text2,
        vec![
            ent(v.doc, "document", "Watanya contract", &["عقد وطنية"], &[]),
            ent(v.office, "place", "Nasr City office", &["مكتب مدينة نصر"], &[]),
            safe,
        ],
    );
    i2.candidates = vec![pipeline_support::cand(note, "Handover")];
    push(
        &w,
        ids::LINKING,
        &i2,
        out(
            vec![event("stored-at", "The Watanya contract", Some(v.doc), Some(("the safe", v.safe)), None, None, 0.8, text2, text2)],
            vec![],
        ),
    );
    enqueue(&w, a, "link", note2).await;
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i2);
    let s = suggestions(&w, a).await;
    assert_eq!(
        (s[1].0.clone(), s[1].2["reason"].clone()),
        ("custody".to_owned(), json!("low_confidence"))
    );
    w.finish().await;
}
