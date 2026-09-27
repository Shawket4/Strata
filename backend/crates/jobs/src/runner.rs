//! The job runner (PLAN §5.2 "fair scheduling", §9.2).
//!
//! - **Discovery.** `jobs` is user-owned, so an unscoped worker sees nothing. The runner reads
//!   the content-free `job_wakeups` hints ([`AppDb::due_job_users`]) to find users with due
//!   work, then claims inside that user's [`strata_index::ScopedTx`] with `FOR UPDATE SKIP
//!   LOCKED`, so two runners (or two passes) never claim one job twice.
//! - **Fairness.** Each pass visits due users in round-robin order (least recently served
//!   first) and claims at most one job per user per round; a user who floods the queue gets
//!   one slot per round like everyone else, so other users keep progressing.
//! - **Limits.** A global concurrency limit (`jobs.max_concurrency`, within the 1-core VPS
//!   budget) and a per-kind limit ([`JobHandler::max_concurrency`]: embeddings one at a time).
//!   Only kinds this process has handlers for are claimed; jobs of other kinds stay queued.
//! - **Retries.** A retryable failure re-queues the job after `base · 2^(attempt−1)` (capped),
//!   until `max_attempts`; then it fails and `job.failed` is published.
//! - **Pauses.** [`JobError::Paused`] (budget reached, provider usage limit) puts the job back
//!   for the pause without spending an attempt, and LLM jobs of that user (or of everyone,
//!   for global and provider pauses) are not claimed until then. The daily job limit
//!   (`ai.daily_job_limit`) pauses LLM jobs until the next UTC day the same way.
//! - **Shutdown.** [`RunnerHandle::shutdown`] stops claiming and waits (bounded) for running
//!   jobs; a job still running at exit stays `running` and is re-queued by
//!   [`Runner::recover_stale`] at the next start (one process per deployment).
//!
//! All time comes from the injected [`Clock`], so retries, pauses and schedules are tested
//! with a fake clock; [`Runner::tick`] and [`Runner::run_until_idle`] drive the runner
//! step by step in tests.

use std::collections::{BTreeMap, HashMap};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use futures::FutureExt;
use strata_ai::PauseReason;
use strata_common::{Clock, UserId};
use strata_index::repo::jobs::{self, Job};
use strata_index::{AppDb, ScopeIssuer, UserScope};
use tokio::sync::{Notify, watch};
use tokio::task::JoinSet;

use crate::events::{JobEvents, JobNotice, JobOutcome};
use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::repo;

/// Runner settings (from `[jobs]` and `ai.daily_job_limit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    /// Jobs running at once across all users.
    pub max_concurrency: usize,
    /// Wait between passes when nothing is runnable.
    pub poll_interval: Duration,
    /// How long a user whose queued jobs are all of unhandled kinds is skipped.
    pub idle_recheck: chrono::Duration,
    /// First retry delay.
    pub backoff_base: chrono::Duration,
    /// Longest retry delay.
    pub backoff_max: chrono::Duration,
    /// Pause when a paused job does not say until when.
    pub default_pause: chrono::Duration,
    /// LLM jobs per UTC day across all users (0 = unlimited).
    pub daily_llm_jobs: u32,
    /// Users looked at per pass.
    pub users_per_pass: i64,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 2,
            poll_interval: Duration::from_secs(5),
            idle_recheck: chrono::Duration::seconds(60),
            backoff_base: chrono::Duration::seconds(30),
            backoff_max: chrono::Duration::seconds(3600),
            default_pause: chrono::Duration::minutes(5),
            daily_llm_jobs: 0,
            users_per_pass: 256,
        }
    }
}

impl RunnerConfig {
    /// From `[jobs]` and `ai.daily_job_limit`.
    pub fn from_config(config: &strata_common::Config) -> Self {
        let j = &config.jobs;
        Self {
            max_concurrency: usize::try_from(j.max_concurrency.max(1)).unwrap_or(1),
            poll_interval: Duration::from_secs(u64::from(j.poll_interval_secs.max(1))),
            idle_recheck: chrono::Duration::seconds(i64::from(j.idle_recheck_secs)),
            backoff_base: chrono::Duration::seconds(i64::from(j.backoff_base_secs)),
            backoff_max: chrono::Duration::seconds(i64::from(j.backoff_max_secs)),
            daily_llm_jobs: config.ai.daily_job_limit,
            ..Self::default()
        }
    }

