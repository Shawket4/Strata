//! `AiService` with the fake provider: schema validation and its retry limit, budget accounting
//! and pauses across day boundaries (fake clock), routing per user, streaming usage, status, and
//! the fake provider's fixture keying.
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::NaiveDate;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::budget::{BudgetLimits, UsageTotals};
use strata_ai::fake::{FakeLlmProvider, Fixture};
use strata_ai::outputs::{Lang, Summary};
use strata_ai::prompts::{self, ids};
use strata_ai::provider::{HealthState, ProviderHealth};
use strata_ai::status::{AiStatus, PauseInfo, ProviderStatus, UsageStatus};
use strata_ai::{
    AiCaller, AiError, AiService, BudgetGuard, LlmProvider, MemoryUsageStore, PauseReason,
    ProviderError, ProviderRouter, StreamEvent, Usage,
};
use strata_common::FakeClock;
use strata_common::config::AiProviderKind;

struct World {
    clock: FakeClock,
    store: Arc<MemoryUsageStore>,
    cli: FakeLlmProvider,
    api: FakeLlmProvider,
    service: AiService,
}

fn world(limits: BudgetLimits, tz: chrono_tz::Tz) -> World {
    let clock = FakeClock::at_default_epoch();
    let store = Arc::new(MemoryUsageStore::default());
    let cli = FakeLlmProvider::new().named("claude_cli");
    let api = FakeLlmProvider::new().named("anthropic_api");
    let per_user: BTreeMap<String, AiProviderKind> = [
        ("bob".to_owned(), AiProviderKind::AnthropicApi),
        ("carol".to_owned(), AiProviderKind::Disabled),
    ]
    .into();
    let router = ProviderRouter::new(AiProviderKind::ClaudeCli, per_user)
        .with_provider(AiProviderKind::ClaudeCli, Arc::new(cli.clone()))
        .with_provider(AiProviderKind::AnthropicApi, Arc::new(api.clone()));
    let budget = BudgetGuard::new(limits, tz, Arc::new(clock.clone()), store.clone());
    World {
        clock,
        store,
        cli,
        api,
        service: AiService::new(router, budget),
    }
}

fn alice() -> AiCaller {
    common::caller("alice", 1)
}

fn bob() -> AiCaller {
    common::caller("bob", 2)
}

fn summary_request(caller: AiCaller) -> strata_ai::JsonRequest {
    prompts::latest(ids::SUMMARY)
        .expect("summary prompt")
        .json_request(
            caller,
            &json!({"note": {"id": "n1", "title": "Watanya", "text": "اتفقنا"}}),
            1024,
        )
        .expect("request")
}

fn valid_summary() -> serde_json::Value {
    json!({"summary": "اتفقنا مع وطنية على فاتورة شهرية.", "lang": "ar"})
}

fn usage(input: u64, output: u64) -> Usage {
    Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    }
}

#[tokio::test]
async fn valid_output_is_returned_typed_with_provenance_and_usage_is_recorded() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let req = summary_request(alice());
    w.cli.push(
        &req.prompt,
        &req.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(300, 40)),
    );
    let prompt = prompts::latest(ids::SUMMARY).expect("prompt");
    let out = w
        .service
        .complete::<Summary>(
            alice(),
            prompt,
            &json!({"note": {"id": "n1", "title": "Watanya", "text": "اتفقنا"}}),
            1024,
        )
        .await
        .expect("ok");
    assert_eq!(
        out.value,
        Summary {
            summary: "اتفقنا مع وطنية على فاتورة شهرية.".into(),
            lang: Lang::Ar
        }
    );
    assert_eq!(
        (out.provider, out.model.as_str(), out.attempts),
        ("claude_cli", "fake-model", 1)
    );
    assert_eq!(out.prompt.to_string(), "summary.v1");
    let day = NaiveDate::from_ymd_opt(2026, 9, 27).expect("date");
    assert_eq!(
        w.store.rows(),
        [(
            (
                common::user_id(1),
                day,
                "claude_cli".to_owned(),
                "fake-model".to_owned()
            ),
            UsageTotals {
                calls: 1,
                input_tokens: 300,
                output_tokens: 40,
                cost_micros: 0
            }
        )]
        .into()
    );
}

