//! `stratad.toml` configuration with environment overrides (PLAN §14 "Config").
//!
//! Layering, lowest to highest precedence: built-in defaults → TOML file → environment.
//! Environment variables named `STRATA__<SECTION>__<KEY>` (double underscores between path
//! segments, case-insensitive) override the matching key, e.g.
//! `STRATA__DATABASE__APP_URL=postgres://…` or `STRATA__THRESHOLDS__DEDUPE__TASK__NEAR=0.6`.
//! The override value is parsed with the type of the key it replaces (string, integer, float,
//! boolean, or a TOML array literal such as `["sudo", "-n"]`); keys that are unset by default are parsed as boolean/integer/float if possible and
//! as a string otherwise. Unknown keys — in the file or the environment — are errors, so typos
//! never pass silently.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// Prefix of configuration environment variables.
pub const ENV_PREFIX: &str = "STRATA__";

/// The complete server configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Root of per-user data (`users/<user_id>/vault`, PLAN §5.2).
    pub data_root: PathBuf,
    /// Address the API listens on (behind nginx).
    pub bind: SocketAddr,
    /// IANA timezone used for users who have not set one (tasks, reminders, digests).
    pub default_timezone: String,
    /// Postgres connection settings, one URL per role (§5.2).
    pub database: DatabaseConfig,
    /// AI provider settings (§9.1, D20, D23).
    pub ai: AiConfig,
    /// Confidence and similarity thresholds.
    pub thresholds: Thresholds,
    /// AI budgets (§9.1 budget guard).
    pub budgets: Budgets,
    /// Account lifecycle settings (D22, D25).
    pub accounts: AccountsConfig,
    /// Push provider credentials (D27).
    pub push: PushConfig,
    /// Tokens, passwords and rate limits (PLAN §8, D6).
    pub auth: AuthConfig,
}

/// One connection URL per database role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    /// `strata_owner`: migrations only.
    pub owner_url: String,
    /// `strata_app`: all request and job work (RLS-scoped).
    pub app_url: String,
    /// `strata_accounts`: login, signup, admin user management.
    pub accounts_url: String,
    /// Upper bound per pool (VPS: 1 core, keep small).
    pub max_connections: u32,
}

/// Which LLM backend serves a user (D20/D23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    /// `claude -p` headless (L18).
    ClaudeCli,
    /// Anthropic Messages API with a server key.
    AnthropicApi,
    /// AI off for this user (principle 6: everything else still works).
    Disabled,
}

/// AI provider settings (PLAN §9.1, §9.1b, D9, D20, D23).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    /// Provider for users without an entry in `user_providers` (D23: `claude_cli` serves every
    /// account by default).
    pub default_provider: AiProviderKind,
    /// Per-user provider override, keyed by username exactly as stored (`users.username`), so
    /// the API provider can take over a user without code changes (D23).
    pub user_providers: BTreeMap<String, AiProviderKind>,
    /// Maximum AI jobs per day across all users (0 = unlimited; §9.1 keeps Strata from
    /// crowding out the owner's interactive use of the subscription).
    pub daily_job_limit: u32,
    /// `claude -p` provider (L18, D20 = a).
    pub claude_cli: ClaudeCliSettings,
    /// Anthropic Messages API provider (D20, D23).
    pub anthropic_api: AnthropicApiSettings,
    /// Local embeddings (L19, D9 = a).
    pub embedding: EmbeddingSettings,
}

/// How `stratad` runs `claude -p` (docs/RUNBOOK.md §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeCliSettings {
    /// Launcher argument vector; the `claude` arguments are appended. Production:
    /// `["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]`.
    pub command: Vec<String>,
    /// Empty scratch working directory of the process.
    pub scratch_dir: PathBuf,
    /// `--model` (alias or full name); unset lets the CLI choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Concurrent `claude` processes (§9.1: low).
    pub max_concurrency: u32,
    /// Wall-clock limit per call, in seconds.
    pub timeout_secs: u32,
    /// Seconds between SIGTERM and SIGKILL when a call is cancelled or times out.
    pub kill_grace_secs: u32,
    /// Pause after a usage limit whose reset time is unknown, in seconds.
    pub usage_limit_pause_secs: u32,
}

/// The Anthropic Messages API provider (docs/RUNBOOK.md §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicApiSettings {
    /// File holding the API key (0600; read at startup, never logged). Required when any
    /// user is routed to `anthropic_api`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_file: Option<PathBuf>,
    /// Model ID.
    pub model: String,
    /// API base URL (no trailing slash).
    pub base_url: String,
    /// `output_config.effort` (`low`, `medium`, `high`, `xhigh`, `max`); unset = the model's
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Retries after the first attempt for transient errors (408/409/429/5xx/529).
    pub max_retries: u32,
    /// Per-request timeout in seconds (the whole response, including a stream).
    pub timeout_secs: u32,
    /// Uncached input price of `model`, micro-USD per million tokens (cost caps).
    pub input_micros_per_mtok: u64,
    /// Output price of `model`, micro-USD per million tokens.
    pub output_micros_per_mtok: u64,
}

/// How token states become one embedding vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingPooling {
    /// The first token's hidden state (granite-embedding r2).
    Cls,
    /// The mean over attended tokens.
    Mean,
}

/// The in-process ONNX embedder (docs/RUNBOOK.md §10). Embeddings are off, with a log line at
/// startup, while `model_dir` or `onnxruntime_lib` is unset or missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingSettings {
    /// Model directory (the Hugging Face repository layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<PathBuf>,
    /// `libonnxruntime.so` (ONNX Runtime ≥ 1.22), loaded at run time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onnxruntime_lib: Option<PathBuf>,
    /// ONNX file, relative to `model_dir`.
    pub model_file: PathBuf,
    /// `tokenizer.json`, relative to `model_dir`.
    pub tokenizer_file: PathBuf,
    /// Model ID stored with every vector (a change triggers a full re-embed).
    pub model_id: String,
    /// Output dimensions (the `chunks.embedding` column is `vector(384)`).
    pub dims: u32,
    /// Pooling.
    pub pooling: EmbeddingPooling,
    /// Tokens per text; longer texts are truncated.
    pub max_tokens: u32,
    /// Padded tokens per batch (bounds peak memory).
    pub max_batch_tokens: u32,
    /// Whether texts of different lengths may share a padded batch (off for the quint8
    /// export, whose output changes with padding).
    pub pad_batches: bool,
    /// Nice value of the embedding thread (0–19).
    pub nice: i32,
}

