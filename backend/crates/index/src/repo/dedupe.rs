//! Duplicate keys (§9.7): exact lookup on the normalised key, near lookup by trigram
//! similarity, and remembered keep-both pairs.

use chrono::{DateTime, Utc};

use crate::error::Result;
use crate::repo::entities::set_trigram_threshold;
use crate::scope::ScopedTx;

/// A near-duplicate candidate.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct NearMatch {
    /// Candidate item.
    pub item_id: String,
    /// Trigram similarity 0..1.
    pub score: f32,
}

/// Inserts or replaces an item's keys (normalise both with `/crates/text-normalize`).
pub async fn upsert_key(
    tx: &mut ScopedTx,
    kind: &str,
    item_id: &str,
    exact_key: &str,
    trigram_text: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO dedupe_keys (user_id, kind, item_id, exact_key, trigram_text) \
         VALUES (strata_current_user(), $1, $2, $3, $4) \
         ON CONFLICT (user_id, kind, item_id) DO UPDATE SET exact_key = EXCLUDED.exact_key, \
           trigram_text = EXCLUDED.trigram_text",
    )
    .bind(kind)
    .bind(item_id)
    .bind(exact_key)
    .bind(trigram_text)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Removes an item's keys. False if absent.
pub async fn remove_key(tx: &mut ScopedTx, kind: &str, item_id: &str) -> Result<bool> {
    let done = sqlx::query("DELETE FROM dedupe_keys WHERE kind = $1 AND item_id = $2")
        .bind(kind)
        .bind(item_id)
        .execute(tx.conn())
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Items of `kind` whose exact key equals `exact_key`, sorted.
pub async fn exact_matches(tx: &mut ScopedTx, kind: &str, exact_key: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT item_id FROM dedupe_keys WHERE kind = $1 AND exact_key = $2 ORDER BY item_id",
    )
    .bind(kind)
    .bind(exact_key)
    .fetch_all(tx.conn())
    .await?)
}

/// Items of `kind` with trigram similarity ≥ `threshold`, best first, ties by ID.
pub async fn near_matches(
    tx: &mut ScopedTx,
    kind: &str,
    trigram_text: &str,
    threshold: f32,
    limit: i64,
) -> Result<Vec<NearMatch>> {
    set_trigram_threshold(tx, threshold).await?;
    Ok(sqlx::query_as(
        "SELECT item_id, similarity(trigram_text, $2) AS score FROM dedupe_keys \
         WHERE kind = $1 AND trigram_text % $2 ORDER BY score DESC, item_id LIMIT $3",
    )
    .bind(kind)
    .bind(trigram_text)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

fn ordered<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Remembers "keep both" for a pair (order-insensitive, idempotent). A pair of equal IDs is
/// rejected by the table's check constraint.
pub async fn add_keep_both(
    tx: &mut ScopedTx,
    kind: &str,
    a: &str,
    b: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    let (lo, hi) = ordered(a, b);
    sqlx::query(
        "INSERT INTO dedupe_keep_both (user_id, kind, a_id, b_id, at) \
         VALUES (strata_current_user(), $1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(kind)
    .bind(lo)
    .bind(hi)
    .bind(at)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// True if the pair was marked keep-both (either order).
pub async fn is_keep_both(tx: &mut ScopedTx, kind: &str, a: &str, b: &str) -> Result<bool> {
    let (lo, hi) = ordered(a, b);
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM dedupe_keep_both WHERE kind = $1 AND a_id = $2 AND b_id = $3)",
    )
    .bind(kind)
    .bind(lo)
    .bind(hi)
    .fetch_one(tx.conn())
    .await?)
}

/// Every item kept together with `item`, sorted.
pub async fn keep_both_partners(tx: &mut ScopedTx, kind: &str, item: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT other FROM ( \
           SELECT CASE WHEN a_id = $2 THEN b_id ELSE a_id END AS other FROM dedupe_keep_both \
           WHERE kind = $1 AND (a_id = $2 OR b_id = $2)) p \
         ORDER BY other COLLATE \"C\"",
    )
    .bind(kind)
    .bind(item)
    .fetch_all(tx.conn())
    .await?)
}
