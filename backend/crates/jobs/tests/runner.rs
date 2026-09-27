//! The job runner (PLAN §5.2 fair scheduling, §9.2): fairness across users, exactly-once
//! claiming under concurrency, retries with exponential backoff, pauses that wait, debounce,
//! events, unhandled kinds, the daily limit, graceful shutdown, and the scheduler — all with
//! the fake clock.
#![allow(clippy::expect_used, clippy::too_many_lines, clippy::many_single_char_names)]

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use common::World;
use pretty_assertions::assert_eq;
use strata_ai::PauseReason;
use strata_common::{Clock, JobId, UserId};
use strata_index::UserScope;
use strata_index::repo::jobs::{self, NewJob};
use strata_jobs::{
    JobClass, JobContext, JobError, JobHandler, JobNotice, JobOutcome, Runner, RunnerConfig,
    Scheduler, StaticUsers,
};

/// Plays the outcome listed in the payload for each attempt (`ok` after the list ends).
#[derive(Debug)]
struct Script {
    kind: &'static str,
    class: JobClass,
    max: usize,
    log: Arc<Mutex<Vec<(UserId, JobId, i32)>>>,
    gate: Option<Arc<tokio::sync::Semaphore>>,
    started: Option<Arc<tokio::sync::Notify>>,
    until: Option<DateTime<Utc>>,
}

impl Script {
    fn new(kind: &'static str, class: JobClass) -> Self {
        Self {
            kind,
            class,
            max: 8,
            log: Arc::default(),
            gate: None,
            started: None,
            until: None,
        }
    }

    fn runs(&self) -> Vec<(UserId, JobId, i32)> {
        self.log.lock().expect("log").clone()
    }
}

#[async_trait::async_trait]
impl JobHandler for Script {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn class(&self) -> JobClass {
        self.class
    }
    fn max_concurrency(&self) -> usize {
        self.max
    }
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        self.log
            .lock()
            .expect("log")
            .push((ctx.scope.user_id(), ctx.job.id, ctx.job.attempts));
        if let Some(s) = &self.started {
            s.notify_one();
        }
        if let Some(g) = &self.gate {
            g.acquire().await.expect("gate").forget();
        }
        let plan: Vec<String> = rmp_serde::from_slice(&ctx.job.payload).unwrap_or_default();
        let step = usize::try_from(ctx.job.attempts - 1).expect("attempts ≥ 1");
        // A pause does not spend an attempt: it holds until `until`, then the job succeeds.
        let paused = self.until.is_some_and(|u| ctx.now < u);
        match plan.get(step).map_or("ok", String::as_str) {
            "pause-user" | "pause-provider" if !paused => Ok(()),
            "retry" => Err(JobError::Retry("flaky".into())),
            "fatal" => Err(JobError::Fatal("broken".into())),
            "pause-user" => Err(JobError::Paused {
                reason: PauseReason::UserBudget,
                until: self.until,
            }),
            "pause-provider" => Err(JobError::Paused {
                reason: PauseReason::ProviderUsageLimit,
                until: self.until,
            }),
            _ => Ok(()),
        }
    }
}

async fn enqueue(
    w: &World,
    scope: &UserScope,
    kind: &str,
    plan: &[&str],
    key: Option<&str>,
    max_attempts: i32,
) -> JobId {
    let mut tx = w.db.app_db.begin(scope).await.expect("tx");
    let now = w.db.clock.now();
    let job = jobs::enqueue(
        &mut tx,
        &NewJob {
            id: JobId::generate(w.db.ids.as_ref()),
            kind: kind.to_owned(),
            note_id: None,
            payload: rmp_serde::to_vec(&plan).expect("payload"),
            run_after: now,
            max_attempts,
            dedupe_key: key.map(str::to_owned),
        },
        now,
    )
    .await
    .expect("enqueue");
    tx.commit().await.expect("commit");
    job.id
}

async fn job(w: &World, scope: &UserScope, id: JobId) -> jobs::Job {
    let mut tx = w.db.app_db.begin(scope).await.expect("tx");
    let j = jobs::get_job(&mut tx, id).await.expect("get").expect("job");
    tx.commit().await.expect("commit");
    j
}

