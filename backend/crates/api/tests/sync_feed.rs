//! `GET /sync/bootstrap` and `GET /sync/changes` (PLAN §7.4, §7.5 Sync, §12.4, §16.3):
//! stable paging under concurrent writes, exact change records with tombstones, `410` on an
//! epoch change, and isolation of the feeds.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod sync_harness;

use std::collections::BTreeSet;

use domain::{NoteKind, RelationOrigin};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use sync_harness::{Device, H, User, version};
use sync_model::changes::{NoteRecord, RelationRecord};
use sync_model::ops::{self as o, Op};
use sync_model::{ChangeRecord, ChangesPage, EntityType, OpResult, Record, SyncOp, Version};
use ulid::Ulid;

const OP_BASE: u128 = 0x0199_0000_0000_0000_0000_0000_0000_0000;
const ID_BASE: u128 = 0x0199_1111_0000_0000_0000_0000_0000_0000;

fn id(n: u128) -> Ulid {
    Ulid(ID_BASE + n)
}

fn op(n: u128, base: Option<Version>, op: Op) -> SyncOp {
    SyncOp::new(Ulid(OP_BASE + n), base, op)
}

fn create(n: u128, path: &str, body: &str) -> Op {
    Op::NoteCreate(o::NoteCreate {
        id: id(n),
        path: path.into(),
        content: body.into(),
        force: true,
    })
}

async fn push_ok(h: &H, user: &User, ops_: Vec<SyncOp>) {
    let (res, _) = h.push(user, ops_).await;
    for r in res.results {
        assert!(
            matches!(r.result, OpResult::Applied { .. }),
            "{:?}",
            r.result
        );
    }
}

fn note(h: &H, user: &User, n: u128, path: &str) -> Record {
    let content = h.read(user.id, path);
    Record::Note(NoteRecord {
        id: id(n),
        path: path.into(),
        version: Version::of_text(&content),
        content,
        kind: NoteKind::Note,
        summary: None,
    })
}

#[tokio::test]
async fn bootstrap_pages_are_stable_under_concurrent_writes() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    for n in 1..=6 {
        push_ok(
            &h,
            &alice,
            vec![op(n, None, create(n, &format!("notes/N{n}.md"), &format!("note {n}\n")))],
        )
        .await;
    }
    push_ok(
        &h,
        &alice,
        vec![op(
            7,
            None,
            Op::RelationAdd(o::RelationRef {
                src_id: id(1),
                dst_id: id(2),
                relation: "related".parse().expect("rel"),
            }),
        )],
    )
    .await;

    // Page 1 captures the log position.
    let first = h
        .bootstrap_page(&alice, None, Some(3))
        .await
        .expect("page 1");
    let seq = first.seq;
    assert_eq!(first.epoch, 1);
    assert_eq!(first.records.len(), 3);
    assert_eq!(
        first.records,
        vec![
            note(&h, &alice, 1, "notes/N1.md"),
            note(&h, &alice, 2, "notes/N2.md"),
            note(&h, &alice, 3, "notes/N3.md"),
        ]
    );
    let mut device = Device::default();
    device.apply_page(&first);

    // Concurrent writes: before and after the paging position.
    let n5 = h.read(alice.id, "notes/N5.md");
    let n1 = h.read(alice.id, "notes/N1.md");
    let n2 = h.read(alice.id, "notes/N2.md");
    push_ok(
        &h,
        &alice,
        vec![
            op(10, None, create(7, "notes/N7.md", "note 7\n")),
            op(
                11,
                Some(version(&n5)),
                Op::NoteUpdate(o::NoteUpdate {
                    id: id(5),
                    content: n5.replace("note 5", "note 5 edited"),
                }),
            ),
            op(
                12,
                Some(version(&h.read(alice.id, "notes/N6.md"))),
                Op::NoteDelete(o::NoteRef { id: id(6) }),
            ),
            op(
                13,
                Some(version(&n1)),
                Op::NoteUpdate(o::NoteUpdate {
                    id: id(1),
                    content: n1.replace("note 1", "note 1 edited"),
                }),
            ),
            op(
                14,
                Some(version(&n2)),
                Op::NoteMove(o::NoteMove {
                    id: id(2),
                    new_path: "archive/N2.md".into(),
                }),
            ),
        ],
    )
    .await;

    // The rest of the pages continue from the cursor, with the same snapshot position.
    let mut keys: Vec<(EntityType, String)> = first
        .records
        .iter()
        .map(|r| (r.entity_type(), r.entity_id()))
        .collect();
    let mut cursor = first.next_cursor.clone();
    let mut pages = 1;
    while let Some(c) = cursor {
        let page = h
            .bootstrap_page(&alice, Some(&c), Some(3))
            .await
            .expect("page");
        assert_eq!((page.epoch, page.seq), (1, seq));
        keys.extend(page.records.iter().map(|r| (r.entity_type(), r.entity_id())));
        device.apply_page(&page);
        cursor = page.next_cursor;
        pages += 1;
    }
    assert!(pages >= 3, "{pages}");
    // Nothing twice; every note that existed throughout exactly once.
    let unique: BTreeSet<_> = keys.iter().cloned().collect();
    assert_eq!(unique.len(), keys.len(), "{keys:?}");
    for n in 1..=5 {
        assert!(unique.contains(&(EntityType::Note, id(n).to_string())), "N{n}");
    }
    // The later pages show the current state of what they cover.
    assert_eq!(
        device.records.get(&(EntityType::Note, id(5).to_string())),
        Some(&note(&h, &alice, 5, "notes/N5.md"))
    );
    assert!(!unique.contains(&(EntityType::Note, id(6).to_string())));

    // Pulling the changes after the snapshot converges on the server state.
    h.pull(&alice, &mut device).await;
    let fresh = h.bootstrap(&alice, None).await;
    assert_eq!(device.state(), fresh.state());
    assert_eq!(device.cursor, fresh.cursor);
    assert_eq!(
        device.notes().keys().cloned().collect::<Vec<_>>(),
        vec![
            "archive/N2.md",
            "notes/N1.md",
            "notes/N3.md",
            "notes/N4.md",
            "notes/N5.md",
            "notes/N7.md"
        ]
    );
    // The generated client decodes the same pages.
    let page = ops::sync_bootstrap(&alice.client, None, None)
        .await
        .expect("generated bootstrap");
    assert_eq!(page.seq, fresh.cursor.expect("cursor").seq);
    assert_eq!(page.records.len(), fresh.records.len());
    h.finish().await;
}