    /// Delay before retry number `attempt` (1 = after the first failed run).
    pub fn backoff(&self, attempt: i32) -> chrono::Duration {
        let exp = u32::try_from(attempt.saturating_sub(1).clamp(0, 30)).unwrap_or(0);
        let factor = 1i64 << exp;
        let secs = self
            .backoff_base
            .num_seconds()
            .saturating_mul(factor)
            .min(self.backoff_max.num_seconds());
        chrono::Duration::seconds(secs)
    }
}

/// A pause of LLM work in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmPause {
    /// Why.
    pub reason: PauseReason,
    /// Until when.
    pub until: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct State {
    running_total: usize,
    running_kind: HashMap<&'static str, usize>,
    running_user: HashMap<UserId, usize>,
    /// Pass counter value at a user's last claim (round-robin order).
    last_served: HashMap<UserId, u64>,
    served_counter: u64,
    skip_until: HashMap<UserId, DateTime<Utc>>,
    llm_paused_user: HashMap<UserId, LlmPause>,
    llm_paused_all: Option<LlmPause>,
    llm_day: Option<(NaiveDate, u32)>,
}

struct Inner {
    db: AppDb,
    issuer: ScopeIssuer,
    clock: Arc<dyn Clock>,
    events: Arc<dyn JobEvents>,
    handlers: BTreeMap<&'static str, Arc<dyn JobHandler>>,
    config: RunnerConfig,
    state: Mutex<State>,
    tasks: tokio::sync::Mutex<JoinSet<()>>,
    finished: Notify,
}

/// The job runner (cheap to clone).
#[derive(Clone)]
pub struct Runner {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("kinds", &self.inner.handlers.keys().collect::<Vec<_>>())
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

/// One job claimed by a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    /// Owner.
    pub user: UserId,
    /// Job ID.
    pub id: strata_common::JobId,
    /// Kind.
    pub kind: String,
}