fn one_at_a_time() -> RunnerConfig {
    RunnerConfig {
        max_concurrency: 1,
        ..RunnerConfig::default()
    }
}

fn users_of(runs: &[(UserId, JobId, i32)]) -> Vec<UserId> {
    runs.iter().map(|r| r.0).collect()
}

#[tokio::test]
async fn a_flooding_user_does_not_starve_another() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let (b, sb) = w.user("bob").await;
    for _ in 0..12 {
        enqueue(&w, &sa, "work", &[], None, 3).await;
    }
    w.db.clock.advance(chrono::Duration::seconds(1));
    for _ in 0..3 {
        enqueue(&w, &sb, "work", &[], None, 3).await;
    }
    let script = Arc::new(Script::new("work", JobClass::Light));
    let runner = w.runner(one_at_a_time(), vec![script.clone()]);
    // One job per pass (capacity 1): users alternate although Alice queued first and more.
    for _ in 0..8 {
        assert_eq!(runner.tick().await.len(), 1);
        runner.drain().await;
    }
    assert_eq!(users_of(&script.runs()), vec![a, b, a, b, a, b, a, a]);
    // Wider capacity: each round gives every due user one slot.
    let runner = w.runner(
        RunnerConfig {
            max_concurrency: 4,
            ..RunnerConfig::default()
        },
        vec![script.clone()],
    );
    let claimed: Vec<UserId> = runner.tick().await.into_iter().map(|c| c.user).collect();
    assert_eq!(claimed, vec![a, a, a, a]);
    runner.drain().await;
    runner.run_until_idle().await;
    let runs = script.runs();
    assert_eq!(runs.len(), 15);
    assert_eq!(runs.iter().filter(|r| r.0 == b).count(), 3);
    assert_eq!(runner.tick().await, vec![]);
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_runners_claim_every_job_exactly_once() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let (_, sb) = w.user("bob").await;
    let mut all = Vec::new();
    for i in 0..30 {
        let s = if i % 3 == 0 { &sb } else { &sa };
        all.push(enqueue(&w, s, "work", &[], None, 3).await);
    }
    let script = Arc::new(Script::new("work", JobClass::Light));
    let config = RunnerConfig {
        max_concurrency: 4,
        ..RunnerConfig::default()
    };
    let r1 = w.runner(config.clone(), vec![script.clone()]);
    let r2 = w.runner(config.clone(), vec![script.clone()]);
    let r3 = w.runner(config, vec![script.clone()]);
    let (c1, c2, c3) = tokio::join!(r1.run_until_idle(), r2.run_until_idle(), r3.run_until_idle());
    let mut ran: Vec<JobId> = script.runs().into_iter().map(|r| r.1).collect();
    ran.sort();
    all.sort();
    assert_eq!(ran, all, "every job ran exactly once");
    assert_eq!(c1.len() + c2.len() + c3.len(), 30);
    let done: Vec<String> = w
        .jobs(sa.user_id())
        .await
        .into_iter()
        .chain(w.jobs(sb.user_id()).await)
        .map(|j| j.1)
        .collect();
    assert_eq!(done, vec!["done".to_owned(); 30]);
    assert_eq!(w.events.notices().len(), 30);
    w.finish().await;
}

