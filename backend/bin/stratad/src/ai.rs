//! Builds the AI foundation from the `STRATA_AI__…` and `STRATA_BUDGETS__…` settings (PLAN §9.1, §9.1b, D9, D20, D23):
//! the providers the routing rules need, the per-user router, the budget guard and the
//! optional in-process embedder, sharing one [`CpuGate`] so an embedding never overlaps a
//! `claude -p` process.
//!
//! - `claude_cli` is built when any user is routed to it (by default every user is, D23);
//!   `anthropic_api` when any user is; a provider nobody uses is not built (no key read, no
//!   launcher check).
//! - Embeddings are optional: while `ai.embedding.model_dir` or `ai.embedding.onnxruntime_lib`
//!   is unset or a file is missing, they are off and one log line says why; everything else
//!   keeps working (principle 6). When configured, a
//!   [`LazyEmbedder`] keeps it loaded from start-up (`ai.embedding.idle_unload_secs = 0`, the
//!   default) or loads it on first use and unloads it after `ai.embedding.idle_unload_secs`
//!   without calls (§9.1b); a load failure fails only the calls that needed it.

use std::sync::Arc;
use std::time::Duration;

use strata_ai::anthropic_api::{AnthropicApiConfig, AnthropicApiProvider, ApiKey, Pricing};
use strata_ai::claude_cli::{ClaudeCliConfig, ClaudeCliProvider};
use strata_ai::embed::Pooling;
use strata_ai::embed::onnx::{OnnxEmbedder, OnnxEmbedderConfig};
use strata_ai::retry::TokioSleeper;
use strata_ai::{
    AiService, BudgetGuard, BudgetLimits, CpuGate, Embedder, EmbedderLoader, LazyEmbedder,
    LlmProvider, PgUsageStore, ProviderRouter,
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
    /// The embedder, when on (the [`LazyEmbedder`] below).
    pub embedder: Option<Arc<dyn Embedder>>,
    /// The on-demand embedder, for its idle reaper.
    pub lazy_embedder: Option<Arc<LazyEmbedder>>,
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
    cfg.launch_dir.clone_from(&s.launch_dir);
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
            (None, None) => {
                "STRATA_AI__EMBEDDING__MODEL_DIR and STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB are"
            }
            (None, Some(_)) => "STRATA_AI__EMBEDDING__MODEL_DIR is",
            _ => "STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB is",
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
    clock: Arc<dyn Clock>,
) -> (Option<Arc<LazyEmbedder>>, EmbedderState) {
    match embedder_config(s) {
        Ok(cfg) => {
            let model_id = cfg.model_id.clone();
            let dims = cfg.dims;
            let load_gate = gate.clone();
            let loader: EmbedderLoader = Arc::new(move || {
                OnnxEmbedder::load(&cfg, load_gate.clone())
                    .map(|e| Arc::new(e) as Arc<dyn Embedder>)
            });
            tracing::info!(
                model = %model_id,
                idle_unload_secs = s.idle_unload_secs,
                "embeddings enabled (idle_unload_secs = 0: loaded at start and kept loaded; \
                 otherwise loaded on first use)"
            );
            let lazy = LazyEmbedder::new(
                model_id.clone(),
                dims,
                loader,
                clock,
                Duration::from_secs(u64::from(s.idle_unload_secs)),
                Some(gate.clone()),
            );
            (Some(Arc::new(lazy)), EmbedderState::Ready { model_id })
        }
        Err(reason) => {
            tracing::warn!(
                reason = %reason,
                "embeddings disabled: semantic search, similarity edges and duplicate \
                 embeddings stay off until STRATA_AI__EMBEDDING__… is configured (docs/RUNBOOK.md §10)"
            );
            (None, EmbedderState::Disabled { reason })
        }
    }
}

/// Builds the AI foundation. Blocks while the embedding model loads; call it from a blocking
/// context (`serve` uses `spawn_blocking`).
pub fn build(config: &Config, db: AppDb, clock: Arc<dyn Clock>) -> Result<AiParts, StartupError> {
    let lazy_clock = clock.clone();
    let gate = CpuGate::new();
    let mut router = ProviderRouter::from_config(&config.ai);
    let claude_cli: Option<Arc<dyn LlmProvider>> =
        if config.ai_provider_in_use(AiProviderKind::ClaudeCli) {
            let cfg = claude_cli_config(config);
            if let Some(problem) = cfg.start_dir_problem() {
                tracing::warn!("{problem}");
            }
            let p = ClaudeCliProvider::new(cfg, clock.clone(), gate.clone()).map_err(ai_error)?;
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
                .ok_or_else(|| ai_error("STRATA_AI__ANTHROPIC_API__API_KEY_FILE is not set"))?;
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
    let (lazy_embedder, embedder_state) = load_embedder(&config.ai.embedding, &gate, lazy_clock);
    let embedder: Option<Arc<dyn Embedder>> = lazy_embedder.clone().map(|e| e as Arc<dyn Embedder>);
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
        lazy_embedder,
        embedder_state,
        gate,
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
        config.ai.claude_cli.launch_dir = Some("/".into());
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
                cli.launch_dir,
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
                Some("/".into()),
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
            Err("STRATA_AI__EMBEDDING__MODEL_DIR and STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB are not set".into())
        );
        s.onnxruntime_lib = Some("/nonexistent/libonnxruntime.so".into());
        assert_eq!(
            embedder_config(&s),
            Err("STRATA_AI__EMBEDDING__MODEL_DIR is not set".into())
        );
        let dir = tempfile::tempdir().expect("tempdir");
        s.model_dir = Some(dir.path().to_path_buf());
        s.onnxruntime_lib = None;
        assert_eq!(
            embedder_config(&s),
            Err("STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB is not set".into())
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
                dir.path().join("onnx/model.onnx").display()
            ))
        );
        std::fs::create_dir_all(dir.path().join("onnx")).expect("dir");
        std::fs::write(dir.path().join("onnx/model.onnx"), b"").expect("model");
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
}
