//! Builds the AI foundation from `[ai]` and `[budgets]` (PLAN §9.1, §9.1b, D9, D20, D23):
//! the providers the routing rules need, the per-user router, the budget guard and the
//! optional in-process embedder, sharing one [`CpuGate`] so an embedding never overlaps a
//! `claude -p` process.
//!
//! - `claude_cli` is built when any user is routed to it (by default every user is, D23);
//!   `anthropic_api` when any user is; a provider nobody uses is not built (no key read, no
//!   launcher check).
//! - Embeddings are optional: while `ai.embedding.model_dir` or `ai.embedding.onnxruntime_lib`
//!   is unset or missing, or the model fails to load, they are off and one log line says why;
//!   everything else keeps working (principle 6).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use strata_ai::anthropic_api::{AnthropicApiConfig, AnthropicApiProvider, ApiKey, Pricing};
use strata_ai::claude_cli::{ClaudeCliConfig, ClaudeCliProvider};
use strata_ai::embed::Pooling;
use strata_ai::embed::onnx::{OnnxEmbedder, OnnxEmbedderConfig};
use strata_ai::retry::TokioSleeper;
use strata_ai::{
    AiService, BudgetGuard, BudgetLimits, CpuGate, Embedder, LlmProvider, PgUsageStore,
    ProviderRouter,
};
use strata_common::config::{AiProviderKind, EmbeddingPooling, EmbeddingSettings};
use strata_common::{Clock, Config};
use strata_index::AppDb;

use crate::checks::StartupError;

/// Whether embeddings are on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedderState {
    /// Loaded; vectors carry this model ID.
    Ready {
        /// Model ID.
        model_id: String,
    },
    /// Off, and why (logged once at startup).
    Disabled {
        /// The reason.
        reason: String,
    },
}

/// The assembled AI foundation.
#[derive(Debug, Clone)]
pub struct AiParts {
    /// Routing + budgets + validation: the entry point for AI jobs and handlers.
    pub service: AiService,
    /// The router inside `service` (per-user provider choice).
    pub router: ProviderRouter,
    /// The `claude -p` provider, when some user is routed to it.
    pub claude_cli: Option<Arc<dyn LlmProvider>>,
    /// The Messages API provider, when some user is routed to it.
    pub anthropic_api: Option<Arc<dyn LlmProvider>>,
    /// The budget guard inside `service`.
    pub budget: BudgetGuard,
    /// The embedder, when on.
    pub embedder: Option<Arc<dyn Embedder>>,
    /// Whether embeddings are on, and why not.
    pub embedder_state: EmbedderState,
    /// The gate shared by the CLI provider and the embedder.
    pub gate: CpuGate,
}

fn secs(s: u32) -> Duration {
    Duration::from_secs(u64::from(s))
}

fn ai_error(e: impl std::fmt::Display) -> StartupError {
    StartupError::Config(format!("ai: {e}"))
}

/// The `claude -p` provider configuration from `ai.claude_cli`.
pub fn claude_cli_config(config: &Config) -> ClaudeCliConfig {
    let s = &config.ai.claude_cli;
    let mut cfg = ClaudeCliConfig::new(s.command.clone(), s.scratch_dir.clone());
    cfg.model.clone_from(&s.model);
    cfg.timeout = secs(s.timeout_secs);
    cfg.max_concurrency = usize::try_from(s.max_concurrency).unwrap_or(usize::MAX);
    cfg.kill_grace = secs(s.kill_grace_secs);
    cfg.default_pause = secs(s.usage_limit_pause_secs);
    cfg
}

/// The Messages API provider configuration from `ai.anthropic_api`.
pub fn anthropic_api_config(config: &Config) -> AnthropicApiConfig {
    let s = &config.ai.anthropic_api;
    AnthropicApiConfig {
        base_url: s.base_url.clone(),
        model: s.model.clone(),
        effort: s.effort.clone(),
        timeout: secs(s.timeout_secs),
        max_retries: s.max_retries,
        pricing: Pricing {
            input_micros_per_mtok: s.input_micros_per_mtok,
            output_micros_per_mtok: s.output_micros_per_mtok,
        },
        ..AnthropicApiConfig::default()
    }
}

