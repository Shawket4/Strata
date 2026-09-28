//! Exactly-once sync push across crashes (PLAN §7.5 Sync, §16.3–16.4 "replayed pushes are
//! idempotent", D19): a crash is injected into the vault write that applies an op — after
//! its files are written, after its git commit, after its database commit — the writer is
//! restarted, and the device replays the push. Each op must take effect exactly once and the
//! replay must answer exactly what an uninterrupted push answers: alice's pushes crash, bob
//! pushes the same ops (same client IDs, same fake clock) without crashes, and every result
//! byte, file, commit message and change-log row of alice's must equal bob's.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod sync_harness;

use chrono::{TimeZone, Utc};
use pretty_assertions::assert_eq;
use strata_common::{OpId, UserId};
use strata_vault::CrashPoint;
use sync_harness::{H, User};
use sync_model::SyncOp;
use sync_model::ops::{self as o, Op};
use ulid::Ulid;

const OP_BASE: u128 = 0x0199_2222_0000_0000_0000_0000_0000_0000;
const ID_BASE: u128 = 0x0199_3333_0000_0000_0000_0000_0000_0000;
const TASK: &str = "t-01j9crashaaaaaaaaaaaaaaaa";

fn op_id(n: u128) -> Ulid {
    Ulid(OP_BASE + n)
}

fn id(n: u128) -> Ulid {
    Ulid(ID_BASE + n)
}

/// The setup ops (never crashed): a person to link to and a note to edit.
fn setup() -> Vec<SyncOp> {
    vec![
        SyncOp::new(
            op_id(1),
            None,
            Op::EntityCreate(o::EntityCreate {
                id: id(1),
                kind: domain::NoteKind::Person,
                name: "Sam Hany".into(),
                aliases: vec![],
                fields: Default::default(),
                force: false,
            }),
        ),
        SyncOp::new(
            op_id(2),
            None,
            Op::NoteCreate(o::NoteCreate {
                id: id(2),
                path: "notes/A.md".into(),
                content: "# A\n\none\n".into(),
                force: false,
            }),
        ),
    ]
}

/// The ops pushed with a crash, in order; `a` is the content of `notes/A.md` after setup.
fn crashing(a: &str) -> Vec<SyncOp> {
    let at = Utc
        .with_ymd_and_hms(2026, 9, 26, 8, 30, 5)
        .single()
        .expect("ts")
        .fixed_offset();
    vec![
        SyncOp::new(
            op_id(10),
            None,
            Op::NoteCreate(o::NoteCreate {
                id: id(10),
                path: "notes/Plan.md".into(),
                content: "# Plan\n\nship it\n".into(),
                force: false,
            }),
        ),
        SyncOp::new(
            op_id(11),
            Some(sync_model::Version::of_text(a)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(2),
                content: a.replace("one\n", "one\ntwo\n"),
            }),
        ),
        SyncOp::new(
            op_id(12),
            None,
            Op::TaskCreate(o::TaskCreate {
                id: TASK.into(),
                // In a note with a client ID (the task home note's ID is the server's own).
                note_id: Some(id(2)),
                text: "Pay rent".into(),
                due: None,
                scheduled: None,
                start: None,
                recurrence: None,
                reminders: vec![],
                priority: None,
                force: false,
            }),
        ),
        SyncOp::new(
            op_id(13),
            None,
            Op::DocumentCreate(o::DocumentCreate {
                id: id(13),
                name: "Lease contract".into(),
                aliases: vec![],
                doc_type: Some("contract".into()),
                copy: None,
                copy_of: None,
                companies: vec![],
                people: vec![id(1)],
                expires: None,
                force: false,
            }),
        ),
        SyncOp::new(
            op_id(14),
            None,
            Op::Capture(o::Capture {
                id: id(14),
                text: "call the landlord".into(),
                created: at,
            }),
        ),
    ]
}

