//! Budget guard (PLAN §9.1): per-user daily token/cost caps and a global cap. Usage is recorded
//! in `ai_usage` (per user, RLS-scoped) and `ai_usage_global` (per day, all users). When a cap
//! is reached, work is paused until the next day boundary (it waits, it never fails).
//!
//! Days are calendar days in one configured timezone (`budget_timezone`, default the server's
//! default timezone), shared by the per-user and global caps. The check runs before each call
//! and a call is always recorded afterwards, so a day may end slightly over its cap by at most
//! the calls that were already in flight.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use strata_common::{Clock, UserId};
use strata_index::repo::settings::{self, AiUsage, GlobalAiUsage};
use strata_index::{AppDb, UserScope};

use crate::error::{AiError, PauseReason};
use crate::request::{AiCaller, Usage};
use crate::status::PauseInfo;

/// Daily caps; `0` means unlimited.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BudgetLimits {
    /// Tokens (all input + output) per user per day.
    pub per_user_daily_tokens: u64,
    /// Estimated micro-USD per user per day.
    pub per_user_daily_cost_micros: u64,
    /// Tokens per day across all users.
    pub global_daily_tokens: u64,
    /// Estimated micro-USD per day across all users.
    pub global_daily_cost_micros: u64,
}

impl BudgetLimits {
    /// From the `[budgets]` config section (it has no global cost cap: unlimited).
    pub fn from_config(b: &strata_common::config::Budgets) -> Self {
        Self {
            per_user_daily_tokens: b.per_user_daily_tokens,
            per_user_daily_cost_micros: b.per_user_daily_cost_micros,
            global_daily_tokens: b.global_daily_tokens,
            global_daily_cost_micros: 0,
        }
    }
}

/// Totals for one day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct UsageTotals {
    /// Provider calls.
    pub calls: u64,
    /// Input tokens (including prompt-cache writes and reads).
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Estimated micro-USD.
    pub cost_micros: u64,
}

impl UsageTotals {
    /// Input + output tokens.
    pub fn tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    fn add(&mut self, u: &Usage) {
        self.calls += 1;
        self.input_tokens = self.input_tokens.saturating_add(u.total_input());
        self.output_tokens = self.output_tokens.saturating_add(u.output_tokens);
        self.cost_micros = self.cost_micros.saturating_add(u.cost_micros.unwrap_or(0));
    }
}

/// Where usage is recorded.
#[async_trait::async_trait]
pub trait UsageStore: Send + Sync + fmt::Debug {
    /// Adds one call to the user's and the global totals for `day`.
    async fn add(
        &self,
        scope: &UserScope,
        day: NaiveDate,
        provider: &str,
        model: &str,
        usage: &Usage,
    ) -> Result<(), AiError>;

