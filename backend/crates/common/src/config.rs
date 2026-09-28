//! Server configuration from environment variables and a `.env` file (PLAN §14 "Config").
//!
//! Every setting is one `STRATA_` variable: the key path in [`Config`] with its segments joined
//! by double underscores, upper-cased (`bind` → `STRATA_BIND`, `database.app_url` →
//! `STRATA_DATABASE__APP_URL`, `thresholds.dedupe.task.near` →
//! `STRATA_THRESHOLDS__DEDUPE__TASK__NEAR`). Layering, lowest to highest precedence: built-in
//! defaults → the env file → the process environment. Loading, value syntax and error rules
//! are in [`env`]; `deploy/stratad.env.example` lists every variable with its default.

pub mod env;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// The complete server configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Root of per-user data (`users/<user_id>/vault`, PLAN §5.2).
    pub data_root: PathBuf,
    /// Address the API listens on (behind nginx).
    pub bind: SocketAddr,
    /// IANA timezone used for users who have not set one (tasks, reminders, digests). Times
    /// are stored and written in UTC; this only anchors dates and display.
    pub default_timezone: String,
    /// How far (seconds) a device's `created` time on a create may be ahead of the server's
    /// clock before the create is refused (`422 created_in_future`).
    pub max_future_skew_secs: u32,
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
    /// Background job runner (PLAN §5.2 fair scheduling, §9.2).
    pub jobs: JobsConfig,
}

/// The background job runner (PLAN §5.2, §9.2): fair round-robin across users, a global
/// concurrency limit within the VPS budget, retries with exponential backoff, and the
/// nightly/weekly scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobsConfig {
    /// Jobs running at once across all users (1 CPU core: keep it low). Embedding jobs are
    /// additionally limited to one at a time and LLM jobs by `ai.claude_cli.max_concurrency`.
    pub max_concurrency: u32,
    /// Seconds between scheduling passes when nothing is runnable.
    pub poll_interval_secs: u32,
    /// Seconds a user whose queued jobs are all of kinds this process does not run is skipped
    /// before being looked at again.
    pub idle_recheck_secs: u32,
    /// First retry delay in seconds; doubles per attempt.
    pub backoff_base_secs: u32,
    /// Longest retry delay in seconds.
    pub backoff_max_secs: u32,
    /// Local hour (0–23, in `default_timezone`) at which nightly jobs (`dedupe`) run.
    pub nightly_hour: u32,
    /// Day of the weekly AI digest (`mon` … `sun`), at `nightly_hour` (PLAN §9.2).
    pub digest_weekday: String,
    /// Seconds a graceful shutdown waits for running jobs (unfinished ones are re-queued at
    /// the next start).
    pub shutdown_grace_secs: u32,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 2,
            poll_interval_secs: 5,
            idle_recheck_secs: 60,
            backoff_base_secs: 30,
            backoff_max_secs: 3600,
            nightly_hour: 3,
            digest_weekday: "mon".to_owned(),
            shutdown_grace_secs: 30,
        }
    }
}

impl JobsConfig {
    /// The digest day (validated at load; Monday if unparsable).
    pub fn digest_day(&self) -> chrono::Weekday {
        self.digest_weekday.parse().unwrap_or(chrono::Weekday::Mon)
    }
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
    /// Directory `command` is started in instead of `scratch_dir`: set it (production: `/`)
    /// when `command` is a launcher whose wrapper changes into the scratch directory as
    /// `strata-ai`, which `stratad` itself cannot enter (RUNBOOK §9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_dir: Option<PathBuf>,
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
    /// Seconds without an embedding call after which the model is unloaded to free its memory
    /// (≈ 390 MB for the fp32 export); it is loaded again on the next call (§9.1b).
    pub idle_unload_secs: u32,
}