#[tokio::test]
async fn invalid_output_is_retried_with_feedback_and_succeeds_on_the_third_attempt() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let req = summary_request(alice());
    // Attempt 1: wrong enum; attempt 2: not JSON; attempt 3: valid.
    w.cli.push(
        &req.prompt,
        &req.input_hash(),
        Fixture::json(json!({"summary": "x", "lang": "fr"})),
    );
    let second_user = format!(
        "{}\n\n---\nYour previous reply did not match the required JSON schema:\n- /lang: \"fr\" is not one of \"ar\", \"en\" or \"mixed\"\n\nReply again with exactly one JSON object that matches the JSON schema supplied with this request.",
        req.user
    );
    let second_hash = strata_ai::request::input_hash(&req.system, &second_user);
    w.cli
        .push(&req.prompt, &second_hash, Fixture::error("not_json", None));
    let third_user = format!(
        "{}\n\n---\nYour previous reply was not a JSON value.\nReply again with exactly one JSON object that matches the JSON schema supplied with this request.",
        req.user
    );
    let third_hash = strata_ai::request::input_hash(&req.system, &third_user);
    w.cli
        .push(&req.prompt, &third_hash, Fixture::json(valid_summary()));

    let out = w
        .service
        .complete_json(req.clone())
        .await
        .expect("valid on attempt 3");
    assert_eq!(out.attempts, 3);
    assert_eq!(out.value, valid_summary());
    // Two attempts produced output (and were billed); the not-JSON one reported no usage.
    assert_eq!(out.usage, usage(200, 100));
    let calls = w.cli.calls();
    assert_eq!(
        calls
            .iter()
            .map(|c| c.input_hash.clone())
            .collect::<Vec<_>>(),
        vec![req.input_hash(), second_hash, third_hash]
    );
    assert_eq!(calls[1].user, second_user);
}

#[tokio::test]
async fn output_invalid_three_times_fails_with_a_typed_error_after_two_retries() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let req = summary_request(alice());
    let bad = Fixture::json(json!({"summary": "", "lang": "ar", "extra": 1}));
    // Every attempt (whatever its feedback) replays the same invalid output.
    let mut user = req.user.clone();
    let feedback = "\n\n---\nYour previous reply did not match the required JSON schema:\n- /summary: \"\" is shorter than 1 character\n- /: Additional properties are not allowed ('extra' was unexpected)\n\nReply again with exactly one JSON object that matches the JSON schema supplied with this request.";
    for _ in 0..3 {
        w.cli.push(
            &req.prompt,
            &strata_ai::request::input_hash(&req.system, &user),
            bad.clone(),
        );
        user = format!("{}{feedback}", req.user);
    }
    let err = w.service.complete_json(req).await.expect_err("invalid");
    assert_eq!(
        err,
        AiError::InvalidOutput {
            prompt_id: "summary".into(),
            version: 1,
            attempts: 3,
            errors: vec![
                "/summary: value is shorter than 1 character".into(),
                "/: Additional properties are not allowed ('extra' was unexpected)".into(),
            ],
        }
    );
    assert_eq!(w.cli.calls().len(), 3);
    assert_eq!(w.store.rows().values().map(|t| t.calls).sum::<u64>(), 3);
}

#[tokio::test]
async fn provider_failures_other_than_invalid_output_are_not_retried() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let req = summary_request(alice());
    let until = "2026-09-27T17:00:00Z".parse().expect("rfc3339");
    w.cli.push(
        &req.prompt,
        &req.input_hash(),
        Fixture::error("paused", Some(until)),
    );
    let err = w.service.complete_json(req).await.expect_err("paused");
    assert_eq!(
        err,
        AiError::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: Some(until)
        }
    );
    assert!(err.is_pause());
    assert_eq!(w.cli.calls().len(), 1);
}

