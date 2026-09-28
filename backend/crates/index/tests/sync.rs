//! Change log, epochs and idempotency.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use std::collections::BTreeSet;

use chrono::Duration;
use pretty_assertions::assert_eq;
use strata_common::{Clock, DeviceId, OpId};
use strata_index::IndexError;
use strata_index::repo::sync::{self, Change, NewChange, SyncPosition};
use strata_index::types::ChangeOp;
use strata_testkit::{TestDb, TestUser};

fn change(id: &str, at: chrono::DateTime<chrono::Utc>) -> NewChange<'_> {
    NewChange {
        entity_type: "note",
        entity_id: id,
        op: ChangeOp::Upsert,
        version: Some("v1"),
        at,
    }
}

#[tokio::test]
async fn seq_starts_at_one_and_is_gapless_per_user() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let b = TestUser::new("bob").create(&db).await.expect("b").id;
    let t = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(
        sync::sync_position(&mut tx).await.expect("pos"),
        SyncPosition {
            epoch: 1,
            last_seq: 0
        }
    );
    let first = sync::append_change(&mut tx, &change("n1", t))
        .await
        .expect("append");
    assert_eq!(
        first,
        Change {
            seq: 1,
            epoch: 1,
            entity_type: "note".into(),
            entity_id: "n1".into(),
            op: ChangeOp::Upsert,
            version: Some("v1".into()),
            at: t,
        }
    );
    let del = NewChange {
        op: ChangeOp::Delete,
        version: None,
        ..change("n1", t)
    };
    assert_eq!(
        sync::append_change(&mut tx, &del)
            .await
            .expect("append")
            .seq,
        2
    );
    tx.commit().await.expect("commit");

    // B has an independent sequence.
    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(
        sync::append_change(&mut tx, &change("m1", t))
            .await
            .expect("append")
            .seq,
        1
    );
    tx.commit().await.expect("commit");

    let mut tx = db.begin(a).await.expect("tx");
    let all = sync::changes_since(&mut tx, 1, 0, 100)
        .await
        .expect("changes");
    assert_eq!(
        all.iter().map(|c| (c.seq, c.op)).collect::<Vec<_>>(),
        vec![(1, ChangeOp::Upsert), (2, ChangeOp::Delete)]
    );
    assert_eq!(
        sync::changes_since(&mut tx, 1, 1, 100)
            .await
            .expect("since 1")
            .len(),
        1
    );
    assert_eq!(
        sync::changes_since(&mut tx, 1, 0, 1)
            .await
            .expect("limit")
            .len(),
        1
    );
    assert_eq!(
        sync::sync_position(&mut tx).await.expect("pos"),
        SyncPosition {
            epoch: 1,
            last_seq: 2
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_appends_get_unique_consecutive_seqs() {
    const WRITERS: i64 = 24;
    let db = TestDb::with_pool_size(8).await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let t = db.clock.now();
    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let app = db.app_db.clone();
        let scope = db.scope(a);
        handles.push(tokio::spawn(async move {
            let id = format!("n{i}");
            let mut tx = app.begin(&scope).await.expect("tx");
            let c = sync::append_change(&mut tx, &change(&id, t))
                .await
                .expect("append");
            tx.commit().await.expect("commit");
            c.seq
        }));
    }
    let mut seqs = BTreeSet::new();
    for h in handles {
        assert!(seqs.insert(h.await.expect("writer")), "duplicate seq");
    }
    assert_eq!(seqs, (1..=WRITERS).collect::<BTreeSet<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_later_append_waits_for_the_earlier_commit_so_readers_never_skip() {
    let db = TestDb::with_pool_size(4).await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let t = db.clock.now();

    let mut first = db.begin(a).await.expect("tx1");
    assert_eq!(
        sync::append_change(&mut first, &change("n1", t))
            .await
            .expect("append")
            .seq,
        1
    );

    let app = db.app_db.clone();
    let scope = db.scope(a);
    let second = tokio::spawn(async move {
        let mut tx = app.begin(&scope).await.expect("tx2");
        let seq = sync::append_change(&mut tx, &change("n2", t))
            .await
            .expect("append")
            .seq;
        tx.commit().await.expect("commit");
        seq
    });
    // Explicit synchronisation (no sleeps): wait until the second writer is blocked on a lock.
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() \
             AND usename = 'strata_app' AND wait_event_type = 'Lock'",
        )
        .fetch_one(&db.superuser)
        .await
        .expect("activity");
        if waiting == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    // While the first is uncommitted, a reader sees nothing — and nothing with seq 2 exists.
    let mut reader = db.begin(a).await.expect("reader");
    assert_eq!(
        sync::changes_since(&mut reader, 1, 0, 10)
            .await
            .expect("read"),
        vec![]
    );
    reader.commit().await.expect("commit");

    first.commit().await.expect("commit 1");
    assert_eq!(second.await.expect("writer 2"), 2);
    let mut reader = db.begin(a).await.expect("reader");
    let seqs: Vec<i64> = sync::changes_since(&mut reader, 1, 0, 10)
        .await
        .expect("read")
        .iter()
        .map(|c| c.seq)
        .collect();
    assert_eq!(seqs, vec![1, 2]);
}

#[tokio::test]
async fn epoch_bump_invalidates_old_cursors_and_keeps_seq_monotonic() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let t = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    for id in ["n1", "n2", "n3"] {
        sync::append_change(&mut tx, &change(id, t))
            .await
            .expect("append");
    }
    let pos = sync::bump_epoch(&mut tx, t + Duration::minutes(1))
        .await
        .expect("bump");
    assert_eq!(
        pos,
        SyncPosition {
            epoch: 2,
            last_seq: 3
        }
    );
    match sync::changes_since(&mut tx, 1, 0, 10).await {
        Err(IndexError::EpochChanged { current }) => assert_eq!(current, 2),
        other => panic!("expected EpochChanged, got {other:?}"),
    }
    assert_eq!(
        sync::changes_since(&mut tx, 2, 0, 10)
            .await
            .expect("new epoch"),
        vec![]
    );
    let next = sync::append_change(&mut tx, &change("n4", t))
        .await
        .expect("append");
    assert_eq!((next.seq, next.epoch), (4, 2));
    let old_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM change_log WHERE epoch = 1")
        .fetch_one(tx.conn())
        .await
        .expect("count");
    assert_eq!(old_rows, 0);
}

#[tokio::test]
async fn idempotency_put_keeps_the_first_result() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let b = TestUser::new("bob").create(&db).await.expect("b").id;
    let t = db.clock.now();
    let op = OpId::generate(db.ids.as_ref());
    let device = DeviceId::generate(db.ids.as_ref());
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(sync::idempotency_get(&mut tx, op).await.expect("get"), None);
    let first = sync::idempotency_put(&mut tx, op, device, &[0x91, 0x01], t)
        .await
        .expect("put");
    let replay = sync::idempotency_put(&mut tx, op, device, &[0xff], t + Duration::seconds(5))
        .await
        .expect("replay");
    assert_eq!(replay, first);
    assert_eq!(
        (first.result.as_slice(), first.created),
        (&[0x91u8, 0x01][..], t)
    );
    assert_eq!(
        sync::idempotency_get(&mut tx, op).await.expect("get"),
        Some(first)
    );
    tx.commit().await.expect("commit");

    // The same op_id under another user is a different key (and invisible across users).
    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(sync::idempotency_get(&mut tx, op).await.expect("get"), None);
    sync::idempotency_put(&mut tx, op, device, &[0x02], t)
        .await
        .expect("put");
    assert_eq!(
        sync::idempotency_purge_before(&mut tx, t + Duration::seconds(1))
            .await
            .expect("purge"),
        1
    );
    tx.commit().await.expect("commit");
    let mut tx = db.begin(a).await.expect("tx");
    assert!(
        sync::idempotency_get(&mut tx, op)
            .await
            .expect("get")
            .is_some(),
        "A's record untouched"
    );
}