/// The embedder configuration from `ai.embedding`, or why embeddings are off.
pub fn embedder_config(s: &EmbeddingSettings) -> Result<OnnxEmbedderConfig, String> {
    let (Some(model_dir), Some(lib)) = (&s.model_dir, &s.onnxruntime_lib) else {
        let missing = match (&s.model_dir, &s.onnxruntime_lib) {
            (None, None) => "ai.embedding.model_dir and ai.embedding.onnxruntime_lib are",
            (None, Some(_)) => "ai.embedding.model_dir is",
            _ => "ai.embedding.onnxruntime_lib is",
        };
        return Err(format!("{missing} not set"));
    };
    let model_file = model_dir.join(&s.model_file);
    let tokenizer = model_dir.join(&s.tokenizer_file);
    for (what, path) in [
        ("ONNX Runtime library", lib.as_path()),
        ("model file", model_file.as_path()),
        ("tokenizer", tokenizer.as_path()),
    ] {
        if !path.is_file() {
            return Err(format!("{what} {} does not exist", path.display()));
        }
    }
    Ok(OnnxEmbedderConfig {
        model_dir: model_dir.clone(),
        model_file: s.model_file.clone(),
        tokenizer_file: s.tokenizer_file.clone(),
        onnxruntime_lib: lib.clone(),
        model_id: s.model_id.clone(),
        dims: usize::try_from(s.dims).unwrap_or(usize::MAX),
        pooling: match s.pooling {
            EmbeddingPooling::Cls => Pooling::Cls,
            EmbeddingPooling::Mean => Pooling::Mean,
        },
        max_tokens: usize::try_from(s.max_tokens).unwrap_or(usize::MAX),
        max_batch_tokens: usize::try_from(s.max_batch_tokens).unwrap_or(usize::MAX),
        pad_batches: s.pad_batches,
        nice: s.nice,
    })
}

fn load_embedder(
    s: &EmbeddingSettings,
    gate: &CpuGate,
) -> (Option<Arc<dyn Embedder>>, EmbedderState) {
    let loaded = embedder_config(s).and_then(|cfg| {
        OnnxEmbedder::load(&cfg, gate.clone()).map_err(|e| format!("loading the model failed: {e}"))
    });
    match loaded {
        Ok(embedder) => {
            let model_id = embedder.model_id().to_owned();
            tracing::info!(model = %model_id, "embeddings enabled");
            (
                Some(Arc::new(embedder) as Arc<dyn Embedder>),
                EmbedderState::Ready { model_id },
            )
        }
        Err(reason) => {
            tracing::warn!(
                reason = %reason,
                "embeddings disabled: semantic search, similarity edges and duplicate \
                 embeddings stay off until ai.embedding is configured (docs/RUNBOOK.md §10)"
            );
            (None, EmbedderState::Disabled { reason })
        }
    }
}

/// Builds the AI foundation. Blocks while the embedding model loads; call it from a blocking
/// context (`serve` uses `spawn_blocking`).
pub fn build(config: &Config, db: AppDb, clock: Arc<dyn Clock>) -> Result<AiParts, StartupError> {
    let gate = CpuGate::new();
    let mut router = ProviderRouter::from_config(&config.ai);
    let claude_cli: Option<Arc<dyn LlmProvider>> =
        if config.ai_provider_in_use(AiProviderKind::ClaudeCli) {
            let p = ClaudeCliProvider::new(claude_cli_config(config), clock.clone(), gate.clone())
                .map_err(ai_error)?;
            if let Some(hint) = scratch_dir_hint(&config.ai.claude_cli.scratch_dir) {
                tracing::info!("{hint}");
            }
            Some(Arc::new(p))
        } else {
            None
        };
    let anthropic_api: Option<Arc<dyn LlmProvider>> =
        if config.ai_provider_in_use(AiProviderKind::AnthropicApi) {
            let path = config
                .ai
                .anthropic_api
                .api_key_file
                .as_deref()
                .ok_or_else(|| ai_error("ai.anthropic_api.api_key_file is not set"))?;
            let key = ApiKey::from_file(path).map_err(ai_error)?;
            let p = AnthropicApiProvider::new(
                anthropic_api_config(config),
                key,
                clock.clone(),
                Arc::new(TokioSleeper),
            )
            .map_err(ai_error)?;
            Some(Arc::new(p))
        } else {
            None
        };
    if let Some(p) = &claude_cli {
        router = router.with_provider(AiProviderKind::ClaudeCli, p.clone());
    }
    if let Some(p) = &anthropic_api {
        router = router.with_provider(AiProviderKind::AnthropicApi, p.clone());
    }
    let tz = config
        .budget_tz()
        .map_err(|e| StartupError::Config(e.to_string()))?;
    let budget = BudgetGuard::new(
        BudgetLimits::from_config(&config.budgets),
        tz,
        clock,
        Arc::new(PgUsageStore::new(db)),
    );
    let (embedder, embedder_state) = load_embedder(&config.ai.embedding, &gate);
    let mut service = AiService::new(router.clone(), budget.clone());
    if let Some(e) = &embedder {
        service = service.with_embedder(e.clone());
    }
    tracing::info!(
        default_provider = ?config.ai.default_provider,
        overrides = config.ai.user_providers.len(),
        claude_cli = claude_cli.is_some(),
        anthropic_api = anthropic_api.is_some(),
        "AI providers ready"
    );
    Ok(AiParts {
        service,
        router,
        claude_cli,
        anthropic_api,
        budget,
        embedder,
        embedder_state,
        gate,
    })
}

