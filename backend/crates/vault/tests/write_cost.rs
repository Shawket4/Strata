//! The cost of one write does not grow with the vault (docs/ARCHITECTURE.md "Vault store",
//! write cost). Timing-free regression tests for what made sync push slow at 10 000 files:
//!
//! - the index update of a write (re-deriving notes, their keep-both pairs, the before/after
//!   snapshots, the change log, purge) reads only the written notes' rows, not the rest of a
//!   vault of 450 other notes: every statement is an index lookup (planner statistics as in
//!   a live database, sequential scans disabled so a missing index shows as a scan of the
//!   user's rows);
//! - the git commit of a write writes the same number of objects whatever the number of
//!   files, and builds its tree from `HEAD`'s tree rather than from the index.
#![allow(clippy::expect_used, clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use pretty_assertions::assert_eq;
use strata_common::NoteId;
use strata_index::repo::dedupe;
use strata_index::repo::sync::{self, NewChange};
use strata_index::repo::vault as vrepo;
use strata_index::types::ChangeOp;
use strata_testkit::{TestDb, TestUser};
use strata_vault::derive::{self, Context, Derived};
use strata_vault::diff::{KeepBoth, Snapshot};
use strata_vault::state::{NoteMeta, VaultState};
use strata_vault::{fsio, git, indexer};
use vault_format::sidecar::{KeepBoth as SidecarKeepBoth, NoteSidecar};

fn id(n: u128) -> NoteId {
    NoteId::from_ulid(ulid::Ulid(0x0199_0000_0000_0000_0000_0000_0000_0000 + n))
}

fn at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0)
        .single()
        .expect("time")
}

/// A note of the vault: path, text, sidecar.
type Note = (String, String, Option<NoteSidecar>);

/// A note linking (body, frontmatter relation, task) to `other`, with a tag, a block and a
/// keep-both pair with `other` when `keep_both`.
fn note(n: u128, name: &str, other: &str, other_id: NoteId, keep_both: bool) -> Note {
    let nid = id(n);
    let text = format!(
        "---\nid: {}\ncreated: 2026-09-27T12:00:00+00:00\nrelated: [\"[[{other}]]\"]\n---\n\
         {name} with [[{other}]] #work ^b{n}\n\n- [ ] Call [[{other}]] 📅 2026-10-01 ^t{n}\n",
        nid.as_ulid()
    );
    let sidecar = keep_both.then(|| {
        let mut s = NoteSidecar::new(nid.as_ulid());
        s.keep_both.push(SidecarKeepBoth {
            other_id: other_id.as_ulid(),
            at: at().fixed_offset(),
        });
        s
    });
    (format!("notes/{name}.md"), text, sidecar)
}

fn person(n: u128, name: &str) -> Note {
    let text = format!(
        "---\nid: {}\nkind: person\ncreated: 2026-09-27T12:00:00+00:00\naliases: [\"شادي {n}\"]\n---\n## Notes\n",
        id(n).as_ulid()
    );
    (format!("people/{name}.md"), text, None)
}

fn derive_all(state: &VaultState, notes: &[Note]) -> Vec<Derived> {
    let ctx = Context {
        index: state.path_index(),
        state,
        tz: Tz::UTC,
    };
    notes
        .iter()
        .map(|(path, text, sc)| {
            derive::derive(path, text, sc.as_ref(), &ctx, false).expect("derived")
        })
        .collect()
}

/// Rows and index entries each relation of the schema returned in this transaction.
async fn reads(tx: &mut strata_index::ScopedTx) -> BTreeMap<String, i64> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT c.relname::text, \
                pg_stat_get_xact_tuples_returned(c.oid) + pg_stat_get_xact_tuples_fetched(c.oid) \
         FROM pg_class c WHERE c.relnamespace = 'strata'::regnamespace AND c.relkind IN ('r', 'i')",
    )
    .fetch_all(tx.conn())
    .await
    .expect("stats");
    rows.into_iter().filter(|(_, n)| *n > 0).collect()
}

