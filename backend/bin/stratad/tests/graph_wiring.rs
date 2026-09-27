//! `stratad serve` wiring of the graph component (PLAN §9.2, §9.6, §10): the `cluster` job
//! is registered next to the standard handlers and scheduled nightly, and the graph API
//! gets similarity edges exactly when an embedding model is configured.
#![allow(clippy::expect_used)] // tests: expect with messages

use std::sync::Arc;

use pretty_assertions::assert_eq;
use strata_api::events::{BusConfig, EventBus};
use strata_common::{Clock, Config};
use strata_graph::assemble::SimilarityStatus;
use strata_graph::query::GraphQuery;
use strata_jobs::{Cadence, Periodic};
use strata_testkit::{TempDataRoot, TestDb, TestUser};

#[tokio::test]
async fn the_cluster_job_is_registered_and_scheduled_nightly() {
    let db = TestDb::new().await.expect("db");
    let data = TempDataRoot::new().expect("data");
    let config = Config {
        data_root: data.path().to_path_buf(),
        ..Config::default()
    };
    let clock: Arc<dyn Clock> = Arc::new(db.clock.clone());
    let parts = stratad::ai::build(&config, db.app_db.clone(), clock.clone()).expect("parts");
    let vault =
        stratad::serve::vault_service(&config, db.app_db.clone(), clock.clone(), db.ids.clone());
    let deps = strata_jobs::Deps {
        db: db.app_db.clone(),
        vault,
        ai: Arc::new(parts.service.clone()),
        embedder: None,
        clock,
        ids: db.ids.clone(),
        thresholds: strata_vault::dup::thresholds(&std::collections::BTreeMap::new()),
    };
    let kinds: Vec<&str> = stratad::jobs::handlers(&deps, Arc::new(EventBus::new(BusConfig::default())))
        .iter()
        .map(|h| h.kind())
        .collect();
    assert_eq!(
        kinds,
        vec!["embed", "embed_backfill", "summarize", "dedupe", "cluster"]
    );
    assert_eq!(
        stratad::jobs::periodic(),
        vec![
            Periodic {
                kind: "dedupe",
                cadence: Cadence::Nightly
            },
            Periodic {
                kind: "cluster",
                cadence: Cadence::Nightly
            },
        ]
    );
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn similarity_edges_follow_the_embedding_configuration() {
    let db = TestDb::new().await.expect("db");
    let data = TempDataRoot::new().expect("data");
    let alice = TestUser::new("alice").create(&db).await.expect("user").id;
    let scope = db.scope(alice);
    let config = Config {
        data_root: data.path().to_path_buf(),
        ..Config::default()
    };
    let clock: Arc<dyn Clock> = Arc::new(db.clock.clone());
    let vault =
        stratad::serve::vault_service(&config, db.app_db.clone(), clock.clone(), db.ids.clone());
    let query = GraphQuery {
        include_similarity: true,
        ..GraphQuery::default()
    };

    let parts = stratad::ai::build(&config, db.app_db.clone(), clock.clone()).expect("parts");
    let api = stratad::jobs::graph_api(&parts, &db.app_db, &vault, clock.clone(), db.ids.clone());
    let g = api.graph.graph(&scope, &query).await.expect("graph");
    assert_eq!(g.similarity, SimilarityStatus::Unavailable);

    // With a model configured (loaded on demand, never at startup).
    let dir = tempfile::tempdir().expect("model dir");
    std::fs::create_dir_all(dir.path().join("onnx")).expect("onnx");
    std::fs::write(dir.path().join("onnx/model.onnx"), b"").expect("model");
    std::fs::write(dir.path().join("tokenizer.json"), b"{}").expect("tokenizer");
    let lib = dir.path().join("libonnxruntime.so");
    std::fs::write(&lib, b"").expect("lib");
    let mut with_model = config.clone();
    with_model.ai.embedding.model_dir = Some(dir.path().to_path_buf());
    with_model.ai.embedding.onnxruntime_lib = Some(lib);
    let parts = stratad::ai::build(&with_model, db.app_db.clone(), clock.clone()).expect("parts");
    let api = stratad::jobs::graph_api(&parts, &db.app_db, &vault, clock, db.ids.clone());
    let g = api.graph.graph(&scope, &query).await.expect("graph");
    assert_eq!(g.similarity, SimilarityStatus::Complete);
    db.cleanup().await.expect("cleanup");
}