#[tokio::test]
async fn retries_back_off_exponentially_then_fail_with_an_event() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let flaky = enqueue(&w, &sa, "work", &["retry", "retry", "ok"], None, 5).await;
    let doomed = enqueue(&w, &sa, "work", &["retry", "retry"], None, 2).await;
    let script = Arc::new(Script::new("work", JobClass::Light));
    let runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    let t0 = w.db.clock.now();

    runner.run_until_idle().await;
    let j = job(&w, &sa, flaky).await;
    assert_eq!(
        (j.status.as_str(), j.attempts, j.run_after, j.last_error.as_deref()),
        ("queued", 1, t0 + chrono::Duration::seconds(30), Some("flaky"))
    );
    // Not due yet: nothing runs.
    w.db.clock.advance(chrono::Duration::seconds(29));
    assert_eq!(runner.run_until_idle().await, vec![]);
    w.db.clock.advance(chrono::Duration::seconds(1));
    runner.run_until_idle().await;
    let j = job(&w, &sa, flaky).await;
    assert_eq!(
        (j.status.as_str(), j.attempts, j.run_after),
        ("queued", 2, t0 + chrono::Duration::seconds(30 + 60))
    );
    // The two-attempt job failed for good on its second run.
    let d = job(&w, &sa, doomed).await;
    assert_eq!((d.status.as_str(), d.attempts), ("failed", 2));
    w.db.clock.advance(chrono::Duration::seconds(60));
    runner.run_until_idle().await;
    let j = job(&w, &sa, flaky).await;
    assert_eq!((j.status.as_str(), j.attempts, j.last_error), ("done", 3, None));
    assert_eq!(
        w.events.notices(),
        vec![
            (
                a,
                JobNotice {
                    id: doomed,
                    kind: "work".into(),
                    note_id: None,
                    outcome: JobOutcome::Failed
                }
            ),
            (
                a,
                JobNotice {
                    id: flaky,
                    kind: "work".into(),
                    note_id: None,
                    outcome: JobOutcome::Completed
                }
            ),
        ]
    );
    // A fatal error fails at once, attempts left or not.
    let fatal = enqueue(&w, &sa, "work", &["fatal"], None, 5).await;
    runner.run_until_idle().await;
    let f = job(&w, &sa, fatal).await;
    assert_eq!(
        (f.status.as_str(), f.attempts, f.last_error.as_deref()),
        ("failed", 1, Some("broken"))
    );
    w.finish().await;
}

#[tokio::test]
async fn a_paused_job_waits_without_spending_an_attempt_and_resumes() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let (_, sb) = w.user("bob").await;
    let until = w.db.clock.now() + chrono::Duration::hours(1);
    let mut script = Script::new("llm", JobClass::Llm);
    script.until = Some(until);
    let script = Arc::new(script);
    let light = Arc::new(Script::new("light", JobClass::Light));
    let runner = w.runner(RunnerConfig::default(), vec![script.clone(), light.clone()]);
    let paused = enqueue(&w, &sa, "llm", &["pause-user", "ok"], None, 1).await;
    runner.run_until_idle().await;
    let j = job(&w, &sa, paused).await;
    assert_eq!(
        (j.status.as_str(), j.attempts, j.run_after, j.last_error.as_deref()),
        ("queued", 0, until, Some("paused: UserBudget"))
    );
    assert_eq!(
        runner.llm_pause(a).map(|p| (p.reason, p.until)),
        Some((PauseReason::UserBudget, until))
    );
    // While Alice is paused her other LLM jobs wait; her light jobs and Bob's LLM jobs run.
    let later = enqueue(&w, &sa, "llm", &[], None, 3).await;
    let bobs = enqueue(&w, &sb, "llm", &[], None, 3).await;
    let light_job = enqueue(&w, &sa, "light", &[], None, 3).await;
    runner.run_until_idle().await;
    let ran: Vec<JobId> = script.runs().into_iter().map(|r| r.1).collect();
    assert_eq!(ran, vec![paused, bobs]);
    assert_eq!(light.runs().into_iter().map(|r| r.1).collect::<Vec<_>>(), vec![light_job]);
    assert_eq!(job(&w, &sa, later).await.status.as_str(), "queued");
    // After the pause both run; the paused job still had its single attempt.
    w.db.clock.set(until);
    runner.run_until_idle().await;
    assert_eq!(job(&w, &sa, paused).await.status.as_str(), "done");
    assert_eq!(job(&w, &sa, later).await.status.as_str(), "done");
    assert_eq!(runner.llm_pause(a), None);
    w.finish().await;
}

