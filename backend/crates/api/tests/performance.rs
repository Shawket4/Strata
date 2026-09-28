//! Performance suite at the §16.7 scale: a 10 000-file synthetic vault
//! (`strata_testkit::SyntheticVault`: mixed Arabic/English notes, links, relations, people,
//! companies, nested places, documents, tasks) loaded into the complete production
//! composition, then timed against the budgets documented in `docs/TESTING.md` §
//! "Performance":
//!
//! | measurement | budget |
//! |---|---|
//! | import of the vault (`POST /import`, one commit) | [`IMPORT_BUDGET`] |
//! | full reindex (`VaultService::reindex`, what `stratad reindex` runs) | [`REINDEX_BUDGET`] |
//! | `GET /graph` assembly (median of 5) | [`GRAPH_BUDGET`] |
//! | keyword search, 200 queries | p50 [`SEARCH_P50_BUDGET`], p95 [`SEARCH_P95_BUDGET`] |
//! | `GET /sync/bootstrap`, every page | [`BOOTSTRAP_BUDGET`] |
//! | `POST /sync/push` of 500 note creates | [`PUSH_BUDGET`] |
//!
//! Ignored in normal runs (minutes of work). Run in release mode, alone:
//! `cargo test --release -p strata-api --test performance -- --ignored --nocapture --test-threads=1`
//! (`STRATA_PERF_FILES` overrides the vault size for quick checks; budgets apply at 10 000).
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::cast_precision_loss
)]

mod hardening;

use std::io::{Cursor, Write};
use std::time::{Duration, Instant};

use hardening::http::{Req, encode};
use hardening::{H, Options};
use pretty_assertions::assert_eq;
use strata_testkit::{SyntheticConfig, SyntheticVault};
use sync_model::ops::{self as o, Op};
use sync_model::{BootstrapPage, OpResult, PushRequest, PushResponse, Record, SyncOp};

/// Budgets at 10 000 files on the reference machine (docs/TESTING.md).
const IMPORT_BUDGET: Duration = Duration::from_secs(240);
const REINDEX_BUDGET: Duration = Duration::from_secs(180);
const GRAPH_BUDGET: Duration = Duration::from_secs(3);
const SEARCH_P50_BUDGET: Duration = Duration::from_millis(100);
const SEARCH_P95_BUDGET: Duration = Duration::from_millis(250);
const BOOTSTRAP_BUDGET: Duration = Duration::from_secs(30);
const PUSH_BUDGET: Duration = Duration::from_secs(120);

fn zip_of(vault: &SyntheticVault) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for f in &vault.files {
        z.start_file(f.path.as_str(), options).expect("entry");
        z.write_all(f.content.as_bytes()).expect("write");
    }
    z.finish().expect("zip").into_inner()
}

