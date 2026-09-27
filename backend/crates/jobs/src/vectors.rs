//! Vector storage (migration `…011`): chunk rows with their embeddings, note vectors (the
//! L2-normalised mean of the chunk vectors, PLAN §9.1b) and duplicate-check item vectors
//! (§9.7). Every function takes a [`ScopedTx`]; RLS keeps each user's vectors to themselves.
//!
//! Nearest-neighbour queries scan one user's rows exactly (a materialised distance, then a
//! sort) instead of using an approximate index, which under row-level security could return
//! other users' candidates first and too few of the user's own (see the migration).

use chrono::{DateTime, Utc};
use pgvector::Vector;
use strata_common::{ChunkId, IdGenerator, NoteId};
use strata_index::ScopedTx;

use crate::chunk::ChunkDraft;
use crate::handler::JobError;

/// The mean of `vectors`, L2-normalised (`None` for no vectors or a zero mean).
pub fn normalized_mean(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let first = vectors.first()?;
    let mut sum = vec![0f64; first.len()];
    for v in vectors {
        if v.len() != sum.len() {
            return None;
        }
        for (s, x) in sum.iter_mut().zip(v) {
            *s += f64::from(*x);
        }
    }
    let norm = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)] // unit-length components fit f32
    Some(sum.iter().map(|x| (x / norm) as f32).collect())
}

/// Cosine similarity of two unit vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The stored vector of a note.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct NoteVector {
    /// Model that produced it.
    pub model: String,
    /// Note version it was computed from.
    pub content_hash: String,
    /// The vector.
    pub embedding: Vector,
    /// Chunks averaged.
    pub chunk_count: i32,
}

/// The note vector of `note`, if any.
pub async fn note_vector(tx: &mut ScopedTx, note: NoteId) -> Result<Option<NoteVector>, JobError> {
    Ok(sqlx::query_as(
        "SELECT model, content_hash, embedding, chunk_count FROM note_vectors WHERE note_id = $1",
    )
    .bind(note)
    .fetch_optional(tx.conn())
    .await?)
}

