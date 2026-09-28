//! Vault store mechanics (PLAN §7.2, §16.3): write path, commits, versions, the single
//! writer, move/rename link rewriting, soft delete, history and revert.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

mod common;

use std::collections::BTreeSet;

use common::{World, header};
use domain::RelationType;
use pretty_assertions::assert_eq;
use strata_common::{Clock, NoteId};
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::relations::AiEdge;
use strata_vault::{Author, VaultError};
use vault_format::RelationKey;

fn note(path: &str, content: &str) -> CreateNote {
    CreateNote {
        created: strata_common::clock::default_test_epoch(),
        path: path.to_owned(),
        content: content.to_owned(),
        id: None,
        force: false,
    }
}

#[tokio::test]
async fn create_update_move_delete_restore_purge() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    assert_eq!(w.log(u), vec!["system: initialize vault"]);

    let a = w
        .vault
        .create_note(&s, note("notes/A.md", "# A\n\nFirst [[B]].\n"))
        .await
        .expect("create");
    let id = a.id.to_string();
    let expected = format!("{}# A\n\nFirst [[B]].\n", header(&id));
    assert_eq!(a.content, expected);
    assert_eq!(w.read(u, "notes/A.md"), expected);
    assert_eq!(
        a.version,
        strata_vault::fsio::version_of(expected.as_bytes())
    );
    assert_eq!(
        (a.path.as_str(), a.title.as_str(), a.trashed),
        ("notes/A.md", "A", false)
    );
    assert_eq!(
        w.log(u),
        vec!["user: create notes/A.md", "system: initialize vault"]
    );
    assert_eq!(w.last_commit_paths(u), vec!["notes/A.md"]);

    // Update with the right version; `updated` is refreshed at the (advanced) clock.
    w.db.clock.advance(chrono::Duration::minutes(5));
    let body = format!("{}# A\n\nSecond.\n", header(&id));
    let a2 = w
        .vault
        .update_note(&s, a.id, body, a.version.clone())
        .await
        .expect("update");
    assert_eq!(
        a2.content,
        format!(
            "---\nid: {id}\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:05:00+00:00\n---\n# A\n\nSecond.\n"
        )
    );
    assert_eq!(w.log(u)[0], "user: update notes/A.md");

    // Stale If-Match: 409 with the current version.
    let err = w
        .vault
        .update_note(&s, a.id, "x".into(), a.version.clone())
        .await
        .expect_err("stale");
    assert!(
        matches!(&err, VaultError::VersionConflict { current } if *current == a2.version),
        "{err:?}"
    );

    // Move.
    let moved = w
        .vault
        .move_note(&s, a.id, "archive/A renamed.md".into(), None)
        .await
        .expect("move");
    assert_eq!(moved.path, "archive/A renamed.md");
    assert_eq!(moved.content, a2.content);
    assert!(!w.exists(u, "notes/A.md"));
    assert_eq!(w.log(u)[0], "user: move notes/A.md -> archive/A renamed.md");

    // Soft delete, restore, delete again, purge.
    let trashed = w.vault.delete_note(&s, a.id).await.expect("delete");
    assert_eq!(
        (trashed.path.as_str(), trashed.trashed),
        (".trash/archive/A renamed.md", true)
    );
    assert_eq!(w.log(u)[0], "user: delete archive/A renamed.md");
    let restored = w.vault.restore_note(&s, a.id).await.expect("restore");
    assert_eq!(
        (restored.path.as_str(), restored.trashed),
        ("archive/A renamed.md", false)
    );
    assert_eq!(w.log(u)[0], "user: restore archive/A renamed.md");
    w.vault.delete_note(&s, a.id).await.expect("delete");
    // Purge only works on trashed notes.
    let other = w
        .vault
        .create_note(&s, note("notes/Other.md", "x\n"))
        .await
        .expect("create");
    assert!(matches!(
        w.vault.purge_note(&s, other.id).await,
        Err(VaultError::NotFound)
    ));
    w.vault.purge_note(&s, a.id).await.expect("purge");
    assert!(!w.exists(u, ".trash/archive/A renamed.md"));
    assert_eq!(w.log(u)[0], "user: purge .trash/archive/A renamed.md");
    assert!(matches!(
        w.vault.note(&s, a.id).await,
        Err(VaultError::NotFound)
    ));
    assert_eq!(
        w.log(u),
        vec![
            "user: purge .trash/archive/A renamed.md",
            "user: create notes/Other.md",
            "user: delete archive/A renamed.md",
            "user: restore archive/A renamed.md",
            "user: delete archive/A renamed.md",
            "user: move notes/A.md -> archive/A renamed.md",
            "user: update notes/A.md",
            "user: create notes/A.md",
            "system: initialize vault",
        ]
    );
    w.finish().await;
}