#[tokio::test]
async fn routing_follows_the_per_user_configuration() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let a = summary_request(alice());
    let b = summary_request(bob());
    // Same input → same hash; each provider has its own fixture.
    w.cli
        .push(&a.prompt, &a.input_hash(), Fixture::json(valid_summary()));
    w.api
        .push(&b.prompt, &b.input_hash(), Fixture::json(valid_summary()));
    assert_eq!(
        w.service.complete_json(a).await.expect("alice").provider,
        "claude_cli"
    );
    assert_eq!(
        w.service.complete_json(b).await.expect("bob").provider,
        "anthropic_api"
    );
    assert_eq!((w.cli.calls().len(), w.api.calls().len()), (1, 1));
    assert_eq!(
        w.service
            .complete_json(summary_request(common::caller("carol", 3)))
            .await,
        Err(AiError::Disabled)
    );
    let only_cli = ProviderRouter::new(AiProviderKind::AnthropicApi, BTreeMap::new())
        .with_provider(AiProviderKind::ClaudeCli, Arc::new(FakeLlmProvider::new()));
    assert_eq!(
        only_cli.route("dave").err(),
        Some(AiError::ProviderNotConfigured("anthropic_api"))
    );
    assert_eq!(only_cli.kind_for("dave"), AiProviderKind::AnthropicApi);
    let from_config = ProviderRouter::from_config(&strata_common::Config::default().ai);
    assert_eq!(from_config.kind_for("anyone"), AiProviderKind::ClaudeCli);
}

fn caps(per_user: u64, global: u64) -> BudgetLimits {
    BudgetLimits {
        per_user_daily_tokens: per_user,
        global_daily_tokens: global,
        ..BudgetLimits::default()
    }
}

#[tokio::test]
async fn per_user_cap_pauses_that_user_until_midnight_and_resumes_the_next_day() {
    let w = world(caps(1000, 0), chrono_tz::UTC);
    let a = summary_request(alice());
    w.cli.push(
        &a.prompt,
        &a.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(900, 100)),
    );
    let b = summary_request(bob());
    w.api.push(
        &b.prompt,
        &b.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(10, 10)),
    );

    w.service
        .complete_json(a.clone())
        .await
        .expect("first call is under the cap");
    let paused = AiError::Paused {
        reason: PauseReason::UserBudget,
        until: Some("2026-09-28T00:00:00Z".parse().expect("rfc3339")),
    };
    assert_eq!(
        w.service.complete_json(a.clone()).await,
        Err(paused.clone())
    );
    assert_eq!(w.cli.calls().len(), 1, "no provider call while paused");
    // Other users are unaffected.
    w.service.complete_json(b).await.expect("bob still works");

    // One second before midnight: still paused. At midnight: a new budget day.
    w.clock
        .set("2026-09-27T23:59:59Z".parse().expect("rfc3339"));
    assert_eq!(w.service.complete_json(a.clone()).await, Err(paused));
    w.clock
        .set("2026-09-28T00:00:00Z".parse().expect("rfc3339"));
    w.service.complete_json(a).await.expect("resumed");
    assert_eq!(w.cli.calls().len(), 2);
}

#[tokio::test]
async fn global_cap_pauses_everyone_and_days_follow_the_budget_timezone() {
    let w = world(caps(0, 500), chrono_tz::Africa::Cairo);
    let a = summary_request(alice());
    w.cli.push(
        &a.prompt,
        &a.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(400, 100)),
    );
    w.service
        .complete_json(a.clone())
        .await
        .expect("under the global cap");

    // Cairo is UTC+3 on 2026-09-27: the budget day ends at 21:00Z.
    let until = Some("2026-09-27T21:00:00Z".parse().expect("rfc3339"));
    let global = AiError::Paused {
        reason: PauseReason::GlobalBudget,
        until,
    };
    assert_eq!(
        w.service.complete_json(a.clone()).await,
        Err(global.clone())
    );
    assert_eq!(
        w.service.complete_json(summary_request(bob())).await,
        Err(global)
    );
    assert_eq!(
        w.service.budget().day(),
        NaiveDate::from_ymd_opt(2026, 9, 27).expect("date")
    );

    w.clock
        .set("2026-09-27T21:00:00Z".parse().expect("rfc3339"));
    assert_eq!(
        w.service.budget().day(),
        NaiveDate::from_ymd_opt(2026, 9, 28).expect("date")
    );
    w.service.complete_json(a).await.expect("new day in Cairo");
}

