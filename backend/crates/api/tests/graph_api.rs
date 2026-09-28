//! Graph and map endpoints (PLAN §6.8, §7.5 Graph, §10) through the generated client with
//! MessagePack on the wire; every response is validated against the production contract:
//! exact graph payloads, filters, the entity lens, local neighbourhoods, parameter problems,
//! isolation, map CRUD with validation and optimistic concurrency, canvas rewrite on note
//! rename, the rate-limited recluster trigger, and `cluster.updated` on the event bus.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod graph_harness;

use std::sync::Arc;

use graph_harness::{H, assert_problem, field, plain};
use pretty_assertions::assert_eq;
use strata_api::events::{BusConfig, Event, EventBus};
use strata_api::graph::{BusClusterEvents, RECLUSTER_LIMIT};
use strata_api::wire::ws::Frame;
use strata_client::{operations as ops, types};
use strata_common::config::RateLimit;
use strata_graph::cluster::ClusterEvents;

fn node(
    id: strata_common::NoteId,
    title: &str,
    kind: types::NoteKind,
    path: &str,
    degree: u32,
    depth: Option<i32>,
    updated: chrono::DateTime<chrono::Utc>,
) -> types::GraphNode {
    types::GraphNode {
        cluster_id: None,
        degree,
        depth,
        id: id.as_ulid(),
        kind,
        lang: None,
        path: path.to_owned(),
        summary: None,
        title: title.to_owned(),
        updated,
    }
}

fn user_edge(
    source: strata_common::NoteId,
    target: strata_common::NoteId,
    kind: &str,
) -> types::GraphEdge {
    types::GraphEdge {
        by: Some("user".into()),
        confidence: None,
        kind: kind.to_owned(),
        notes: None,
        reason: None,
        source: source.as_ulid(),
        target: target.as_ulid(),
        weight: None,
    }
}

