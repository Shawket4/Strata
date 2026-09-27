//! The `summarize` job (PLAN §9.2, §6.5): exactly the sidecar change in one `ai:` commit,
//! skipped when unchanged, disabled users and budget pauses (the job waits, never fails).
#![allow(clippy::expect_used, clippy::too_many_lines, clippy::many_single_char_names)]

mod common;

use std::sync::Arc;

use common::{World, eager};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::{self, ids};
use strata_ai::request::input_hash;
use strata_ai::{BudgetLimits, PromptRef};
use strata_common::{Clock, NoteId, UserId};
use strata_index::UserScope;
use strata_jobs::JobHandler;
use strata_jobs::summarize::SummarizeHandler;
use strata_testkit::Fixture;

fn summary_prompt() -> PromptRef {
    prompts::latest(ids::SUMMARY).expect("summary").prompt_ref()
}

fn input(id: NoteId, title: &str, text: &str) -> String {
    format!(
        "{{\n  \"note\": {{\n    \"id\": \"{id}\",\n    \"title\": \"{title}\",\n    \"created\": \"2026-09-27T12:00:00+00:00\",\n    \"text\": \"{text}\",\n    \"truncated\": false\n  }}\n}}"
    )
}

fn push_summary(w: &World, user_input: &str, summary: &str) {
    let system = prompts::latest(ids::SUMMARY).expect("summary").text;
    w.llm.push(
        &summary_prompt(),
        &input_hash(system, user_input),
        Fixture::json(json!({"summary": summary, "lang": "en"})),
    );
}

async fn enqueue_summarize(w: &World, user: UserId, note: NoteId) {
    let mut tx = w.db.begin(user).await.expect("tx");
    let now = w.db.clock.now();
    strata_jobs::repo::enqueue_for_note(&mut tx, w.db.ids.as_ref(), "summarize", note, now, now)
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
}

fn handler(w: &World) -> Arc<dyn JobHandler> {
    Arc::new(SummarizeHandler::new(w.vault.clone(), w.ai.clone()))
}

async fn create(w: &World, scope: &UserScope, path: &str, body: &str) -> NoteId {
    w.create(scope, path, body).await
}

#[tokio::test]
async fn the_summary_goes_into_the_sidecar_in_one_ai_commit_and_is_not_redone() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let note = create(&w, &sa, "notes/Acme.md", "Prefers weekly invoicing.\n").await;
    let before = w.read(a, "notes/Acme.md");
    let version = w.vault.note(&sa, note).await.expect("note").version;
    let user_input = input(note, "Acme", "Prefers weekly invoicing.");
    push_summary(&w, &user_input, "Acme prefers weekly invoicing.");
    enqueue_summarize(&w, a, note).await;
    let runner = w.runner(eager(), vec![handler(&w)]);
    runner.run_until_idle().await;

    let calls = w.llm.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].user, user_input);
    assert_eq!(w.log(a)[0], "ai: summarize notes/Acme.md");
    let sidecar = format!(".meta/notes/{note}.json");
    assert_eq!(w.last_commit_paths(a), vec![sidecar.clone()]);
    assert_eq!(w.read(a, "notes/Acme.md"), before, "the note is never touched");
    let sc: serde_json::Value = serde_json::from_str(&w.read(a, &sidecar)).expect("json");
    assert_eq!(
        (sc["summary"].clone(), sc["content_hash"].clone()),
        (json!("Acme prefers weekly invoicing."), json!(version))
    );

    // Same version: no call, no commit.
    let commits = w.log(a).len();
    enqueue_summarize(&w, a, note).await;
    runner.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 1);
    assert_eq!(w.log(a).len(), commits);
    w.finish().await;
}

#[tokio::test]
async fn users_with_ai_disabled_are_skipped() {
    let w = World::new().await;
    let (a, sa) = w.user("noai").await;
    let note = create(&w, &sa, "notes/X.md", "text\n").await;
    enqueue_summarize(&w, a, note).await;
    let commits = w.log(a).len();
    w.runner(eager(), vec![handler(&w)]).run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 0);
    assert_eq!(w.log(a).len(), commits);
    assert_eq!(
        w.jobs(a).await.into_iter().filter(|j| j.0 == "summarize").collect::<Vec<_>>(),
        vec![("summarize".into(), "done".into(), 1)]
    );
    w.finish().await;
}

#[tokio::test]
async fn a_reached_budget_pauses_the_job_until_the_next_day() {
    let w = World::with_limits(BudgetLimits {
        per_user_daily_tokens: 100,
        ..BudgetLimits::default()
    })
    .await;
    let (a, sa) = w.user("alice").await;
    let first = create(&w, &sa, "notes/One.md", "first\n").await;
    let second = create(&w, &sa, "notes/Two.md", "second\n").await;
    push_summary(&w, &input(first, "One", "first"), "One.");
    push_summary(&w, &input(second, "Two", "second"), "Two.");
    enqueue_summarize(&w, a, first).await;
    let runner = w.runner(eager(), vec![handler(&w)]);
    runner.run_until_idle().await;
    // The first call used 150 tokens: the cap is reached, the next job waits.
    enqueue_summarize(&w, a, second).await;
    runner.run_until_idle().await;
    let mut tx = w.db.begin(a).await.expect("tx");
    let row: (String, i32, chrono::DateTime<chrono::Utc>, Option<String>) = sqlx::query_as(
        "SELECT status, attempts, run_after, last_error FROM jobs WHERE kind = 'summarize' AND note_id = $1",
    )
    .bind(second)
    .fetch_one(tx.conn())
    .await
    .expect("job");
    tx.commit().await.expect("commit");
    assert_eq!(
        row,
        (
            "queued".to_owned(),
            0,
            "2026-09-28T00:00:00Z".parse().expect("time"),
            Some("paused: UserBudget".to_owned())
        )
    );
    assert_eq!(w.llm.calls().len(), 1);
    // The next budget day it runs.
    w.db.clock.set("2026-09-28T00:00:00Z".parse().expect("time"));
    runner.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 2);
    assert_eq!(w.log(a)[0], "ai: summarize notes/Two.md");
    w.finish().await;
}
