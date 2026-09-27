//! The semantic level of the synchronous duplicate check on create (PLAN §9.7), registered on
//! the vault store with `VaultService::set_semantic`.
//!
//! It only answers when an embedding is cheap: the model is already loaded (a create never
//! waits for a model load) and the new text is short ([`MAX_TEXT_CHARS`]). The new item's
//! text is embedded and compared with the stored item vectors of compatible kinds (computed
//! by the `embed` job from the same text the exact and near levels use); matches at or above
//! the kind's candidate threshold are returned as evidence. Borderline scores are not
//! confirmed by an LLM here (that would block the create on a `claude -p` call), so only
//! cosines at or above the confirmed threshold turn into `409 duplicate_candidates`; the
//! nightly sweep confirms borderline pairs. Any failure yields no evidence: exact and near
//! still run (principle 6).

use std::sync::Arc;

use futures::future::BoxFuture;
use strata_ai::Embedder;
use strata_index::ScopedTx;
use strata_vault::semantic::{SemanticDuplicates, SemanticMatch};

use crate::vectors;

/// Longest text (characters) embedded on the create path.
pub const MAX_TEXT_CHARS: usize = 600;

/// Stored items compared per check.
const LIMIT: i64 = 20;

/// The semantic duplicate source.
#[derive(Debug, Clone)]
pub struct SemanticDupSource {
    embedder: Arc<dyn Embedder>,
    thresholds: dedupe::Thresholds,
}

impl SemanticDupSource {
    /// A source embedding with `embedder`, deciding with `thresholds`.
    pub fn new(embedder: Arc<dyn Embedder>, thresholds: dedupe::Thresholds) -> Self {
        Self {
            embedder,
            thresholds,
        }
    }

    async fn find(&self, tx: &mut ScopedTx, item: &dedupe::Item) -> Vec<SemanticMatch> {
        if !self.embedder.is_loaded() {
            return Vec::new();
        }
        let text = vectors::item_text(item);
        if text.trim().is_empty() || text.chars().count() > MAX_TEXT_CHARS {
            return Vec::new();
        }
        let kinds: Vec<dedupe::DedupeKind> = dedupe::compatible_kinds(item.kind)
            .iter()
            .copied()
            .filter(|k| self.thresholds.semantic_between(item.kind, *k).is_some())
            .collect();
        let Some(floor) = kinds
            .iter()
            .filter_map(|k| self.thresholds.semantic_between(item.kind, *k))
            .map(|t| t.candidate)
            .reduce(f32::min)
        else {
            return Vec::new();
        };
        let vector = match self.embedder.embed(&[text.to_owned()]).await {
            Ok(mut v) => match v.pop() {
                Some(e) => e.vector,
                None => return Vec::new(),
            },
            Err(e) => {
                tracing::warn!(error = %e, "semantic duplicate check skipped");
                return Vec::new();
            }
        };
        let kind_names: Vec<String> = kinds.iter().map(|k| k.as_str().to_owned()).collect();
        // A failing statement must not abort the create's transaction.
        if sqlx::query("SAVEPOINT strata_semantic_dup")
            .execute(tx.conn())
            .await
            .is_err()
        {
            return Vec::new();
        }
        let found = vectors::nearest_items(
            tx,
            &vector,
            self.embedder.model_id(),
            &kind_names,
            f64::from(floor),
            LIMIT,
        )
        .await;
        let end = if found.is_ok() {
            "RELEASE SAVEPOINT strata_semantic_dup"
        } else {
            "ROLLBACK TO SAVEPOINT strata_semantic_dup"
        };
        if sqlx::query(end).execute(tx.conn()).await.is_err() {
            return Vec::new();
        }
        match found {
            Ok(found) => found
                .into_iter()
                .filter(|n| n.item.item.id.is_some() && n.item.item.id != item.id)
                .map(|n| SemanticMatch {
                    item: n.item.item,
                    cosine: n.similarity,
                })
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "semantic duplicate lookup failed");
                Vec::new()
            }
        }
    }
}

impl SemanticDuplicates for SemanticDupSource {
    fn evidence<'a>(
        &'a self,
        tx: &'a mut ScopedTx,
        item: &'a dedupe::Item,
    ) -> BoxFuture<'a, Vec<SemanticMatch>> {
        Box::pin(self.find(tx, item))
    }
}