#[tokio::test]
async fn cost_cap_counts_estimated_micros() {
    let limits = BudgetLimits {
        per_user_daily_cost_micros: 10_000,
        ..BudgetLimits::default()
    };
    let w = world(limits, chrono_tz::UTC);
    let a = summary_request(alice());
    let costly = Usage {
        cost_micros: Some(10_000),
        ..usage(1, 1)
    };
    w.cli.push(
        &a.prompt,
        &a.input_hash(),
        Fixture::json(valid_summary()).with_usage(costly),
    );
    w.service.complete_json(a.clone()).await.expect("first");
    assert_eq!(
        w.service.complete_json(a).await.err().map(|e| e.is_pause()),
        Some(true)
    );
}

#[tokio::test]
async fn stream_records_usage_when_done() {
    let w = world(BudgetLimits::default(), chrono_tz::UTC);
    let req = prompts::latest(ids::ASK)
        .expect("ask")
        .chat_request(
            alice(),
            &json!({"question": {"text": "فين العقد؟"}, "sources": []}),
            4096,
        )
        .expect("request");
    w.cli.push(
        &req.prompt,
        &req.input_hash(),
        Fixture::stream(&["العقد ", "في الخزنة"]).with_usage(usage(70, 7)),
    );
    let items: Vec<_> = w.service.stream(req).await.expect("stream").collect().await;
    assert_eq!(
        items,
        vec![
            Ok(StreamEvent::Text("العقد ".into())),
            Ok(StreamEvent::Text("في الخزنة".into())),
            Ok(StreamEvent::Done {
                usage: usage(70, 7),
                model: "fake-model".into()
            }),
        ]
    );
    assert_eq!(
        w.store.rows().values().copied().collect::<Vec<_>>(),
        vec![UsageTotals {
            calls: 1,
            input_tokens: 70,
            output_tokens: 7,
            cost_micros: 0
        }]
    );
}

#[tokio::test]
async fn status_reports_provider_usage_budget_and_pause() {
    let w = world(caps(1000, 5000), chrono_tz::UTC);
    let a = summary_request(alice());
    w.cli.push(
        &a.prompt,
        &a.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(990, 10)),
    );
    let b = summary_request(bob());
    w.api.push(
        &b.prompt,
        &b.input_hash(),
        Fixture::json(valid_summary()).with_usage(usage(20, 5)),
    );
    w.service.complete_json(a).await.expect("alice");
    w.service.complete_json(b).await.expect("bob");

    let day = NaiveDate::from_ymd_opt(2026, 9, 27).expect("date");
    assert_eq!(
        w.service.status(&alice()).await.expect("status"),
        AiStatus {
            enabled: true,
            provider: Some(ProviderStatus {
                name: "claude_cli".into(),
                model: "fake-model".into(),
                health: ProviderHealth::ready(),
            }),
            paused: Some(PauseInfo {
                reason: PauseReason::UserBudget,
                until: Some("2026-09-28T00:00:00Z".parse().expect("rfc3339")),
            }),
            queue_depth: None,
            failed_jobs: None,
            usage: UsageStatus {
                day,
                user: UsageTotals {
                    calls: 1,
                    input_tokens: 990,
                    output_tokens: 10,
                    cost_micros: 0
                },
                global: UsageTotals {
                    calls: 2,
                    input_tokens: 1010,
                    output_tokens: 15,
                    cost_micros: 0
                },
            },
            limits: caps(1000, 5000),
            embeddings: None,
            embedding_progress: None,
        }
    );
    let carol = w
        .service
        .status(&common::caller("carol", 3))
        .await
        .expect("status");
    assert_eq!(
        (carol.enabled, carol.provider, carol.paused),
        (false, None, None)
    );
    assert_eq!(
        serde_json::to_value(w.service.status(&bob()).await.expect("status")).expect("json")["provider"],
        json!({"name": "anthropic_api", "model": "fake-model", "health": {"state": {"state": "ready"}, "last_error": null, "last_error_at": null}})
    );
    // Health states serialise with a tag.
    assert_eq!(
        serde_json::to_value(HealthState::Paused {
            reason: PauseReason::ProviderUsageLimit,
            until: None
        })
        .expect("json"),
        json!({"state": "paused", "reason": "provider_usage_limit", "until": null})
    );
}

