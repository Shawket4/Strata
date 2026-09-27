//! Parsing of `claude -p --output-format stream-json --verbose` output (one JSON object per
//! line) and classification of failures, including subscription usage limits (→ pause).
//!
//! Line shapes (verified against Claude Code 2.1.283, see `tests/fixtures/claude_cli/`):
//! - `{"type":"system","subtype":"init","model":…}` first, then `system/status`, and
//!   `system/api_retry` before the CLI's own retries;
//! - `{"type":"stream_event","event":{…Messages API stream event…},"parent_tool_use_id":null}`
//!   with `--include-partial-messages` (text arrives as `content_block_delta`/`text_delta`);
//! - `{"type":"assistant","message":{…}}` per completed assistant message;
//! - `{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"|"rejected"|…,"resetsAt":<unix s>,…}}`;
//! - last, `{"type":"result","subtype":"success"|"error_…","is_error":…,"result":…,
//!   "structured_output":…,"usage":{…},"modelUsage":{"<model>":{…}},"total_cost_usd":…,
//!   "api_error_status":…}`.

use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc};
use serde_json::Value;

use crate::error::{PauseReason, ProviderError};
use crate::request::Usage;

/// One parsed output line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CliLine {
    /// `system/init`: the session's model.
    Init { model: Option<String> },
    /// A text delta of the main conversation.
    Text(String),
    /// A rate-limit report.
    RateLimit(RateLimitInfo),
    /// The final result.
    Result(Box<ResultMsg>),
    /// Anything else (status, retries, thinking, assistant snapshots, unknown future types).
    Other,
}

/// `rate_limit_event.rate_limit_info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RateLimitInfo {
    /// `allowed`, `allowed_warning`, `rejected`, …
    pub status: String,
    /// Unix seconds when the limit resets.
    pub resets_at: Option<i64>,
}

impl RateLimitInfo {
    pub(crate) fn rejected(&self) -> bool {
        self.status == "rejected"
    }
}

/// The final `result` line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResultMsg {
    pub is_error: bool,
    pub subtype: String,
    pub result: Option<String>,
    pub structured_output: Option<Value>,
    pub usage: Usage,
    pub model: Option<String>,
    pub api_error_status: Option<u64>,
    pub errors: Vec<String>,
}

/// Parses one line. Blank lines are `Other`; invalid JSON is an error.
pub(crate) fn parse_line(line: &str) -> Result<CliLine, ProviderError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(CliLine::Other);
    }
    let v: Value = serde_json::from_str(line)
        .map_err(|e| ProviderError::Protocol(format!("claude output line is not JSON: {e}")))?;
    let kind = v.get("type").and_then(Value::as_str).unwrap_or_default();
    Ok(match kind {
        "system" if v.get("subtype").and_then(Value::as_str) == Some("init") => CliLine::Init {
            model: v.get("model").and_then(Value::as_str).map(str::to_owned),
        },
        "stream_event" => {
            let top_level = v.get("parent_tool_use_id").is_none_or(Value::is_null);
            let ev = v.get("event").unwrap_or(&Value::Null);
            let delta = ev.get("delta").unwrap_or(&Value::Null);
            match (
                top_level,
                ev.get("type").and_then(Value::as_str),
                delta.get("type").and_then(Value::as_str),
                delta.get("text").and_then(Value::as_str),
            ) {
                (true, Some("content_block_delta"), Some("text_delta"), Some(text)) => {
                    CliLine::Text(text.to_owned())
                }
                _ => CliLine::Other,
            }
        }
        "rate_limit_event" => {
            let info = v.get("rate_limit_info").unwrap_or(&Value::Null);
            CliLine::RateLimit(RateLimitInfo {
                status: info
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                resets_at: info.get("resetsAt").and_then(Value::as_i64),
            })
        }
        "result" => CliLine::Result(Box::new(parse_result(&v))),
        _ => CliLine::Other,
    })
}

