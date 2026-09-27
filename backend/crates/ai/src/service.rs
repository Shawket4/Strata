//! `AiService`: the entry point jobs and handlers use. It routes each call to the user's
//! provider, enforces the budget, records usage, and validates structured output against the
//! request's schema — retrying at most [`MAX_INVALID_OUTPUT_RETRIES`] times with the validation
//! errors fed back, then failing with [`AiError::InvalidOutput`] (§9.1).

use std::pin::Pin;
use std::sync::Arc;

use futures::{Stream, StreamExt};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::budget::BudgetGuard;
use crate::embed::Embedder;
use crate::error::{AiError, ProviderError};
use crate::prompts::PromptDef;
use crate::request::{AiCaller, ChatRequest, JsonRequest, PromptRef, StreamEvent, Usage};
use crate::router::ProviderRouter;
use crate::schema::{self, Violation};
use crate::status::{AiStatus, EmbeddingsStatus, PauseInfo, ProviderStatus, UsageStatus};

/// Retries after an invalid structured output (so at most three attempts in total).
pub const MAX_INVALID_OUTPUT_RETRIES: u32 = 2;

/// A schema-valid structured output with its provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedJson {
    /// The output.
    pub value: serde_json::Value,
    /// Provider name.
    pub provider: &'static str,
    /// Model that produced it (`provider/model` goes into sidecar provenance).
    pub model: String,
    /// Prompt it answers.
    pub prompt: PromptRef,
    /// Attempts used (1 = valid at once).
    pub attempts: u32,
    /// Usage summed over all attempts.
    pub usage: Usage,
}

/// A typed structured output.
#[derive(Debug, Clone, PartialEq)]
pub struct Structured<T> {
    /// The output.
    pub value: T,
    /// Provider name.
    pub provider: &'static str,
    /// Model.
    pub model: String,
    /// Prompt.
    pub prompt: PromptRef,
    /// Attempts used.
    pub attempts: u32,
}

/// A streamed answer as seen by callers.
pub type AiTokenStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AiError>> + Send>>;

/// Routing + budget + validation.
#[derive(Debug, Clone)]
pub struct AiService {
    router: ProviderRouter,
    budget: BudgetGuard,
    embedder: Option<Arc<dyn Embedder>>,
}

impl AiService {
    /// The service.
    pub fn new(router: ProviderRouter, budget: BudgetGuard) -> Self {
        Self {
            router,
            budget,
            embedder: None,
        }
    }