    /// The user's totals for `day` (all providers and models).
    async fn user_day(&self, scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError>;

    /// Totals for `day` across all users (read inside the caller's scope).
    async fn global_day(&self, scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError>;
}

/// In-memory store (tests, and a process without a database).
#[derive(Debug, Default)]
pub struct MemoryUsageStore {
    rows: Mutex<BTreeMap<(UserId, NaiveDate, String, String), UsageTotals>>,
}

impl MemoryUsageStore {
    /// Every row, as `ai_usage` would hold it.
    pub fn rows(&self) -> BTreeMap<(UserId, NaiveDate, String, String), UsageTotals> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait::async_trait]
impl UsageStore for MemoryUsageStore {
    async fn add(
        &self,
        scope: &UserScope,
        day: NaiveDate,
        provider: &str,
        model: &str,
        usage: &Usage,
    ) -> Result<(), AiError> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry((scope.user_id(), day, provider.to_owned(), model.to_owned()))
            .or_default()
            .add(usage);
        Ok(())
    }

    async fn user_day(&self, scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError> {
        Ok(self.sum(|u, d| u == scope.user_id() && d == day))
    }

    async fn global_day(&self, _scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError> {
        Ok(self.sum(|_, d| d == day))
    }
}

impl MemoryUsageStore {
    fn sum(&self, keep: impl Fn(UserId, NaiveDate) -> bool) -> UsageTotals {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|((u, d, _, _), _)| keep(*u, *d))
            .fold(UsageTotals::default(), |mut acc, (_, t)| {
                acc.calls += t.calls;
                acc.input_tokens += t.input_tokens;
                acc.output_tokens += t.output_tokens;
                acc.cost_micros += t.cost_micros;
                acc
            })
    }
}

/// `PostgreSQL` store: `ai_usage` (scoped) and `ai_usage_global`, updated in one transaction.
#[derive(Debug, Clone)]
pub struct PgUsageStore {
    db: AppDb,
}

impl PgUsageStore {
    /// Over the `strata_app` handle.
    pub fn new(db: AppDb) -> Self {
        Self { db }
    }
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn to_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

#[async_trait::async_trait]
impl UsageStore for PgUsageStore {
    async fn add(
        &self,
        scope: &UserScope,
        day: NaiveDate,
        provider: &str,
        model: &str,
        usage: &Usage,
    ) -> Result<(), AiError> {
        let mut tx = self.db.begin(scope).await?;
        let delta = AiUsage {
            day,
            provider: provider.to_owned(),
            model: model.to_owned(),
            calls: 1,
            input_tokens: to_i64(usage.total_input()),
            output_tokens: to_i64(usage.output_tokens),
            est_cost_micros: to_i64(usage.cost_micros.unwrap_or(0)),
        };
        settings::add_ai_usage(&mut tx, &delta).await?;
        settings::add_global_ai_usage(
            &mut tx,
            &GlobalAiUsage {
                day,
                calls: 1,
                input_tokens: delta.input_tokens,
                output_tokens: delta.output_tokens,
                est_cost_micros: delta.est_cost_micros,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn user_day(&self, scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError> {
        let mut tx = self.db.begin(scope).await?;
        let rows = settings::ai_usage_for_day(&mut tx, day).await?;
        tx.commit().await?;
        Ok(rows.iter().fold(UsageTotals::default(), |mut acc, r| {
            acc.calls += u64::try_from(r.calls).unwrap_or(0);
            acc.input_tokens += to_u64(r.input_tokens);
            acc.output_tokens += to_u64(r.output_tokens);
            acc.cost_micros += to_u64(r.est_cost_micros);
            acc
        }))
    }

    async fn global_day(&self, scope: &UserScope, day: NaiveDate) -> Result<UsageTotals, AiError> {
        let mut tx = self.db.begin(scope).await?;
        let row = settings::global_ai_usage_for_day(&mut tx, day).await?;
        tx.commit().await?;
        Ok(row.map_or_else(UsageTotals::default, |r| UsageTotals {
            calls: to_u64(r.calls),
            input_tokens: to_u64(r.input_tokens),
            output_tokens: to_u64(r.output_tokens),
            cost_micros: to_u64(r.est_cost_micros),
        }))
    }
}

/// Enforces [`BudgetLimits`].
#[derive(Debug, Clone)]
pub struct BudgetGuard {
    limits: BudgetLimits,
    tz: Tz,
    clock: Arc<dyn Clock>,
    store: Arc<dyn UsageStore>,
}

/// A user's and the global usage for the current budget day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetSnapshot {
    /// Budget day.
    pub day: NaiveDate,
    /// The user's totals.
    pub user: UsageTotals,
    /// All users' totals.
    pub global: UsageTotals,
    /// The budget pause in effect for this user, if any.
    pub paused: Option<PauseInfo>,
}

impl BudgetGuard {
    /// A guard over `store` with days in `tz`.
    pub fn new(
        limits: BudgetLimits,
        tz: Tz,
        clock: Arc<dyn Clock>,
        store: Arc<dyn UsageStore>,
    ) -> Self {
        Self {
            limits,
            tz,
            clock,
            store,
        }
    }

    /// The limits.
    pub fn limits(&self) -> BudgetLimits {
        self.limits
    }

    /// The current budget day.
    pub fn day(&self) -> NaiveDate {
        self.clock.now().with_timezone(&self.tz).date_naive()
    }

    /// The start of the next budget day (when paused work resumes).
    pub fn next_reset(&self) -> DateTime<Utc> {
        let local = self.clock.now().with_timezone(&self.tz).date_naive();
        let mut day = local;
        // Midnight may not exist on a DST transition day in some zones; take the first instant
        // of the next day that does.
        for _ in 0..3 {
            day = day.succ_opt().unwrap_or(day);
            for hour in 0..3 {
                if let Some(t) = day
                    .and_hms_opt(hour, 0, 0)
                    .and_then(|dt| self.tz.from_local_datetime(&dt).earliest())
                {
                    return t.with_timezone(&Utc);
                }
            }
        }
        self.clock.now() + chrono::Duration::days(1)
    }

    fn pause_for(&self, user: &UsageTotals, global: &UsageTotals) -> Option<PauseInfo> {
        let over = |value: u64, cap: u64| cap > 0 && value >= cap;
        let reason = if over(user.tokens(), self.limits.per_user_daily_tokens)
            || over(user.cost_micros, self.limits.per_user_daily_cost_micros)
        {
            PauseReason::UserBudget
        } else if over(global.tokens(), self.limits.global_daily_tokens)
            || over(global.cost_micros, self.limits.global_daily_cost_micros)
        {
            PauseReason::GlobalBudget
        } else {
            return None;
        };
        Some(PauseInfo {
            reason,
            until: Some(self.next_reset()),
        })
    }

    /// Current usage and pause state for `caller`.
    pub async fn snapshot(&self, caller: &AiCaller) -> Result<BudgetSnapshot, AiError> {
        let day = self.day();
        let user = self.store.user_day(&caller.scope, day).await?;
        let global = self.store.global_day(&caller.scope, day).await?;
        Ok(BudgetSnapshot {
            day,
            paused: self.pause_for(&user, &global),
            user,
            global,
        })
    }

    /// `Err(Paused)` when `caller` or everyone is over a cap today.
    pub async fn check(&self, caller: &AiCaller) -> Result<(), AiError> {
        match self.snapshot(caller).await?.paused {
            Some(p) => Err(AiError::Paused {
                reason: p.reason,
                until: p.until,
            }),
            None => Ok(()),
        }
    }

    /// Records one call's usage for today.
    pub async fn record(
        &self,
        caller: &AiCaller,
        provider: &str,
        model: &str,
        usage: &Usage,
    ) -> Result<(), AiError> {
        self.store
            .add(&caller.scope, self.day(), provider, model, usage)
            .await
    }
}
