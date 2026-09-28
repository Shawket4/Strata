//! The `file_inbox` job (PLAN §9.3, §6.6, §6.11, §16.3): the exact prompt input; concepts
//! created with an AI-owned Summary and linked; task suggestions; with auto-file off a
//! `filing` suggestion whose acceptance (with edits) moves, titles and tags the capture in one
//! commit; with auto-file on the same in the job's one `ai:` commit.
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

use common::World;
use pipeline_support::{
    CREATED, WRITTEN, assert_input, block, decisions, push, runner, set_auto_file, suggestion_ids,
    suggestions,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_common::NoteId;
use strata_jobs::file_inbox::{FilingInput, FilingNote};
use strata_jobs::pipeline::generated_block_id;

const TEXT: &str = "Watanya ETA invoice is due on the 1st every month.";

fn filing_input(note: NoteId, clients: NoteId) -> FilingInput {
    FilingInput {
        note: FilingNote {
            id: note.to_string(),
            created: CREATED.into(),
            blocks: vec![block(TEXT)],
        },
        folders: vec!["notes".into(), "notes/Clients".into()],
        top_tags: vec!["client".into()],
        candidates: vec![pipeline_support::cand(clients, "Watanya")],
        concepts: vec![],
        entities: vec![],
        rejected: vec![],
    }
}

fn filing_output() -> serde_json::Value {
    json!({
        "title": "Watanya ETA invoice", "tags": ["watanya", "invoices"],
        "destination_folder": "notes/Clients", "lang": "en", "is_correction": false,
        "relations": [],
        "concepts": [{"name": "ETA e-invoicing", "existing_id": null, "confidence": 0.8,
                      "summary": "Egypt's electronic invoicing system for company invoices."}],
        "people": [], "companies": [], "custody": [],
        "tasks": [{"title": "Make Watanya's ETA invoice", "due": "2026-10-01",
                   "date_source": "explicit", "recurrence": "every month on the 1st",
                   "reminders": ["2026-10-01 09:00"], "entities": [], "confidence": 0.9,
                   "evidence_block_id": generated_block_id(TEXT)}]
    })
}

async fn setup(w: &World, s: &strata_index::UserScope) -> (NoteId, NoteId) {
    let clients = w
        .create(
            s,
            "notes/Clients/Watanya.md",
            "---\ntags: [client]\n---\nWatanya invoices monthly.\n",
        )
        .await;
    let capture = w
        .vault
        .capture(s, TEXT.into(), strata_common::clock::default_test_epoch())
        .await
        .expect("capture")
        .note
        .id;
    (clients, capture)
}

#[tokio::test]
async fn with_auto_file_off_filing_is_a_suggestion_and_accepting_files_in_one_commit() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let (clients, capture) = setup(&w, &sa).await;
    let i = filing_input(capture, clients);
    push(&w, ids::INBOX_FILING, &i, filing_output());
    runner(&w, &["file_inbox"]).run_until_idle().await;
    assert_input(&w, ids::INBOX_FILING, &i);
    let path = "inbox/2026-09-27-120000.md";
    assert_eq!(w.log(a)[0], format!("ai: file_inbox {path}"));
    // The concept note exists with only the AI-owned Summary; the capture links to it and
    // stays in the inbox.
    let concept = w.read(a, "concepts/ETA e-invoicing.md");
    let concept_id = concept
        .lines()
        .find_map(|l| l.strip_prefix("id: "))
        .expect("id")
        .to_owned();
    assert_eq!(
        concept,
        format!(
            "---\nid: {concept_id}\nkind: concept\ncreated: {WRITTEN}\nupdated: {WRITTEN}\n---\n## Summary\nEgypt's electronic invoicing system for company invoices.\n"
        )
    );
    assert_eq!(
        w.read(a, path),
        format!(
            "---\nid: {capture}\ncreated: {WRITTEN}\nconcepts: [\"[[ETA e-invoicing]]\"]\n---\n{TEXT}\n"
        )
    );
    let s = suggestions(&w, a).await;
    assert_eq!(
        s.iter()
            .map(|x| (x.0.clone(), x.1.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("task".to_owned(), "pending".to_owned()),
            ("filing".to_owned(), "pending".to_owned())
        ]
    );
    assert_eq!(
        s[0].2,
        json!({
            "decision_id": s[0].2["decision_id"], "source_note": capture.to_string(),
            "block_id": generated_block_id(TEXT), "title": "Make Watanya's ETA invoice",
            "due": "2026-10-01", "recurrence": "every month on the 1st",
            "reminders": ["2026-10-01T09:00:00"], "entities": [], "confidence": 0.9
        })
    );
    assert_eq!(
        s[1].2,
        json!({
            "decision_id": s[1].2["decision_id"], "title": "Watanya ETA invoice",
            "tags": ["watanya", "invoices"], "folder": "notes/Clients"
        })
    );
    assert_eq!(
        decisions(&w, a)
            .await
            .into_iter()
            .map(|d| (d.kind, d.target, d.suggested, d.committed))
            .collect::<Vec<_>>(),
        vec![
            ("concept".to_owned(), concept_id.clone(), false, true),
            (
                "task_suggestion".to_owned(),
                "Make Watanya's ETA invoice".to_owned(),
                true,
                false
            ),
            (
                "filing".to_owned(),
                "notes/Clients/Watanya ETA invoice.md".to_owned(),
                true,
                false
            ),
        ]
    );
    // Accept the filing with an edited title: move + title + tags in one `user:` commit.
    let ids_ = suggestion_ids(&w, a).await;
    w.vault
        .decide_suggestion_with(
            &sa,
            ids_[1],
            true,
            Some(sync_model::ops::SuggestionEdits {
                title: Some("Watanya ETA invoicing".into()),
                ..sync_model::ops::SuggestionEdits::default()
            }),
        )
        .await
        .expect("accept filing");
    assert_eq!(
        w.log(a)[0],
        format!("user: accept filing {path} -> notes/Clients/Watanya ETA invoicing.md")
    );
    assert!(!w.dir(a).join(path).exists());
    assert_eq!(
        w.read(a, "notes/Clients/Watanya ETA invoicing.md"),
        format!(
            "---\nid: {capture}\ntags: [watanya, invoices]\ncreated: {WRITTEN}\nconcepts: [\"[[ETA e-invoicing]]\"]\n---\n{TEXT}\n"
        )
    );
    // Accept the task: the line goes to tasks/Tasks.md.
    w.vault
        .decide_suggestion(&sa, ids_[0], true)
        .await
        .expect("accept task");
    let tasks = w.read(a, "tasks/Tasks.md");
    let line = tasks.lines().last().expect("line");
    assert_eq!(
        line.rsplit_once(" ^t-").map(|x| x.0),
        Some(
            "- [ ] Make Watanya's ETA invoice (@2026-10-01 09:00) 🔁 every month on the 1st 📅 2026-10-01"
        )
    );
    assert_eq!(w.log(a)[0], "user: task create tasks/Tasks.md");
    w.finish().await;
}

#[tokio::test]
async fn with_auto_file_on_the_capture_is_filed_in_the_jobs_commit() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    set_auto_file(&w, a, true).await;
    let (clients, capture) = setup(&w, &sa).await;
    let i = filing_input(capture, clients);
    push(&w, ids::INBOX_FILING, &i, filing_output());
    runner(&w, &["file_inbox"]).run_until_idle().await;
    assert_input(&w, ids::INBOX_FILING, &i);
    assert_eq!(
        w.log(a)[0],
        "ai: file_inbox inbox/2026-09-27-120000.md -> notes/Clients/Watanya ETA invoice.md"
    );
    assert!(!w.dir(a).join("inbox/2026-09-27-120000.md").exists());
    assert_eq!(
        w.read(a, "notes/Clients/Watanya ETA invoice.md"),
        format!(
            "---\nid: {capture}\ntags: [watanya, invoices]\ncreated: {WRITTEN}\nconcepts: [\"[[ETA e-invoicing]]\"]\n---\n{TEXT}\n"
        )
    );
    assert_eq!(
        suggestions(&w, a)
            .await
            .into_iter()
            .map(|s| s.0)
            .collect::<Vec<_>>(),
        vec!["task".to_owned()]
    );
    let d = decisions(&w, a).await;
    assert_eq!(
        (d[2].kind.clone(), d[2].committed, d[2].suggested),
        ("filing".to_owned(), true, false)
    );
    // Filed and linked at this version: the follow-up link job makes no call.
    let calls = w.llm.calls().len();
    pipeline_support::enqueue(&w, a, "link", capture).await;
    runner(&w, &["link"]).run_until_idle().await;
    assert_eq!(w.llm.calls().len(), calls);
    w.finish().await;
}
