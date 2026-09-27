//! `FakeLlmProvider` (feature `test-support`, PLAN §9.1/§16.3): replays recorded responses
//! keyed by (prompt id, prompt version, input hash). An input without a fixture fails loudly
//! with its hash and the path where the fixture belongs, so it can be recorded
//! ([`FixtureRecorder`] records real responses in the same layout).
//!
//! Fixture file: `<dir>/<prompt id>/v<version>/<input hash>.json`, one of
//! - `{"json": <value>, "usage": {…}?, "model": "…"?}`
//! - `{"stream": ["tok", "tok", …], "usage": {…}?, "model": "…"?}`
//! - `{"error": "paused" | "timeout" | "unavailable" | "not_json", "until": "<RFC 3339>"?}`
//!
//! A key may hold a sequence of responses (the builder's `push_*`); each call takes the next
//! one and the last one repeats.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{PauseReason, ProviderError};
use crate::provider::{LlmProvider, ProviderHealth};
use crate::request::{ChatRequest, JsonCompletion, JsonRequest, PromptRef, StreamEvent, TokenStream, Usage};

/// One recorded response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    /// A structured reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    /// A streamed reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<Vec<String>>,
    /// A failure: `paused`, `timeout`, `unavailable`, `not_json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// For `paused`: when it ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,
    /// Usage reported (default: 100 input, 50 output tokens).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// Model reported (default: `fake-model`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Fixture {
    /// A structured reply.
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            json: Some(value),
            stream: None,
            error: None,
            until: None,
            usage: None,
            model: None,
        }
    }

    /// A streamed reply.
    pub fn stream(tokens: &[&str]) -> Self {
        Self {
            stream: Some(tokens.iter().map(|t| (*t).to_owned()).collect()),
            ..Self::json(serde_json::Value::Null)
        }
        .without_json()
    }

    /// A failure (`paused`, `timeout`, `unavailable`, `not_json`).
    pub fn error(kind: &str, until: Option<DateTime<Utc>>) -> Self {
        Self {
            error: Some(kind.to_owned()),
            until,
            ..Self::json(serde_json::Value::Null)
        }
        .without_json()
    }

    /// With explicit usage.
    #[must_use]
    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = Some(usage);
        self
    }

    fn without_json(mut self) -> Self {
        self.json = None;
        self
    }

    fn usage(&self) -> Usage {
        self.usage.unwrap_or(Usage {
            input_tokens: 100,
            output_tokens: 50,
            ..Usage::default()
        })
    }

    fn model(&self) -> String {
        self.model.clone().unwrap_or_else(|| "fake-model".to_owned())
    }

    fn failure(&self) -> Option<ProviderError> {
        self.error.as_deref().map(|kind| match kind {
            "paused" => ProviderError::Paused {
                reason: PauseReason::ProviderUsageLimit,
                until: self.until,
            },
            "timeout" => ProviderError::Timeout(Duration::from_secs(300)),
            "not_json" => ProviderError::NotJson("fixture".into()),
            other => ProviderError::Unavailable(format!("fixture error {other}")),
        })
    }
}

/// Fixture key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixtureKey {
    /// Prompt.
    pub prompt: PromptRef,
    /// SHA-256 (hex) of the input.
    pub input_hash: String,
}

/// One call the fake received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    /// Prompt.
    pub prompt: PromptRef,
    /// Input hash.
    pub input_hash: String,
    /// The user content sent.
    pub user: String,
}

