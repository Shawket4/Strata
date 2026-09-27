//! `AnthropicApiProvider` against a local mock of the Messages API (wiremock), using response
//! shapes from the API documentation.
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages

mod common;

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use strata_ai::anthropic_api::ANTHROPIC_VERSION;
use strata_ai::retry::RecordingSleeper;
use strata_ai::{
    AiError, AnthropicApiConfig, AnthropicApiProvider, ApiKey, ChatRequest, JsonCompletion,
    JsonRequest, LlmProvider, PauseReason, PromptRef, ProviderError, StreamEvent, Usage,
};
use strata_common::FakeClock;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-ant-test-key";

fn schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "verdict": {"type": "string", "enum": ["duplicate", "distinct", "uncertain"]},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "reason": {"type": "string", "minLength": 1}
        },
        "required": ["verdict", "confidence", "reason"],
        "additionalProperties": false
    })
}

fn json_request() -> JsonRequest {
    JsonRequest {
        prompt: PromptRef {
            id: "duplicate_confirm".into(),
            version: 1,
        },
        system: "Decide duplicates.".into(),
        user: "{\"kind\":\"task\"}".into(),
        schema: Arc::new(schema()),
        max_tokens: 2048,
        caller: common::caller("bob", 2),
    }
}

fn provider(server: &MockServer, sleeper: &RecordingSleeper) -> AnthropicApiProvider {
    let cfg = AnthropicApiConfig {
        base_url: server.uri(),
        ..AnthropicApiConfig::default()
    };
    AnthropicApiProvider::new(
        cfg,
        ApiKey::new(KEY),
        Arc::new(FakeClock::at_default_epoch()),
        Arc::new(sleeper.clone()),
    )
    .expect("provider")
}

/// A Messages API response (documented shape) whose text block holds `text`.
fn message(text: &str, stop_reason: &str) -> Value {
    json!({
        "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
        "type": "message",
        "role": "assistant",
        "model": "claude-opus-5",
        "content": [
            {"type": "thinking", "thinking": "", "signature": "EqQB"},
            {"type": "text", "text": text}
        ],
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": 1200,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 400,
            "output_tokens": 80
        }
    })
}

fn error_body(kind: &str) -> Value {
    json!({"type": "error", "error": {"type": kind, "message": "details"}, "request_id": "req_1"})
}

#[tokio::test]
async fn structured_call_sends_the_documented_request_and_parses_the_reply() {
    let server = MockServer::start().await;
    let expected_body = json!({
        "model": "claude-opus-5",
        "max_tokens": 2048,
        "system": "Decide duplicates.",
        "messages": [{"role": "user", "content": "{\"kind\":\"task\"}"}],
        "output_config": {"format": {"type": "json_schema", "schema": {
            "type": "object",
            "properties": {
                "verdict": {"type": "string", "enum": ["duplicate", "distinct", "uncertain"]},
                "confidence": {"type": "number"},
                "reason": {"type": "string"}
            },
            "required": ["verdict", "confidence", "reason"],
            "additionalProperties": false
        }}}
    });
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", KEY))
        .and(header("anthropic-version", ANTHROPIC_VERSION))
        .and(header("content-type", "application/json"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(200).set_body_json(message(
            r#"{"verdict": "duplicate", "confidence": 0.83, "reason": "Same monthly invoice."}"#,
            "end_turn",
        )))
        .expect(1)
        .mount(&server)
        .await;
    let sleeper = RecordingSleeper::default();
    let out = provider(&server, &sleeper)
        .complete_json(json_request())
        .await
        .expect("ok");
    assert_eq!(
        out,
        JsonCompletion {
            value: json!({"verdict": "duplicate", "confidence": 0.83, "reason": "Same monthly invoice."}),
            usage: Usage {
                input_tokens: 1200,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 400,
                output_tokens: 80,
                // 1200 × $5/M + 400 × $0.5/M + 80 × $25/M = $0.0082
                cost_micros: Some(8200),
            },
            model: "claude-opus-5".into(),
        }
    );
    assert_eq!(sleeper.delays(), Vec::<Duration>::new());
}

#[tokio::test]
async fn overloaded_and_server_errors_are_retried_with_backoff_and_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(529).set_body_json(error_body("overloaded_error")))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("retry-after", "7")
                .set_body_json(error_body("api_error")),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(message(
            r#"{"verdict": "distinct", "confidence": 0.9, "reason": "Different people."}"#,
            "end_turn",
        )))
        .expect(1)
        .mount(&server)
        .await;
    let sleeper = RecordingSleeper::default();
    let out = provider(&server, &sleeper)
        .complete_json(json_request())
        .await
        .expect("ok");
    assert_eq!(out.value["verdict"], "distinct");
    assert_eq!(
        sleeper.delays(),
        vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(7)
        ]
    );
}

