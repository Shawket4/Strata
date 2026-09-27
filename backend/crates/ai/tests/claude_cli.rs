//! `ClaudeCliProvider` against a fake `claude` executable that replays output recorded from
//! Claude Code 2.1.283 (`tests/fixtures/claude_cli/`).
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::{FutureExt, StreamExt};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::provider::HealthState;
use strata_ai::{
    ChatRequest, ClaudeCliConfig, ClaudeCliProvider, CpuGate, JsonCompletion, JsonRequest,
    LlmProvider, PauseReason, PromptRef, ProviderError, StreamEvent, Usage,
};
use strata_common::FakeClock;
use tempfile::TempDir;

const SYSTEM: &str = "You summarise notes. Reply with JSON.";

struct Harness {
    log: TempDir,
    work: TempDir,
    clock: FakeClock,
    gate: CpuGate,
}

impl Harness {
    fn new() -> Self {
        Self {
            log: TempDir::new().expect("log dir"),
            work: TempDir::new().expect("scratch dir"),
            clock: FakeClock::at_default_epoch(),
            gate: CpuGate::new(),
        }
    }

    fn config(&self, scenario: &str) -> ClaudeCliConfig {
        let mut cfg = ClaudeCliConfig::new(
            vec![
                common::fixture("claude_cli/fake-claude")
                    .display()
                    .to_string(),
            ],
            self.work.path().to_path_buf(),
        );
        cfg.env.insert(
            "FAKE_CLAUDE_LOG".into(),
            self.log.path().display().to_string(),
        );
        cfg.env
            .insert("FAKE_CLAUDE_SCENARIO".into(), scenario.into());
        cfg.model = Some("sonnet".into());
        cfg.timeout = Duration::from_secs(20);
        cfg.kill_grace = Duration::from_millis(200);
        cfg
    }

    fn provider(&self, cfg: ClaudeCliConfig) -> ClaudeCliProvider {
        ClaudeCliProvider::new(cfg, Arc::new(self.clock.clone()), self.gate.clone())
            .expect("provider")
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.log.path().join(name)).expect("log file")
    }

    fn calls(&self) -> u32 {
        std::fs::read_to_string(self.log.path().join("count"))
            .map_or(0, |s| s.trim().parse().expect("count"))
    }

    fn argv(&self) -> Vec<String> {
        split_nul(&std::fs::read(self.log.path().join("argv")).expect("argv"))
    }

    fn env(&self) -> Vec<String> {
        let mut e = split_nul(&std::fs::read(self.log.path().join("env")).expect("env"));
        e.sort();
        e
    }
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .map(|s| String::from_utf8(s.to_vec()).expect("utf-8"))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(String::is_empty)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {"summary": {"type": "string"}, "lang": {"type": "string", "enum": ["ar", "en", "mixed"]}},
        "required": ["summary", "lang"],
        "additionalProperties": false
    })
}

fn json_request(user: &str) -> JsonRequest {
    JsonRequest {
        prompt: PromptRef {
            id: "summary".into(),
            version: 1,
        },
        system: SYSTEM.into(),
        user: user.into(),
        schema: Arc::new(schema()),
        max_tokens: 4096,
        caller: common::caller("owner", 1),
    }
}

fn chat_request(user: &str) -> ChatRequest {
    ChatRequest {
        prompt: PromptRef {
            id: "ask".into(),
            version: 1,
        },
        system: SYSTEM.into(),
        user: user.into(),
        max_tokens: 4096,
        caller: common::caller("owner", 1),
    }
}

fn instant(s: &str) -> DateTime<Utc> {
    s.parse().expect("rfc3339")
}

