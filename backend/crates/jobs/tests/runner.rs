//! The job runner (PLAN §5.2 fair scheduling, §9.2): fairness across users, exactly-once
//! claiming under concurrency, retries with exponential backoff, pauses that wait, debounce,
//! events, unhandled kinds, the daily limit, graceful shutdown, and the scheduler — all with
//! the fake clock.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

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
use strata_index::types::JobStatus;
use strata_jobs::{
    JobClass, JobContext, JobError, JobHandler, JobNotice, JobOutcome, JobStart, Runner,
    RunnerConfig, Scheduler, StaticUsers,
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
    interactive: bool,
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
            interactive: false,
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
    fn interactive(&self) -> bool {
        self.interactive
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
            "down" => Err(JobError::Provider("provider unavailable: exit 1".into())),
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
    let (c1, c2, c3) = tokio::join!(
        r1.run_until_idle(),
        r2.run_until_idle(),
        r3.run_until_idle()
    );
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
        (
            j.status.as_str(),
            j.attempts,
            j.run_after,
            j.last_error.as_deref()
        ),
        (
            "queued",
            1,
            t0 + chrono::Duration::seconds(30),
            Some("flaky")
        )
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
    assert_eq!(
        (j.status.as_str(), j.attempts, j.last_error),
        ("done", 3, None)
    );
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
        (
            j.status.as_str(),
            j.attempts,
            j.run_after,
            j.last_error.as_deref()
        ),
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
    assert_eq!(
        light.runs().into_iter().map(|r| r.1).collect::<Vec<_>>(),
        vec![light_job]
    );
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
    assert_eq!(
        w.jobs(sa.user_id()).await,
        vec![("work".into(), "done".into(), 1)]
    );
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
    assert_eq!(
        claimed.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![later]
    );
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
        vec![strata_jobs::Periodic {
            kind: strata_jobs::dedupe_sweep::DEDUPE,
            cadence: strata_jobs::Cadence::Nightly,
        }],
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
    assert_eq!(
        scheduler
            .enqueue_for_all("embed_backfill", "backfill")
            .await,
        2
    );
    assert_eq!(
        scheduler
            .enqueue_for_all("embed_backfill", "backfill")
            .await,
        2
    );
    let kinds: BTreeMap<String, usize> =
        w.jobs(b)
            .await
            .into_iter()
            .fold(BTreeMap::new(), |mut m, j| {
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

#[tokio::test]
async fn jobs_that_failed_while_the_provider_was_down_run_again_once_it_works() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let (b, sb) = w.user("bob").await;
    let a_down = enqueue(&w, &sa, "summarize", &["down", "down"], None, 2).await;
    let b_down = enqueue(&w, &sb, "summarize", &["down", "down"], None, 2).await;
    let other = enqueue(&w, &sb, "summarize", &["fatal"], None, 2).await;
    let script = Arc::new(Script::new("summarize", JobClass::Llm));
    let runner = w.runner(RunnerConfig::default(), vec![script.clone()]);
    runner.run_until_idle().await;
    w.db.clock.advance(chrono::Duration::seconds(30));
    runner.run_until_idle().await;
    for (scope, id) in [(&sa, a_down), (&sb, b_down), (&sb, other)] {
        assert_eq!(job(&w, scope, id).await.status, JobStatus::Failed);
    }
    let failed = |scope: UserScope, provider_only: bool| {
        let db = w.db.app_db.clone();
        async move {
            let mut tx = db.begin(&scope).await.expect("tx");
            let n = jobs::retryable_failed(&mut tx, provider_only)
                .await
                .expect("count");
            tx.commit().await.expect("commit");
            n
        }
    };
    assert_eq!(
        failed(sb, true).await,
        1,
        "the fatal failure is not a provider failure"
    );
    assert_eq!(failed(sb, false).await, 2);

    // The provider works again: Alice's next AI job succeeds and her failed job is queued
    // again with fresh attempts; Bob's waits for the scheduler's pass.
    w.db.clock.advance(chrono::Duration::minutes(5));
    let ok = enqueue(&w, &sa, "summarize", &[], None, 2).await;
    assert_eq!(runner.tick().await.len(), 1);
    runner.drain().await;
    let now = w.db.clock.now();
    assert_eq!(job(&w, &sa, ok).await.status, JobStatus::Done);
    let requeued = job(&w, &sa, a_down).await;
    assert_eq!(
        (requeued.status, requeued.attempts, requeued.run_after),
        (JobStatus::Queued, 0, now)
    );
    assert_eq!(runner.recovery().last_success(), Some(now));
    assert_eq!(job(&w, &sb, b_down).await.status, JobStatus::Failed);

    let scheduler = Scheduler::new(
        w.db.app_db.clone(),
        w.db.issuer.clone(),
        Arc::new(w.db.clock.clone()),
        w.db.ids.clone(),
        Arc::new(StaticUsers(vec![a, b])),
        "Africa/Cairo".parse().expect("tz"),
        3,
        vec![],
    );
    assert_eq!(
        scheduler.requeue_recovered().await,
        0,
        "no recovery signal wired"
    );
    let scheduler = scheduler.with_recovery(runner.recovery());
    assert_eq!(scheduler.requeue_recovered().await, 1);
    assert_eq!(job(&w, &sb, b_down).await.status, JobStatus::Queued);
    assert_eq!(job(&w, &sb, other).await.status, JobStatus::Failed);
    assert_eq!(scheduler.requeue_recovered().await, 0);
    w.finish().await;
}

#[tokio::test]
async fn interactive_jobs_go_first_and_always_have_a_slot() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    // Long background work is queued first and holds the only regular slot.
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let mut slow = Script::new("digest_like", JobClass::Llm);
    slow.gate = Some(gate.clone());
    let slow = Arc::new(slow);
    let mut filing = Script::new("file_like", JobClass::Llm);
    filing.interactive = true;
    let filing = Arc::new(filing);
    let runner = w.runner(one_at_a_time(), vec![slow.clone(), filing.clone()]);
    let digest = enqueue(&w, &sa, "digest_like", &[], None, 3).await;
    enqueue(&w, &sa, "digest_like", &[], None, 3).await;
    let kinds =
        |c: Vec<strata_jobs::runner::Claimed>| c.into_iter().map(|c| c.kind).collect::<Vec<_>>();
    assert_eq!(kinds(runner.tick().await), ["digest_like"]);

    // A capture's filing arrives: it runs in the reserved slot while the digest still runs,
    // and the second background job still waits.
    let file = enqueue(&w, &sa, "file_like", &[], None, 3).await;
    assert_eq!(kinds(runner.tick().await), ["file_like"]);
    assert_eq!(kinds(runner.tick().await), Vec::<String>::new());
    assert_eq!(
        w.events.starts(),
        [(
            a,
            JobStart {
                id: file,
                kind: "file_like".into(),
                note_id: None,
            }
        )],
        "only interactive jobs announce their start"
    );
    gate.add_permits(2);
    runner.drain().await;
    assert_eq!(kinds(runner.run_until_idle().await), ["digest_like"]);
    assert_eq!(job(&w, &sa, digest).await.status, JobStatus::Done);

    // Both queued and due: the interactive one is claimed first, though queued later.
    enqueue(&w, &sa, "digest_like", &[], None, 3).await;
    w.db.clock.advance(chrono::Duration::seconds(1));
    enqueue(&w, &sa, "file_like", &[], None, 3).await;
    gate.add_permits(1);
    assert_eq!(
        kinds(runner.run_until_idle().await),
        ["file_like", "digest_like"]
    );
    w.finish().await;
}

#[tokio::test]
async fn a_wake_starts_queued_work_without_waiting_for_the_poll() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let started = Arc::new(tokio::sync::Notify::new());
    let mut filing = Script::new("file_like", JobClass::Llm);
    filing.interactive = true;
    filing.started = Some(started.clone());
    let runner = w.runner(
        RunnerConfig {
            poll_interval: Duration::from_secs(3600),
            ..RunnerConfig::default()
        },
        vec![Arc::new(filing)],
    );
    // The loop's first pass finds nothing and waits an hour for the next poll.
    let handle = runner.start_loop();
    tokio::task::yield_now().await;
    enqueue(&w, &sa, "file_like", &[], None, 3).await;
    runner.wake();
    tokio::time::timeout(Duration::from_secs(30), started.notified())
        .await
        .expect("the wake started the job long before the next poll");
    handle.shutdown(Duration::from_secs(5)).await;
    w.finish().await;
}
