//! The `embed` and `embed_backfill` jobs (PLAN §9.1b, §9.2): heading-aware chunks with block
//! IDs, the note vector as the normalised mean of the chunk vectors, idempotency on an
//! unchanged content hash, re-embedding on change, a model change as a resumable full
//! re-embed with progress, duplicate-check item vectors, and trashed notes.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

mod common;

use std::sync::Arc;

use common::{MODEL, World, eager};
use pgvector::Vector;
use pretty_assertions::assert_eq;
use strata_ai::embed::fake::FakeEmbedder;
use strata_ai::{AiCaller, Embedder};
use strata_common::{Clock, NoteId, UserId};
use strata_index::UserScope;
use strata_jobs::embed::{BackfillHandler, EmbedHandler};
use strata_jobs::{JobHandler, RunnerConfig};

fn words(word: &str, n: usize) -> String {
    vec![word; n].join(" ")
}

fn unit(i: usize) -> Vec<f32> {
    let mut v = vec![0.0; 384];
    v[i] = 1.0;
    v
}

fn embed_handler(w: &World, embedder: Arc<dyn Embedder>) -> Arc<dyn JobHandler> {
    Arc::new(EmbedHandler::new(
        w.db.app_db.clone(),
        w.vault.clone(),
        Some(embedder),
        Arc::new(w.db.clock.clone()),
        w.db.ids.clone(),
        true,
    ))
}

type ChunkRow = (i32, Option<String>, String, String, i32, i32, i32, Option<String>, String);

async fn chunk_rows(w: &World, user: UserId, note: NoteId) -> Vec<ChunkRow> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows = sqlx::query_as(
        "SELECT ord, block_id, heading_path, text, start_offset, end_offset, anchor_len, model, \
         content_hash FROM chunks WHERE note_id = $1 ORDER BY ord",
    )
    .bind(note)
    .fetch_all(tx.conn())
    .await
    .expect("chunks");
    tx.commit().await.expect("commit");
    rows
}

async fn note_vector(w: &World, user: UserId, note: NoteId) -> Option<(String, String, Vec<f32>, i32)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let row: Option<(String, String, Vector, i32)> = sqlx::query_as(
        "SELECT model, content_hash, embedding, chunk_count FROM note_vectors WHERE note_id = $1",
    )
    .bind(note)
    .fetch_optional(tx.conn())
    .await
    .expect("vector");
    tx.commit().await.expect("commit");
    row.map(|(m, h, v, c)| (m, h, v.to_vec(), c))
}

async fn item_vectors(w: &World, user: UserId) -> Vec<(String, String, String)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let rows = sqlx::query_as(
        "SELECT kind, item_id, model FROM dedupe_vectors ORDER BY kind, item_id",
    )
    .fetch_all(tx.conn())
    .await
    .expect("items");
    tx.commit().await.expect("commit");
    rows
}

async fn version(w: &World, scope: &UserScope, note: NoteId) -> String {
    w.vault.note(scope, note).await.expect("note").version
}

