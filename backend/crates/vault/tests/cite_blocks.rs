//! Making blocks citable for an Ask answer (PLAN §9.5, §6.3): the requested IDs are appended
//! in one `ai:` commit across notes; a block that already has an ID keeps it, two requests
//! for one block share the first ID, and headings, malformed IDs, IDs the note already uses
//! and unknown notes are answered `Missing` (cite the note instead).
#![allow(clippy::expect_used)]

mod common;

use common::World;
use pretty_assertions::assert_eq;
use strata_vault::ops::ai::{CiteOutcome, CiteRequest};
use strata_vault::ops::notes::CreateNote;

const CALL: &str = "# Call\n\nPrices go up.\n\nChurn fell. ^c1\n\nMona leads.\n";

#[tokio::test]
async fn cited_blocks_get_ids_in_one_commit_and_the_rest_is_missing() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let create = |path: &str, content: &str| CreateNote {
        created: strata_common::clock::default_test_epoch(),
        path: path.to_owned(),
        content: content.to_owned(),
        id: None,
        force: true,
    };
    let call = w
        .vault
        .create_note(&s, create("notes/Call.md", CALL))
        .await
        .expect("call");
    let plan = w
        .vault
        .create_note(&s, create("notes/Plan.md", "Plan it.\n"))
        .await
        .expect("plan");
    let request =
        |note: &strata_vault::model::NoteView, body: &str, text: &str, id: &str| CiteRequest {
            note_id: note.id,
            version: note.version.clone(),
            block_start: body.find(text).expect("block"),
            block_text: text.to_owned(),
            block_id: id.to_owned(),
        };
    let unknown = strata_common::NoteId::from_ulid(ulid::Ulid::from_parts(1, 1));
    let requests = vec![
        request(&call, CALL, "# Call", "ask-0"),
        request(&call, CALL, "Prices go up.", "ask-1"),
        request(&call, CALL, "Prices go up.", "ask-2"),
        request(&call, CALL, "Churn fell. ^c1", "ask-3"),
        request(&call, CALL, "Mona leads.", "c1"),
        request(&call, CALL, "Mona leads.", "Not An ID"),
        request(&plan, "Plan it.\n", "Plan it.", "ask-9"),
        CiteRequest {
            note_id: unknown,
            version: "0".repeat(64),
            block_start: 0,
            block_text: "Gone.".into(),
            block_id: "ask-8".into(),
        },
    ];
    let commits = w.log(u).len();

    let outcomes = w
        .vault
        .ai_cite_blocks(&s, requests, "ask".into())
        .await
        .expect("cite");

    assert_eq!(
        outcomes,
        [
            CiteOutcome::Missing,
            CiteOutcome::Appended,
            CiteOutcome::Existing("ask-1".into()),
            CiteOutcome::Existing("c1".into()),
            CiteOutcome::Missing,
            CiteOutcome::Missing,
            CiteOutcome::Appended,
            CiteOutcome::Missing,
        ]
    );
    assert_eq!(w.log(u).len(), commits + 1);
    assert_eq!(w.log(u)[0], "ai: ask 2 notes");
    let body = |path: &str| {
        vault_format::Document::parse(&w.read(u, path))
            .body()
            .to_owned()
    };
    assert_eq!(
        body("notes/Call.md"),
        "# Call\n\nPrices go up. ^ask-1\n\nChurn fell. ^c1\n\nMona leads.\n"
    );
    assert_eq!(body("notes/Plan.md"), "Plan it. ^ask-9\n");

    // Retrieved again after the append: the block keeps its ID, nothing is committed.
    let now = w.vault.note(&s, plan.id).await.expect("plan now");
    let again = w
        .vault
        .ai_cite_blocks(
            &s,
            vec![request(
                &now,
                "Plan it. ^ask-9\n",
                "Plan it. ^ask-9",
                "ask-10",
            )],
            "ask".into(),
        )
        .await
        .expect("cite again");
    assert_eq!(again, [CiteOutcome::Existing("ask-9".into())]);
    assert_eq!(w.log(u).len(), commits + 1);
    w.finish().await;
}
