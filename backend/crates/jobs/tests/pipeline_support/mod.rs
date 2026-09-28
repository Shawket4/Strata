//! Helpers for the AI pipeline tests: fixtures keyed by the exact prompt input (built from
//! the jobs' own input types with literal values, so every test pins the prompt input),
//! runners over a subset of the standard handlers, and readers of the rows the pipelines
//! write.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use strata_ai::prompts;
use strata_ai::request::input_hash;
use strata_common::{Clock, NoteId, UserId};
use strata_index::UserScope;
use strata_jobs::pipeline::{BlockInput, CandidateInput, EntityInput, generated_block_id};
use strata_jobs::{JobHandler, Runner};
use strata_testkit::Fixture;

use crate::common::{World, eager};

/// `created` of notes written at the test epoch (UTC users).
pub const CREATED: &str = "2026-09-27T12:00:00+00:00";

/// The provenance model of the fake provider.
pub const MODEL: &str = "claude_cli/fake-model";

/// Registers `out` as the reply of prompt `id` to exactly `input`.
pub fn push<T: Serialize>(w: &World, id: &str, input: &T, out: Value) {
    push_fixture(w, id, input, Fixture::json(out));
}

/// Registers a fixture for prompt `id` and `input`.
pub fn push_fixture<T: Serialize>(w: &World, id: &str, input: &T, f: Fixture) {
    let p = prompts::latest(id).expect("prompt");
    let user = prompts::render_input(input).expect("render");
    w.llm.push(&p.prompt_ref(), &input_hash(p.text, &user), f);
}

/// The standard handlers (no embedding model) of `kinds`.
pub fn handlers(w: &World, kinds: &[&str]) -> Vec<Arc<dyn JobHandler>> {
    strata_jobs::standard_handlers(&w.deps_with(None))
        .into_iter()
        .filter(|h| kinds.contains(&h.kind()))
        .collect()
}

/// A runner over `kinds`.
pub fn runner(w: &World, kinds: &[&str]) -> Runner {
    w.runner(eager(), handlers(w, kinds))
}

/// Enqueues `kind` for `note` now.
pub async fn enqueue(w: &World, user: UserId, kind: &str, note: NoteId) {
    let mut tx = w.db.begin(user).await.expect("tx");
    let now = w.db.clock.now();
    strata_jobs::repo::enqueue_for_note(&mut tx, w.db.ids.as_ref(), kind, note, now, now)
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
}

/// Moves the fake clock forward.
pub fn advance(w: &World, secs: i64) {
    let now = w.db.clock.now();
    w.db.clock.set(now + chrono::Duration::seconds(secs));
}

/// A block without an ID (the pipeline generates one from its text).
pub fn block(text: &str) -> BlockInput {
    BlockInput {
        block_id: generated_block_id(text),
        text: text.to_owned(),
    }
}

/// A block with its ID.
pub fn block_with(id: &str, text: &str) -> BlockInput {
    BlockInput {
        block_id: id.to_owned(),
        text: text.to_owned(),
    }
}

/// A candidate note.
pub fn cand(id: NoteId, title: &str) -> CandidateInput {
    CandidateInput {
        id: id.to_string(),
        title: title.to_owned(),
        kind: "note".to_owned(),
        summary: None,
    }
}

/// An entity.
pub fn ent(id: NoteId, kind: &str, name: &str, aliases: &[&str], hints: &[&str]) -> EntityInput {
    EntityInput {
        id: id.to_string(),
        kind: kind.to_owned(),
        name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        hints: hints.iter().map(|a| (*a).to_owned()).collect(),
        part_of: None,
    }
}

/// One decision row, as the tests compare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Kind.
    pub kind: String,
    /// Source note.
    pub source: Option<String>,
    /// Target.
    pub target: String,
    /// Summary.
    pub summary: String,
    /// Relation key or custody type.
    pub rel: Option<String>,
    /// Mention.
    pub mention: Option<String>,
    /// Became a suggestion.
    pub suggested: bool,
    /// Applied by a commit.
    pub committed: bool,
    /// Reverted.
    pub reverted: bool,
}

