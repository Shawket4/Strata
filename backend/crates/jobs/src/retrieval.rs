//! Semantic and hybrid retrieval (PLAN §7.5 Search, §9.5, §9.6).
//!
//! - **Semantic search**: the query is embedded and compared with every chunk vector of the
//!   user's live notes (current model only); a note ranks by its best chunk, scored by cosine
//!   similarity.
//! - **Hybrid search**: reciprocal-rank fusion (RRF, `k = 60`) of the full-text ranking
//!   (Arabic/Latin-normalised Postgres FTS, the keyword mode) and the semantic ranking:
//!   `score = Σ 1 / (60 + rank)`; ties by note ID.
//! - **Ask retrieval**: the same fusion at chunk level — the vector ranking of chunks and the
//!   keyword ranking (full-text notes in rank order, each note's chunks by how many query terms
//!   they contain) — giving the top chunks to cite.
//! - **Similarity edges**: top-n neighbours of a note by note vector above a floor, computed
//!   on request (never stored).
//!
//! Every query runs in the caller's scoped transaction, so row-level security keeps it to the
//! caller's own notes and chunks (principle 7).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use strata_ai::{EmbedError, Embedder};
use strata_common::{ChunkId, NoteId};
use strata_index::repo::notes::{self, Note};
use strata_index::{AppDb, UserScope};
use text_normalize::normalize_for_search;

use crate::handler::JobError;
use crate::vectors::{self, StoredChunk};

/// The RRF constant.
pub const RRF_K: f64 = 60.0;

/// Search modes that need embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Embedding similarity.
    Semantic,
    /// Keyword and semantic fused.
    Hybrid,
}

/// A retrieval failure.
#[derive(Debug, thiserror::Error)]
pub enum RetrievalError {
    /// The query is empty.
    #[error("the query is empty")]
    EmptyQuery,
    /// The embedding model could not run.
    #[error("embeddings unavailable: {0}")]
    Unavailable(EmbedError),
    /// Database failure.
    #[error("{0}")]
    Store(JobError),
}

impl From<JobError> for RetrievalError {
    fn from(e: JobError) -> Self {
        Self::Store(e)
    }
}

impl From<strata_index::IndexError> for RetrievalError {
    fn from(e: strata_index::IndexError) -> Self {
        Self::Store(e.into())
    }
}

/// A search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The note.
    pub note: Note,
    /// Cosine similarity (semantic) or RRF score (hybrid).
    pub score: f64,
    /// The first line of the best-matching chunk.
    pub snippet: Option<String>,
}

/// A retrieved chunk with its note.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedChunk {
    /// The chunk.
    pub chunk: StoredChunk,
    /// Its note.
    pub note: Note,
    /// Fused score.
    pub score: f64,
}

/// Embedding-backed retrieval for one embedding model.
#[derive(Debug, Clone)]
pub struct Retriever {
    db: AppDb,
    embedder: Arc<dyn Embedder>,
}

/// `Σ 1 / (k + rank)` over rankings (1-based ranks); ties by key.
pub fn rrf<K: Ord + Clone>(rankings: &[Vec<K>]) -> Vec<(K, f64)> {
    let mut scores: BTreeMap<K, f64> = BTreeMap::new();
    for ranking in rankings {
        for (i, key) in ranking.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)] // ranks are small
            let rank = (i + 1) as f64;
            *scores.entry(key.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank);
        }
    }
    let mut out: Vec<(K, f64)> = scores.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// The first non-empty line of `text`, at most 160 characters.
pub fn first_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let s: String = line.chars().take(160).collect();
    Some(if line.chars().count() > 160 {
        format!("{s}…")
    } else {
        s
    })
}

fn query_terms(normalized: &str) -> Vec<String> {
    normalized
        .split_whitespace()
        .filter(|t| !t.starts_with('-') && *t != "or")
        .map(|t| t.trim_matches('"').to_owned())
        .filter(|t| !t.is_empty())
        .collect()
}

impl Retriever {
    /// Retrieval with `embedder` (its model ID selects the vectors).
    pub fn new(db: AppDb, embedder: Arc<dyn Embedder>) -> Self {
        Self { db, embedder }
    }