/// Whether `path` names an existing directory (the CLI scratch directory is checked by the
/// operator's setup, not required here: it belongs to the `strata-ai` user).
pub fn scratch_dir_hint(path: &Path) -> Option<String> {
    (!path.is_dir()).then(|| {
        format!(
            "ai.claude_cli.scratch_dir {} is not visible to stratad (fine when it belongs to \
             the strata-ai user; see docs/RUNBOOK.md §9)",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn provider_configs_follow_the_settings() {
        let mut config = Config::default();
        config.ai.claude_cli.command = vec!["sudo".into(), "-n".into()];
        config.ai.claude_cli.model = Some("opus".into());
        config.ai.claude_cli.max_concurrency = 2;
        config.ai.claude_cli.timeout_secs = 90;
        config.ai.anthropic_api.effort = Some("medium".into());
        config.ai.anthropic_api.max_retries = 4;
        let cli = claude_cli_config(&config);
        assert_eq!(
            (
                cli.command,
                cli.working_dir,
                cli.model,
                cli.max_concurrency,
                cli.timeout,
                cli.kill_grace,
                cli.default_pause,
                cli.env,
            ),
            (
                vec!["sudo".to_owned(), "-n".to_owned()],
                "/var/lib/strata-ai/scratch".into(),
                Some("opus".to_owned()),
                2,
                Duration::from_secs(90),
                Duration::from_secs(5),
                Duration::from_secs(1800),
                ClaudeCliConfig::default_env(),
            )
        );
        let api = anthropic_api_config(&config);
        assert_eq!(
            api,
            AnthropicApiConfig {
                base_url: "https://api.anthropic.com".into(),
                model: "claude-opus-5-5".into(),
                effort: Some("medium".into()),
                timeout: Duration::from_secs(600),
                max_retries: 4,
                pricing: Pricing {
                    input_micros_per_mtok: 4_000_000,
                    output_micros_per_mtok: 20_000_000,
                },
                ..AnthropicApiConfig::default()
            }
        );
    }

    #[test]
    fn embedder_is_off_until_both_paths_exist() {
        let mut s = Config::default().ai.embedding;
        assert_eq!(
            embedder_config(&s),
            Err("ai.embedding.model_dir and ai.embedding.onnxruntime_lib are not set".into())
        );
        s.onnxruntime_lib = Some("/nonexistent/libonnxruntime.so".into());
        assert_eq!(
            embedder_config(&s),
            Err("ai.embedding.model_dir is not set".into())
        );
        let dir = tempfile::tempdir().expect("tempdir");
        s.model_dir = Some(dir.path().to_path_buf());
        s.onnxruntime_lib = None;
        assert_eq!(
            embedder_config(&s),
            Err("ai.embedding.onnxruntime_lib is not set".into())
        );
        let lib = dir.path().join("libonnxruntime.so");
        s.onnxruntime_lib = Some(lib.clone());
        assert_eq!(
            embedder_config(&s),
            Err(format!(
                "ONNX Runtime library {} does not exist",
                lib.display()
            ))
        );
        std::fs::write(&lib, b"").expect("lib");
        assert_eq!(
            embedder_config(&s),
            Err(format!(
                "model file {} does not exist",
                dir.path().join("onnx/model_quint8_avx2.onnx").display()
            ))
        );
        std::fs::create_dir_all(dir.path().join("onnx")).expect("dir");
        std::fs::write(dir.path().join("onnx/model_quint8_avx2.onnx"), b"").expect("model");
        assert_eq!(
            embedder_config(&s),
            Err(format!(
                "tokenizer {} does not exist",
                dir.path().join("tokenizer.json").display()
            ))
        );
        std::fs::write(dir.path().join("tokenizer.json"), b"{}").expect("tokenizer");
        s.pooling = EmbeddingPooling::Mean;
        s.max_tokens = 512;
        let cfg = embedder_config(&s).expect("complete");
        let mut expected = OnnxEmbedderConfig::granite_97m_r2(dir.path().to_path_buf(), lib);
        expected.pooling = Pooling::Mean;
        expected.max_tokens = 512;
        assert_eq!(cfg, expected);
    }

    #[test]
    fn scratch_dir_hint_names_invisible_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(scratch_dir_hint(dir.path()), None);
        assert_eq!(
            scratch_dir_hint(Path::new("/nonexistent/scratch")),
            Some(
                "ai.claude_cli.scratch_dir /nonexistent/scratch is not visible to stratad (fine \
                 when it belongs to the strata-ai user; see docs/RUNBOOK.md §9)"
                    .into()
            )
        );
    }
}