/// Effort levels accepted by the Messages API.
pub const ANTHROPIC_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Per-kind duplicate-detection thresholds (§9.7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DedupeThreshold {
    /// Trigram similarity (0..1) at or above which an item is a near duplicate.
    pub near: f64,
    /// Embedding cosine similarity (0..1) at or above which an item is a semantic duplicate.
    pub semantic: f64,
}

/// Confidence thresholds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// AI relations at or above this are applied automatically (§9.4, default 0.7).
    pub relation: f64,
    /// Custody events at or above this are applied automatically (§6.12, default 0.85).
    pub custody: f64,
    /// Duplicate thresholds keyed by item kind (`note`, `capture`, `task`, `person`, …).
    pub dedupe: BTreeMap<String, DedupeThreshold>,
}

/// AI budgets; exceeding one pauses (never fails) the affected work until the next budget day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    /// IANA timezone whose calendar days the caps count (unset = `default_timezone`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Tokens (input + output) per user per day (0 = unlimited).
    pub per_user_daily_tokens: u64,
    /// Estimated cost per user per day in micro-USD (0 = unlimited).
    pub per_user_daily_cost_micros: u64,
    /// Tokens per day across all users (0 = unlimited).
    pub global_daily_tokens: u64,
    /// Estimated cost per day across all users in micro-USD (0 = unlimited).
    pub global_daily_cost_micros: u64,
}

/// Account lifecycle settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountsConfig {
    /// Grace period between scheduling a deletion and the purge (D25, default 14 days).
    pub deletion_grace_days: u32,
    /// Maximum accounts waiting in `pending` (§15 signup cap).
    pub max_pending_signups: u32,
    /// How often the purge job looks for accounts whose grace period ended (seconds).
    pub purge_interval_secs: u32,
}

/// Authentication settings (PLAN §8, D6 = b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    /// Ed25519 signing key for access tokens: PKCS#8 PEM, mode 0600, created by
    /// `stratad keygen`. Read at startup, never logged.
    pub signing_key_file: PathBuf,
    /// `iss` claim of access tokens.
    pub issuer: String,
    /// `aud` claim of access tokens.
    pub audience: String,
    /// Access-token lifetime in seconds (PLAN §8: 15 minutes).
    pub access_token_ttl_secs: u32,
    /// Absolute lifetime of a device session and its refresh tokens, in days.
    pub session_ttl_days: u32,
    /// Period of the revocation-set reload from the database, in seconds.
    pub revocation_reload_secs: u32,
    /// Take the client IP from `X-Forwarded-For`/`Forwarded` (only behind nginx, which sets
    /// them); otherwise the socket peer address is used.
    pub trust_forwarded_for: bool,
    /// Minimum password length in characters.
    pub min_password_length: u32,
    /// Argon2id cost parameters for new password hashes.
    pub argon2: Argon2Config,
    /// Login and signup rate limits (§15).
    pub rate_limits: RateLimits,
}

/// Argon2id parameters (RFC 9106; defaults are OWASP's 19 MiB, 2 passes, 1 lane).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Argon2Config {
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Number of passes.
    pub iterations: u32,
    /// Degree of parallelism.
    pub parallelism: u32,
}

/// At most `max` events per `window_secs` (sliding window).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimit {
    /// Allowed events per window.
    pub max: u32,
    /// Window length in seconds.
    pub window_secs: u32,
}

/// Rate limits for the unauthenticated account endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimits {
    /// Login attempts per client IP.
    pub login_per_ip: RateLimit,
    /// Login attempts per (normalised) username.
    pub login_per_username: RateLimit,
    /// Sign-ups per client IP.
    pub signup_per_ip: RateLimit,
    /// Sign-ups across all clients.
    pub signup_global: RateLimit,
}

/// Push provider credentials (paths only; files are read at startup and never logged).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushConfig {
    /// Firebase service-account JSON (FCM, Android).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcm_service_account_path: Option<PathBuf>,
    /// APNs auth key (`.p8`, iOS/macOS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns_key_path: Option<PathBuf>,
    /// APNs key ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns_key_id: Option<String>,
    /// APNs team ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns_team_id: Option<String>,
    /// APNs topic (bundle ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apns_topic: Option<String>,
    /// WNS client credentials file (Windows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wns_credentials_path: Option<PathBuf>,
}

/// Default Anthropic API model (Claude Opus 5.5; list price $4 / $20 per million input/output
/// tokens, the default `input_micros_per_mtok` / `output_micros_per_mtok`).
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";

