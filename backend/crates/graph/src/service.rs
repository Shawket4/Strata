//! [`GraphService`]: the graph endpoints' entry points. Each request reads the caller's
//! graph in one scoped transaction (principle 7), adds similarity edges when asked and an
//! embedding model is configured, adds hover summaries from the sidecars, and assembles the
//! response with [`crate::assemble`].

use std::collections::BTreeSet;
use std::sync::Arc;

use strata_common::NoteId;
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;

use crate::assemble::{self, GraphView, SimilarPair, SimilarityStatus};
use crate::error::{GraphError, Result};
use crate::load::{self, GraphData};
use crate::query::{GraphQuery, LocalQuery};
use crate::similarity::SimilaritySource;

/// Graph reads for the API.
#[derive(Debug, Clone)]
pub struct GraphService {
    db: AppDb,
    vault: VaultService,
    similarity: Option<Arc<dyn SimilaritySource>>,
}

impl GraphService {
    /// The service; `similarity` is `None` without an embedding model.
    pub fn new(
        db: AppDb,
        vault: VaultService,
        similarity: Option<Arc<dyn SimilaritySource>>,
    ) -> Self {
        Self {
            db,
            vault,
            similarity,
        }
    }

    /// The `strata_app` database.
    pub fn db(&self) -> &AppDb {
        &self.db
    }

    /// The vault store.
    pub fn vault(&self) -> &VaultService {
        &self.vault
    }

    /// The scope's graph as stored in the index.
    pub async fn data(&self, scope: &UserScope) -> Result<GraphData> {
        let mut tx = self.db.begin(scope).await?;
        let data = load::load(&mut tx).await?;
        tx.commit().await?;
        Ok(data)
    }

    async fn summaries(
        &self,
        scope: &UserScope,
        ids: BTreeSet<NoteId>,
    ) -> std::collections::HashMap<NoteId, String> {
        crate::summaries::load(self.vault.vault_dir(scope.user_id()), ids).await
    }

    /// `GET /graph`.
    pub async fn graph(&self, scope: &UserScope, query: &GraphQuery) -> Result<GraphView> {
        let data = self.data(scope).await?;
        if let Some(lens) = query.lens {
            let ids = data
                .nodes
                .iter()
                .filter(|n| n.kind == lens.kind())
                .map(|n| n.id)
                .collect();
            let summaries = self.summaries(scope, ids).await;
            return Ok(assemble::lens(&data, lens, &query.edges, &summaries));
        }
        let (pairs, status) = match (&self.similarity, query.include_similarity) {
            (_, false) => (Vec::new(), SimilarityStatus::Off),
            (None, true) => (Vec::new(), SimilarityStatus::Unavailable),
            (Some(src), true) => {
                let p = src.pairs(scope).await?;
                let status = if p.truncated {
                    SimilarityStatus::Truncated
                } else {
                    SimilarityStatus::Complete
                };
                (p.pairs, status)
            }
        };
        let ids = data
            .nodes
            .iter()
            .filter(|n| query.nodes.allows(n.kind))
            .map(|n| n.id)
            .collect();
        let summaries = self.summaries(scope, ids).await;
        Ok(assemble::global(&data, query, &pairs, status, &summaries))
    }

    /// `GET /graph/local/{id}`; `404` unless `focus` is a live note of the scope.
    pub async fn local(
        &self,
        scope: &UserScope,
        focus: NoteId,
        query: &LocalQuery,
    ) -> Result<GraphView> {
        let data = self.data(scope).await?;
        if !data.nodes.iter().any(|n| n.id == focus) {
            return Err(GraphError::NotFound);
        }
        let (similar, status) = match (&self.similarity, query.include_similarity) {
            (_, false) => (Vec::new(), SimilarityStatus::Off),
            (None, true) => (Vec::new(), SimilarityStatus::Unavailable),
            (Some(src), true) => (
                src.similar_to(scope, focus)
                    .await?
                    .into_iter()
                    .map(|(target, score)| SimilarPair {
                        source: focus,
                        target,
                        score,
                    })
                    .collect(),
                SimilarityStatus::Complete,
            ),
        };
        let view = assemble::local(
            &data,
            focus,
            query,
            &similar,
            status,
            &std::collections::HashMap::new(),
        )?;
        let ids = view.nodes.iter().map(|n| n.id).collect();
        let summaries = self.summaries(scope, ids).await;
        let mut view = view;
        for n in &mut view.nodes {
            n.summary = summaries.get(&n.id).cloned();
        }
        Ok(view)
    }
}