#[tokio::test]
async fn rate_limit_after_all_retries_becomes_a_pause() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .set_body_json(error_body("rate_limit_error")),
        )
        .expect(4)
        .mount(&server)
        .await;
    let sleeper = RecordingSleeper::default();
    let p = provider(&server, &sleeper);
    let err = p.complete_json(json_request()).await.expect_err("limited");
    assert_eq!(
        err,
        ProviderError::Paused {
            reason: PauseReason::ProviderRateLimit,
            until: Some("2026-09-27T12:00:30Z".parse().expect("rfc3339")),
        }
    );
    assert_eq!(sleeper.delays(), vec![Duration::from_secs(30); 3]);
    assert_eq!(
        AiError::from(err),
        AiError::Paused {
            reason: PauseReason::ProviderRateLimit,
            until: Some("2026-09-27T12:00:30Z".parse().expect("rfc3339")),
        }
    );
}

#[tokio::test]
async fn client_errors_are_not_retried() {
    for (status, kind, expected) in [
        (
            400,
            "invalid_request_error",
            ProviderError::Rejected("invalid_request_error (HTTP 400)".into()),
        ),
        (401, "authentication_error", ProviderError::Auth),
        (
            404,
            "not_found_error",
            ProviderError::Rejected("not_found_error (HTTP 404)".into()),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_json(error_body(kind)))
            .expect(1)
            .mount(&server)
            .await;
        let sleeper = RecordingSleeper::default();
        assert_eq!(
            provider(&server, &sleeper)
                .complete_json(json_request())
                .await,
            Err(expected)
        );
        assert_eq!(sleeper.delays(), Vec::<Duration>::new());
    }
}

#[tokio::test]
async fn truncated_refused_and_prose_replies_are_typed() {
    for (text, stop, expected) in [
        (
            r#"{"verdict": "dup"#,
            "max_tokens",
            ProviderError::Truncated,
        ),
        ("", "refusal", ProviderError::Refused),
        (
            "I think they are duplicates.",
            "end_turn",
            ProviderError::NotJson("expected value at line 1 column 1".into()),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(message(text, stop)))
            .mount(&server)
            .await;
        let sleeper = RecordingSleeper::default();
        assert_eq!(
            provider(&server, &sleeper)
                .complete_json(json_request())
                .await,
            Err(expected)
        );
    }
}

/// The streaming event sequence from the API documentation.
const SSE: &str = "event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-opus-5\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}

event: content_block_start
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}

event: ping
data: {\"type\": \"ping\"}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"العقد في الخزنة \"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"[[Capture 2026-09-20#^c1d2]]\"}}

event: content_block_stop
data: {\"type\":\"content_block_stop\",\"index\":0}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":15}}

event: message_stop
data: {\"type\":\"message_stop\"}

";

fn chat() -> ChatRequest {
    ChatRequest {
        prompt: PromptRef {
            id: "ask".into(),
            version: 1,
        },
        system: "Answer with citations.".into(),
        user: "{\"question\":\"فين العقد؟\"}".into(),
        max_tokens: 64000,
        caller: common::caller("bob", 2),
    }
}

#[tokio::test]
async fn stream_parses_sse_text_deltas_and_usage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(body_json(json!({
            "model": "claude-opus-5",
            "max_tokens": 64000,
            "system": "Answer with citations.",
            "messages": [{"role": "user", "content": "{\"question\":\"فين العقد؟\"}"}],
            "stream": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SSE, "text/event-stream"))
        .expect(1)
        .mount(&server)
        .await;
    let sleeper = RecordingSleeper::default();
    let items: Vec<_> = provider(&server, &sleeper)
        .stream(chat())
        .await
        .expect("stream")
        .collect()
        .await;
    assert_eq!(
        items,
        vec![
            Ok(StreamEvent::Text("العقد في الخزنة ".into())),
            Ok(StreamEvent::Text("[[Capture 2026-09-20#^c1d2]]".into())),
            Ok(StreamEvent::Done {
                usage: Usage {
                    input_tokens: 25,
                    output_tokens: 15,
                    // 25 × $5/M + 15 × $25/M = $0.0005
                    cost_micros: Some(500),
                    ..Usage::default()
                },
                model: "claude-opus-5".into(),
            }),
        ]
    );
}

#[tokio::test]
async fn stream_error_event_and_early_end_are_typed() {
    let error_sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5\",\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n\
event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
    let early = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n";
    for (body, last) in [
        (
            error_sse,
            ProviderError::Unavailable("overloaded_error (in stream)".into()),
        ),
        (
            early,
            ProviderError::Protocol("stream ended before message_stop".into()),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
            .mount(&server)
            .await;
        let sleeper = RecordingSleeper::default();
        let items: Vec<_> = provider(&server, &sleeper)
            .stream(chat())
            .await
            .expect("stream")
            .collect()
            .await;
        assert_eq!(items, vec![Ok(StreamEvent::Text("Hi".into())), Err(last)]);
    }
}

#[test]
fn api_key_file_must_be_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::TempDir::new().expect("dir");
    let path = dir.path().join("anthropic.key");
    std::fs::write(&path, "sk-ant-abc\n").expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    assert_eq!(
        ApiKey::from_file(&path),
        Err(AiError::Config(format!(
            "api key file {} has mode 644; it must not be readable by group or others (0600)",
            path.display()
        )))
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    assert_eq!(ApiKey::from_file(&path), Ok(ApiKey::new("sk-ant-abc")));
    std::fs::write(&path, "  \n").expect("write");
    assert_eq!(
        ApiKey::from_file(&path),
        Err(AiError::Config(format!(
            "api key file {} is empty",
            path.display()
        )))
    );
}