#[tokio::test]
async fn notes_are_chunked_by_heading_with_block_ids_and_the_note_vector_is_the_normalised_mean() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let long = words("aaaa", 320);
    let body = format!("# One\n\n{long}\n\nSecond line\n\n## Two\n\nTiny tail ^t1\n");
    let note = w.create(&sa, "notes/Pricing.md", &body).await;
    let first_input = format!("Pricing\nOne\n{long}\n\nSecond line");
    let second_input = "Pricing\nOne/Two\nTiny tail".to_owned();
    w.embedder.set(&first_input, &unit(0));
    w.embedder.set(&second_input, &[0.0, 3.0, 4.0]);
    let runner = w.runner(eager(), vec![embed_handler(&w, w.embedder_arc())]);
    runner.run_until_idle().await;

    let v = version(&w, &sa, note).await;
    let stored = w.vault.note(&sa, note).await.expect("note").content;
    let body_start = stored.find("# One").expect("body");
    let body_text = &stored[body_start..];
    let long_start = body_text.find("aaaa").expect("long");
    let tail_start = body_text.find("Tiny tail").expect("tail");
    assert_eq!(
        chunk_rows(&w, a, note).await,
        vec![
            (
                0,
                None,
                "One".to_owned(),
                format!("{long}\n\nSecond line"),
                i32::try_from(long_start).expect("offset"),
                i32::try_from(body_text.find("Second line").expect("second") + "Second line".len())
                    .expect("offset"),
                i32::try_from(long.len()).expect("len"),
                Some(MODEL.to_owned()),
                v.clone(),
            ),
            (
                1,
                Some("t1".to_owned()),
                "One/Two".to_owned(),
                "Tiny tail".to_owned(),
                i32::try_from(tail_start).expect("offset"),
                i32::try_from(tail_start + "Tiny tail".len()).expect("offset"),
                i32::try_from("Tiny tail".len()).expect("len"),
                Some(MODEL.to_owned()),
                v.clone(),
            ),
        ]
    );
    // mean([1,0,0], [0,0.6,0.8]) = [0.5, 0.3, 0.4]; |·| = √0.5 → [0.7071, 0.4243, 0.5657].
    let (model, hash, vector, count) = note_vector(&w, a, note).await.expect("note vector");
    assert_eq!((model.as_str(), hash, count), (MODEL, v.clone(), 2));
    let norm = 0.5f32.sqrt();
    let expected = [0.5 / norm, 0.3 / norm, 0.4 / norm];
    for (got, want) in vector.iter().zip(expected) {
        assert!((got - want).abs() < 1e-6, "{got} vs {want}");
    }
    assert!(vector[3..].iter().all(|x| *x == 0.0));
    let len: f32 = vector.iter().map(|x| x * x).sum();
    assert!((len - 1.0).abs() < 1e-6);
    // One call per chunk, then the note's duplicate-check item (its title).
    assert_eq!(
        w.embedder.calls(),
        vec![vec![first_input], vec![second_input], vec!["Pricing".to_owned()]]
    );
    assert_eq!(
        item_vectors(&w, a).await,
        vec![("note".into(), note.to_string(), MODEL.into())]
    );
    // The re-embed queued a summary.
    assert_eq!(
        w.jobs(a).await,
        vec![
            ("embed".into(), "done".into(), 1),
            ("summarize".into(), "queued".into(), 0)
        ]
    );
    w.finish().await;
}

#[tokio::test]
async fn unchanged_notes_are_skipped_and_changed_ones_re_embedded() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let note = w.create(&sa, "notes/Acme.md", "Prefers weekly invoicing.\n").await;
    let runner = w.runner(eager(), vec![embed_handler(&w, w.embedder_arc())]);
    runner.run_until_idle().await;
    let calls = w.embedder.calls().len();
    assert_eq!(calls, 2, "one chunk and one item");
    let before = chunk_rows(&w, a, note).await;

    // Same content hash: the job runs and embeds nothing.
    let mut tx = w.db.begin(a).await.expect("tx");
    let now = w.db.clock.now();
    strata_jobs::repo::enqueue_for_note(&mut tx, w.db.ids.as_ref(), "embed", note, now, now)
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
    runner.run_until_idle().await;
    assert_eq!(w.embedder.calls().len(), calls);
    assert_eq!(chunk_rows(&w, a, note).await, before);

    // An edit changes the hash: the vault queues embed and the chunks follow the new text.
    let v = version(&w, &sa, note).await;
    let content = w.vault.note(&sa, note).await.expect("note").content;
    w.vault
        .update_note(
            &sa,
            note,
            content.replace("weekly", "monthly"),
            v.clone(),
        )
        .await
        .expect("update");
    runner.run_until_idle().await;
    let after = chunk_rows(&w, a, note).await;
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].3, "Prefers monthly invoicing.");
    assert_ne!(after[0].8, v);
    // The title did not change, so the item vector was kept: one new call only.
    assert_eq!(w.embedder.calls().len(), calls + 1);
    w.finish().await;
}