    /// The embedder.
    pub fn embedder(&self) -> &Arc<dyn Embedder> {
        &self.embedder
    }

    /// Embeds a query.
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>, RetrievalError> {
        let mut out = self
            .embedder
            .embed(&[query.to_owned()])
            .await
            .map_err(RetrievalError::Unavailable)?;
        out.pop()
            .map(|e| e.vector)
            .ok_or(RetrievalError::Unavailable(EmbedError::Output(
                "no vector returned".into(),
            )))
    }

    /// `GET /search?mode=semantic|hybrid`: up to `limit` notes, best first.
    pub async fn search(
        &self,
        scope: &UserScope,
        query: &str,
        mode: Mode,
        limit: u32,
    ) -> Result<Vec<Hit>, RetrievalError> {
        if query.trim().is_empty() {
            return Err(RetrievalError::EmptyQuery);
        }
        let limit = usize::try_from(limit.clamp(1, 100)).unwrap_or(20);
        let qv = self.embed_query(query).await?;
        let model = self.embedder.model_id().to_owned();
        let mut tx = self.db.begin(scope).await?;
        let near = vectors::nearest_chunks(
            &mut tx,
            &qv,
            &model,
            i64::try_from(limit * 5).unwrap_or(500),
        )
        .await?;
        // Best chunk per note, in order.
        let mut best: Vec<(NoteId, f64, ChunkId)> = Vec::new();
        for n in near {
            if !best.iter().any(|(id, _, _)| *id == n.note_id) {
                best.push((n.note_id, n.similarity, n.chunk_id));
            }
        }
        let ranked: Vec<(NoteId, f64)> = match mode {
            Mode::Semantic => best.iter().map(|(id, s, _)| (*id, *s)).collect(),
            Mode::Hybrid => {
                let normalized = normalize_for_search(query);
                let fts: Vec<NoteId> = if normalized.is_empty() {
                    Vec::new()
                } else {
                    notes::search_notes(
                        &mut tx,
                        &normalized,
                        i64::try_from(limit * 2).unwrap_or(200),
                    )
                    .await?
                    .into_iter()
                    .map(|h| h.note_id)
                    .collect()
                };
                let semantic: Vec<NoteId> = best.iter().map(|(id, _, _)| *id).collect();
                rrf(&[fts, semantic])
            }
        };
        let mut hits = Vec::new();
        for (id, score) in ranked {
            if hits.len() == limit {
                break;
            }
            let Some(note) = notes::get_note(&mut tx, id).await?.filter(|n| !n.trashed) else {
                continue;
            };
            let chunk_id = best.iter().find(|(n, _, _)| *n == id).map(|(_, _, c)| *c);
            let snippet = match chunk_id {
                Some(c) => vectors::chunk(&mut tx, c).await?,
                None => vectors::chunks_of(&mut tx, id).await?.into_iter().next(),
            }
            .and_then(|c| first_line(&c.text));
            hits.push(Hit {
                note,
                score,
                snippet,
            });
        }
        tx.commit().await?;
        Ok(hits)
    }

