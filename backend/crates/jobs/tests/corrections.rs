//! Corrections in words (PLAN §9.8, §16.3): a capture the filing call flags as a correction
//! queues the `correct` job; the model gets the recent decisions and candidate entities,
//! picks the decision, and a confident unambiguous fix is applied in one `ai: correct`
//! commit — the wrong link removed and recorded as rejected, the right one added, hints
//! stored — and the hint is shown to the next resolution, which then links the right person.
//! An ambiguous correction becomes a suggestion with the model's question.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::float_cmp
)]

mod common;
mod pipeline_support;

use std::collections::BTreeMap;

use common::World;
use domain::NoteKind;
use pipeline_support::{
    CREATED, assert_input, block, decisions, enqueue, ent, push, runner, suggestions,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_common::NoteId;
use strata_index::UserScope;
use strata_jobs::correct::{CorrectionInput, DecisionInput, MessageInput};
use strata_jobs::file_inbox::{FilingInput, FilingNote};
use strata_jobs::link::{LinkInput, LinkNote};
use strata_jobs::pipeline::generated_block_id;
use strata_vault::ops::entities::NewEntity;

async fn entity(w: &World, s: &UserScope, kind: NoteKind, name: &str, aliases: &[&str]) -> NoteId {
    w.vault
        .create_entity(
            s,
            NewEntity {
                created: strata_common::clock::default_test_epoch(),
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

fn ahmed_link(target: NoteId, text: &str) -> serde_json::Value {
    json!({
        "relations": [], "concepts": [], "entity_relations": [], "custody": [], "tasks": [],
        "mentions": [{"text": "Ahmed", "kind": "person", "existing_id": target.to_string(),
                      "candidate_ids": [], "is_nickname": false, "confidence": 0.9,
                      "evidence_block_id": generated_block_id(text)}]
    })
}

const CORRECTION: &str = "the Ahmed in yesterday's Acme call is Ahmed Fathy";

#[tokio::test]
async fn a_correction_in_words_repoints_the_decision_and_its_hint_changes_the_next_resolution() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let samir = entity(&w, &sa, NoteKind::Person, "Ahmed Samir", &["Ahmed"]).await;
    let fathy = entity(&w, &sa, NoteKind::Person, "Ahmed Fathy", &["Ahmed"]).await;
    let acme = entity(&w, &sa, NoteKind::Company, "Acme", &[]).await;
    let people = |hints_fathy: &[&str]| {
        vec![
            ent(acme, "company", "Acme", &[], &[]),
            ent(fathy, "person", "Ahmed Fathy", &["Ahmed"], hints_fathy),
            ent(samir, "person", "Ahmed Samir", &["Ahmed"], &[]),
        ]
    };
    // 1. The AI links "Ahmed" in the Acme call to Ahmed Samir.
    let text = "Call with Ahmed from Acme.";
    let call = w
        .create(&sa, "notes/Acme call.md", &format!("{text}\n"))
        .await;
    let mut i = link_input(call, "Acme call", text);
    i.entities = people(&[]);
    push(&w, ids::LINKING, &i, ahmed_link(samir, text));
    enqueue(&w, a, "link", call).await;
    let r = runner(&w, &["link", "file_inbox", "correct"]);
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i);
    assert!(
        w.read(a, "notes/Acme call.md")
            .contains("people: [\"[[Ahmed Samir]]\"]")
    );
    assert_eq!(decisions(&w, a).await.len(), 1);

    // 2. The user captures the correction; filing flags it and queues the correction job.
    let capture = w
        .vault
        .capture(
            &sa,
            CORRECTION.into(),
            strata_common::clock::default_test_epoch(),
        )
        .await
        .expect("capture")
        .note
        .id;
    let filing = FilingInput {
        note: FilingNote {
            id: capture.to_string(),
            created: CREATED.into(),
            blocks: vec![block(CORRECTION)],
        },
        folders: vec!["notes".into()],
        top_tags: vec![],
        candidates: vec![pipeline_support::cand(call, "Acme call")],
        concepts: vec![],
        entities: people(&[]),
        rejected: vec![],
    };
    push(
        &w,
        ids::INBOX_FILING,
        &filing,
        json!({"title": "Ahmed correction", "tags": [], "destination_folder": "notes",
               "lang": "en", "is_correction": true, "relations": [], "concepts": [],
               "people": [], "companies": [], "custody": [], "tasks": []}),
    );
    let mut tx = w.db.begin(a).await.expect("tx");
    let d1: strata_common::DecisionId = sqlx::query_scalar("SELECT id FROM ai_decisions")
        .fetch_one(tx.conn())
        .await
        .expect("id");
    tx.commit().await.expect("commit");
    // The correction prompt sees the filing decision (made just before) and the link.
    let correction_input = |filing_decision: strata_common::DecisionId| CorrectionInput {
        message: MessageInput {
            text: CORRECTION.into(),
            created: CREATED.into(),
        },
        thread: None,
        decisions: vec![
            DecisionInput {
                id: filing_decision.to_string(),
                kind: "filing".into(),
                source_note_id: Some(capture.to_string()),
                source_title: Some("2026-09-27-120000".into()),
                source_created: Some("2026-09-27T12:00:00+00:00".into()),
                mention: None,
                target_id: Some("notes/Ahmed correction.md".into()),
                target_name: None,
                rel_type: None,
                created: "2026-09-27T12:00:00+00:00".into(),
            },
            DecisionInput {
                id: d1.to_string(),
                kind: "entity_link".into(),
                source_note_id: Some(call.to_string()),
                source_title: Some("Acme call".into()),
                source_created: Some("2026-09-27T12:00:00+00:00".into()),
                mention: Some("Ahmed".into()),
                target_id: Some(samir.to_string()),
                target_name: Some("Ahmed Samir".into()),
                rel_type: Some("people".into()),
                created: "2026-09-27T12:00:00+00:00".into(),
            },
        ],
        candidates: people(&[]),
    };
    // The filing decision's ID is known only after filing runs; run filing first.
    let r_file = runner(&w, &["file_inbox"]);
    r_file.run_until_idle().await;
    assert_input(&w, ids::INBOX_FILING, &filing);
    let rows: Vec<(strata_common::DecisionId, String)> = {
        let mut tx = w.db.begin(a).await.expect("tx");
        let rows = sqlx::query_as("SELECT id, kind FROM ai_decisions ORDER BY created, id")
            .fetch_all(tx.conn())
            .await
            .expect("ids");
        tx.commit().await.expect("commit");
        rows
    };
    let filing_decision = rows.iter().find(|r| r.1 == "filing").expect("filing").0;
    let ci = correction_input(filing_decision);
    push(
        &w,
        ids::CORRECTION,
        &ci,
        json!({
            "is_correction": true,
            "fixes": [{"decision_id": d1.to_string(), "action": "repoint",
                       "new_target_id": fathy.to_string(), "new_type": null,
                       "confidence": 0.9, "reason": "The user says it was Ahmed Fathy."}],
            "hints": [{"entity_id": fathy.to_string(), "text": "Ahmed at Acme = Ahmed Fathy"}],
            "ambiguous": false, "clarification_question": null
        }),
    );
    r.run_until_idle().await;
    assert_input(&w, ids::CORRECTION, &ci);
    // Repointed in one `ai: correct` commit: Samir removed and rejected, Fathy linked.
    assert_eq!(w.log(a)[0], "ai: correct inbox/2026-09-27-120000.md");
    let call_text = w.read(a, "notes/Acme call.md");
    assert!(
        call_text.contains("people: [\"[[Ahmed Fathy]]\"]"),
        "{call_text}"
    );
    let sc = pipeline_support::sidecar(&w, a, call);
    assert_eq!(
        (
            sc["rejected"][0]["type"].clone(),
            sc["rejected"][0]["target_id"].clone()
        ),
        (json!("people"), json!(samir.to_string()))
    );
    let d = decisions(&w, a).await;
    assert_eq!(
        d.iter()
            .map(|x| (x.kind.clone(), x.reverted, x.committed))
            .collect::<Vec<_>>(),
        vec![
            ("entity_mention".to_owned(), true, true),
            ("filing".to_owned(), false, false),
            ("correction".to_owned(), false, true),
        ]
    );
    let hints = pipeline_support::sidecar(&w, a, fathy)["hints"]
        .as_array()
        .expect("hints")
        .iter()
        .map(|h| h["text"].as_str().expect("text").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        hints,
        vec![
            "Ahmed at Acme = Ahmed Fathy".to_owned(),
            "\"Ahmed\" in \"Acme call\" = Ahmed Fathy".to_owned()
        ]
    );

    // 3. The next Acme note: the hints are in the prompt, and the model links Fathy.
    let text2 = "Ahmed from Acme sent the invoice.";
    let next = w
        .create(&sa, "notes/Invoice.md", &format!("{text2}\n"))
        .await;
    let mut i2 = link_input(next, "Invoice", text2);
    i2.entities = people(&[
        "Ahmed at Acme = Ahmed Fathy",
        "\"Ahmed\" in \"Acme call\" = Ahmed Fathy",
    ]);
    i2.candidates = vec![
        pipeline_support::cand(call, "Acme call"),
        pipeline_support::cand(capture, "2026-09-27-120000"),
    ];
    push(&w, ids::LINKING, &i2, ahmed_link(fathy, text2));
    enqueue(&w, a, "link", next).await;
    r.run_until_idle().await;
    assert_input(&w, ids::LINKING, &i2);
    assert!(
        w.read(a, "notes/Invoice.md")
            .contains("people: [\"[[Ahmed Fathy]]\"]")
    );
    assert_eq!(
        suggestions(&w, a)
            .await
            .into_iter()
            .map(|s| s.0)
            .collect::<Vec<_>>(),
        vec!["filing".to_owned()]
    );
    w.finish().await;
}