/// Fixture-replaying provider.
#[derive(Debug, Clone)]
pub struct FakeLlmProvider {
    name: &'static str,
    dir: Option<PathBuf>,
    fixtures: Arc<Mutex<BTreeMap<FixtureKey, Vec<Fixture>>>>,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl Default for FakeLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeLlmProvider {
    /// No fixtures; add them with [`push`](Self::push).
    pub fn new() -> Self {
        Self {
            name: "fake",
            dir: None,
            fixtures: Arc::default(),
            calls: Arc::default(),
        }
    }

    /// Also reads fixture files from `dir` (in-memory fixtures win).
    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: Some(dir.into()),
            ..Self::new()
        }
    }

    /// Reports `name` as its provider name (e.g. to stand in for `claude_cli` in routing).
    #[must_use]
    pub fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Appends a response for (prompt, input hash).
    pub fn push(&self, prompt: &PromptRef, input_hash: &str, fixture: Fixture) {
        self.fixtures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(FixtureKey {
                prompt: prompt.clone(),
                input_hash: input_hash.to_owned(),
            })
            .or_default()
            .push(fixture);
    }

    /// Every call so far, in order.
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Where the fixture for (prompt, hash) lives under `dir`.
    pub fn fixture_path(dir: &Path, prompt: &PromptRef, input_hash: &str) -> PathBuf {
        dir.join(&prompt.id)
            .join(format!("v{}", prompt.version))
            .join(format!("{input_hash}.json"))
    }

    fn lookup(&self, prompt: &PromptRef, input_hash: &str, user: &str) -> Result<Fixture, ProviderError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(RecordedCall {
                prompt: prompt.clone(),
                input_hash: input_hash.to_owned(),
                user: user.to_owned(),
            });
        let key = FixtureKey {
            prompt: prompt.clone(),
            input_hash: input_hash.to_owned(),
        };
        {
            let mut all = self.fixtures.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(queue) = all.get_mut(&key) {
                if queue.len() > 1 {
                    return Ok(queue.remove(0));
                }
                if let Some(last) = queue.first() {
                    return Ok(last.clone());
                }
            }
        }
        let path = self.dir.as_deref().map_or_else(
            || PathBuf::from(format!("<fixtures>/{}/v{}/{input_hash}.json", prompt.id, prompt.version)),
            |d| Self::fixture_path(d, prompt, input_hash),
        );
        if self.dir.is_some()
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            return serde_json::from_str(&text)
                .map_err(|e| ProviderError::Protocol(format!("fixture {}: {e}", path.display())));
        }
        Err(ProviderError::MissingFixture {
            prompt_id: prompt.id.clone(),
            version: prompt.version,
            input_hash: input_hash.to_owned(),
            path: path.display().to_string(),
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for FakeLlmProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn model(&self) -> &'static str {
        "fake-model"
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::ready()
    }

    async fn complete_json(&self, req: JsonRequest) -> Result<JsonCompletion, ProviderError> {
        let f = self.lookup(&req.prompt, &req.input_hash(), &req.user)?;
        if let Some(e) = f.failure() {
            return Err(e);
        }
        let value = f
            .json
            .clone()
            .ok_or_else(|| ProviderError::Protocol("fixture has no json".into()))?;
        Ok(JsonCompletion {
            value,
            usage: f.usage(),
            model: f.model(),
        })
    }

    async fn stream(&self, req: ChatRequest) -> Result<TokenStream, ProviderError> {
        let f = self.lookup(&req.prompt, &req.input_hash(), &req.user)?;
        if let Some(e) = f.failure() {
            return Err(e);
        }
        let tokens = f
            .stream
            .clone()
            .ok_or_else(|| ProviderError::Protocol("fixture has no stream".into()))?;
        let mut items: Vec<Result<StreamEvent, ProviderError>> =
            tokens.into_iter().map(|t| Ok(StreamEvent::Text(t))).collect();
        items.push(Ok(StreamEvent::Done {
            usage: f.usage(),
            model: f.model(),
        }));
        Ok(Box::pin(futures::stream::iter(items)))
    }
}

/// Wraps a real provider and writes each successful response as a fixture file (for recording
/// fixtures by hand against a real provider; never used in CI).
#[derive(Debug)]
pub struct FixtureRecorder<P> {
    inner: P,
    dir: PathBuf,
}

impl<P: LlmProvider> FixtureRecorder<P> {
    /// Records into `dir`.
    pub fn new(inner: P, dir: impl Into<PathBuf>) -> Self {
        Self {
            inner,
            dir: dir.into(),
        }
    }

    fn write(&self, prompt: &PromptRef, hash: &str, f: &Fixture) -> Result<(), ProviderError> {
        let path = FakeLlmProvider::fixture_path(&self.dir, prompt, hash);
        let io = |e: std::io::Error| ProviderError::Protocol(format!("writing fixture: {e}"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let text = serde_json::to_string_pretty(f).map_err(|e| ProviderError::Protocol(e.to_string()))?;
        std::fs::write(&path, text + "\n").map_err(io)
    }
}

#[async_trait::async_trait]
impl<P: LlmProvider> LlmProvider for FixtureRecorder<P> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn health(&self) -> ProviderHealth {
        self.inner.health()
    }

    async fn complete_json(&self, req: JsonRequest) -> Result<JsonCompletion, ProviderError> {
        let (prompt, hash) = (req.prompt.clone(), req.input_hash());
        let out = self.inner.complete_json(req).await?;
        let mut f = Fixture::json(out.value.clone()).with_usage(out.usage);
        f.model = Some(out.model.clone());
        self.write(&prompt, &hash, &f)?;
        Ok(out)
    }

    async fn stream(&self, req: ChatRequest) -> Result<TokenStream, ProviderError> {
        use futures::StreamExt;
        let (prompt, hash) = (req.prompt.clone(), req.input_hash());
        let items: Vec<_> = self.inner.stream(req).await?.collect().await;
        let mut tokens = Vec::new();
        for item in &items {
            match item {
                Ok(StreamEvent::Text(t)) => tokens.push(t.clone()),
                Ok(StreamEvent::Done { usage, model }) => {
                    let mut f = Fixture::stream(&[]).with_usage(*usage);
                    f.stream = Some(tokens.clone());
                    f.model = Some(model.clone());
                    self.write(&prompt, &hash, &f)?;
                }
                Err(_) => {}
            }
        }
        Ok(Box::pin(futures::stream::iter(items)))
    }
}