    /// The top `k` chunks for an Ask question (hybrid, chunk level), optionally only from
    /// notes under `folder`.
    pub async fn ask_chunks(
        &self,
        scope: &UserScope,
        question: &str,
        k: usize,
        folder: Option<&str>,
    ) -> Result<Vec<RankedChunk>, RetrievalError> {
        if question.trim().is_empty() {
            return Err(RetrievalError::EmptyQuery);
        }
        let qv = self.embed_query(question).await?;
        let model = self.embedder.model_id().to_owned();
        let pool = i64::try_from(k.max(1) * 6).unwrap_or(60);
        let mut tx = self.db.begin(scope).await?;
        let semantic: Vec<ChunkId> = vectors::nearest_chunks(&mut tx, &qv, &model, pool)
            .await?
            .into_iter()
            .map(|n| n.chunk_id)
            .collect();
        let normalized = normalize_for_search(question);
        let terms = query_terms(&normalized);
        let mut keyword: Vec<ChunkId> = Vec::new();
        let mut chunk_cache: HashMap<ChunkId, StoredChunk> = HashMap::new();
        if !normalized.is_empty() {
            for hit in notes::search_notes(&mut tx, &normalized, pool).await? {
                let mut chunks = vectors::chunks_of(&mut tx, hit.note_id).await?;
                let hits = |c: &StoredChunk| {
                    let text = normalize_for_search(&c.text);
                    terms.iter().filter(|t| text.contains(t.as_str())).count()
                };
                chunks.sort_by(|a, b| hits(b).cmp(&hits(a)).then(a.ord.cmp(&b.ord)));
                let any_hit = chunks.iter().any(|c| hits(c) > 0);
                for c in chunks {
                    if !any_hit || hits(&c) > 0 {
                        keyword.push(c.id);
                        chunk_cache.insert(c.id, c);
                    }
                }
            }
        }
        let fused = rrf(&[semantic, keyword]);
        let mut out = Vec::new();
        let mut notes_cache: HashMap<NoteId, Option<Note>> = HashMap::new();
        for (id, score) in fused {
            if out.len() == k {
                break;
            }
            let chunk = match chunk_cache.remove(&id) {
                Some(c) => Some(c),
                None => vectors::chunk(&mut tx, id).await?,
            };
            let Some(chunk) = chunk else { continue };
            let note = if let Some(n) = notes_cache.get(&chunk.note_id) {
                n.clone()
            } else {
                let n = notes::get_note(&mut tx, chunk.note_id).await?;
                notes_cache.insert(chunk.note_id, n.clone());
                n
            };
            let Some(note) = note.filter(|n| !n.trashed) else {
                continue;
            };
            if let Some(f) = folder
                && !note.path.starts_with(&format!("{}/", f.trim_end_matches('/')))
            {
                continue;
            }
            out.push(RankedChunk { chunk, note, score });
        }
        tx.commit().await?;
        Ok(out)
    }

    /// Similarity edges (§9.6): the `n` notes most similar to `note` at cosine ≥ `floor`.
    /// Empty when the note has no vector yet.
    pub async fn similar(
        &self,
        scope: &UserScope,
        note: NoteId,
        n: u32,
        floor: f64,
    ) -> Result<Vec<(NoteId, f64)>, RetrievalError> {
        similar_notes(&self.db, scope, self.embedder.model_id(), note, n, floor).await
    }
}

/// Similarity edges (§9.6) for vectors of `model` (no embedder needed: only stored vectors).
pub async fn similar_notes(
    db: &AppDb,
    scope: &UserScope,
    model: &str,
    note: NoteId,
    n: u32,
    floor: f64,
) -> Result<Vec<(NoteId, f64)>, RetrievalError> {
    let mut tx = db.begin(scope).await?;
    let out = vectors::similar_notes(&mut tx, note, model, i64::from(n), floor)
        .await?
        .into_iter()
        .map(|s| (s.note_id, s.similarity))
        .collect();
    tx.commit().await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn rrf_sums_reciprocal_ranks_and_breaks_ties_by_key() {
        let fused = rrf(&[vec!["a", "b", "c"], vec!["c", "a"]]);
        let keys: Vec<&str> = fused.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec!["a", "c", "b"]);
        assert!((fused[0].1 - (1.0 / 61.0 + 1.0 / 62.0)).abs() < 1e-12);
        assert!((fused[1].1 - (1.0 / 63.0 + 1.0 / 61.0)).abs() < 1e-12);
        assert!((fused[2].1 - 1.0 / 62.0).abs() < 1e-12);
        // Equal scores: by key.
        let tie = rrf(&[vec!["y"], vec!["x"]]);
        assert_eq!(tie.iter().map(|(k, _)| *k).collect::<Vec<_>>(), vec!["x", "y"]);
    }

    #[test]
    fn first_lines_are_trimmed_and_cut() {
        assert_eq!(first_line("\n  hello  \nworld"), Some("hello".into()));
        assert_eq!(first_line("   \n"), None);
        let long = "x".repeat(200);
        assert_eq!(first_line(&long), Some(format!("{}…", "x".repeat(160))));
    }
}
