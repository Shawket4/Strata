//! The scheduler for periodic job kinds (PLAN §9.2: `dedupe` nightly, `digest` weekly, …).
//!
//! For every active user it makes sure one job of each periodic kind is queued (or running);
//! a missing one is enqueued for the next occurrence of its schedule in the configured time
//! zone. It is idempotent (the debounce key is the cadence), so the composition root calls
//! [`Scheduler::ensure`] at start and then hourly; a completed or failed periodic job is
//! simply replaced by the next pass.

use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use strata_common::{Clock, IdGenerator, JobId, UserId};
use strata_index::repo::jobs::NewJob;
use strata_index::{AppDb, ScopeIssuer};

use crate::handler::JobError;
use crate::repo;

/// Who has a vault (active and deletion-pending accounts keep their jobs running).
#[async_trait::async_trait]
pub trait UserDirectory: Send + Sync {
    /// Users whose background jobs should run.
    async fn active_users(&self) -> Result<Vec<UserId>, String>;
}

/// A fixed list of users (tests).
#[derive(Debug, Clone, Default)]
pub struct StaticUsers(pub Vec<UserId>);

#[async_trait::async_trait]
impl UserDirectory for StaticUsers {
    async fn active_users(&self) -> Result<Vec<UserId>, String> {
        Ok(self.0.clone())
    }
}

/// When a periodic kind runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// Every day at the nightly hour.
    Nightly,
    /// Every week on this day at the nightly hour.
    Weekly(Weekday),
}

impl Cadence {
    fn key(self) -> String {
        match self {
            Self::Nightly => "nightly".to_owned(),
            Self::Weekly(d) => format!("weekly-{d}"),
        }
    }
}

/// A periodic job kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Periodic {
    /// Job kind.
    pub kind: &'static str,
    /// Schedule.
    pub cadence: Cadence,
}

/// The next time at or after `now` (exclusive of `now` itself) that `cadence` fires at
/// `hour`:00 local time in `tz`.
pub fn next_run(now: DateTime<Utc>, cadence: Cadence, hour: u32, tz: Tz) -> DateTime<Utc> {
    let local = now.with_timezone(&tz);
    let at = NaiveTime::from_hms_opt(hour.min(23), 0, 0).unwrap_or(NaiveTime::MIN);
    for days in 0..=8 {
        let date = local.date_naive() + Duration::days(days);
        let matches = match cadence {
            Cadence::Nightly => true,
            Cadence::Weekly(d) => date.weekday() == d,
        };
        if !matches {
            continue;
        }
        // A skipped local time (DST gap) takes the next valid instant.
        let candidate = tz
            .from_local_datetime(&date.and_time(at))
            .earliest()
            .or_else(|| {
                tz.from_local_datetime(&date.and_time(at + Duration::hours(1)))
                    .earliest()
            });
        if let Some(c) = candidate {
            let c = c.with_timezone(&Utc);
            if c > now {
                return c;
            }
        }
    }
    now + Duration::days(1)
}

/// Keeps one queued job of each periodic kind per active user.
pub struct Scheduler {
    db: AppDb,
    issuer: ScopeIssuer,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    users: Arc<dyn UserDirectory>,
    tz: Tz,
    hour: u32,
    periodic: Vec<Periodic>,
    recovery: Option<Arc<crate::LlmRecovery>>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("tz", &self.tz)
            .field("hour", &self.hour)
            .field("periodic", &self.periodic)
            .finish_non_exhaustive()
    }
}

impl Scheduler {
    /// A scheduler firing `periodic` kinds at `hour`:00 in `tz`.
    #[allow(clippy::too_many_arguments)] // the composition root wires each part explicitly
    pub fn new(
        db: AppDb,
        issuer: ScopeIssuer,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        users: Arc<dyn UserDirectory>,
        tz: Tz,
        hour: u32,
        periodic: Vec<Periodic>,
    ) -> Self {
        Self {
            db,
            issuer,
            clock,
            ids,
            users,
            tz,
            hour,
            periodic,
            recovery: None,
        }
    }

    /// Also requeues, on every pass, each user's jobs that failed while the AI provider was
    /// down before `recovery`'s last successful LLM job (see [`crate::recovery`]).
    #[must_use]
    pub fn with_recovery(mut self, recovery: Arc<crate::LlmRecovery>) -> Self {
        self.recovery = Some(recovery);
        self
    }

    /// Requeues provider failures older than the last successful LLM job for every user;
    /// returns how many jobs were requeued.
    pub async fn requeue_recovered(&self) -> u64 {
        let Some(since) = self.recovery.as_ref().and_then(|r| r.last_success()) else {
            return 0;
        };
        let now = self.clock.now();
        let mut total = 0;
        for user in self.users().await {
            let scope = self.issuer.issue(user);
            let result = async {
                let mut tx = self.db.begin(&scope).await?;
                let n = strata_index::repo::jobs::requeue_failed(&mut tx, true, Some(since), now)
                    .await?;
                tx.commit().await?;
                Ok::<_, strata_index::IndexError>(n)
            }
            .await;
            match result {
                Ok(n) => total += n,
                Err(e) => {
                    tracing::error!(user = %user, error = %e, "requeueing failed jobs failed");
                }
            }
        }
        if total > 0 {
            tracing::info!(
                jobs = total,
                "requeued jobs that failed while the AI provider was down"
            );
        }
        total
    }