#[tokio::test]
async fn json_call_runs_isolated_without_tools_or_api_key_and_parses_structured_output() {
    let h = Harness::new();
    let p = h.provider(h.config("json_success"));
    let user = "{\"note\": {\"text\": \"اتفقنا مع وطنية\"}}";
    let out = p.complete_json(json_request(user)).await.expect("success");

    assert_eq!(
        out,
        JsonCompletion {
            value: json!({"summary": "اتفقنا مع وطنية على فاتورة ETA شهرية.", "lang": "ar"}),
            usage: Usage {
                input_tokens: 801,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                output_tokens: 93,
                cost_micros: Some(2532),
            },
            model: "claude-sonnet-5".into(),
        }
    );
    // Exact command line: every tool off, no MCP, no settings files, no skills, no session
    // files, Strata's system prompt, the schema; no bare mode.
    assert_eq!(
        h.argv(),
        vec![
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
            SYSTEM,
            "--model",
            "sonnet",
            "--json-schema",
            // The `$schema` URI is dropped (the CLI rejects draft 2020-12's); the rest is kept.
            r#"{"additionalProperties":false,"properties":{"lang":{"enum":["ar","en","mixed"],"type":"string"},"summary":{"type":"string"}},"required":["summary","lang"],"type":"object"}"#,
        ]
    );
    // Exact environment: the parent's is cleared (no ANTHROPIC_API_KEY can leak through).
    let log = h.log.path().display().to_string();
    assert_eq!(
        h.env(),
        vec![
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1".to_owned(),
            "DISABLE_AUTOUPDATER=1".to_owned(),
            format!("FAKE_CLAUDE_LOG={log}"),
            "FAKE_CLAUDE_SCENARIO=json_success".to_owned(),
            "LANG=C.UTF-8".to_owned(),
            "PATH=/usr/local/bin:/usr/bin:/bin".to_owned(),
        ]
    );
    assert!(h.env().iter().all(|v| !v.starts_with("ANTHROPIC_")));
    // Content goes over stdin, never argv; the process runs in the scratch directory.
    assert_eq!(h.read("stdin"), user);
    assert!(!h.argv().iter().any(|a| a.contains("وطنية")));
    assert_eq!(
        h.read("cwd").trim(),
        h.work
            .path()
            .canonicalize()
            .expect("canonical")
            .display()
            .to_string()
    );
    assert_eq!(p.health().state, HealthState::Ready);
}

#[tokio::test]
async fn json_in_a_fenced_text_result_is_parsed_and_prose_is_not_json() {
    let h = Harness::new();
    let out = h
        .provider(h.config("json_in_text"))
        .complete_json(json_request("x"))
        .await
        .expect("success");
    assert_eq!(
        out.value,
        json!({"summary": "اتفقنا مع وطنية على فاتورة ETA شهرية.", "lang": "ar"})
    );

    let h = Harness::new();
    let err = h
        .provider(h.config("not_json"))
        .complete_json(json_request("x"))
        .await
        .expect_err("prose");
    assert_eq!(
        err,
        ProviderError::NotJson("expected value at line 1 column 1".into())
    );
}

#[tokio::test]
async fn stream_yields_main_conversation_text_then_done_with_usage() {
    let h = Harness::new();
    let p = h.provider(h.config("stream_success"));
    let items: Vec<_> = p
        .stream(chat_request("where is the contract?"))
        .await
        .expect("stream")
        .collect()
        .await;
    assert_eq!(
        items,
        vec![
            Ok(StreamEvent::Text("The contract is in the safe ".into())),
            Ok(StreamEvent::Text("[[Capture 2026-09-20#^c1d2]].".into())),
            Ok(StreamEvent::Done {
                usage: Usage {
                    input_tokens: 2,
                    cache_creation_input_tokens: 5290,
                    cache_read_input_tokens: 3289,
                    output_tokens: 27,
                    cost_micros: Some(22_092),
                },
                model: "claude-sonnet-5".into(),
            }),
        ]
    );
    let argv = h.argv();
    assert_eq!(
        argv.last().map(String::as_str),
        Some("--include-partial-messages")
    );
    assert!(!argv.iter().any(|a| a == "--json-schema"));
    assert_eq!(h.read("stdin"), "where is the contract?");
}

#[tokio::test]
async fn stream_failure_mid_way_ends_with_a_typed_error() {
    let h = Harness::new();
    let items: Vec<_> = h
        .provider(h.config("stream_error"))
        .stream(chat_request("q"))
        .await
        .expect("stream")
        .collect()
        .await;
    assert_eq!(
        items,
        vec![
            Ok(StreamEvent::Text("Partial".into())),
            Err(ProviderError::Unavailable(
                "claude run failed (error_during_execution)".into()
            )),
        ]
    );
}

#[tokio::test]
async fn usage_limit_pauses_until_reset_without_spawning_again_then_resumes() {
    let h = Harness::new();
    let p = h.provider(h.config("usage_limit_event"));
    let paused = ProviderError::Paused {
        reason: PauseReason::ProviderUsageLimit,
        until: Some(instant("2026-09-27T17:00:00Z")),
    };
    assert_eq!(
        p.complete_json(json_request("x")).await,
        Err(paused.clone())
    );
    assert_eq!(h.calls(), 1);
    assert_eq!(
        p.health().state,
        HealthState::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(instant("2026-09-27T17:00:00Z"))
        }
    );
    // While paused no process is started.
    assert_eq!(
        p.complete_json(json_request("x")).await,
        Err(paused.clone())
    );
    assert_eq!(p.stream(chat_request("q")).await.err(), Some(paused));
    assert_eq!(h.calls(), 1);

    // After the reset the provider tries again.
    h.clock.set(instant("2026-09-27T17:00:00Z"));
    let _ = p.complete_json(json_request("x")).await;
    assert_eq!(h.calls(), 2);
}

