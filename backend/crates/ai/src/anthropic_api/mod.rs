//! `AnthropicApiProvider`: the Messages API (`POST /v1/messages`) with a server API key
//! (D20: built and selectable per user by config, D23).
//!
//! - JSON over HTTPS between the backend and Anthropic (not the client wire, L21).
//! - Structured output via `output_config.format = {type: "json_schema", schema}` with the
//!   schema reduced to what the API supports ([`crate::schema::api_compatible`]); the reply is
//!   validated against the full schema by [`crate::AiService`].
//! - Streaming via SSE (`stream: true`), parsed by [`sse::SseParser`].
//! - Retries with exponential backoff (honouring `retry-after`) on 408/409/429/5xx/529 and
//!   connection errors; a `429` that outlasts the retries becomes a pause.
//! - The key is read once from a `0600` file and never logged ([`ApiKey`]'s `Debug` redacts).

pub mod sse;

use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt};
use serde_json::{Value, json};
use strata_common::Clock;

use crate::error::{AiError, PauseReason, ProviderError};
use crate::provider::{HealthTracker, LlmProvider, ProviderHealth, error_kind};
use crate::request::{ChatRequest, JsonCompletion, JsonRequest, StreamEvent, TokenStream, Usage};
use crate::retry::{Backoff, Sleeper};
use sse::{SseEvent, SseParser};

/// The API version header value.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// An Anthropic API key. `Debug` never prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl ApiKey {
    /// Wraps a key held in memory (tests, or a caller that already read the secret).
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Reads the key from `path`: a regular file with no group/other permissions (0600 or
    /// stricter, PLAN §8/§15), containing the key (surrounding whitespace ignored).
    pub fn from_file(path: &Path) -> Result<Self, AiError> {
        let shown = path.display();
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| AiError::Config(format!("api key file {shown}: {:?}", e.kind())))?;
        if !meta.file_type().is_file() {
            return Err(AiError::Config(format!("api key file {shown} is not a regular file")));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                return Err(AiError::Config(format!(
                    "api key file {shown} has mode {mode:o}; it must not be readable by group or others (0600)"
                )));
            }
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| AiError::Config(format!("api key file {shown}: {:?}", e.kind())))?;
        let key = text.trim();
        if key.is_empty() {
            return Err(AiError::Config(format!("api key file {shown} is empty")));
        }
        Ok(Self(key.to_owned()))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

/// Per-model prices in micro-USD per million tokens (for cost estimates and cost caps).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pricing {
    /// Uncached input.
    pub input_micros_per_mtok: u64,
    /// Output.
    pub output_micros_per_mtok: u64,
}

impl Pricing {
    /// Claude Opus 5 list price ($5 / $25 per million tokens).
    pub const CLAUDE_OPUS_5: Self = Self {
        input_micros_per_mtok: 5_000_000,
        output_micros_per_mtok: 25_000_000,
    };

    /// Estimated cost of `u` in micro-USD: cache writes at 1.25× and cache reads at 0.1× the
    /// input price.
    pub fn cost_micros(&self, u: &Usage) -> u64 {
        let input = u128::from(u.input_tokens) * 100
            + u128::from(u.cache_creation_input_tokens) * 125
            + u128::from(u.cache_read_input_tokens) * 10;
        let total = input * u128::from(self.input_micros_per_mtok)
            + u128::from(u.output_tokens) * 100 * u128::from(self.output_micros_per_mtok);
        u64::try_from(total.div_ceil(100 * 1_000_000)).unwrap_or(u64::MAX)
    }
}

/// Configuration of [`AnthropicApiProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicApiConfig {
    /// API base URL (no trailing slash).
    pub base_url: String,
    /// Model ID.
    pub model: String,
    /// `output_config.effort` (`low` … `max`); `None` = the model default.
    pub effort: Option<String>,
    /// Per-request timeout (the whole response, including a stream).
    pub timeout: Duration,
    /// Retries after the first attempt.
    pub max_retries: u32,
    /// Retry delays.
    pub backoff: Backoff,
    /// Prices of `model`.
    pub pricing: Pricing,
}