/// Item kinds with duplicate thresholds by default, and their defaults `(near, semantic)`.
pub const DEFAULT_DEDUPE_THRESHOLDS: &[(&str, f64, f64)] = &[
    ("alias", 0.8, 0.9),
    ("capture", 0.6, 0.9),
    ("company", 0.7, 0.9),
    ("concept", 0.7, 0.88),
    ("document", 0.7, 0.9),
    ("note", 0.6, 0.9),
    ("person", 0.7, 0.9),
    ("place", 0.7, 0.9),
    ("task", 0.6, 0.88),
];

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            default_provider: AiProviderKind::ClaudeCli,
            user_providers: BTreeMap::new(),
            daily_job_limit: 0,
            claude_cli: ClaudeCliSettings {
                command: vec!["/usr/local/bin/claude".to_owned()],
                scratch_dir: PathBuf::from("/var/lib/strata-ai/scratch"),
                model: None,
                max_concurrency: 1,
                timeout_secs: 300,
                kill_grace_secs: 5,
                usage_limit_pause_secs: 1800,
            },
            anthropic_api: AnthropicApiSettings {
                api_key_file: None,
                model: DEFAULT_ANTHROPIC_MODEL.to_owned(),
                base_url: "https://api.anthropic.com".to_owned(),
                effort: None,
                max_retries: 3,
                timeout_secs: 600,
                input_micros_per_mtok: 4_000_000,
                output_micros_per_mtok: 20_000_000,
            },
            embedding: EmbeddingSettings {
                model_dir: None,
                onnxruntime_lib: None,
                model_file: PathBuf::from("onnx/model_quint8_avx2.onnx"),
                tokenizer_file: PathBuf::from("tokenizer.json"),
                model_id:
                    "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model_quint8_avx2"
                        .to_owned(),
                dims: 384,
                pooling: EmbeddingPooling::Cls,
                max_tokens: 2048,
                max_batch_tokens: 8192,
                pad_batches: false,
                nice: 19,
            },
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_root: PathBuf::from("/srv/strata"),
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            default_timezone: "UTC".to_owned(),
            database: DatabaseConfig {
                owner_url: "postgres://strata_owner@localhost/strata".to_owned(),
                app_url: "postgres://strata_app@localhost/strata".to_owned(),
                accounts_url: "postgres://strata_accounts@localhost/strata".to_owned(),
                max_connections: 5,
            },
            ai: AiConfig::default(),
            thresholds: Thresholds {
                relation: 0.7,
                custody: 0.85,
                dedupe: DEFAULT_DEDUPE_THRESHOLDS
                    .iter()
                    .map(|&(kind, near, semantic)| {
                        (kind.to_owned(), DedupeThreshold { near, semantic })
                    })
                    .collect(),
            },
            budgets: Budgets {
                timezone: None,
                per_user_daily_tokens: 2_000_000,
                per_user_daily_cost_micros: 0,
                global_daily_tokens: 4_000_000,
                global_daily_cost_micros: 0,
            },
            accounts: AccountsConfig {
                deletion_grace_days: 14,
                max_pending_signups: 20,
                purge_interval_secs: 300,
            },
            push: PushConfig::default(),
            auth: AuthConfig {
                signing_key_file: PathBuf::from("/etc/strata/token-signing-key.pem"),
                issuer: "strata".to_owned(),
                audience: "strata-api".to_owned(),
                access_token_ttl_secs: 900,
                session_ttl_days: 90,
                revocation_reload_secs: 30,
                trust_forwarded_for: false,
                min_password_length: 10,
                argon2: Argon2Config {
                    memory_kib: 19 * 1024,
                    iterations: 2,
                    parallelism: 1,
                },
                rate_limits: RateLimits {
                    login_per_ip: RateLimit {
                        max: 30,
                        window_secs: 900,
                    },
                    login_per_username: RateLimit {
                        max: 10,
                        window_secs: 900,
                    },
                    signup_per_ip: RateLimit {
                        max: 5,
                        window_secs: 3600,
                    },
                    signup_global: RateLimit {
                        max: 30,
                        window_secs: 3600,
                    },
                },
            },
        }
    }
}

