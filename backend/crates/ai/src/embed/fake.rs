//! `FakeEmbedder` (feature `test-support`): deterministic vectors for tests (PLAN §16.1: no
//! model, no network).
//!
//! A text registered with [`FakeEmbedder::set`] gets exactly that vector (normalised). Any
//! other text gets a hashed bag of words: every word (letters and digits of any script,
//! lowercased) adds 1 to dimension `sha256(word) mod dims`, and the sum is L2-normalised; a
//! text without words is the first unit vector. Texts sharing words are therefore similar,
//! which is enough to test ranking; exact numerical assertions register their vectors.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use sha2::{Digest, Sha256};

use super::pooling::l2_normalize;
use super::{EmbedError, Embedder, Embedding};

#[derive(Debug, Default)]
struct State {
    fixed: BTreeMap<String, Vec<f32>>,
    calls: Vec<Vec<String>>,
    failure: Option<EmbedError>,
    loaded: bool,
}

/// A deterministic in-memory embedder.
#[derive(Debug, Clone)]
pub struct FakeEmbedder {
    model_id: String,
    dims: usize,
    state: Arc<Mutex<State>>,
}

impl FakeEmbedder {
    /// An embedder reporting `model_id` with `dims` dimensions (loaded).
    pub fn new(model_id: &str, dims: usize) -> Self {
        Self {
            model_id: model_id.to_owned(),
            dims,
            state: Arc::new(Mutex::new(State {
                loaded: true,
                ..State::default()
            })),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Embeds `text` as `vector` (normalised; padded or cut to `dims`).
    pub fn set(&self, text: &str, vector: &[f32]) {
        let mut v = vector.to_vec();
        v.resize(self.dims, 0.0);
        l2_normalize(&mut v);
        self.lock().fixed.insert(text.to_owned(), v);
    }

    /// Every later call fails with `error` (`None` restores normal operation).
    pub fn fail_with(&self, error: Option<EmbedError>) {
        self.lock().failure = error;
    }

    /// What [`Embedder::is_loaded`] reports (default `true`).
    pub fn set_loaded(&self, loaded: bool) {
        self.lock().loaded = loaded;
    }

    /// The texts of every call so far, per call.
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.lock().calls.clone()
    }

    /// The vector this embedder returns for `text`.
    pub fn vector(&self, text: &str) -> Vec<f32> {
        if let Some(v) = self.lock().fixed.get(text) {
            return v.clone();
        }
        bag_of_words(text, self.dims)
    }
}

/// The hashed bag-of-words vector of `text` (see the module docs).
pub fn bag_of_words(text: &str, dims: usize) -> Vec<f32> {
    let mut v = vec![0f32; dims.max(1)];
    let mut any = false;
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let lower = word.to_lowercase();
        let digest = Sha256::digest(lower.as_bytes());
        let mut n = [0u8; 8];
        n.copy_from_slice(&digest[..8]);
        let dim = usize::try_from(u64::from_le_bytes(n) % (dims.max(1) as u64)).unwrap_or(0);
        v[dim] += 1.0;
        any = true;
    }
    if !any {
        v[0] = 1.0;
    }
    l2_normalize(&mut v);
    v
}

#[async_trait::async_trait]
impl Embedder for FakeEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, EmbedError> {
        let failure = {
            let mut s = self.lock();
            s.calls.push(texts.to_vec());
            s.failure.clone()
        };
        if let Some(e) = failure {
            return Err(e);
        }
        Ok(texts
            .iter()
            .map(|t| Embedding {
                model_id: self.model_id.clone(),
                vector: self.vector(t),
            })
            .collect())
    }

    fn is_loaded(&self) -> bool {
        self.lock().loaded
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::embed::pooling::dot;

    #[tokio::test]
    async fn vectors_are_deterministic_normalised_and_overridable() {
        let e = FakeEmbedder::new("fake@1", 8);
        let a = e.vector("Watanya contract safe");
        assert_eq!(a, e.vector("watanya CONTRACT, safe!"), "case and punctuation");
        assert!((dot(&a, &a) - 1.0).abs() < 1e-6);
        assert_eq!(e.vector("..."), vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        e.set("x", &[3.0, 4.0]);
        assert_eq!(e.vector("x"), vec![0.6, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let out = e.embed(&["x".into()]).await.expect("embed");
        assert_eq!(
            out,
            vec![Embedding {
                model_id: "fake@1".into(),
                vector: vec![0.6, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
            }]
        );
        assert_eq!(e.calls(), vec![vec!["x".to_owned()]]);
        e.fail_with(Some(EmbedError::Stopped));
        assert_eq!(e.embed(&["x".into()]).await, Err(EmbedError::Stopped));
        e.set_loaded(false);
        assert!(!e.is_loaded());
    }
}
