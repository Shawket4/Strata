//! Request and response types shared by every provider.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strata_index::UserScope;

use crate::error::ProviderError;

/// Who a call is made for: the user's scope (budget accounting runs in it, principle 7) and
/// username (provider routing is configured per username, D23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCaller {
    /// The authenticated user or job owner.
    pub scope: UserScope,
    /// The user's username (routing key).
    pub username: String,
}

/// Identity of the prompt a request was built from; recorded in provenance (§9.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PromptRef {
    /// Prompt id, e.g. `linking`.
    pub id: String,
    /// Prompt version.
    pub version: u32,
}

impl fmt::Display for PromptRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.v{}", self.id, self.version)
    }
}

/// A free-text (streamed) request.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    /// Prompt identity.
    pub prompt: PromptRef,
    /// System prompt (the prompt file's text).
    pub system: String,
    /// User content (the job's input, usually JSON).
    pub user: String,
    /// Output token limit (the API enforces it; `claude -p` has no such flag).
    pub max_tokens: u32,
    /// Who the call is for.
    pub caller: AiCaller,
}

impl ChatRequest {
    /// SHA-256 (hex) of the model input: system prompt, a NUL separator, user content. Keys
    /// fake-provider fixtures (§9.1).
    pub fn input_hash(&self) -> String {
        input_hash(&self.system, &self.user)
    }
}

/// A structured-output request: the reply must be one JSON value matching `schema`.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonRequest {
    /// Prompt identity.
    pub prompt: PromptRef,
    /// System prompt.
    pub system: String,
    /// User content.
    pub user: String,
    /// JSON schema (draft 2020-12) of the expected output.
    pub schema: Arc<serde_json::Value>,
    /// Output token limit.
    pub max_tokens: u32,
    /// Who the call is for.
    pub caller: AiCaller,
}

impl JsonRequest {
    /// SHA-256 (hex) of the model input (see [`ChatRequest::input_hash`]).
    pub fn input_hash(&self) -> String {
        input_hash(&self.system, &self.user)
    }
}

/// SHA-256 (hex) of `system` + NUL + `user`.
pub fn input_hash(system: &str, user: &str) -> String {
    let mut h = Sha256::new();
    h.update(system.as_bytes());
    h.update([0u8]);
    h.update(user.as_bytes());
    hex::encode(h.finalize())
}

/// Token usage of one call. Budgets count every token: input (including prompt-cache writes
/// and reads) plus output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Uncached input tokens.
    pub input_tokens: u64,
    /// Input tokens written to the prompt cache.
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    /// Input tokens read from the prompt cache.
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    /// Output tokens (including thinking).
    pub output_tokens: u64,
    /// Estimated cost in micro-USD, when known.
    #[serde(default)]
    pub cost_micros: Option<u64>,
}

impl Usage {
    /// All input tokens (uncached + cache writes + cache reads).
    pub fn total_input(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_creation_input_tokens)
            .saturating_add(self.cache_read_input_tokens)
    }

    /// All tokens counted against budgets.
    pub fn total(&self) -> u64 {
        self.total_input().saturating_add(self.output_tokens)
    }
}

/// A successful structured call.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonCompletion {
    /// The parsed JSON output (validated against the schema by [`crate::AiService`]).
    pub value: serde_json::Value,
    /// Usage for budgeting.
    pub usage: Usage,
    /// The model that answered.
    pub model: String,
}

/// One item of a streamed answer.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Next piece of answer text.
    Text(String),
    /// The answer is complete.
    Done {
        /// Usage for budgeting.
        usage: Usage,
        /// The model that answered.
        model: String,
    },
}

/// A streamed answer. Ends after [`StreamEvent::Done`] or the first error.
pub type TokenStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_hash_is_sha256_of_system_nul_user() {
        // printf 'sys\0user' | sha256sum
        assert_eq!(
            input_hash("sys", "user"),
            "d9a85eb23dd96f9c5e0bfaa3121633c2d678e44c5582ec82120d4132f70ff461"
        );
        // The separator matters: "sy" + "suser" hashes differently.
        assert_ne!(input_hash("sy", "suser"), input_hash("sys", "user"));
    }

    #[test]
    fn usage_totals_count_cache_tokens_as_input() {
        let u = Usage {
            input_tokens: 2,
            cache_creation_input_tokens: 5290,
            cache_read_input_tokens: 3289,
            output_tokens: 27,
            cost_micros: None,
        };
        assert_eq!(u.total_input(), 8581);
        assert_eq!(u.total(), 8608);
    }
}