impl Config {
    /// Loads `path` (if given) and applies overrides from the process environment.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let text = match path {
            Some(p) => std::fs::read_to_string(p).map_err(|e| ConfigError::Read {
                path: p.to_path_buf(),
                message: e.to_string(),
            })?,
            None => String::new(),
        };
        Self::from_sources(&text, std::env::vars())
    }

    /// Builds a config from TOML text and explicit `(name, value)` environment pairs. Pairs
    /// whose name does not start with [`ENV_PREFIX`] are ignored.
    pub fn from_sources(
        toml_text: &str,
        env: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ConfigError> {
        let mut merged = toml::Table::try_from(Self::default())
            .map_err(|e| ConfigError::Invalid(format!("defaults do not serialise: {e}")))?;
        let file: toml::Table =
            toml::from_str(toml_text).map_err(|e| ConfigError::Parse(e.to_string()))?;
        merge_tables(&mut merged, file);

        let mut overrides: Vec<(String, String)> = env
            .into_iter()
            .filter(|(k, _)| k.starts_with(ENV_PREFIX))
            .collect();
        overrides.sort();
        for (name, value) in overrides {
            apply_env_override(&mut merged, &name, &value)?;
        }

        let config: Self = toml::Value::Table(merged)
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Checks value ranges and cross-field rules.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.default_tz()?;
        let unit = |name: String, v: f64| {
            if (0.0..=1.0).contains(&v) {
                Ok(())
            } else {
                Err(ConfigError::Invalid(format!(
                    "{name} must be within 0..=1, got {v}"
                )))
            }
        };
        unit("thresholds.relation".into(), self.thresholds.relation)?;
        unit("thresholds.custody".into(), self.thresholds.custody)?;
        for (kind, t) in &self.thresholds.dedupe {
            unit(format!("thresholds.dedupe.{kind}.near"), t.near)?;
            unit(format!("thresholds.dedupe.{kind}.semantic"), t.semantic)?;
        }
        if self.database.max_connections == 0 {
            return Err(ConfigError::Invalid(
                "database.max_connections must be at least 1".into(),
            ));
        }
        self.validate_ai()?;
        self.validate_auth()?;
        for (name, url) in [
            ("database.owner_url", &self.database.owner_url),
            ("database.app_url", &self.database.app_url),
            ("database.accounts_url", &self.database.accounts_url),
        ] {
            if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
                return Err(ConfigError::Invalid(format!(
                    "{name} must be a postgres:// URL"
                )));
            }
        }
        Ok(())
    }

    fn validate_ai(&self) -> Result<(), ConfigError> {
        let invalid = |m: String| Err(ConfigError::Invalid(m));
        let positive = |name: &str, v: u64| {
            if v == 0 {
                invalid(format!("{name} must be at least 1"))
            } else {
                Ok(())
            }
        };
        let ai = &self.ai;
        let cli = &ai.claude_cli;
        if cli.command.first().is_none_or(|c| c.trim().is_empty()) {
            return invalid("ai.claude_cli.command must name a program".into());
        }
        if !cli.scratch_dir.is_absolute() {
            return invalid("ai.claude_cli.scratch_dir must be an absolute path".into());
        }
        positive(
            "ai.claude_cli.max_concurrency",
            u64::from(cli.max_concurrency),
        )?;
        positive("ai.claude_cli.timeout_secs", u64::from(cli.timeout_secs))?;
        positive(
            "ai.claude_cli.usage_limit_pause_secs",
            u64::from(cli.usage_limit_pause_secs),
        )?;
        let api = &ai.anthropic_api;
        if api.model.trim().is_empty() {
            return invalid("ai.anthropic_api.model must not be empty".into());
        }
        if !api.base_url.starts_with("https://") && !api.base_url.starts_with("http://")
            || api.base_url.ends_with('/')
        {
            return invalid(
                "ai.anthropic_api.base_url must be an http(s) URL without a trailing slash".into(),
            );
        }
        if let Some(e) = &api.effort
            && !ANTHROPIC_EFFORTS.contains(&e.as_str())
        {
            return invalid(format!(
                "ai.anthropic_api.effort must be one of {}, got `{e}`",
                ANTHROPIC_EFFORTS.join(", ")
            ));
        }
        if api.max_retries > 10 {
            return invalid("ai.anthropic_api.max_retries must be at most 10".into());
        }
        positive("ai.anthropic_api.timeout_secs", u64::from(api.timeout_secs))?;
        if api.api_key_file.is_none() {
            if ai.default_provider == AiProviderKind::AnthropicApi {
                return invalid(
                    "ai.anthropic_api.api_key_file is required: ai.default_provider is anthropic_api"
                        .into(),
                );
            }
            if let Some((user, _)) = ai
                .user_providers
                .iter()
                .find(|(_, k)| **k == AiProviderKind::AnthropicApi)
            {
                return invalid(format!(
                    "ai.anthropic_api.api_key_file is required: user `{user}` is routed to anthropic_api"
                ));
            }
        }
        if ai.user_providers.keys().any(|u| u.trim().is_empty()) {
            return invalid("ai.user_providers keys must be usernames".into());
        }
        let emb = &ai.embedding;
        if emb.model_id.trim().is_empty() {
            return invalid("ai.embedding.model_id must not be empty".into());
        }
        positive("ai.embedding.dims", u64::from(emb.dims))?;
        positive("ai.embedding.max_tokens", u64::from(emb.max_tokens))?;
        if emb.max_batch_tokens < emb.max_tokens {
            return invalid(
                "ai.embedding.max_batch_tokens must be at least ai.embedding.max_tokens".into(),
            );
        }
        if !(0..=19).contains(&emb.nice) {
            return invalid(format!(
                "ai.embedding.nice must be within 0..=19, got {}",
                emb.nice
            ));
        }
        self.budget_tz()?;
        Ok(())
    }

    fn validate_auth(&self) -> Result<(), ConfigError> {
        let auth = &self.auth;
        let positive = |name: &str, v: u32| {
            if v == 0 {
                Err(ConfigError::Invalid(format!("{name} must be at least 1")))
            } else {
                Ok(())
            }
        };
        positive("auth.access_token_ttl_secs", auth.access_token_ttl_secs)?;
        positive("auth.session_ttl_days", auth.session_ttl_days)?;
        positive("auth.revocation_reload_secs", auth.revocation_reload_secs)?;
        positive("auth.min_password_length", auth.min_password_length)?;
        positive(
            "accounts.purge_interval_secs",
            self.accounts.purge_interval_secs,
        )?;
        positive("auth.argon2.iterations", auth.argon2.iterations)?;
        positive("auth.argon2.parallelism", auth.argon2.parallelism)?;
        if auth.argon2.memory_kib < 8 * auth.argon2.parallelism {
            return Err(ConfigError::Invalid(
                "auth.argon2.memory_kib must be at least 8 × parallelism".into(),
            ));
        }
        for (name, limit) in [
            ("login_per_ip", auth.rate_limits.login_per_ip),
            ("login_per_username", auth.rate_limits.login_per_username),
            ("signup_per_ip", auth.rate_limits.signup_per_ip),
            ("signup_global", auth.rate_limits.signup_global),
        ] {
            positive(&format!("auth.rate_limits.{name}.max"), limit.max)?;
            positive(
                &format!("auth.rate_limits.{name}.window_secs"),
                limit.window_secs,
            )?;
        }
        if auth.issuer.is_empty() || auth.audience.is_empty() {
            return Err(ConfigError::Invalid(
                "auth.issuer and auth.audience must not be empty".into(),
            ));
        }
        Ok(())
    }

    /// The parsed default timezone.
    pub fn default_tz(&self) -> Result<Tz, ConfigError> {
        Tz::from_str(&self.default_timezone).map_err(|_| {
            ConfigError::Invalid(format!(
                "default_timezone `{}` is not an IANA timezone",
                self.default_timezone
            ))
        })
    }

    /// The timezone whose days the AI budgets count (`budgets.timezone`, else
    /// `default_timezone`).
    pub fn budget_tz(&self) -> Result<Tz, ConfigError> {
        match &self.budgets.timezone {
            None => self.default_tz(),
            Some(name) => Tz::from_str(name).map_err(|_| {
                ConfigError::Invalid(format!("budgets.timezone `{name}` is not an IANA timezone"))
            }),
        }
    }

    /// Whether any user is routed to `kind` (the default or an override).
    pub fn ai_provider_in_use(&self, kind: AiProviderKind) -> bool {
        self.ai.default_provider == kind || self.ai.user_providers.values().any(|k| *k == kind)
    }

    /// The account-deletion grace period as a duration.
    pub fn deletion_grace_period(&self) -> chrono::Duration {
        chrono::Duration::days(i64::from(self.accounts.deletion_grace_days))
    }

    /// The provider serving `username` (per-user override, else the default).
    pub fn ai_provider_for(&self, username: &str) -> AiProviderKind {
        self.ai
            .user_providers
            .get(username)
            .copied()
            .unwrap_or(self.ai.default_provider)
    }

    /// Duplicate thresholds for `kind`, if configured.
    pub fn dedupe_threshold(&self, kind: &str) -> Option<DedupeThreshold> {
        self.thresholds.dedupe.get(kind).copied()
    }
}