#[tokio::test]
async fn graph_payloads_filters_lens_and_local_neighbourhoods() {
    let h = H::new(RECLUSTER_LIMIT).await;
    let alice = h.user("alice").await;
    let a = alice.id;
    let shady = h
        .note(a, "people/Shady.md", "---\nkind: person\n---\n")
        .await;
    let mona = h
        .note(
            a,
            "people/Mona.md",
            "---\nkind: person\nknows: [\"[[Shady]]\"]\n---\n",
        )
        .await;
    let plan = h.note(a, "notes/Plan.md", "Plan.\n").await;
    let call = h
        .note(
            a,
            "notes/Call.md",
            "---\npeople: [\"[[Shady]]\", \"[[Mona]]\"]\n---\nSee [[Plan]].\n",
        )
        .await;
    let t = h.now();
    let c = &alice.client;

    let g = ops::get_graph(c, None, None, None, None)
        .await
        .expect("graph");
    assert_eq!(
        g,
        types::Graph {
            clusters: vec![],
            edges: vec![
                user_edge(mona, shady, "entity:knows"),
                user_edge(call, shady, "mention"),
                user_edge(call, mona, "mention"),
                user_edge(call, plan, "link"),
            ],
            nodes: vec![
                node(
                    shady,
                    "Shady",
                    types::NoteKind::Person,
                    "people/Shady.md",
                    2,
                    None,
                    t
                ),
                node(
                    mona,
                    "Mona",
                    types::NoteKind::Person,
                    "people/Mona.md",
                    2,
                    None,
                    t
                ),
                node(
                    plan,
                    "Plan",
                    types::NoteKind::Note,
                    "notes/Plan.md",
                    1,
                    None,
                    t
                ),
                node(
                    call,
                    "Call",
                    types::NoteKind::Note,
                    "notes/Call.md",
                    3,
                    None,
                    t
                ),
            ],
            similarity: types::SimilarityStatus::Off,
        }
    );

    // Filters, and similarity without a model.
    let g = ops::get_graph(c, Some("link"), Some("note"), Some(true), None)
        .await
        .expect("filtered");
    assert_eq!(
        (g.edges, g.nodes.len(), g.similarity),
        (
            vec![user_edge(call, plan, "link")],
            2,
            types::SimilarityStatus::Unavailable
        )
    );

    // People lens: Call names both people, so they are tied with strength 1.
    let g = ops::get_graph(c, None, None, None, Some("people"))
        .await
        .expect("lens");
    assert_eq!(
        g.edges,
        vec![
            types::GraphEdge {
                by: None,
                confidence: None,
                kind: "co-mention".into(),
                notes: Some(1),
                reason: None,
                source: shady.as_ulid(),
                target: mona.as_ulid(),
                weight: Some(1.0),
            },
            user_edge(mona, shady, "entity:knows"),
        ]
    );
    assert_eq!(
        g.nodes
            .iter()
            .map(|n| (n.title.as_str(), n.degree))
            .collect::<Vec<_>>(),
        vec![("Shady", 2), ("Mona", 2)]
    );

    // Local: Plan at depth 1 and 2.
    let l = ops::get_local_graph(c, plan.as_ulid(), Some(1), None, None, None)
        .await
        .expect("local");
    assert_eq!(
        l.nodes,
        vec![
            node(
                plan,
                "Plan",
                types::NoteKind::Note,
                "notes/Plan.md",
                1,
                Some(0),
                t
            ),
            node(
                call,
                "Call",
                types::NoteKind::Note,
                "notes/Call.md",
                1,
                Some(1),
                t
            ),
        ]
    );
    let l = ops::get_local_graph(c, plan.as_ulid(), Some(2), Some("link,mention"), None, None)
        .await
        .expect("local");
    assert_eq!(
        l.nodes
            .iter()
            .map(|n| (n.title.as_str(), n.depth))
            .collect::<Vec<_>>(),
        vec![
            ("Shady", Some(2)),
            ("Mona", Some(2)),
            ("Plan", Some(0)),
            ("Call", Some(1))
        ]
    );
    assert_eq!(l.edges.len(), 3, "entity:knows is filtered out");

    // Parameter problems.
    assert_problem(
        ops::get_graph(c, Some("link,likes"), None, None, None).await,
        &types::Problem {
            errors: vec![field(
                "unknown_edge_type",
                "types",
                "unknown edge type `likes`",
            )],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("unknown edge type `likes`"),
            )
        },
    );
    assert_problem(
        ops::get_graph(c, None, Some("tag"), None, None).await,
        &types::Problem {
            errors: vec![field(
                "unknown_node_kind",
                "kinds",
                "unknown node kind `tag`",
            )],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("unknown node kind `tag`"),
            )
        },
    );
    assert_problem(
        ops::get_graph(c, None, None, None, Some("places")).await,
        &types::Problem {
            errors: vec![field(
                "unknown_lens",
                "lens",
                "unknown lens `places`; use people or companies",
            )],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("unknown lens `places`; use people or companies"),
            )
        },
    );
    assert_problem(
        ops::get_local_graph(c, plan.as_ulid(), Some(4), None, None, None).await,
        &types::Problem {
            errors: vec![field("invalid_depth", "depth", "depth must be 1, 2 or 3")],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("depth must be 1, 2 or 3"),
            )
        },
    );

    // Isolation: Bob sees none of Alice's graph; her note IDs are 404 for him.
    let bob = h.user("bob").await;
    let g = ops::get_graph(&bob.client, None, None, None, None)
        .await
        .expect("bob");
    assert_eq!((g.nodes.len(), g.edges.len()), (0, 0));
    assert_problem(
        ops::get_local_graph(&bob.client, plan.as_ulid(), None, None, None, None).await,
        &plain("not_found", "Not found", 404, None),
    );
    h.finish().await;
}

const CANVAS: &str = r#"{
  "nodes": [
    {"id": "n1", "type": "file", "file": "notes/Plan.md", "x": 0, "y": 0, "width": 250, "height": 60},
    {"id": "n2", "type": "text", "text": "Idea", "x": 300, "y": 0, "width": 200, "height": 60, "color": "4"}
  ],
  "edges": [
    {"id": "e1", "fromNode": "n1", "toNode": "n2", "label": "related"}
  ]
}"#;

const STORED: &str = "{\n\t\"nodes\":[\n\t\t{\"id\":\"n1\",\"type\":\"file\",\"file\":\"notes/Plan.md\",\"x\":0,\"y\":0,\"width\":250,\"height\":60},\n\t\t{\"id\":\"n2\",\"type\":\"text\",\"text\":\"Idea\",\"x\":300,\"y\":0,\"width\":200,\"height\":60,\"color\":\"4\"}\n\t],\n\t\"edges\":[\n\t\t{\"id\":\"e1\",\"fromNode\":\"n1\",\"toNode\":\"n2\",\"label\":\"related\"}\n\t]\n}";

