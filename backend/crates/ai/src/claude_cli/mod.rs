//! `ClaudeCliProvider`: `claude -p` (Claude Code headless mode) as the LLM transport (L18,
//! D20 = a, §9.1).
//!
//! Every call spawns one `claude` process:
//! - through a configurable launcher (`command`, e.g. `sudo -n -u strata-ai <wrapper>`) so it
//!   runs as a dedicated OS user with no access to vaults or data (docs/RUNBOOK.md §9);
//! - in an empty scratch working directory, with the environment cleared and replaced by an
//!   explicit allow-list that may never contain `ANTHROPIC_API_KEY` (billing stays on the
//!   subscription login) and never enables bare mode;
//! - with every tool disabled (`--tools ""`), no MCP servers (`--strict-mcp-config` without a
//!   config), no settings files (`--setting-sources ""`), no skills/slash commands, no session
//!   persistence, and Strata's prompt replacing Claude Code's system prompt;
//! - with the user content on stdin (never in argv, which other local users can read);
//! - under a concurrency limit (default 1) and the shared [`CpuGate`] (§9.1b);
//! - with a timeout that terminates the whole process group.
//!
//! Output is `--output-format stream-json --verbose` for both call kinds, because only that
//! format reports `rate_limit_event`s with the limit's reset time (see [`parse`]). stderr is
//! captured (bounded) for failure classification only and is never logged or returned.

mod parse;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use futures::stream;
use serde_json::Value;
use strata_common::Clock;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::{OwnedRwLockReadGuard, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::error::{AiError, ProviderError};
use crate::gate::CpuGate;
use crate::provider::{HealthTracker, LlmProvider, ProviderHealth, error_kind};
use crate::request::{ChatRequest, JsonCompletion, JsonRequest, StreamEvent, TokenStream, Usage};
use parse::{CliLine, FailureContext, RateLimitInfo, ResultMsg, classify_failure, parse_line};

/// Environment variables that must never reach the CLI: an API key or auth token would switch
/// billing away from the subscription (§9.1); `CLAUDE_CODE_SIMPLE` is bare mode, which ignores
/// the subscription login; the cloud-provider switches would route calls elsewhere.
pub const FORBIDDEN_ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_SIMPLE",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

/// Configuration of [`ClaudeCliProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCliConfig {
    /// Argument vector that starts the CLI; the `claude` arguments are appended. Production:
    /// `["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]` (RUNBOOK §9);
    /// development: `["/usr/bin/claude"]`.
    pub command: Vec<String>,
    /// Empty scratch directory the process runs in.
    pub working_dir: PathBuf,
    /// The child's complete environment (the parent's is cleared).
    pub env: BTreeMap<String, String>,
    /// `--model` (alias or full name); `None` lets the CLI choose.
    pub model: Option<String>,
    /// Wall-clock limit per call.
    pub timeout: Duration,
    /// Concurrent processes (§9.1: low, default 1).
    pub max_concurrency: usize,
    /// Pause length after a usage limit whose reset time is unknown.
    pub default_pause: Duration,
    /// Time between SIGTERM and SIGKILL when a call is cancelled.
    pub kill_grace: Duration,
    /// Bytes of stderr kept for failure classification.
    pub stderr_limit: usize,
}

impl ClaudeCliConfig {
    /// Defaults around `command` and `working_dir`.
    pub fn new(command: Vec<String>, working_dir: PathBuf) -> Self {
        Self {
            command,
            working_dir,
            env: Self::default_env(),
            model: None,
            timeout: Duration::from_secs(300),
            max_concurrency: 1,
            default_pause: Duration::from_secs(30 * 60),
            kill_grace: Duration::from_secs(5),
            stderr_limit: 16 * 1024,
        }
    }

