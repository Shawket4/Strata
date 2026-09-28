//! Loading [`Config`] from `STRATA_` variables: a `.env` file plus the process environment.
//!
//! **Names.** Every setting is one variable, `STRATA_` followed by its key path in [`Config`]
//! with the segments joined by `__` and upper-cased: `STRATA_BIND`,
//! `STRATA_DATABASE__APP_URL`, `STRATA_AI__CLAUDE_CLI__TIMEOUT_SECS`,
//! `STRATA_THRESHOLDS__DEDUPE__NOTE__NEAR`, `STRATA_AUTH__RATE_LIMITS__LOGIN_PER_IP__MAX`.
//! [`variables`] lists them all; `deploy/stratad.env.example` documents each with its default
//! (a test keeps the two in step).
//!
//! **Sources.** Built-in defaults, then the env file, then the process environment (a real
//! environment variable always wins over the file). The file is `--env-file <path>` /
//! `STRATA_ENV_FILE` when given (it must exist), else `.env` in the working directory when
//! present, else none. The file is parsed with `dotenvy` into values only; the process
//! environment is never modified.
//!
//! **Values.** Numbers, `true`/`false`, paths and text as written; `STRATA_AI__CLAUDE_CLI__COMMAND`
//! is the program and its arguments separated by spaces; `STRATA_AI__USER_PROVIDERS` is
//! comma-separated `username=provider` pairs. An empty value unsets an optional setting (and
//! is an error for a required one). In the file, quote a value holding spaces, `#`, `$` or
//! quotes with single quotes (`dotenvy` expands `$NAME` in unquoted and double-quoted values).
//!
//! **Strictness.** Every name in the file must be a setting. In the environment, a name
//! starting with `STRATA_` that contains `__` must be a setting (so a misspelt nested key is an
//! error, not silently ignored); other `STRATA_` names belong to other tools (tests, the CLI)
//! and are ignored. Errors name the variable and never echo a value.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use super::{AiProviderKind, Config, DEFAULT_DEDUPE_THRESHOLDS, DedupeThreshold, EmbeddingPooling};
use crate::error::ConfigError;

/// Prefix of every configuration variable.
pub const PREFIX: &str = "STRATA_";
/// Variable naming the env file (the `--env-file` flag of `stratad`).
pub const ENV_FILE_VAR: &str = "STRATA_ENV_FILE";
/// The env file read from the working directory when none is named.
pub const DEFAULT_ENV_FILE: &str = ".env";

/// The variables of one env file, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvFile {
    /// Where they came from (`None`: no file).
    pub path: Option<PathBuf>,
    /// `(name, value)` pairs, each name once.
    pub vars: Vec<(String, String)>,
}

impl EnvFile {
    /// No file.
    pub fn none() -> Self {
        Self::default()
    }

    /// Reads and parses `path`.
    pub fn read(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Read {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        Self::parse(path, &text)
    }

    /// Parses env-file `text` (read from `path`, used in errors).
    pub fn parse(path: &Path, text: &str) -> Result<Self, ConfigError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut vars: Vec<(String, String)> = Vec::new();
        for item in dotenvy::from_read_iter(text.as_bytes()) {
            match item {
                Ok((name, value)) => {
                    if vars.iter().any(|(n, _)| *n == name) {
                        return Err(ConfigError::Duplicate {
                            path: path.to_path_buf(),
                            name,
                        });
                    }
                    vars.push((name, value));
                }
                Err(dotenvy::Error::LineParse(line, _)) => {
                    return Err(ConfigError::Syntax {
                        path: path.to_path_buf(),
                        line: line_number(text, &line),
                    });
                }
                Err(e) => {
                    return Err(ConfigError::Read {
                        path: path.to_path_buf(),
                        message: e.to_string(),
                    });
                }
            }
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            vars,
        })
    }
}

/// The 1-based line of `text` where the rejected `fragment` (as `dotenvy` reports it) starts.
fn line_number(text: &str, fragment: &str) -> usize {
    let first = fragment.lines().next().unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .position(|l| *l == first)
        .or_else(|| {
            (!first.is_empty())
                .then(|| lines.iter().position(|l| l.contains(first)))
                .flatten()
        })
        .map_or(lines.len().max(1), |i| i + 1)
}