#[tokio::test]
async fn a_provider_pause_holds_every_users_llm_jobs() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let (b, sb) = w.user("bob").await;
    let until = w.db.clock.now() + chrono::Duration::minutes(30);
    let mut script = Script::new("llm", JobClass::Llm);
    script.until = Some(until);
    let script = Arc::new(script);
    let runner = w.runner(one_at_a_time(), vec![script.clone()]);
    enqueue(&w, &sa, "llm", &["pause-provider"], None, 3).await;
    runner.run_until_idle().await;
    let bobs = enqueue(&w, &sb, "llm", &[], None, 3).await;
    runner.run_until_idle().await;
    assert_eq!(job(&w, &sb, bobs).await.status.as_str(), "queued");
    assert_eq!(
        runner.llm_pause(b).map(|p| p.reason),
        Some(PauseReason::ProviderUsageLimit)
    );
    w.db.clock.set(until);
    runner.run_until_idle().await;
    assert_eq!(job(&w, &sb, bobs).await.status.as_str(), "done");
    w.finish().await;
}

#[tokio::test]
async fn debounced_jobs_run_once_after_the_last_enqueue() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let first = enqueue(&w, &sa, "work", &[], Some("note-1"), 3).await;
    w.db.clock.advance(chrono::Duration::seconds(10));
    let second = enqueue(&w, &sa, "work", &[], Some("note-1"), 3).await;
    assert_eq!(first, second, "the queued job was re-timed, not duplicated");
    let script = Arc::new(Script::new("work", JobClass::Light));
    let runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    runner.run_until_idle().await;
    assert_eq!(script.runs().len(), 1);
    assert_eq!(w.jobs(sa.user_id()).await, vec![("work".into(), "done".into(), 1)]);
    // Once it ran, the same key queues a new job.
    let third = enqueue(&w, &sa, "work", &[], Some("note-1"), 3).await;
    assert_ne!(third, first);
    w.finish().await;
}

#[tokio::test]
async fn jobs_of_kinds_without_a_handler_stay_queued_and_the_user_is_skipped_for_a_while() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    enqueue(&w, &sa, "file_inbox", &[], None, 3).await;
    let script = Arc::new(Script::new("work", JobClass::Light));
    let runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    assert_eq!(runner.run_until_idle().await, vec![]);
    // Skipped for idle_recheck (60 s): a new job of a handled kind waits until then.
    let later = enqueue(&w, &sa, "work", &[], None, 3).await;
    assert_eq!(runner.run_until_idle().await, vec![]);
    w.db.clock.advance(chrono::Duration::seconds(60));
    let claimed = runner.run_until_idle().await;
    assert_eq!(claimed.iter().map(|c| c.id).collect::<Vec<_>>(), vec![later]);
    assert_eq!(
        w.jobs(sa.user_id()).await,
        vec![
            ("file_inbox".into(), "queued".into(), 0),
            ("work".into(), "done".into(), 1)
        ]
    );
    w.finish().await;
}

#[tokio::test]
async fn per_kind_limits_and_the_daily_llm_limit_hold() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let (_, sb) = w.user("bob").await;
    for s in [&sa, &sb, &sa, &sb] {
        enqueue(&w, s, "embed_like", &[], None, 3).await;
    }
    let mut embed = Script::new("embed_like", JobClass::Embed);
    embed.max = 1;
    let embed = Arc::new(embed);
    let runner = w.runner(
        RunnerConfig {
            max_concurrency: 4,
            ..RunnerConfig::default()
        },
        vec![embed.clone()],
    );
    // One at a time: each pass claims a single embedding job.
    assert_eq!(runner.tick().await.len(), 1);
    runner.drain().await;
    assert_eq!(runner.run_until_idle().await.len(), 3);

    let llm = Arc::new(Script::new("llm", JobClass::Llm));
    let runner = w.runner(
        RunnerConfig {
            daily_llm_jobs: 2,
            ..RunnerConfig::default()
        },
        vec![llm.clone()],
    );
    for s in [&sa, &sb, &sa] {
        enqueue(&w, s, "llm", &[], None, 3).await;
    }
    assert_eq!(runner.run_until_idle().await.len(), 2);
    // The next UTC day resets the count.
    w.db.clock.advance(chrono::Duration::days(1));
    assert_eq!(runner.run_until_idle().await.len(), 1);
    w.finish().await;
}

