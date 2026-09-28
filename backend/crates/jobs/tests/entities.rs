//! Entity mentions in linking (PLAN §6.7, D13 = b, §9.8, §16.3): resolution across Arabic
//! and Latin aliases (and transliteration), entity-to-entity relations, ambiguous mentions as
//! suggestions, threaded replies re-proposing, nicknames as link-or-create suggestions whose
//! acceptance adds the alias so the next mention links automatically.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

mod common;
mod pipeline_support;

use std::collections::BTreeMap;

use common::World;
use domain::NoteKind;
use pipeline_support::{
    CREATED, Row, assert_input, at, block, decisions, enqueue, ent, jobs_of, push, runner,
    suggestion_ids, suggestions,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_common::NoteId;
use strata_index::UserScope;
use strata_index::types::SuggestionStatus;
use strata_jobs::correct::{
    CorrectionInput, DecisionInput, MessageInput, ThreadInput, ThreadReply, ThreadSuggestion,
};
use strata_jobs::link::{LinkInput, LinkNote};
use strata_vault::ops::entities::NewEntity;

pub async fn entity(
    w: &World,
    s: &UserScope,
    kind: NoteKind,
    name: &str,
    aliases: &[&str],
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
                parent: None,
                id: None,
                force: true,
            },
        )
        .await
        .expect("entity")
        .id
}

fn link_input(note: NoteId, title: &str, text: &str) -> LinkInput {
    LinkInput {
        note: LinkNote {
            id: note.to_string(),
            title: title.to_owned(),
            created: CREATED.to_owned(),
            blocks: vec![block(text)],
        },
        candidates: vec![],
        concepts: vec![],
        entities: vec![],
        rejected: vec![],
    }
}

fn mention(text: &str, kind: &str, existing: Option<NoteId>, cands: &[NoteId], nick: bool, conf: f64, block_text: &str) -> serde_json::Value {
    json!({
        "text": text, "kind": kind, "existing_id": existing.map(|e| e.to_string()),
        "candidate_ids": cands.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "is_nickname": nick, "confidence": conf,
        "evidence_block_id": strata_jobs::pipeline::generated_block_id(block_text)
    })
}

fn out(mentions: Vec<serde_json::Value>, entity_relations: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "relations": [], "concepts": [], "mentions": mentions,
        "entity_relations": entity_relations, "custody": [], "tasks": []
    })
}

