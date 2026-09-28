//! The `entity_insights` job (PLAN §6.7, §16.3): the exact prompt input (mentioning notes
//! newest first, one-line captures flagged, descriptive properties only), uncited and
//! speculative bullets rejected, contact details dropped, the relative date "بكرة" resolved
//! against the note's `created`, user sections preserved byte for byte, contact fields
//! untouched, cited blocks made citable, one `ai:` commit, and no second call for unchanged
//! input.
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
use pipeline_support::{CREATED, assert_input, block, block_with, enqueue, push, runner};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_jobs::insights::{InsightEntity, InsightNote, InsightsInput};
use strata_jobs::pipeline::generated_block_id;
use strata_vault::ops::entities::NewEntity;

#[tokio::test]
async fn insights_are_cited_validated_dated_and_leave_user_content_alone() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let shady = w
        .vault
        .create_entity(
            &sa,
            NewEntity {
                kind: NoteKind::Person,
                name: "Shady".into(),
                aliases: vec!["شادي".into()],
                tags: vec![],
                fields: BTreeMap::from([
                    ("role".to_owned(), "Driver".to_owned()),
                    ("phone".to_owned(), "+20 100 000 0000".to_owned()),
                ]),
                parent: None,
                id: None,
                force: true,
            },
        )
        .await
        .expect("entity");
    // The user's own sections on the page.
    let user_body = "## Notes\nMet him at the fair. Keep this.\n\n## Family\nUser heading kept.\n";
    let page = w
        .read(a, "people/Shady.md")
        .replace("## Notes\n", user_body);
    w.vault
        .update_note(&sa, shady.id, page, shady.version.clone())
        .await
        .expect("user sections");
    let meeting = w
        .create(
            &sa,
            "notes/Meeting.md",
            "---\npeople: [\"[[Shady]]\"]\n---\nMet Shady about the Watanya contract.\n\nShady prefers weekly invoicing. ^pref\n",
        )
        .await;
    let capture_text = "هسلم العقد لشادي بكرة";
    let capture = w
        .create(
            &sa,
            "notes/Capture.md",
            &format!("---\npeople: [\"[[Shady]]\"]\n---\n{capture_text}\n"),
        )
        .await;
    let before = w.read(a, "people/Shady.md");

    let input = InsightsInput {
        entity: InsightEntity {
            id: shady.id.to_string(),
            kind: "person".into(),
            name: "Shady".into(),
            aliases: vec!["شادي".into()],
            properties: BTreeMap::from([("role".to_owned(), "Driver".to_owned())]),
            hints: vec![],
        },
        notes: vec![
            InsightNote {
                id: capture.to_string(),
                title: "Capture".into(),
                created: CREATED.into(),
                one_line: true,
                blocks: vec![block(capture_text)],
            },
            InsightNote {
                id: meeting.to_string(),
                title: "Meeting".into(),
                created: CREATED.into(),
                one_line: false,
                blocks: vec![
                    block("Met Shady about the Watanya contract."),
                    block_with("pref", "Shady prefers weekly invoicing."),
                ],
            },
        ],
    };
    let cb = generated_block_id(capture_text);
    let mb = generated_block_id("Met Shady about the Watanya contract.");
    let cite =
        |n: strata_common::NoteId, b: &str| json!([{"note_id": n.to_string(), "block_id": b}]);
    push(
        &w,
        ids::ENTITY_INSIGHTS,
        &input,
        json!({
            "summary": "Shady handles the Watanya paperwork. His phone is 01001234567.",
            "insights": [
                {"text": "Prefers weekly invoicing", "citations": cite(meeting, "pref")},
                {"text": "Is very reliable", "citations": cite(meeting, "nope")},
                {"text": "Will deliver contracts personally", "citations": cite(capture, &cb)},
                {"text": "Reach him on 0100 123 4567", "citations": cite(meeting, "pref")}
            ],
            "open_items": [
                {"text": "Bring the Watanya contract", "citations": cite(capture, &cb)}
            ],
            "timeline": [
                {"date": "2026-09-27", "text": "Discussed the Watanya contract", "citations": cite(meeting, &mb)},
                {"date": "2026-09-27", "text": "Will hand over the contract", "citations": cite(capture, &cb)}
            ]
        }),
    );
    enqueue(&w, a, "entity_insights", shady.id).await;
    let r = runner(&w, &["entity_insights"]);
    r.run_until_idle().await;
    assert_input(&w, ids::ENTITY_INSIGHTS, &input);
    let page = w.read(a, "people/Shady.md");
    // Frontmatter (the phone field included) is untouched; the user's sections are kept
    // byte for byte; the uncited, speculative and contact bullets are gone; "بكرة" is
    // 2026-09-28 (the capture's `created` + 1), newest first.
    let front_end = before.find("\n---\n").expect("frontmatter") + 5;
    assert_eq!(page[..front_end], before[..front_end]);
    assert_eq!(
        &page[front_end..],
        format!(
            "## Summary\nShady handles the Watanya paperwork.\n\n## Insights\n- Prefers weekly invoicing [[Meeting#^pref]]\n\n## Open items\n\n## Timeline\n- 2026-09-28 — Will hand over the contract [[Capture#^{cb}]]\n- 2026-09-27 — Discussed the Watanya contract [[Meeting#^{mb}]]\n\n{user_body}"
        )
    );
    assert!(page.contains("phone: +20 100 000 0000"));
    // The cited blocks got their IDs; one `ai:` commit for everything.
    assert!(
        w.read(a, "notes/Capture.md")
            .contains(&format!("{capture_text} ^{cb}"))
    );
    assert!(
        w.read(a, "notes/Meeting.md")
            .contains(&format!("Met Shady about the Watanya contract. ^{mb}"))
    );
    assert_eq!(w.log(a)[0], "ai: entity_insights people/Shady.md");
    let mut paths = w.last_commit_paths(a);
    paths.sort();
    assert_eq!(
        paths,
        vec![
            format!(".meta/notes/{}.json", shady.id),
            "notes/Capture.md".to_owned(),
            "notes/Meeting.md".to_owned(),
            "people/Shady.md".to_owned(),
        ]
    );
    // Same input again: no call, no commit.
    let (calls, commits) = (w.llm.calls().len(), w.log(a).len());
    enqueue(&w, a, "entity_insights", shady.id).await;
    r.run_until_idle().await;
    assert_eq!((w.llm.calls().len(), w.log(a).len()), (calls, commits));
    w.finish().await;
}
