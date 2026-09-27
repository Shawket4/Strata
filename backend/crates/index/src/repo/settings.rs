//! Per-user settings (`MessagePack` values) and AI usage accounting.

use chrono::{DateTime, NaiveDate, Utc};

use crate::error::Result;
use crate::scope::ScopedTx;

/// An `ai_usage` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AiUsage {
    /// Day (user timezone, decided by the caller).
    pub day: NaiveDate,
    /// Provider.
    pub provider: String,
    /// Model.
    pub model: String,
    /// Calls.
    pub calls: i32,
    /// Input tokens.
    pub input_tokens: i64,
    /// Output tokens.
    pub output_tokens: i64,
    /// Estimated cost in micro-USD.
    pub est_cost_micros: i64,
}

/// An `ai_usage_global` row: one day's totals across all users (no user ID, no content).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct GlobalAiUsage {
    /// Day (budget timezone, decided by the caller).
    pub day: NaiveDate,
    /// Calls.
    pub calls: i64,
    /// Input tokens.
    pub input_tokens: i64,
    /// Output tokens.
    pub output_tokens: i64,
    /// Estimated cost in micro-USD.
    pub est_cost_micros: i64,
}

/// A setting's `MessagePack` value.
pub async fn get_setting(tx: &mut ScopedTx, key: &str) -> Result<Option<Vec<u8>>> {
    Ok(
        sqlx::query_scalar("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(tx.conn())
            .await?,
    )
}

/// Sets a setting.
pub async fn put_setting(
    tx: &mut ScopedTx,
    key: &str,
    value: &[u8],
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings (user_id, key, value, updated) VALUES (strata_current_user(), $1, $2, $3) \
         ON CONFLICT (user_id, key) DO UPDATE SET value = EXCLUDED.value, updated = EXCLUDED.updated",
    )
    .bind(key)
    .bind(value)
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// All settings, by key.
pub async fn list_settings(tx: &mut ScopedTx) -> Result<Vec<(String, Vec<u8>)>> {
    Ok(
        sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
            .fetch_all(tx.conn())
            .await?,
    )
}

/// Adds one call's usage to the day's totals and returns the new totals.
pub async fn add_ai_usage(tx: &mut ScopedTx, delta: &AiUsage) -> Result<AiUsage> {
    Ok(sqlx::query_as(
        "INSERT INTO ai_usage AS u (user_id, day, provider, model, calls, input_tokens, output_tokens, est_cost_micros) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (user_id, day, provider, model) DO UPDATE SET \
           calls = u.calls + EXCLUDED.calls, input_tokens = u.input_tokens + EXCLUDED.input_tokens, \
           output_tokens = u.output_tokens + EXCLUDED.output_tokens, \
           est_cost_micros = u.est_cost_micros + EXCLUDED.est_cost_micros \
         RETURNING day, provider, model, calls, input_tokens, output_tokens, est_cost_micros",
    )
    .bind(delta.day)
    .bind(&delta.provider)
    .bind(&delta.model)
    .bind(delta.calls)
    .bind(delta.input_tokens)
    .bind(delta.output_tokens)
    .bind(delta.est_cost_micros)
    .fetch_one(tx.conn())
    .await?)
}

/// The scoped user's usage rows for `day`, by provider and model.
pub async fn ai_usage_for_day(tx: &mut ScopedTx, day: NaiveDate) -> Result<Vec<AiUsage>> {
    Ok(sqlx::query_as(
        "SELECT day, provider, model, calls, input_tokens, output_tokens, est_cost_micros \
         FROM ai_usage WHERE day = $1 ORDER BY provider, model",
    )
    .bind(day)
    .fetch_all(tx.conn())
    .await?)
}

/// Adds one call's usage to the day's global totals (AI budget guard, PLAN §9.1) and returns the
/// new totals. Runs in any user's scope; the table is global.
pub async fn add_global_ai_usage(
    tx: &mut ScopedTx,
    delta: &GlobalAiUsage,
) -> Result<GlobalAiUsage> {
    Ok(sqlx::query_as(
        "INSERT INTO ai_usage_global AS g (day, calls, input_tokens, output_tokens, est_cost_micros) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (day) DO UPDATE SET \
           calls = g.calls + EXCLUDED.calls, input_tokens = g.input_tokens + EXCLUDED.input_tokens, \
           output_tokens = g.output_tokens + EXCLUDED.output_tokens, \
           est_cost_micros = g.est_cost_micros + EXCLUDED.est_cost_micros \
         RETURNING day, calls, input_tokens, output_tokens, est_cost_micros",
    )
    .bind(delta.day)
    .bind(delta.calls)
    .bind(delta.input_tokens)
    .bind(delta.output_tokens)
    .bind(delta.est_cost_micros)
    .fetch_one(tx.conn())
    .await?)
}

/// The global totals for `day` across all users, if any call was recorded.
pub async fn global_ai_usage_for_day(
    tx: &mut ScopedTx,
    day: NaiveDate,
) -> Result<Option<GlobalAiUsage>> {
    Ok(sqlx::query_as(
        "SELECT day, calls, input_tokens, output_tokens, est_cost_micros FROM ai_usage_global WHERE day = $1",
    )
    .bind(day)
    .fetch_optional(tx.conn())
    .await?)
}