    /// Reports `embedder` in [`AiStatus`].
    #[must_use]
    pub fn with_embedder(mut self, embedder: Arc<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// The budget guard.
    pub fn budget(&self) -> &BudgetGuard {
        &self.budget
    }

    /// One structured call, validated against `req.schema`.
    pub async fn complete_json(&self, req: JsonRequest) -> Result<ValidatedJson, AiError> {
        let provider = self.router.route(&req.caller.username)?;
        let validator = schema::compile(&req.schema)?;
        let original_user = req.user.clone();
        let mut attempt_req = req;
        let mut total = Usage::default();
        let mut last: Vec<String> = Vec::new();
        for attempt in 0..=MAX_INVALID_OUTPUT_RETRIES {
            self.budget.check(&attempt_req.caller).await?;
            let outcome = provider.complete_json(attempt_req.clone()).await;
            let feedback = match outcome {
                Ok(done) => {
                    self.budget
                        .record(
                            &attempt_req.caller,
                            provider.name(),
                            &done.model,
                            &done.usage,
                        )
                        .await?;
                    add_usage(&mut total, &done.usage);
                    match schema::validate(&validator, &done.value) {
                        Ok(()) => {
                            return Ok(ValidatedJson {
                                value: done.value,
                                provider: provider.name(),
                                model: done.model,
                                prompt: attempt_req.prompt,
                                attempts: attempt + 1,
                                usage: total,
                            });
                        }
                        Err(violations) => {
                            last = violations.iter().map(Violation::summary).collect();
                            schema_feedback(&violations)
                        }
                    }
                }
                Err(ProviderError::NotJson(_)) => {
                    last = vec!["/: not JSON".to_owned()];
                    "Your previous reply was not a JSON value.".to_owned()
                }
                Err(e) => return Err(e.into()),
            };
            tracing::info!(
                prompt = %attempt_req.prompt,
                attempt = attempt + 1,
                violations = last.len(),
                "structured output invalid"
            );
            attempt_req.user = format!(
                "{original_user}\n\n---\n{feedback}\nReply again with exactly one JSON object that matches the JSON schema supplied with this request."
            );
        }
        Err(AiError::InvalidOutput {
            prompt_id: attempt_req.prompt.id,
            version: attempt_req.prompt.version,
            attempts: MAX_INVALID_OUTPUT_RETRIES + 1,
            errors: last,
        })
    }

    /// Runs `prompt` on `input` and deserializes the validated output into `T`.
    pub async fn complete<T: DeserializeOwned>(
        &self,
        caller: AiCaller,
        prompt: &PromptDef,
        input: &impl Serialize,
        max_tokens: u32,
    ) -> Result<Structured<T>, AiError> {
        let out = self
            .complete_json(prompt.json_request(caller, input, max_tokens)?)
            .await?;
        let value = serde_json::from_value(out.value).map_err(|e| AiError::OutputType {
            prompt_id: out.prompt.id.clone(),
            version: out.prompt.version,
            message: e.to_string(),
        })?;
        Ok(Structured {
            value,
            provider: out.provider,
            model: out.model,
            prompt: out.prompt,
            attempts: out.attempts,
        })
    }

    /// A streamed answer; usage is recorded when the stream completes.
    pub async fn stream(&self, req: ChatRequest) -> Result<AiTokenStream, AiError> {
        let provider = self.router.route(&req.caller.username)?;
        self.budget.check(&req.caller).await?;
        let caller = req.caller.clone();
        let inner = provider.stream(req).await?;
        let budget = self.budget.clone();
        let name = provider.name();
        Ok(Box::pin(inner.then(move |item| {
            let budget = budget.clone();
            let caller = caller.clone();
            async move {
                let ev = item?;
                if let StreamEvent::Done { usage, model } = &ev {
                    budget.record(&caller, name, model, usage).await?;
                }
                Ok(ev)
            }
        })))
    }

    /// The `GET /ai/status` snapshot for `caller` (`queue_depth` is filled in by the caller).
    pub async fn status(&self, caller: &AiCaller) -> Result<AiStatus, AiError> {
        let budget = self.budget.snapshot(caller).await?;
        let (enabled, provider) = match self.router.route(&caller.username) {
            Ok(p) => (true, Some(p)),
            Err(AiError::Disabled) => (false, None),
            Err(_) => (true, None),
        };
        let provider_status = provider.as_ref().map(|p| ProviderStatus {
            name: p.name().to_owned(),
            model: p.model().to_owned(),
            health: p.health(),
        });
        let provider_pause = provider_status.as_ref().and_then(|p| match p.health.state {
            crate::provider::HealthState::Paused { reason, until } => {
                Some(PauseInfo { reason, until })
            }
            _ => None,
        });
        Ok(AiStatus {
            enabled,
            provider: provider_status,
            paused: budget.paused.or(provider_pause),
            queue_depth: None,
            usage: UsageStatus {
                day: budget.day,
                user: budget.user,
                global: budget.global,
            },
            limits: self.budget.limits(),
            embeddings: self.embedder.as_ref().map(|e| EmbeddingsStatus {
                model_id: e.model_id().to_owned(),
                dims: e.dims(),
            }),
        })
    }
}

fn add_usage(total: &mut Usage, u: &Usage) {
    total.input_tokens += u.input_tokens;
    total.cache_creation_input_tokens += u.cache_creation_input_tokens;
    total.cache_read_input_tokens += u.cache_read_input_tokens;
    total.output_tokens += u.output_tokens;
    total.cost_micros = match (total.cost_micros, u.cost_micros) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    };
}

fn schema_feedback(violations: &[Violation]) -> String {
    let mut s = String::from("Your previous reply did not match the required JSON schema:\n");
    for v in violations {
        let path = if v.instance_path.is_empty() {
            "/"
        } else {
            &v.instance_path
        };
        s.push_str("- ");
        s.push_str(path);
        s.push_str(": ");
        s.push_str(&v.detail);
        s.push('\n');
    }
    s
}