/// The `p`-th percentile (whole percent) of sorted samples: index `round(p / 100 · (len − 1))`.
fn percentile(sorted: &[Duration], p: usize) -> Duration {
    let last = sorted.len() - 1;
    sorted[((p.min(100) * last + 50) / 100).min(last)]
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

#[tokio::test]
#[ignore = "performance suite: minutes of work; run in release mode (see module docs)"]
async fn ten_thousand_file_vault_meets_the_budgets() {
    let files = std::env::var("STRATA_PERF_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);
    let full_size = files >= 10_000;
    let generated = Instant::now();
    let vault = SyntheticVault::generate(&SyntheticConfig {
        files,
        ..SyntheticConfig::default()
    });
    let generate_time = generated.elapsed();
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let archive = zip_of(&vault);
    let mut report = vec![format!(
        "synthetic vault: {} files ({} notes, {} people, {} companies, {} places, {} documents, {} tasks, {} links, {} relations), {:.1} MiB, zip {:.1} MiB, generated in {}",
        vault.files.len(),
        vault.counts.notes,
        vault.counts.people,
        vault.counts.companies,
        vault.counts.places,
        vault.counts.documents,
        vault.counts.tasks,
        vault.counts.links,
        vault.counts.relations,
        vault.total_bytes() as f64 / 1_048_576.0,
        archive.len() as f64 / 1_048_576.0,
        ms(generate_time)
    )];

    // Load: one import commit.
    let started = Instant::now();
    let resp = h
        .send(
            Some("import_vault"),
            &Req {
                timeout: Some(Duration::from_secs(1800)),
                ..Req::new("POST", "/api/v1/import")
                    .token(&alice.token)
                    .body(strata_api::wire::ZIP, archive)
            },
        )
        .await;
    let import_time = started.elapsed();
    assert_eq!(resp.status, 200, "{}", String::from_utf8_lossy(&resp.body));
    report.push(format!("import (POST /import): {}", ms(import_time)));
    let integrity = strata_client::operations::get_integrity(&alice.client)
        .await
        .expect("integrity");
    assert_eq!(integrity.warnings.len(), 0, "{:?}", integrity.warnings);

    // Full reindex.
    let scope = h.db.issuer.issue(alice.id);
    let started = Instant::now();
    let reindexed = h.vault.reindex(&scope).await.expect("reindex");
    let reindex_time = started.elapsed();
    assert!(
        reindexed >= vault.files.len(),
        "{reindexed} notes reindexed"
    );
    report.push(format!(
        "full reindex ({reindexed} notes): {}",
        ms(reindex_time)
    ));

    // Graph assembly.
    let mut graph_times = Vec::new();
    let mut graph_bytes = 0;
    for _ in 0..5 {
        let started = Instant::now();
        let resp = h
            .send(
                Some("get_graph"),
                &Req::new("GET", "/api/v1/graph").token(&alice.token),
            )
            .await;
        graph_times.push(started.elapsed());
        assert_eq!(resp.status, 200);
        graph_bytes = resp.body.len();
    }
    graph_times.sort();
    let graph_time = graph_times[2];
    report.push(format!(
        "GET /graph: median {} (min {}, max {}), {:.1} MiB",
        ms(graph_time),
        ms(graph_times[0]),
        ms(graph_times[4]),
        graph_bytes as f64 / 1_048_576.0
    ));

    // Keyword search.
    let mut latencies = Vec::new();
    let mut empty = 0;
    for q in vault.queries(200, 7) {
        let target = format!("/api/v1/search?q={}&mode=keyword&limit=20", encode(&q));
        let started = Instant::now();
        let resp = h
            .send(Some("search"), &Req::new("GET", target).token(&alice.token))
            .await;
        latencies.push(started.elapsed());
        assert_eq!(resp.status, 200);
        let results: strata_client::types::SearchResults =
            rmp_serde::from_slice(&resp.body).expect("results");
        if results.hits.is_empty() {
            empty += 1;
        }
    }
    latencies.sort();
    let (p50, p95) = (percentile(&latencies, 50), percentile(&latencies, 95));
    report.push(format!(
        "keyword search (200 queries): p50 {}, p95 {}, max {}, {empty} without hits",
        ms(p50),
        ms(p95),
        ms(latencies[latencies.len() - 1])
    ));

    // Bootstrap, every page.
    let started = Instant::now();
    let mut cursor: Option<String> = None;
    let (mut pages, mut notes, mut records) = (0, 0, 0);
    loop {
        let target = cursor.as_ref().map_or_else(
            || "/api/v1/sync/bootstrap".to_owned(),
            |c| format!("/api/v1/sync/bootstrap?cursor={c}"),
        );
        let resp = h
            .send(
                Some("sync_bootstrap"),
                &Req::new("GET", target).token(&alice.token),
            )
            .await;
        assert_eq!(resp.status, 200);
        let page: BootstrapPage = rmp_serde::from_slice(&resp.body).expect("page");
        pages += 1;
        records += page.records.len();
        notes += page
            .records
            .iter()
            .filter(|r| matches!(r, Record::Note(_)))
            .count();
        match page.next_cursor {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    let bootstrap_time = started.elapsed();
    assert_eq!(notes, vault.files.len(), "every note bootstrapped once");
    report.push(format!(
        "GET /sync/bootstrap: {pages} pages, {records} records ({notes} notes) in {}",
        ms(bootstrap_time)
    ));

    // Push of 500 ops.
    let ops: Vec<SyncOp> = (0..500u128)
        .map(|n| {
            SyncOp::new(
                ulid::Ulid(0x0199_0000_0000_0000_0000_0000_0000_0000 + n),
                None,
                Op::NoteCreate(o::NoteCreate {
                    created: strata_common::clock::default_test_epoch(),
                    id: ulid::Ulid(0x0199_1111_0000_0000_0000_0000_0000_0000 + n),
                    path: format!("inbox/Pushed {n:03}.md"),
                    content: format!("Pushed offline {n} — ملاحظة {n}\n"),
                    force: true,
                }),
            )
        })
        .collect();
    let request = PushRequest { ops };
    let body = rmp_serde::to_vec_named(&request).expect("encode");
    let started = Instant::now();
    let resp = h
        .send(
            Some("sync_push"),
            &Req {
                timeout: Some(Duration::from_secs(1800)),
                ..Req::new("POST", "/api/v1/sync/push")
                    .token(&alice.token)
                    .msgpack(body)
            },
        )
        .await;
    let push_time = started.elapsed();
    assert_eq!(resp.status, 200, "{}", String::from_utf8_lossy(&resp.body));
    let pushed: PushResponse = rmp_serde::from_slice(&resp.body).expect("push response");
    let applied = pushed
        .results
        .iter()
        .filter(|r| matches!(r.result, OpResult::Applied { .. }))
        .count();
    assert_eq!(applied, 500);
    report.push(format!(
        "POST /sync/push (500 note.create): {} ({} per op)",
        ms(push_time),
        ms(push_time / 500)
    ));

    for line in &report {
        eprintln!("perf: {line}");
    }
    if full_size {
        let over: Vec<String> = [
            ("import", import_time, IMPORT_BUDGET),
            ("reindex", reindex_time, REINDEX_BUDGET),
            ("graph", graph_time, GRAPH_BUDGET),
            ("search p50", p50, SEARCH_P50_BUDGET),
            ("search p95", p95, SEARCH_P95_BUDGET),
            ("bootstrap", bootstrap_time, BOOTSTRAP_BUDGET),
            ("push", push_time, PUSH_BUDGET),
        ]
        .iter()
        .filter(|(_, took, budget)| took > budget)
        .map(|(what, took, budget)| format!("{what}: {} > budget {}", ms(*took), ms(*budget)))
        .collect();
        assert_eq!(over, Vec::<String>::new());
    }
    h.finish().await;
}
