//! Job queue: enqueue/debounce, claim order, complete/fail/retry, stale recovery, wakeup hints,
//! and exactly-once claiming under concurrency.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::Duration;
use pretty_assertions::assert_eq;
use strata_common::{Clock, JobId, NoteId, UserId};
use strata_index::repo::jobs::{self, Job, NewJob};
use strata_index::types::JobStatus;
use strata_testkit::{TestDb, TestUser, default_test_epoch};
use tokio::sync::Barrier;

fn new_job(db: &TestDb, kind: &str, run_after: chrono::DateTime<chrono::Utc>) -> NewJob {
    NewJob {
        id: JobId::generate(db.ids.as_ref()),
        kind: kind.to_owned(),
        note_id: None,
        payload: vec![0x81, 0xa1, 0x61, 0x01],
        run_after,
        max_attempts: 3,
        dedupe_key: None,
    }
}

async fn wakeups(db: &TestDb) -> Vec<(UserId, chrono::DateTime<chrono::Utc>)> {
    sqlx::query_as("SELECT user_id, run_after FROM strata.job_wakeups ORDER BY user_id")
        .fetch_all(&db.superuser)
        .await
        .expect("wakeups")
}

#[tokio::test]
async fn enqueue_returns_the_full_row_and_sets_the_wakeup() {
    let db = TestDb::new().await.expect("db");
    let u = TestUser::new("alice").create(&db).await.expect("u").id;
    let now = db.clock.now();
    let mut tx = db.begin(u).await.expect("tx");
    let mut spec = new_job(&db, "embed", now + Duration::seconds(5));
    let note = NoteId::generate(db.ids.as_ref());
    spec.note_id = Some(note);
    let job = jobs::enqueue(&mut tx, &spec, now).await.expect("enqueue");
    tx.commit().await.expect("commit");
    assert_eq!(
        job,
        Job {
            id: spec.id,
            kind: "embed".into(),
            note_id: Some(note),
            payload: vec![0x81, 0xa1, 0x61, 0x01],
            status: JobStatus::Queued,
            attempts: 0,
            max_attempts: 3,
            run_after: now + Duration::seconds(5),
            locked_at: None,
            last_error: None,
            dedupe_key: None,
            created: now,
            updated: now,
        }
    );
    assert_eq!(wakeups(&db).await, vec![(u, now + Duration::seconds(5))]);
    // Not due yet.
    assert_eq!(db.app_db.due_job_users(now, 10).await.expect("due"), vec![]);
    assert_eq!(
        db.app_db
            .due_job_users(now + Duration::seconds(5), 10)
            .await
            .expect("due"),
        vec![u]
    );
}