#[tokio::test]
async fn a_model_change_triggers_a_resumable_full_re_embed_with_progress() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let n1 = w.create(&sa, "notes/One.md", "first\n").await;
    w.db.clock.advance(chrono::Duration::seconds(1));
    let n2 = w.create(&sa, "notes/Two.md", "second\n").await;
    w.db.clock.advance(chrono::Duration::seconds(1));
    let n3 = w.create(&sa, "notes/Three.md", "third\n").await;
    w.standard_runner().run_until_idle().await;
    let caller = AiCaller {
        scope: sa,
        username: "alice".into(),
    };
    let status = strata_jobs::status::ai_status(&w.ai, &w.db.app_db, &caller)
        .await
        .expect("status");
    assert_eq!(
        (status.embedding_progress, status.queue_depth),
        (
            Some(strata_ai::EmbeddingProgress {
                embedded: 3,
                total: 3
            }),
            // Three summaries wait for fixtures (the fake provider has none): queued for retry.
            Some(3)
        )
    );

    // Configure a new model: every vector is stale.
    let v2 = FakeEmbedder::new("fake-embed@2", 384);
    let v2_arc: Arc<dyn Embedder> = Arc::new(v2.clone());
    let clock = Arc::new(w.db.clock.clone());
    let backfill = Arc::new(BackfillHandler::new(
        w.db.app_db.clone(),
        Some(v2_arc.clone()),
        clock.clone(),
        w.db.ids.clone(),
        2,
    ));
    let runner = w.runner(
        RunnerConfig::default(),
        vec![embed_handler(&w, v2_arc.clone()), backfill],
    );
    let scheduler = strata_jobs::Scheduler::new(
        w.db.app_db.clone(),
        w.db.issuer.clone(),
        clock,
        w.db.ids.clone(),
        Arc::new(strata_jobs::StaticUsers(vec![a])),
        chrono_tz::UTC,
        3,
        vec![],
    );
    assert_eq!(scheduler.enqueue_for_all("embed_backfill", "backfill").await, 1);
    let mut tx = w.db.begin(a).await.expect("tx");
    assert_eq!(
        strata_jobs::vectors::coverage(&mut tx, "fake-embed@2").await.expect("coverage"),
        (0, 3)
    );
    tx.commit().await.expect("commit");

    // First pass: a batch of two (most recently updated first), then the backfill waits.
    runner.run_until_idle().await;
    let mut tx = w.db.begin(a).await.expect("tx");
    assert_eq!(
        strata_jobs::vectors::coverage(&mut tx, "fake-embed@2").await.expect("coverage"),
        (2, 3)
    );
    tx.commit().await.expect("commit");
    assert_eq!(note_vector(&w, a, n3).await.expect("v").0, "fake-embed@2");
    assert_eq!(note_vector(&w, a, n2).await.expect("v").0, "fake-embed@2");
    assert_eq!(note_vector(&w, a, n1).await.expect("v").0, MODEL);

    // It resumes 30 s later and finishes; then it stops re-enqueueing itself.
    w.db.clock.advance(chrono::Duration::seconds(30));
    runner.run_until_idle().await;
    w.db.clock.advance(chrono::Duration::seconds(30));
    runner.run_until_idle().await;
    let mut tx = w.db.begin(a).await.expect("tx");
    assert_eq!(
        strata_jobs::vectors::coverage(&mut tx, "fake-embed@2").await.expect("coverage"),
        (3, 3)
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM jobs WHERE kind = 'embed_backfill' AND status = 'queued'",
    )
    .fetch_one(tx.conn())
    .await
    .expect("count");
    let models: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT model FROM chunks ORDER BY model")
            .fetch_all(tx.conn())
            .await
            .expect("models");
    tx.commit().await.expect("commit");
    assert_eq!((pending, models), (0, vec!["fake-embed@2".to_owned()]));
    assert_eq!(v2.calls().len(), 6, "three chunks and three items");
    w.finish().await;
}

#[tokio::test]
async fn trashed_notes_lose_their_vectors_and_task_lines_get_item_vectors() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let note = w
        .create(&sa, "notes/Todo.md", "- [ ] Send the ETA invoice to Watanya\n")
        .await;
    let runner = w.runner(eager(), vec![embed_handler(&w, w.embedder_arc())]);
    runner.run_until_idle().await;
    let items = item_vectors(&w, a).await;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0], ("note".into(), note.to_string(), MODEL.into()));
    assert_eq!((items[1].0.as_str(), items[1].1.starts_with("t-")), ("task", true));
    assert_eq!(
        w.embedder.calls().last().cloned(),
        Some(vec!["Send the ETA invoice to Watanya".to_owned()])
    );

    w.vault.delete_note(&sa, note).await.expect("delete");
    let mut tx = w.db.begin(a).await.expect("tx");
    let now = w.db.clock.now();
    strata_jobs::repo::enqueue_for_note(&mut tx, w.db.ids.as_ref(), "embed", note, now, now)
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
    runner.run_until_idle().await;
    assert_eq!(chunk_rows(&w, a, note).await, vec![]);
    assert_eq!(note_vector(&w, a, note).await, None);
    assert_eq!(item_vectors(&w, a).await, vec![]);
    w.finish().await;
}

#[tokio::test]
async fn without_an_embedder_embed_jobs_complete_without_vectors() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let note = w.create(&sa, "notes/X.md", "text\n").await;
    let runner = w.runner(
        RunnerConfig::default(),
        strata_jobs::standard_handlers(&w.deps_with(None)),
    );
    runner.run_until_idle().await;
    assert_eq!(w.jobs(a).await, vec![("embed".into(), "done".into(), 1)]);
    assert_eq!(note_vector(&w, a, note).await, None);
    w.finish().await;
}