#[tokio::test]
async fn shutdown_waits_for_running_jobs_and_stale_claims_are_requeued_at_start() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let started = Arc::new(tokio::sync::Notify::new());
    let mut script = Script::new("work", JobClass::Light);
    script.gate = Some(gate.clone());
    script.started = Some(started.clone());
    let script = Arc::new(script);
    let runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    // A job released during shutdown: shutdown waits for it, nothing is abandoned.
    let quick = enqueue(&w, &sa, "work", &[], None, 3).await;
    let handle = runner.start_loop();
    started.notified().await;
    gate.add_permits(1);
    assert_eq!(handle.shutdown(Duration::from_secs(60)).await, 0);
    assert_eq!(job(&w, &sa, quick).await.status.as_str(), "done");

    // A job that never finishes: shutdown gives up after the grace period and leaves it
    // `running` (whatever the timing, the outcome is the same: it is blocked for good).
    let stuck = enqueue(&w, &sa, "work", &[], None, 3).await;
    let handle = runner.start_loop();
    started.notified().await;
    assert_eq!(handle.shutdown(Duration::from_millis(50)).await, 1);
    assert_eq!(job(&w, &sa, stuck).await.status.as_str(), "running");
    // The next process re-queues it and it runs again.
    let next = w.runner(RunnerConfig::default(), vec![script.clone()]);
    assert_eq!(next.recover_stale(&[a]).await, 1);
    gate.add_permits(1);
    next.run_until_idle().await;
    let j = job(&w, &sa, stuck).await;
    assert_eq!((j.status.as_str(), j.attempts), ("done", 2));
    w.finish().await;
}

#[tokio::test]
async fn the_scheduler_keeps_one_nightly_job_per_user() {
    let w = World::new().await;
    let (a, _) = w.user("alice").await;
    let (b, _) = w.user("bob").await;
    let scheduler = Scheduler::new(
        w.db.app_db.clone(),
        w.db.issuer.clone(),
        Arc::new(w.db.clock.clone()),
        w.db.ids.clone(),
        Arc::new(StaticUsers(vec![a, b])),
        "Africa/Cairo".parse().expect("tz"),
        3,
        strata_jobs::standard_periodic(),
    );
    assert_eq!(scheduler.ensure().await, 2);
    assert_eq!(scheduler.ensure().await, 0, "idempotent");
    let mut tx = w.db.begin(a).await.expect("tx");
    let rows: Vec<(String, DateTime<Utc>, Option<String>)> =
        sqlx::query_as("SELECT kind, run_after, dedupe_key FROM jobs")
            .fetch_all(tx.conn())
            .await
            .expect("jobs");
    tx.commit().await.expect("commit");
    // The test epoch is 2026-09-27T12:00Z; 03:00 in Cairo (UTC+3) is 00:00Z.
    assert_eq!(
        rows,
        vec![(
            "dedupe".to_owned(),
            "2026-09-28T00:00:00Z".parse().expect("time"),
            Some("nightly".to_owned())
        )]
    );
    // Backfill fan-out, debounced per user.
    assert_eq!(scheduler.enqueue_for_all("embed_backfill", "backfill").await, 2);
    assert_eq!(scheduler.enqueue_for_all("embed_backfill", "backfill").await, 2);
    let kinds: BTreeMap<String, usize> =
        w.jobs(b).await.into_iter().fold(BTreeMap::new(), |mut m, j| {
            *m.entry(j.0).or_insert(0) += 1;
            m
        });
    assert_eq!(
        kinds,
        BTreeMap::from([("dedupe".to_owned(), 1), ("embed_backfill".to_owned(), 1)])
    );
    w.finish().await;
}

#[tokio::test]
async fn runner_is_a_no_op_without_due_work() {
    let w = World::new().await;
    let script = Arc::new(Script::new("work", JobClass::Light));
    let runner: Runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    assert_eq!(runner.kinds(), vec!["work"]);
    assert_eq!(runner.run_until_idle().await, vec![]);
    assert_eq!(runner.running(), 0);
    w.finish().await;
}