/// Effort levels accepted by the Messages API.
pub const ANTHROPIC_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Per-kind duplicate-detection thresholds (§9.7). The defaults are the tuned
/// `domain::DedupeThresholds::default_for` values ([`DEFAULT_DEDUPE_THRESHOLDS`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DedupeThreshold {
    /// Trigram similarity (0..1) at or above which an item is a near duplicate.
    pub near: f64,
    /// Embedding cosine similarity (0..1) at or above which an item is a semantic duplicate
    /// candidate (borderline scores are confirmed by one LLM call). Absent for kinds without
    /// a semantic level (aliases: single names, where embeddings add nothing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<f64>,
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
    /// Login, signup, capture and ask rate limits (§8, §15).
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

/// Rate limits for the account endpoints (per client/username) and for capture and ask (per
/// user).
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
    /// `POST /capture` requests per user (§8: capture/ask limits are per user).
    pub capture_per_user: RateLimit,
    /// `POST /ask` requests per user.
    pub ask_per_user: RateLimit,
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

/// Item kinds with duplicate thresholds by default, and their defaults `(near, semantic)`:
/// the calibrated `domain::DedupeThresholds::default_for` values (near; semantic = the
/// candidate level), so an unchanged configuration behaves exactly as tested (a unit test
/// keeps the two equal).
pub const DEFAULT_DEDUPE_THRESHOLDS: &[(&str, f64, Option<f64>)] = &[
    ("alias", 0.6, None),
    ("capture", 0.6, Some(0.9)),
    ("company", 0.5, Some(0.9)),
    ("concept", 0.5, Some(0.88)),
    ("document", 0.8, Some(0.92)),
    ("note", 0.6, Some(0.9)),
    ("person", 0.5, Some(0.9)),
    ("place", 0.8, Some(0.92)),
    ("task", 0.6, Some(0.88)),
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
                launch_dir: None,
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
                model_file: PathBuf::from("onnx/model.onnx"),
                tokenizer_file: PathBuf::from("tokenizer.json"),
                model_id: "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model".to_owned(),
                dims: 384,
                pooling: EmbeddingPooling::Cls,
                max_tokens: 2048,
                max_batch_tokens: 8192,
                pad_batches: true,
                nice: 19,
                idle_unload_secs: 300,
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
            max_future_skew_secs: 300,
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
                    capture_per_user: RateLimit {
                        max: 60,
                        window_secs: 60,
                    },
                    ask_per_user: RateLimit {
                        max: 20,
                        window_secs: 60,
                    },
                },
            },
            jobs: JobsConfig::default(),
        }
    }
}

impl Config {
    /// Loads the configuration: `env_file` (else `.env` in the working directory, if present),
    /// overridden by the process environment, then validated (see [`env`]).
    pub fn load(env_file: Option<&Path>) -> Result<Self, ConfigError> {
        env::load(env_file)
    }

    /// Builds a config from the variables of an env file and the process environment
    /// (`(name, value)` pairs; the environment wins), then validates it (see [`env`]).
    pub fn from_vars(
        file: &env::EnvFile,
        environment: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ConfigError> {
        env::from_vars(file, environment)
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
        unit(
            "STRATA_THRESHOLDS__RELATION".into(),
            self.thresholds.relation,
        )?;
        unit("STRATA_THRESHOLDS__CUSTODY".into(), self.thresholds.custody)?;
        for (kind, t) in &self.thresholds.dedupe {
            let kind = kind.to_ascii_uppercase();
            unit(format!("STRATA_THRESHOLDS__DEDUPE__{kind}__NEAR"), t.near)?;
            if let Some(semantic) = t.semantic {
                unit(
                    format!("STRATA_THRESHOLDS__DEDUPE__{kind}__SEMANTIC"),
                    semantic,
                )?;
            }
        }
        if self.database.max_connections == 0 {
            return Err(ConfigError::Invalid(
                "STRATA_DATABASE__MAX_CONNECTIONS must be at least 1".into(),
            ));
        }
        self.validate_ai()?;
        self.validate_auth()?;
        for (name, url) in [
            ("STRATA_DATABASE__OWNER_URL", &self.database.owner_url),
            ("STRATA_DATABASE__APP_URL", &self.database.app_url),
            ("STRATA_DATABASE__ACCOUNTS_URL", &self.database.accounts_url),
        ] {
            if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
                return Err(ConfigError::Invalid(format!(
                    "{name} must be a postgres:// URL"
                )));
            }
        }
        Ok(())
    }