#[tokio::test]
async fn changes_since_a_seq_are_exact_records_with_tombstones() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    push_ok(
        &h,
        &alice,
        vec![
            op(1, None, create(1, "notes/A.md", "a\n")),
            op(2, None, create(3, "notes/C.md", "c\n")),
        ],
    )
    .await;
    let device = h.bootstrap(&alice, None).await;
    let start = device.cursor.expect("cursor");
    let a = h.read(alice.id, "notes/A.md");
    push_ok(
        &h,
        &alice,
        vec![
            op(
                3,
                Some(version(&a)),
                Op::NoteUpdate(o::NoteUpdate {
                    id: id(1),
                    content: a.replace("a\n", "a2\n"),
                }),
            ),
            op(4, None, create(2, "notes/B.md", "b\n")),
            op(
                5,
                None,
                Op::RelationAdd(o::RelationRef {
                    src_id: id(1),
                    dst_id: id(2),
                    relation: "related".parse().expect("rel"),
                }),
            ),
        ],
    )
    .await;
    let b = h.read(alice.id, "notes/B.md");
    push_ok(
        &h,
        &alice,
        vec![
            op(6, Some(version(&b)), Op::NoteDelete(o::NoteRef { id: id(2) })),
            op(
                7,
                None,
                Op::RelationAdd(o::RelationRef {
                    src_id: id(3),
                    dst_id: id(1),
                    relation: "supports".parse().expect("rel"),
                }),
            ),
        ],
    )
    .await;
    let s = start.seq;
    let a_now = note(&h, &alice, 1, "notes/A.md");
    let c_now = note(&h, &alice, 3, "notes/C.md");
    let related = format!("{}:related:{}", id(1), id(2));
    // Log: A (update), B (create), A (relation) + relation, B (trash) + relation removed,
    // C (relation) + relation.
    let page1 = h
        .changes_page(&alice, s, start.epoch, Some(2))
        .await
        .expect("page 1");
    assert_eq!(
        page1,
        ChangesPage {
            epoch: 1,
            changes: vec![
                // Records are the current state; B is in the trash now.
                ChangeRecord::upsert(s + 1, 1, a_now.clone()),
                ChangeRecord::delete(s + 2, 1, EntityType::Note, &id(2).to_string()),
            ],
            next_seq: s + 2,
            has_more: true,
        }
    );
    let page2 = h
        .changes_page(&alice, s + 2, start.epoch, None)
        .await
        .expect("page 2");
    let supports = Record::Relation(RelationRecord {
        src_id: id(3),
        dst_id: id(1),
        relation: "supports".parse().expect("rel"),
        by: RelationOrigin::User,
        confidence: None,
        reason: None,
        created: Some(strata_testkit::default_test_epoch().fixed_offset()),
    });
    assert_eq!(
        page2,
        ChangesPage {
            epoch: 1,
            changes: vec![
                // The newest row of A in this page only.
                ChangeRecord::upsert(s + 3, 1, a_now),
                ChangeRecord::delete(s + 5, 1, EntityType::Note, &id(2).to_string()),
                ChangeRecord::delete(s + 6, 1, EntityType::Relation, &related),
                ChangeRecord::upsert(s + 7, 1, c_now),
                ChangeRecord::upsert(s + 8, 1, supports),
            ],
            next_seq: s + 8,
            has_more: false,
        }
    );
    // Nothing new: an empty page at the same position.
    let empty = h
        .changes_page(&alice, s + 8, 1, None)
        .await
        .expect("empty");
    assert_eq!(
        empty,
        ChangesPage {
            epoch: 1,
            changes: vec![],
            next_seq: s + 8,
            has_more: false,
        }
    );
    // The generated client decodes the page too.
    let generated = ops::sync_changes(&alice.client, s, 1, None)
        .await
        .expect("generated changes");
    assert_eq!(generated.next_seq, s + 8);
    assert_eq!(generated.changes.len(), 5);
    assert!(matches!(
        generated.changes[1].change,
        types::SyncChange::Delete
    ));
    h.finish().await;
}