/// `POST /sync/push` expected to fail (the injected crash): (status, problem type).
async fn crashed_push(h: &H, user: &User, ops: Vec<SyncOp>) -> (u16, String) {
    let body = rmp_serde::to_vec_named(&sync_model::PushRequest { ops }).expect("encode");
    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/sync/push", h.base_url()))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Content-Type", strata_client::MSGPACK)
        .body(body)
        .send()
        .await
        .expect("response");
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.expect("body");
    let problem: sync_model::Problem = rmp_serde::from_slice(&bytes).expect("problem");
    (status, problem.problem_type)
}

/// Change-log rows as (type, entity, op, version), in seq order.
async fn change_log(h: &H, user: UserId) -> Vec<(String, String, String, Option<String>)> {
    let mut tx = h.db.begin(user).await.expect("tx");
    let rows =
        sqlx::query_as("SELECT entity_type, entity_id, op, version FROM change_log ORDER BY seq")
            .fetch_all(tx.conn())
            .await
            .expect("rows");
    tx.commit().await.expect("commit");
    rows
}

/// Integrity warnings as (id, kind, path).
async fn warnings(h: &H, user: UserId) -> Vec<(uuid::Uuid, String, Option<String>)> {
    let mut tx = h.db.begin(user).await.expect("tx");
    let rows = sqlx::query_as("SELECT id, kind, path FROM integrity_warnings ORDER BY id")
        .fetch_all(tx.conn())
        .await
        .expect("rows");
    tx.commit().await.expect("commit");
    rows
}

async fn stored(h: &H, user: UserId, op: Ulid) -> Option<Vec<u8>> {
    let mut tx = h.db.begin(user).await.expect("tx");
    let rec = strata_index::repo::sync::idempotency_get(&mut tx, OpId::from_ulid(op))
        .await
        .expect("get");
    tx.commit().await.expect("commit");
    rec.map(|r| r.result)
}

/// The rows `after` has beyond `before` (the log only grows), sorted.
fn added<T: Clone + Ord>(before: &[T], after: &[T]) -> Vec<T> {
    let mut out = after[before.len()..].to_vec();
    out.sort();
    out
}

fn head_paths(h: &H, user: UserId) -> Vec<String> {
    let dir = h.dir(user);
    let head = strata_vault::git::head(&dir)
        .expect("head")
        .expect("commit");
    let mut paths = strata_vault::git::changed_paths(&dir, &head.id).expect("paths");
    paths.sort();
    paths
}

fn file(h: &H, user: UserId, rel: &str) -> Option<String> {
    std::fs::read_to_string(h.dir(user).join(rel)).ok()
}

/// The raw message of `HEAD` (trailers included).
fn head_message(h: &H, user: UserId) -> String {
    strata_vault::git::commits_since(&h.dir(user), None, 1)
        .expect("walk")
        .pop()
        .expect("commit")
        .1
}