fn parse_result(v: &Value) -> ResultMsg {
    let u = v.get("usage").unwrap_or(&Value::Null);
    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
    let cost_micros = v
        .get("total_cost_usd")
        .and_then(Value::as_f64)
        .filter(|c| c.is_finite() && *c >= 0.0)
        .map(usd_to_micros);
    let model = v
        .get("modelUsage")
        .and_then(Value::as_object)
        .and_then(|m| m.keys().next().cloned());
    let errors = v
        .get("errors")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|e| e.as_str().map_or_else(|| e.to_string(), str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    ResultMsg {
        is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
        subtype: v
            .get("subtype")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        result: v.get("result").and_then(Value::as_str).map(str::to_owned),
        structured_output: v.get("structured_output").filter(|s| !s.is_null()).cloned(),
        usage: Usage {
            input_tokens: n("input_tokens"),
            cache_creation_input_tokens: n("cache_creation_input_tokens"),
            cache_read_input_tokens: n("cache_read_input_tokens"),
            output_tokens: n("output_tokens"),
            cost_micros,
        },
        model,
        api_error_status: v.get("api_error_status").and_then(Value::as_u64),
        errors,
    }
}

/// USD (float, as the CLI reports it) to integer micro-USD, rounded.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "callers pass finite, non-negative amounts; saturating float-to-int cast is intended"
)]
pub(crate) fn usd_to_micros(usd: f64) -> u64 {
    (usd * 1_000_000.0).round() as u64
}

/// Why a run failed, in the terms the provider reports.
#[derive(Debug, Default)]
pub(crate) struct FailureContext<'a> {
    pub result: Option<&'a ResultMsg>,
    pub rate_limit: Option<&'a RateLimitInfo>,
    pub stderr: &'a [u8],
    pub exit_code: Option<i32>,
}

/// Classifies a failed run. Usage limits become [`ProviderError::Paused`] with the reset time
/// from the rate-limit event, an epoch in the message (`…|1790553600`), or a clock time such
/// as `resets 3pm (Africa/Cairo)`; otherwise `now + default_pause`.
pub(crate) fn classify_failure(
    ctx: &FailureContext<'_>,
    now: DateTime<Utc>,
    default_pause: Duration,
) -> ProviderError {
    let mut text = String::new();
    if let Some(r) = ctx.result {
        text.push_str(r.result.as_deref().unwrap_or_default());
        for e in &r.errors {
            text.push('\n');
            text.push_str(e);
        }
    }
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(ctx.stderr));
    let lower = text.to_lowercase();
    let status = ctx.result.and_then(|r| r.api_error_status);

    let rejected = ctx.rate_limit.filter(|r| r.rejected());
    if rejected.is_some() || status == Some(429) || is_usage_limit(&lower) {
        let until = rejected
            .and_then(|r| r.resets_at)
            .and_then(|s| DateTime::from_timestamp(s, 0))
            .or_else(|| epoch_after_pipe(&text))
            .or_else(|| reset_clock_time(&lower, now))
            .unwrap_or(now + default_pause);
        return ProviderError::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(until),
        };
    }
    if matches!(status, Some(401 | 403)) || is_auth_failure(&lower) {
        return ProviderError::Auth;
    }
    if let Some(r) = ctx.result {
        if r.subtype.starts_with("error_") {
            return ProviderError::Unavailable(format!("claude run failed ({})", r.subtype));
        }
        if let Some(s) = status {
            return ProviderError::Unavailable(format!("claude API error status {s}"));
        }
        return ProviderError::Unavailable("claude reported an error".into());
    }
    match ctx.exit_code {
        Some(code) => ProviderError::Unavailable(format!("claude exited with status {code}")),
        None => ProviderError::Unavailable("claude was terminated by a signal".into()),
    }
}

const USAGE_LIMIT_MARKERS: &[&str] = &[
    "usage limit",
    "limit reached",
    "hit your limit",
    "out of extra usage",
    "limit will reset",
];

