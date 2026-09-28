//! The two automated note edits (PLAN §6.3, §7.2): a job's `ai:` content edit against the
//! version it read, and appending a block ID to a paragraph before citing it — each one
//! commit named for the job; a stale version or a position that is not a block is refused.
#![allow(clippy::expect_used)]

mod common;

use common::World;
use pretty_assertions::assert_eq;
use strata_vault::ops::notes::CreateNote;
use strata_vault::{Author, VaultError};

#[tokio::test]
async fn jobs_edit_notes_and_append_block_ids_in_their_own_commits() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let created = w
        .vault
        .create_note(
            &s,
            CreateNote {
                created: strata_common::clock::default_test_epoch(),
                path: "notes/Call.md".into(),
                content: "Weekly sync.\n\nPrices go up.\n".into(),
                id: None,
                force: false,
            },
        )
        .await
        .expect("create");
    let body = |v: &strata_vault::model::NoteView| {
        vault_format::Document::parse(&v.content).body().to_owned()
    };

    // A job's edit against the version it read.
    let edited_text = created
        .content
        .replace("Weekly sync.", "Weekly sync with Acme.");
    let edited = w
        .vault
        .ai_update_note(
            &s,
            "enrich".into(),
            created.id,
            edited_text.clone(),
            created.version.clone(),
        )
        .await
        .expect("ai edit");
    assert_eq!(body(&edited), "Weekly sync with Acme.\n\nPrices go up.\n");
    assert_eq!(w.log(u)[0], "ai: enrich notes/Call.md");
    // The same edit against the old version is stale now.
    assert!(matches!(
        w.vault
            .ai_update_note(&s, "enrich".into(), created.id, edited_text, created.version)
            .await,
        Err(VaultError::VersionConflict { current }) if current == edited.version
    ));

    // Citing the second paragraph: its block gets an ID.
    let start = body(&edited).find("Prices").expect("paragraph");
    let cited = w
        .vault
        .append_block_id(
            &s,
            created.id,
            start,
            "ask-000001".into(),
            Author::Ai("ask".into()),
        )
        .await
        .expect("cite");
    assert_eq!(
        body(&cited),
        "Weekly sync with Acme.\n\nPrices go up. ^ask-000001\n"
    );
    assert_eq!(w.log(u)[0], "ai: ask notes/Call.md");
    let commits = w.log(u).len();
    // A position between blocks is refused and nothing is committed.
    assert!(matches!(
        w.vault
            .append_block_id(&s, created.id, start - 1, "ask-000002".into(), Author::Ai("ask".into()))
            .await,
        Err(VaultError::Invalid(m)) if m == "a block id cannot be appended there"
    ));
    assert_eq!(w.log(u).len(), commits);
    w.finish().await;
}