/// Replaces the chunks of `note` and its note vector in one go (`embeddings[i]` belongs to
/// `chunks[i]`).
#[allow(clippy::too_many_arguments)] // one row set, written together
pub async fn store_note_embeddings(
    tx: &mut ScopedTx,
    ids: &dyn IdGenerator,
    note: NoteId,
    version: &str,
    model: &str,
    chunks: &[ChunkDraft],
    embeddings: &[Vec<f32>],
    note_vector: &[f32],
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    sqlx::query("DELETE FROM chunks WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    let int = |v: usize| i32::try_from(v).unwrap_or(i32::MAX);
    for (c, e) in chunks.iter().zip(embeddings) {
        sqlx::query(
            "INSERT INTO chunks (user_id, id, note_id, block_id, text, token_count, embedding, \
               model, ord, heading_path, start_offset, end_offset, anchor_len, content_hash) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(ChunkId::generate(ids))
        .bind(note)
        .bind(&c.block_id)
        .bind(&c.text)
        .bind(int(c.token_count))
        .bind(Vector::from(e.clone()))
        .bind(model)
        .bind(int(c.ord))
        .bind(&c.heading_path)
        .bind(int(c.start))
        .bind(int(c.end))
        .bind(int(c.anchor_len))
        .bind(version)
        .execute(tx.conn())
        .await?;
    }
    sqlx::query(
        "INSERT INTO note_vectors (user_id, note_id, model, content_hash, embedding, chunk_count, updated) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6) \
         ON CONFLICT (user_id, note_id) DO UPDATE SET model = EXCLUDED.model, \
           content_hash = EXCLUDED.content_hash, embedding = EXCLUDED.embedding, \
           chunk_count = EXCLUDED.chunk_count, updated = EXCLUDED.updated",
    )
    .bind(note)
    .bind(model)
    .bind(version)
    .bind(Vector::from(note_vector.to_vec()))
    .bind(int(chunks.len()))
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Removes every vector of `note` (trashed or gone).
pub async fn delete_note_embeddings(tx: &mut ScopedTx, note: NoteId) -> Result<(), JobError> {
    for sql in [
        "DELETE FROM chunks WHERE note_id = $1",
        "DELETE FROM note_vectors WHERE note_id = $1",
        "DELETE FROM dedupe_vectors WHERE note_id = $1",
    ] {
        sqlx::query(sql).bind(note).execute(tx.conn()).await?;
    }
    Ok(())
}

/// A stored chunk.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct StoredChunk {
    /// Chunk ID.
    pub id: ChunkId,
    /// Note.
    pub note_id: NoteId,
    /// Position in the note.
    pub ord: i32,
    /// First block's ID.
    pub block_id: Option<String>,
    /// Text.
    pub text: String,
    /// Heading path.
    pub heading_path: String,
    /// Body offsets.
    pub start_offset: i32,
    /// Body offsets.
    pub end_offset: i32,
    /// First block's length in `text`.
    pub anchor_len: i32,
    /// Note version.
    pub content_hash: String,
}

/// The chunks of `note` with `model` vectors, in order.
pub async fn chunks_of(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<StoredChunk>, JobError> {
    Ok(sqlx::query_as(
        "SELECT id, note_id, ord, block_id, text, heading_path, start_offset, end_offset, \
           anchor_len, content_hash FROM chunks WHERE note_id = $1 ORDER BY ord, id",
    )
    .bind(note)
    .fetch_all(tx.conn())
    .await?)
}

/// A nearest chunk.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct ChunkNeighbour {
    /// Chunk ID.
    pub chunk_id: ChunkId,
    /// Note.
    pub note_id: NoteId,
    /// Cosine similarity.
    pub similarity: f64,
}

/// The `limit` chunks of live notes closest to `query` among `model` vectors (exact).
pub async fn nearest_chunks(
    tx: &mut ScopedTx,
    query: &[f32],
    model: &str,
    limit: i64,
) -> Result<Vec<ChunkNeighbour>, JobError> {
    Ok(sqlx::query_as(
        "WITH d AS MATERIALIZED ( \
           SELECT c.id, c.note_id, 1 - (c.embedding <=> $1)::float8 AS similarity \
           FROM chunks c JOIN notes n ON n.id = c.note_id AND n.user_id = c.user_id \
           WHERE c.model = $2 AND c.embedding IS NOT NULL AND NOT n.trashed) \
         SELECT id AS chunk_id, note_id, similarity FROM d \
         ORDER BY similarity DESC, id LIMIT $3",
    )
    .bind(Vector::from(query.to_vec()))
    .bind(model)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

/// A chunk by ID.
pub async fn chunk(tx: &mut ScopedTx, id: ChunkId) -> Result<Option<StoredChunk>, JobError> {
    Ok(sqlx::query_as(
        "SELECT id, note_id, ord, block_id, text, heading_path, start_offset, end_offset, \
           anchor_len, content_hash FROM chunks WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// A similar note (§9.6).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct NoteNeighbour {
    /// Note.
    pub note_id: NoteId,
    /// Cosine similarity of the note vectors.
    pub similarity: f64,
}

/// The `n` live notes whose note vectors are most similar to `note`'s, at or above `floor`,
/// most similar first (ties by ID). Empty when `note` has no vector for `model`. Computed on
/// request, never stored (PLAN §9.6).
pub async fn similar_notes(
    tx: &mut ScopedTx,
    note: NoteId,
    model: &str,
    n: i64,
    floor: f64,
) -> Result<Vec<NoteNeighbour>, JobError> {
    Ok(sqlx::query_as(
        "WITH q AS (SELECT embedding FROM note_vectors WHERE note_id = $1 AND model = $2), \
         d AS MATERIALIZED ( \
           SELECT v.note_id, 1 - (v.embedding <=> q.embedding)::float8 AS similarity \
           FROM note_vectors v CROSS JOIN q \
           JOIN notes n ON n.id = v.note_id AND n.user_id = v.user_id \
           WHERE v.model = $2 AND v.note_id <> $1 AND NOT n.trashed) \
         SELECT note_id, similarity FROM d WHERE similarity >= $4 \
         ORDER BY similarity DESC, note_id LIMIT $3",
    )
    .bind(note)
    .bind(model)
    .bind(n)
    .bind(floor)
    .fetch_all(tx.conn())
    .await?)
}

/// Live notes and how many of them have a current vector for `model` (`AiStatus`).
pub async fn coverage(tx: &mut ScopedTx, model: &str) -> Result<(u64, u64), JobError> {
    let (embedded, total): (i64, i64) = sqlx::query_as(
        "SELECT count(v.note_id), count(*) FROM notes n \
         LEFT JOIN note_vectors v ON v.note_id = n.id AND v.user_id = n.user_id \
           AND v.model = $1 AND v.content_hash = n.content_hash \
         WHERE NOT n.trashed",
    )
    .bind(model)
    .fetch_one(tx.conn())
    .await?;
    Ok((
        u64::try_from(embedded).unwrap_or(0),
        u64::try_from(total).unwrap_or(0),
    ))
}

/// Up to `limit` live notes without a current vector for `model` and without a queued embed
/// job, most recently updated first (the backfill's next batch).
pub async fn stale_notes(
    tx: &mut ScopedTx,
    model: &str,
    limit: i64,
) -> Result<Vec<NoteId>, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT n.id FROM notes n \
         LEFT JOIN note_vectors v ON v.note_id = n.id AND v.user_id = n.user_id \
         WHERE NOT n.trashed \
           AND (v.note_id IS NULL OR v.model <> $1 OR v.content_hash <> n.content_hash) \
           AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.kind = 'embed' AND j.note_id = n.id \
                           AND j.status IN ('queued', 'running')) \
         ORDER BY n.updated DESC, n.id LIMIT $2",
    )
    .bind(model)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

// ---- duplicate-check item vectors (§9.7) ----------------------------------------------------

/// A stored duplicate-check item (`dedupe_keys`, first name).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredItem {
    /// Kind.
    pub kind: String,
    /// Item ID.
    pub item_id: String,
    /// The decoded item.
    pub item: dedupe::Item,
}

fn decode_items(rows: Vec<(String, String, Vec<u8>)>) -> Vec<StoredItem> {
    rows.into_iter()
        .filter_map(|(kind, item_id, blob)| {
            rmp_serde::from_slice::<dedupe::Item>(&blob)
                .ok()
                .map(|item| StoredItem {
                    kind,
                    item_id,
                    item,
                })
        })
        .collect()
}

/// The duplicate-check items of `note`: the note itself and its task lines.
pub async fn items_of_note(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<StoredItem>, JobError> {
    let rows: Vec<(String, String, Vec<u8>)> = sqlx::query_as(
        "SELECT DISTINCT ON (kind, item_id) kind, item_id, item FROM dedupe_keys \
         WHERE item IS NOT NULL AND (item_id = $1 \
           OR item_id IN (SELECT id FROM tasks WHERE note_id = $2)) \
         ORDER BY kind, item_id, key_no",
    )
    .bind(note.to_string())
    .bind(note)
    .fetch_all(tx.conn())
    .await?;
    Ok(decode_items(rows))
}

/// Every duplicate-check item of the user.
pub async fn all_items(tx: &mut ScopedTx) -> Result<Vec<StoredItem>, JobError> {
    let rows: Vec<(String, String, Vec<u8>)> = sqlx::query_as(
        "SELECT DISTINCT ON (kind, item_id) kind, item_id, item FROM dedupe_keys \
         WHERE item IS NOT NULL ORDER BY kind, item_id, key_no",
    )
    .fetch_all(tx.conn())
    .await?;
    Ok(decode_items(rows))
}

/// The text a duplicate-check item is embedded from: its text, or its title.
pub fn item_text(item: &dedupe::Item) -> &str {
    if item.text.trim().is_empty() {
        &item.title
    } else {
        &item.text
    }
}

/// SHA-256 (hex) of `model` NUL `text` (skip re-embedding unchanged items).
pub fn text_hash(model: &str, text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(model.as_bytes());
    h.update([0u8]);
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

/// Stored text hashes of `note`'s item vectors: (kind, item ID) → hash.
pub async fn item_hashes(
    tx: &mut ScopedTx,
    note: NoteId,
) -> Result<Vec<(String, String, String)>, JobError> {
    Ok(sqlx::query_as(
        "SELECT kind, item_id, text_hash FROM dedupe_vectors WHERE note_id = $1 \
         ORDER BY kind, item_id",
    )
    .bind(note)
    .fetch_all(tx.conn())
    .await?)
}

/// Stores the vector of an item.
#[allow(clippy::too_many_arguments)] // one row
pub async fn upsert_item_vector(
    tx: &mut ScopedTx,
    kind: &str,
    item_id: &str,
    note: NoteId,
    model: &str,
    hash: &str,
    vector: &[f32],
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    sqlx::query(
        "INSERT INTO dedupe_vectors (user_id, kind, item_id, note_id, model, text_hash, embedding, updated) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (user_id, kind, item_id) DO UPDATE SET note_id = EXCLUDED.note_id, \
           model = EXCLUDED.model, text_hash = EXCLUDED.text_hash, \
           embedding = EXCLUDED.embedding, updated = EXCLUDED.updated",
    )
    .bind(kind)
    .bind(item_id)
    .bind(note)
    .bind(model)
    .bind(hash)
    .bind(Vector::from(vector.to_vec()))
    .bind(now)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Removes item vectors of `note` that are not in `keep` ((kind, item ID) pairs).
pub async fn prune_item_vectors(
    tx: &mut ScopedTx,
    note: NoteId,
    keep: &[(String, String)],
) -> Result<(), JobError> {
    let kinds: Vec<&str> = keep.iter().map(|(k, _)| k.as_str()).collect();
    let items: Vec<&str> = keep.iter().map(|(_, i)| i.as_str()).collect();
    sqlx::query(
        "DELETE FROM dedupe_vectors WHERE note_id = $1 AND (kind, item_id) NOT IN \
         (SELECT * FROM unnest($2::text[], $3::text[]))",
    )
    .bind(note)
    .bind(&kinds)
    .bind(&items)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// An item whose vector is close to a query vector.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemNeighbour {
    /// The stored item.
    pub item: StoredItem,
    /// Its note.
    pub note_id: Option<NoteId>,
    /// Cosine similarity.
    pub similarity: f32,
}

/// Items of `kinds` with `model` vectors at cosine ≥ `floor` from `query`, most similar
/// first, at most `limit` (exact scan; only items still in `dedupe_keys`).
pub async fn nearest_items(
    tx: &mut ScopedTx,
    query: &[f32],
    model: &str,
    kinds: &[String],
    floor: f64,
    limit: i64,
) -> Result<Vec<ItemNeighbour>, JobError> {
    type Row = (String, String, Option<NoteId>, f64, Vec<u8>);
    let rows: Vec<Row> = sqlx::query_as(
        "WITH d AS MATERIALIZED ( \
           SELECT v.kind, v.item_id, v.note_id, 1 - (v.embedding <=> $1)::float8 AS similarity \
           FROM dedupe_vectors v WHERE v.model = $2 AND v.kind = ANY($3)) \
         SELECT d.kind, d.item_id, d.note_id, d.similarity, k.item FROM d \
         JOIN LATERAL (SELECT item FROM dedupe_keys k WHERE k.kind = d.kind \
           AND k.item_id = d.item_id AND k.item IS NOT NULL ORDER BY k.key_no LIMIT 1) k ON true \
         WHERE d.similarity >= $4 ORDER BY d.similarity DESC, d.kind, d.item_id LIMIT $5",
    )
    .bind(Vector::from(query.to_vec()))
    .bind(model)
    .bind(kinds)
    .bind(floor)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(kind, item_id, note_id, similarity, blob)| {
            let item = rmp_serde::from_slice::<dedupe::Item>(&blob).ok()?;
            #[allow(clippy::cast_possible_truncation)] // cosine in [-1, 1]
            Some(ItemNeighbour {
                item: StoredItem {
                    kind,
                    item_id,
                    item,
                },
                note_id,
                similarity: similarity as f32,
            })
        })
        .collect())
}

/// The stored vector of an item.
pub async fn item_vector(
    tx: &mut ScopedTx,
    kind: &str,
    item_id: &str,
    model: &str,
) -> Result<Option<(Vec<f32>, Option<NoteId>)>, JobError> {
    let row: Option<(Vector, Option<NoteId>)> = sqlx::query_as(
        "SELECT embedding, note_id FROM dedupe_vectors WHERE kind = $1 AND item_id = $2 AND model = $3",
    )
    .bind(kind)
    .bind(item_id)
    .bind(model)
    .fetch_optional(tx.conn())
    .await?;
    Ok(row.map(|(v, n)| (v.to_vec(), n)))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn mean_is_normalised_and_rejects_ragged_or_zero_input() {
        let m = normalized_mean(&[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]]).expect("mean");
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(m, vec![h, h, 0.0]);
        assert_eq!(normalized_mean(&[vec![0.6, 0.8]]), Some(vec![0.6, 0.8]));
        assert_eq!(normalized_mean(&[]), None);
        assert_eq!(normalized_mean(&[vec![1.0], vec![1.0, 0.0]]), None);
        assert_eq!(normalized_mean(&[vec![1.0, 0.0], vec![-1.0, 0.0]]), None);
        assert!((cosine(&m, &m) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn text_hash_separates_model_and_text() {
        assert_ne!(text_hash("m", "ab"), text_hash("ma", "b"));
        assert_eq!(text_hash("m", "x").len(), 64);
    }
}
