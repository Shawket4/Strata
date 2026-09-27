//! Provider routing per user (D23): a default provider plus per-username overrides, so the API
//! provider can take over a user without code changes.

use std::collections::BTreeMap;
use std::sync::Arc;

use strata_common::config::{AiConfig, AiProviderKind};

use crate::error::AiError;
use crate::provider::LlmProvider;

/// Chooses the provider for a user.
#[derive(Debug, Clone)]
pub struct ProviderRouter {
    default: AiProviderKind,
    per_user: BTreeMap<String, AiProviderKind>,
    claude_cli: Option<Arc<dyn LlmProvider>>,
    anthropic_api: Option<Arc<dyn LlmProvider>>,
}

impl ProviderRouter {
    /// Routing rules without providers; add them with [`with_provider`](Self::with_provider).
    pub fn new(default: AiProviderKind, per_user: BTreeMap<String, AiProviderKind>) -> Self {
        Self {
            default,
            per_user,
            claude_cli: None,
            anthropic_api: None,
        }
    }

    /// Rules from `ai.default_provider` and `ai.user_providers`.
    pub fn from_config(cfg: &AiConfig) -> Self {
        Self::new(cfg.default_provider, cfg.user_providers.clone())
    }

    /// Registers the provider serving `kind` (ignored for `Disabled`).
    #[must_use]
    pub fn with_provider(mut self, kind: AiProviderKind, provider: Arc<dyn LlmProvider>) -> Self {
        match kind {
            AiProviderKind::ClaudeCli => self.claude_cli = Some(provider),
            AiProviderKind::AnthropicApi => self.anthropic_api = Some(provider),
            AiProviderKind::Disabled => {}
        }
        self
    }

    /// The provider kind configured for `username`.
    pub fn kind_for(&self, username: &str) -> AiProviderKind {
        self.per_user.get(username).copied().unwrap_or(self.default)
    }

    /// The provider serving `username`.
    pub fn route(&self, username: &str) -> Result<Arc<dyn LlmProvider>, AiError> {
        match self.kind_for(username) {
            AiProviderKind::Disabled => Err(AiError::Disabled),
            AiProviderKind::ClaudeCli => self
                .claude_cli
                .clone()
                .ok_or(AiError::ProviderNotConfigured("claude_cli")),
            AiProviderKind::AnthropicApi => self
                .anthropic_api
                .clone()
                .ok_or(AiError::ProviderNotConfigured("anthropic_api")),
        }
    }
}
