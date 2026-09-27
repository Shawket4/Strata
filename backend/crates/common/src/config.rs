//! `stratad.toml` configuration with environment overrides (PLAN §14 "Config").
//!
//! Layering, lowest to highest precedence: built-in defaults → TOML file → environment.
//! Environment variables named `STRATA__<SECTION>__<KEY>` (double underscores between path
//! segments, case-insensitive) override the matching key, e.g.
//! `STRATA__DATABASE__APP_URL=postgres://…` or `STRATA__THRESHOLDS__DEDUPE__TASK__NEAR=0.6`.
//! The override value is parsed with the type of the key it replaces (string, integer, float,
//! boolean); keys that are unset by default are parsed as boolean/integer/float if possible and
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
    /// PostgreSQL connection settings, one URL per role (§5.2).
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

/// AI provider settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfig {
    /// Provider for users without an entry in `user_providers`.
    pub default_provider: AiProviderKind,
    /// Per-username provider override (D23: chosen per user in config).
    pub user_providers: BTreeMap<String, AiProviderKind>,
    /// Path of the `claude` binary.
    pub claude_bin: PathBuf,
    /// Model identifier passed to the provider.
    pub model: String,
    /// File holding the Anthropic API key (0600; read at startup, never logged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_file: Option<PathBuf>,
    /// Directory with the int8 ONNX embedding model and tokenizer (L19, D9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model_dir: Option<PathBuf>,
    /// Concurrent provider calls (§9.1: low by default).
    pub max_concurrency: u32,
    /// Maximum AI jobs per day across all users (0 = unlimited).
    pub daily_job_limit: u32,
}

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

/// AI budgets; exceeding one pauses (never fails) the affected jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    /// Tokens (input + output) per user per day (0 = unlimited).
    pub per_user_daily_tokens: u64,
    /// Tokens per day across all users (0 = unlimited).
    pub global_daily_tokens: u64,
    /// Estimated cost per user per day in micro-USD (0 = unlimited).
    pub per_user_daily_cost_micros: u64,
}

/// Account lifecycle settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountsConfig {
    /// Grace period between scheduling a deletion and the purge (D25, default 14 days).
    pub deletion_grace_days: u32,
    /// Maximum accounts waiting in `pending` (§15 signup cap).
    pub max_pending_signups: u32,
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
            ai: AiConfig {
                default_provider: AiProviderKind::ClaudeCli,
                user_providers: BTreeMap::new(),
                claude_bin: PathBuf::from("/usr/bin/claude"),
                model: "default".to_owned(),
                api_key_file: None,
                embedding_model_dir: None,
                max_concurrency: 1,
                daily_job_limit: 0,
            },
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
                per_user_daily_tokens: 2_000_000,
                global_daily_tokens: 4_000_000,
                per_user_daily_cost_micros: 0,
            },
            accounts: AccountsConfig {
                deletion_grace_days: 14,
                max_pending_signups: 20,
            },
            push: PushConfig::default(),
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
                Err(ConfigError::Invalid(format!("{name} must be within 0..=1, got {v}")))
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
        if self.ai.max_concurrency == 0 {
            return Err(ConfigError::Invalid("ai.max_concurrency must be at least 1".into()));
        }
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

    /// The parsed default timezone.
    pub fn default_tz(&self) -> Result<Tz, ConfigError> {
        Tz::from_str(&self.default_timezone).map_err(|_| {
            ConfigError::Invalid(format!(
                "default_timezone `{}` is not an IANA timezone",
                self.default_timezone
            ))
        })
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
            Some(DedupeThreshold { near: 0.6, semantic: 0.88 })
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
            DedupeThreshold { near: 0.5, semantic: 0.8 },
        );
        expected.push.apns_key_id = Some("ABC123".into());
        assert_eq!(config, expected);
        assert_eq!(config.default_tz().expect("valid tz"), chrono_tz::Africa::Cairo);
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
        assert_eq!(config.bind, "0.0.0.0:9000".parse::<SocketAddr>().expect("addr"));
        assert_eq!(config.thresholds.dedupe["note"].near, 0.65);
        assert_eq!(config.thresholds.dedupe["note"].semantic, 0.9);
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
            Err(ConfigError::Invalid("thresholds.custody must be within 0..=1, got 1.5".into()))
        );
        assert_eq!(
            Config::from_sources("[database]\napp_url = \"mysql://x\"", env(&[])),
            Err(ConfigError::Invalid("database.app_url must be a postgres:// URL".into()))
        );
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
