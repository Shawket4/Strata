//! The semantic level of the duplicate check on create (PLAN §9.7), supplied by the AI
//! subsystem through [`SemanticDuplicates`] (registered with
//! [`crate::VaultService::set_semantic`]). The vault store has no embeddings of its own; with
//! nothing registered, or when the source cannot answer cheaply, only the exact and near
//! levels run (principle 6).

use futures_util::future::BoxFuture;
use strata_index::ScopedTx;

/// A stored item whose vector is close to the new item's text.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticMatch {
    /// The stored item (as in `dedupe_keys`).
    pub item: dedupe::Item,
    /// Cosine similarity to the new text.
    pub cosine: f32,
}

/// Finds semantically similar stored items for the synchronous duplicate check.
pub trait SemanticDuplicates: Send + Sync {
    /// Items of kinds compatible with `item` whose vectors are close to `item`'s text, read in
    /// the caller's scoped transaction. Must stay fast (embed only when the model is loaded
    /// and the text is short) and never fail the create: on any problem, return nothing.
    fn evidence<'a>(
        &'a self,
        tx: &'a mut ScopedTx,
        item: &'a dedupe::Item,
    ) -> BoxFuture<'a, Vec<SemanticMatch>>;
}