#[tokio::test]
async fn create_rejects_taken_paths_and_bad_names_and_keeps_client_ids() {
    let w = World::new().await;
    let (_, s) = w.user("alice").await;
    let client_id: NoteId = "01J8ZK3M4X7Q9W2E5R6T8Y0V1H".parse().expect("ulid");
    let n = w
        .vault
        .create_note(
            &s,
            CreateNote {
                id: Some(client_id),
                ..note("notes/X.md", "x\n")
            },
        )
        .await
        .expect("create");
    assert_eq!(n.id, client_id);
    assert!(matches!(
        w.vault.create_note(&s, note("notes/X.md", "y\n")).await,
        Err(VaultError::PathTaken)
    ));
    for bad in [
        "../x.md",
        ".meta/x.md",
        "notes/a:b.md",
        "/abs.md",
        "notes/x.txt",
    ] {
        assert!(
            matches!(
                w.vault.create_note(&s, note(bad, "y\n")).await,
                Err(VaultError::InvalidName(_))
            ),
            "{bad}"
        );
    }
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_writer_serialises_concurrent_writes() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let mut handles = Vec::new();
    for i in 0..16 {
        let vault = w.vault.clone();
        handles.push(tokio::spawn(async move {
            vault
                .create_note(
                    &s,
                    note(&format!("notes/N{i:02}.md"), &format!("note {i}\n")),
                )
                .await
        }));
    }
    for h in handles {
        h.await.expect("join").expect("create");
    }
    let log = w.log(u);
    assert_eq!(log.len(), 17);
    let mut created: Vec<String> = log[..16].to_vec();
    created.sort();
    let expected: Vec<String> = (0..16)
        .map(|i| format!("user: create notes/N{i:02}.md"))
        .collect();
    assert_eq!(created, expected);
    // Every commit holds exactly one note: nothing interleaved.
    for c in &strata_vault::git::log(&w.dir(u)).expect("log")[..16] {
        let paths = strata_vault::git::changed_paths(&w.dir(u), &c.id).expect("paths");
        assert_eq!(paths.len(), 1, "{}", c.message);
    }

    // Concurrent updates with the same If-Match: exactly one wins, the rest see its version.
    let target = w
        .vault
        .note_by_path(&s, "notes/N00.md")
        .await
        .expect("note");
    let mut handles = Vec::new();
    for i in 0..8 {
        let vault = w.vault.clone();
        let version = target.version.clone();
        let content = target.content.replace("note 0", &format!("edit {i}"));
        handles.push(tokio::spawn(async move {
            vault.update_note(&s, target.id, content, version).await
        }));
    }
    let mut ok = Vec::new();
    let mut conflicts = Vec::new();
    for h in handles {
        match h.await.expect("join") {
            Ok(v) => ok.push(v),
            Err(VaultError::VersionConflict { current }) => conflicts.push(current),
            Err(e) => panic!("unexpected {e:?}"),
        }
    }
    assert_eq!(ok.len(), 1);
    assert_eq!(conflicts, vec![ok[0].version.clone(); 7]);
    assert_eq!(w.log(u).len(), 18);
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writers_of_different_users_do_not_block_each_other() {
    let w = World::new().await;
    let (_, a) = w.user("alice").await;
    let (_, b) = w.user("bob").await;
    w.vault.ready(&a).await.expect("ready");
    let (hold_tx, hold_rx) = tokio::sync::oneshot::channel::<()>();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let vault = w.vault.clone();
    let blocked = tokio::spawn(async move {
        vault
            .exec(&a, move |_, _| {
                Box::pin(async move {
                    let _ = started_tx.send(());
                    let _ = hold_rx.await;
                    Ok(())
                })
            })
            .await
    });
    started_rx.await.expect("alice's writer is busy");
    // Bob writes while Alice's writer is held.
    let n = w
        .vault
        .create_note(&b, note("notes/Bob.md", "hi\n"))
        .await
        .expect("bob's write completes");
    assert_eq!(n.path, "notes/Bob.md");
    hold_tx.send(()).expect("release");
    blocked.await.expect("join").expect("alice job");
    w.finish().await;
}

#[tokio::test]
async fn move_rewrites_every_inbound_link_form_and_relation_in_one_commit() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let a = w
        .vault
        .create_note(
            &s,
            note(
                "notes/Pricing.md",
                "# Pricing\n\n## Tiers\n\nBasic ^tier1\n",
            ),
        )
        .await
        .expect("a");
    let src = "Links: [[Pricing]], [[Pricing|the plan]], [[Pricing#Tiers]], [[Pricing#^tier1]], ![[Pricing]], [[notes/Pricing]], [[Pricing.md]], `[[Pricing]]` in code.\n";
    let b = w
        .vault
        .create_note(
            &s,
            note(
                "notes/Refs.md",
                &format!("---\nrelated: [\"[[Pricing]]\"]\nsupports: [\"[[Pricing|p]]\"]\ncustom: \"[[Pricing]]\"\n---\n{src}"),
            ),
        )
        .await
        .expect("b");
    let canvas = "{\n\t\"nodes\":[\n\t\t{\"id\":\"n1\",\"type\":\"file\",\"file\":\"notes/Pricing.md\",\"x\":0,\"y\":0,\"width\":400,\"height\":300}\n\t],\n\t\"edges\":[]\n}";
    std::fs::create_dir_all(w.dir(u).join("maps")).expect("mkdir");
    std::fs::write(w.dir(u).join("maps/Plan.canvas"), canvas).expect("write");
    strata_vault::git::commit_paths(
        &w.dir(u),
        &["maps/Plan.canvas".into()],
        "user: create maps/Plan.canvas",
        w.db.clock.now(),
    )
    .expect("commit");
    w.vault.evict(u);

    let commits_before = w.log(u).len();
    w.vault
        .move_note(&s, a.id, "archive/Pricing 2026.md".into(), None)
        .await
        .expect("move");
    assert_eq!(w.log(u).len(), commits_before + 1);
    assert_eq!(
        w.log(u)[0],
        "user: move notes/Pricing.md -> archive/Pricing 2026.md"
    );
    let mut paths = w.last_commit_paths(u);
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "archive/Pricing 2026.md",
            "maps/Plan.canvas",
            "notes/Pricing.md",
            "notes/Refs.md"
        ]
    );
    let refs = w.read(u, "notes/Refs.md");
    assert_eq!(
        refs,
        format!(
            "---\nid: {}\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:00:00+00:00\nrelated: [\"[[Pricing 2026]]\"]\nsupports: [\"[[Pricing 2026|p]]\"]\ncustom: \"[[Pricing]]\"\n---\nLinks: [[Pricing 2026]], [[Pricing 2026|the plan]], [[Pricing 2026#Tiers]], [[Pricing 2026#^tier1]], ![[Pricing 2026]], [[Pricing 2026]], [[Pricing 2026.md]], `[[Pricing]]` in code.\n",
            b.id
        )
    );
    assert_eq!(
        w.read(u, "maps/Plan.canvas"),
        canvas.replace("notes/Pricing.md", "archive/Pricing 2026.md")
    );
    // The index resolves every rewritten link and both relations to the moved note.
    let groups = w.vault.backlinks(&s, a.id).await.expect("backlinks");
    let summary: Vec<(String, usize)> = groups
        .iter()
        .map(|g| (g.kind.clone(), g.items.len()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("link".into(), 3),
            ("embed".into(), 1),
            ("related".into(), 1),
            ("supports".into(), 1)
        ]
    );
    let links: Vec<(Option<String>, Option<String>)> = groups[0]
        .items
        .iter()
        .map(|l| (l.anchor.clone(), l.block_id.clone()))
        .collect();
    assert_eq!(
        links,
        vec![
            (None, None),
            (None, Some("tier1".into())),
            (Some("Tiers".into()), None)
        ]
    );
    w.finish().await;
}