/// Loads the configuration from `env_file` (else `./.env` if present) and the process
/// environment.
pub fn load(env_file: Option<&Path>) -> Result<Config, ConfigError> {
    let file = if let Some(path) = env_file {
        EnvFile::read(path)?
    } else if Path::new(DEFAULT_ENV_FILE).is_file() {
        EnvFile::read(Path::new(DEFAULT_ENV_FILE))?
    } else {
        EnvFile::none()
    };
    let mut environment = Vec::new();
    for (name, value) in std::env::vars_os() {
        let Ok(name) = name.into_string() else {
            continue;
        };
        if !name.starts_with(PREFIX) {
            continue;
        }
        match value.into_string() {
            Ok(value) => environment.push((name, value)),
            Err(_) => {
                return Err(ConfigError::Value {
                    name,
                    message: "not valid UTF-8".into(),
                });
            }
        }
    }
    from_vars(&file, environment)
}

/// Builds and validates a config from `file` and `environment` pairs (the environment wins;
/// names without the `STRATA_` prefix are ignored).
pub fn from_vars(
    file: &EnvFile,
    environment: impl IntoIterator<Item = (String, String)>,
) -> Result<Config, ConfigError> {
    let vars = variables_table();
    let known: HashMap<&str, usize> = vars
        .iter()
        .enumerate()
        .map(|(i, v)| (v.name.as_str(), i))
        .collect();
    let mut values: BTreeMap<usize, String> = BTreeMap::new();
    let origin = file
        .path
        .as_ref()
        .map_or_else(String::new, |p| p.display().to_string());
    for (name, value) in &file.vars {
        let Some(&i) = known.get(name.as_str()) else {
            return Err(unknown(name, &origin, &vars));
        };
        values.insert(i, value.clone());
    }
    let mut environment: Vec<(String, String)> = environment
        .into_iter()
        .filter(|(n, _)| n.starts_with(PREFIX))
        .collect();
    environment.sort();
    for (name, value) in environment {
        if let Some(&i) = known.get(name.as_str()) {
            values.insert(i, value);
        } else if name[PREFIX.len()..].contains("__") {
            return Err(unknown(&name, "the environment", &vars));
        }
    }
    let mut config = Config::default();
    for (i, raw) in values {
        let var = &vars[i];
        (var.set)(&mut config, &raw).map_err(|message| ConfigError::Value {
            name: var.name.clone(),
            message,
        })?;
    }
    config.validate()?;
    Ok(config)
}

fn unknown(name: &str, origin: &str, vars: &[Var]) -> ConfigError {
    let hint = if let Some(rest) = name.strip_prefix("STRATA__") {
        Some(format!(
            "the old STRATA__ prefix is now STRATA_ (STRATA_{rest})"
        ))
    } else if !name.starts_with(PREFIX) {
        Some("only STRATA_ settings belong in the env file".to_owned())
    } else {
        vars.iter()
            .map(|v| (edit_distance(name, &v.name), &v.name))
            .filter(|(d, _)| *d <= 3)
            .min()
            .map(|(_, n)| format!("did you mean {n}?"))
    };
    ConfigError::Unknown {
        name: name.to_owned(),
        origin: origin.to_owned(),
        hint,
    }
}

/// Levenshtein distance (names are ASCII and short).
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = if ca == *cb {
                prev
            } else {
                1 + prev.min(cur).min(row[j])
            };
            prev = cur;
        }
    }
    row[b.len()]
}

/// Every configuration variable name, in documentation order.
pub fn variables() -> Vec<String> {
    variables_table().into_iter().map(|v| v.name).collect()
}

/// Every variable of `config` as `(name, value)`, in documentation order; `from_vars` of
/// this list gives `config` back. Values are raw: database URLs may carry passwords.
pub fn to_vars(config: &Config) -> Vec<(String, String)> {
    variables_table()
        .into_iter()
        .map(|v| {
            let value = (v.get)(config);
            (v.name, value)
        })
        .collect()
}

