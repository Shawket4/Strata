//! Local embeddings (L19, D9 = a, §9.1b).
//!
//! The model is IBM granite-embedding-97m-multilingual-r2 (ModernBERT, 384 dimensions). Per
//! its model card and `1_Pooling/config.json` it uses **CLS pooling** (the hidden state of the
//! first token) followed by L2 normalisation; mean pooling is supported for other models.
//! Every [`Embedding`] carries the model ID that produced it, to be stored with the vector.

pub mod pooling;

#[cfg(feature = "onnx")]
pub mod onnx;

use std::fmt;

/// One embedding vector and the model that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    /// Model ID (stored with every vector; a change triggers a full re-embed).
    pub model_id: String,
    /// Unit-length vector.
    pub vector: Vec<f32>,
}

/// An embedding failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbedError {
    /// The model, tokenizer or runtime could not be loaded.
    #[error("embedding model unavailable: {0}")]
    Load(String),
    /// Tokenisation failed.
    #[error("tokenisation failed: {0}")]
    Tokenize(String),
    /// Inference failed.
    #[error("inference failed: {0}")]
    Inference(String),
    /// The model output has an unexpected shape.
    #[error("unexpected model output: {0}")]
    Output(String),
    /// The embedding worker is gone.
    #[error("embedding worker stopped")]
    Stopped,
}

/// Produces embeddings (PLAN §9.1).
#[async_trait::async_trait]
pub trait Embedder: Send + Sync + fmt::Debug {
    /// Model ID stored with every vector.
    fn model_id(&self) -> &str;

    /// Vector dimensions.
    fn dims(&self) -> usize;

    /// Embeds `texts` (one vector per text, same order).
    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, EmbedError>;
}

/// How token states become one vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    /// The first token's hidden state (granite-embedding r2).
    Cls,
    /// The mean over attended tokens.
    Mean,
}