#[tokio::test]
async fn fake_provider_fails_loudly_with_the_hash_and_reads_fixture_files() {
    let dir = tempfile::TempDir::new().expect("dir");
    let fake = FakeLlmProvider::from_dir(dir.path());
    let req = summary_request(alice());
    let hash = req.input_hash();
    let expected_path = dir
        .path()
        .join("summary")
        .join("v1")
        .join(format!("{hash}.json"));
    assert_eq!(
        fake.complete_json(req.clone()).await,
        Err(ProviderError::MissingFixture {
            prompt_id: "summary".into(),
            version: 1,
            input_hash: hash.clone(),
            path: expected_path.display().to_string(),
        })
    );
    let message = fake
        .complete_json(req.clone())
        .await
        .expect_err("missing")
        .to_string();
    assert!(message.contains(&hash) && message.contains("record it at"));

    std::fs::create_dir_all(expected_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        &expected_path,
        serde_json::to_string(&Fixture::json(valid_summary())).expect("json"),
    )
    .expect("write");
    assert_eq!(
        fake.complete_json(req.clone())
            .await
            .expect("fixture")
            .value,
        valid_summary()
    );

    // The key includes the prompt version and the exact input.
    let mut other_version = req.clone();
    other_version.prompt.version = 2;
    assert!(matches!(
        fake.complete_json(other_version).await,
        Err(ProviderError::MissingFixture { version: 2, .. })
    ));
    let mut other_input = req;
    other_input.user.push(' ');
    assert!(matches!(
        fake.complete_json(other_input).await,
        Err(ProviderError::MissingFixture { .. })
    ));
}

#[tokio::test]
async fn testkit_reexports_the_fake_provider() {
    let fake: strata_testkit::FakeLlmProvider = strata_testkit::FakeLlmProvider::new();
    let req = summary_request(alice());
    fake.push(
        &req.prompt,
        &req.input_hash(),
        strata_testkit::Fixture::json(valid_summary()),
    );
    assert_eq!(
        fake.complete_json(req).await.expect("ok").value,
        valid_summary()
    );
}

#[tokio::test]
async fn admins_are_exempt_from_every_cap_and_their_usage_still_counts() {
    // The per-user cap is spent by the admin's first call and the global cap by the second;
    // the admin keeps working while everyone else is paused on the global cap.
    let w = world(caps(100, 300), chrono_tz::UTC);
    w.store.exempt(alice().scope.user_id());
    let a = summary_request(alice());
    for _ in 0..3 {
        w.cli.push(
            &a.prompt,
            &a.input_hash(),
            Fixture::json(valid_summary()).with_usage(usage(150, 50)),
        );
    }
    for n in 1..=3 {
        w.service
            .complete_json(a.clone())
            .await
            .unwrap_or_else(|e| panic!("admin call {n}: {e:?}"));
    }
    assert_eq!(
        w.service.complete_json(summary_request(bob())).await,
        Err(AiError::Paused {
            reason: PauseReason::GlobalBudget,
            until: Some("2026-09-28T00:00:00Z".parse().expect("rfc3339")),
        })
    );
    let status = w.service.status(&alice()).await.expect("status");
    assert_eq!(
        (status.paused, status.limits, status.usage.user.tokens()),
        (None, BudgetLimits::default(), 600),
        "no pause and no limit shown to the admin; the usage is recorded"
    );
    assert_eq!(
        w.service.status(&bob()).await.expect("status").limits,
        caps(100, 300)
    );
}
