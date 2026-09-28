//! `POST /sync/push` (PLAN §7.5 Sync, §12.4, §16.3–16.4, D19): every op kind with exact
//! results, the clean-merge and conflict-copy paths, duplicates then a forced retry,
//! byte-identical replays without a second commit, ordering within one push, and isolation.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::assigning_clones,
    clippy::default_trait_access,
    clippy::float_cmp
)]

mod sync_harness;

use chrono::{NaiveDate, TimeZone, Utc};
use domain::{CustodyEventType, DedupeKind, MatchLevel, NoteKind};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use sync_harness::{H, User, header, version};
use sync_model::ops::{self as o, Op};
use sync_model::{ConflictResolution, OpResult, Problem, SyncOp, Version};
use ulid::Ulid;

const OP_BASE: u128 = 0x0199_0000_0000_0000_0000_0000_0000_0000;
const ID_BASE: u128 = 0x0199_1111_0000_0000_0000_0000_0000_0000;

fn op_id(n: u128) -> Ulid {
    Ulid(OP_BASE + n)
}

fn id(n: u128) -> Ulid {
    Ulid(ID_BASE + n)
}

fn op(n: u128, base: Option<Version>, op: Op) -> SyncOp {
    SyncOp::new(op_id(n), base, op)
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("date")
}

fn applied(v: &str) -> OpResult {
    OpResult::Applied {
        new_version: Some(version(v)),
        merged: false,
    }
}

fn applied_none() -> OpResult {
    OpResult::Applied {
        new_version: None,
        merged: false,
    }
}

fn not_found() -> OpResult {
    OpResult::Rejected {
        problem: Problem {
            problem_type: "not_found".into(),
            title: "Not found".into(),
            status: 404,
            detail: None,
        },
    }
}

/// The payload of a forced `link`/`file_inbox` job (`POST /notes/{id}/relink`).
fn forced_link_payload() -> Vec<u8> {
    rmp_serde::to_vec_named(&strata_jobs::link::LinkParams { force: true }).expect("params")
}

async fn one(h: &H, user: &User, sync_op: SyncOp) -> OpResult {
    let (res, _) = h.push(user, vec![sync_op]).await;
    res.results.into_iter().next().expect("one result").result
}

fn create(n: u128, path: &str, body: &str) -> Op {
    Op::NoteCreate(o::NoteCreate {
        created: strata_common::clock::default_test_epoch(),
        id: id(n),
        path: path.into(),
        content: body.into(),
        force: false,
    })
}

