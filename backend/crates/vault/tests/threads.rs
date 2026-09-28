//! A note's AI follow-up thread (owner decision 2026-09-28): appends make one `ai: thread`
//! commit with the `note_threads` row and a `thread` change-log row; a repeated append changes
//! nothing; unknown notes are refused; reindex restores the rows from the files and logs only
//! what changed.
#![allow(clippy::expect_used, clippy::many_single_char_names)]

mod common;

use chrono::{DateTime, FixedOffset};
use common::World;
use pretty_assertions::assert_eq;
use strata_common::NoteId;
use strata_index::repo::sync as log;
use strata_index::repo::threads;
use strata_index::types::ChangeOp;
use strata_vault::VaultError;
use strata_vault::ops::notes::CreateNote;
use ulid::Ulid;
use vault_format::thread::{NoteThread, ThreadCitation, ThreadMessage, ThreadRole};

fn at(s: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s).expect("time")
}

fn exchange(n: u16, note: NoteId) -> (ThreadMessage, ThreadMessage) {
    let id = |k: u16| Ulid::from_parts(1_790_000_000_000, u128::from(k));
    (
        ThreadMessage {
            id: id(n * 2),
            role: ThreadRole::User,
            text: format!("Question {n}?"),
            citations: Vec::new(),
            model: None,
            created: at("2026-09-28T19:00:00+03:00"),
        },
        ThreadMessage {
            id: id(n * 2 + 1),
            role: ThreadRole::Assistant,
            text: format!("Answer {n} [[Pricing]]."),
            citations: vec![ThreadCitation {
                note_id: note.as_ulid(),
                target: "Pricing".into(),
                block_id: None,
            }],
            model: Some("fake/model".into()),
            created: at("2026-09-28T16:00:05Z"),
        },
    )
}

async fn thread_changes(w: &World, user: strata_common::UserId) -> Vec<(String, ChangeOp)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let pos = log::sync_position(&mut tx).await.expect("pos");
    let all = log::changes_since(&mut tx, pos.epoch, 0, 1000)
        .await
        .expect("changes");
    tx.commit().await.expect("commit");
    all.into_iter()
        .filter(|c| c.entity_type == "thread")
        .map(|c| (c.entity_id, c.op))
        .collect()
}

#[tokio::test]
async fn appending_to_a_thread_commits_indexes_and_logs_it() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let note = w
        .vault
        .create_note(
            &s,
            CreateNote {
                created: strata_common::clock::default_test_epoch(),
                path: "notes/Pricing.md".into(),
                content: "Delivery by Friday.\n".into(),
                id: None,
                force: false,
            },
        )
        .await
        .expect("note")
        .id;
    let path = NoteThread::path_for(note.as_ulid());

    let (q, a) = exchange(1, note);
    let thread = w
        .vault
        .append_thread(&s, note, q.clone(), a.clone())
        .await
        .expect("append");
    assert_eq!(thread.messages, [q.clone(), a.clone()]);
    assert_eq!(w.log(u)[0], format!("ai: thread {path}"));
    assert_eq!(w.last_commit_paths(u), std::slice::from_ref(&path));
    assert_eq!(
        NoteThread::from_json(&w.read(u, &path)).expect("file"),
        thread
    );
    assert_eq!(
        w.vault.note_thread(&s, note).await.expect("read"),
        Some(thread.clone())
    );
    let mut tx = w.db.begin(u).await.expect("tx");
    let row = threads::get(&mut tx, note)
        .await
        .expect("row")
        .expect("some");
    tx.commit().await.expect("commit");
    assert_eq!(row.thread, w.read(u, &path));
    assert_eq!(
        thread_changes(&w, u).await,
        [(note.to_string(), ChangeOp::Upsert)]
    );

    // The same exchange again (a retried write): no commit, no change.
    let commits = w.log(u).len();
    w.vault.append_thread(&s, note, q, a).await.expect("again");
    assert_eq!(w.log(u).len(), commits);
    assert_eq!(thread_changes(&w, u).await.len(), 1);

    // A second exchange appends.
    let (q2, a2) = exchange(2, note);
    let two = w
        .vault
        .append_thread(&s, note, q2, a2)
        .await
        .expect("second");
    assert_eq!(two.messages.len(), 4);
    assert_eq!(thread_changes(&w, u).await.len(), 2);

    // An unknown note is refused, and nothing is written for it.
    let unknown = NoteId::from_ulid(Ulid::from_parts(1_790_000_000_000, 99));
    let (q3, a3) = exchange(3, unknown);
    assert!(matches!(
        w.vault.append_thread(&s, unknown, q3, a3).await,
        Err(VaultError::NotFound)
    ));
    assert!(!w.exists(u, &NoteThread::path_for(unknown.as_ulid())));

    // Reindex restores the row from the file and logs nothing new; the index equals the
    // incremental one.
    let before = w.snapshot(u).await;
    w.vault.reindex(&s).await.expect("reindex");
    assert_eq!(w.snapshot(u).await, before);
    assert_eq!(thread_changes(&w, u).await.len(), 2);

    // A thread file removed out of band: reindex drops the row and logs its deletion.
    std::fs::remove_file(w.dir(u).join(&path)).expect("rm");
    w.vault.reindex(&s).await.expect("reindex");
    let changes = thread_changes(&w, u).await;
    assert_eq!(changes.last(), Some(&(note.to_string(), ChangeOp::Delete)));
    let mut tx = w.db.begin(u).await.expect("tx");
    assert_eq!(threads::get(&mut tx, note).await.expect("row"), None);
    tx.commit().await.expect("commit");
    w.finish().await;
}