/// `config` as env-file text (one `NAME=value` line per variable, quoted where needed).
pub fn to_env_file(config: &Config) -> String {
    let mut out = String::new();
    for (name, value) in to_vars(config) {
        out.push_str(&name);
        out.push('=');
        out.push_str(&quote(&value));
        out.push('\n');
    }
    out
}

/// `value` written so `dotenvy` reads it back unchanged.
pub fn quote(value: &str) -> String {
    let plain = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_-./:@+,=%".contains(c));
    if plain {
        value.to_owned()
    } else if !value.contains('\'') {
        format!("'{value}'")
    } else {
        let mut out = String::from("\"");
        for c in value.chars() {
            if matches!(c, '\\' | '"' | '$') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    }
}

/// `url` with the password of its user-info part replaced by `***` (for display).
pub fn redact_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_owned();
    };
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let Some(at) = rest[..authority_end].rfind('@') else {
        return url.to_owned();
    };
    let Some(colon) = rest[..at].find(':') else {
        return url.to_owned();
    };
    format!(
        "{}{}:***{}",
        &url[..scheme_end + 3],
        &rest[..colon],
        &rest[at..]
    )
}

type Setter = Box<dyn Fn(&mut Config, &str) -> Result<(), String>>;
type Getter = Box<dyn Fn(&Config) -> String>;

struct Var {
    name: String,
    set: Setter,
    get: Getter,
}

/// `a.b_c.d` → `STRATA_A__B_C__D`.
fn var_name(path: &str) -> String {
    let path: String = path.chars().filter(|c| !c.is_whitespace()).collect();
    format!("{PREFIX}{}", path.replace('.', "__").to_ascii_uppercase())
}

macro_rules! var {
    ($($field:ident).+) => {
        Var {
            name: var_name(stringify!($($field).+)),
            set: Box::new(|c: &mut Config, raw: &str| {
                c.$($field).+ = parse_var(raw, &c.$($field).+)?;
                Ok(())
            }),
            get: Box::new(|c: &Config| EnvValue::render(&c.$($field).+)),
        }
    };
}