#[tokio::test]
async fn deleting_a_note_unresolves_links_and_restoring_resolves_them_again() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let a = w
        .vault
        .create_note(&s, note("notes/A.md", "a\n"))
        .await
        .expect("a");
    let b = w
        .vault
        .create_note(&s, note("notes/B.md", "see [[A]]\n"))
        .await
        .expect("b");
    async fn dst(w: &World, u: strata_common::UserId, b: NoteId) -> Option<NoteId> {
        let mut tx = w.db.begin(u).await.expect("tx");
        let l = strata_index::repo::graph::links_from(&mut tx, b)
            .await
            .expect("links");
        tx.commit().await.expect("commit");
        l[0].dst_id
    }
    assert_eq!(dst(&w, u, b.id).await, Some(a.id));
    w.vault.delete_note(&s, a.id).await.expect("delete");
    assert_eq!(dst(&w, u, b.id).await, None);
    let tree: Vec<String> = w
        .vault
        .tree(&s)
        .await
        .expect("tree")
        .into_iter()
        .filter_map(|e| match e {
            strata_vault::model::TreeEntry::Note { path, .. } => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(tree, vec!["notes/B.md"]);
    w.vault.restore_note(&s, a.id).await.expect("restore");
    assert_eq!(dst(&w, u, b.id).await, Some(a.id));
    w.finish().await;
}

#[tokio::test]
async fn history_file_at_revision_revert_and_whole_commit_revert() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let t = w
        .vault
        .create_note(&s, note("notes/T.md", "target\n"))
        .await
        .expect("t");
    let n = w
        .vault
        .create_note(&s, note("notes/H.md", "one\ntwo\nthree\n"))
        .await
        .expect("create");
    let v1 = n.content.clone();
    let ai_commit = w
        .vault
        .ai_add_relations(
            &s,
            "link".into(),
            n.id,
            vec![AiEdge {
                rel: RelationKey::Note(RelationType::Related),
                dst: t.id,
                confidence: 0.9,
                reason: "same topic".into(),
                model: "fake/model".into(),
            }],
        )
        .await
        .expect("ai edit")
        .expect("a commit");
    assert_eq!(w.log(u)[0], "ai: link notes/H.md");
    let after_ai = w.vault.note(&s, n.id).await.expect("note");
    assert_eq!(
        after_ai.content,
        v1.replace("---\none", "related: [\"[[T]]\"]\n---\none")
    );
    let sidecar = format!(".meta/notes/{}.json", n.id);
    assert_eq!(
        w.read(u, &sidecar),
        format!(
            "{{\n  \"id\": \"{}\",\n  \"relations\": [\n    {{\n      \"type\": \"related\",\n      \"target_id\": \"{}\",\n      \"by\": \"ai\",\n      \"confidence\": 0.9,\n      \"reason\": \"same topic\",\n      \"model\": \"fake/model\",\n      \"created\": \"2026-09-27T12:00:00Z\"\n    }}\n  ],\n  \"rejected\": []\n}}\n",
            n.id, t.id
        )
    );
    w.db.clock.advance(chrono::Duration::minutes(1));
    let user_edit = w
        .vault
        .update_note(
            &s,
            n.id,
            after_ai.content.replace("three", "THREE"),
            after_ai.version.clone(),
        )
        .await
        .expect("user edit");
    let history = w.vault.history(&s, n.id).await.expect("history");
    let summary: Vec<(String, String, String)> = history
        .iter()
        .map(|r| (r.author.clone(), r.message.clone(), r.change.clone()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "user".into(),
                "user: update notes/H.md".into(),
                "modified".into()
            ),
            ("ai".into(), "ai: link notes/H.md".into(), "modified".into()),
            (
                "user".into(),
                "user: create notes/H.md".into(),
                "added".into()
            ),
        ]
    );
    let first = w
        .vault
        .note_at(&s, n.id, &history[2].commit)
        .await
        .expect("at revision");
    assert_eq!(first.content, v1);
    assert!(matches!(
        w.vault
            .note_at(&s, n.id, "0000000000000000000000000000000000000000")
            .await,
        Err(VaultError::NotFound)
    ));

    // Revert the whole ai commit: the relation and its provenance go, the later user edit
    // (body and `updated`) survives.
    let (commit, paths) = w
        .vault
        .revert_commit(&s, ai_commit.clone())
        .await
        .expect("revert");
    assert!(commit.is_some());
    assert_eq!(paths, vec![sidecar.clone(), "notes/H.md".to_owned()]);
    let short: String = ai_commit.chars().take(12).collect();
    assert_eq!(w.log(u)[0], format!("user: revert commit {short}"));
    let after = w.vault.note(&s, n.id).await.expect("note");
    assert_eq!(
        after.content,
        user_edit.content.replace("related: [\"[[T]]\"]\n", "")
    );
    assert!(!w.exists(u, &sidecar));
    assert_eq!(
        w.vault.backlinks(&s, t.id).await.expect("backlinks"),
        vec![]
    );

    // Revert the file to its first revision.
    let reverted = w
        .vault
        .revert_note(&s, n.id, history[2].commit.clone())
        .await
        .expect("revert note");
    assert_eq!(reverted.content, v1);
    assert_eq!(w.log(u)[0], "user: revert notes/H.md");
    w.finish().await;
}