fn put(content: &str) -> types::PutMapRequest {
    types::PutMapRequest {
        content: content.to_owned(),
    }
}

#[tokio::test]
async fn maps_are_saved_as_validated_json_canvas_and_follow_note_renames() {
    let h = H::new(RECLUSTER_LIMIT).await;
    let alice = h.user("alice").await;
    let a = alice.id;
    let plan = h.note(a, "notes/Plan.md", "Plan.\n").await;
    let c = &alice.client;
    assert_eq!(
        ops::list_maps(c).await.expect("empty"),
        types::MapList { maps: vec![] }
    );

    // Create (no If-Match): stored in Obsidian's layout, one user commit.
    let created = ops::put_map(c, "Roadmap", None, &put(CANVAS))
        .await
        .expect("create");
    let version = strata_vault::fsio::version_of(STORED.as_bytes());
    let expected = types::Map {
        content: STORED.to_owned(),
        files: vec![types::MapFile {
            node_id: "n1".into(),
            note_id: Some(plan.as_ulid()),
            path: "notes/Plan.md".into(),
        }],
        id: "Roadmap".into(),
        path: "maps/Roadmap.canvas".into(),
        version: version.clone(),
    };
    assert_eq!(created, expected);
    assert_eq!(h.read(a, "maps/Roadmap.canvas"), STORED);
    assert_eq!(h.log(a)[0], "user: save map maps/Roadmap.canvas");
    assert_eq!(ops::get_map(c, "Roadmap").await.expect("get"), expected);
    assert_eq!(
        ops::list_maps(c).await.expect("list"),
        types::MapList {
            maps: vec![types::MapSummary {
                edges: Some(1),
                id: "Roadmap".into(),
                nodes: Some(2),
                path: "maps/Roadmap.canvas".into(),
                version: version.clone(),
            }]
        }
    );

    // Existing map: If-Match required and checked.
    let conflict = types::Problem {
        current_version: Some(version.clone()),
        ..plain("version_conflict", "Version conflict", 409, None)
    };
    assert_problem(
        ops::put_map(c, "Roadmap", None, &put(CANVAS)).await,
        &conflict,
    );
    assert_problem(
        ops::put_map(c, "Roadmap", Some("sha256:0000"), &put(CANVAS)).await,
        &conflict,
    );
    let moved = CANVAS.replace("\"x\": 300", "\"x\": 320");
    let updated = ops::put_map(c, "Roadmap", Some(&version), &put(&moved))
        .await
        .expect("update");
    assert_eq!(updated.content, STORED.replace("\"x\":300", "\"x\":320"));
    assert_eq!(h.log(a)[0], "user: save map maps/Roadmap.canvas");

    // Validation: every problem is listed; nothing is written.
    let commits = h.log(a).len();
    assert_problem(
        ops::put_map(c, "Bad", None, &put("{\"nodes\": 5}")).await,
        &types::Problem {
            errors: vec![field(
                "invalid_canvas",
                "/content",
                "invalid canvas: `nodes` must be an array",
            )],
            ..plain(
                "invalid_body",
                "Request body is invalid",
                422,
                Some("the map is not a valid canvas for this vault"),
            )
        },
    );
    let bad = r##"{"nodes": [
        {"id": "a", "type": "file", "file": "notes/Missing.md", "x": 0, "y": 0, "width": 10, "height": 10},
        {"id": "b", "type": "file", "file": ".meta/notes/x.json", "x": 0, "y": 0, "width": 0, "height": 10, "color": "#12"}
      ],
      "edges": [{"id": "e", "fromNode": "a", "toNode": "zz"}]}"##;
    assert_problem(
        ops::put_map(c, "Bad", None, &put(bad)).await,
        &types::Problem {
            errors: vec![
                field(
                    "bad_size",
                    "/content",
                    "node `b` needs a positive width and height",
                ),
                field(
                    "bad_color",
                    "/content",
                    "colour `#12` is neither a preset 1-6 nor #RRGGBB",
                ),
                field(
                    "dangling_edge",
                    "/content",
                    "edge `e` points at missing node `zz`",
                ),
                field(
                    "invalid_file",
                    "/content",
                    "file node references `.meta/notes/x.json`, which is not a file of this vault",
                ),
                field(
                    "unknown_file",
                    "/content",
                    "file node references `notes/Missing.md`, which is not a file of this vault",
                ),
            ],
            ..plain(
                "invalid_body",
                "Request body is invalid",
                422,
                Some("the map is not a valid canvas for this vault"),
            )
        },
    );
    assert_eq!(h.log(a).len(), commits);
    assert_problem(
        ops::get_map(c, "Missing").await,
        &plain("not_found", "Not found", 404, None),
    );

    // Renaming the note rewrites the canvas in the same commit.
    ops::move_note(
        c,
        plan.as_ulid(),
        None,
        &types::MoveNoteRequest {
            new_path: "notes/Roadmap plan.md".into(),
        },
    )
    .await
    .expect("move");
    let after = ops::get_map(c, "Roadmap").await.expect("get");
    assert_eq!(
        after.files,
        vec![types::MapFile {
            node_id: "n1".into(),
            note_id: Some(plan.as_ulid()),
            path: "notes/Roadmap plan.md".into(),
        }]
    );
    assert!(after.content.contains("\"file\":\"notes/Roadmap plan.md\""));
    assert_eq!(
        h.log(a)[0],
        "user: move notes/Plan.md -> notes/Roadmap plan.md"
    );

    // Isolation: Bob has no maps and cannot read Alice's.
    let bob = h.user("bob").await;
    assert_eq!(
        ops::list_maps(&bob.client).await.expect("bob"),
        types::MapList { maps: vec![] }
    );
    assert_problem(
        ops::get_map(&bob.client, "Roadmap").await,
        &plain("not_found", "Not found", 404, None),
    );
    h.finish().await;
}