fn merge_tables(base: &mut toml::Table, overlay: toml::Table) {
    for (key, value) in overlay {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                merge_tables(existing, incoming);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn apply_env_override(root: &mut toml::Table, name: &str, raw: &str) -> Result<(), ConfigError> {
    let path: Vec<String> = name[ENV_PREFIX.len()..]
        .split("__")
        .map(str::to_ascii_lowercase)
        .collect();
    if path.iter().any(String::is_empty) {
        return Err(ConfigError::Env {
            name: name.to_owned(),
            message: "empty path segment".into(),
        });
    }
    let (leaf, parents) = path.split_last().ok_or_else(|| ConfigError::Env {
        name: name.to_owned(),
        message: "no key".into(),
    })?;
    let mut table = root;
    for segment in parents {
        let entry = table
            .entry(segment.clone())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        table = match entry {
            toml::Value::Table(t) => t,
            _ => {
                return Err(ConfigError::Env {
                    name: name.to_owned(),
                    message: format!("`{segment}` is not a section"),
                });
            }
        };
    }
    let bad = |what: &str| ConfigError::Env {
        name: name.to_owned(),
        message: format!("expected {what}"),
    };
    let value = match table.get(leaf.as_str()) {
        Some(toml::Value::String(_)) => toml::Value::String(raw.to_owned()),
        Some(toml::Value::Integer(_)) => {
            toml::Value::Integer(raw.trim().parse().map_err(|_| bad("an integer"))?)
        }
        Some(toml::Value::Float(_)) => {
            toml::Value::Float(raw.trim().parse().map_err(|_| bad("a number"))?)
        }
        Some(toml::Value::Boolean(_)) => {
            toml::Value::Boolean(raw.trim().parse().map_err(|_| bad("true or false"))?)
        }
        Some(toml::Value::Array(_)) => {
            let parsed: toml::Table = toml::from_str(&format!("v = {raw}"))
                .map_err(|_| bad("a TOML array, e.g. [\"a\", \"b\"]"))?;
            match parsed.get("v") {
                Some(v @ toml::Value::Array(_)) => v.clone(),
                _ => return Err(bad("a TOML array, e.g. [\"a\", \"b\"]")),
            }
        }
        Some(toml::Value::Table(_)) => return Err(bad("a section path, not a value")),
        Some(_) => return Err(bad("a scalar key")),
        None => infer_scalar(raw),
    };
    table.insert(leaf.clone(), value);
    Ok(())
}

fn infer_scalar(raw: &str) -> toml::Value {
    if let Ok(b) = raw.parse::<bool>() {
        toml::Value::Boolean(b)
    } else if let Ok(i) = raw.parse::<i64>() {
        toml::Value::Integer(i)
    } else if let Ok(f) = raw.parse::<f64>() {
        toml::Value::Float(f)
    } else {
        toml::Value::String(raw.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn empty_sources_yield_defaults() {
        let config = Config::from_sources("", env(&[])).expect("defaults are valid");
        assert_eq!(config, Config::default());
        assert_eq!(config.deletion_grace_period(), chrono::Duration::days(14));
        assert_eq!(config.default_tz().expect("UTC"), Tz::UTC);
        assert_eq!(
            config.dedupe_threshold("task"),
            Some(DedupeThreshold {
                near: 0.6,
                semantic: 0.88
            })
        );
    }

    #[test]
    fn file_values_override_defaults_and_keep_the_rest() {
        let text = r#"
            data_root = "/data/strata"
            default_timezone = "Africa/Cairo"
            [database]
            app_url = "postgres://strata_app:pw@db/strata"
            [ai]
            default_provider = "disabled"
            user_providers = { owner = "claude_cli" }
            [thresholds.dedupe.task]
            near = 0.5
            semantic = 0.8
            [push]
            apns_key_id = "ABC123"
        "#;
        let config = Config::from_sources(text, env(&[])).expect("valid");
        let mut expected = Config::default();
        expected.data_root = PathBuf::from("/data/strata");
        expected.default_timezone = "Africa/Cairo".into();
        expected.database.app_url = "postgres://strata_app:pw@db/strata".into();
        expected.ai.default_provider = AiProviderKind::Disabled;
        expected
            .ai
            .user_providers
            .insert("owner".into(), AiProviderKind::ClaudeCli);
        expected.thresholds.dedupe.insert(
            "task".into(),
            DedupeThreshold {
                near: 0.5,
                semantic: 0.8,
            },
        );
        expected.push.apns_key_id = Some("ABC123".into());
        assert_eq!(config, expected);
        assert_eq!(
            config.default_tz().expect("valid tz"),
            chrono_tz::Africa::Cairo
        );
        assert_eq!(config.ai_provider_for("owner"), AiProviderKind::ClaudeCli);
        assert_eq!(config.ai_provider_for("guest"), AiProviderKind::Disabled);
    }

    #[test]
    fn env_overrides_win_and_are_typed_by_the_replaced_key() {
        let config = Config::from_sources(
            "[accounts]\ndeletion_grace_days = 30\n",
            env(&[
                ("STRATA__ACCOUNTS__DELETION_GRACE_DAYS", "7"),
                ("STRATA__DATABASE__MAX_CONNECTIONS", "3"),
                ("STRATA__THRESHOLDS__RELATION", "0.75"),
                ("STRATA__BIND", "0.0.0.0:9000"),
                ("STRATA__THRESHOLDS__DEDUPE__NOTE__NEAR", "0.65"),
                ("UNRELATED", "ignored"),
                ("STRATA_TEST_DATABASE_URL", "ignored too"),
            ]),
        )
        .expect("valid");
        assert_eq!(config.accounts.deletion_grace_days, 7);
        assert_eq!(config.database.max_connections, 3);
        assert_eq!(config.thresholds.relation, 0.75);
        assert_eq!(
            config.bind,
            "0.0.0.0:9000".parse::<SocketAddr>().expect("addr")
        );
        assert_eq!(config.thresholds.dedupe["note"].near, 0.65);
        assert_eq!(config.thresholds.dedupe["note"].semantic, 0.9);
    }

    #[test]
    fn auth_settings_have_documented_defaults_and_overrides() {
        let config = Config::from_sources(
            "[auth]\nsigning_key_file = \"/k.pem\"\n[auth.rate_limits.login_per_ip]\nmax = 3\nwindow_secs = 60\n",
            env(&[("STRATA__AUTH__ACCESS_TOKEN_TTL_SECS", "600")]),
        )
        .expect("valid");
        assert_eq!(config.auth.signing_key_file, PathBuf::from("/k.pem"));
        assert_eq!(config.auth.access_token_ttl_secs, 600);
        assert_eq!(
            config.auth.rate_limits.login_per_ip,
            RateLimit {
                max: 3,
                window_secs: 60
            }
        );
        assert_eq!(config.auth.rate_limits.login_per_username.max, 10);
        assert_eq!(
            config.auth.argon2,
            Argon2Config {
                memory_kib: 19_456,
                iterations: 2,
                parallelism: 1
            }
        );
        assert_eq!(config.accounts.purge_interval_secs, 300);
    }

    /// Keys that are unset by default have no type to guide parsing, so a numeric-looking value
    /// becomes an integer and fails to deserialise into `Option<String>`; such values belong in
    /// the TOML file (quoted). The failure is loud, never a silent misconfiguration.
    #[test]
    fn numeric_env_value_for_unset_string_key_is_reported() {
        let err = Config::from_sources("", env(&[("STRATA__PUSH__APNS_TEAM_ID", "1234567890")]))
            .expect_err("integer into Option<String>");
        assert!(matches!(err, ConfigError::Parse(_)), "{err:?}");
        let ok = Config::from_sources("", env(&[("STRATA__PUSH__APNS_TEAM_ID", "TEAM42")]))
            .expect("string");
        assert_eq!(ok.push.apns_team_id.as_deref(), Some("TEAM42"));
    }

    #[test]
    fn unknown_keys_and_bad_values_are_errors() {
        assert!(matches!(
            Config::from_sources("databse = 1", env(&[])),
            Err(ConfigError::Parse(_))
        ));
        assert!(matches!(
            Config::from_sources("", env(&[("STRATA__DATABASE__TYPO", "x")])),
            Err(ConfigError::Parse(_))
        ));
        assert_eq!(
            Config::from_sources("", env(&[("STRATA__DATABASE__MAX_CONNECTIONS", "many")])),
            Err(ConfigError::Env {
                name: "STRATA__DATABASE__MAX_CONNECTIONS".into(),
                message: "expected an integer".into()
            })
        );
        assert_eq!(
            Config::from_sources("default_timezone = \"Mars/Base\"", env(&[])),
            Err(ConfigError::Invalid(
                "default_timezone `Mars/Base` is not an IANA timezone".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[thresholds]\ncustody = 1.5", env(&[])),
            Err(ConfigError::Invalid(
                "thresholds.custody must be within 0..=1, got 1.5".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[auth]\naccess_token_ttl_secs = 0", env(&[])),
            Err(ConfigError::Invalid(
                "auth.access_token_ttl_secs must be at least 1".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[auth.rate_limits.signup_global]\nmax = 0", env(&[])),
            Err(ConfigError::Invalid(
                "auth.rate_limits.signup_global.max must be at least 1".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[auth.argon2]\nmemory_kib = 7", env(&[])),
            Err(ConfigError::Invalid(
                "auth.argon2.memory_kib must be at least 8 × parallelism".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[auth]\nissuer = \"\"", env(&[])),
            Err(ConfigError::Invalid(
                "auth.issuer and auth.audience must not be empty".into()
            ))
        );
        assert_eq!(
            Config::from_sources("[database]\napp_url = \"mysql://x\"", env(&[])),
            Err(ConfigError::Invalid(
                "database.app_url must be a postgres:// URL".into()
            ))
        );
    }

    #[test]
    fn ai_settings_have_documented_defaults() {
        let config = Config::from_sources("", env(&[])).expect("defaults");
        let ai = &config.ai;
        assert_eq!(ai.default_provider, AiProviderKind::ClaudeCli);
        assert_eq!(ai.user_providers, BTreeMap::new());
        assert_eq!(
            ai.claude_cli,
            ClaudeCliSettings {
                command: vec!["/usr/local/bin/claude".into()],
                scratch_dir: PathBuf::from("/var/lib/strata-ai/scratch"),
                model: None,
                max_concurrency: 1,
                timeout_secs: 300,
                kill_grace_secs: 5,
                usage_limit_pause_secs: 1800,
            }
        );
        assert_eq!(
            ai.anthropic_api,
            AnthropicApiSettings {
                api_key_file: None,
                model: "claude-opus-5-5".into(),
                base_url: "https://api.anthropic.com".into(),
                effort: None,
                max_retries: 3,
                timeout_secs: 600,
                input_micros_per_mtok: 4_000_000,
                output_micros_per_mtok: 20_000_000,
            }
        );
        assert_eq!(
            (
                ai.embedding.model_dir.as_ref(),
                ai.embedding.onnxruntime_lib.as_ref(),
                ai.embedding.pooling,
                ai.embedding.max_tokens,
                ai.embedding.dims,
                ai.embedding.nice
            ),
            (None, None, EmbeddingPooling::Cls, 2048, 384, 19)
        );
        assert_eq!(
            config.budgets,
            Budgets {
                timezone: None,
                per_user_daily_tokens: 2_000_000,
                per_user_daily_cost_micros: 0,
                global_daily_tokens: 4_000_000,
                global_daily_cost_micros: 0,
            }
        );
        assert_eq!(config.budget_tz().expect("tz"), Tz::UTC);
        // D23: claude -p serves every account unless configured otherwise.
        assert_eq!(config.ai_provider_for("anyone"), AiProviderKind::ClaudeCli);
        assert!(config.ai_provider_in_use(AiProviderKind::ClaudeCli));
        assert!(!config.ai_provider_in_use(AiProviderKind::AnthropicApi));
    }

    #[test]
    fn ai_settings_from_file_and_environment() {
        let text = r#"
            default_timezone = "Africa/Cairo"
            [ai]
            user_providers = { guest = "anthropic_api", kid = "disabled" }
            [ai.claude_cli]
            command = ["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]
            model = "opus"
            [ai.anthropic_api]
            api_key_file = "/etc/strata/anthropic.key"
            effort = "high"
            [ai.embedding]
            model_dir = "/opt/models/granite"
            onnxruntime_lib = "/opt/onnxruntime/lib/libonnxruntime.so.1.30.0"
            pooling = "mean"
            [budgets]
            timezone = "Europe/Berlin"
            global_daily_cost_micros = 5000000
        "#;
        let config = Config::from_sources(
            text,
            env(&[
                ("STRATA__AI__CLAUDE_CLI__COMMAND", r#"["/usr/bin/claude"]"#),
                ("STRATA__AI__CLAUDE_CLI__TIMEOUT_SECS", "120"),
                ("STRATA__AI__CLAUDE_CLI__SCRATCH_DIR", "/tmp/scratch"),
                ("STRATA__AI__ANTHROPIC_API__MAX_RETRIES", "5"),
                ("STRATA__AI__ANTHROPIC_API__MODEL", "claude-opus-5"),
                ("STRATA__AI__USER_PROVIDERS__OWNER", "claude_cli"),
                ("STRATA__AI__EMBEDDING__MAX_TOKENS", "1024"),
                ("STRATA__BUDGETS__PER_USER_DAILY_COST_MICROS", "250000"),
            ]),
        )
        .expect("valid");
        let mut expected = Config::default();
        expected.default_timezone = "Africa/Cairo".into();
        expected.ai.user_providers = [
            ("guest".to_owned(), AiProviderKind::AnthropicApi),
            ("kid".to_owned(), AiProviderKind::Disabled),
            ("owner".to_owned(), AiProviderKind::ClaudeCli),
        ]
        .into_iter()
        .collect();
        expected.ai.claude_cli.command = vec!["/usr/bin/claude".into()];
        expected.ai.claude_cli.model = Some("opus".into());
        expected.ai.claude_cli.timeout_secs = 120;
        expected.ai.claude_cli.scratch_dir = PathBuf::from("/tmp/scratch");
        expected.ai.anthropic_api.api_key_file = Some("/etc/strata/anthropic.key".into());
        expected.ai.anthropic_api.effort = Some("high".into());
        expected.ai.anthropic_api.max_retries = 5;
        expected.ai.anthropic_api.model = "claude-opus-5".into();
        expected.ai.embedding.model_dir = Some("/opt/models/granite".into());
        expected.ai.embedding.onnxruntime_lib =
            Some("/opt/onnxruntime/lib/libonnxruntime.so.1.30.0".into());
        expected.ai.embedding.pooling = EmbeddingPooling::Mean;
        expected.ai.embedding.max_tokens = 1024;
        expected.budgets.timezone = Some("Europe/Berlin".into());
        expected.budgets.global_daily_cost_micros = 5_000_000;
        expected.budgets.per_user_daily_cost_micros = 250_000;
        assert_eq!(config, expected);
        assert_eq!(config.budget_tz().expect("tz"), chrono_tz::Europe::Berlin);
        assert_eq!(
            config.ai_provider_for("guest"),
            AiProviderKind::AnthropicApi
        );
        assert_eq!(config.ai_provider_for("kid"), AiProviderKind::Disabled);
        assert_eq!(config.ai_provider_for("owner"), AiProviderKind::ClaudeCli);
        assert_eq!(config.ai_provider_for("other"), AiProviderKind::ClaudeCli);
        assert!(config.ai_provider_in_use(AiProviderKind::AnthropicApi));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // one assertion per rule
    fn ai_settings_are_validated() {
        let invalid = |text: &str, message: &str| {
            assert_eq!(
                Config::from_sources(text, env(&[])),
                Err(ConfigError::Invalid(message.into())),
                "{text}"
            );
        };
        invalid(
            "[ai.claude_cli]\ncommand = []",
            "ai.claude_cli.command must name a program",
        );
        invalid(
            "[ai.claude_cli]\ncommand = [\" \"]",
            "ai.claude_cli.command must name a program",
        );
        invalid(
            "[ai.claude_cli]\nscratch_dir = \"scratch\"",
            "ai.claude_cli.scratch_dir must be an absolute path",
        );
        invalid(
            "[ai.claude_cli]\nmax_concurrency = 0",
            "ai.claude_cli.max_concurrency must be at least 1",
        );
        invalid(
            "[ai.claude_cli]\ntimeout_secs = 0",
            "ai.claude_cli.timeout_secs must be at least 1",
        );
        invalid(
            "[ai.anthropic_api]\nmodel = \"\"",
            "ai.anthropic_api.model must not be empty",
        );
        invalid(
            "[ai.anthropic_api]\nbase_url = \"https://api.anthropic.com/\"",
            "ai.anthropic_api.base_url must be an http(s) URL without a trailing slash",
        );
        invalid(
            "[ai.anthropic_api]\neffort = \"extreme\"",
            "ai.anthropic_api.effort must be one of low, medium, high, xhigh, max, got `extreme`",
        );
        invalid(
            "[ai.anthropic_api]\nmax_retries = 11",
            "ai.anthropic_api.max_retries must be at most 10",
        );
        invalid(
            "[ai]\ndefault_provider = \"anthropic_api\"",
            "ai.anthropic_api.api_key_file is required: ai.default_provider is anthropic_api",
        );
        invalid(
            "[ai.user_providers]\nguest = \"anthropic_api\"",
            "ai.anthropic_api.api_key_file is required: user `guest` is routed to anthropic_api",
        );
        invalid(
            "[ai.embedding]\nmax_tokens = 9000",
            "ai.embedding.max_batch_tokens must be at least ai.embedding.max_tokens",
        );
        invalid(
            "[ai.embedding]\nnice = 20",
            "ai.embedding.nice must be within 0..=19, got 20",
        );
        invalid(
            "[ai.embedding]\ndims = 0",
            "ai.embedding.dims must be at least 1",
        );
        invalid(
            "[budgets]\ntimezone = \"Mars/Base\"",
            "budgets.timezone `Mars/Base` is not an IANA timezone",
        );
        // Unknown keys and wrong types are rejected, in the file and the environment.
        assert!(matches!(
            Config::from_sources("[ai]\nclaude_bin = \"/usr/bin/claude\"", env(&[])),
            Err(ConfigError::Parse(_))
        ));
        assert!(matches!(
            Config::from_sources("[ai.embedding]\npooling = \"max\"", env(&[])),
            Err(ConfigError::Parse(_))
        ));
        assert!(matches!(
            Config::from_sources("", env(&[("STRATA__AI__CLAUDE_CLI__TYPO", "1")])),
            Err(ConfigError::Parse(_))
        ));
        assert!(matches!(
            Config::from_sources("", env(&[("STRATA__AI__USER_PROVIDERS__BOB", "gpt")])),
            Err(ConfigError::Parse(_))
        ));
        for raw in ["/usr/bin/claude", "[1, 2", "\"x\""] {
            assert_eq!(
                Config::from_sources("", env(&[("STRATA__AI__CLAUDE_CLI__COMMAND", raw)])),
                Err(ConfigError::Env {
                    name: "STRATA__AI__CLAUDE_CLI__COMMAND".into(),
                    message: "expected a TOML array, e.g. [\"a\", \"b\"]".into()
                }),
                "{raw}"
            );
        }
        let with_key = Config::from_sources(
            "[ai]\ndefault_provider = \"anthropic_api\"\n[ai.anthropic_api]\napi_key_file = \"/k\"",
            env(&[]),
        )
        .expect("key given");
        assert_eq!(
            with_key.ai_provider_for("anyone"),
            AiProviderKind::AnthropicApi
        );
    }

    /// `deploy/stratad.example.toml` loads to the defaults and documents every key, set or
    /// commented out, under its section.
    #[test]
    fn example_file_documents_every_key_with_its_default() {
        fn leaves(prefix: &str, table: &toml::Table, out: &mut Vec<(String, String)>) {
            for (key, value) in table {
                match value {
                    toml::Value::Table(t) => {
                        let p = if prefix.is_empty() {
                            key.clone()
                        } else {
                            format!("{prefix}.{key}")
                        };
                        leaves(&p, t, out);
                    }
                    _ => out.push((prefix.to_owned(), key.clone())),
                }
            }
        }
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../deploy/stratad.example.toml"
        );
        let text = std::fs::read_to_string(path).expect("example file");
        assert_eq!(
            Config::from_sources(&text, env(&[])).expect("example is valid"),
            Config::default()
        );
        // (section, key) of every line `key = …` or `# key = …`.
        let mut documented = std::collections::BTreeSet::new();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.to_owned();
            } else if let Some((key, _)) = line.trim_start_matches("# ").split_once(" = ")
                && !key.contains(' ')
            {
                documented.insert((section.clone(), key.to_owned()));
            }
        }
        let mut expected = Vec::new();
        leaves(
            "",
            &toml::Table::try_from(Config::default()).expect("serialise"),
            &mut expected,
        );
        // Keys that are unset by default (not serialised).
        for (section, key) in [
            ("ai.claude_cli", "model"),
            ("ai.anthropic_api", "api_key_file"),
            ("ai.anthropic_api", "effort"),
            ("ai.embedding", "model_dir"),
            ("ai.embedding", "onnxruntime_lib"),
            ("budgets", "timezone"),
            ("push", "fcm_service_account_path"),
            ("push", "apns_key_path"),
            ("push", "apns_key_id"),
            ("push", "apns_team_id"),
            ("push", "apns_topic"),
            ("push", "wns_credentials_path"),
        ] {
            expected.push((section.to_owned(), key.to_owned()));
        }
        let missing: Vec<_> = expected
            .into_iter()
            .filter(|k| !documented.contains(k))
            .collect();
        assert_eq!(missing, Vec::<(String, String)>::new());
    }

    #[test]
    fn load_reads_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stratad.toml");
        std::fs::write(&path, "bind = \"127.0.0.1:7000\"\n").expect("write");
        let config = Config::load(Some(&path)).expect("valid");
        assert_eq!(config.bind.port(), 7000);
        let missing = Config::load(Some(&dir.path().join("missing.toml"))).expect_err("missing");
        assert!(matches!(missing, ConfigError::Read { .. }), "{missing:?}");
    }
}
