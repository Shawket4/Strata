//! Graph assembly from the index (PLAN §7.5 Graph, §9.6, §10) on the fixture vault: exact
//! node and edge payloads, type and kind filters, the entity lens with co-mention weights,
//! local neighbourhoods by depth and filter, similarity edges, and isolation between users.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

mod common;

use std::sync::Arc;

use common::{Fixture, World, edge, edge_triples, fixture, node_degrees};
use domain::{GraphNodeKind, RelationOrigin};
use pretty_assertions::assert_eq;
use strata_common::NoteId;
use strata_graph::GraphError;
use strata_graph::assemble::{NodeId, NodeView, SimilarityStatus};
use strata_graph::query::{EdgeFilter, GraphQuery, Lens, LocalQuery, NodeFilter};
use strata_graph::similarity::{NoteVectorSimilarity, SimilarityConfig, SimilaritySource};
use strata_index::UserScope;

fn t(a: &str, b: &str, k: &str) -> (String, String, String) {
    (a.to_owned(), b.to_owned(), k.to_owned())
}

fn d(title: &str, degree: u32) -> (String, u32) {
    (title.to_owned(), degree)
}

async fn world() -> (World, UserScope, Fixture) {
    let w = World::new().await;
    let (_, s) = w.user("alice").await;
    let f = fixture(&w, &s).await;
    (w, s, f)
}

#[tokio::test]
async fn the_global_graph_has_every_live_note_and_every_typed_edge() {
    let (w, s, f) = world().await;
    let g = w
        .service(None)
        .graph(&s, &GraphQuery::default())
        .await
        .expect("graph");
    assert_eq!(
        node_degrees(&g),
        vec![
            d("Shady", 4),
            d("Omar", 1),
            d("Watanya", 3),
            d("Acme", 0),
            d("Mona", 4),
            d("Nasr City office", 1),
            d("Safe", 2),
            d("Watanya contract", 3),
            d("Pricing", 1),
            d("Budget", 1),
            d("Plan", 4),
            d("Call", 7),
            d("Meeting", 3),
            d("Lonely", 0),
        ]
    );
    assert_eq!(
        edge_triples(&g),
        vec![
            t("Mona", "Shady", "entity:knows"),
            t("Mona", "Watanya", "entity:works-at"),
            t("Safe", "Nasr City office", "part-of-place"),
            t("Watanya contract", "Shady", "custody:last-holder"),
            t("Watanya contract", "Watanya", "mention"),
            t("Watanya contract", "Safe", "custody:location"),
            t("Plan", "Budget", "relation:contradicts"),
            t("Call", "Shady", "mention"),
            t("Call", "Watanya", "mention"),
            t("Call", "Mona", "mention"),
            t("Call", "Pricing", "concept"),
            t("Call", "Plan", "link"),
            t("Call", "Plan", "embed"),
            t("Call", "Plan", "relation:related"),
            t("Meeting", "Shady", "mention"),
            t("Meeting", "Omar", "mention"),
            t("Meeting", "Mona", "mention"),
        ]
    );
    // Provenance of an AI relation, of a user edge, and the full payload of one node.
    let ai = edge(&g, f.plan, f.budget, "relation:contradicts");
    assert_eq!(
        (ai.by, ai.confidence, ai.reason.as_deref(), ai.weight),
        (
            Some(RelationOrigin::Ai),
            Some(0.72),
            Some("10% flat vs a 5% cap"),
            None
        )
    );
    let user = edge(&g, f.call, f.plan, "relation:related");
    assert_eq!(
        (user.by, user.confidence, user.reason.as_deref()),
        (Some(RelationOrigin::User), None, None)
    );
    let contract = g
        .nodes
        .iter()
        .find(|n| n.id == f.contract)
        .expect("contract");
    assert_eq!(
        contract,
        &NodeView {
            id: f.contract.into(),
            title: "Watanya contract".into(),
            kind: GraphNodeKind::Document,
            path: Some("documents/Watanya contract.md".into()),
            cluster_id: None,
            degree: 3,
            lang: None,
            updated: contract.updated,
            summary: None,
            depth: None,
        }
    );
    assert_eq!(contract.updated, Some(strata_testkit::default_test_epoch()));
    assert_eq!(g.clusters, vec![]);
    assert_eq!(g.similarity, SimilarityStatus::Off);
    w.finish().await;
}