/// Writes a vault of `filler` unrelated notes plus the three measured notes, then measures one
/// write of the three (as the write path runs it) and returns what it read.
async fn measured_write(db: &TestDb, name: &str, filler: u128) -> BTreeMap<String, i64> {
    let user = TestUser::new(name).create(db).await.expect("user").id;
    let mut notes: Vec<Note> = Vec::new();
    for n in 0..filler {
        notes.push(note(
            100 + n,
            &format!("Filler {n}"),
            &format!("Filler {}", (n + 1) % filler),
            id(100 + (n + 1) % filler),
            true,
        ));
    }
    let measured = vec![
        person(2, "Shady"),
        note(1, "Plan", "Shady", id(2), false),
        note(3, "Plan 2", "Plan", id(1), true),
    ];
    let mut state = VaultState::default();
    for (path, text, _) in notes.iter().chain(&measured) {
        let doc = vault_format::Document::parse(text);
        let nid = NoteId::from_ulid(
            doc.frontmatter()
                .and_then(|f| f.id().ok().flatten())
                .expect("id"),
        );
        state.put_note(
            path,
            NoteMeta {
                id: nid,
                kind: derive::kind_of(&doc),
                version: fsio::version_of(text.as_bytes()),
                link_names: BTreeSet::new(),
            },
        );
    }
    let mut tx = db.begin(user).await.expect("tx");
    let all: Vec<Note> = notes.iter().chain(&measured).cloned().collect();
    indexer::write(&mut tx, &derive_all(&state, &all))
        .await
        .expect("vault");
    tx.commit().await.expect("commit");

    // Planner statistics as a live database has them (a fresh table has none, and every
    // plan then looks alike).
    sqlx::query("ANALYZE")
        .execute(&db.superuser)
        .await
        .expect("analyze");
    let batch = derive_all(&state, &measured);
    let pairs: Vec<(String, String, String)> = batch
        .iter()
        .flat_map(|d| d.keep_both.iter())
        .map(|k| (k.kind.clone(), k.a.clone(), k.b.clone()))
        .collect();
    assert_eq!(pairs.len(), 1, "the sidecar's keep-both pair is derived");
    let ids: BTreeSet<NoteId> = [id(1), id(2), id(3)].into();
    let mut tx = db.begin(user).await.expect("tx");
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(tx.conn())
        .await
        .expect("set");
    let start = reads(&mut tx).await;
    let before = Snapshot::take(&mut tx, &ids, KeepBoth::Among(&pairs))
        .await
        .expect("before");
    indexer::write(&mut tx, &batch).await.expect("rewrite");
    let after = Snapshot::take(&mut tx, &ids, KeepBoth::Among(&pairs))
        .await
        .expect("after");
    assert_eq!(before, after, "rewriting the same rows changes nothing");
    let entity = id(3).to_string();
    let changes: Vec<NewChange<'_>> = ["x", "y"]
        .iter()
        .map(|v| NewChange {
            entity_type: "note",
            entity_id: &entity,
            op: ChangeOp::Upsert,
            version: Some(v),
            at: at(),
        })
        .collect();
    sync::append_changes(&mut tx, &changes).await.expect("log");
    assert_eq!(
        vrepo::keep_both_pairs_of(&mut tx, &id(1).to_string())
            .await
            .expect("pairs")
            .len(),
        1
    );
    assert_eq!(
        dedupe::keep_both_among(&mut tx, &pairs)
            .await
            .expect("among")
            .len(),
        1
    );
    indexer::purge(&mut tx, id(1), batch.get(1))
        .await
        .expect("purge");
    let end = reads(&mut tx).await;
    tx.commit().await.expect("commit");

    let mut tx = db.begin(user).await.expect("tx");
    let log: Vec<(i64, Option<String>)> = sync::changes_since(&mut tx, 1, 0, 10)
        .await
        .expect("changes")
        .into_iter()
        .map(|c| (c.seq, c.version))
        .collect();
    assert_eq!(
        log,
        vec![(1, Some("x".into())), (2, Some("y".into()))],
        "consecutive seqs"
    );
    tx.commit().await.expect("commit");
    end.into_iter()
        .map(|(rel, n)| {
            let n = n - start.get(&rel).copied().unwrap_or(0);
            (rel, n)
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// Filler notes of the large vault: every per-note table (notes, links, relations, tags,
/// blocks, tasks, dedupe keys, keep-both pairs) holds at least this many other rows.
const LARGE: u128 = 450;

/// Most rows and index entries any one relation may return for the measured write: the
/// written notes' own rows and the foreign-key probes of their inserts, far below [`LARGE`],
/// so a statement that reads a table's rows of the whole vault fails.
const BOUND: i64 = 150;

#[tokio::test]
async fn the_index_update_of_a_write_does_not_read_the_rest_of_the_vault() {
    let db = TestDb::new().await.expect("db");
    let reads = measured_write(&db, "alice", LARGE).await;
    assert!(reads.contains_key("notes_pkey"), "{reads:?}");
    let over: BTreeMap<&String, &i64> = reads.iter().filter(|(_, n)| **n > BOUND).collect();
    assert_eq!(over, BTreeMap::new(), "{reads:?}");
    db.cleanup().await.expect("cleanup");
}

/// Loose objects in the repository.
fn objects(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir.join(".git/objects"))
        .expect("objects")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().len() == 2)
        .map(|e| std::fs::read_dir(e.path()).expect("fanout").count())
        .sum()
}

/// Objects one commit of `notes/deep/folder/New <n>.md` writes into a vault of `files`
/// files spread over 20 folders.
fn objects_per_commit(files: usize) -> usize {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    git::init(dir).expect("init");
    for n in 0..files {
        fsio::atomic_write(
            dir,
            &format!("notes/deep/folder/f{}/Note {n}.md", n % 20),
            format!("note {n}\n").as_bytes(),
        )
        .expect("write");
    }
    git::commit_all(dir, "system: import", at()).expect("import");
    let before = objects(dir);
    fsio::atomic_write(dir, "notes/deep/folder/New.md", b"new\n").expect("write");
    git::commit_paths(
        dir,
        &["notes/deep/folder/New.md".to_owned()],
        "user: create notes/deep/folder/New.md",
        at(),
    )
    .expect("commit")
    .expect("changed");
    objects(dir) - before
}

#[test]
fn a_commit_writes_the_same_objects_whatever_the_vault_size() {
    // One blob, the four trees from the root to the file's folder, one commit.
    assert_eq!(objects_per_commit(20), 6);
    assert_eq!(objects_per_commit(2_000), 6);
}

#[test]
fn a_commit_builds_on_heads_tree_not_on_the_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    git::init(dir).expect("init");
    fsio::atomic_write(dir, "notes/A.md", b"a\n").expect("write");
    fsio::atomic_write(dir, "notes/sub/B.md", b"b\n").expect("write");
    let first = git::commit_all(dir, "system: import", at())
        .expect("commit")
        .expect("changed");
    // An index that lost every entry: a tree written from it would drop A and B.
    std::fs::remove_file(dir.join(".git/index")).expect("drop index");
    fsio::atomic_write(dir, "notes/sub/C.md", b"c\n").expect("write");
    fsio::remove(dir, "notes/A.md").expect("remove");
    let second = git::commit_paths(
        dir,
        &["notes/sub/C.md".to_owned(), "notes/A.md".to_owned()],
        "user: edit",
        at(),
    )
    .expect("commit")
    .expect("changed");
    assert_eq!(
        git::changed_paths(dir, &second).expect("paths"),
        vec!["notes/A.md".to_owned(), "notes/sub/C.md".to_owned()]
    );
    assert_eq!(
        git::blob_at(dir, &second, "notes/sub/B.md").expect("blob"),
        Some(b"b\n".to_vec())
    );
    assert_eq!(
        git::blob_at(dir, &second, "notes/A.md").expect("blob"),
        None
    );
    assert_eq!(
        git::blob_at(dir, &first, "notes/A.md").expect("blob"),
        Some(b"a\n".to_vec())
    );
    // Committing unchanged paths is no commit.
    assert_eq!(
        git::commit_paths(dir, &["notes/sub/C.md".to_owned()], "user: noop", at()).expect("commit"),
        None
    );
}
