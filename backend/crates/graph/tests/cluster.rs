//! The `cluster` job (PLAN §9.2, §10, D10): Leiden communities written to
//! `.meta/clusters.json` and the `clusters` / `cluster_names` tables in one `ai:` commit with
//! change-log rows and one `cluster.updated`; determinism; stable IDs and names across runs
//! (only new or changed clusters are named); the AI-down and AI-disabled paths; retirement;
//! the resolution preference; the job through the runner; isolation between users.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::World;
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::{self, ids, render_input};
use strata_ai::request::input_hash;
use strata_common::{Clock, NoteId, UserId};
use strata_graph::cluster::{self, ClusterConfig, NamingInput, Planned};
use strata_index::UserScope;
use strata_jobs::{JobHandler, RecordedEvents, Runner, RunnerConfig};
use strata_testkit::Fixture;
use vault_format::clusters::{CLUSTERS_PATH, Clusters};

/// Two tight groups of four notes and a concept each, joined by one link, plus a loner.
struct Groups {
    a: Vec<NoteId>,
    b: Vec<NoteId>,
    pricing: NoteId,
    hiring: NoteId,
    lonely: NoteId,
}

fn related(links: &[&str]) -> String {
    let items: Vec<String> = links.iter().map(|l| format!("\"[[{l}]]\"")).collect();
    format!("related: [{}]\n", items.join(", "))
}

async fn groups(w: &World, s: &UserScope) -> Groups {
    let pricing = w
        .create(s, "concepts/Pricing.md", "---\nkind: concept\n---\n")
        .await;
    let hiring = w
        .create(s, "concepts/Hiring.md", "---\nkind: concept\n---\n")
        .await;
    let mut a = Vec::new();
    let names_a = ["Discount policy", "Price list", "Offer to Watanya", "Quote template"];
    for (i, name) in names_a.iter().enumerate() {
        let later: Vec<&str> = names_a[i + 1..].to_vec();
        let concepts = if i < 2 {
            "concepts: [\"[[Pricing]]\"]\n"
        } else {
            ""
        };
        let rel = if later.is_empty() {
            String::new()
        } else {
            related(&later)
        };
        a.push(
            w.create(
                s,
                &format!("notes/{name}.md"),
                &format!("---\n{rel}{concepts}---\n{name}.\n"),
            )
            .await,
        );
    }
    let mut b = Vec::new();
    let names_b = ["Hiring plan", "Interview notes", "Job ad", "Onboarding"];
    for (i, name) in names_b.iter().enumerate() {
        let later: Vec<&str> = names_b[i + 1..].to_vec();
        let concepts = if i < 2 {
            "concepts: [\"[[Hiring]]\"]\n"
        } else {
            ""
        };
        let rel = if later.is_empty() {
            String::new()
        } else {
            related(&later)
        };
        let body = if i == 0 {
            format!("{name}. See [[Quote template]].\n")
        } else {
            format!("{name}.\n")
        };
        b.push(
            w.create(
                s,
                &format!("notes/{name}.md"),
                &format!("---\n{rel}{concepts}---\n{body}"),
            )
            .await,
        );
    }
    let lonely = w.create(s, "notes/Lonely.md", "Alone.\n").await;
    Groups {
        a,
        b,
        pricing,
        hiring,
        lonely,
    }
}

fn read_clusters(w: &World, user: UserId) -> Clusters {
    Clusters::from_json(&w.read(user, CLUSTERS_PATH)).expect("clusters.json")
}

fn previous(w: &World, user: UserId) -> Option<Clusters> {
    std::fs::read_to_string(w.dir(user).join(CLUSTERS_PATH))
        .ok()
        .and_then(|t| Clusters::from_json(&t).ok())
}

