//! AI foundation (PLAN §9, L5, L18, L19, D9, D20, D23): provider abstraction, prompts,
//! structured-output validation, budgets, status, and local embeddings. No job logic lives here;
//! the linking, filing, entity, custody and ask jobs build on these pieces.
//!
//! - [`LlmProvider`]: the provider trait, implemented by [`ClaudeCliProvider`] (`claude -p`,
//!   the default, D20 = a), [`AnthropicApiProvider`] (Messages API, selectable per user, D23)
//!   and, with the `test-support` feature, [`fake::FakeLlmProvider`] (fixture replay).
//! - [`ProviderRouter`]: default provider plus per-user overrides.
//! - [`AiService`]: routing + [`BudgetGuard`] + schema validation with at most two retries on
//!   invalid output ([`MAX_INVALID_OUTPUT_RETRIES`]); the entry point jobs use.
//! - [`prompts`]: the versioned prompt registry (`prompts/*.md`, hashed at build time) and the
//!   typed outputs in [`outputs`].
//! - [`embed`]: the [`Embedder`] trait and the in-process ONNX embedder (feature `onnx`), which
//!   never runs concurrently with a `claude -p` call ([`CpuGate`], §9.1b).

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::float_cmp,
        clippy::field_reassign_with_default
    )
)]

pub mod anthropic_api;
pub mod budget;
pub mod claude_cli;
pub mod embed;
pub mod error;
#[cfg(feature = "test-support")]
pub mod fake;
pub mod gate;
pub mod outputs;
pub mod prompts;
pub mod provider;
pub mod request;
pub mod retry;
pub mod router;
pub mod schema;
pub mod service;
pub mod status;

pub use anthropic_api::{AnthropicApiConfig, AnthropicApiProvider, ApiKey, Pricing};
pub use budget::{BudgetGuard, BudgetLimits, MemoryUsageStore, PgUsageStore, UsageStore};
pub use claude_cli::{ClaudeCliConfig, ClaudeCliProvider};
pub use embed::{Embedder, Embedding};
pub use error::{AiError, PauseReason, ProviderError};
pub use gate::CpuGate;
pub use prompts::{LANGUAGE_INSTRUCTION, PROMPTS, PromptDef};
pub use provider::{HealthState, LlmProvider, ProviderHealth};
pub use request::{
    AiCaller, ChatRequest, JsonCompletion, JsonRequest, PromptRef, StreamEvent, TokenStream, Usage,
};
pub use router::ProviderRouter;
pub use service::{
    AiService, AiTokenStream, MAX_INVALID_OUTPUT_RETRIES, Structured, ValidatedJson,
};
pub use status::{AiStatus, PauseInfo, UsageStatus};
