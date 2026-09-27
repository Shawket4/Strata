//! `stratad serve` wiring of the job runner (PLAN §5.2, §9.2): job notices reach the event
//! bus as `job.completed` / `job.failed`, the scheduler's user directory lists exactly the
//! accounts whose jobs run, and the API's AI features follow the embedder configuration.
#![allow(clippy::expect_used)] // tests: expect with messages

use std::sync::Arc;

use pretty_assertions::assert_eq;
use strata_api::events::{BusConfig, Event, EventBus};
use strata_api::wire::ws::Frame;
use strata_common::{Clock, Config, JobId, NoteId};
use strata_index::types::UserStatus;
use strata_jobs::{JobEvents, JobNotice, JobOutcome, UserDirectory};
use strata_testkit::{TempDataRoot, TestDb, TestUser};
use stratad::jobs::{AccountUsers, BusEvents};

#[tokio::test]
async fn job_notices_are_published_as_job_events_of_their_user() {
    let db = TestDb::new().await.expect("db");
    let alice = TestUser::new("alice").create(&db).await.expect("user").id;
    let bus = Arc::new(EventBus::new(BusConfig::default()));
    let events = BusEvents(bus.clone());
    let job = JobId::generate(db.ids.as_ref());
    let note = NoteId::generate(db.ids.as_ref());
    events.job_finished(
        alice,
        &JobNotice {
            id: job,
            kind: "embed".into(),
            note_id: Some(note),
            outcome: JobOutcome::Completed,
        },
    );
    events.job_finished(
        alice,
        &JobNotice {
            id: job,
            kind: "dedupe".into(),
            note_id: None,
            outcome: JobOutcome::Failed,
        },
    );
    let sub = bus.subscribe(alice, Some(0));
    assert_eq!(
        sub.replay,
        vec![
            Frame::Data {
                seq: 1,
                payload: Event::JobCompleted {
                    id: job.as_ulid(),
                    kind: "embed".into(),
                    note_id: Some(note.as_ulid()),
                }
            },
            Frame::Data {
                seq: 2,
                payload: Event::JobFailed {
                    id: job.as_ulid(),
                    kind: "dedupe".into(),
                    note_id: None,
                }
            },
        ]
    );
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn the_scheduler_runs_jobs_for_active_and_deletion_pending_accounts_only() {
    let db = TestDb::new().await.expect("db");
    let active = TestUser::new("active").create(&db).await.expect("user").id;
    let _pending = TestUser::new("waiting").pending().create(&db).await.expect("user");
    let leaving = TestUser::new("leaving").create(&db).await.expect("user").id;
    sqlx::query("UPDATE users SET status = $1, deletion_at = now() + interval '14 days' WHERE id = $2")
        .bind(UserStatus::DeletionPending)
        .bind(leaving)
        .execute(&db.accounts)
        .await
        .expect("status");
    let mut users = AccountUsers(db.accounts_db.clone())
        .active_users()
        .await
        .expect("users");
    users.sort();
    let mut expected = vec![active, leaving];
    expected.sort();
    assert_eq!(users, expected);
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn ai_features_follow_the_embedding_configuration() {
    let db = TestDb::new().await.expect("db");
    let data = TempDataRoot::new().expect("data");
    let config = Config::default();
    let clock: Arc<dyn Clock> = Arc::new(db.clock.clone());
    let parts =
        stratad::ai::build(&config, db.app_db.clone(), clock.clone()).expect("ai parts");
    let vault = stratad::serve::vault_service(
        &Config {
            data_root: data.path().to_path_buf(),
            ..Config::default()
        },
        db.app_db.clone(),
        clock.clone(),
        db.ids.clone(),
    );
    let api = stratad::jobs::ai_api(&parts, &db.app_db, &vault, clock, db.ids.clone(), &config)
        .expect("ai api");
    // No embedding model configured: no semantic retrieval (search answers 503), Ask still
    // works from keyword retrieval.
    assert!(api.retriever.is_none());
    assert!(parts.lazy_embedder.is_none());

    // Configured: the model is not loaded at startup (it loads on first use, §9.1b).
    let dir = tempfile::tempdir().expect("model dir");
    std::fs::create_dir_all(dir.path().join("onnx")).expect("onnx");
    std::fs::write(dir.path().join("onnx/model.onnx"), b"").expect("model");
    std::fs::write(dir.path().join("tokenizer.json"), b"{}").expect("tokenizer");
    let lib = dir.path().join("libonnxruntime.so");
    std::fs::write(&lib, b"").expect("lib");
    let mut with_model = Config::default();
    with_model.ai.embedding.model_dir = Some(dir.path().to_path_buf());
    with_model.ai.embedding.onnxruntime_lib = Some(lib);
    let clock: Arc<dyn Clock> = Arc::new(db.clock.clone());
    let parts = stratad::ai::build(&with_model, db.app_db.clone(), clock.clone()).expect("parts");
    let lazy = parts.lazy_embedder.clone().expect("lazy embedder");
    assert_eq!(
        (
            strata_ai::Embedder::is_loaded(lazy.as_ref()),
            lazy.loads(),
            strata_ai::Embedder::model_id(lazy.as_ref())
        ),
        (false, 0, "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model")
    );
    let api = stratad::jobs::ai_api(&parts, &db.app_db, &vault, clock, db.ids.clone(), &with_model)
        .expect("ai api");
    assert!(api.retriever.is_some());
    db.cleanup().await.expect("cleanup");
}
