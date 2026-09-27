//! Ask (PLAN §9.5): hybrid retrieval into the prompt, the streamed answer, citations resolved
//! to real blocks (IDs appended when missing, one `ai:` commit), unknown refs dropped, and the
//! unavailable / paused cases.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod common;

use std::sync::{Arc, Mutex};

use common::{World, fresh_ids};
use pretty_assertions::assert_eq;
use strata_ai::prompts::{self, ids};
use strata_ai::request::input_hash;
use strata_ai::{BudgetLimits, PauseReason};
use strata_jobs::ask::{AskConfig, AskEngine, AskError, AskEvent, Citation};
use strata_jobs::retrieval::Retriever;
use strata_testkit::Fixture;

fn engine(w: &World) -> AskEngine {
    AskEngine::new(
        w.db.app_db.clone(),
        w.vault.clone(),
        w.ai.clone(),
        Some(Retriever::new(w.db.app_db.clone(), w.embedder_arc())),
        fresh_ids(),
        Arc::new(w.db.clock.clone()),
        AskConfig {
            top_k: 3,
            ..AskConfig::default()
        },
    )
}

const QUESTION: &str = "What did Acme ask for on the call?";

#[tokio::test]
async fn answers_stream_then_citations_resolve_to_blocks_with_ids_appended_in_one_commit() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let call = w
        .create(
            &sa,
            "notes/Call 2026-09-12.md",
            "Acme prefers weekly invoicing.\n\nThey asked for a discount.\n",
        )
        .await;
    let pricing = w
        .create(
            &sa,
            "notes/Pricing.md",
            "Discounts are capped at 5%. ^cap\n",
        )
        .await;
    w.create(&sa, "notes/Banana.md", "Banana bread with walnuts.\n")
        .await;
    w.standard_runner().run_until_idle().await;
    let commits = w.log(a).len();

    // First attempt: no fixture yet; the recorded prompt input shows the sources.
    let err = engine(&w)
        .start(sa, "alice".into(), QUESTION, None)
        .await
        .expect_err("no fixture");
    assert!(matches!(err, AskError::Unavailable(_)), "{err:?}");
    let recorded = w.llm.calls().last().cloned().expect("a call");
    assert_eq!(
        recorded.prompt,
        prompts::latest(ids::ASK).expect("ask").prompt_ref()
    );
    let input: serde_json::Value = serde_json::from_str(&recorded.user).expect("json");
    assert_eq!(
        input["question"],
        serde_json::json!({"text": QUESTION, "asked_at": "2026-09-27T12:00:00+00:00"})
    );
    let mut sources: Vec<(String, String, String)> = input["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .map(|s| {
            assert_eq!(s["created"], "2026-09-27T12:00:00+00:00");
            (
                s["ref"].as_str().expect("ref").to_owned(),
                s["note_title"].as_str().expect("title").to_owned(),
                s["text"].as_str().expect("text").to_owned(),
            )
        })
        .collect();
    sources.sort_by(|x, y| x.1.cmp(&y.1));
    let call_ref = sources
        .iter()
        .find(|s| s.1 == "Call 2026-09-12")
        .expect("call source")
        .0
        .clone();
    assert!(call_ref.starts_with("Call 2026-09-12#^ask-"), "{call_ref}");
    let new_id = call_ref.trim_start_matches("Call 2026-09-12#^").to_owned();
    assert_eq!(
        sources
            .iter()
            .map(|s| (s.1.as_str(), s.2.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Banana", "Banana bread with walnuts."),
            (
                "Call 2026-09-12",
                "Acme prefers weekly invoicing.\n\nThey asked for a discount."
            ),
            ("Pricing", "Discounts are capped at 5%."),
        ]
    );
    assert!(
        sources.iter().any(|s| s.0 == "Pricing#^cap"),
        "existing IDs are reused"
    );
    assert_eq!(
        w.log(a).len(),
        commits,
        "nothing is written before the answer"
    );

    // Second attempt with the same IDs: the recorded input has a fixture now.
    let system = prompts::latest(ids::ASK).expect("ask").text;
    let tokens = [
        "Acme wants weekly invoicing ".to_owned(),
        format!("[[{call_ref}]]"),
        ". Discounts are capped at 5% [[Pricing#^cap]]".to_owned(),
        " [[Invented]].".to_owned(),
    ];
    let token_refs: Vec<&str> = tokens.iter().map(String::as_str).collect();
    w.llm.push(
        &recorded.prompt,
        &input_hash(system, &recorded.user),
        Fixture::stream(&token_refs),
    );
    let run = engine(&w)
        .start(sa, "alice".into(), QUESTION, None)
        .await
        .expect("started");
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    run.run(move |e| sink.lock().expect("events").push(e)).await;
    let events = events.lock().expect("events").clone();
    let answer_raw = tokens.concat();
    assert_eq!(
        events,
        vec![
            AskEvent::Tokens(answer_raw),
            AskEvent::Citation(Citation {
                index: 1,
                reference: call_ref.clone(),
                note_id: call,
                path: "notes/Call 2026-09-12.md".into(),
                title: "Call 2026-09-12".into(),
                block_id: Some(new_id.clone()),
                target: call_ref.clone(),
            }),
            AskEvent::Citation(Citation {
                index: 2,
                reference: "Pricing#^cap".into(),
                note_id: pricing,
                path: "notes/Pricing.md".into(),
                title: "Pricing".into(),
                block_id: Some("cap".into()),
                target: "Pricing#^cap".into(),
            }),
            AskEvent::Done {
                answer: format!(
                    "Acme wants weekly invoicing [[{call_ref}]]. Discounts are capped at 5% \
                     [[Pricing#^cap]] Invented."
                ),
            },
        ]
    );
    // Exactly one commit: the cited block got its ID; the uncited ones did not.
    assert_eq!(w.log(a).len(), commits + 1);
    assert_eq!(w.log(a)[0], "ai: ask notes/Call 2026-09-12.md");
    assert_eq!(
        w.last_commit_paths(a),
        vec!["notes/Call 2026-09-12.md".to_owned()]
    );
    let text = w.read(a, "notes/Call 2026-09-12.md");
    assert!(
        text.ends_with(&format!(
            "Acme prefers weekly invoicing. ^{new_id}\n\nThey asked for a discount.\n"
        )),
        "{text}"
    );
    assert!(!w.read(a, "notes/Banana.md").contains('^'));
    w.finish().await;
}