#[allow(clippy::too_many_lines)] // one line per variable
fn variables_table() -> Vec<Var> {
    let mut vars = vec![
        var!(data_root),
        var!(bind),
        var!(default_timezone),
        var!(max_future_skew_secs),
        var!(database.owner_url),
        var!(database.app_url),
        var!(database.accounts_url),
        var!(database.max_connections),
        var!(ai.default_provider),
        var!(ai.user_providers),
        var!(ai.daily_job_limit),
        var!(ai.claude_cli.command),
        var!(ai.claude_cli.scratch_dir),
        var!(ai.claude_cli.launch_dir),
        var!(ai.claude_cli.model),
        var!(ai.claude_cli.max_concurrency),
        var!(ai.claude_cli.timeout_secs),
        var!(ai.claude_cli.kill_grace_secs),
        var!(ai.claude_cli.usage_limit_pause_secs),
        var!(ai.claude_cli.warm_pool),
        var!(ai.claude_cli.warm_max_idle_secs),
        var!(ai.anthropic_api.api_key_file),
        var!(ai.anthropic_api.model),
        var!(ai.anthropic_api.base_url),
        var!(ai.anthropic_api.effort),
        var!(ai.anthropic_api.max_retries),
        var!(ai.anthropic_api.timeout_secs),
        var!(ai.anthropic_api.input_micros_per_mtok),
        var!(ai.anthropic_api.output_micros_per_mtok),
        var!(ai.embedding.model_dir),
        var!(ai.embedding.onnxruntime_lib),
        var!(ai.embedding.model_file),
        var!(ai.embedding.tokenizer_file),
        var!(ai.embedding.model_id),
        var!(ai.embedding.dims),
        var!(ai.embedding.pooling),
        var!(ai.embedding.max_tokens),
        var!(ai.embedding.max_batch_tokens),
        var!(ai.embedding.pad_batches),
        var!(ai.embedding.nice),
        var!(ai.embedding.idle_unload_secs),
        var!(thresholds.relation),
        var!(thresholds.custody),
    ];
    for &(kind, near, semantic) in DEFAULT_DEDUPE_THRESHOLDS {
        let upper = kind.to_ascii_uppercase();
        let default = DedupeThreshold { near, semantic };
        vars.push(Var {
            name: format!("{PREFIX}THRESHOLDS__DEDUPE__{upper}__NEAR"),
            set: Box::new(move |c: &mut Config, raw: &str| {
                let t = c
                    .thresholds
                    .dedupe
                    .entry(kind.to_owned())
                    .or_insert(default);
                t.near = parse_var(raw, &t.near)?;
                Ok(())
            }),
            get: Box::new(move |c: &Config| {
                c.thresholds
                    .dedupe
                    .get(kind)
                    .map(|t| t.near.render())
                    .unwrap_or_default()
            }),
        });
        vars.push(Var {
            name: format!("{PREFIX}THRESHOLDS__DEDUPE__{upper}__SEMANTIC"),
            set: Box::new(move |c: &mut Config, raw: &str| {
                let t = c
                    .thresholds
                    .dedupe
                    .entry(kind.to_owned())
                    .or_insert(default);
                t.semantic = parse_var(raw, &t.semantic)?;
                Ok(())
            }),
            get: Box::new(move |c: &Config| {
                c.thresholds
                    .dedupe
                    .get(kind)
                    .map(|t| t.semantic.render())
                    .unwrap_or_default()
            }),
        });
    }
    vars.extend([
        var!(budgets.timezone),
        var!(budgets.per_user_daily_tokens),
        var!(budgets.per_user_daily_cost_micros),
        var!(budgets.global_daily_tokens),
        var!(budgets.global_daily_cost_micros),
        var!(accounts.deletion_grace_days),
        var!(accounts.max_pending_signups),
        var!(accounts.purge_interval_secs),
        var!(push.fcm_service_account_path),
        var!(push.apns_key_path),
        var!(push.apns_key_id),
        var!(push.apns_team_id),
        var!(push.apns_topic),
        var!(push.wns_credentials_path),
        var!(auth.signing_key_file),
        var!(auth.issuer),
        var!(auth.audience),
        var!(auth.access_token_ttl_secs),
        var!(auth.session_ttl_days),
        var!(auth.revocation_reload_secs),
        var!(auth.trust_forwarded_for),
        var!(auth.min_password_length),
        var!(auth.argon2.memory_kib),
        var!(auth.argon2.iterations),
        var!(auth.argon2.parallelism),
        var!(auth.rate_limits.login_per_ip.max),
        var!(auth.rate_limits.login_per_ip.window_secs),
        var!(auth.rate_limits.login_per_username.max),
        var!(auth.rate_limits.login_per_username.window_secs),
        var!(auth.rate_limits.signup_per_ip.max),
        var!(auth.rate_limits.signup_per_ip.window_secs),
        var!(auth.rate_limits.signup_global.max),
        var!(auth.rate_limits.signup_global.window_secs),
        var!(auth.rate_limits.capture_per_user.max),
        var!(auth.rate_limits.capture_per_user.window_secs),
        var!(auth.rate_limits.ask_per_user.max),
        var!(auth.rate_limits.ask_per_user.window_secs),
        var!(jobs.max_concurrency),
        var!(jobs.poll_interval_secs),
        var!(jobs.idle_recheck_secs),
        var!(jobs.backoff_base_secs),
        var!(jobs.backoff_max_secs),
        var!(jobs.nightly_hour),
        var!(jobs.digest_weekday),
        var!(jobs.shutdown_grace_secs),
    ]);
    vars
}

/// Parses `raw` as the type of `current` (the value it replaces, used in the message for an
/// empty required value).
fn parse_var<T: EnvValue>(raw: &str, current: &T) -> Result<T, String> {
    if raw.is_empty() && !T::EMPTY_OK {
        return Err(format!(
            "must not be empty; remove the line to use the default `{}`",
            current.render()
        ));
    }
    T::parse(raw).ok_or_else(|| format!("expected {}", T::expected()))
}

/// A configuration value type: how it is written in a variable.
trait EnvValue: Sized {
    /// Whether an empty value is valid (optional settings: empty = unset).
    const EMPTY_OK: bool = false;
    /// What a valid value looks like, for errors.
    fn expected() -> String;
    /// Parses a value (`None` if malformed).
    fn parse(raw: &str) -> Option<Self>;
    /// Writes the value (`parse(render(v)) == Some(v)`).
    fn render(&self) -> String;
}