    /// `PATH`, `LANG`, and the switches that turn off telemetry and auto-update.
    pub fn default_env() -> BTreeMap<String, String> {
        [
            ("PATH", "/usr/local/bin:/usr/bin:/bin"),
            ("LANG", "C.UTF-8"),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
            ("DISABLE_AUTOUPDATER", "1"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    /// Checks the rules above.
    pub fn validate(&self) -> Result<(), AiError> {
        if self.command.first().is_none_or(String::is_empty) {
            return Err(AiError::Config("claude_cli.command is empty".into()));
        }
        if let Some(k) = self
            .env
            .keys()
            .find(|k| FORBIDDEN_ENV.iter().any(|f| f.eq_ignore_ascii_case(k)))
        {
            return Err(AiError::Config(format!(
                "claude_cli.env must not set {k} (billing must stay on the subscription login)"
            )));
        }
        if self.max_concurrency == 0 {
            return Err(AiError::Config(
                "claude_cli.max_concurrency must be at least 1".into(),
            ));
        }
        if self.timeout.is_zero() {
            return Err(AiError::Config("claude_cli.timeout must be positive".into()));
        }
        Ok(())
    }
}

/// The `claude -p` provider.
#[derive(Debug)]
pub struct ClaudeCliProvider {
    cfg: Arc<ClaudeCliConfig>,
    model_label: String,
    clock: Arc<dyn Clock>,
    gate: CpuGate,
    permits: Arc<Semaphore>,
    health: Arc<HealthTracker>,
}

impl ClaudeCliProvider {
    /// Validates `cfg` and builds the provider. `gate` must be the one the embedder uses.
    pub fn new(cfg: ClaudeCliConfig, clock: Arc<dyn Clock>, gate: CpuGate) -> Result<Self, AiError> {
        cfg.validate()?;
        Ok(Self {
            model_label: cfg.model.clone().unwrap_or_else(|| "default".to_owned()),
            permits: Arc::new(Semaphore::new(cfg.max_concurrency)),
            cfg: Arc::new(cfg),
            clock,
            gate,
            health: Arc::new(HealthTracker::default()),
        })
    }

    /// The CLI arguments for one call (after `command`).
    pub fn args(&self, system: &str, schema: Option<&Value>) -> Vec<String> {
        let mut a: Vec<String> = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--tools",
            "",
            "--strict-mcp-config",
            "--setting-sources",
            "",
            "--disable-slash-commands",
            "--no-session-persistence",
            "--system-prompt",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        a.push(system.to_owned());
        if let Some(m) = &self.cfg.model {
            a.push("--model".into());
            a.push(m.clone());
        }
        match schema {
            Some(s) => {
                a.push("--json-schema".into());
                a.push(cli_schema(s).to_string());
            }
            None => a.push("--include-partial-messages".into()),
        }
        a
    }

    fn check_pause(&self) -> Result<(), ProviderError> {
        match self.health.active_pause(self.clock.now()) {
            Some((reason, until)) => Err(ProviderError::Paused { reason, until }),
            None => Ok(()),
        }
    }

    async fn start(&self, system: &str, schema: Option<&Value>, user: String) -> Result<Running, ProviderError> {
        self.check_pause()?;
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| ProviderError::Unavailable("provider shut down".into()))?;
        let gate = self.gate.llm().await;
        // A pause may have started while this call waited for a permit.
        self.check_pause()?;
        let deadline = Instant::now() + self.cfg.timeout;
        let (program, launcher_args) = self
            .cfg
            .command
            .split_first()
            .ok_or_else(|| ProviderError::Unavailable("claude command is empty".into()))?;
        let mut child = Command::new(program)
            .args(launcher_args)
            .args(self.args(system, schema))
            .env_clear()
            .envs(&self.cfg.env)
            .current_dir(&self.cfg.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ProviderError::Unavailable(format!("failed to start claude: {:?}", e.kind())))?;
        tracing::debug!(provider = "claude_cli", pid = child.id(), "claude started");

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProviderError::Protocol("claude stdin missing".into()))?;
        // Written from a task so a child that answers before reading all input cannot deadlock.
        tokio::spawn(async move {
            let _ = stdin.write_all(user.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProviderError::Protocol("claude stdout missing".into()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| ProviderError::Protocol("claude stderr missing".into()))?;
        let limit = self.cfg.stderr_limit;
        let stderr_task = tokio::spawn(async move {
            let mut kept = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let room = limit.saturating_sub(kept.len());
                        kept.extend_from_slice(&buf[..n.min(room)]);
                    }
                }
            }
            kept
        });
        Ok(Running {
            pgid: child.id().and_then(|id| i32::try_from(id).ok()).and_then(rustix::process::Pid::from_raw),
            child,
            lines: BufReader::new(stdout).lines(),
            stderr_task: Some(stderr_task),
            deadline,
            timeout: self.cfg.timeout,
            kill_grace: self.cfg.kill_grace,
            model: None,
            rate_limit: None,
            _permit: permit,
            _gate: gate,
        })
    }

    fn record(&self, outcome: Result<(), &ProviderError>) {
        match outcome {
            Ok(()) => self.health.success(),
            Err(e) => {
                tracing::warn!(provider = "claude_cli", error = error_kind(e), "claude call failed");
                self.health.failure(e, self.clock.now());
            }
        }
    }

    fn default_pause(&self) -> chrono::Duration {
        chrono::Duration::from_std(self.cfg.default_pause).unwrap_or(chrono::Duration::minutes(30))
    }
}

/// One running `claude` process and everything that must live as long as it does.
struct Running {
    child: Child,
    pgid: Option<rustix::process::Pid>,
    lines: Lines<BufReader<ChildStdout>>,
    stderr_task: Option<JoinHandle<Vec<u8>>>,
    deadline: Instant,
    timeout: Duration,
    kill_grace: Duration,
    model: Option<String>,
    rate_limit: Option<RateLimitInfo>,
    _permit: OwnedSemaphorePermit,
    _gate: OwnedRwLockReadGuard<()>,
}

/// What the next stdout line meant.
enum Step {
    Text(String),
    Result(Box<ResultMsg>),
    Eof,
}

impl Running {
    /// Next meaningful line; `Err(Timeout)` after terminating the process group.
    async fn step(&mut self) -> Result<Step, ProviderError> {
        loop {
            let line = match tokio::time::timeout_at(self.deadline, self.lines.next_line()).await {
                Err(_) => {
                    self.terminate().await;
                    return Err(ProviderError::Timeout(self.timeout));
                }
                Ok(Err(e)) => {
                    return Err(ProviderError::Protocol(format!("reading claude output: {:?}", e.kind())));
                }
                Ok(Ok(None)) => return Ok(Step::Eof),
                Ok(Ok(Some(line))) => line,
            };
            match parse_line(&line)? {
                CliLine::Init { model } => self.model = model,
                CliLine::RateLimit(info) => self.rate_limit = Some(info),
                CliLine::Text(t) => return Ok(Step::Text(t)),
                CliLine::Result(r) => return Ok(Step::Result(r)),
                CliLine::Other => {}
            }
        }
    }