#[tokio::test]
async fn mentions_resolve_across_arabic_and_latin_aliases_with_entity_relations() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let ahmed = entity(&w, &sa, NoteKind::Person, "Ahmed Samir", &["أحمد سمير"]).await;
    let acme = entity(&w, &sa, NoteKind::Company, "Acme Logistics", &["أكمي"]).await;
    let text = "كلمت أحمد سمير من أكمي عن الفواتير.";
    let note = w.create(&sa, "notes/Call.md", &format!("{text}\n")).await;
    let mut i = link_input(note, "Call", text);
    i.entities = vec![
        ent(acme, "company", "Acme Logistics", &["أكمي"], &[]),
        ent(ahmed, "person", "Ahmed Samir", &["أحمد سمير"], &[]),
    ];
    push(
        &w,
        ids::LINKING,
        &i,
        out(
            vec![
                mention("أحمد سمير", "person", Some(ahmed), &[], false, 0.95, text),
                mention("أكمي", "company", Some(acme), &[], false, 0.9, text),
            ],
            vec![json!({
                "from": "أحمد سمير", "type": "works-at", "to": "أكمي", "confidence": 0.8,
                "reason": "أحمد سمير من أكمي.",
                "evidence_block_id": strata_jobs::pipeline::generated_block_id(text)
            })],
        ),
    );
    enqueue(&w, a, "link", note).await;
    let r = runner(&w, &["link"]);
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i);
    assert_eq!(
        w.read(a, "notes/Call.md"),
        format!(
            "---\nid: {note}\ncreated: {CREATED}\nupdated: {CREATED}\npeople: [\"[[Ahmed Samir]]\"]\ncompanies: [\"[[Acme Logistics]]\"]\n---\n{text}\n"
        )
    );
    assert!(
        w.read(a, "people/Ahmed Samir.md")
            .contains("works-at: [\"[[Acme Logistics]]\"]")
    );
    assert_eq!(w.log(a)[0], "ai: link notes/Call.md");
    let mut paths = w.last_commit_paths(a);
    paths.sort();
    assert_eq!(
        paths,
        vec![
            format!(".meta/notes/{ahmed}.json"),
            format!(".meta/notes/{note}.json"),
            "notes/Call.md".to_owned(),
            "people/Ahmed Samir.md".to_owned(),
        ]
    );
    assert_eq!(
        decisions(&w, a)
            .await
            .into_iter()
            .map(|d| (d.kind, d.target, d.rel, d.mention, d.committed))
            .collect::<Vec<_>>(),
        vec![
            (
                "entity_mention".to_owned(),
                ahmed.to_string(),
                Some("people".to_owned()),
                Some("أحمد سمير".to_owned()),
                true
            ),
            (
                "entity_mention".to_owned(),
                acme.to_string(),
                Some("companies".to_owned()),
                Some("أكمي".to_owned()),
                true
            ),
            ("relation".to_owned(), acme.to_string(), Some("works-at".to_owned()), None, true),
        ]
    );
    // Both entities' insights are refreshed five minutes later.
    assert_eq!(
        jobs_of(&w, a, "entity_insights").await,
        vec![("queued".to_owned(), 0, at(300)), ("queued".to_owned(), 0, at(300))]
    );

    // A Latin spelling of the Arabic-only alias still offers the entity (transliteration).
    let text2 = "Called Ahmad Sameer about invoices.";
    let note2 = w.create(&sa, "notes/Call 2.md", &format!("{text2}\n")).await;
    let mut i2 = link_input(note2, "Call 2", text2);
    i2.entities = vec![ent(ahmed, "person", "Ahmed Samir", &["أحمد سمير"], &[])];
    i2.candidates = vec![pipeline_support::cand(note, "Call")];
    push(
        &w,
        ids::LINKING,
        &i2,
        out(vec![mention("Ahmad Sameer", "person", Some(ahmed), &[], false, 0.9, text2)], vec![]),
    );
    enqueue(&w, a, "link", note2).await;
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i2);
    assert!(w.read(a, "notes/Call 2.md").contains("people: [\"[[Ahmed Samir]]\"]"));
    w.finish().await;
}