impl Default for AnthropicApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.anthropic.com".to_owned(),
            model: "claude-opus-5".to_owned(),
            effort: None,
            timeout: Duration::from_secs(600),
            max_retries: 3,
            backoff: Backoff::default(),
            pricing: Pricing::CLAUDE_OPUS_5,
        }
    }
}

/// The Messages API provider.
#[derive(Debug)]
pub struct AnthropicApiProvider {
    cfg: AnthropicApiConfig,
    key: ApiKey,
    http: reqwest::Client,
    sleeper: Arc<dyn Sleeper>,
    clock: Arc<dyn Clock>,
    health: HealthTracker,
}

/// The outcome of a failed attempt.
struct AttemptError {
    error: ProviderError,
    retryable: bool,
    retry_after: Option<Duration>,
    rate_limited: bool,
}

impl AnthropicApiProvider {
    /// Builds the provider.
    pub fn new(
        cfg: AnthropicApiConfig,
        key: ApiKey,
        clock: Arc<dyn Clock>,
        sleeper: Arc<dyn Sleeper>,
    ) -> Result<Self, AiError> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AiError::Config(format!("http client: {e}")))?;
        Ok(Self {
            cfg,
            key,
            http,
            sleeper,
            clock,
            health: HealthTracker::default(),
        })
    }

    /// The request body for `system`/`user` (+ structured output when `schema` is given).
    pub fn body(&self, system: &str, user: &str, max_tokens: u32, schema: Option<&Value>, stream: bool) -> Value {
        let mut body = json!({
            "model": self.cfg.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": user}],
        });
        let mut output_config = serde_json::Map::new();
        if let Some(s) = schema {
            output_config.insert(
                "format".into(),
                json!({"type": "json_schema", "schema": crate::schema::api_compatible(s)}),
            );
        }
        if let Some(e) = &self.cfg.effort {
            output_config.insert("effort".into(), Value::String(e.clone()));
        }
        if !output_config.is_empty() {
            body["output_config"] = Value::Object(output_config);
        }
        if stream {
            body["stream"] = Value::Bool(true);
        }
        body
    }

    async fn attempt(&self, body: &[u8]) -> Result<reqwest::Response, AttemptError> {
        let resp = self
            .http
            .post(format!("{}/v1/messages", self.cfg.base_url))
            .header("x-api-key", self.key.expose())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .timeout(self.cfg.timeout)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| AttemptError {
                error: if e.is_timeout() {
                    ProviderError::Timeout(self.cfg.timeout)
                } else {
                    ProviderError::Unavailable("connection error".into())
                },
                retryable: true,
                retry_after: None,
                rate_limited: false,
            })?;
        let status = resp.status().as_u16();
        if resp.status().is_success() {
            return Ok(resp);
        }
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let error_type = resp
            .bytes()
            .await
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.pointer("/error/type").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| format!("http_{status}"));
        let (error, retryable) = match status {
            401 | 403 => (ProviderError::Auth, false),
            408 | 409 | 429 | 500..=599 => (
                ProviderError::Unavailable(format!("{error_type} (HTTP {status})")),
                true,
            ),
            _ => (ProviderError::Rejected(format!("{error_type} (HTTP {status})")), false),
        };
        Err(AttemptError {
            error,
            retryable,
            retry_after,
            rate_limited: status == 429,
        })
    }

    /// Sends `body`, retrying transient failures.
    async fn send(&self, body: &Value) -> Result<reqwest::Response, ProviderError> {
        let bytes = serde_json::to_vec(body).map_err(|e| ProviderError::Protocol(e.to_string()))?;
        let mut attempt = 0;
        loop {
            match self.attempt(&bytes).await {
                Ok(r) => return Ok(r),
                Err(e) if e.retryable && attempt < self.cfg.max_retries => {
                    let delay = self.cfg.backoff.delay(attempt, e.retry_after);
                    tracing::info!(
                        provider = "anthropic_api",
                        error = error_kind(&e.error),
                        attempt,
                        delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                        "retrying"
                    );
                    self.sleeper.sleep(delay).await;
                    attempt += 1;
                }
                Err(e) if e.rate_limited => {
                    let wait = e.retry_after.unwrap_or(self.cfg.backoff.max);
                    let until = self.clock.now()
                        + chrono::Duration::from_std(wait).unwrap_or(chrono::Duration::minutes(1));
                    return Err(ProviderError::Paused {
                        reason: PauseReason::ProviderRateLimit,
                        until: Some(until),
                    });
                }
                Err(e) => return Err(e.error),
            }
        }
    }

    fn usage_from(&self, u: &Value) -> Usage {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        let mut usage = Usage {
            input_tokens: n("input_tokens"),
            cache_creation_input_tokens: n("cache_creation_input_tokens"),
            cache_read_input_tokens: n("cache_read_input_tokens"),
            output_tokens: n("output_tokens"),
            cost_micros: None,
        };
        usage.cost_micros = Some(self.cfg.pricing.cost_micros(&usage));
        usage
    }

    async fn complete_inner(&self, req: &JsonRequest) -> Result<JsonCompletion, ProviderError> {
        let body = self.body(&req.system, &req.user, req.max_tokens, Some(&req.schema), false);
        let resp = self.send(&body).await?;
        let bytes = resp.bytes().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(self.cfg.timeout)
            } else {
                ProviderError::Unavailable("connection error".into())
            }
        })?;
        let msg: Value = serde_json::from_slice(&bytes)
            .map_err(|e| ProviderError::Protocol(format!("response is not JSON: {e}")))?;
        check_stop_reason(msg.get("stop_reason").and_then(Value::as_str))?;
        let text: String = msg
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Protocol("response has no content".into()))?
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect();
        let value: Value = serde_json::from_str(text.trim())
            .map_err(|e| ProviderError::NotJson(e.to_string()))?;
        Ok(JsonCompletion {
            value,
            usage: self.usage_from(msg.get("usage").unwrap_or(&Value::Null)),
            model: msg
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.cfg.model)
                .to_owned(),
        })
    }

    fn record(&self, outcome: Result<(), &ProviderError>) {
        match outcome {
            Ok(()) => self.health.success(),
            Err(e) => {
                tracing::warn!(provider = "anthropic_api", error = error_kind(e), "call failed");
                self.health.failure(e, self.clock.now());
            }
        }
    }
}