/// The AI decisions of `user`, oldest first.
pub async fn decisions(w: &World, user: UserId) -> Vec<Row> {
    let mut tx = w.db.begin(user).await.expect("tx");
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        Option<NoteId>,
        String,
        String,
        Option<String>,
        Option<String>,
        bool,
        bool,
        bool,
    )> = sqlx::query_as(
        "SELECT kind, source_note_id, target_id, summary, rel_type, mention, \
           suggestion_id IS NOT NULL, git_commit IS NOT NULL, reverted_at IS NOT NULL \
         FROM ai_decisions ORDER BY created, id",
    )
    .fetch_all(tx.conn())
    .await
    .expect("decisions");
    tx.commit().await.expect("commit");
    rows.into_iter()
        .map(|r| Row {
            kind: r.0,
            source: r.1.map(|n| n.to_string()),
            target: r.2,
            summary: r.3,
            rel: r.4,
            mention: r.5,
            suggested: r.6,
            committed: r.7,
            reverted: r.8,
        })
        .collect()
}

/// `(kind, status, payload as JSON)` of every suggestion of `user`, oldest first.
pub async fn suggestions(w: &World, user: UserId) -> Vec<(String, String, Value)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows: Vec<(String, String, Vec<u8>)> =
        sqlx::query_as("SELECT kind, status, payload FROM suggestions ORDER BY created, id")
            .fetch_all(tx.conn())
            .await
            .expect("suggestions");
    tx.commit().await.expect("commit");
    rows.into_iter()
        .map(|(k, s, p)| {
            let v: Value = rmp_serde::from_slice(&p).expect("payload");
            (k, s, v)
        })
        .collect()
}

/// The suggestion IDs of `user`, oldest first.
pub async fn suggestion_ids(w: &World, user: UserId) -> Vec<strata_common::SuggestionId> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows: Vec<(strata_common::SuggestionId,)> =
        sqlx::query_as("SELECT id FROM suggestions ORDER BY created, id")
            .fetch_all(tx.conn())
            .await
            .expect("ids");
    tx.commit().await.expect("commit");
    rows.into_iter().map(|r| r.0).collect()
}

/// The sidecar of `note` as JSON (`Null` when absent).
pub fn sidecar(w: &World, user: UserId, note: NoteId) -> Value {
    let path = w.dir(user).join(format!(".meta/notes/{note}.json"));
    std::fs::read_to_string(path)
        .map(|t| serde_json::from_str(&t).expect("json"))
        .unwrap_or(Value::Null)
}

/// The path of note `id`.
pub async fn path_of(w: &World, scope: &UserScope, id: NoteId) -> String {
    w.vault.note(scope, id).await.expect("note").path
}

/// The version of note `id`.
pub async fn version_of(w: &World, scope: &UserScope, id: NoteId) -> String {
    w.vault.note(scope, id).await.expect("note").version
}

/// `(kind, status, run_after)` of every job of `kind`.
pub async fn jobs_of(w: &World, user: UserId, kind: &str) -> Vec<(String, i32, DateTime<Utc>)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows = sqlx::query_as(
        "SELECT status, attempts, run_after FROM jobs WHERE kind = $1 ORDER BY created, id",
    )
    .bind(kind)
    .fetch_all(tx.conn())
    .await
    .expect("jobs");
    tx.commit().await.expect("commit");
    rows
}

/// The test epoch plus `secs`.
pub fn at(secs: i64) -> DateTime<Utc> {
    strata_testkit::default_test_epoch() + chrono::Duration::seconds(secs)
}

/// Asserts that the last call of prompt `id` had exactly `input` (a readable diff when a
/// fixture did not match).
pub fn assert_input<T: Serialize>(w: &World, id: &str, input: &T) {
    let want = prompts::render_input(input).expect("render");
    let got = w
        .llm
        .calls()
        .into_iter()
        .filter(|c| c.prompt.id == id)
        .last()
        .map(|c| c.user)
        .unwrap_or_default();
    pretty_assertions::assert_eq!(got, want);
}

/// Sets the user's auto-file setting.
pub async fn set_auto_file(w: &World, user: UserId, on: bool) {
    let mut tx = w.db.begin(user).await.expect("tx");
    let now = w.db.clock.now();
    strata_index::repo::settings::put_setting(
        &mut tx,
        "auto_file",
        &rmp_serde::to_vec(&on).expect("bool"),
        now,
    )
    .await
    .expect("setting");
    tx.commit().await.expect("commit");
}