#[tokio::test]
async fn ask_is_refused_when_ai_is_off_or_paused_and_works_without_embeddings() {
    let w = World::with_limits(BudgetLimits {
        per_user_daily_tokens: 1,
        ..BudgetLimits::default()
    })
    .await;
    let (_, sn) = w.user("noai").await;
    let err = engine(&w)
        .start(sn, "noai".into(), QUESTION, None)
        .await
        .expect_err("disabled");
    assert_eq!(
        err,
        AskError::Unavailable("AI is disabled for this account".into())
    );
    let (_, sa) = w.user("alice").await;
    assert_eq!(
        engine(&w)
            .start(sa, "alice".into(), "  ", None)
            .await
            .expect_err("empty"),
        AskError::EmptyQuestion
    );
    // Keyword-only retrieval (no embedder): the call reaches the provider; its usage then
    // exhausts the 1-token budget, so the next ask is paused until tomorrow.
    w.create(&sa, "notes/Acme.md", "Acme prefers weekly invoicing.\n")
        .await;
    let keyword_only = AskEngine::new(
        w.db.app_db.clone(),
        w.vault.clone(),
        w.ai.clone(),
        None,
        fresh_ids(),
        Arc::new(w.db.clock.clone()),
        AskConfig::default(),
    );
    let first = keyword_only
        .start(sa, "alice".into(), "weekly invoicing", None)
        .await
        .expect_err("no fixture");
    assert!(matches!(first, AskError::Unavailable(_)));
    let input: serde_json::Value =
        serde_json::from_str(&w.llm.calls().last().expect("call").user).expect("json");
    assert_eq!(input["sources"][0]["note_title"], "Acme");
    assert_eq!(
        input["sources"][0]["ref"]
            .as_str()
            .map(|r| r.starts_with("Acme#^ask-")),
        Some(true)
    );
    let system = prompts::latest(ids::ASK).expect("ask").text;
    let recorded = w.llm.calls().last().cloned().expect("call");
    w.llm.push(
        &recorded.prompt,
        &input_hash(system, &recorded.user),
        Fixture::stream(&["Weekly."]),
    );
    // `keyword_only`'s generator moved on: an engine with a fresh one reproduces the IDs.
    let again = AskEngine::new(
        w.db.app_db.clone(),
        w.vault.clone(),
        w.ai.clone(),
        None,
        fresh_ids(),
        Arc::new(w.db.clock.clone()),
        AskConfig::default(),
    );
    let run = again
        .start(sa, "alice".into(), "weekly invoicing", None)
        .await
        .expect("started");
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    run.run(move |e| sink.lock().expect("events").push(e)).await;
    assert_eq!(
        events.lock().expect("events").clone(),
        vec![
            AskEvent::Tokens("Weekly.".into()),
            AskEvent::Done {
                answer: "Weekly.".into()
            }
        ]
    );
    let paused = again
        .start(sa, "alice".into(), "weekly invoicing", None)
        .await
        .expect_err("budget");
    assert_eq!(
        paused,
        AskError::Paused {
            reason: PauseReason::UserBudget,
            until: Some("2026-09-28T00:00:00Z".parse().expect("time")),
        }
    );
    w.finish().await;
}