#[tokio::test]
async fn an_epoch_change_answers_410_and_a_new_bootstrap_starts_over() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    push_ok(&h, &alice, vec![op(1, None, create(1, "notes/A.md", "a\n"))]).await;
    let first = h
        .bootstrap_page(&alice, None, Some(1))
        .await
        .expect("page");
    let cursor = first.next_cursor.clone().expect("more pages");
    let mut tx = h
        .db
        .app_db
        .begin(&h.db.issuer.issue(alice.id))
        .await
        .expect("tx");
    let pos = strata_index::repo::sync::bump_epoch(&mut tx, strata_testkit::default_test_epoch())
        .await
        .expect("bump");
    tx.commit().await.expect("commit");
    assert_eq!(pos.epoch, 2);
    let gone = |bytes: &[u8]| -> types::Problem { rmp_serde::from_slice(bytes).expect("problem") };
    let expected = types::Problem {
        candidates: vec![],
        current_version: None,
        detail: Some("the sync epoch is now 2: bootstrap again".into()),
        errors: vec![],
        instance: None,
        status: 410,
        title: "Sync epoch changed".into(),
        type_: "epoch_changed".into(),
    };
    let (status, bytes) = h
        .changes_page(&alice, first.seq, 1, None)
        .await
        .expect_err("410");
    assert_eq!((status, gone(&bytes)), (410, expected.clone()));
    let (status, bytes) = h
        .bootstrap_page(&alice, Some(&cursor), Some(1))
        .await
        .expect_err("410");
    assert_eq!((status, gone(&bytes)), (410, expected));
    // A malformed cursor is a parameter error.
    let (status, bytes) = h
        .bootstrap_page(&alice, Some("not-a-cursor"), None)
        .await
        .expect_err("422");
    assert_eq!(status, 422);
    assert_eq!(gone(&bytes).type_, "invalid_parameter");
    let device = h.bootstrap(&alice, None).await;
    assert_eq!(device.cursor.expect("cursor").epoch, 2);
    assert_eq!(device.notes().len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn feeds_never_include_another_users_data() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let bob_start = h.bootstrap(&bob, None).await;
    push_ok(
        &h,
        &alice,
        vec![
            op(1, None, create(1, "notes/Same title.md", "alice\n")),
            op(
                2,
                None,
                Op::Capture(o::Capture {
                    id: id(2),
                    text: "alice capture".into(),
                    created: strata_testkit::default_test_epoch().fixed_offset(),
                }),
            ),
        ],
    )
    .await;
    push_ok(&h, &bob, vec![op(1, None, create(9, "notes/Same title.md", "bob\n"))]).await;
    let mut bob_device = bob_start.clone();
    h.pull(&bob, &mut bob_device).await;
    assert_eq!(
        bob_device.notes().values().map(|n| n.id).collect::<Vec<_>>(),
        vec![id(9)]
    );
    let bob_fresh = h.bootstrap(&bob, None).await;
    assert_eq!(bob_fresh.state(), bob_device.state());
    for r in bob_fresh.records.values() {
        let text = format!("{r:?}");
        assert!(!text.contains("alice"), "{text}");
        assert!(!text.contains(&id(1).to_string()), "{text}");
    }
    // Bob's feed positions are his own.
    assert_eq!(
        (bob_start.cursor.expect("c").seq, bob_device.cursor.expect("c").seq),
        (0, 1)
    );
    h.finish().await;
}