    fn validate_claude_cli(&self) -> Result<(), ConfigError> {
        let invalid = |m: String| Err(ConfigError::Invalid(m));
        let positive = |name: &str, v: u64| {
            if v == 0 {
                invalid(format!("{name} must be at least 1"))
            } else {
                Ok(())
            }
        };
        let cli = &self.ai.claude_cli;
        if cli.command.first().is_none_or(|c| c.trim().is_empty()) {
            return invalid("STRATA_AI__CLAUDE_CLI__COMMAND must name a program".into());
        }
        if !cli.scratch_dir.is_absolute() {
            return invalid("STRATA_AI__CLAUDE_CLI__SCRATCH_DIR must be an absolute path".into());
        }
        if cli.launch_dir.as_ref().is_some_and(|d| !d.is_absolute()) {
            return invalid("STRATA_AI__CLAUDE_CLI__LAUNCH_DIR must be an absolute path".into());
        }
        positive(
            "STRATA_AI__CLAUDE_CLI__MAX_CONCURRENCY",
            u64::from(cli.max_concurrency),
        )?;
        positive(
            "STRATA_AI__CLAUDE_CLI__TIMEOUT_SECS",
            u64::from(cli.timeout_secs),
        )?;
        positive(
            "STRATA_AI__CLAUDE_CLI__USAGE_LIMIT_PAUSE_SECS",
            u64::from(cli.usage_limit_pause_secs),
        )?;
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
        self.validate_claude_cli()?;
        let api = &ai.anthropic_api;
        if api.model.trim().is_empty() {
            return invalid("STRATA_AI__ANTHROPIC_API__MODEL must not be empty".into());
        }
        if !api.base_url.starts_with("https://") && !api.base_url.starts_with("http://")
            || api.base_url.ends_with('/')
        {
            return invalid(
                "STRATA_AI__ANTHROPIC_API__BASE_URL must be an http(s) URL without a trailing slash".into(),
            );
        }
        if let Some(e) = &api.effort
            && !ANTHROPIC_EFFORTS.contains(&e.as_str())
        {
            return invalid(format!(
                "STRATA_AI__ANTHROPIC_API__EFFORT must be one of {}, got `{e}`",
                ANTHROPIC_EFFORTS.join(", ")
            ));
        }
        if api.max_retries > 10 {
            return invalid("STRATA_AI__ANTHROPIC_API__MAX_RETRIES must be at most 10".into());
        }
        positive(
            "STRATA_AI__ANTHROPIC_API__TIMEOUT_SECS",
            u64::from(api.timeout_secs),
        )?;
        if api.api_key_file.is_none() {
            if ai.default_provider == AiProviderKind::AnthropicApi {
                return invalid(
                    "STRATA_AI__ANTHROPIC_API__API_KEY_FILE is required: STRATA_AI__DEFAULT_PROVIDER is anthropic_api"
                        .into(),
                );
            }
            if let Some((user, _)) = ai
                .user_providers
                .iter()
                .find(|(_, k)| **k == AiProviderKind::AnthropicApi)
            {
                return invalid(format!(
                    "STRATA_AI__ANTHROPIC_API__API_KEY_FILE is required: user `{user}` is routed to anthropic_api"
                ));
            }
        }
        if ai.user_providers.keys().any(|u| u.trim().is_empty()) {
            return invalid("STRATA_AI__USER_PROVIDERS names an empty username".into());
        }
        let emb = &ai.embedding;
        if emb.model_id.trim().is_empty() {
            return invalid("STRATA_AI__EMBEDDING__MODEL_ID must not be empty".into());
        }
        positive("STRATA_AI__EMBEDDING__DIMS", u64::from(emb.dims))?;
        positive(
            "STRATA_AI__EMBEDDING__MAX_TOKENS",
            u64::from(emb.max_tokens),
        )?;
        if emb.max_batch_tokens < emb.max_tokens {
            return invalid(
                "STRATA_AI__EMBEDDING__MAX_BATCH_TOKENS must be at least STRATA_AI__EMBEDDING__MAX_TOKENS".into(),
            );
        }
        if !(0..=19).contains(&emb.nice) {
            return invalid(format!(
                "STRATA_AI__EMBEDDING__NICE must be within 0..=19, got {}",
                emb.nice
            ));
        }
        positive(
            "STRATA_AI__EMBEDDING__IDLE_UNLOAD_SECS",
            u64::from(emb.idle_unload_secs),
        )?;
        self.budget_tz()?;
        self.validate_jobs()
    }