    /// Waits for exit (bounded by the deadline plus the grace period) and collects stderr.
    async fn finish(&mut self) -> (Option<i32>, Vec<u8>) {
        let wait_until = self.deadline + self.kill_grace;
        let code = if let Ok(Ok(status)) =
            tokio::time::timeout_at(wait_until, self.child.wait()).await
        {
            status.code()
        } else {
            self.terminate().await;
            None
        };
        let stderr = match self.stderr_task.take() {
            Some(t) => tokio::time::timeout(self.kill_grace, t)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        (code, stderr)
    }

    /// SIGTERM to the process group (a launcher such as sudo relays it), then SIGKILL after the
    /// grace period.
    async fn terminate(&mut self) {
        if let Some(pgid) = self.pgid {
            let _ = rustix::process::kill_process_group(pgid, rustix::process::Signal::TERM);
            if tokio::time::timeout(self.kill_grace, self.child.wait()).await.is_ok() {
                return;
            }
            let _ = rustix::process::kill_process_group(pgid, rustix::process::Signal::KILL);
        } else {
            let _ = self.child.start_kill();
        }
        let _ = self.child.wait().await;
    }

    async fn failure(&mut self, result: Option<&ResultMsg>, now: chrono::DateTime<chrono::Utc>, default_pause: chrono::Duration) -> ProviderError {
        let (exit_code, stderr) = self.finish().await;
        classify_failure(
            &FailureContext {
                result,
                rate_limit: self.rate_limit.as_ref(),
                stderr: &stderr,
                exit_code,
            },
            now,
            default_pause,
        )
    }
}

/// The schema as `--json-schema` accepts it: the CLI validates schemas with its own default
/// draft and rejects a `$schema` URI it does not know (observed with Claude Code 2.1.283 and
/// draft 2020-12), so the meta-schema reference is dropped. Every other keyword is kept.
pub fn cli_schema(schema: &Value) -> Value {
    let mut s = schema.clone();
    if let Value::Object(map) = &mut s {
        map.remove("$schema");
    }
    s
}

/// The JSON value of a successful result: `structured_output`, else the `result` text parsed as
/// JSON (a fenced code block is unwrapped).
fn result_value(r: &ResultMsg) -> Result<Value, ProviderError> {
    if let Some(v) = &r.structured_output {
        return Ok(v.clone());
    }
    let text = r
        .result
        .as_deref()
        .ok_or_else(|| ProviderError::Protocol("claude result has no output".into()))?
        .trim();
    let inner = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|t| t.strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    serde_json::from_str(inner)
        .map_err(|e| ProviderError::NotJson(e.to_string()))
}

fn succeeded(r: &ResultMsg) -> bool {
    !r.is_error && r.subtype == "success"
}

#[async_trait::async_trait]
impl LlmProvider for ClaudeCliProvider {
    fn name(&self) -> &'static str {
        "claude_cli"
    }

    fn model(&self) -> &str {
        &self.model_label
    }

    fn health(&self) -> ProviderHealth {
        self.health.snapshot(self.clock.now())
    }