/// The naming input the job will send now, with its fixture answer.
async fn push_names(w: &World, s: &UserScope, user: UserId, names: &[(u32, &str)]) -> String {
    let data = w.service(None).data(s).await.expect("data");
    let plan = cluster::plan(
        &data,
        previous(w, user).as_ref(),
        1.0,
        &ClusterConfig::default(),
    );
    let wanted: Vec<&Planned> = plan.clusters.iter().filter(|p| p.needs_name).collect();
    let user_input = render_input(&NamingInput::of(&wanted)).expect("input");
    let prompt = prompts::latest(ids::CLUSTER_NAMING).expect("prompt");
    let answer: Vec<serde_json::Value> = names
        .iter()
        .map(|(id, name)| json!({"cluster_id": id.to_string(), "name": name}))
        .collect();
    w.llm.push(
        &prompt.prompt_ref(),
        &input_hash(prompt.text, &user_input),
        Fixture::json(json!({ "names": answer })),
    );
    user_input
}

async fn rows(w: &World, user: UserId) -> (Vec<(NoteId, i64)>, Vec<(i64, String)>) {
    let mut tx = w.db.begin(user).await.expect("tx");
    let a: Vec<(NoteId, i64)> =
        sqlx::query_as("SELECT note_id, cluster_id FROM clusters ORDER BY note_id")
            .fetch_all(tx.conn())
            .await
            .expect("clusters");
    let n: Vec<(i64, String)> =
        sqlx::query_as("SELECT cluster_id, name FROM cluster_names ORDER BY cluster_id")
            .fetch_all(tx.conn())
            .await
            .expect("names");
    tx.commit().await.expect("commit");
    (a, n)
}

async fn cluster_changes(w: &World, user: UserId) -> Vec<(String, String, String)> {
    let mut tx = w.db.begin(user).await.expect("tx");
    let r: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT entity_type, entity_id, op FROM change_log \
         WHERE entity_type IN ('cluster_assignment', 'cluster_name') ORDER BY seq",
    )
    .fetch_all(tx.conn())
    .await
    .expect("log");
    tx.commit().await.expect("commit");
    r
}

fn sorted(mut v: Vec<NoteId>) -> Vec<NoteId> {
    v.sort();
    v
}

fn ulids(v: &[NoteId]) -> Vec<ulid::Ulid> {
    let mut u: Vec<ulid::Ulid> = v.iter().map(NoteId::as_ulid).collect();
    u.sort();
    u
}