fn check_stop_reason(stop: Option<&str>) -> Result<(), ProviderError> {
    match stop {
        Some("max_tokens") => Err(ProviderError::Truncated),
        Some("refusal") => Err(ProviderError::Refused),
        _ => Ok(()),
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicApiProvider {
    fn name(&self) -> &'static str {
        "anthropic_api"
    }

    fn model(&self) -> &str {
        &self.cfg.model
    }

    fn health(&self) -> ProviderHealth {
        self.health.snapshot(self.clock.now())
    }

    async fn complete_json(&self, req: JsonRequest) -> Result<JsonCompletion, ProviderError> {
        let out = self.complete_inner(&req).await;
        self.record(out.as_ref().map(|_| ()));
        out
    }

    async fn stream(&self, req: ChatRequest) -> Result<TokenStream, ProviderError> {
        let body = self.body(&req.system, &req.user, req.max_tokens, None, true);
        let resp = match self.send(&body).await {
            Ok(r) => r,
            Err(e) => {
                self.record(Err(&e));
                return Err(e);
            }
        };
        self.record(Ok(()));
        let state = SseState {
            bytes: resp.bytes_stream().boxed(),
            parser: SseParser::new(),
            pending: VecDeque::new(),
            usage: Usage::default(),
            model: self.cfg.model.clone(),
            pricing: self.cfg.pricing,
            timeout: self.cfg.timeout,
            done: false,
        };
        Ok(Box::pin(stream::unfold(state, |mut st| async move {
            if st.done {
                return None;
            }
            let item = st.next_item().await;
            if !matches!(item, Ok(StreamEvent::Text(_))) {
                st.done = true;
            }
            Some((item, st))
        })))
    }
}

struct SseState {
    bytes: BoxStream<'static, reqwest::Result<Bytes>>,
    parser: SseParser,
    pending: VecDeque<SseEvent>,
    usage: Usage,
    model: String,
    pricing: Pricing,
    timeout: Duration,
    done: bool,
}

impl SseState {
    async fn next_item(&mut self) -> Result<StreamEvent, ProviderError> {
        loop {
            while let Some(ev) = self.pending.pop_front() {
                if let Some(item) = self.handle(&ev)? {
                    return Ok(item);
                }
            }
            match self.bytes.next().await {
                Some(Ok(chunk)) => self.pending.extend(self.parser.push(&chunk)),
                Some(Err(e)) if e.is_timeout() => return Err(ProviderError::Timeout(self.timeout)),
                Some(Err(_)) => return Err(ProviderError::Unavailable("stream interrupted".into())),
                None => return Err(ProviderError::Protocol("stream ended before message_stop".into())),
            }
        }
    }

    fn handle(&mut self, ev: &SseEvent) -> Result<Option<StreamEvent>, ProviderError> {
        if ev.event == "ping" {
            return Ok(None);
        }
        let v: Value = serde_json::from_str(&ev.data)
            .map_err(|e| ProviderError::Protocol(format!("SSE data is not JSON: {e}")))?;
        match v.get("type").and_then(Value::as_str).unwrap_or(&ev.event) {
            "message_start" => {
                let msg = v.get("message").unwrap_or(&Value::Null);
                if let Some(m) = msg.get("model").and_then(Value::as_str) {
                    m.clone_into(&mut self.model);
                }
                let u = msg.get("usage").unwrap_or(&Value::Null);
                let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
                self.usage.input_tokens = n("input_tokens");
                self.usage.cache_creation_input_tokens = n("cache_creation_input_tokens");
                self.usage.cache_read_input_tokens = n("cache_read_input_tokens");
                self.usage.output_tokens = n("output_tokens");
                Ok(None)
            }
            "content_block_delta" => Ok(v
                .pointer("/delta/text")
                .filter(|_| v.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta"))
                .and_then(Value::as_str)
                .map(|t| StreamEvent::Text(t.to_owned()))),
            "message_delta" => {
                if let Some(out) = v.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    self.usage.output_tokens = out;
                }
                check_stop_reason(v.pointer("/delta/stop_reason").and_then(Value::as_str))?;
                Ok(None)
            }
            "message_stop" => {
                let mut usage = self.usage;
                usage.cost_micros = Some(self.pricing.cost_micros(&usage));
                Ok(Some(StreamEvent::Done {
                    usage,
                    model: self.model.clone(),
                }))
            }
            "error" => {
                let kind = v
                    .pointer("/error/type")
                    .and_then(Value::as_str)
                    .unwrap_or("error");
                Err(ProviderError::Unavailable(format!("{kind} (in stream)")))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn cost_estimate_uses_cache_multipliers_and_rounds_up() {
        let p = Pricing::CLAUDE_OPUS_5;
        let u = Usage {
            input_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cost_micros: None,
        };
        // 5 + 6.25 + 0.5 + 25 USD
        assert_eq!(p.cost_micros(&u), 36_750_000);
        let one = Usage {
            input_tokens: 1,
            ..Usage::default()
        };
        assert_eq!(p.cost_micros(&one), 5);
        assert_eq!(p.cost_micros(&Usage::default()), 0);
    }

    #[test]
    fn api_key_debug_is_redacted() {
        assert_eq!(format!("{:?}", ApiKey::new("sk-ant-secret")), "ApiKey(<redacted>)");
    }
}
