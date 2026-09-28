//! Performance gate (PLAN §16.7): `GET /graph` assembly for a 10k-note synthetic vault —
//! 10,000 notes, 20,000 body links, 20,000 relations (half AI with a confidence and a
//! reason), a hover summary for every note, and every note clustered — must finish within
//! [`BUDGET`]: the scoped index read, the sidecar summaries and the payload assembly
//! together. The median of three runs is compared.
//!
//! Measured in the dev container (4 vCPU Intel Xeon @ 2.10 GHz, `cargo test` debug profile,
//! `PostgreSQL` 16 on the same host, 2026-09-27): 0.53 s median. The 2 s budget leaves about
//! 4× headroom for slower CI machines; the release build is several times faster.
#![allow(
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

mod common;

use std::time::{Duration, Instant};

use common::World;
use strata_common::NoteId;
use strata_graph::query::GraphQuery;
use ulid::Ulid;

const NOTES: u64 = 10_000;
const BUDGET: Duration = Duration::from_secs(2);

fn id(i: u64) -> NoteId {
    NoteId::from_ulid(Ulid::from_parts(1_800_000_000_000 + i, u128::from(i)))
}

#[tokio::test]
async fn global_graph_assembly_of_10k_notes_stays_within_budget() {
    let w = World::new().await;
    let (user, s) = w.user("alice").await;
    let ids: Vec<NoteId> = (0..NOTES).map(id).collect();
    let paths: Vec<String> = (0..NOTES).map(|i| format!("notes/n{i}.md")).collect();
    let titles: Vec<String> = (0..NOTES).map(|i| format!("Note {i}")).collect();
    let mut tx = w.db.app_db.begin(&s).await.expect("tx");
    sqlx::query(
        "INSERT INTO notes (user_id, id, path, title, kind, lang, created, updated, content_hash) \
         SELECT strata_current_user(), u.id, u.path, u.title, 'note', 'en', now(), now(), 'sha256:x' \
         FROM unnest($1::uuid[], $2::text[], $3::text[]) AS u(id, path, title)",
    )
    .bind(&ids)
    .bind(&paths)
    .bind(&titles)
    .execute(tx.conn())
    .await
    .expect("notes");
    // Each note links to the next two and relates to the notes 7 and 13 ahead.
    let n = NOTES as i64;
    sqlx::query(
        "INSERT INTO links (user_id, src_id, ord, dst_id, dst_raw, kind) \
         SELECT strata_current_user(), a.id, k, b.id, 'x', 'link' \
         FROM unnest($1::uuid[]) WITH ORDINALITY AS a(id, i) \
         CROSS JOIN generate_series(0, 1) AS k \
         JOIN unnest($1::uuid[]) WITH ORDINALITY AS b(id, j) ON b.j = ((a.i + k) % $2) + 1",
    )
    .bind(&ids)
    .bind(n)
    .execute(tx.conn())
    .await
    .expect("links");
    sqlx::query(
        "INSERT INTO relations (user_id, src_id, dst_id, type, by, confidence, reason, created) \
         SELECT strata_current_user(), a.id, b.id, 'related', \
           CASE WHEN step = 7 THEN 'user' ELSE 'ai' END, \
           CASE WHEN step = 7 THEN NULL ELSE 0.8 END, \
           CASE WHEN step = 7 THEN NULL ELSE 'shared topic' END, now() \
         FROM unnest($1::uuid[]) WITH ORDINALITY AS a(id, i) \
         CROSS JOIN (VALUES (7), (13)) AS s(step) \
         JOIN unnest($1::uuid[]) WITH ORDINALITY AS b(id, j) ON b.j = ((a.i + step - 1) % $2) + 1",
    )
    .bind(&ids)
    .bind(n)
    .execute(tx.conn())
    .await
    .expect("relations");
    let clusters: Vec<i64> = (0..NOTES as i64).map(|i| i / 50 + 1).collect();
    sqlx::query(
        "INSERT INTO clusters (user_id, note_id, cluster_id) \
         SELECT strata_current_user(), u.id, u.c FROM unnest($1::uuid[], $2::bigint[]) AS u(id, c)",
    )
    .bind(&ids)
    .bind(&clusters)
    .execute(tx.conn())
    .await
    .expect("clusters");
    tx.commit().await.expect("commit");
    let meta = w.dir(user).join(".meta/notes");
    std::fs::create_dir_all(&meta).expect("meta");
    for i in &ids {
        std::fs::write(
            meta.join(format!("{i}.json")),
            format!("{{\"id\": \"{i}\", \"summary\": \"A synthetic note used by the performance gate of the graph endpoint.\"}}"),
        )
        .expect("sidecar");
    }

    let svc = w.service(None);
    let mut times = Vec::new();
    let mut last = None;
    for _ in 0..3 {
        let start = Instant::now();
        let g = svc.graph(&s, &GraphQuery::default()).await.expect("graph");
        times.push(start.elapsed());
        last = Some(g);
    }
    let g = last.expect("graph");
    assert_eq!(
        (g.nodes.len(), g.edges.len(), g.clusters.len()),
        (10_000, 40_000, 200)
    );
    assert!(g.nodes.iter().all(|n| n.summary.is_some() && n.degree == 8));
    times.sort();
    let median = times[1];
    eprintln!("10k-note graph assembly: {times:?} (median {median:?})");
    assert!(
        median <= BUDGET,
        "graph assembly took {median:?}, budget {BUDGET:?}"
    );
    w.finish().await;
}