#[tokio::test]
async fn recluster_queues_one_job_and_is_rate_limited() {
    let h = H::new(RateLimit {
        max: 2,
        window_secs: 3600,
    })
    .await;
    let alice = h.user("alice").await;
    let first = ops::recluster_graph(&alice.client).await.expect("queued");
    assert_eq!(first.run_after, h.now());
    let second = ops::recluster_graph(&alice.client).await.expect("queued");
    assert_eq!(second.job_id, first.job_id, "a queued run is reused");
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    let jobs: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT kind, status, dedupe_key FROM jobs WHERE kind = 'cluster'")
            .fetch_all(tx.conn())
            .await
            .expect("jobs");
    tx.commit().await.expect("commit");
    assert_eq!(
        jobs,
        vec![("cluster".into(), "queued".into(), Some("manual".into()))]
    );
    let (status, retry_after, body) = h
        .raw(
            &alice.token,
            reqwest::Method::POST,
            "/api/v1/graph/recluster",
            "recluster_graph",
        )
        .await;
    assert_eq!((status, retry_after.as_deref()), (429, Some("3600")));
    let p: types::Problem = rmp_serde::from_slice(&body).expect("problem");
    assert_eq!(
        p,
        plain(
            "rate_limited",
            "Too many requests",
            429,
            Some("too many re-clusterings; try again later"),
        )
    );
    // The limit is per user.
    let bob = h.user("bob").await;
    ops::recluster_graph(&bob.client).await.expect("bob queued");
    h.finish().await;
}

#[test]
fn cluster_updates_reach_the_users_event_stream_only() {
    let bus = Arc::new(EventBus::new(BusConfig::default()));
    let events = BusClusterEvents(bus.clone());
    let alice = strata_common::UserId::from_ulid(ulid::Ulid::from_parts(1, 1));
    let bob = strata_common::UserId::from_ulid(ulid::Ulid::from_parts(2, 2));
    events.clusters_updated(alice, &["1".to_owned(), "4".to_owned()]);
    assert_eq!(
        bus.subscribe(alice, Some(0)).replay,
        vec![Frame::Data {
            seq: 1,
            payload: Event::ClusterUpdated {
                cluster_ids: vec!["1".into(), "4".into()]
            }
        }]
    );
    assert_eq!(bus.subscribe(bob, Some(0)).replay, vec![]);
}