#[tokio::test]
async fn type_and_kind_filters_select_edges_and_nodes() {
    let (w, s, _) = world().await;
    let svc = w.service(None);
    let only = |types: &str, kinds: &str| GraphQuery {
        edges: EdgeFilter::parse(Some(types)).expect("types"),
        nodes: NodeFilter::parse(Some(kinds)).expect("kinds"),
        ..GraphQuery::default()
    };
    let g = svc
        .graph(&s, &only("custody,part-of-place", "document,place,person"))
        .await
        .expect("graph");
    assert_eq!(
        node_degrees(&g),
        vec![
            d("Shady", 1),
            d("Omar", 0),
            d("Mona", 0),
            d("Nasr City office", 1),
            d("Safe", 2),
            d("Watanya contract", 2),
        ]
    );
    assert_eq!(
        edge_triples(&g),
        vec![
            t("Safe", "Nasr City office", "part-of-place"),
            t("Watanya contract", "Shady", "custody:last-holder"),
            t("Watanya contract", "Safe", "custody:location"),
        ]
    );
    let g = svc
        .graph(&s, &only("relation,link", "note"))
        .await
        .expect("graph");
    assert_eq!(
        edge_triples(&g),
        vec![
            t("Plan", "Budget", "relation:contradicts"),
            t("Call", "Plan", "link"),
            t("Call", "Plan", "relation:related"),
        ]
    );
    assert_eq!(
        node_degrees(&g),
        vec![
            d("Budget", 1),
            d("Plan", 3),
            d("Call", 2),
            d("Meeting", 0),
            d("Lonely", 0),
        ]
    );
    w.finish().await;
}