impl EnvValue for String {
    fn expected() -> String {
        "text".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        Some(raw.to_owned())
    }
    fn render(&self) -> String {
        self.clone()
    }
}

impl EnvValue for PathBuf {
    fn expected() -> String {
        "a path".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        Some(Self::from(raw))
    }
    fn render(&self) -> String {
        self.to_string_lossy().into_owned()
    }
}

macro_rules! integer {
    ($($t:ty),*) => {$(
        impl EnvValue for $t {
            fn expected() -> String {
                format!("a whole number from {} to {}", <$t>::MIN, <$t>::MAX)
            }
            fn parse(raw: &str) -> Option<Self> {
                raw.trim().parse().ok()
            }
            fn render(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
integer!(u32, u64, i32);

impl EnvValue for f64 {
    fn expected() -> String {
        "a number, e.g. 0.75".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        raw.trim().parse::<Self>().ok().filter(|v| v.is_finite())
    }
    fn render(&self) -> String {
        self.to_string()
    }
}

impl EnvValue for bool {
    fn expected() -> String {
        "true or false".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
    fn render(&self) -> String {
        self.to_string()
    }
}

impl EnvValue for SocketAddr {
    fn expected() -> String {
        "an IP address and port, e.g. 127.0.0.1:8080".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        raw.trim().parse().ok()
    }
    fn render(&self) -> String {
        self.to_string()
    }
}

/// A program and its arguments, separated by spaces (arguments cannot contain spaces).
impl EnvValue for Vec<String> {
    fn expected() -> String {
        "a program and its arguments separated by spaces".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        Some(raw.split_whitespace().map(str::to_owned).collect())
    }
    fn render(&self) -> String {
        self.join(" ")
    }
}

const PROVIDERS: [(&str, AiProviderKind); 3] = [
    ("claude_cli", AiProviderKind::ClaudeCli),
    ("anthropic_api", AiProviderKind::AnthropicApi),
    ("disabled", AiProviderKind::Disabled),
];

impl EnvValue for AiProviderKind {
    fn expected() -> String {
        "one of claude_cli, anthropic_api, disabled".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        PROVIDERS.iter().find(|(n, _)| *n == raw).map(|(_, k)| *k)
    }
    fn render(&self) -> String {
        PROVIDERS
            .iter()
            .find(|(_, k)| k == self)
            .map(|(n, _)| (*n).to_owned())
            .unwrap_or_default()
    }
}

impl EnvValue for EmbeddingPooling {
    fn expected() -> String {
        "cls or mean".into()
    }
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "cls" => Some(Self::Cls),
            "mean" => Some(Self::Mean),
            _ => None,
        }
    }
    fn render(&self) -> String {
        match self {
            Self::Cls => "cls",
            Self::Mean => "mean",
        }
        .to_owned()
    }
}

/// Per-user providers: `owner=claude_cli,guest=anthropic_api`.
impl EnvValue for BTreeMap<String, AiProviderKind> {
    const EMPTY_OK: bool = true;
    fn expected() -> String {
        "comma-separated username=provider pairs, e.g. owner=claude_cli,guest=anthropic_api \
         (providers: claude_cli, anthropic_api, disabled; each username once)"
            .into()
    }
    fn parse(raw: &str) -> Option<Self> {
        let mut map = Self::new();
        for item in raw.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let (user, provider) = item.split_once('=')?;
            let user = user.trim();
            if user.is_empty() {
                return None;
            }
            let provider = AiProviderKind::parse(provider)?;
            if map.insert(user.to_owned(), provider).is_some() {
                return None;
            }
        }
        Some(map)
    }
    fn render(&self) -> String {
        self.iter()
            .map(|(user, provider)| format!("{user}={}", provider.render()))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl<T: EnvValue> EnvValue for Option<T> {
    const EMPTY_OK: bool = true;
    fn expected() -> String {
        format!("{}, or nothing to leave it unset", T::expected())
    }
    fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            Some(None)
        } else {
            T::parse(raw).map(Some)
        }
    }
    fn render(&self) -> String {
        self.as_ref().map(EnvValue::render).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