async fn crash_every_op_at(point: CrashPoint) {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let (a, b) = (alice.id, bob.id);
    h.push(&alice, setup()).await;
    h.push(&bob, setup()).await;
    let note_a = h.read(a, "notes/A.md");
    assert_eq!(note_a, h.read(b, "notes/A.md"), "same clock, same IDs");

    for op in crashing(&note_a) {
        let label = format!("{:?}", op.op.kind());
        let (log_a, log_b) = (change_log(&h, a).await, change_log(&h, b).await);
        let warned = warnings(&h, a).await;
        let commits = h.log(a).len();

        h.vault.inject_crash(point);
        assert_eq!(
            crashed_push(&h, &alice, vec![op.clone()]).await,
            (500, "internal".to_owned()),
            "{label}: the push fails with the crash"
        );
        // The process restarts: the writer reloads (and recovers) on the replay.
        h.vault.evict(a);
        let (first, first_bytes) = h.push(&alice, vec![op.clone()]).await;
        let (again, again_bytes) = h.push(&alice, vec![op.clone()]).await;
        let (control, control_bytes) = h.push(&bob, vec![op.clone()]).await;

        // Identical replay results, equal to the uninterrupted push's.
        assert_eq!(again_bytes, first_bytes, "{label}: byte-identical replay");
        assert_eq!(again, first, "{label}");
        assert_eq!(first, control, "{label}: same result as without a crash");
        assert_eq!(first_bytes, control_bytes, "{label}");
        let result = rmp_serde::to_vec_named(&first.results[0].result).expect("encode");
        assert_eq!(
            stored(&h, a, op.op_id).await,
            Some(result.clone()),
            "{label}"
        );
        assert!(
            matches!(
                first.results[0].result,
                sync_model::OpResult::Applied { .. }
            ),
            "{label}: {first:?}"
        );

        // Exactly one commit, the same as bob's, recording the op and its result.
        assert_eq!(h.log(a).len(), commits + 1, "{label}: one commit");
        assert_eq!(h.log(a), h.log(b), "{label}: same history");
        let paths = head_paths(&h, a);
        assert_eq!(paths, head_paths(&h, b), "{label}");
        for p in &paths {
            assert_eq!(file(&h, a, p), file(&h, b, p), "{label}: {p}");
        }
        let recorded =
            strata_vault::receipt::parse_trailers(&head_message(&h, a)).expect("trailers");
        assert_eq!(
            (recorded.op_id, recorded.device, recorded.result),
            (
                OpId::from_ulid(op.op_id),
                strata_common::DeviceId::from_ulid(alice.device),
                result
            ),
            "{label}"
        );

        // The same change-log rows as the uninterrupted write, once.
        let (new_a, new_b) = (
            added(&log_a, &change_log(&h, a).await),
            added(&log_b, &change_log(&h, b).await),
        );
        assert_eq!(new_a, new_b, "{label}: change log");
        assert!(!new_a.is_empty(), "{label}");

        // What recovery reported.
        let notes: Vec<&String> = paths.iter().filter(|p| p.ends_with(".md")).collect();
        let mut expected: Vec<(String, Option<String>)> = match point {
            CrashPoint::AfterFileWrite => paths
                .iter()
                .map(|p| ("interrupted_write_rolled_back".to_owned(), Some(p.clone())))
                .collect(),
            CrashPoint::AfterGitCommit => notes
                .iter()
                .map(|p| ("index_repaired".to_owned(), Some((*p).clone())))
                .chain([("op_result_recovered".to_owned(), None)])
                .collect(),
            CrashPoint::AfterDbCommit => vec![],
        };
        expected.sort();
        let now = warnings(&h, a).await;
        let mut new_warnings: Vec<(String, Option<String>)> = now
            .iter()
            .filter(|w| !warned.contains(w))
            .map(|(_, k, p)| (k.clone(), p.clone()))
            .collect();
        new_warnings.sort();
        assert_eq!(new_warnings, expected, "{label}");
    }

    // A fresh device of each sees the same state.
    let (da, db) = (
        h.bootstrap(&alice, None).await,
        h.bootstrap(&bob, None).await,
    );
    assert_eq!(da.notes(), db.notes());
    assert_eq!(da.notes().len(), db.notes().len());
    assert!(da.notes().contains_key("documents/Lease contract.md"));
    h.finish().await;
}

#[tokio::test]
async fn a_crash_after_the_file_write_rolls_back_and_the_replay_applies_once() {
    crash_every_op_at(CrashPoint::AfterFileWrite).await;
}

#[tokio::test]
async fn a_crash_after_the_git_commit_recovers_the_result_and_the_replay_answers_it() {
    crash_every_op_at(CrashPoint::AfterGitCommit).await;
}

#[tokio::test]
async fn a_crash_after_the_database_commit_answers_the_stored_result() {
    crash_every_op_at(CrashPoint::AfterDbCommit).await;
}