#[tokio::test]
async fn vault_head_is_per_user_and_leaves_the_sync_position_alone() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let b = TestUser::new("bob").create(&db).await.expect("b").id;
    let t = db.clock.now();
    let (h1, h2) = ("a".repeat(40), format!("{}0", "b".repeat(39)));
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(sync::vault_head(&mut tx).await.expect("head"), None);
    // Before any change: the row is created at epoch 1, seq 0.
    sync::set_vault_head(&mut tx, &h1, t).await.expect("set");
    assert_eq!(
        sync::vault_head(&mut tx).await.expect("head"),
        Some(h1.clone())
    );
    assert_eq!(
        sync::sync_position(&mut tx).await.expect("pos"),
        SyncPosition {
            epoch: 1,
            last_seq: 0
        }
    );
    let first = sync::append_change(&mut tx, &change("n1", t))
        .await
        .expect("append");
    assert_eq!(first.seq, 1);
    sync::set_vault_head(&mut tx, &h2, t).await.expect("set");
    assert_eq!(
        sync::vault_head(&mut tx).await.expect("head"),
        Some(h2.clone())
    );
    assert_eq!(
        sync::sync_position(&mut tx).await.expect("pos"),
        SyncPosition {
            epoch: 1,
            last_seq: 1
        }
    );
    tx.commit().await.expect("commit");

    // Another user's head is invisible and separate.
    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(sync::vault_head(&mut tx).await.expect("head"), None);
    // Only full commit IDs are stored.
    let err = sync::set_vault_head(&mut tx, "HEAD", t)
        .await
        .expect_err("not a commit id");
    assert!(err.to_string().contains("check"), "{err}");
    tx.rollback().await.expect("rollback");
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(sync::vault_head(&mut tx).await.expect("head"), Some(h2));
    tx.commit().await.expect("commit");
}