#[tokio::test]
async fn an_ambiguous_mention_is_a_suggestion_and_a_reply_re_proposes() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let samir = entity(&w, &sa, NoteKind::Person, "Ahmed Samir", &["Ahmed"]).await;
    let fathy = entity(&w, &sa, NoteKind::Person, "Ahmed Fathy", &["Ahmed"]).await;
    let petrol = entity(&w, &sa, NoteKind::Company, "Petrol Arrows", &[]).await;
    let text = "Ahmed called about the invoice.";
    let note = w.create(&sa, "notes/Invoice call.md", &format!("{text}\n")).await;
    let mut i = link_input(note, "Invoice call", text);
    i.entities = vec![
        ent(fathy, "person", "Ahmed Fathy", &["Ahmed"], &[]),
        ent(samir, "person", "Ahmed Samir", &["Ahmed"], &[]),
    ];
    push(
        &w,
        ids::LINKING,
        &i,
        out(
            vec![mention("Ahmed", "person", None, &[samir, fathy], false, 0.5, text)],
            vec![],
        ),
    );
    enqueue(&w, a, "link", note).await;
    let r = runner(&w, &["link", "suggestion_reply"]);
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i);
    assert!(!w.read(a, "notes/Invoice call.md").contains("people"));
    let d = decisions(&w, a).await;
    assert_eq!(
        d,
        vec![Row {
            kind: "entity_mention".into(),
            source: Some(note.to_string()),
            target: String::new(),
            summary: "\"Ahmed\": link or create (ambiguous)".into(),
            rel: Some("people".into()),
            mention: Some("Ahmed".into()),
            suggested: true,
            committed: false,
            reverted: false,
        }]
    );
    let s = suggestions(&w, a).await;
    let decision_id = s[0].2["decision_id"].clone();
    assert_eq!(
        s,
        vec![(
            "entity_link".to_owned(),
            "pending".to_owned(),
            json!({
                "decision_id": decision_id, "mention": "Ahmed", "kind": "person",
                "source_note": note.to_string(),
                "block_id": strata_jobs::pipeline::generated_block_id(text),
                "proposed": null, "candidates": [samir.to_string(), fathy.to_string()],
                "is_nickname": false, "confidence": 0.5, "reason": "ambiguous"
            })
        )]
    );

    // "no, the Petrol Arrows one": the AI re-proposes with the reply in context.
    let sid = suggestion_ids(&w, a).await[0];
    w.vault
        .reply_suggestion(&sa, sid, "no, the Petrol Arrows one".into())
        .await
        .expect("reply");
    let reply_input = CorrectionInput {
        message: MessageInput {
            text: "no, the Petrol Arrows one".into(),
            created: "2026-09-27T12:00:00+00:00".into(),
        },
        thread: Some(ThreadInput {
            suggestion: ThreadSuggestion {
                id: sid.to_string(),
                kind: "entity_link".into(),
                summary: "\"Ahmed\": link or create (ambiguous)".into(),
                decision_id: Some(decision_id.as_str().expect("id").to_owned()),
            },
            replies: vec![ThreadReply {
                author: "user".into(),
                text: "no, the Petrol Arrows one".into(),
            }],
        }),
        decisions: vec![DecisionInput {
            id: decision_id.as_str().expect("id").to_owned(),
            kind: "entity_link".into(),
            source_note_id: Some(note.to_string()),
            source_title: Some("Invoice call".into()),
            source_created: Some("2026-09-27T12:00:00+00:00".into()),
            mention: Some("Ahmed".into()),
            target_id: None,
            target_name: None,
            rel_type: Some("people".into()),
            created: "2026-09-27T12:00:00+00:00".into(),
        }],
        candidates: vec![
            ent(petrol, "company", "Petrol Arrows", &[], &[]),
            ent(fathy, "person", "Ahmed Fathy", &["Ahmed"], &[]),
            ent(samir, "person", "Ahmed Samir", &["Ahmed"], &[]),
        ],
    };
    push(
        &w,
        ids::CORRECTION,
        &reply_input,
        json!({
            "is_correction": true,
            "fixes": [{"decision_id": decision_id, "action": "repoint",
                       "new_target_id": fathy.to_string(), "new_type": null,
                       "confidence": 0.9, "reason": "The Ahmed of Petrol Arrows is Ahmed Fathy."}],
            "hints": [{"entity_id": fathy.to_string(), "text": "Ahmed at Petrol Arrows = Ahmed Fathy"}],
            "ambiguous": false, "clarification_question": null
        }),
    );
    r.run_until_idle().await;
    assert_input(&w, ids::CORRECTION, &reply_input);
    let s = suggestions(&w, a).await;
    assert_eq!(
        s.iter().map(|x| (x.0.clone(), x.1.clone())).collect::<Vec<_>>(),
        vec![
            ("entity_link".to_owned(), "superseded".to_owned()),
            ("entity_link".to_owned(), "pending".to_owned())
        ]
    );
    assert_eq!(
        (s[1].2["proposed"].clone(), s[1].2["candidates"].clone(), s[1].2["reason"].clone()),
        (json!(fathy.to_string()), json!([fathy.to_string()]), json!("reply"))
    );
    let thread = w
        .vault
        .suggestions(&sa, SuggestionStatus::Superseded)
        .await
        .expect("list");
    assert_eq!(
        thread[0]
            .replies
            .iter()
            .map(|r| (r.author, r.body.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                strata_index::types::ReplyAuthor::User,
                "no, the Petrol Arrows one".to_owned()
            ),
            (
                strata_index::types::ReplyAuthor::Ai,
                "New suggestion: \"Ahmed\" → Ahmed Fathy.".to_owned()
            ),
        ]
    );
    // The hint is remembered on the entity (table and sidecar).
    let sc = pipeline_support::sidecar(&w, a, fathy);
    assert_eq!(sc["hints"][0]["text"], json!("Ahmed at Petrol Arrows = Ahmed Fathy"));
    // Accepting the re-proposal links the note.
    let new_sid = suggestion_ids(&w, a).await[1];
    w.vault
        .decide_suggestion(&sa, new_sid, true)
        .await
        .expect("accept");
    assert!(
        w.read(a, "notes/Invoice call.md")
            .contains("people: [\"[[Ahmed Fathy]]\"]")
    );
    assert_eq!(w.log(a)[0], "user: accept entity_link notes/Invoice call.md");
    w.finish().await;
}