    /// The users the directory lists.
    pub async fn users(&self) -> Vec<UserId> {
        match self.users.active_users().await {
            Ok(users) => users,
            Err(e) => {
                tracing::error!(error = %e, "listing users for the scheduler failed");
                Vec::new()
            }
        }
    }

    /// Enqueues every missing periodic job; returns how many were enqueued.
    pub async fn ensure(&self) -> usize {
        let now = self.clock.now();
        let mut added = 0;
        for user in self.users().await {
            for p in &self.periodic {
                match self.ensure_one(user, p, now).await {
                    Ok(true) => added += 1,
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(user = %user, kind = p.kind, error = %e, "scheduling failed");
                    }
                }
            }
        }
        added
    }

    async fn ensure_one(
        &self,
        user: UserId,
        p: &Periodic,
        now: DateTime<Utc>,
    ) -> Result<bool, JobError> {
        let scope = self.issuer.issue(user);
        let mut tx = self.db.begin(&scope).await?;
        if repo::exists_pending(&mut tx, p.kind).await? {
            tx.commit().await?;
            return Ok(false);
        }
        repo::enqueue(
            &mut tx,
            &NewJob {
                id: JobId::generate(self.ids.as_ref()),
                kind: p.kind.to_owned(),
                note_id: None,
                payload: Vec::new(),
                run_after: next_run(now, p.cadence, self.hour, self.tz),
                max_attempts: 3,
                dedupe_key: Some(p.cadence.key()),
            },
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Enqueues a one-off job of `kind` (debounced by `key`) for every active user, runnable
    /// now: e.g. the embedding backfill at start. Returns how many users got one.
    pub async fn enqueue_for_all(&self, kind: &str, key: &str) -> usize {
        let now = self.clock.now();
        let mut n = 0;
        for user in self.users().await {
            let scope = self.issuer.issue(user);
            let result = async {
                let mut tx = self.db.begin(&scope).await?;
                repo::enqueue(
                    &mut tx,
                    &NewJob {
                        id: JobId::generate(self.ids.as_ref()),
                        kind: kind.to_owned(),
                        note_id: None,
                        payload: Vec::new(),
                        run_after: now,
                        max_attempts: 5,
                        dedupe_key: Some(key.to_owned()),
                    },
                    now,
                )
                .await?;
                tx.commit().await?;
                Ok::<(), JobError>(())
            }
            .await;
            match result {
                Ok(()) => n += 1,
                Err(e) => tracing::error!(user = %user, kind, error = %e, "enqueueing failed"),
            }
        }
        n
    }

    /// Calls [`Self::ensure`] every `period` until the task is aborted.
    pub fn spawn(self: Arc<Self>, period: std::time::Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(period);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                self.ensure().await;
                self.requeue_recovered().await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().expect("rfc3339")
    }

    #[test]
    fn nightly_runs_at_the_next_local_hour() {
        let cairo: Tz = "Africa/Cairo".parse().expect("tz");
        // 2026-09-27 is EEST (UTC+3): 03:00 local = 00:00 UTC.
        assert_eq!(
            next_run(at("2026-09-27T12:00:00Z"), Cadence::Nightly, 3, cairo),
            at("2026-09-28T00:00:00Z")
        );
        assert_eq!(
            next_run(at("2026-09-27T23:59:59Z"), Cadence::Nightly, 3, cairo),
            at("2026-09-28T00:00:00Z")
        );
        // Exactly at the time: the next day.
        assert_eq!(
            next_run(at("2026-09-28T00:00:00Z"), Cadence::Nightly, 3, cairo),
            at("2026-09-29T00:00:00Z")
        );
        assert_eq!(
            next_run(at("2026-09-27T12:00:00Z"), Cadence::Nightly, 3, Tz::UTC),
            at("2026-09-28T03:00:00Z")
        );
    }

    #[test]
    fn weekly_runs_on_its_day_and_dst_gaps_move_forward() {
        // 2026-09-27 is a Sunday.
        assert_eq!(
            next_run(
                at("2026-09-27T12:00:00Z"),
                Cadence::Weekly(Weekday::Mon),
                3,
                Tz::UTC
            ),
            at("2026-09-28T03:00:00Z")
        );
        assert_eq!(
            next_run(
                at("2026-09-27T12:00:00Z"),
                Cadence::Weekly(Weekday::Sun),
                3,
                Tz::UTC
            ),
            at("2026-10-04T03:00:00Z")
        );
        // Europe/Berlin springs forward at 02:00 on 2027-03-28: 02:00 does not exist.
        let berlin: Tz = "Europe/Berlin".parse().expect("tz");
        assert_eq!(
            next_run(at("2027-03-27T12:00:00Z"), Cadence::Nightly, 2, berlin),
            at("2027-03-28T01:00:00Z")
        );
    }
}