    async fn complete_json(&self, req: JsonRequest) -> Result<JsonCompletion, ProviderError> {
        let outcome = async {
            let mut run = self.start(&req.system, Some(&req.schema), req.user).await?;
            loop {
                match run.step().await? {
                    Step::Text(_) => {}
                    Step::Result(r) if succeeded(&r) => {
                        let value = result_value(&r)?;
                        let model = r
                            .model
                            .clone()
                            .or_else(|| run.model.clone())
                            .unwrap_or_else(|| self.model_label.clone());
                        let _ = run.finish().await;
                        return Ok(JsonCompletion { value, usage: r.usage, model });
                    }
                    Step::Result(r) => {
                        return Err(run.failure(Some(&r), self.clock.now(), self.default_pause()).await);
                    }
                    Step::Eof => {
                        return Err(run.failure(None, self.clock.now(), self.default_pause()).await);
                    }
                }
            }
        }
        .await;
        self.record(outcome.as_ref().map(|_| ()));
        outcome
    }

    async fn stream(&self, req: ChatRequest) -> Result<TokenStream, ProviderError> {
        let run = match self.start(&req.system, None, req.user).await {
            Ok(run) => run,
            Err(e) => {
                self.record(Err(&e));
                return Err(e);
            }
        };
        let state = StreamState {
            run,
            done: false,
            clock: Arc::clone(&self.clock),
            health: Arc::clone(&self.health),
            default_pause: self.default_pause(),
            model_label: self.model_label.clone(),
        };
        Ok(Box::pin(stream::unfold(state, |mut st| async move {
            if st.done {
                return None;
            }
            let item = st.next_item().await;
            match &item {
                Ok(StreamEvent::Text(_)) => {}
                Ok(StreamEvent::Done { .. }) => {
                    st.done = true;
                    st.health.success();
                }
                Err(e) => {
                    st.done = true;
                    tracing::warn!(provider = "claude_cli", error = error_kind(e), "claude stream failed");
                    st.health.failure(e, st.clock.now());
                }
            }
            Some((item, st))
        })))
    }
}

struct StreamState {
    run: Running,
    done: bool,
    clock: Arc<dyn Clock>,
    health: Arc<HealthTracker>,
    default_pause: chrono::Duration,
    model_label: String,
}

impl StreamState {
    async fn next_item(&mut self) -> Result<StreamEvent, ProviderError> {
        match self.run.step().await? {
            Step::Text(t) => Ok(StreamEvent::Text(t)),
            Step::Result(r) if succeeded(&r) => {
                let usage: Usage = r.usage;
                let model = r
                    .model
                    .clone()
                    .or_else(|| self.run.model.clone())
                    .unwrap_or_else(|| self.model_label.clone());
                let _ = self.run.finish().await;
                Ok(StreamEvent::Done { usage, model })
            }
            Step::Result(r) => Err(self.run.failure(Some(&r), self.clock.now(), self.default_pause).await),
            Step::Eof => Err(self.run.failure(None, self.clock.now(), self.default_pause).await),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn config_rejects_api_key_bare_mode_and_empty_command() {
        let mut cfg = ClaudeCliConfig::new(vec!["/usr/bin/claude".into()], "/tmp".into());
        assert_eq!(cfg.validate(), Ok(()));
        cfg.env.insert("anthropic_api_key".into(), "sk".into());
        assert_eq!(
            cfg.validate(),
            Err(AiError::Config(
                "claude_cli.env must not set anthropic_api_key (billing must stay on the subscription login)".into()
            ))
        );
        cfg.env = ClaudeCliConfig::default_env();
        cfg.env.insert("CLAUDE_CODE_SIMPLE".into(), "1".into());
        assert!(cfg.validate().is_err());
        let empty = ClaudeCliConfig::new(vec![], "/tmp".into());
        assert_eq!(empty.validate(), Err(AiError::Config("claude_cli.command is empty".into())));
    }

    #[test]
    fn result_value_prefers_structured_output_then_parses_fenced_text() {
        let mut r = ResultMsg {
            is_error: false,
            subtype: "success".into(),
            result: Some("```json\n{\"a\": 1}\n```".into()),
            structured_output: None,
            usage: Usage::default(),
            model: None,
            api_error_status: None,
            errors: vec![],
        };
        assert_eq!(result_value(&r), Ok(serde_json::json!({"a": 1})));
        r.structured_output = Some(serde_json::json!({"b": 2}));
        assert_eq!(result_value(&r), Ok(serde_json::json!({"b": 2})));
        r.structured_output = None;
        r.result = Some("not json".into());
        assert_eq!(
            result_value(&r),
            Err(ProviderError::NotJson("expected ident at line 1 column 2".into()))
        );
    }
}