fn is_usage_limit(lower: &str) -> bool {
    USAGE_LIMIT_MARKERS.iter().any(|m| lower.contains(m))
}

const AUTH_MARKERS: &[&str] = &[
    "not logged in",
    "please run /login",
    "invalid api key",
    "authentication_error",
    "oauth token has expired",
    "authentication failed",
];

fn is_auth_failure(lower: &str) -> bool {
    AUTH_MARKERS.iter().any(|m| lower.contains(m))
}

/// `Claude AI usage limit reached|1790553600` → that instant.
fn epoch_after_pipe(text: &str) -> Option<DateTime<Utc>> {
    let idx = text.find('|')?;
    let digits: String = text[idx + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.len() < 9 {
        return None;
    }
    DateTime::from_timestamp(digits.parse().ok()?, 0)
}

/// `resets 3pm (Africa/Cairo)` / `reset at 5:30 am (UTC)` → the next such wall-clock time after
/// `now` in that zone. Without a zone the time is ambiguous and `None` is returned.
fn reset_clock_time(lower: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let start = ["resets at ", "reset at ", "resets "]
        .iter()
        .find_map(|m| lower.find(m).map(|i| i + m.len()))?;
    let rest = lower[start..].trim_start();
    let hour_digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let mut hour: u32 = hour_digits.parse().ok()?;
    let mut rest = &rest[hour_digits.len()..];
    let mut minute = 0;
    if let Some(r) = rest.strip_prefix(':') {
        let m: String = r.chars().take_while(char::is_ascii_digit).collect();
        minute = m.parse().ok()?;
        rest = &r[m.len()..];
    }
    let rest = rest.trim_start();
    if let Some(r) = rest.strip_prefix("am") {
        hour %= 12;
        rest_after_meridiem(r)
    } else if let Some(r) = rest.strip_prefix("pm") {
        hour = hour % 12 + 12;
        rest_after_meridiem(r)
    } else {
        Some(rest)
    }
    .and_then(|after| {
        let tz_name = after.trim_start().strip_prefix('(')?.split(')').next()?;
        // Zone names are case-sensitive in the tz database; the text was lower-cased.
        let tz: chrono_tz::Tz = chrono_tz::TZ_VARIANTS
            .iter()
            .find(|z| z.name().eq_ignore_ascii_case(tz_name.trim()))
            .copied()?;
        let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
        let local_now = now.with_timezone(&tz);
        let mut day = local_now.date_naive();
        for _ in 0..3 {
            if let Some(candidate) = tz.from_local_datetime(&day.and_time(time)).earliest() {
                if candidate > local_now {
                    return Some(candidate.with_timezone(&Utc));
                }
            }
            day = day.succ_opt()?;
        }
        None
    })
}

fn rest_after_meridiem(r: &str) -> Option<&str> {
    Some(r)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn now() -> DateTime<Utc> {
        // 2026-09-27T12:00:00Z
        DateTime::from_timestamp(1_790_510_400, 0).expect("valid")
    }

    fn result(text: &str, status: Option<u64>) -> ResultMsg {
        ResultMsg {
            is_error: true,
            subtype: "success".into(),
            result: Some(text.into()),
            structured_output: None,
            usage: Usage::default(),
            model: None,
            api_error_status: status,
            errors: vec![],
        }
    }

    fn classify(r: Option<&ResultMsg>, rl: Option<&RateLimitInfo>, stderr: &str) -> ProviderError {
        classify_failure(
            &FailureContext {
                result: r,
                rate_limit: rl,
                stderr: stderr.as_bytes(),
                exit_code: Some(1),
            },
            now(),
            Duration::minutes(30),
        )
    }

    fn paused_until(s: &str) -> ProviderError {
        ProviderError::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(s.parse().expect("rfc3339")),
        }
    }

    #[test]
    fn text_delta_of_the_main_conversation_only() {
        let main = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hi"}},"parent_tool_use_id":null}"#;
        assert_eq!(parse_line(main).expect("ok"), CliLine::Text("Hi".into()));
        let sub = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"x"}},"parent_tool_use_id":"toolu_1"}"#;
        assert_eq!(parse_line(sub).expect("ok"), CliLine::Other);
        let thinking = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}},"parent_tool_use_id":null}"#;
        assert_eq!(parse_line(thinking).expect("ok"), CliLine::Other);
        assert_eq!(parse_line("  ").expect("ok"), CliLine::Other);
        assert_eq!(
            parse_line("not json"),
            Err(ProviderError::Protocol(
                "claude output line is not JSON: expected ident at line 1 column 2".into()
            ))
        );
    }

    #[test]
    fn rejected_rate_limit_event_pauses_until_its_reset() {
        let rl = RateLimitInfo {
            status: "rejected".into(),
            resets_at: Some(1_790_528_400),
        };
        assert_eq!(
            classify(Some(&result("You've hit your limit", Some(429))), Some(&rl), ""),
            paused_until("2026-09-27T17:00:00Z")
        );
    }

    #[test]
    fn usage_limit_message_with_epoch_pauses_until_that_epoch() {
        assert_eq!(
            classify(Some(&result("Claude AI usage limit reached|1790528400", None)), None, ""),
            paused_until("2026-09-27T17:00:00Z")
        );
    }

    #[test]
    fn usage_limit_with_clock_time_and_zone_resolves_the_next_occurrence() {
        // 12:00Z = 15:00 in Cairo (UTC+3, EEST on 2026-09-27): 3pm has passed → next day.
        assert_eq!(
            classify(Some(&result("5-hour limit reached ∙ resets 3pm (Africa/Cairo)", None)), None, ""),
            paused_until("2026-09-28T12:00:00Z")
        );
        assert_eq!(
            classify(None, None, "You've hit your limit · resets 5:30pm (Europe/Berlin)"),
            paused_until("2026-09-27T15:30:00Z")
        );
        assert_eq!(
            classify(None, None, "Claude usage limit reached. Your limit will reset at 11 am (UTC)."),
            paused_until("2026-09-28T11:00:00Z")
        );
    }

    #[test]
    fn usage_limit_without_a_reset_time_pauses_for_the_default() {
        assert_eq!(
            classify(Some(&result("You've hit your limit", None)), None, ""),
            paused_until("2026-09-27T12:30:00Z")
        );
        // A 429 from the API behind the CLI is the subscription limit too.
        assert_eq!(
            classify(Some(&result("API Error: 429", Some(429))), None, ""),
            paused_until("2026-09-27T12:30:00Z")
        );
        // An allowed event does not pause by itself.
        let allowed = RateLimitInfo {
            status: "allowed".into(),
            resets_at: Some(1_790_528_400),
        };
        assert_eq!(
            classify(Some(&result("boom", None)), Some(&allowed), ""),
            ProviderError::Unavailable("claude reported an error".into())
        );
    }

    #[test]
    fn auth_and_other_failures_are_typed() {
        assert_eq!(
            classify(Some(&result("Not logged in · Please run /login", None)), None, ""),
            ProviderError::Auth
        );
        assert_eq!(classify(None, None, "Invalid API key"), ProviderError::Auth);
        let mut r = result("", Some(500));
        r.subtype = "error_during_execution".into();
        assert_eq!(
            classify(Some(&r), None, ""),
            ProviderError::Unavailable("claude run failed (error_during_execution)".into())
        );
        assert_eq!(
            classify(Some(&result("", Some(500))), None, ""),
            ProviderError::Unavailable("claude API error status 500".into())
        );
        assert_eq!(
            classify(None, None, "segfault"),
            ProviderError::Unavailable("claude exited with status 1".into())
        );
    }

    #[test]
    fn cost_is_rounded_to_micros() {
        assert_eq!(usd_to_micros(0.022_091_8), 22_092);
        assert_eq!(usd_to_micros(0.0), 0);
    }
}