#[tokio::test]
async fn the_first_run_writes_the_file_the_tables_and_the_change_log_in_one_ai_commit() {
    let w = World::new().await;
    let (alice, s) = w.user("alice").await;
    let g = groups(&w, &s).await;
    let input = push_names(&w, &s, alice, &[(1, "Pricing"), (2, "Hiring")]).await;
    // Central titles first (weighted degree, then title), concepts by frequency.
    assert_eq!(
        input,
        "{\n  \"clusters\": [\n    {\n      \"cluster_id\": \"1\",\n      \"titles\": [\n        \"Discount policy\",\n        \"Price list\",\n        \"Quote template\",\n        \"Offer to Watanya\",\n        \"Pricing\"\n      ],\n      \"concepts\": [\n        \"Pricing\"\n      ],\n      \"previous_name\": null\n    },\n    {\n      \"cluster_id\": \"2\",\n      \"titles\": [\n        \"Hiring plan\",\n        \"Interview notes\",\n        \"Job ad\",\n        \"Onboarding\",\n        \"Hiring\"\n      ],\n      \"concepts\": [\n        \"Hiring\"\n      ],\n      \"previous_name\": null\n    }\n  ]\n}"
    );
    let commits = w.log(alice).len();
    let out = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    assert_eq!(out.changed, vec!["1".to_owned(), "2".to_owned()]);
    assert_eq!(out.ai_calls, 1);
    assert_eq!(w.llm.calls().len(), 1);
    assert_eq!(w.llm.calls()[0].user, input);
    assert_eq!(w.log(alice).len(), commits + 1);
    assert_eq!(w.log(alice)[0], "ai: cluster .meta/clusters.json");
    assert_eq!(w.last_commit_paths(alice), vec![CLUSTERS_PATH.to_owned()]);

    let mut a_members = g.a.clone();
    a_members.push(g.pricing);
    let mut b_members = g.b.clone();
    b_members.push(g.hiring);
    let file: serde_json::Value =
        serde_json::from_str(&w.read(alice, CLUSTERS_PATH)).expect("json");
    assert_eq!(
        file,
        json!({
            "version": 1,
            "generated": "2026-09-27T12:00:00Z",
            "algorithm": "leiden",
            "clusters": [
                {"id": 1, "name": "Pricing", "named_by": "ai",
                 "notes": ulids(&a_members).iter().map(ToString::to_string).collect::<Vec<_>>()},
                {"id": 2, "name": "Hiring", "named_by": "ai",
                 "notes": ulids(&b_members).iter().map(ToString::to_string).collect::<Vec<_>>()},
            ],
            "next_id": 3,
        })
    );
    let (assign, names) = rows(&w, alice).await;
    let mut expected: Vec<(NoteId, i64)> = a_members
        .iter()
        .map(|n| (*n, 1))
        .chain(b_members.iter().map(|n| (*n, 2)))
        .collect();
    expected.sort();
    assert_eq!(assign, expected);
    assert_eq!(names, vec![(1, "Pricing".into()), (2, "Hiring".into())]);
    assert!(!assign.iter().any(|(n, _)| *n == g.lonely), "loners stay unclustered");
    // Sync: one upsert per assignment and per name.
    let log = cluster_changes(&w, alice).await;
    let mut expected_log: Vec<(String, String, String)> = sorted(
        a_members.iter().chain(&b_members).copied().collect(),
    )
    .into_iter()
    .map(|n| ("cluster_assignment".into(), n.to_string(), "upsert".into()))
    .collect();
    expected_log.push(("cluster_name".into(), "1".into(), "upsert".into()));
    expected_log.push(("cluster_name".into(), "2".into(), "upsert".into()));
    assert_eq!(log, expected_log);
    assert_eq!(
        w.events.take(),
        vec![(alice, vec!["1".to_owned(), "2".to_owned()])]
    );

    // The graph now carries the clusters.
    let graph = w
        .service(None)
        .graph(&s, &strata_graph::query::GraphQuery::default())
        .await
        .expect("graph");
    assert_eq!(
        graph
            .clusters
            .iter()
            .map(|c| (c.id.as_str(), c.name.as_str(), c.size))
            .collect::<Vec<_>>(),
        vec![("1", "Pricing", 5), ("2", "Hiring", 5)]
    );
    let lonely = graph.nodes.iter().find(|n| n.id == g.lonely).expect("lonely");
    assert_eq!(lonely.cluster_id, None);

    // Same graph again: deterministic, nothing to write, no call, no event.
    let again = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    assert_eq!(
        (again.commit, again.changed, again.ai_calls),
        (None, vec![], 0)
    );
    assert_eq!(w.llm.calls().len(), 1);
    assert_eq!(w.log(alice).len(), commits + 1);
    assert_eq!(w.events.take(), vec![]);
    w.finish().await;
}