#[tokio::test]
async fn debounced_enqueue_updates_the_queued_job_instead_of_adding_one() {
    let db = TestDb::new().await.expect("db");
    let u = TestUser::new("alice").create(&db).await.expect("u").id;
    let t0 = db.clock.now();
    let mut tx = db.begin(u).await.expect("tx");
    let mut first = new_job(&db, "link", t0 + Duration::seconds(30));
    first.dedupe_key = Some("note:1".into());
    let a = jobs::enqueue(&mut tx, &first, t0).await.expect("first");
    let mut second = new_job(&db, "link", t0 + Duration::seconds(40));
    second.dedupe_key = Some("note:1".into());
    second.payload = vec![0xc0];
    let b = jobs::enqueue(&mut tx, &second, t0 + Duration::seconds(10))
        .await
        .expect("second");
    assert_eq!(b.id, a.id);
    assert_eq!(
        (b.run_after, b.payload.clone(), b.updated),
        (
            t0 + Duration::seconds(40),
            vec![0xc0],
            t0 + Duration::seconds(10)
        )
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(tx.conn())
        .await
        .expect("count");
    assert_eq!(count, 1);
    // Once running, the same key queues a fresh job.
    let claimed = jobs::claim_next(&mut tx, t0 + Duration::seconds(40))
        .await
        .expect("claim")
        .expect("job");
    assert_eq!(claimed.id, a.id);
    let c = jobs::enqueue(&mut tx, &second, t0 + Duration::seconds(41))
        .await
        .expect("third");
    assert_eq!(c.id, second.id);
    assert_ne!(c.id, a.id);
}

#[tokio::test]
async fn claim_takes_the_earliest_due_job_and_complete_finishes_it() {
    let db = TestDb::new().await.expect("db");
    let u = TestUser::new("alice").create(&db).await.expect("u").id;
    let t0 = db.clock.now();
    let mut tx = db.begin(u).await.expect("tx");
    let late = jobs::enqueue(
        &mut tx,
        &new_job(&db, "embed", t0 + Duration::seconds(10)),
        t0,
    )
    .await
    .expect("late");
    let early = jobs::enqueue(&mut tx, &new_job(&db, "embed", t0), t0)
        .await
        .expect("early");
    let claimed = jobs::claim_next(&mut tx, t0)
        .await
        .expect("claim")
        .expect("due job");
    assert_eq!(claimed.id, early.id);
    assert_eq!(
        (claimed.status, claimed.attempts, claimed.locked_at),
        (JobStatus::Running, 1, Some(t0))
    );
    assert_eq!(
        jobs::claim_next(&mut tx, t0).await.expect("claim"),
        None,
        "late job is not due"
    );
    assert!(
        jobs::complete(&mut tx, early.id, t0 + Duration::seconds(1))
            .await
            .expect("complete")
    );
    assert!(
        !jobs::complete(&mut tx, early.id, t0 + Duration::seconds(1))
            .await
            .expect("again")
    );
    let done = jobs::get_job(&mut tx, early.id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(
        (done.status, done.locked_at, done.updated),
        (JobStatus::Done, None, t0 + Duration::seconds(1))
    );
    let next = jobs::claim_next(&mut tx, t0 + Duration::seconds(10))
        .await
        .expect("claim")
        .expect("late");
    assert_eq!(next.id, late.id);
}

#[tokio::test]
async fn fail_requeues_with_backoff_until_attempts_run_out() {
    let db = TestDb::new().await.expect("db");
    let u = TestUser::new("alice").create(&db).await.expect("u").id;
    let t0 = db.clock.now();
    let mut tx = db.begin(u).await.expect("tx");
    let mut spec = new_job(&db, "summarize", t0);
    spec.max_attempts = 2;
    let job = jobs::enqueue(&mut tx, &spec, t0).await.expect("enqueue");

    jobs::claim_next(&mut tx, t0)
        .await
        .expect("claim")
        .expect("job");
    let retry_at = t0 + Duration::seconds(60);
    let failed = jobs::fail(&mut tx, job.id, "provider timeout", Some(retry_at), t0)
        .await
        .expect("fail")
        .expect("running");
    assert_eq!(
        (
            failed.status,
            failed.attempts,
            failed.run_after,
            failed.last_error.as_deref(),
            failed.locked_at
        ),
        (
            JobStatus::Queued,
            1,
            retry_at,
            Some("provider timeout"),
            None
        )
    );
    assert_eq!(
        jobs::claim_next(&mut tx, t0).await.expect("claim"),
        None,
        "backoff respected"
    );

    jobs::claim_next(&mut tx, retry_at)
        .await
        .expect("claim")
        .expect("retry");
    let dead = jobs::fail(
        &mut tx,
        job.id,
        "still down",
        Some(retry_at + Duration::seconds(60)),
        retry_at,
    )
    .await
    .expect("fail")
    .expect("running");
    assert_eq!(
        (dead.status, dead.attempts, dead.last_error.as_deref()),
        (JobStatus::Failed, 2, Some("still down"))
    );
    assert_eq!(
        jobs::fail(&mut tx, job.id, "x", None, retry_at)
            .await
            .expect("fail"),
        None,
        "not running"
    );
}

#[tokio::test]
async fn stale_claims_are_requeued() {
    let db = TestDb::new().await.expect("db");
    let u = TestUser::new("alice").create(&db).await.expect("u").id;
    let t0 = db.clock.now();
    let mut tx = db.begin(u).await.expect("tx");
    let job = jobs::enqueue(&mut tx, &new_job(&db, "embed", t0), t0)
        .await
        .expect("enqueue");
    jobs::claim_next(&mut tx, t0)
        .await
        .expect("claim")
        .expect("job");
    assert_eq!(
        jobs::requeue_stale(&mut tx, t0, t0 + Duration::minutes(10))
            .await
            .expect("none stale"),
        0
    );
    let later = t0 + Duration::minutes(10);
    assert_eq!(
        jobs::requeue_stale(&mut tx, t0 + Duration::minutes(5), later)
            .await
            .expect("stale"),
        1
    );
    let j = jobs::get_job(&mut tx, job.id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(
        (j.status, j.run_after, j.locked_at, j.last_error.as_deref()),
        (JobStatus::Queued, later, None, Some("claim expired"))
    );
}

#[tokio::test]
async fn wakeup_hints_track_each_users_earliest_queued_job() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let b = TestUser::new("bob").create(&db).await.expect("b").id;
    let t0 = db.clock.now();
    for (u, offset) in [(a, 20), (a, 10), (b, 5)] {
        let mut tx = db.begin(u).await.expect("tx");
        jobs::enqueue(
            &mut tx,
            &new_job(&db, "embed", t0 + Duration::seconds(offset)),
            t0,
        )
        .await
        .expect("enqueue");
        tx.commit().await.expect("commit");
    }
    assert_eq!(
        wakeups(&db).await,
        vec![
            (a, t0 + Duration::seconds(10)),
            (b, t0 + Duration::seconds(5))
        ]
    );
    assert_eq!(
        db.app_db
            .due_job_users(t0 + Duration::seconds(30), 10)
            .await
            .expect("due"),
        vec![b, a]
    );

    // A drains one job; refresh raises the hint to the next one, then deletes it when empty.
    let mut tx = db.begin(a).await.expect("tx");
    let j = jobs::claim_next(&mut tx, t0 + Duration::seconds(30))
        .await
        .expect("claim")
        .expect("job");
    assert_eq!(
        jobs::refresh_wakeup(&mut tx).await.expect("refresh"),
        Some(t0 + Duration::seconds(20))
    );
    jobs::complete(&mut tx, j.id, t0).await.expect("complete");
    tx.commit().await.expect("commit");
    assert_eq!(wakeups(&db).await[0], (a, t0 + Duration::seconds(20)));
    let mut tx = db.begin(a).await.expect("tx");
    jobs::claim_next(&mut tx, t0 + Duration::seconds(30))
        .await
        .expect("claim")
        .expect("job");
    assert_eq!(jobs::refresh_wakeup(&mut tx).await.expect("refresh"), None);
    tx.commit().await.expect("commit");
    assert_eq!(wakeups(&db).await, vec![(b, t0 + Duration::seconds(5))]);

    // A scoped claim never sees another user's jobs.
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(
        jobs::claim_next(&mut tx, t0 + Duration::days(1))
            .await
            .expect("claim"),
        None
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_workers_claim_each_job_exactly_once() {
    const JOBS: usize = 60;
    const WORKERS: usize = 8;
    let db = TestDb::with_pool_size(u32::try_from(WORKERS).expect("small"))
        .await
        .expect("db");
    let users = [
        TestUser::new("alice").create(&db).await.expect("a").id,
        TestUser::new("bob").create(&db).await.expect("b").id,
    ];
    let t0 = default_test_epoch();
    let mut expected = BTreeSet::new();
    for (i, u) in (0..JOBS).map(|i| (i, users[i % 2])) {
        let mut tx = db.begin(u).await.expect("tx");
        let spec = new_job(
            &db,
            "embed",
            t0 + Duration::milliseconds(i64::try_from(i).expect("small")),
        );
        expected.insert(jobs::enqueue(&mut tx, &spec, t0).await.expect("enqueue").id);
        tx.commit().await.expect("commit");
    }

    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut handles = Vec::new();
    for w in 0..WORKERS {
        let app = db.app_db.clone();
        let issuer = db.issuer.clone();
        let barrier = barrier.clone();
        let user = users[w % 2];
        handles.push(tokio::spawn(async move {
            let scope = issuer.issue(user);
            barrier.wait().await;
            let mut mine = Vec::new();
            loop {
                let mut tx = app.begin(&scope).await.expect("tx");
                let claimed = jobs::claim_next(&mut tx, t0 + Duration::hours(1))
                    .await
                    .expect("claim");
                tx.commit().await.expect("commit");
                match claimed {
                    Some(job) => {
                        assert_eq!(job.attempts, 1, "a job was claimed twice");
                        mine.push(job.id);
                    }
                    None => break,
                }
            }
            mine
        }));
    }
    let mut claimed = Vec::new();
    for h in handles {
        claimed.extend(h.await.expect("worker"));
    }
    assert_eq!(claimed.len(), JOBS, "every job claimed, none twice");
    assert_eq!(claimed.iter().copied().collect::<BTreeSet<_>>(), expected);
    let running: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM strata.jobs WHERE status = 'running' AND attempts = 1",
    )
    .fetch_one(&db.superuser)
    .await
    .expect("count");
    assert_eq!(running, i64::try_from(JOBS).expect("small"));
}
