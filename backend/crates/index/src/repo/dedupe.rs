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
         ON CONFLICT (user_id, kind, item_id, key_no) DO UPDATE SET exact_key = EXCLUDED.exact_key, \
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

/// Removes an item's keys (every name). False if absent.
pub async fn remove_key(tx: &mut ScopedTx, kind: &str, item_id: &str) -> Result<bool> {
    let done = sqlx::query("DELETE FROM dedupe_keys WHERE kind = $1 AND item_id = $2")
        .bind(kind)
        .bind(item_id)
        .execute(tx.conn())
        .await?;
    Ok(done.rows_affected() >= 1)
}

/// Items of `kind` whose exact key equals `exact_key`, sorted.
pub async fn exact_matches(tx: &mut ScopedTx, kind: &str, exact_key: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT DISTINCT item_id FROM dedupe_keys WHERE kind = $1 AND exact_key = $2 ORDER BY item_id",
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
        "SELECT item_id, max(similarity(trigram_text, $2)) AS score FROM dedupe_keys \
         WHERE kind = $1 AND trigram_text % $2 GROUP BY item_id ORDER BY score DESC, item_id LIMIT $3",
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

/// A keep-both pair to store: (kind, a, b, when). Order-insensitive.
pub type KeepBothPair<'a> = (&'a str, &'a str, &'a str, DateTime<Utc>);

/// [`add_keep_both`] for many pairs in one statement (a forced create can record hundreds).
pub async fn add_keep_both_many(tx: &mut ScopedTx, pairs: &[KeepBothPair<'_>]) -> Result<()> {
    if pairs.is_empty() {
        return Ok(());
    }
    let mut kinds = Vec::with_capacity(pairs.len());
    let mut los = Vec::with_capacity(pairs.len());
    let mut his = Vec::with_capacity(pairs.len());
    let mut ats = Vec::with_capacity(pairs.len());
    for (kind, a, b, at) in pairs {
        let (lo, hi) = ordered(a, b);
        kinds.push(*kind);
        los.push(lo);
        his.push(hi);
        ats.push(*at);
    }
    sqlx::query(
        "INSERT INTO dedupe_keep_both (user_id, kind, a_id, b_id, at)          SELECT strata_current_user(), k, a, b, t          FROM unnest($1::text[], $2::text[], $3::text[], $4::timestamptz[]) AS p(k, a, b, t)          ON CONFLICT DO NOTHING",
    )
    .bind(&kinds)
    .bind(&los)
    .bind(&his)
    .bind(&ats)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Which of the (kind, a, b) pairs (order-insensitive) are stored as keep-both, as stored
/// (a < b), sorted. One primary-key probe per pair, whatever the size of the table.
pub async fn keep_both_among(
    tx: &mut ScopedTx,
    pairs: &[(String, String, String)],
) -> Result<Vec<(String, String, String)>> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let mut kinds = Vec::with_capacity(pairs.len());
    let mut los = Vec::with_capacity(pairs.len());
    let mut his = Vec::with_capacity(pairs.len());
    for (kind, a, b) in pairs {
        let (lo, hi) = ordered(a, b);
        kinds.push(kind.as_str());
        los.push(lo);
        his.push(hi);
    }
    Ok(sqlx::query_as(
        "SELECT DISTINCT k.kind, k.a_id, k.b_id          FROM unnest($1::text[], $2::text[], $3::text[]) AS p(kind, a, b)          JOIN dedupe_keep_both k ON k.kind = p.kind AND k.a_id = p.a AND k.b_id = p.b          ORDER BY 1, 2, 3",
    )
    .bind(&kinds)
    .bind(&los)
    .bind(&his)
    .fetch_all(tx.conn())
    .await?)
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