#[tokio::test]
async fn ids_and_names_persist_and_only_changed_clusters_are_renamed() {
    let w = World::new().await;
    let (alice, s) = w.user("alice").await;
    let g = groups(&w, &s).await;
    push_names(&w, &s, alice, &[(1, "Pricing"), (2, "Hiring")]).await;
    w.clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("first");
    w.events.take();
    // A fifth hiring note joins cluster 2.
    let extra = w
        .create(
            &s,
            "notes/Offer letter.md",
            "---\nrelated: [\"[[Hiring plan]]\", \"[[Job ad]]\", \"[[Onboarding]]\"]\n---\nOffer.\n",
        )
        .await;
    let input = push_names(&w, &s, alice, &[(2, "Recruiting")]).await;
    assert!(input.contains("\"cluster_id\": \"2\""));
    assert!(input.contains("\"previous_name\": \"Hiring\""));
    assert!(!input.contains("\"cluster_id\": \"1\""), "cluster 1 is unchanged");
    let out = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("second");
    assert_eq!(out.changed, vec!["2".to_owned()]);
    assert_eq!(w.llm.calls().len(), 2);
    let file = read_clusters(&w, alice);
    let summary: Vec<(u32, String, usize)> = file
        .clusters
        .iter()
        .map(|c| (c.id, c.name.clone(), c.notes.len()))
        .collect();
    assert_eq!(
        summary,
        vec![(1, "Pricing".into(), 5), (2, "Recruiting".into(), 6)]
    );
    assert!(file.clusters[1].notes.contains(&extra.as_ulid()));
    // Only the new member and the renamed cluster are logged.
    let log = cluster_changes(&w, alice).await;
    assert_eq!(
        log[log.len() - 2..].to_vec(),
        vec![
            ("cluster_assignment".into(), extra.to_string(), "upsert".into()),
            ("cluster_name".into(), "2".into(), "upsert".into()),
        ]
    );
    assert_eq!(w.events.take(), vec![(alice, vec!["2".to_owned()])]);

    // The hiring notes go to the trash: cluster 2 is retired, its ID never reused.
    for n in g.b.iter().chain([&g.hiring, &extra]) {
        w.vault.delete_note(&s, *n).await.expect("trash");
    }
    let out = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("third");
    assert_eq!(out.changed, vec!["2".to_owned()]);
    let file = read_clusters(&w, alice);
    assert_eq!(
        file.clusters.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(file.extra.get("next_id"), Some(&json!(3)));
    let (_, names) = rows(&w, alice).await;
    assert_eq!(names, vec![(1, "Pricing".into())]);
    let log = cluster_changes(&w, alice).await;
    assert_eq!(
        log.last().cloned(),
        Some(("cluster_name".into(), "2".into(), "delete".into()))
    );
    w.finish().await;
}

#[tokio::test]
async fn with_the_ai_down_clusters_are_written_with_placeholder_names_and_named_later() {
    let w = World::new().await;
    let (alice, s) = w.user("alice").await;
    groups(&w, &s).await;
    // No fixture: the provider fails.
    let out = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    assert_eq!(out.changed, vec!["1".to_owned(), "2".to_owned()]);
    let file = read_clusters(&w, alice);
    assert_eq!(
        file.clusters
            .iter()
            .map(|c| (c.id, c.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "Cluster 1"), (2, "Cluster 2")]
    );
    assert_eq!(file.extra.get("unnamed"), Some(&json!([1, 2])));
    // The AI is back: the placeholders are named, membership unchanged.
    push_names(&w, &s, alice, &[(1, "Pricing"), (2, "Hiring")]).await;
    let out = w
        .clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    assert_eq!(out.changed, vec!["1".to_owned(), "2".to_owned()]);
    let file = read_clusters(&w, alice);
    assert_eq!(
        file.clusters
            .iter()
            .map(|c| (c.id, c.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "Pricing"), (2, "Hiring")]
    );
    assert_eq!(file.extra.get("unnamed"), None);
    // A later outage keeps the previous names.
    w.create(
        &s,
        "notes/Offer letter.md",
        "---\nrelated: [\"[[Hiring plan]]\", \"[[Job ad]]\", \"[[Onboarding]]\"]\n---\nOffer.\n",
    )
    .await;
    w.clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    let file = read_clusters(&w, alice);
    assert_eq!(
        (file.clusters[1].name.as_str(), file.extra.get("unnamed")),
        ("Hiring", Some(&json!([2])))
    );
    w.finish().await;
}

#[tokio::test]
async fn users_without_ai_get_placeholder_names_without_calls() {
    let w = World::new().await;
    let (noai, s) = w.user("noai").await;
    groups(&w, &s).await;
    let out = w
        .clusterer()
        .run_for(&s, "noai", w.db.clock.now())
        .await
        .expect("run");
    assert_eq!(out.ai_calls, 0);
    assert_eq!(w.llm.calls().len(), 0);
    let (_, names) = rows(&w, noai).await;
    assert_eq!(
        names,
        vec![(1, "Cluster 1".into()), (2, "Cluster 2".into())]
    );
    w.finish().await;
}

#[tokio::test]
async fn the_resolution_preference_changes_the_partition() {
    let w = World::new().await;
    let (alice, s) = w.user("alice").await;
    groups(&w, &s).await;
    let mut tx = w.db.begin(alice).await.expect("tx");
    let prefs: BTreeMap<String, String> =
        [("graph.cluster_resolution".to_owned(), "0.05".to_owned())].into();
    strata_index::repo::settings::put_setting(
        &mut tx,
        "preferences",
        &rmp_serde::to_vec(&prefs).expect("encode"),
        w.db.clock.now(),
    )
    .await
    .expect("pref");
    tx.commit().await.expect("commit");
    // A very low resolution merges both groups into one cluster.
    w.clusterer()
        .run_for(&s, "alice", w.db.clock.now())
        .await
        .expect("run");
    let file = read_clusters(&w, alice);
    assert_eq!(
        file.clusters
            .iter()
            .map(|c| (c.id, c.notes.len()))
            .collect::<Vec<_>>(),
        vec![(1, 10)]
    );
    w.finish().await;
}

#[tokio::test]
async fn the_job_runs_through_the_runner_and_never_touches_another_user() {
    let w = World::new().await;
    let (alice, sa) = w.user("alice").await;
    let (bob, sb) = w.user("bob").await;
    let g = groups(&w, &sa).await;
    let bob_note = w.create(&sb, "notes/Discount policy.md", "Bob's.\n").await;
    push_names(&w, &sa, alice, &[(1, "Pricing"), (2, "Hiring")]).await;
    let mut tx = w.db.begin(alice).await.expect("tx");
    cluster::enqueue(&mut tx, w.db.ids.as_ref(), "manual", w.db.clock.now())
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
    let handler: Arc<dyn JobHandler> = Arc::new(w.clusterer());
    let runner = Runner::new(
        w.db.app_db.clone(),
        w.db.issuer.clone(),
        Arc::new(w.db.clock.clone()),
        Arc::new(RecordedEvents::default()),
        RunnerConfig {
            idle_recheck: chrono::Duration::zero(),
            ..RunnerConfig::default()
        },
        vec![handler],
    );
    runner.run_until_idle().await;
    let mut tx = w.db.begin(alice).await.expect("tx");
    let jobs: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, status FROM jobs WHERE kind = 'cluster'")
            .fetch_all(tx.conn())
            .await
            .expect("jobs");
    tx.commit().await.expect("commit");
    assert_eq!(jobs, vec![("cluster".to_owned(), "done".to_owned())]);
    let file = read_clusters(&w, alice);
    let members: Vec<ulid::Ulid> = file.clusters.iter().flat_map(|c| c.notes.clone()).collect();
    assert_eq!(members.len(), 10);
    assert!(!members.contains(&bob_note.as_ulid()));
    assert!(members.contains(&g.a[0].as_ulid()));
    // Bob: no file, no rows, no changes, no events.
    assert!(!w.dir(bob).join(CLUSTERS_PATH).exists());
    assert_eq!(rows(&w, bob).await, (vec![], vec![]));
    assert_eq!(cluster_changes(&w, bob).await, vec![]);
    assert!(w.events.take().iter().all(|(u, _)| *u == alice));
    w.finish().await;
}

#[tokio::test]
async fn planning_is_deterministic_and_user_names_are_kept() {
    let w = World::new().await;
    let (alice, s) = w.user("alice").await;
    groups(&w, &s).await;
    let data = w.service(None).data(&s).await.expect("data");
    let cfg = ClusterConfig::default();
    let first = cluster::plan(&data, None, 1.0, &cfg);
    assert_eq!(cluster::plan(&data, None, 1.0, &cfg), first);
    // A user-named cluster keeps its name even when its members change.
    let now = w.db.clock.now();
    let mut file = cluster::clusters_file(&first, &BTreeMap::new(), None, now);
    file.clusters[1].name = "My hiring".into();
    file.clusters[1].named_by = vault_format::sidecar::By::User;
    file.clusters[1].notes.pop();
    let next = cluster::plan(&data, Some(&file), 1.0, &cfg);
    assert_eq!(
        next.clusters
            .iter()
            .map(|p| (p.id, p.changed, p.needs_name))
            .collect::<Vec<_>>(),
        vec![(1, false, true), (2, true, false)]
    );
    let out = cluster::clusters_file(&next, &BTreeMap::new(), Some(&file), now);
    assert_eq!(out.clusters[1].name, "My hiring");
    let _ = alice;
    w.finish().await;
}
