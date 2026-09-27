//! `stratad serve` builds the AI foundation from the configuration: the providers the
//! routing rules need (D23: `claude -p` for everyone by default, per-user overrides), the
//! budget guard, and an optional embedder that is off with a reason when not configured.
#![allow(clippy::expect_used)] // tests: expect with messages

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pretty_assertions::assert_eq;
use strata_ai::{AiError, BudgetLimits};
use strata_common::config::AiProviderKind;
use strata_common::{Clock, Config};
use strata_testkit::TestDb;
use stratad::ai::{self, AiParts, EmbedderState};
use stratad::checks::StartupError;

fn key_file(dir: &Path, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("anthropic.key");
    std::fs::write(&path, "sk-ant-test\n").expect("key");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    path
}

async fn build(config: &Config) -> Result<AiParts, StartupError> {
    let db = TestDb::new().await.expect("db");
    let clock: Arc<dyn Clock> = Arc::new(db.clock.clone());
    ai::build(config, db.app_db.clone(), clock)
}

/// `(provider name, model)` serving `username`, or the routing error.
fn route(parts: &AiParts, username: &str) -> Result<(&'static str, String), AiError> {
    parts
        .router
        .route(username)
        .map(|p| (p.name(), p.model().to_owned()))
}

#[tokio::test]
async fn defaults_route_everyone_to_claude_cli_without_embeddings() {
    let parts = build(&Config::default()).await.expect("built");
    assert_eq!(
        (
            parts.claude_cli.as_ref().map(|p| p.name()),
            parts.anthropic_api.is_some(),
            parts.embedder.is_some()
        ),
        (Some("claude_cli"), false, false)
    );
    assert_eq!(route(&parts, "owner"), Ok(("claude_cli", "default".into())));
    assert_eq!(route(&parts, "guest"), Ok(("claude_cli", "default".into())));
    assert_eq!(
        parts.embedder_state,
        EmbedderState::Disabled {
            reason: "ai.embedding.model_dir and ai.embedding.onnxruntime_lib are not set".into()
        }
    );
    assert_eq!(
        parts.budget.limits(),
        BudgetLimits {
            per_user_daily_tokens: 2_000_000,
            per_user_daily_cost_micros: 0,
            global_daily_tokens: 4_000_000,
            global_daily_cost_micros: 0,
        }
    );
}

#[tokio::test]
async fn per_user_overrides_build_both_providers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = Config::default();
    config.ai.claude_cli.model = Some("opus".into());
    config.ai.anthropic_api.api_key_file = Some(key_file(dir.path(), 0o600));
    config.ai.user_providers = [
        ("guest".to_owned(), AiProviderKind::AnthropicApi),
        ("kid".to_owned(), AiProviderKind::Disabled),
    ]
    .into_iter()
    .collect();
    config.budgets.timezone = Some("Africa/Cairo".into());
    config.budgets.global_daily_cost_micros = 9_000_000;
    config.validate().expect("valid");
    let parts = build(&config).await.expect("built");
    assert_eq!(route(&parts, "owner"), Ok(("claude_cli", "opus".into())));
    assert_eq!(
        route(&parts, "guest"),
        Ok(("anthropic_api", "claude-opus-5-5".into()))
    );
    assert!(matches!(route(&parts, "kid"), Err(AiError::Disabled)));
    assert_eq!(parts.budget.limits().global_daily_cost_micros, 9_000_000);
}

#[tokio::test]
async fn api_as_default_builds_only_the_api_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = Config::default();
    config.ai.default_provider = AiProviderKind::AnthropicApi;
    config.ai.anthropic_api.api_key_file = Some(key_file(dir.path(), 0o400));
    config.ai.anthropic_api.model = "claude-opus-5".into();
    let parts = build(&config).await.expect("built");
    assert_eq!(
        (parts.claude_cli.is_some(), parts.anthropic_api.is_some()),
        (false, true)
    );
    assert_eq!(
        route(&parts, "owner"),
        Ok(("anthropic_api", "claude-opus-5".into()))
    );
}

#[tokio::test]
async fn unusable_settings_refuse_to_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = Config::default();
    config.ai.default_provider = AiProviderKind::AnthropicApi;
    let key = key_file(dir.path(), 0o644);
    config.ai.anthropic_api.api_key_file = Some(key.clone());
    assert_eq!(
        build(&config).await.map(|_| ()),
        Err(StartupError::Config(format!(
            "ai: invalid AI configuration: api key file {} has mode 644; it must not be readable by group or others (0600)",
            key.display()
        )))
    );
    // A disabled default with no overrides builds no provider at all.
    config.ai.default_provider = AiProviderKind::Disabled;
    let parts = build(&config).await.expect("built");
    assert_eq!(
        (parts.claude_cli.is_some(), parts.anthropic_api.is_some()),
        (false, false)
    );
    assert!(matches!(route(&parts, "owner"), Err(AiError::Disabled)));
}