#[tokio::test]
async fn every_op_kind_applies_with_exact_results_one_commit_each() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let commits = |h: &H| h.log(uid).len();

    // note.create: the server stamps id/created/updated; the result is the stored version.
    let before = commits(&h);
    let r = one(
        &h,
        &alice,
        op(1, None, create(1, "notes/A.md", "# A\n\nline one\n")),
    )
    .await;
    let a = h.read(uid, "notes/A.md");
    assert_eq!(
        a,
        format!("{}# A\n\nline one\n", header(&id(1).to_string()))
    );
    assert_eq!(r, applied(&a));
    assert_eq!(commits(&h), before + 1);
    assert_eq!(h.log(uid)[0], "user: create notes/A.md");
    let r = one(&h, &alice, op(2, None, create(2, "notes/B.md", "# B\n"))).await;
    let b = h.read(uid, "notes/B.md");
    assert_eq!(r, applied(&b));

    // note.update with the current version.
    let edited = a.replace("line one\n", "line one\nline two\n");
    let r = one(
        &h,
        &alice,
        op(
            3,
            Some(version(&a)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: edited.clone(),
            }),
        ),
    )
    .await;
    assert_eq!(h.read(uid, "notes/A.md"), edited);
    assert_eq!(r, applied(&edited));
    assert_eq!(h.log(uid)[0], "user: update notes/A.md");

    // capture keeps the client ID and names the inbox file from the device time.
    let at = Utc
        .with_ymd_and_hms(2026, 9, 26, 8, 30, 5)
        .single()
        .expect("ts")
        .fixed_offset();
    let r = one(
        &h,
        &alice,
        op(
            4,
            None,
            Op::Capture(o::Capture {
                id: id(3),
                text: "Call the notary".into(),
                created: at,
            }),
        ),
    )
    .await;
    let cap = h.read(uid, "inbox/2026-09-26-083005.md");
    assert_eq!(
        cap,
        format!(
            "---\nid: {}\ncreated: 2026-09-26T08:30:05+00:00\n---\nCall the notary\n",
            id(3)
        )
    );
    assert_eq!(r, applied(&cap));

    // relation.add / retype / remove: the source note's new version.
    let rel = |t: &str| t.parse().expect("relation");
    let r = one(
        &h,
        &alice,
        op(
            5,
            None,
            Op::RelationAdd(o::RelationRef {
                src_id: id(1),
                dst_id: id(2),
                relation: rel("related"),
            }),
        ),
    )
    .await;
    let a2 = h.read(uid, "notes/A.md");
    assert!(a2.contains("related: [\"[[B]]\"]\n"), "{a2}");
    assert_eq!(r, applied(&a2));
    let r = one(
        &h,
        &alice,
        op(
            6,
            None,
            Op::RelationRetype(o::RelationRetype {
                src_id: id(1),
                dst_id: id(2),
                relation: rel("related"),
                new_type: rel("supports"),
            }),
        ),
    )
    .await;
    let a3 = h.read(uid, "notes/A.md");
    assert!(
        a3.contains("supports: [\"[[B]]\"]\n") && !a3.contains("related:"),
        "{a3}"
    );
    assert_eq!(r, applied(&a3));
    let r = one(
        &h,
        &alice,
        op(
            7,
            None,
            Op::RelationRemove(o::RelationRef {
                src_id: id(1),
                dst_id: id(2),
                relation: rel("supports"),
            }),
        ),
    )
    .await;
    assert_eq!(h.read(uid, "notes/A.md"), edited);
    assert_eq!(r, applied(&edited));
    // Removing it again (another device did too) is already in effect.
    let r = one(
        &h,
        &alice,
        op(
            8,
            None,
            Op::RelationRemove(o::RelationRef {
                src_id: id(1),
                dst_id: id(2),
                relation: rel("supports"),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied(&edited));

    // entity.create / patch / merge.
    let r = one(
        &h,
        &alice,
        op(
            9,
            None,
            Op::EntityCreate(o::EntityCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(4),
                kind: NoteKind::Person,
                name: "Sam Hany".into(),
                aliases: vec!["سام".into()],
                fields: [("role".to_owned(), "Accountant".to_owned())].into(),
                force: false,
            }),
        ),
    )
    .await;
    let sam = h.read(uid, "people/Sam Hany.md");
    assert_eq!(r, applied(&sam));
    assert!(
        sam.contains("kind: person\n") && sam.contains("role: Accountant\n"),
        "{sam}"
    );
    let r = one(
        &h,
        &alice,
        op(
            10,
            Some(version(&sam)),
            Op::EntityPatch(o::EntityPatch {
                id: id(4),
                set: [("phone".to_owned(), "0100".to_owned())].into(),
                unset: vec!["role".into()],
                add_aliases: vec!["Sammy H".into()],
                remove_aliases: vec![],
            }),
        ),
    )
    .await;
    let sam2 = h.read(uid, "people/Sam Hany.md");
    assert!(
        sam2.contains("phone: \"0100\"\n") || sam2.contains("phone: '0100'\n"),
        "{sam2}"
    );
    assert!(
        !sam2.contains("role:") && sam2.contains("Sammy H"),
        "{sam2}"
    );
    assert_eq!(r, applied(&sam2));
    let r = one(
        &h,
        &alice,
        op(
            11,
            None,
            Op::EntityCreate(o::EntityCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(5),
                kind: NoteKind::Person,
                name: "Samuel Hany".into(),
                aliases: vec![],
                fields: Default::default(),
                force: true,
            }),
        ),
    )
    .await;
    assert_eq!(r, applied(&h.read(uid, "people/Samuel Hany.md")));
    let r = one(
        &h,
        &alice,
        op(
            12,
            None,
            Op::EntityMerge(o::EntityMerge {
                id: id(5),
                into_id: id(4),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied(&h.read(uid, "people/Sam Hany.md")));
    assert!(!h.exists(uid, "people/Samuel Hany.md"));
    assert_eq!(
        h.log(uid)[0],
        "user: merge people/Samuel Hany.md -> people/Sam Hany.md"
    );

    // place.create / patch, document.create / patch / custody.
    let r = one(
        &h,
        &alice,
        op(
            13,
            None,
            Op::PlaceCreate(o::PlaceCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(6),
                name: "Office safe".into(),
                aliases: vec![],
                parent_id: None,
                address: None,
                force: false,
            }),
        ),
    )
    .await;
    let safe = h.read(uid, "places/Office safe.md");
    assert_eq!(r, applied(&safe));
    let r = one(
        &h,
        &alice,
        op(
            14,
            Some(version(&safe)),
            Op::PlacePatch(o::EntityPatch {
                id: id(6),
                set: [("address".to_owned(), "Nasr City".to_owned())].into(),
                ..o::EntityPatch::default()
            }),
        ),
    )
    .await;
    let safe2 = h.read(uid, "places/Office safe.md");
    assert!(safe2.contains("address: Nasr City\n"), "{safe2}");
    assert_eq!(r, applied(&safe2));
    let r = one(
        &h,
        &alice,
        op(
            15,
            None,
            Op::DocumentCreate(o::DocumentCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(7),
                name: "Lease contract".into(),
                aliases: vec![],
                doc_type: Some("contract".into()),
                copy: Some(domain::CopyKind::Original),
                copy_of: None,
                companies: vec![],
                people: vec![id(4)],
                expires: None,
                force: false,
            }),
        ),
    )
    .await;
    let lease = h.read(uid, "documents/Lease contract.md");
    assert!(lease.contains("people: [\"[[Sam Hany]]\"]\n"), "{lease}");
    assert_eq!(r, applied(&lease));
    let r = one(
        &h,
        &alice,
        op(
            16,
            Some(version(&lease)),
            Op::DocumentPatch(o::EntityPatch {
                id: id(7),
                set: [("expires".to_owned(), "2027-01-31".to_owned())].into(),
                ..o::EntityPatch::default()
            }),
        ),
    )
    .await;
    let lease2 = h.read(uid, "documents/Lease contract.md");
    assert!(lease2.contains("expires: 2027-01-31\n"), "{lease2}");
    assert_eq!(r, applied(&lease2));
    let r = one(
        &h,
        &alice,
        op(
            17,
            None,
            Op::DocumentCustody(o::DocumentCustody {
                document_id: id(7),
                event: CustodyEventType::StoredAt,
                at: d(2026, 9, 20),
                place_id: Some(id(6)),
                person_id: None,
                counterparty_id: None,
            }),
        ),
    )
    .await;
    let lease3 = h.read(uid, "documents/Lease contract.md");
    assert!(
        lease3.contains("location: \"[[Office safe]]\"\n"),
        "{lease3}"
    );
    assert!(lease3.contains("## Custody\n"), "{lease3}");
    assert_eq!(r, applied(&lease3));
    assert_eq!(h.log(uid)[0], "user: custody documents/Lease contract.md");
    // A patch to the wrong kind is refused like a missing entity.
    let r = one(
        &h,
        &alice,
        op(
            18,
            Some(version(&lease3)),
            Op::PlacePatch(o::EntityPatch {
                id: id(7),
                ..o::EntityPatch::default()
            }),
        ),
    )
    .await;
    assert_eq!(r, not_found());

    // task.create / update / complete / reopen / cancel / delete (line versions).
    let tid = "t-01j9taskaaaaaaaaaaaaaaaaaa".to_owned();
    let r = one(
        &h,
        &alice,
        op(
            19,
            None,
            Op::TaskCreate(o::TaskCreate {
                created: strata_common::clock::default_test_epoch(),
                home_id: None,
                id: tid.clone(),
                note_id: None,
                text: "Renew the lease".into(),
                due: Some(d(2026, 10, 5)),
                scheduled: None,
                start: None,
                recurrence: None,
                reminders: vec![],
                priority: None,
                force: false,
            }),
        ),
    )
    .await;
    let line1 = format!("- [ ] Renew the lease 📅 2026-10-05 ^{tid}");
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{line1}\n"))
    );
    assert_eq!(r, applied(&line1));
    let r = one(
        &h,
        &alice,
        op(
            20,
            Some(version(&line1)),
            Op::TaskUpdate(o::TaskUpdate {
                id: tid.clone(),
                text: Some("Renew the office lease".into()),
                due: Some(None),
                ..o::TaskUpdate::default()
            }),
        ),
    )
    .await;
    let line2 = format!("- [ ] Renew the office lease ^{tid}");
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{line2}\n"))
    );
    assert_eq!(r, applied(&line2));
    let r = one(
        &h,
        &alice,
        op(
            21,
            Some(version(&line2)),
            Op::TaskComplete(o::TaskComplete {
                id: tid.clone(),
                done: d(2026, 9, 25),
                next_id: None,
            }),
        ),
    )
    .await;
    let line3 = format!("- [x] Renew the office lease ✅ 2026-09-25 ^{tid}");
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{line3}\n"))
    );
    assert_eq!(r, applied(&line3));
    assert_eq!(h.log(uid)[0], "user: task complete tasks/Tasks.md");
    let r = one(
        &h,
        &alice,
        op(
            22,
            Some(version(&line3)),
            Op::TaskReopen(o::TaskRef { id: tid.clone() }),
        ),
    )
    .await;
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{line2}\n"))
    );
    assert_eq!(r, applied(&line2));
    let r = one(
        &h,
        &alice,
        op(
            23,
            Some(version(&line2)),
            Op::TaskCancel(o::TaskCancel {
                id: tid.clone(),
                date: d(2026, 9, 26),
            }),
        ),
    )
    .await;
    let line4 = format!("- [-] Renew the office lease ❌ 2026-09-26 ^{tid}");
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{line4}\n"))
    );
    assert_eq!(r, applied(&line4));
    let r = one(
        &h,
        &alice,
        op(
            24,
            Some(version(&line4)),
            Op::TaskDelete(o::TaskRef { id: tid.clone() }),
        ),
    )
    .await;
    assert!(!h.read(uid, "tasks/Tasks.md").contains(&tid));
    assert_eq!(r, applied_none());
    assert_eq!(h.log(uid)[0], "user: task delete tasks/Tasks.md");

    // note.move and note.delete.
    let r = one(
        &h,
        &alice,
        op(
            25,
            Some(version(&b)),
            Op::NoteMove(o::NoteMove {
                id: id(2),
                new_path: "archive/B.md".into(),
            }),
        ),
    )
    .await;
    let b2 = h.read(uid, "archive/B.md");
    assert_eq!(b2, b);
    assert_eq!(r, applied(&b2));
    let r = one(
        &h,
        &alice,
        op(
            26,
            Some(version(&b2)),
            Op::NoteDelete(o::NoteRef { id: id(2) }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    assert_eq!(h.read(uid, ".trash/archive/B.md"), b2);
    // Deleting again is already in effect.
    let r = one(
        &h,
        &alice,
        op(
            27,
            Some(version(&b2)),
            Op::NoteDelete(o::NoteRef { id: id(2) }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());

    // relink.request queues a forced linking run (no commit).
    let before = commits(&h);
    let r = one(
        &h,
        &alice,
        op(28, None, Op::RelinkRequest(o::NoteRef { id: id(1) })),
    )
    .await;
    assert_eq!(r, applied_none());
    assert_eq!(commits(&h), before);
    let mut tx =
        h.db.app_db
            .begin(&h.db.issuer.issue(uid))
            .await
            .expect("tx");
    let queued: Vec<(String, uuid::Uuid, Vec<u8>)> =
        sqlx::query_as("SELECT kind, note_id, payload FROM jobs WHERE kind = 'link'")
            .fetch_all(tx.conn())
            .await
            .expect("jobs");
    tx.commit().await.expect("commit");
    assert_eq!(
        queued,
        vec![(
            "link".to_owned(),
            uuid::Uuid::from(id(1)),
            forced_link_payload()
        )]
    );

    // device.settings on the pushing device.
    let r = one(
        &h,
        &alice,
        op(
            29,
            None,
            Op::DeviceSettings(o::DeviceSettings {
                device_id: alice.device,
                reminders_enabled: Some(false),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    let devices = ops::list_devices(&alice.client).await.expect("devices");
    assert!(!devices[0].reminders_enabled);

    // suggestion.reply / accept / reject on duplicate suggestions from captures.
    let dup = |n: u128, i: u128| {
        op(
            n,
            None,
            Op::Capture(o::Capture {
                id: id(i),
                text: "Call the notary".into(),
                created: at + chrono::Duration::seconds(i64::try_from(i).expect("small")),
            }),
        )
    };
    let (res, _) = h.push(&alice, vec![dup(30, 20), dup(31, 21)]).await;
    assert!(matches!(res.results[0].result, OpResult::Applied { .. }));
    let pending = ops::list_suggestions(&alice.client, None)
        .await
        .expect("suggestions");
    let ids: Vec<Ulid> = pending.items.iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), 2, "{pending:?}");
    let r = one(
        &h,
        &alice,
        op(
            32,
            None,
            Op::SuggestionReply(o::SuggestionReply {
                id: ids[0],
                reply_id: id(30),
                text: "keep them both".into(),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    let r = one(
        &h,
        &alice,
        op(
            33,
            None,
            Op::SuggestionAccept(o::SuggestionAccept {
                created: strata_common::clock::default_test_epoch(),
                id: ids[0],
                edits: None,
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    let r = one(
        &h,
        &alice,
        op(
            34,
            None,
            Op::SuggestionReject(o::SuggestionReject {
                id: ids[1],
                reason: None,
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    // Deciding again (another device did it first) keeps the server's decision.
    let r = one(
        &h,
        &alice,
        op(
            35,
            None,
            Op::SuggestionAccept(o::SuggestionAccept {
                created: strata_common::clock::default_test_epoch(),
                id: ids[1],
                edits: None,
            }),
        ),
    )
    .await;
    assert_eq!(
        r,
        OpResult::Conflict {
            server_version: None,
            resolution: ConflictResolution::ServerKept {
                reason: "the suggestion was already decided".into()
            }
        }
    );
    let all = ops::list_suggestions(&alice.client, Some(&types::SuggestionStatus::Accepted))
        .await
        .expect("accepted");
    assert_eq!(all.items.len(), 1);
    assert_eq!(all.items[0].replies.len(), 1);
    assert_eq!(all.items[0].replies[0].id, id(30));

    // Every kind was exercised.
    h.finish().await;
}

#[tokio::test]
async fn stale_updates_merge_cleanly_or_become_conflict_copies() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let body = "# Plan\n\nalpha\nbeta\ngamma\n";
    one(&h, &alice, op(1, None, create(1, "notes/Plan.md", body))).await;
    let v1 = h.read(uid, "notes/Plan.md");
    // The server (another device) edits the first line.
    let server = v1.replace("alpha\n", "ALPHA\n");
    one(
        &h,
        &alice,
        op(
            2,
            Some(version(&v1)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: server.clone(),
            }),
        ),
    )
    .await;
    let v2 = h.read(uid, "notes/Plan.md");
    assert_eq!(v2, server);

    // A device still on v1 edits the last line: non-overlapping → clean 3-way merge.
    let device = v1.replace("gamma\n", "GAMMA\n");
    let commits = h.log(uid).len();
    let r = one(
        &h,
        &alice,
        op(
            3,
            Some(version(&v1)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: device,
            }),
        ),
    )
    .await;
    let merged = v1
        .replace("alpha\n", "ALPHA\n")
        .replace("gamma\n", "GAMMA\n");
    assert_eq!(h.read(uid, "notes/Plan.md"), merged);
    assert_eq!(
        r,
        OpResult::Applied {
            new_version: Some(version(&merged)),
            merged: true
        }
    );
    assert_eq!(h.log(uid).len(), commits + 1);

    // Another device on v1 changes the first line differently: overlap → conflict copy.
    let other = v1.replace("alpha\n", "Alpha (draft)\n");
    let r = one(
        &h,
        &alice,
        op(
            4,
            Some(version(&v1)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: other.clone(),
            }),
        ),
    )
    .await;
    // The server version stands.
    assert_eq!(h.read(uid, "notes/Plan.md"), merged);
    let copy_path = "notes/Plan (conflict 2026-09-27 120000).md";
    let copy = h.read(uid, copy_path);
    // The copy is the device's content under its own ID (the op ID).
    assert_eq!(
        copy,
        other.replace(&format!("id: {}", id(1)), &format!("id: {}", op_id(4)))
    );
    assert_eq!(
        r,
        OpResult::Conflict {
            server_version: Some(version(&merged)),
            resolution: ConflictResolution::ConflictCopy {
                note_id: op_id(4),
                path: copy_path.into(),
                version: version(&copy),
            }
        }
    );
    // The conflict is recorded as a suggestion on the note.
    let pending = ops::list_suggestions(&alice.client, None)
        .await
        .expect("suggestions");
    assert_eq!(pending.items.len(), 1);
    let s = &pending.items[0];
    assert_eq!(
        (s.id, s.kind.as_str(), s.note_id),
        (op_id(4), "conflict", Some(id(1)))
    );
    let payload: sync_model::suggestions::ConflictPayload = match &s.payload {
        types::SuggestionPayload::Opaque { data } => {
            rmp_serde::from_slice(data).expect("conflict payload")
        }
        other => panic!("unexpected payload {other:?}"),
    };
    assert_eq!(
        payload,
        sync_model::suggestions::ConflictPayload {
            op_id: op_id(4),
            copy_id: op_id(4),
            copy_path: copy_path.into(),
            base_version: version(&v1).to_string(),
            server_version: version(&merged).to_string(),
            hunks: 1,
        }
    );
    // An edit equal to the current content is already applied.
    let r = one(
        &h,
        &alice,
        op(
            5,
            Some(version(&v1)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: merged.clone(),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied(&merged));
    h.finish().await;
}

#[tokio::test]
async fn duplicate_creates_answer_candidates_then_a_forced_retry_creates_and_keeps_both() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    one(
        &h,
        &alice,
        op(
            1,
            None,
            create(1, "notes/ETA invoice for Watanya.md", "Monthly invoice\n"),
        ),
    )
    .await;
    let dup = create(2, "notes/ETA invoice for Watanya.md", "Monthly invoice\n");
    let Op::NoteCreate(mut p) = dup else {
        unreachable!()
    };
    p.path = "notes/Watanya ETA invoice.md".into();
    let commits = h.log(uid).len();
    let r = one(&h, &alice, op(2, None, Op::NoteCreate(p.clone()))).await;
    let OpResult::Duplicate { candidates } = &r else {
        panic!("expected duplicate, got {r:?}");
    };
    assert_eq!(candidates.len(), 1);
    let c = &candidates[0];
    assert_eq!(
        (c.id.as_str(), c.kind, c.title.as_str(), c.level),
        (
            id(1).to_string().as_str(),
            DedupeKind::Note,
            "ETA invoice for Watanya",
            MatchLevel::Exact
        )
    );
    assert_eq!(c.score, 1.0);
    assert_eq!(h.log(uid).len(), commits, "nothing written");
    assert!(!h.exists(uid, "notes/Watanya ETA invoice.md"));
    // The device retries with force (a new op).
    p.force = true;
    let r = one(&h, &alice, op(3, None, Op::NoteCreate(p))).await;
    let created = h.read(uid, "notes/Watanya ETA invoice.md");
    assert_eq!(r, applied(&created));
    let sidecar = h.read(uid, &format!(".meta/notes/{}.json", id(2)));
    assert!(sidecar.contains(&id(1).to_string()), "{sidecar}");
    // The pair is never flagged again.
    let again = create(4, "notes/Invoice ETA Watanya.md", "Monthly invoice\n");
    let r = one(&h, &alice, op(4, None, again)).await;
    assert!(
        matches!(r, OpResult::Duplicate { .. }),
        "a third note is still checked: {r:?}"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_replayed_op_returns_the_stored_bytes_and_is_not_applied_again() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let ops_ = vec![
        op(1, None, create(1, "notes/A.md", "one\n")),
        op(
            2,
            None,
            Op::TaskCreate(o::TaskCreate {
                created: strata_common::clock::default_test_epoch(),
                home_id: None,
                id: "t-01j9replayaaaaaaaaaaaaaaaa".into(),
                note_id: None,
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
    ];
    let (first, first_bytes) = h.push(&alice, ops_.clone()).await;
    let commits = h.log(uid).len();
    let a = h.read(uid, "notes/A.md");
    // Change the note meanwhile: the replay must still answer the original result.
    one(
        &h,
        &alice,
        op(
            3,
            Some(version(&a)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: a.replace("one", "two"),
            }),
        ),
    )
    .await;
    let commits_after_edit = h.log(uid).len();
    assert_eq!(commits_after_edit, commits + 1);
    let (second, second_bytes) = h.push(&alice, ops_).await;
    assert_eq!(second, first);
    assert_eq!(second_bytes, first_bytes, "byte-identical replay");
    assert_eq!(h.log(uid).len(), commits_after_edit, "no second commit");
    // The same op twice in one push: applied once, answered twice identically.
    let dup = op(4, None, create(5, "notes/C.md", "c\n"));
    let (res, bytes) = h.push(&alice, vec![dup.clone(), dup]).await;
    assert_eq!(res.results[0], res.results[1]);
    assert_eq!(h.log(uid)[0], "user: create notes/C.md");
    assert_eq!(h.log(uid)[1], "user: update notes/A.md");
    // Stored as MessagePack under the op ID.
    let mut tx =
        h.db.app_db
            .begin(&h.db.issuer.issue(uid))
            .await
            .expect("tx");
    let stored = strata_index::repo::sync::idempotency_get(
        &mut tx,
        strata_common::OpId::from_ulid(op_id(4)),
    )
    .await
    .expect("get")
    .expect("stored");
    tx.commit().await.expect("commit");
    assert_eq!(
        stored.result,
        rmp_serde::to_vec_named(&res.results[0].result).expect("enc")
    );
    assert!(
        bytes
            .windows(stored.result.len())
            .any(|w| w == stored.result.as_slice())
    );
    h.finish().await;
}

#[tokio::test]
async fn ops_apply_in_order_within_one_push() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let content = "# Draft\n\nfirst\n";
    let edit = "# Draft\n\nfirst\nsecond\n";
    let (res, _) = h
        .push(
            &alice,
            vec![
                op(1, None, create(1, "notes/Draft.md", content)),
                // Based on the content the device just created (the server stamped it).
                op(
                    2,
                    Some(version(content)),
                    Op::NoteUpdate(o::NoteUpdate {
                        id: id(1),
                        content: edit.into(),
                    }),
                ),
                op(
                    3,
                    Some(version(edit)),
                    Op::NoteMove(o::NoteMove {
                        id: id(1),
                        new_path: "notes/Final.md".into(),
                    }),
                ),
                // Depends on the move having happened (forced: same title as before).
                op(
                    4,
                    None,
                    Op::NoteCreate(o::NoteCreate {
                        created: strata_common::clock::default_test_epoch(),
                        id: id(2),
                        path: "notes/Draft.md".into(),
                        content: "# Another draft\n".into(),
                        force: true,
                    }),
                ),
                // Invalid: the previous op must not stop later ones.
                op(
                    5,
                    Some(version("x")),
                    Op::NoteUpdate(o::NoteUpdate {
                        id: id(99),
                        content: "nothing".into(),
                    }),
                ),
                op(6, None, create(3, "notes/Last.md", "last\n")),
            ],
        )
        .await;
    let fin = h.read(uid, "notes/Final.md");
    assert_eq!(fin, format!("{}{edit}", header(&id(1).to_string())));
    let results: Vec<OpResult> = res.results.into_iter().map(|r| r.result).collect();
    let created1 = format!("{}{content}", header(&id(1).to_string()));
    assert_eq!(
        results,
        vec![
            applied(&created1),
            OpResult::Applied {
                new_version: Some(version(&fin)),
                merged: true
            },
            applied(&fin),
            applied(&h.read(uid, "notes/Draft.md")),
            not_found(),
            applied(&h.read(uid, "notes/Last.md")),
        ]
    );
    assert_eq!(
        h.log(uid)[..5].to_vec(),
        vec![
            "user: create notes/Last.md",
            "user: create notes/Draft.md",
            "user: move notes/Draft.md -> notes/Final.md",
            "user: update notes/Draft.md",
            "user: create notes/Draft.md",
        ]
    );
    h.finish().await;
}

#[tokio::test]
async fn invalid_envelopes_are_rejected_per_op() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let mut bad = op(1, None, create(1, "notes/A.md", "a\n"));
    bad.entity_id = "something-else".into();
    let missing_base = op(
        2,
        None,
        Op::NoteUpdate(o::NoteUpdate {
            id: id(1),
            content: "b".into(),
        }),
    );
    let (res, _) = h.push(&alice, vec![bad, missing_base]).await;
    let invalid = |detail: &str| OpResult::Rejected {
        problem: Problem {
            problem_type: "invalid_body".into(),
            title: "Request body is invalid".into(),
            status: 422,
            detail: Some(detail.into()),
        },
    };
    assert_eq!(
        res.results
            .into_iter()
            .map(|r| r.result)
            .collect::<Vec<_>>(),
        vec![
            invalid(&format!(
                "entity_id \"something-else\" does not match the payload (\"{}\")",
                id(1)
            )),
            invalid("`note.update` requires a base_version"),
        ]
    );
    h.finish().await;
}

#[tokio::test]
async fn another_users_ids_are_rejected_as_not_found_and_never_applied() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    one(
        &h,
        &alice,
        op(1, None, create(1, "notes/Secret.md", "alice's\n")),
    )
    .await;
    let secret = h.read(alice.id, "notes/Secret.md");
    one(
        &h,
        &alice,
        op(
            2,
            None,
            Op::EntityCreate(o::EntityCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(2),
                kind: NoteKind::Person,
                name: "Sam".into(),
                aliases: vec![],
                fields: Default::default(),
                force: false,
            }),
        ),
    )
    .await;
    let alice_commits = h.log(alice.id).len();
    let bob_commits = h.log(bob.id).len();
    // Bob's own note to point relations at.
    one(&h, &bob, op(3, None, create(3, "notes/Mine.md", "bob's\n"))).await;
    let foreign = vec![
        op(
            10,
            Some(version(&secret)),
            Op::NoteUpdate(o::NoteUpdate {
                id: id(1),
                content: "pwned".into(),
            }),
        ),
        op(
            11,
            Some(version(&secret)),
            Op::NoteMove(o::NoteMove {
                id: id(1),
                new_path: "notes/Moved.md".into(),
            }),
        ),
        op(
            12,
            Some(version(&secret)),
            Op::NoteDelete(o::NoteRef { id: id(1) }),
        ),
        op(
            13,
            None,
            Op::RelationAdd(o::RelationRef {
                src_id: id(3),
                dst_id: id(1),
                relation: "related".parse().expect("rel"),
            }),
        ),
        op(
            14,
            Some(version(&secret)),
            Op::EntityPatch(o::EntityPatch {
                id: id(2),
                ..o::EntityPatch::default()
            }),
        ),
        op(
            15,
            None,
            Op::EntityMerge(o::EntityMerge {
                id: id(2),
                into_id: id(2),
            }),
        ),
        op(16, None, Op::RelinkRequest(o::NoteRef { id: id(1) })),
        op(
            17,
            None,
            Op::DeviceSettings(o::DeviceSettings {
                device_id: alice.device,
                reminders_enabled: Some(false),
            }),
        ),
        op(
            18,
            None,
            Op::DocumentCustody(o::DocumentCustody {
                document_id: id(2),
                event: CustodyEventType::Lost,
                at: d(2026, 9, 1),
                place_id: None,
                person_id: None,
                counterparty_id: None,
            }),
        ),
    ];
    let n = foreign.len();
    let (res, _) = h.push(&bob, foreign).await;
    let results: Vec<OpResult> = res.results.into_iter().map(|r| r.result).collect();
    let merge_self = OpResult::Rejected {
        problem: Problem {
            problem_type: "invalid_body".into(),
            title: "Request body is invalid".into(),
            status: 422,
            detail: Some("an entity cannot be merged into itself".into()),
        },
    };
    let mut expected = vec![not_found(); n];
    expected[5] = merge_self;
    assert_eq!(results, expected);
    // Nothing of Alice's changed.
    assert_eq!(h.read(alice.id, "notes/Secret.md"), secret);
    assert_eq!(h.log(alice.id).len(), alice_commits);
    assert_eq!(h.log(bob.id).len(), bob_commits + 1);
    let devices = ops::list_devices(&alice.client).await.expect("devices");
    assert!(devices[0].reminders_enabled);
    // Bob's op IDs are his own: Alice may reuse one without seeing Bob's result.
    let r = one(&h, &alice, op(10, None, create(9, "notes/Other.md", "x\n"))).await;
    assert_eq!(r, applied(&h.read(alice.id, "notes/Other.md")));
    h.finish().await;
}

#[tokio::test]
async fn task_edits_use_the_devices_dates_and_ids_and_conflict_per_line() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let tid = "t-01j9recuraaaaaaaaaaaaaaaa".to_owned();
    let next = "t-01j9nextaaaaaaaaaaaaaaaaa".to_owned();
    one(
        &h,
        &alice,
        op(
            1,
            None,
            Op::TaskCreate(o::TaskCreate {
                created: strata_common::clock::default_test_epoch(),
                home_id: None,
                id: tid.clone(),
                note_id: None,
                text: "Pay rent".into(),
                due: Some(d(2026, 10, 1)),
                scheduled: None,
                start: None,
                recurrence: Some("every month on the 1st".into()),
                reminders: vec![],
                priority: None,
                force: false,
            }),
        ),
    )
    .await;
    let open = format!("- [ ] Pay rent 🔁 every month on the 1st 📅 2026-10-01 ^{tid}");
    assert!(h.read(uid, "tasks/Tasks.md").contains(&open));
    let r = one(
        &h,
        &alice,
        op(
            2,
            Some(version(&open)),
            Op::TaskComplete(o::TaskComplete {
                id: tid.clone(),
                done: d(2026, 9, 30),
                next_id: Some(next.clone()),
            }),
        ),
    )
    .await;
    let done =
        format!("- [x] Pay rent 🔁 every month on the 1st 📅 2026-10-01 ✅ 2026-09-30 ^{tid}");
    let upcoming = format!("- [ ] Pay rent 🔁 every month on the 1st 📅 2026-11-01 ^{next}");
    assert!(
        h.read(uid, "tasks/Tasks.md")
            .contains(&format!("{upcoming}\n{done}\n")),
        "{}",
        h.read(uid, "tasks/Tasks.md")
    );
    assert_eq!(r, applied(&done));
    // Another device completes the same occurrence from the old line: already in effect.
    let r = one(
        &h,
        &alice,
        op(
            3,
            Some(version(&open)),
            Op::TaskComplete(o::TaskComplete {
                id: tid.clone(),
                done: d(2026, 9, 30),
                next_id: Some("t-01j9otheraaaaaaaaaaaaaaaa".into()),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied(&done));
    // A text edit based on the old line conflicts at line level; the server line stands.
    let r = one(
        &h,
        &alice,
        op(
            4,
            Some(version(&open)),
            Op::TaskUpdate(o::TaskUpdate {
                id: tid.clone(),
                text: Some("Pay the rent".into()),
                ..o::TaskUpdate::default()
            }),
        ),
    )
    .await;
    assert_eq!(
        r,
        OpResult::Conflict {
            server_version: Some(version(&done)),
            resolution: ConflictResolution::ServerKept {
                reason: "the task line changed on the server".into()
            }
        }
    );
    assert!(h.read(uid, "tasks/Tasks.md").contains(&done));
    h.finish().await;
}

fn entity(n: u128, kind: NoteKind, name: &str) -> Op {
    Op::EntityCreate(o::EntityCreate {
        created: strata_common::clock::default_test_epoch(),
        id: id(n),
        kind,
        name: name.into(),
        aliases: vec![],
        fields: Default::default(),
        force: false,
    })
}

fn document(n: u128, name: &str, copy_of: Option<Ulid>, companies: &[u128], people: &[u128]) -> Op {
    Op::DocumentCreate(o::DocumentCreate {
        created: strata_common::clock::default_test_epoch(),
        id: id(n),
        name: name.into(),
        aliases: vec![],
        doc_type: Some("contract".into()),
        copy: Some(domain::CopyKind::Copy),
        copy_of,
        companies: companies.iter().map(|c| id(*c)).collect(),
        people: people.iter().map(|p| id(*p)).collect(),
        expires: None,
        force: false,
    })
}

#[tokio::test]
async fn document_create_writes_its_links_in_the_same_single_commit() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    one(
        &h,
        &alice,
        op(1, None, entity(1, NoteKind::Person, "Sam Hany")),
    )
    .await;
    one(
        &h,
        &alice,
        op(2, None, entity(2, NoteKind::Company, "Watanya")),
    )
    .await;
    one(
        &h,
        &alice,
        op(3, None, entity(3, NoteKind::Person, "Mona Adel")),
    )
    .await;
    one(
        &h,
        &alice,
        op(4, None, document(4, "Lease original", None, &[], &[])),
    )
    .await;
    let commits = h.log(uid).len();

    let r = one(
        &h,
        &alice,
        op(
            5,
            None,
            document(5, "Lease copy", Some(id(4)), &[2], &[1, 3]),
        ),
    )
    .await;
    let copy = h.read(uid, "documents/Lease copy.md");
    assert_eq!(
        copy,
        format!(
            "---\nid: {}\nkind: document\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:00:00+00:00\ndoc-type: contract\ncopy: copy\npeople: [\"[[Sam Hany]]\", \"[[Mona Adel]]\"]\ncompanies: [\"[[Watanya]]\"]\ncopy-of: [\"[[Lease original]]\"]\n---\n## Notes\n",
            id(5)
        )
    );
    assert_eq!(r, applied(&copy));
    assert_eq!(
        h.log(uid).len(),
        commits + 1,
        "one commit for the create and its links"
    );
    assert_eq!(h.log(uid)[0], "user: create documents/Lease copy.md");
    let mut paths = strata_vault::git::changed_paths(
        &h.dir(uid),
        &strata_vault::git::head(&h.dir(uid))
            .expect("head")
            .expect("commit")
            .id,
    )
    .expect("paths");
    paths.sort();
    assert_eq!(paths, vec!["documents/Lease copy.md".to_owned()]);
    // The links are indexed as relations of the new document.
    let related = ops::get_document(&alice.client, id(5))
        .await
        .expect("document");
    assert_eq!(related.document.copy_of, Some(id(4)));
    let original = ops::get_document(&alice.client, id(4))
        .await
        .expect("document");
    assert_eq!(original.copies, vec![id(5).to_string()]);

    // A link to a missing note refuses the whole create: nothing is written.
    let r = one(
        &h,
        &alice,
        op(6, None, document(6, "Orphan copy", Some(id(99)), &[], &[1])),
    )
    .await;
    assert_eq!(r, not_found());
    assert_eq!(h.log(uid).len(), commits + 1);
    assert!(!h.exists(uid, "documents/Orphan copy.md"));
    h.finish().await;
}

/// A semantic source that reports one stored note as a near-certain paraphrase.
struct Paraphrase(dedupe::Item);

impl strata_vault::semantic::SemanticDuplicates for Paraphrase {
    fn evidence<'a>(
        &'a self,
        _tx: &'a mut strata_index::ScopedTx,
        _item: &'a dedupe::Item,
    ) -> futures_util::future::BoxFuture<'a, Vec<strata_vault::semantic::SemanticMatch>> {
        Box::pin(async move {
            vec![strata_vault::semantic::SemanticMatch {
                item: self.0.clone(),
                cosine: 0.99,
            }]
        })
    }
}

#[tokio::test]
async fn semantic_duplicates_are_answered_with_the_semantic_level() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    one(
        &h,
        &alice,
        op(
            1,
            None,
            create(1, "notes/Quarterly budget review.md", "Numbers for Q3\n"),
        ),
    )
    .await;
    let text = h.read(uid, "notes/Quarterly budget review.md");
    let stored = strata_vault::dup::note_item(
        "notes/Quarterly budget review.md",
        Some(strata_common::NoteId::from_ulid(id(1))),
        &vault_format::Document::parse(&text),
    );
    h.vault
        .set_semantic(std::sync::Arc::new(Paraphrase(stored)));
    let commits = h.log(uid).len();
    let r = one(
        &h,
        &alice,
        op(
            2,
            None,
            create(2, "notes/Money planning.md", "Spending plan\n"),
        ),
    )
    .await;
    let OpResult::Duplicate { candidates } = &r else {
        panic!("expected duplicate, got {r:?}");
    };
    assert_eq!(
        candidates
            .iter()
            .map(|c| (c.id.clone(), c.kind, c.title.clone(), c.level))
            .collect::<Vec<_>>(),
        vec![(
            id(1).to_string(),
            DedupeKind::Note,
            "Quarterly budget review".to_owned(),
            MatchLevel::Semantic
        )]
    );
    assert_eq!(candidates[0].score, 0.99);
    assert_eq!(h.log(uid).len(), commits, "nothing written");
    h.finish().await;
}

/// `suggestion.accept` with edits is `POST /suggestions/{id}/accept-with-edits`: a filing
/// accepted with an edited title, tags and folder is filed in one `user:` commit; edits on a
/// kind without them are rejected and nothing is written; `relink.request` on a capture
/// queues a forced `file_inbox` run.
#[tokio::test]
async fn suggestion_accept_applies_edits_and_relink_forces_a_run() {
    use strata_common::{NoteId, SuggestionId};
    use sync_model::suggestions::{FilingPayload, kinds};

    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let scope = h.db.issuer.issue(uid);
    let at = Utc
        .with_ymd_and_hms(2026, 9, 26, 8, 30, 5)
        .single()
        .expect("time")
        .fixed_offset();
    // A user folder to file into, and a capture.
    let (res, _) = h
        .push(
            &alice,
            vec![
                op(1, None, create(1, "notes/Clients/Acme.md", "# Acme\n")),
                op(
                    2,
                    None,
                    Op::Capture(o::Capture {
                        id: id(2),
                        text: "Watanya wants ETA invoices monthly".into(),
                        created: at,
                    }),
                ),
            ],
        )
        .await;
    assert!(
        res.results
            .iter()
            .all(|r| matches!(r.result, OpResult::Applied { .. })),
        "{res:?}"
    );
    let capture = "inbox/2026-09-26-083005.md";
    let captured = h.read(uid, capture);
    // The filing job's proposal (as `file_inbox` stores it).
    let filing = SuggestionId::from_ulid(id(3));
    let payload = sync_model::SuggestionPayload::from(FilingPayload {
        decision_id: id(4),
        title: "Watanya invoices".into(),
        tags: vec!["watanya".into()],
        folder: "notes".into(),
    });
    h.vault
        .create_suggestion(
            &scope,
            filing,
            Some(NoteId::from_ulid(id(2))),
            kinds::FILING,
            &payload.to_bytes(),
        )
        .await
        .expect("filing suggestion");

    // Edits on a kind that has none (a `duplicate` flag) are rejected; nothing is written.
    let dup = SuggestionId::from_ulid(id(5));
    h.vault
        .create_suggestion(
            &scope,
            dup,
            Some(NoteId::from_ulid(id(2))),
            kinds::DUPLICATE,
            &sync_model::SuggestionPayload::from(sync_model::suggestions::DuplicatePayload {
                candidates: vec![],
            })
            .to_bytes(),
        )
        .await
        .expect("duplicate suggestion");
    let before = h.log(uid);
    let r = one(
        &h,
        &alice,
        op(
            3,
            None,
            Op::SuggestionAccept(o::SuggestionAccept {
                created: strata_common::clock::default_test_epoch(),
                id: id(5),
                edits: Some(o::SuggestionEdits {
                    title: Some("Other".into()),
                    ..Default::default()
                }),
            }),
        ),
    )
    .await;
    assert_eq!(
        r,
        OpResult::Rejected {
            problem: Problem {
                problem_type: "invalid_body".into(),
                title: "Request body is invalid".into(),
                status: 422,
                detail: Some("edits are not supported for this suggestion kind".into()),
            }
        }
    );
    assert_eq!(h.log(uid), before);

    // Accepting the filing with edits: title, tags and folder, one commit.
    let r = one(
        &h,
        &alice,
        op(
            4,
            None,
            Op::SuggestionAccept(o::SuggestionAccept {
                created: strata_common::clock::default_test_epoch(),
                id: id(3),
                edits: Some(o::SuggestionEdits {
                    title: Some("Watanya ETA invoicing".into()),
                    tags: Some(vec!["watanya".into(), "invoices".into()]),
                    folder: Some("notes/Clients".into()),
                    ..Default::default()
                }),
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    let log = h.log(uid);
    assert_eq!(log.len(), before.len() + 1);
    assert_eq!(
        log[0],
        format!("user: accept filing {capture} -> notes/Clients/Watanya ETA invoicing.md")
    );
    assert!(!h.exists(uid, capture));
    assert_eq!(
        h.read(uid, "notes/Clients/Watanya ETA invoicing.md"),
        captured.replace("\ncreated: ", "\ntags: [watanya, invoices]\ncreated: ")
    );
    let accepted = ops::list_suggestions(&alice.client, Some(&types::SuggestionStatus::Accepted))
        .await
        .expect("accepted");
    assert_eq!(
        accepted.items.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![id(3)]
    );
    // A replay answers the stored result and writes nothing.
    let r = one(
        &h,
        &alice,
        op(
            4,
            None,
            Op::SuggestionAccept(o::SuggestionAccept {
                created: strata_common::clock::default_test_epoch(),
                id: id(3),
                edits: None,
            }),
        ),
    )
    .await;
    assert_eq!(r, applied_none());
    assert_eq!(h.log(uid).len(), log.len());

    // relink.request on a capture queues a forced `file_inbox` run; on a note a forced `link`.
    let (res, _) = h
        .push(
            &alice,
            vec![
                op(
                    5,
                    None,
                    Op::Capture(o::Capture {
                        id: id(6),
                        text: "Ask Shady about the contract".into(),
                        created: at + chrono::Duration::minutes(1),
                    }),
                ),
                op(6, None, Op::RelinkRequest(o::NoteRef { id: id(6) })),
                op(7, None, Op::RelinkRequest(o::NoteRef { id: id(1) })),
                op(8, None, Op::RelinkRequest(o::NoteRef { id: id(99) })),
            ],
        )
        .await;
    assert_eq!(
        res.results
            .iter()
            .skip(1)
            .map(|r| r.result.clone())
            .collect::<Vec<_>>(),
        vec![applied_none(), applied_none(), not_found()]
    );
    let mut tx = h.db.app_db.begin(&scope).await.expect("tx");
    let queued: Vec<(String, uuid::Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT kind, note_id, payload FROM jobs \
         WHERE kind IN ('link', 'file_inbox') AND status = 'queued' AND note_id = ANY($1) \
         ORDER BY kind, note_id",
    )
    .bind(vec![uuid::Uuid::from(id(1)), uuid::Uuid::from(id(6))])
    .fetch_all(tx.conn())
    .await
    .expect("jobs");
    tx.commit().await.expect("commit");
    assert_eq!(
        queued,
        vec![
            (
                "file_inbox".to_owned(),
                uuid::Uuid::from(id(6)),
                forced_link_payload()
            ),
            (
                "link".to_owned(),
                uuid::Uuid::from(id(1)),
                forced_link_payload()
            ),
        ]
    );
    h.finish().await;
}
