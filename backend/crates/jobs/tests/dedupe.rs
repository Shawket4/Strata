//! Semantic duplicates (PLAN §9.7): the semantic level of the synchronous check on create
//! (only when the model is loaded), and the nightly sweep — vector blocking, exact/near
//! within blocks, confirmed and LLM-confirmed semantic pairs as `duplicates` suggestions,
//! keep-both pairs and remembered verdicts respected, rejection recording keep-both.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

mod common;

use std::sync::Arc;

use common::World;
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::{self, ids};
use strata_ai::request::input_hash;
use strata_common::{NoteId, UserId};
use strata_index::UserScope;
use strata_index::types::SuggestionStatus;
use strata_jobs::dedupe_sweep::DedupeHandler;
use strata_jobs::semantic_dup::SemanticDupSource;
use strata_jobs::{JobHandler, RunnerConfig};
use strata_testkit::Fixture;
use strata_vault::VaultError;
use strata_vault::ops::ai::DuplicatesPayload;
use strata_vault::ops::notes::CreateNote;

/// Unit vectors `a = e[i]`, `b = c·e[i] + √(1−c²)·e[j]` (cosine `c`).
fn pair(i: usize, j: usize, c: f32) -> (Vec<f32>, Vec<f32>) {
    let mut a = vec![0.0; 384];
    a[i] = 1.0;
    let mut b = vec![0.0; 384];
    b[i] = c;
    b[j] = (1.0 - c * c).sqrt();
    (a, b)
}

fn set_pair(w: &World, x: &str, y: &str, dims: (usize, usize), c: f32) {
    let (a, b) = pair(dims.0, dims.1, c);
    w.embedder.set(x, &a);
    w.embedder.set(y, &b);
}

async fn try_create(
    w: &World,
    scope: &UserScope,
    path: &str,
    force: bool,
) -> Result<NoteId, VaultError> {
    w.vault
        .create_note(
            scope,
            CreateNote {
                path: path.to_owned(),
                content: "x\n".to_owned(),
                id: None,
                force,
            },
        )
        .await
        .map(|v| v.id)
}

async fn keep_both(w: &World, user: UserId) -> Vec<(String, String, String)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows = sqlx::query_as("SELECT kind, a_id, b_id FROM dedupe_keep_both ORDER BY a_id, b_id")
        .fetch_all(tx.conn())
        .await
        .expect("rows");
    tx.commit().await.expect("commit");
    rows
}

fn ordered(a: NoteId, b: NoteId) -> (String, String) {
    let (a, b) = (a.to_string(), b.to_string());
    if a <= b { (a, b) } else { (b, a) }
}

fn register_semantic(w: &World) {
    w.vault.set_semantic(Arc::new(SemanticDupSource::new(
        w.embedder_arc(),
        dedupe::Thresholds::new(),
    )));
}

#[tokio::test]
async fn creates_check_the_semantic_level_only_with_a_loaded_model() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    register_semantic(&w);
    set_pair(&w, "Tax return 2026", "2026 tax filing", (0, 1), 0.98);
    let (_, papers) = pair(0, 2, 0.99);
    w.embedder.set("Annual tax papers", &papers);
    let tax = try_create(&w, &sa, "notes/Tax return 2026.md", false)
        .await
        .expect("first");
    // No item vector yet: the new note is not found (embedding runs in the background).
    w.standard_runner().run_until_idle().await;

    let err = try_create(&w, &sa, "notes/2026 tax filing.md", false)
        .await
        .expect_err("semantic duplicate");
    let VaultError::Duplicate(candidates) = err else {
        panic!("expected duplicates, got {err:?}");
    };
    assert_eq!(candidates.len(), 1);
    let c = &candidates[0];
    assert_eq!(
        (
            c.item.as_str(),
            c.id,
            c.kind.as_str(),
            c.title.as_str(),
            c.level,
            c.semantic
        ),
        (
            tax.to_string().as_str(),
            tax.as_ulid(),
            "note",
            "Tax return 2026",
            strata_vault::MatchLevel::Near,
            true
        )
    );
    assert!((c.score - 0.98).abs() < 1e-6, "{}", c.score);
    // Create anyway: keep-both is recorded for the pair.
    let filing = try_create(&w, &sa, "notes/2026 tax filing.md", true)
        .await
        .expect("forced");
    let (x, y) = ordered(tax, filing);
    assert_eq!(keep_both(&w, a).await, vec![("note".into(), x, y)]);

    // Unloaded model: exact and near only, so a paraphrase is created without a prompt.
    w.embedder.set_loaded(false);
    try_create(&w, &sa, "notes/Annual tax papers.md", false)
        .await
        .expect("no semantic check while unloaded");
    w.finish().await;
}

