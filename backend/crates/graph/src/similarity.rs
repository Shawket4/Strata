//! Similarity edges (PLAN §9.6): computed on request from note-level embeddings, never
//! stored. [`SimilaritySource`] is what the graph needs; [`NoteVectorSimilarity`] implements
//! it over the `note_vectors` written by the `embed` job, delegating single-note queries to
//! `strata_jobs::retrieval::similar_notes`.
//!
//! The global graph needs top-n neighbours of *every* note: an exact all-pairs pass over one
//! user's vectors (vector queries under RLS are exact scans anyway, see "Jobs and AI
//! pipelines" in `docs/ARCHITECTURE.md`). Its cost is `O(m² · 384)`, so it covers the
//! `max_notes` most recently updated notes and reports [`SimilarPairs::truncated`] beyond.

use std::collections::BTreeMap;
use std::fmt;

use strata_common::NoteId;
use strata_index::{AppDb, UserScope};

use crate::assemble::SimilarPair;
use crate::error::{GraphError, Result};

/// Top-n similarity pairs of a whole vault.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimilarPairs {
    /// Pairs, by (source, target); each unordered pair once.
    pub pairs: Vec<SimilarPair>,
    /// Only the most recently updated notes were compared.
    pub truncated: bool,
}

/// Where similarity edges come from.
#[async_trait::async_trait]
pub trait SimilaritySource: Send + Sync + fmt::Debug {
    /// The notes most similar to `note` (above the floor), most similar first.
    async fn similar_to(&self, scope: &UserScope, note: NoteId) -> Result<Vec<(NoteId, f64)>>;

    /// Top-n neighbours of every note with a vector.
    async fn pairs(&self, scope: &UserScope) -> Result<SimilarPairs>;
}

/// Parameters of [`NoteVectorSimilarity`].
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarityConfig {
    /// Neighbours per note.
    pub top_n: u32,
    /// Minimum cosine similarity.
    pub floor: f64,
    /// Notes compared by the all-pairs pass (most recently updated first).
    pub max_notes: usize,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        Self {
            top_n: 5,
            floor: 0.75,
            max_notes: 2_000,
        }
    }
}

/// Similarity over stored note vectors of one embedding model.
#[derive(Debug, Clone)]
pub struct NoteVectorSimilarity {
    db: AppDb,
    model: String,
    config: SimilarityConfig,
}

impl NoteVectorSimilarity {
    /// Similarity over vectors of `model` (the configured embedder's model ID).
    pub fn new(db: AppDb, model: impl Into<String>, config: SimilarityConfig) -> Self {
        Self {
            db,
            model: model.into(),
            config,
        }
    }
}

/// Top-`n` neighbours (cosine ≥ `floor`) of every vector, merged into unordered pairs (the
/// source is the smaller ID; a pair found from both sides appears once). Ties by ID.
pub fn top_pairs(vectors: &[(NoteId, Vec<f32>)], n: usize, floor: f64) -> Vec<SimilarPair> {
    let unit: Vec<(NoteId, Vec<f64>)> = vectors
        .iter()
        .filter_map(|(id, v)| {
            let norm = v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            (norm > 0.0).then(|| (*id, v.iter().map(|x| f64::from(*x) / norm).collect()))
        })
        .collect();
    let mut pairs: BTreeMap<(NoteId, NoteId), f64> = BTreeMap::new();
    for (i, (a, va)) in unit.iter().enumerate() {
        let mut near: Vec<(f64, NoteId)> = unit
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, (b, vb))| (va.iter().zip(vb).map(|(x, y)| x * y).sum::<f64>(), *b))
            .filter(|(s, _)| *s >= floor)
            .collect();
        near.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        for (score, b) in near.into_iter().take(n) {
            let key = if *a < b { (*a, b) } else { (b, *a) };
            pairs.entry(key).or_insert(score);
        }
    }
    pairs
        .into_iter()
        .map(|((source, target), score)| SimilarPair {
            source,
            target,
            score: round(score),
        })
        .collect()
}

/// Rounds to 6 decimals, so tiny float differences never change a response.
fn round(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

#[async_trait::async_trait]
impl SimilaritySource for NoteVectorSimilarity {
    async fn similar_to(&self, scope: &UserScope, note: NoteId) -> Result<Vec<(NoteId, f64)>> {
        let out = strata_jobs::retrieval::similar_notes(
            &self.db,
            scope,
            &self.model,
            note,
            self.config.top_n,
            self.config.floor,
        )
        .await
        .map_err(|e| GraphError::Similarity(e.to_string()))?;
        Ok(out.into_iter().map(|(n, s)| (n, round(s))).collect())
    }

    async fn pairs(&self, scope: &UserScope) -> Result<SimilarPairs> {
        let mut tx = self.db.begin(scope).await?;
        let limit = i64::try_from(self.config.max_notes).unwrap_or(i64::MAX);
        let rows: Vec<(NoteId, pgvector::Vector)> = sqlx::query_as(
            "SELECT v.note_id, v.embedding FROM note_vectors v \
             JOIN notes n ON n.user_id = v.user_id AND n.id = v.note_id AND NOT n.trashed \
             WHERE v.model = $1 ORDER BY n.updated DESC, n.id LIMIT $2 + 1",
        )
        .bind(&self.model)
        .bind(limit)
        .fetch_all(tx.conn())
        .await?;
        tx.commit().await?;
        let truncated = rows.len() > self.config.max_notes;
        let vectors: Vec<(NoteId, Vec<f32>)> = rows
            .into_iter()
            .take(self.config.max_notes)
            .map(|(id, v)| (id, v.to_vec()))
            .collect();
        let (n, floor) = (self.config.top_n as usize, self.config.floor);
        let pairs = tokio::task::spawn_blocking(move || top_pairs(&vectors, n, floor))
            .await
            .map_err(|e| GraphError::Similarity(format!("similarity task failed: {e}")))?;
        Ok(SimilarPairs { pairs, truncated })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> NoteId {
        NoteId::from_ulid(ulid::Ulid::from_parts(n, 0))
    }

    #[test]
    fn top_pairs_are_exact_merged_and_floored() {
        let v = vec![
            (id(1), vec![1.0, 0.0]),
            (id(2), vec![0.8, 0.6]),
            (id(3), vec![0.0, 1.0]),
            (id(4), vec![0.0, 0.0]),
        ];
        assert_eq!(
            top_pairs(&v, 1, 0.5),
            vec![
                SimilarPair {
                    source: id(1),
                    target: id(2),
                    score: 0.8
                },
                SimilarPair {
                    source: id(2),
                    target: id(3),
                    score: 0.6
                },
            ]
        );
        assert_eq!(top_pairs(&v, 5, 0.9), vec![]);
    }
}