impl Runner {
    /// A runner over `db`; `issuer` scopes each job to its owner (the composition root gives
    /// the issuer only to the auth middleware and the runner, PLAN §5.2).
    pub fn new(
        db: AppDb,
        issuer: ScopeIssuer,
        clock: Arc<dyn Clock>,
        events: Arc<dyn JobEvents>,
        config: RunnerConfig,
        handlers: Vec<Arc<dyn JobHandler>>,
    ) -> Self {
        let handlers = handlers.into_iter().map(|h| (h.kind(), h)).collect();
        Self {
            inner: Arc::new(Inner {
                db,
                issuer,
                clock,
                events,
                handlers,
                config,
                state: Mutex::new(State::default()),
                tasks: tokio::sync::Mutex::new(JoinSet::new()),
                finished: Notify::new(),
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Kinds this runner runs.
    pub fn kinds(&self) -> Vec<&'static str> {
        self.inner.handlers.keys().copied().collect()
    }

    /// The LLM pause in effect for `user` (their own or everyone's), if any.
    pub fn llm_pause(&self, user: UserId) -> Option<LlmPause> {
        let now = self.inner.clock.now();
        let s = self.state();
        [s.llm_paused_all, s.llm_paused_user.get(&user).copied()]
            .into_iter()
            .flatten()
            .filter(|p| p.until > now)
            .max_by_key(|p| p.until)
    }

    /// Jobs running now.
    pub fn running(&self) -> usize {
        self.state().running_total
    }

    /// Re-queues every job of `users` left `running` by a previous process (one process per
    /// deployment: at start, nothing can legitimately be running). Returns how many.
    pub async fn recover_stale(&self, users: &[UserId]) -> u64 {
        let now = self.inner.clock.now();
        let mut total = 0;
        for user in users {
            let scope = self.inner.issuer.issue(*user);
            let result = async {
                let mut tx = self.inner.db.begin(&scope).await?;
                let n =
                    jobs::requeue_stale(&mut tx, now + chrono::Duration::seconds(1), now).await?;
                tx.commit().await?;
                Ok::<u64, strata_index::IndexError>(n)
            }
            .await;
            match result {
                Ok(n) => total += n,
                Err(e) => {
                    tracing::error!(user = %user, error = %e, "re-queueing stale jobs failed")
                }
            }
        }
        if total > 0 {
            tracing::info!(
                count = total,
                "re-queued jobs left running by the previous process"
            );
        }
        total
    }

    /// Kinds with free capacity that `user` may claim now.
    fn claimable_kinds(&self, user: UserId, now: DateTime<Utc>) -> Vec<String> {
        let mut s = self.state();
        if s.running_total >= self.inner.config.max_concurrency {
            return Vec::new();
        }
        let today = now.date_naive();
        let daily_limit_reached = self.inner.config.daily_llm_jobs > 0
            && s.llm_day
                .is_some_and(|(d, n)| d == today && n >= self.inner.config.daily_llm_jobs);
        let paused = s.llm_paused_all.is_some_and(|p| p.until > now)
            || s.llm_paused_user.get(&user).is_some_and(|p| p.until > now)
            || daily_limit_reached;
        s.llm_paused_user.retain(|_, p| p.until > now);
        if s.llm_paused_all.is_some_and(|p| p.until <= now) {
            s.llm_paused_all = None;
        }
        self.inner
            .handlers
            .values()
            .filter(|h| s.running_kind.get(h.kind()).copied().unwrap_or(0) < h.max_concurrency())
            .filter(|h| !(paused && h.class() == JobClass::Llm))
            .map(|h| h.kind().to_owned())
            .collect()
    }

    /// Due users in round-robin order: least recently served first, then by due time.
    async fn due_users(&self, now: DateTime<Utc>) -> Vec<UserId> {
        let due = match self
            .inner
            .db
            .due_job_users(now, self.inner.config.users_per_pass)
            .await
        {
            Ok(users) => users,
            Err(e) => {
                tracing::error!(error = %e, "listing users with due jobs failed");
                return Vec::new();
            }
        };
        let s = self.state();
        let mut users: Vec<(usize, UserId)> = due
            .into_iter()
            .enumerate()
            .filter(|(_, u)| s.skip_until.get(u).is_none_or(|t| *t <= now))
            .collect();
        users.sort_by_key(|(i, u)| (s.last_served.get(u).copied().unwrap_or(0), *i));
        users.into_iter().map(|(_, u)| u).collect()
    }

    /// Claims one job of `user` among `kinds`, with its username.
    async fn claim(
        &self,
        scope: &UserScope,
        now: DateTime<Utc>,
        kinds: &[String],
    ) -> Result<Option<(Job, String)>, JobError> {
        let mut tx = self.inner.db.begin(scope).await?;
        if let Some(job) = repo::claim_of_kinds(&mut tx, now, kinds).await? {
            let username = repo::username(&mut tx).await?.unwrap_or_default();
            tx.commit().await?;
            return Ok(Some((job, username)));
        }
        // Nothing claimable: refresh the wakeup hint. When the hint stays due only because of
        // jobs of kinds this process does not run, skip the user until its next job of a kind
        // we run is due, or for a while (it would otherwise be looked at on every pass).
        // The wakeup row is locked first, so the checks below see every job committed by
        // then (a job running concurrently may just have queued follow-up work).
        let handled: Vec<String> = self.kinds().iter().map(|k| (*k).to_owned()).collect();
        let earliest = jobs::refresh_wakeup(&mut tx).await?;
        let blocked = repo::any_due(&mut tx, now, &handled).await?;
        let next = repo::next_due(&mut tx, &handled).await?;
        tx.commit().await?;
        if !blocked && earliest.is_some_and(|e| e <= now) {
            let recheck = now + self.inner.config.idle_recheck;
            let until = next.map_or(recheck, |n| n.min(recheck));
            let mut s = self.state();
            // A running job of this user may still queue follow-up work: never skip then.
            if s.running_user.get(&scope.user_id()).copied().unwrap_or(0) == 0 {
                s.skip_until.insert(scope.user_id(), until);
            }
        }
        Ok(None)
    }

    /// One scheduling pass: claims as many jobs as the capacity free at its start allows,
    /// fairly across users, and starts them. Returns what it claimed.
    pub async fn tick(&self) -> Vec<Claimed> {
        let now = self.inner.clock.now();
        let budget = self
            .inner
            .config
            .max_concurrency
            .saturating_sub(self.state().running_total);
        let mut claimed = Vec::new();
        if budget == 0 {
            return claimed;
        }
        let users = self.due_users(now).await;
        let mut active: Vec<UserId> = users;
        loop {
            let mut progressed = false;
            let mut next_round = Vec::new();
            for user in active {
                let kinds = self.claimable_kinds(user, now);
                if kinds.is_empty() {
                    if self.state().running_total >= self.inner.config.max_concurrency {
                        return claimed;
                    }
                    continue;
                }
                let scope = self.inner.issuer.issue(user);
                match self.claim(&scope, now, &kinds).await {
                    Ok(Some((job, username))) => {
                        progressed = true;
                        claimed.push(Claimed {
                            user,
                            id: job.id,
                            kind: job.kind.clone(),
                        });
                        self.start(scope, job, username, now).await;
                        next_round.push(user);
                        if claimed.len() >= budget {
                            return claimed;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::error!(user = %user, error = %e, "claiming a job failed"),
                }
            }
            if !progressed || next_round.is_empty() {
                return claimed;
            }
            active = next_round;
        }
    }

    async fn start(&self, scope: UserScope, job: Job, username: String, now: DateTime<Utc>) {
        let Some(handler) = self.inner.handlers.get(job.kind.as_str()).cloned() else {
            return;
        };
        let kind = handler.kind();
        {
            let mut s = self.state();
            s.running_total += 1;
            *s.running_kind.entry(kind).or_insert(0) += 1;
            *s.running_user.entry(scope.user_id()).or_insert(0) += 1;
            s.served_counter += 1;
            let c = s.served_counter;
            s.last_served.insert(scope.user_id(), c);
            if handler.class() == JobClass::Llm {
                let today = now.date_naive();
                s.llm_day = Some(match s.llm_day {
                    Some((d, n)) if d == today => (d, n + 1),
                    _ => (today, 1),
                });
            }
        }
        let me = self.clone();
        let ctx = JobContext {
            job: job.clone(),
            scope,
            username,
            now,
        };
        self.inner.tasks.lock().await.spawn(async move {
            let result = AssertUnwindSafe(handler.run(ctx))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| Err(JobError::Retry("the job panicked".into())));
            me.finish(scope, &job, handler.class(), result).await;
            {
                let mut s = me.state();
                // The job may have queued follow-up work for its user: look again.
                s.skip_until.remove(&scope.user_id());
                s.running_total = s.running_total.saturating_sub(1);
                if let Some(n) = s.running_kind.get_mut(kind) {
                    *n = n.saturating_sub(1);
                }
                if let Some(n) = s.running_user.get_mut(&scope.user_id()) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        s.running_user.remove(&scope.user_id());
                    }
                }
            }
            me.inner.finished.notify_one();
        });
    }

    /// Records the outcome of a run and publishes `job.completed` / `job.failed`.
    async fn finish(
        &self,
        scope: UserScope,
        job: &Job,
        class: JobClass,
        result: Result<(), JobError>,
    ) {
        let now = self.inner.clock.now();
        let user = scope.user_id();
        let outcome = async {
            let mut tx = self.inner.db.begin(&scope).await?;
            let outcome = match &result {
                Ok(()) => {
                    jobs::complete(&mut tx, job.id, now).await?;
                    Some(JobOutcome::Completed)
                }
                Err(JobError::Paused { reason, until }) => {
                    let until = until
                        .filter(|u| *u > now)
                        .unwrap_or(now + self.inner.config.default_pause);
                    let note = format!("paused: {reason:?}");
                    repo::release_paused(&mut tx, job.id, until, &note, now).await?;
                    if class == JobClass::Llm {
                        let pause = LlmPause {
                            reason: *reason,
                            until,
                        };
                        let mut s = self.state();
                        if *reason == PauseReason::UserBudget {
                            s.llm_paused_user.insert(user, pause);
                        } else {
                            s.llm_paused_all = Some(pause);
                        }
                    }
                    tracing::info!(job = %job.id, kind = %job.kind, ?reason, %until, "job paused");
                    None
                }
                Err(JobError::Retry(msg)) => {
                    let retry_at = now + self.inner.config.backoff(job.attempts);
                    let row = jobs::fail(&mut tx, job.id, msg, Some(retry_at), now).await?;
                    tracing::warn!(job = %job.id, kind = %job.kind, attempt = job.attempts, error = %msg, "job failed");
                    row.filter(|r| r.status == strata_index::types::JobStatus::Failed)
                        .map(|_| JobOutcome::Failed)
                }
                Err(JobError::Fatal(msg)) => {
                    tracing::warn!(job = %job.id, kind = %job.kind, error = %msg, "job failed permanently");
                    jobs::fail(&mut tx, job.id, msg, None, now).await?;
                    Some(JobOutcome::Failed)
                }
            };
            tx.commit().await?;
            Ok::<_, JobError>(outcome)
        }
        .await;
        match outcome {
            Ok(Some(outcome)) => self.inner.events.job_finished(
                user,
                &JobNotice {
                    id: job.id,
                    kind: job.kind.clone(),
                    note_id: job.note_id,
                    outcome,
                },
            ),
            Ok(None) => {}
            Err(e) => {
                tracing::error!(job = %job.id, error = %e, "recording a job outcome failed");
            }
        }
    }

    /// Waits until every started job finished.
    pub async fn drain(&self) {
        let mut tasks = self.inner.tasks.lock().await;
        while tasks.join_next().await.is_some() {}
    }

    /// Passes until a pass claims nothing and no job is running (tests; production uses
    /// [`Self::start_loop`]). Returns everything claimed, in order.
    pub async fn run_until_idle(&self) -> Vec<Claimed> {
        let mut all = Vec::new();
        loop {
            let claimed = self.tick().await;
            let empty = claimed.is_empty();
            all.extend(claimed);
            self.drain().await;
            if empty {
                return all;
            }
        }
    }

    /// Starts the scheduling loop on the current runtime.
    pub fn start_loop(&self) -> RunnerHandle {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let me = self.clone();
        let task = tokio::spawn(async move {
            loop {
                if *stop_rx.borrow() {
                    break;
                }
                let claimed = me.tick().await;
                // Reap finished tasks so the set does not grow.
                {
                    let mut tasks = me.inner.tasks.lock().await;
                    while tasks.try_join_next().is_some() {}
                }
                let full = me.running() >= me.inner.config.max_concurrency;
                if !claimed.is_empty() && !full {
                    continue;
                }
                let wait = me.inner.config.poll_interval;
                tokio::select! {
                    _ = stop_rx.changed() => {}
                    () = me.inner.finished.notified() => {}
                    () = tokio::time::sleep(wait) => {}
                }
            }
        });
        RunnerHandle {
            runner: self.clone(),
            stop: stop_tx,
            task,
        }
    }
}

/// The running scheduling loop.
#[derive(Debug)]
pub struct RunnerHandle {
    runner: Runner,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl RunnerHandle {
    /// Stops claiming, waits up to `grace` for running jobs, then abandons the rest (they
    /// stay `running` and are re-queued at the next start). Returns how many were abandoned.
    pub async fn shutdown(self, grace: Duration) -> usize {
        let _ = self.stop.send(true);
        let _ = self.task.await;
        let mut tasks = self.runner.inner.tasks.lock().await;
        let drained =
            tokio::time::timeout(grace, async { while tasks.join_next().await.is_some() {} }).await;
        if drained.is_ok() {
            0
        } else {
            let left = tasks.len();
            tasks.abort_all();
            tracing::warn!(
                count = left,
                "jobs still running at shutdown; re-queued at next start"
            );
            left
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_from_the_base_up_to_the_cap() {
        let c = RunnerConfig::default();
        let secs: Vec<i64> = (1..=9).map(|a| c.backoff(a).num_seconds()).collect();
        assert_eq!(secs, vec![30, 60, 120, 240, 480, 960, 1920, 3600, 3600]);
        assert_eq!(c.backoff(0).num_seconds(), 30);
    }

    #[test]
    fn config_follows_the_jobs_section() {
        let mut config = strata_common::Config::default();
        config.jobs.max_concurrency = 3;
        config.jobs.backoff_base_secs = 10;
        config.ai.daily_job_limit = 50;
        let r = RunnerConfig::from_config(&config);
        assert_eq!(
            (
                r.max_concurrency,
                r.poll_interval,
                r.idle_recheck.num_seconds(),
                r.backoff_base.num_seconds(),
                r.backoff_max.num_seconds(),
                r.daily_llm_jobs
            ),
            (3, Duration::from_secs(5), 60, 10, 3600, 50)
        );
    }
}