fn confirm_input(new: &str, existing_id: NoteId, existing: &str, similarity: &str) -> String {
    format!(
        "{{\n  \"kind\": \"note\",\n  \"new_item\": {{\n    \"text\": \"{new}\",\n    \"due\": null,\n    \"recurrence\": null\n  }},\n  \"existing\": {{\n    \"id\": \"{existing_id}\",\n    \"text\": \"{existing}\",\n    \"due\": null,\n    \"recurrence\": null\n  }},\n  \"similarity\": {similarity}\n}}"
    )
}

#[tokio::test]
async fn the_nightly_sweep_suggests_new_duplicate_pairs_once() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    register_semantic(&w);
    set_pair(
        &w,
        "Car insurance renewal",
        "Vehicle policy renewal",
        (0, 1),
        0.97,
    );
    set_pair(
        &w,
        "Office lease",
        "Rental agreement for the office",
        (2, 3),
        0.93,
    );
    set_pair(&w, "Gym plan", "Fitness schedule", (4, 5), 0.92);
    set_pair(&w, "Tax return 2026", "2026 tax filing", (6, 7), 0.98);
    let mut ids = Vec::new();
    for title in [
        "Car insurance renewal",
        "Vehicle policy renewal",
        "Office lease",
        "Rental agreement for the office",
        "Gym plan",
        "Fitness schedule",
        "Tax return 2026",
    ] {
        ids.push(
            try_create(&w, &sa, &format!("notes/{title}.md"), false)
                .await
                .expect("create"),
        );
    }
    let runner = w.standard_runner();
    runner.run_until_idle().await;
    // Kept on purpose: forced past the semantic check, so the pair is never flagged.
    let filing = try_create(&w, &sa, "notes/2026 tax filing.md", true)
        .await
        .expect("forced");
    // An exact pair made by a rename (moves skip the create check).
    let budget = try_create(&w, &sa, "notes/Budget.md", false)
        .await
        .expect("b");
    let other = try_create(&w, &sa, "notes/Spending sheet.md", false)
        .await
        .expect("b2");
    w.vault
        .move_note(&sa, other, "archive/Budget.md".into(), None)
        .await
        .expect("move");
    runner.run_until_idle().await;

    // Borderline pairs are confirmed by the LLM.
    let system = prompts::latest(ids::DUPLICATE_CONFIRM)
        .expect("prompt")
        .text;
    let prompt = prompts::latest(ids::DUPLICATE_CONFIRM)
        .expect("prompt")
        .prompt_ref();
    w.llm.push(
        &prompt,
        &input_hash(
            system,
            &confirm_input("Office lease", ids[3], "Rental agreement for the office", "0.93"),
        ),
        Fixture::json(json!({"verdict": "duplicate", "confidence": 0.9, "reason": "Both are the office lease."})),
    );
    w.llm.push(
        &prompt,
        &input_hash(system, &confirm_input("Gym plan", ids[5], "Fitness schedule", "0.92")),
        Fixture::json(json!({"verdict": "distinct", "confidence": 0.8, "reason": "A plan is not a schedule."})),
    );
    let sweep: Arc<dyn JobHandler> = Arc::new(DedupeHandler::new(
        w.db.app_db.clone(),
        w.vault.clone(),
        w.ai.clone(),
        Some(w.embedder_arc()),
        dedupe::Thresholds::new(),
        w.db.ids.clone(),
        20,
    ));
    let nightly = w.runner(RunnerConfig::default(), vec![sweep]);
    let scheduler = strata_jobs::Scheduler::new(
        w.db.app_db.clone(),
        w.db.issuer.clone(),
        Arc::new(w.db.clock.clone()),
        w.db.ids.clone(),
        Arc::new(strata_jobs::StaticUsers(vec![a])),
        chrono_tz::UTC,
        3,
        strata_jobs::standard_periodic(),
    );
    assert_eq!(scheduler.ensure().await, 1);
    w.db.clock
        .set("2026-09-28T03:00:00Z".parse().expect("time"));
    let claimed = nightly.run_until_idle().await;
    assert_eq!(claimed.len(), 1);
    let confirm_calls = w
        .llm
        .calls()
        .into_iter()
        .filter(|c| c.prompt == prompt)
        .count();
    assert_eq!(confirm_calls, 2);

    let suggestions = w
        .vault
        .suggestions(&sa, SuggestionStatus::Pending)
        .await
        .expect("suggestions");
    let mut found: Vec<(String, String, String, String, String, Option<String>)> = suggestions
        .iter()
        .filter(|s| s.suggestion.kind == "duplicates")
        .map(|s| {
            let p: DuplicatesPayload =
                rmp_serde::from_slice(&s.suggestion.payload).expect("payload");
            assert_eq!(
                s.suggestion.note_id.map(|n| n.to_string()),
                Some(p.a.item.clone())
            );
            assert_eq!(p.a.match_level, p.b.match_level);
            (
                p.a.title,
                p.b.title,
                p.a.match_level,
                format!("{:.2}", p.a.score),
                p.a.kind,
                p.reason,
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            (
                "Budget".into(),
                "Budget".into(),
                "exact".into(),
                "1.00".into(),
                "note".into(),
                None
            ),
            (
                "Car insurance renewal".into(),
                "Vehicle policy renewal".into(),
                "semantic".into(),
                "0.97".into(),
                "note".into(),
                None
            ),
            (
                "Office lease".into(),
                "Rental agreement for the office".into(),
                "semantic".into(),
                "0.93".into(),
                "note".into(),
                Some("Both are the office lease.".into())
            ),
        ]
    );
    assert_ne!(budget, filing);

    // A second night finds nothing new and asks the LLM nothing (verdicts are remembered).
    scheduler.ensure().await;
    w.db.clock
        .set("2026-09-29T03:00:00Z".parse().expect("time"));
    nightly.run_until_idle().await;
    assert_eq!(
        w.vault
            .suggestions(&sa, SuggestionStatus::Pending)
            .await
            .expect("suggestions")
            .iter()
            .filter(|s| s.suggestion.kind == "duplicates")
            .count(),
        3
    );
    assert_eq!(
        w.llm
            .calls()
            .into_iter()
            .filter(|c| c.prompt == prompt)
            .count(),
        2
    );

    // Rejecting a pair means "distinct": keep-both is recorded.
    let car = suggestions
        .iter()
        .find(|s| {
            s.suggestion.kind == "duplicates"
                && rmp_serde::from_slice::<DuplicatesPayload>(&s.suggestion.payload)
                    .is_ok_and(|p| p.a.title == "Car insurance renewal")
        })
        .expect("car pair");
    let before = keep_both(&w, a).await.len();
    w.vault
        .decide_suggestion(&sa, car.suggestion.id, false)
        .await
        .expect("reject");
    let pairs = keep_both(&w, a).await;
    assert_eq!(pairs.len(), before + 1);
    let (x, y) = ordered(ids[0], ids[1]);
    assert!(pairs.contains(&("note".into(), x, y)));
    w.finish().await;
}
