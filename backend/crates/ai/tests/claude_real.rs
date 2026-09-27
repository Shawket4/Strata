//! Real `claude -p` smoke test (ignored by default: it needs a logged-in Claude Code and spends
//! subscription usage). Run it with
//!
//! ```sh
//! STRATA_CLAUDE_COMMAND=/usr/bin/claude cargo test -p strata-ai --test claude_real -- --ignored
//! ```
//!
//! It runs the `summary` prompt through `AiService` exactly as production does (all tools off,
//! environment cleared except `HOME` for the login) and checks the reply against the schema.
#![allow(clippy::expect_used)] // tests: expect with messages

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde_json::json;
use strata_ai::outputs::Summary;
use strata_ai::prompts::{self, ids};
use strata_ai::{
    AiService, BudgetGuard, BudgetLimits, ClaudeCliConfig, ClaudeCliProvider, CpuGate,
    MemoryUsageStore, ProviderRouter, StreamEvent,
};
use strata_common::SystemClock;
use strata_common::config::AiProviderKind;

#[tokio::test]
#[ignore = "calls the real claude CLI (needs a login; spends subscription usage)"]
async fn real_claude_cli_answers_a_structured_prompt_and_streams() {
    let command = std::env::var("STRATA_CLAUDE_COMMAND").unwrap_or_else(|_| "claude".into());
    let work = tempfile::TempDir::new().expect("scratch");
    let mut cfg = ClaudeCliConfig::new(vec![command], work.path().to_path_buf());
    // Development only: keep the caller's HOME (where the login lives) and PATH.
    for key in ["HOME", "PATH"] {
        if let Ok(v) = std::env::var(key) {
            cfg.env.insert(key.into(), v);
        }
    }
    cfg.timeout = Duration::from_secs(180);
    let clock = Arc::new(SystemClock);
    let provider = ClaudeCliProvider::new(cfg, clock.clone(), CpuGate::new()).expect("provider");
    let router = ProviderRouter::new(AiProviderKind::ClaudeCli, BTreeMap::new())
        .with_provider(AiProviderKind::ClaudeCli, Arc::new(provider));
    let store = Arc::new(MemoryUsageStore::default());
    let service = AiService::new(
        router,
        BudgetGuard::new(
            BudgetLimits::default(),
            chrono_tz::UTC,
            clock,
            store.clone(),
        ),
    );

    let input = json!({"note": {"id": "n1", "title": "Watanya invoicing", "created": "2026-09-27T14:32:00+03:00",
        "text": "اتفقنا مع وطنية إن فاتورة الـ ETA تطلع أول كل شهر، والعقد الأصلي في الخزنة في مكتب مدينة نصر.", "truncated": false}});
    let out = service
        .complete::<Summary>(
            common::caller("owner", 1),
            prompts::latest(ids::SUMMARY).expect("prompt"),
            &input,
            1024,
        )
        .await
        .expect("structured reply");
    eprintln!(
        "summary: {:?} (attempts {}, model {})",
        out.value, out.attempts, out.model
    );
    assert!(!out.value.summary.is_empty());
    assert_eq!(store.rows().len(), 1);

    let ask = prompts::latest(ids::ASK).expect("ask").chat_request(
        common::caller("owner", 1),
        &json!({"question": {"text": "Where is the Watanya contract?", "asked_at": "2026-09-27T15:00:00+03:00"},
            "sources": [{"ref": "Capture 2026-09-20#^c1d2", "note_title": "Capture 2026-09-20", "created": "2026-09-20T10:00:00+03:00",
                "text": "Watanya's contract is at the Nasr City office in the safe, last with Shady."}]}),
        2048,
    ).expect("request");
    let items: Vec<_> = service.stream(ask).await.expect("stream").collect().await;
    let text: String = items
        .iter()
        .filter_map(|i| match i {
            Ok(StreamEvent::Text(t)) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    eprintln!("ask: {text}");
    assert!(matches!(items.last(), Some(Ok(StreamEvent::Done { .. }))));
    assert!(
        text.contains("[[Capture 2026-09-20#^c1d2]]"),
        "answer cites its source"
    );
}