#[tokio::test]
async fn a_nickname_is_a_link_or_create_suggestion_and_its_alias_links_the_next_mention() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let ibrahim = entity(&w, &sa, NoteKind::Person, "Ibrahim Nasr", &[]).await;
    let text = "بابا عنده عقد الشقة.";
    let note = w.create(&sa, "notes/Apartment.md", &format!("{text}\n")).await;
    let i = link_input(note, "Apartment", text);
    // Even a confident guess never links a nickname without an existing alias.
    push(
        &w,
        ids::LINKING,
        &i,
        out(vec![mention("بابا", "person", None, &[], true, 0.9, text)], vec![]),
    );
    enqueue(&w, a, "link", note).await;
    let r = runner(&w, &["link"]);
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i);
    assert!(!w.read(a, "notes/Apartment.md").contains("people"));
    let s = suggestions(&w, a).await;
    assert_eq!(
        (
            s[0].0.clone(),
            s[0].2["reason"].clone(),
            s[0].2["is_nickname"].clone(),
            s[0].2["proposed"].clone()
        ),
        ("entity_link".to_owned(), json!("nickname"), json!(true), json!(null))
    );
    // No entity was created.
    assert!(!w.dir(a).join("people/بابا.md").exists());

    // Accept: link to Ibrahim; "بابا" becomes his alias (with a Latin spelling given too).
    let sid = suggestion_ids(&w, a).await[0];
    w.vault
        .decide_suggestion_with(
            &sa,
            sid,
            true,
            Some(sync_model::ops::SuggestionEdits {
                target_id: Some(ibrahim.as_ulid()),
                aliases: Some(vec!["Baba".into()]),
                ..sync_model::ops::SuggestionEdits::default()
            }),
        )
        .await
        .expect("accept");
    assert!(
        w.read(a, "people/Ibrahim Nasr.md")
            .contains("aliases: [بابا, Baba]")
    );
    assert!(
        w.read(a, "notes/Apartment.md")
            .contains("people: [\"[[Ibrahim Nasr]]\"]")
    );
    assert_eq!(w.log(a)[0], "user: accept entity_link notes/Apartment.md");

    // The next mention resolves through the alias and links automatically.
    let text2 = "كلمت بابا النهارده.";
    let note2 = w.create(&sa, "notes/Call.md", &format!("{text2}\n")).await;
    let mut i2 = link_input(note2, "Call", text2);
    i2.entities = vec![ent(ibrahim, "person", "Ibrahim Nasr", &["Baba", "بابا"], &[])];
    i2.candidates = vec![pipeline_support::cand(note, "Apartment")];
    push(
        &w,
        ids::LINKING,
        &i2,
        out(vec![mention("بابا", "person", Some(ibrahim), &[], true, 0.9, text2)], vec![]),
    );
    enqueue(&w, a, "link", note2).await;
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i2);
    assert!(
        w.read(a, "notes/Call.md")
            .contains("people: [\"[[Ibrahim Nasr]]\"]")
    );
    assert_eq!(
        decisions(&w, a).await.last().map(|d| (d.kind.clone(), d.committed)),
        Some(("entity_mention".to_owned(), true))
    );
    w.finish().await;
}