#[tokio::test]
async fn usage_limit_message_with_epoch_and_other_failures_are_classified() {
    let h = Harness::new();
    assert_eq!(
        h.provider(h.config("usage_limit_text"))
            .complete_json(json_request("x"))
            .await,
        Err(ProviderError::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(instant("2026-09-28T00:00:00Z")),
        })
    );
    let h = Harness::new();
    let p = h.provider(h.config("not_logged_in"));
    assert_eq!(
        p.complete_json(json_request("x")).await,
        Err(ProviderError::Auth)
    );
    assert_eq!(p.health().state, HealthState::Degraded);
    assert_eq!(p.health().last_error.as_deref(), Some("auth"));

    // A crash is reported by exit status; its stderr text never appears in the error.
    let h = Harness::new();
    let err = h
        .provider(h.config("crash"))
        .complete_json(json_request("x"))
        .await
        .expect_err("crash");
    assert_eq!(
        err,
        ProviderError::Unavailable("claude exited with status 3".into())
    );
    assert!(!err.to_string().contains("TypeError"));
}

async fn assert_killed_on_timeout(scenario: &str) {
    let h = Harness::new();
    let mut cfg = h.config(scenario);
    cfg.timeout = Duration::from_millis(500);
    let p = h.provider(cfg);
    assert_eq!(
        p.complete_json(json_request("x")).await,
        Err(ProviderError::Timeout(Duration::from_millis(500)))
    );
    let pid = h.read("pid").trim().to_owned();
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "process {pid} was killed and reaped"
    );
    assert_eq!(p.health().last_error.as_deref(), Some("timeout"));
    // The permit and the CPU gate are released.
    assert!(h.gate.embed().now_or_never().is_some());
}

#[tokio::test]
async fn timeout_terminates_the_process_group() {
    assert_killed_on_timeout("hang").await;
}

#[tokio::test]
async fn timeout_escalates_to_sigkill_when_sigterm_is_ignored() {
    assert_killed_on_timeout("hang_ignore_term").await;
}

#[tokio::test]
async fn concurrency_limit_serialises_calls_and_holds_the_cpu_gate() {
    let h = Harness::new();
    let p = Arc::new(h.provider(h.config("slow")));
    let (a, b) = tokio::join!(
        p.complete_json(json_request("a")),
        p.complete_json(json_request("b"))
    );
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(h.read("order"), "start\nend\nstart\nend\n");

    // While a call runs, an embedding batch cannot start.
    let h = Harness::new();
    let p = Arc::new(h.provider(h.config("slow")));
    let call = tokio::spawn({
        let p = Arc::clone(&p);
        async move { p.complete_json(json_request("a")).await }
    });
    while !h.log.path().join("order").exists() {
        tokio::task::yield_now().await;
    }
    assert!(
        h.gate.embed().now_or_never().is_none(),
        "embedding waits for claude"
    );
    call.await.expect("join").expect("ok");
    assert!(h.gate.embed().now_or_never().is_some());
}

#[tokio::test]
async fn spawn_failure_is_unavailable() {
    let h = Harness::new();
    let mut cfg = h.config("json_success");
    cfg.command = vec!["/nonexistent/claude".into()];
    assert_eq!(
        h.provider(cfg).complete_json(json_request("x")).await,
        Err(ProviderError::Unavailable(
            "failed to start claude: NotFound".into()
        ))
    );
}

#[tokio::test]
async fn a_launcher_prefix_runs_in_front_of_the_claude_arguments() {
    // Stand-in for `sudo -n -u strata-ai <wrapper>`: a launcher that adds one variable.
    let h = Harness::new();
    let mut cfg = h.config("json_success");
    cfg.command = vec![
        "/usr/bin/env".into(),
        "LAUNCHED_BY=launcher".into(),
        common::fixture("claude_cli/fake-claude")
            .display()
            .to_string(),
    ];
    cfg.model = None;
    h.provider(cfg)
        .complete_json(json_request("x"))
        .await
        .expect("success");
    assert_eq!(h.argv()[..3], ["-p", "--output-format", "stream-json"]);
    assert!(!h.argv().iter().any(|a| a == "--model"));
    assert!(h.env().contains(&"LAUNCHED_BY=launcher".to_owned()));
}
