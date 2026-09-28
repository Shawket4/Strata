//! The local write path's refusals and prompts (PLAN §12.3): every invalid vault path is
//! refused with its stable reason code before anything is queued, frontmatter the shared
//! rules cannot edit is refused, and the "Already exists" prompts the server answers for
//! documents, places and tasks name what was being created.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::session::NewTask;
use strata_core::store::outbox;
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::OpResult;
use strata_core::view::build;
use strata_core::view::model::{DocumentDraft, PlaceDraft};

fn invalid(field: &str, reason: &str) -> CoreError {
    CoreError::InvalidInput {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

#[tokio::test]
async fn invalid_note_paths_are_refused_with_their_reason_code() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let long = format!("notes/{}.md", "a".repeat(300));
    for (path, reason) in [
        ("/notes/Pricing.md", "absolute"),
        ("notes//Pricing.md", "empty_segment"),
        ("notes/Pricing: Q4.md", "forbidden_char"),
        ("notes/Pri\u{7}cing.md", "control_char"),
        ("notes/.Pricing.md", "leading_dot"),
        ("notes./Pricing.md", "edge_whitespace_or_dot"),
        ("notes/CON.md", "reserved_name"),
        (long.as_str(), "too_long"),
        ("notes/Pricing.txt", "not_markdown"),
    ] {
        assert_eq!(
            s.create_note(path, "Q4.\n", true),
            Err(invalid("path", reason)),
            "{path:?}"
        );
    }
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), []);
}

#[tokio::test]
async fn frontmatter_the_rules_cannot_edit_is_refused() {
    const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3C";
    const OTHER: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3D";
    let content = format!("---\nid: {NOTE}\ntags: [unclosed\n---\nBody.\n");
    let h = Harness::new();
    h.server.remote_upsert(NOTE, "notes/Pricing.md", &content);
    h.server.remote_upsert(
        OTHER,
        "notes/Churn.md",
        &format!("---\nid: {OTHER}\n---\nChurn.\n"),
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let expected = vault_format::Document::parse(&content)
        .frontmatter()
        .and_then(|f| f.error().cloned())
        .expect("broken frontmatter")
        .to_string();
    assert_eq!(
        s.add_relation(NOTE, OTHER, "related"),
        Err(invalid("frontmatter", &expected))
    );
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), []);
}

#[tokio::test]
async fn already_exists_prompts_name_documents_places_and_tasks() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let doc = s
        .create_document(
            &DocumentDraft {
                name: "Car license".to_owned(),
                aliases: Vec::new(),
                doc_type: None,
                copy: None,
                copy_of: None,
                companies: Vec::new(),
                people: Vec::new(),
                expires: None,
            },
            false,
        )
        .expect("document")
        .id
        .expect("created");
    let place = s
        .create_place(
            &PlaceDraft {
                name: "Desk drawer".to_owned(),
                aliases: Vec::new(),
                parent_id: None,
                address: None,
            },
            false,
        )
        .expect("place")
        .id
        .expect("created");
    let task = s
        .create_task(
            &NewTask {
                note_id: None,
                description: "Renew the car license".to_owned(),
                due: None,
                scheduled: None,
                recurrence: None,
                reminders: Vec::new(),
                priority: None,
            },
            false,
        )
        .expect("task")
        .id
        .expect("created");
    for id in [&doc, &place, &task] {
        h.server.script(
            id,
            OpResult::Duplicate {
                candidates: Vec::new(),
            },
        );
    }
    s.sync(Trigger::AfterWrite).await.expect("push");

    let prompts = s.read(build::duplicate_prompts).expect("prompts").prompts;
    assert_eq!(
        prompts
            .iter()
            .map(|p| (p.kind.as_str(), p.title.as_str(), p.candidates.len()))
            .collect::<Vec<_>>(),
        [
            ("document", "Car license", 0),
            ("place", "Desk drawer", 0),
            ("task", "Renew the car license", 0),
        ]
    );
}