#[tokio::test]
async fn capture_is_saved_first_and_duplicates_only_flag_it() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let first = w
        .vault
        .capture(&s, "Watanya's ETA invoice".into())
        .await
        .expect("capture");
    assert_eq!(first.note.path, "inbox/2026-09-27-120000.md");
    assert_eq!(
        first.note.content,
        format!(
            "---\nid: {}\ncreated: 2026-09-27T12:00:00+00:00\n---\nWatanya's ETA invoice\n",
            first.note.id
        )
    );
    assert_eq!(first.duplicates, vec![]);
    let second = w
        .vault
        .capture(&s, "ETA invoice for Watanya".into())
        .await
        .expect("capture is never refused");
    assert_eq!(second.note.path, "inbox/2026-09-27-120000 2.md");
    let found: Vec<(String, String, strata_vault::MatchLevel)> = second
        .duplicates
        .iter()
        .map(|c| (c.id.to_string(), c.kind.clone(), c.level))
        .collect();
    assert_eq!(
        found,
        vec![(
            first.note.id.to_string(),
            "capture".into(),
            strata_vault::MatchLevel::Exact
        )]
    );
    assert!(second.suggestion.is_some());
    assert_eq!(
        w.log(u)[..2],
        [
            "user: capture inbox/2026-09-27-120000 2.md".to_owned(),
            "user: capture inbox/2026-09-27-120000.md".to_owned()
        ]
    );
    let inbox = w.vault.inbox(&s).await.expect("inbox");
    let got: Vec<(String, usize)> = inbox
        .iter()
        .map(|(n, s)| (n.path.clone(), s.len()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("inbox/2026-09-27-120000 2.md".into(), 1),
            ("inbox/2026-09-27-120000.md".into(), 0)
        ]
    );
    let _ = BTreeSet::<String>::new();
    let _ = Author::User;
    w.finish().await;
}