#[tokio::test]
async fn the_people_lens_weights_co_mentions_by_how_many_people_a_note_names() {
    let (w, s, f) = world().await;
    let svc = w.service(None);
    let g = svc
        .graph(
            &s,
            &GraphQuery {
                lens: Some(Lens::People),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("lens");
    assert_eq!(
        node_degrees(&g),
        vec![d("Shady", 3), d("Omar", 2), d("Mona", 3)]
    );
    assert_eq!(
        edge_triples(&g),
        vec![
            t("Shady", "Omar", "co-mention"),
            t("Shady", "Mona", "co-mention"),
            t("Omar", "Mona", "co-mention"),
            t("Mona", "Shady", "entity:knows"),
        ]
    );
    // Call names 2 people (1/(2−1) = 1 each pair), Meeting names 3 (1/2 each pair).
    let weights: Vec<(Option<f64>, Option<u32>, Option<RelationOrigin>)> =
        g.edges.iter().map(|e| (e.weight, e.notes, e.by)).collect();
    assert_eq!(
        weights,
        vec![
            (Some(0.5), Some(1), None),
            (Some(1.5), Some(2), None),
            (Some(0.5), Some(1), None),
            (None, None, Some(RelationOrigin::User)),
        ]
    );
    assert_eq!(edge(&g, f.shady, f.mona, "co-mention").weight, Some(1.5));

    // Companies: Watanya is named with no other company, so there are no ties.
    let g = svc
        .graph(
            &s,
            &GraphQuery {
                lens: Some(Lens::Companies),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("lens");
    assert_eq!(node_degrees(&g), vec![d("Watanya", 0), d("Acme", 0)]);
    assert_eq!(edge_triples(&g), vec![]);

    // A type filter applies inside the lens.
    let g = svc
        .graph(
            &s,
            &GraphQuery {
                lens: Some(Lens::People),
                edges: EdgeFilter::parse(Some("entity")).expect("types"),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("lens");
    assert_eq!(edge_triples(&g), vec![t("Mona", "Shady", "entity:knows")]);
    w.finish().await;
}

fn local(depth: u8, types: Option<&str>, kinds: Option<&str>) -> LocalQuery {
    LocalQuery {
        depth,
        edges: EdgeFilter::parse(types).expect("types"),
        nodes: NodeFilter::parse(kinds).expect("kinds"),
        include_similarity: false,
        include_tags: false,
    }
}

fn depths(g: &strata_graph::assemble::GraphView) -> Vec<(String, Option<u8>)> {
    g.nodes.iter().map(|n| (n.title.clone(), n.depth)).collect()
}

#[tokio::test]
async fn local_neighbourhoods_grow_with_depth_and_respect_filters() {
    let (w, s, f) = world().await;
    let svc = w.service(None);
    let d1 = svc
        .local(&s, f.plan, &local(1, None, None))
        .await
        .expect("d1");
    assert_eq!(
        depths(&d1),
        vec![
            ("Budget".into(), Some(1)),
            ("Plan".into(), Some(0)),
            ("Call".into(), Some(1)),
        ]
    );
    assert_eq!(
        edge_triples(&d1),
        vec![
            t("Plan", "Budget", "relation:contradicts"),
            t("Call", "Plan", "link"),
            t("Call", "Plan", "embed"),
            t("Call", "Plan", "relation:related"),
        ]
    );
    let d2 = svc
        .local(&s, f.plan, &local(2, None, None))
        .await
        .expect("d2");
    assert_eq!(
        depths(&d2),
        vec![
            ("Shady".into(), Some(2)),
            ("Watanya".into(), Some(2)),
            ("Mona".into(), Some(2)),
            ("Pricing".into(), Some(2)),
            ("Budget".into(), Some(1)),
            ("Plan".into(), Some(0)),
            ("Call".into(), Some(1)),
        ]
    );
    // Edges among included nodes count too (Mona knows Shady, works at Watanya).
    assert_eq!(
        node_degrees(&d2),
        vec![
            d("Shady", 2),
            d("Watanya", 2),
            d("Mona", 3),
            d("Pricing", 1),
            d("Budget", 1),
            d("Plan", 4),
            d("Call", 7),
        ]
    );
    let d3 = svc
        .local(&s, f.plan, &local(3, None, None))
        .await
        .expect("d3");
    assert_eq!(
        depths(&d3)
            .into_iter()
            .filter(|(_, d)| *d == Some(3))
            .map(|(t, _)| t)
            .collect::<Vec<_>>(),
        vec!["Watanya contract", "Meeting"]
    );
    // Only relations: Call is reached, its mentions are not.
    let rel = svc
        .local(&s, f.plan, &local(3, Some("relation"), None))
        .await
        .expect("relations");
    assert_eq!(
        depths(&rel),
        vec![
            ("Budget".into(), Some(1)),
            ("Plan".into(), Some(0)),
            ("Call".into(), Some(1)),
        ]
    );
    // Node kinds: people only around Call (the focus is always kept).
    let people = svc
        .local(&s, f.call, &local(1, None, Some("person")))
        .await
        .expect("people");
    assert_eq!(
        depths(&people),
        vec![
            ("Shady".into(), Some(1)),
            ("Mona".into(), Some(1)),
            ("Call".into(), Some(0)),
        ]
    );
    assert_eq!(
        edge_triples(&people),
        vec![
            t("Mona", "Shady", "entity:knows"),
            t("Call", "Shady", "mention"),
            t("Call", "Mona", "mention"),
        ]
    );
    // Only co-mention (a lens kind) leaves the focus alone.
    let none = svc
        .local(&s, f.call, &local(2, Some("co-mention"), None))
        .await
        .expect("focus only");
    assert_eq!(depths(&none), vec![("Call".into(), Some(0))]);
    assert_eq!(none.nodes[0].degree, 0);
    // Unknown or trashed focus: not found.
    let missing = NoteId::generate(w.db.ids.as_ref());
    assert!(matches!(
        svc.local(&s, missing, &local(1, None, None)).await,
        Err(GraphError::NotFound)
    ));
    w.vault.delete_note(&s, f.lonely).await.expect("trash");
    assert!(matches!(
        svc.local(&s, f.lonely, &local(1, None, None)).await,
        Err(GraphError::NotFound)
    ));
    w.finish().await;
}

#[tokio::test]
async fn trashed_notes_leave_the_graph_with_their_edges() {
    let (w, s, f) = world().await;
    w.vault.delete_note(&s, f.plan).await.expect("trash");
    let g = w
        .service(None)
        .graph(
            &s,
            &GraphQuery {
                edges: EdgeFilter::parse(Some("relation,link,embed")).expect("types"),
                nodes: NodeFilter::parse(Some("note")).expect("kinds"),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("graph");
    assert_eq!(
        node_degrees(&g),
        vec![
            d("Budget", 0),
            d("Call", 0),
            d("Meeting", 0),
            d("Lonely", 0)
        ]
    );
    assert_eq!(edge_triples(&g), vec![]);
    w.finish().await;
}

#[tokio::test]
async fn summaries_come_from_the_sidecar_and_are_short() {
    let (w, s, f) = world().await;
    let version = w.vault.note(&s, f.plan).await.expect("note").version;
    let long = format!("{} end", "Discount policy text. ".repeat(20));
    w.vault
        .ai_set_summary(&s, f.plan, version, long, "summarize".into())
        .await
        .expect("summary");
    let g = w
        .service(None)
        .graph(&s, &GraphQuery::default())
        .await
        .expect("graph");
    let plan = g.nodes.iter().find(|n| n.id == f.plan).expect("plan");
    let expected: String = "Discount policy text. "
        .repeat(20)
        .chars()
        .take(199)
        .collect::<String>()
        .trim_end()
        .to_owned()
        + "…";
    assert_eq!(plan.summary.as_deref(), Some(expected.as_str()));
    assert_eq!(
        g.nodes.iter().filter(|n| n.summary.is_some()).count(),
        1,
        "only Plan has a summary"
    );
    let l = w
        .service(None)
        .local(&s, f.budget, &local(1, None, None))
        .await
        .expect("local");
    assert_eq!(
        l.nodes
            .iter()
            .map(|n| n.summary.is_some())
            .collect::<Vec<_>>(),
        vec![false, true]
    );
    w.finish().await;
}

async fn put_vector(w: &World, s: &UserScope, note: NoteId, v: &[f32]) {
    let mut tx = w.db.app_db.begin(s).await.expect("tx");
    let mut full = vec![0.0_f32; 384];
    full[..v.len()].copy_from_slice(v);
    sqlx::query(
        "INSERT INTO note_vectors (user_id, note_id, model, content_hash, embedding, chunk_count, updated) \
         SELECT strata_current_user(), $1, 'm@1', n.content_hash, $2, 1, now() FROM notes n WHERE n.id = $1",
    )
    .bind(note)
    .bind(pgvector::Vector::from(full))
    .execute(tx.conn())
    .await
    .expect("vector");
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn similarity_edges_are_computed_on_request_from_note_vectors() {
    let (w, s, f) = world().await;
    put_vector(&w, &s, f.plan, &[1.0, 0.0]).await;
    put_vector(&w, &s, f.budget, &[0.8, 0.6]).await;
    put_vector(&w, &s, f.lonely, &[0.6, 0.8]).await;
    put_vector(&w, &s, f.meeting, &[0.0, -1.0]).await;
    let src: Arc<dyn SimilaritySource> = Arc::new(NoteVectorSimilarity::new(
        w.db.app_db.clone(),
        "m@1",
        SimilarityConfig {
            top_n: 1,
            floor: 0.75,
            max_notes: 10,
        },
    ));
    let svc = w.service(Some(src.clone()));
    let with = GraphQuery {
        edges: EdgeFilter::parse(Some("similarity")).expect("types"),
        include_similarity: true,
        ..GraphQuery::default()
    };
    let g = svc.graph(&s, &with).await.expect("graph");
    assert_eq!(g.similarity, SimilarityStatus::Complete);
    // Plan's best is Budget (0.8), Budget's best is Lonely (0.96), Lonely's is Budget.
    let sims: Vec<(NoteId, NoteId, Option<f64>, Option<RelationOrigin>)> = g
        .edges
        .iter()
        .map(|e| {
            (
                e.source.note().expect("note"),
                e.target.note().expect("note"),
                e.weight,
                e.by,
            )
        })
        .collect();
    assert_eq!(
        sims,
        vec![
            (f.budget, f.plan, Some(0.8), Some(RelationOrigin::Ai)),
            (f.budget, f.lonely, Some(0.96), Some(RelationOrigin::Ai)),
        ]
    );
    // Not requested: none, and nothing is stored.
    let g = svc
        .graph(
            &s,
            &GraphQuery {
                edges: EdgeFilter::parse(Some("similarity")).expect("types"),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("graph");
    assert_eq!((g.edges.len(), g.similarity), (0, SimilarityStatus::Off));
    // Truncated to the most recently updated notes.
    let small: Arc<dyn SimilaritySource> = Arc::new(NoteVectorSimilarity::new(
        w.db.app_db.clone(),
        "m@1",
        SimilarityConfig {
            top_n: 1,
            floor: 0.75,
            max_notes: 2,
        },
    ));
    let g = w
        .service(Some(small))
        .graph(&s, &with)
        .await
        .expect("graph");
    assert_eq!(g.similarity, SimilarityStatus::Truncated);
    // Without a model the graph still answers (principle 6).
    let g = w.service(None).graph(&s, &with).await.expect("graph");
    assert_eq!(
        (g.edges.len(), g.similarity),
        (0, SimilarityStatus::Unavailable)
    );
    // Local: the focus's neighbours join at depth 1.
    let mut q = local(1, Some("similarity"), None);
    q.include_similarity = true;
    let l = svc.local(&s, f.plan, &q).await.expect("local");
    assert_eq!(
        depths(&l),
        vec![("Budget".into(), Some(1)), ("Plan".into(), Some(0))]
    );
    assert_eq!(l.edges[0].weight, Some(0.8));
    w.finish().await;
}

#[tokio::test]
async fn a_graph_never_contains_another_users_notes() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let (_, sb) = w.user("bob").await;
    let fa = fixture(&w, &sa).await;
    let bob_note = w
        .create(
            &sb,
            "notes/Plan.md",
            "---\nrelated: [\"[[Budget]]\"]\n---\nBob's plan.\n",
        )
        .await;
    let bob_budget = w.create(&sb, "notes/Budget.md", "Bob's budget.\n").await;
    let svc = w.service(None);
    let gb = svc.graph(&sb, &GraphQuery::default()).await.expect("bob");
    assert_eq!(
        gb.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
        vec![NodeId::Note(bob_note), NodeId::Note(bob_budget)]
    );
    assert_eq!(
        edge_triples(&gb),
        vec![t("Plan", "Budget", "relation:related")]
    );
    let ga = svc.graph(&sa, &GraphQuery::default()).await.expect("alice");
    assert_eq!(ga.nodes.len(), 14);
    assert!(
        ga.nodes
            .iter()
            .all(|n| n.id != bob_note && n.id != bob_budget)
    );
    // Alice's note is not Bob's focus.
    assert!(matches!(
        svc.local(&sb, fa.plan, &local(1, None, None)).await,
        Err(GraphError::NotFound)
    ));
    let lens = svc
        .graph(
            &sb,
            &GraphQuery {
                lens: Some(Lens::People),
                ..GraphQuery::default()
            },
        )
        .await
        .expect("lens");
    assert_eq!((lens.nodes.len(), lens.edges.len()), (0, 0));
    w.finish().await;
}

#[tokio::test]
async fn copies_of_documents_are_document_copy_of_edges() {
    let w = World::new().await;
    let (_, s) = w.user("alice").await;
    let original = w
        .create(
            &s,
            "documents/Contract.md",
            "---\nkind: document\ncopy: original\n---\n",
        )
        .await;
    let copy = w
        .create(
            &s,
            "documents/Contract copy.md",
            "---\nkind: document\ncopy: copy\ncopy-of: [\"[[Contract]]\"]\n---\n",
        )
        .await;
    let other = w.create(&s, "notes/Other.md", "Unrelated.\n").await;
    let svc = w.service(None);
    let g = svc.graph(&s, &GraphQuery::default()).await.expect("graph");
    assert_eq!(
        edge_triples(&g),
        vec![t("Contract copy", "Contract", "document:copy-of")]
    );
    let e = edge(&g, copy, original, "document:copy-of");
    assert_eq!(
        (e.by, e.confidence, e.reason.as_deref(), e.weight, e.notes),
        (Some(RelationOrigin::User), None, None, None, None)
    );
    assert_eq!(
        node_degrees(&g),
        vec![d("Contract", 1), d("Contract copy", 1), d("Other", 0)]
    );
    // The family and the exact kind select it; other kinds leave it out.
    for types in ["document", "document:copy-of"] {
        let only = GraphQuery {
            edges: EdgeFilter::parse(Some(types)).expect("types"),
            ..GraphQuery::default()
        };
        let g = svc.graph(&s, &only).await.expect("filtered");
        assert_eq!(
            edge_triples(&g),
            vec![t("Contract copy", "Contract", "document:copy-of")],
            "{types}"
        );
    }
    let links = GraphQuery {
        edges: EdgeFilter::parse(Some("link,relation")).expect("types"),
        ..GraphQuery::default()
    };
    assert_eq!(
        edge_triples(&svc.graph(&s, &links).await.expect("links")),
        vec![]
    );
    // A local graph follows the edge from the original to its copy.
    let l = svc
        .local(&s, original, &local(1, None, None))
        .await
        .expect("local");
    assert_eq!(
        depths(&l),
        vec![
            ("Contract".into(), Some(0)),
            ("Contract copy".into(), Some(1))
        ]
    );
    assert_eq!(
        edge_triples(&l),
        vec![t("Contract copy", "Contract", "document:copy-of")]
    );
    let _ = other;
    w.finish().await;
}

fn with_tags(types: Option<&str>, kinds: Option<&str>) -> GraphQuery {
    GraphQuery {
        edges: EdgeFilter::parse(types).expect("types"),
        nodes: NodeFilter::parse(kinds).expect("kinds"),
        include_tags: true,
        ..GraphQuery::default()
    }
}

#[tokio::test]
async fn the_tag_toggle_adds_tag_nodes_and_note_to_tag_edges() {
    let w = World::new().await;
    let (_, s) = w.user("alice").await;
    let a = w
        .create(
            &s,
            "notes/A.md",
            "---\ntags: [Pricing, client]\n---\nSee [[B]].\n",
        )
        .await;
    let b = w
        .create(
            &s,
            "notes/B.md",
            "Body #pricing and #project/strata and #PRICING.\n",
        )
        .await;
    w.create(&s, "notes/C.md", "Nothing.\n").await;
    let svc = w.service(None);

    // Off by default.
    let g = svc.graph(&s, &GraphQuery::default()).await.expect("graph");
    assert_eq!(node_degrees(&g), vec![d("A", 1), d("B", 1), d("C", 0)]);

    // On: one node per tag (case folded, titled by the smallest spelling), after the notes.
    let g = svc.graph(&s, &with_tags(None, None)).await.expect("tags");
    assert_eq!(
        node_degrees(&g),
        vec![
            d("A", 3),
            d("B", 3),
            d("C", 0),
            d("#client", 1),
            d("#PRICING", 2),
            d("#project/strata", 1),
        ]
    );
    assert_eq!(
        edge_triples(&g),
        vec![
            t("A", "B", "link"),
            t("A", "#client", "tag"),
            t("A", "#PRICING", "tag"),
            t("B", "#PRICING", "tag"),
            t("B", "#project/strata", "tag"),
        ]
    );
    let pricing = g
        .nodes
        .iter()
        .find(|n| n.id == NodeId::Tag("pricing".into()))
        .expect("pricing tag");
    assert_eq!(
        pricing,
        &NodeView {
            id: NodeId::Tag("pricing".into()),
            title: "PRICING".into(),
            kind: GraphNodeKind::Tag,
            path: None,
            cluster_id: None,
            degree: 2,
            lang: None,
            updated: None,
            summary: None,
            depth: None,
        }
    );
    assert_eq!(pricing.id.to_string(), "tag:pricing");
    let e = g
        .edges
        .iter()
        .find(|e| e.source == b && e.target == NodeId::Tag("project/strata".into()))
        .expect("tag edge");
    assert_eq!(
        (e.by, e.confidence, e.reason.as_deref(), e.weight),
        (Some(RelationOrigin::User), None, None, None)
    );

    // Filters: `tag` names both the node kind and the edge kind.
    let g = svc
        .graph(&s, &with_tags(Some("tag"), Some("tag,note")))
        .await
        .expect("filtered");
    assert_eq!(
        edge_triples(&g),
        vec![
            t("A", "#client", "tag"),
            t("A", "#PRICING", "tag"),
            t("B", "#PRICING", "tag"),
            t("B", "#project/strata", "tag"),
        ]
    );
    let g = svc
        .graph(&s, &with_tags(None, Some("note")))
        .await
        .expect("notes only");
    assert_eq!(node_degrees(&g), vec![d("A", 1), d("B", 1), d("C", 0)]);
    assert_eq!(edge_triples(&g), vec![t("A", "B", "link")]);

    // Local: notes sharing a tag are two hops apart.
    let l = svc
        .local(
            &s,
            a,
            &LocalQuery {
                include_tags: true,
                ..local(2, Some("tag"), None)
            },
        )
        .await
        .expect("local");
    assert_eq!(
        depths(&l),
        vec![
            ("A".into(), Some(0)),
            ("B".into(), Some(2)),
            ("client".into(), Some(1)),
            ("PRICING".into(), Some(1)),
        ]
    );
    assert_eq!(
        edge_triples(&l),
        vec![
            t("A", "#client", "tag"),
            t("A", "#PRICING", "tag"),
            t("B", "#PRICING", "tag"),
        ]
    );
    // Without the toggle the same query finds nothing but the focus.
    let l = svc
        .local(&s, a, &local(2, Some("tag"), None))
        .await
        .expect("local");
    assert_eq!(depths(&l), vec![("A".into(), Some(0))]);

    // A lens never has tag nodes; a trashed note's tags go with it.
    let lens = svc
        .graph(
            &s,
            &GraphQuery {
                lens: Some(Lens::People),
                include_tags: true,
                ..GraphQuery::default()
            },
        )
        .await
        .expect("lens");
    assert_eq!((lens.nodes.len(), lens.edges.len()), (0, 0));
    w.vault.delete_note(&s, b).await.expect("trash");
    let g = svc.graph(&s, &with_tags(None, None)).await.expect("tags");
    assert_eq!(
        node_degrees(&g),
        vec![d("A", 2), d("C", 0), d("#client", 1), d("#Pricing", 1)]
    );
    w.finish().await;
}