    fn validate_jobs(&self) -> Result<(), ConfigError> {
        let invalid = |m: String| Err(ConfigError::Invalid(m));
        let positive = |name: &str, v: u64| {
            if v == 0 {
                invalid(format!("{name} must be at least 1"))
            } else {
                Ok(())
            }
        };
        let jobs = &self.jobs;
        positive(
            "STRATA_JOBS__MAX_CONCURRENCY",
            u64::from(jobs.max_concurrency),
        )?;
        positive(
            "STRATA_JOBS__POLL_INTERVAL_SECS",
            u64::from(jobs.poll_interval_secs),
        )?;
        positive(
            "STRATA_JOBS__BACKOFF_BASE_SECS",
            u64::from(jobs.backoff_base_secs),
        )?;
        if jobs.backoff_max_secs < jobs.backoff_base_secs {
            return invalid(
                "STRATA_JOBS__BACKOFF_MAX_SECS must be at least STRATA_JOBS__BACKOFF_BASE_SECS"
                    .into(),
            );
        }
        if jobs.nightly_hour > 23 {
            return invalid(format!(
                "STRATA_JOBS__NIGHTLY_HOUR must be within 0..=23, got {}",
                jobs.nightly_hour
            ));
        }
        if jobs.digest_weekday.parse::<chrono::Weekday>().is_err() {
            return invalid(format!(
                "STRATA_JOBS__DIGEST_WEEKDAY must be a weekday (mon … sun), got {:?}",
                jobs.digest_weekday
            ));
        }
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
        positive(
            "STRATA_AUTH__ACCESS_TOKEN_TTL_SECS",
            auth.access_token_ttl_secs,
        )?;
        positive("STRATA_AUTH__SESSION_TTL_DAYS", auth.session_ttl_days)?;
        positive(
            "STRATA_AUTH__REVOCATION_RELOAD_SECS",
            auth.revocation_reload_secs,
        )?;
        positive("STRATA_AUTH__MIN_PASSWORD_LENGTH", auth.min_password_length)?;
        positive(
            "STRATA_ACCOUNTS__PURGE_INTERVAL_SECS",
            self.accounts.purge_interval_secs,
        )?;
        positive("STRATA_AUTH__ARGON2__ITERATIONS", auth.argon2.iterations)?;
        positive("STRATA_AUTH__ARGON2__PARALLELISM", auth.argon2.parallelism)?;
        if auth.argon2.memory_kib < 8 * auth.argon2.parallelism {
            return Err(ConfigError::Invalid(
                "STRATA_AUTH__ARGON2__MEMORY_KIB must be at least 8 × STRATA_AUTH__ARGON2__PARALLELISM".into(),
            ));
        }
        for (name, limit) in [
            ("LOGIN_PER_IP", auth.rate_limits.login_per_ip),
            ("LOGIN_PER_USERNAME", auth.rate_limits.login_per_username),
            ("SIGNUP_PER_IP", auth.rate_limits.signup_per_ip),
            ("SIGNUP_GLOBAL", auth.rate_limits.signup_global),
            ("CAPTURE_PER_USER", auth.rate_limits.capture_per_user),
            ("ASK_PER_USER", auth.rate_limits.ask_per_user),
        ] {
            positive(&format!("STRATA_AUTH__RATE_LIMITS__{name}__MAX"), limit.max)?;
            positive(
                &format!("STRATA_AUTH__RATE_LIMITS__{name}__WINDOW_SECS"),
                limit.window_secs,
            )?;
        }
        if auth.issuer.trim().is_empty() || auth.audience.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "STRATA_AUTH__ISSUER and STRATA_AUTH__AUDIENCE must not be empty".into(),
            ));
        }
        Ok(())
    }

    /// The parsed default timezone.
    pub fn default_tz(&self) -> Result<Tz, ConfigError> {
        Tz::from_str(&self.default_timezone).map_err(|_| {
            ConfigError::Invalid(format!(
                "STRATA_DEFAULT_TIMEZONE `{}` is not an IANA timezone",
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
                ConfigError::Invalid(format!(
                    "STRATA_BUDGETS__TIMEZONE `{name}` is not an IANA timezone"
                ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// A config from environment pairs only (no file).
    fn from(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        Config::from_vars(
            &env::EnvFile::none(),
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        )
    }

    #[test]
    fn no_variables_yield_the_defaults() {
        let config = from(&[]).expect("defaults are valid");
        assert_eq!(config, Config::default());
        assert_eq!(config.deletion_grace_period(), chrono::Duration::days(14));
        assert_eq!(config.default_tz().expect("UTC"), Tz::UTC);
        assert_eq!(
            config.dedupe_threshold("task"),
            Some(DedupeThreshold {
                near: 0.6,
                semantic: Some(0.88)
            })
        );
        assert_eq!(
            config.dedupe_threshold("alias"),
            Some(DedupeThreshold {
                near: 0.6,
                semantic: None
            })
        );
        assert_eq!(config.dedupe_threshold("unknown"), None);
    }

    /// The default thresholds are the calibrated `domain` defaults, for every kind (near, and
    /// semantic = the candidate level; none where the kind has no semantic level).
    #[test]
    #[allow(clippy::cast_possible_truncation)] // the domain thresholds are f32
    fn default_dedupe_thresholds_equal_the_calibrated_domain_defaults() {
        let config = Config::default();
        let kinds: Vec<&str> = domain::DedupeKind::ALL.iter().map(|k| k.as_str()).collect();
        let mut configured: Vec<&str> = config
            .thresholds
            .dedupe
            .keys()
            .map(String::as_str)
            .collect();
        configured.sort_unstable();
        let mut sorted = kinds.clone();
        sorted.sort_unstable();
        assert_eq!(configured, sorted);
        for kind in domain::DedupeKind::ALL {
            let d = domain::DedupeThresholds::default_for(*kind);
            let c = config.thresholds.dedupe[kind.as_str()];
            assert_eq!(
                (c.near as f32, c.semantic.map(|s| s as f32)),
                (d.near, d.semantic.map(|s| s.candidate)),
                "{kind}"
            );
        }
    }

    #[test]
    fn auth_settings_have_documented_defaults() {
        let config = Config::default();
        assert_eq!(
            config.auth.signing_key_file,
            PathBuf::from("/etc/strata/token-signing-key.pem")
        );
        assert_eq!(config.auth.access_token_ttl_secs, 900);
        assert_eq!(
            config.auth.rate_limits,
            RateLimits {
                login_per_ip: RateLimit {
                    max: 30,
                    window_secs: 900
                },
                login_per_username: RateLimit {
                    max: 10,
                    window_secs: 900
                },
                signup_per_ip: RateLimit {
                    max: 5,
                    window_secs: 3600
                },
                signup_global: RateLimit {
                    max: 30,
                    window_secs: 3600
                },
                capture_per_user: RateLimit {
                    max: 60,
                    window_secs: 60
                },
                ask_per_user: RateLimit {
                    max: 20,
                    window_secs: 60
                },
            }
        );
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

    #[test]
    fn ai_settings_have_documented_defaults() {
        let config = Config::default();
        let ai = &config.ai;
        assert_eq!(ai.default_provider, AiProviderKind::ClaudeCli);
        assert_eq!(ai.user_providers, BTreeMap::new());
        assert_eq!(
            ai.claude_cli,
            ClaudeCliSettings {
                command: vec!["/usr/local/bin/claude".into()],
                scratch_dir: PathBuf::from("/var/lib/strata-ai/scratch"),
                launch_dir: None,
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
        // fp32 weights by default (owner decision 2026-09-27, PLAN §9.1b), unloaded after 5 min.
        assert_eq!(
            ai.embedding,
            EmbeddingSettings {
                model_dir: None,
                onnxruntime_lib: None,
                model_file: PathBuf::from("onnx/model.onnx"),
                tokenizer_file: PathBuf::from("tokenizer.json"),
                model_id: "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model".into(),
                dims: 384,
                pooling: EmbeddingPooling::Cls,
                max_tokens: 2048,
                max_batch_tokens: 8192,
                pad_batches: true,
                nice: 19,
                idle_unload_secs: 300,
            }
        );
        assert_eq!(
            config.jobs,
            JobsConfig {
                max_concurrency: 2,
                poll_interval_secs: 5,
                idle_recheck_secs: 60,
                backoff_base_secs: 30,
                backoff_max_secs: 3600,
                nightly_hour: 3,
                digest_weekday: "mon".to_owned(),
                shutdown_grace_secs: 30,
            }
        );
        assert_eq!(config.jobs.digest_day(), chrono::Weekday::Mon);
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
    fn provider_routing_and_timezones_follow_the_variables() {
        let config = from(&[
            ("STRATA_DEFAULT_TIMEZONE", "Africa/Cairo"),
            ("STRATA_BUDGETS__TIMEZONE", "Europe/Berlin"),
            ("STRATA_AI__DEFAULT_PROVIDER", "disabled"),
            (
                "STRATA_AI__USER_PROVIDERS",
                "guest=anthropic_api, owner=claude_cli",
            ),
            (
                "STRATA_AI__ANTHROPIC_API__API_KEY_FILE",
                "/etc/strata/anthropic.key",
            ),
            ("STRATA_JOBS__DIGEST_WEEKDAY", "fri"),
        ])
        .expect("valid");
        assert_eq!(config.default_tz().expect("tz"), chrono_tz::Africa::Cairo);
        assert_eq!(config.budget_tz().expect("tz"), chrono_tz::Europe::Berlin);
        assert_eq!(
            config.ai_provider_for("guest"),
            AiProviderKind::AnthropicApi
        );
        assert_eq!(config.ai_provider_for("owner"), AiProviderKind::ClaudeCli);
        assert_eq!(config.ai_provider_for("other"), AiProviderKind::Disabled);
        assert!(config.ai_provider_in_use(AiProviderKind::AnthropicApi));
        assert_eq!(config.jobs.digest_day(), chrono::Weekday::Fri);
    }

    /// Every range and cross-field rule, with its exact message naming the variables.
    #[test]
    #[allow(clippy::too_many_lines)] // one assertion per rule
    fn values_are_validated_with_messages_naming_the_variables() {
        let invalid = |pairs: &[(&str, &str)], message: &str| {
            assert_eq!(
                from(pairs),
                Err(ConfigError::Invalid(message.into())),
                "{pairs:?}"
            );
        };
        invalid(
            &[("STRATA_DEFAULT_TIMEZONE", "Mars/Base")],
            "STRATA_DEFAULT_TIMEZONE `Mars/Base` is not an IANA timezone",
        );
        invalid(
            &[("STRATA_BUDGETS__TIMEZONE", "Mars/Base")],
            "STRATA_BUDGETS__TIMEZONE `Mars/Base` is not an IANA timezone",
        );
        invalid(
            &[("STRATA_THRESHOLDS__RELATION", "1.01")],
            "STRATA_THRESHOLDS__RELATION must be within 0..=1, got 1.01",
        );
        invalid(
            &[("STRATA_THRESHOLDS__CUSTODY", "1.5")],
            "STRATA_THRESHOLDS__CUSTODY must be within 0..=1, got 1.5",
        );
        invalid(
            &[("STRATA_THRESHOLDS__DEDUPE__NOTE__NEAR", "-0.1")],
            "STRATA_THRESHOLDS__DEDUPE__NOTE__NEAR must be within 0..=1, got -0.1",
        );
        invalid(
            &[("STRATA_THRESHOLDS__DEDUPE__TASK__SEMANTIC", "2")],
            "STRATA_THRESHOLDS__DEDUPE__TASK__SEMANTIC must be within 0..=1, got 2",
        );
        invalid(
            &[("STRATA_DATABASE__MAX_CONNECTIONS", "0")],
            "STRATA_DATABASE__MAX_CONNECTIONS must be at least 1",
        );
        invalid(
            &[("STRATA_DATABASE__APP_URL", "mysql://app:hunter2@db/strata")],
            "STRATA_DATABASE__APP_URL must be a postgres:// URL",
        );
        invalid(
            &[("STRATA_DATABASE__OWNER_URL", "localhost")],
            "STRATA_DATABASE__OWNER_URL must be a postgres:// URL",
        );
        invalid(
            &[("STRATA_DATABASE__ACCOUNTS_URL", "x")],
            "STRATA_DATABASE__ACCOUNTS_URL must be a postgres:// URL",
        );
        invalid(
            &[("STRATA_AI__CLAUDE_CLI__COMMAND", "   ")],
            "STRATA_AI__CLAUDE_CLI__COMMAND must name a program",
        );
        invalid(
            &[("STRATA_AI__CLAUDE_CLI__SCRATCH_DIR", "scratch")],
            "STRATA_AI__CLAUDE_CLI__SCRATCH_DIR must be an absolute path",
        );
        invalid(
            &[("STRATA_AI__CLAUDE_CLI__MAX_CONCURRENCY", "0")],
            "STRATA_AI__CLAUDE_CLI__MAX_CONCURRENCY must be at least 1",
        );
        invalid(
            &[("STRATA_AI__CLAUDE_CLI__TIMEOUT_SECS", "0")],
            "STRATA_AI__CLAUDE_CLI__TIMEOUT_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AI__CLAUDE_CLI__USAGE_LIMIT_PAUSE_SECS", "0")],
            "STRATA_AI__CLAUDE_CLI__USAGE_LIMIT_PAUSE_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AI__ANTHROPIC_API__MODEL", " ")],
            "STRATA_AI__ANTHROPIC_API__MODEL must not be empty",
        );
        invalid(
            &[(
                "STRATA_AI__ANTHROPIC_API__BASE_URL",
                "https://api.anthropic.com/",
            )],
            "STRATA_AI__ANTHROPIC_API__BASE_URL must be an http(s) URL without a trailing slash",
        );
        invalid(
            &[("STRATA_AI__ANTHROPIC_API__BASE_URL", "ftp://x")],
            "STRATA_AI__ANTHROPIC_API__BASE_URL must be an http(s) URL without a trailing slash",
        );
        invalid(
            &[("STRATA_AI__ANTHROPIC_API__EFFORT", "extreme")],
            "STRATA_AI__ANTHROPIC_API__EFFORT must be one of low, medium, high, xhigh, max, got `extreme`",
        );
        invalid(
            &[("STRATA_AI__ANTHROPIC_API__MAX_RETRIES", "11")],
            "STRATA_AI__ANTHROPIC_API__MAX_RETRIES must be at most 10",
        );
        invalid(
            &[("STRATA_AI__ANTHROPIC_API__TIMEOUT_SECS", "0")],
            "STRATA_AI__ANTHROPIC_API__TIMEOUT_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AI__DEFAULT_PROVIDER", "anthropic_api")],
            "STRATA_AI__ANTHROPIC_API__API_KEY_FILE is required: STRATA_AI__DEFAULT_PROVIDER is anthropic_api",
        );
        invalid(
            &[("STRATA_AI__USER_PROVIDERS", "guest=anthropic_api")],
            "STRATA_AI__ANTHROPIC_API__API_KEY_FILE is required: user `guest` is routed to anthropic_api",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__MODEL_ID", " ")],
            "STRATA_AI__EMBEDDING__MODEL_ID must not be empty",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__DIMS", "0")],
            "STRATA_AI__EMBEDDING__DIMS must be at least 1",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__MAX_TOKENS", "0")],
            "STRATA_AI__EMBEDDING__MAX_TOKENS must be at least 1",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__MAX_TOKENS", "9000")],
            "STRATA_AI__EMBEDDING__MAX_BATCH_TOKENS must be at least STRATA_AI__EMBEDDING__MAX_TOKENS",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__NICE", "20")],
            "STRATA_AI__EMBEDDING__NICE must be within 0..=19, got 20",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__NICE", "-1")],
            "STRATA_AI__EMBEDDING__NICE must be within 0..=19, got -1",
        );
        invalid(
            &[("STRATA_AI__EMBEDDING__IDLE_UNLOAD_SECS", "0")],
            "STRATA_AI__EMBEDDING__IDLE_UNLOAD_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_JOBS__MAX_CONCURRENCY", "0")],
            "STRATA_JOBS__MAX_CONCURRENCY must be at least 1",
        );
        invalid(
            &[("STRATA_JOBS__POLL_INTERVAL_SECS", "0")],
            "STRATA_JOBS__POLL_INTERVAL_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_JOBS__BACKOFF_BASE_SECS", "0")],
            "STRATA_JOBS__BACKOFF_BASE_SECS must be at least 1",
        );
        invalid(
            &[
                ("STRATA_JOBS__BACKOFF_BASE_SECS", "60"),
                ("STRATA_JOBS__BACKOFF_MAX_SECS", "30"),
            ],
            "STRATA_JOBS__BACKOFF_MAX_SECS must be at least STRATA_JOBS__BACKOFF_BASE_SECS",
        );
        invalid(
            &[("STRATA_JOBS__NIGHTLY_HOUR", "24")],
            "STRATA_JOBS__NIGHTLY_HOUR must be within 0..=23, got 24",
        );
        invalid(
            &[("STRATA_JOBS__DIGEST_WEEKDAY", "someday")],
            "STRATA_JOBS__DIGEST_WEEKDAY must be a weekday (mon … sun), got \"someday\"",
        );
        invalid(
            &[("STRATA_AUTH__ACCESS_TOKEN_TTL_SECS", "0")],
            "STRATA_AUTH__ACCESS_TOKEN_TTL_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__SESSION_TTL_DAYS", "0")],
            "STRATA_AUTH__SESSION_TTL_DAYS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__REVOCATION_RELOAD_SECS", "0")],
            "STRATA_AUTH__REVOCATION_RELOAD_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__MIN_PASSWORD_LENGTH", "0")],
            "STRATA_AUTH__MIN_PASSWORD_LENGTH must be at least 1",
        );
        invalid(
            &[("STRATA_ACCOUNTS__PURGE_INTERVAL_SECS", "0")],
            "STRATA_ACCOUNTS__PURGE_INTERVAL_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__ARGON2__ITERATIONS", "0")],
            "STRATA_AUTH__ARGON2__ITERATIONS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__ARGON2__PARALLELISM", "0")],
            "STRATA_AUTH__ARGON2__PARALLELISM must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__ARGON2__MEMORY_KIB", "7")],
            "STRATA_AUTH__ARGON2__MEMORY_KIB must be at least 8 × STRATA_AUTH__ARGON2__PARALLELISM",
        );
        invalid(
            &[("STRATA_AUTH__RATE_LIMITS__SIGNUP_GLOBAL__MAX", "0")],
            "STRATA_AUTH__RATE_LIMITS__SIGNUP_GLOBAL__MAX must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__RATE_LIMITS__ASK_PER_USER__WINDOW_SECS", "0")],
            "STRATA_AUTH__RATE_LIMITS__ASK_PER_USER__WINDOW_SECS must be at least 1",
        );
        invalid(
            &[("STRATA_AUTH__ISSUER", " ")],
            "STRATA_AUTH__ISSUER and STRATA_AUTH__AUDIENCE must not be empty",
        );
        let with_key = from(&[
            ("STRATA_AI__DEFAULT_PROVIDER", "anthropic_api"),
            ("STRATA_AI__ANTHROPIC_API__API_KEY_FILE", "/k"),
        ])
        .expect("key given");
        assert_eq!(
            with_key.ai_provider_for("anyone"),
            AiProviderKind::AnthropicApi
        );
    }

    /// A misconfigured database URL is reported without its password.
    #[test]
    fn database_url_errors_never_contain_the_url() {
        let err = from(&[("STRATA_DATABASE__APP_URL", "mysql://app:hunter2@db/strata")])
            .expect_err("not postgres");
        assert!(!err.to_string().contains("hunter2"), "{err}");
    }
}
