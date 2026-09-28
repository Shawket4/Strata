//! The global map (PLAN §11 screen 9): the People and Companies lenses with co-mention edges,
//! the focus neighbourhood, clusters (named regions, empty ones left out, one cluster kept by
//! the filter), and the similarity layer's availability offline. Against the fake server,
//! whose cluster records the map draws.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::graph;
use strata_core::session::Session;
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::Record;
use strata_core::view::model::{
    Availability, GlobalGraphView, GraphFilter, GraphLens, GraphNodeKind,
};
use sync_model::changes::{ClusterAssignmentRecord, ClusterNameRecord};
use ulid::Ulid;

const MONA: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VP1";
const AHMED: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VP2";
const ACME: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VC1";
const KICKOFF: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VN1";
const PRICING: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0VN2";

fn filter(lens: GraphLens) -> GraphFilter {
    GraphFilter {
        edge_kinds: Vec::new(),
        node_kinds: Vec::new(),
        similarity: false,
        cluster: None,
        lens,
        focus: None,
        include_tags: false,
    }
}

fn map(s: &Session, f: &GraphFilter) -> GlobalGraphView {
    s.read(|c, ctx| graph::global_graph_filtered(c, ctx, f))
        .expect("map")
}

fn nodes(v: &GlobalGraphView) -> Vec<(&str, GraphNodeKind, Option<&str>)> {
    let mut n: Vec<_> = v
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.kind, n.cluster_id.as_deref()))
        .collect();
    n.sort_unstable_by_key(|x| x.0);
    n
}

async fn world() -> (Harness, std::sync::Arc<Session>) {
    let h = Harness::new();
    for (id, path, kind) in [
        (MONA, "people/Mona Adel.md", "person"),
        (AHMED, "people/Ahmed Samir.md", "person"),
        (ACME, "companies/Acme.md", "company"),
    ] {
        h.server
            .remote_upsert(id, path, &format!("---\nid: {id}\nkind: {kind}\n---\n"));
    }
    h.server.remote_upsert(
        KICKOFF,
        "notes/Kickoff.md",
        &format!(
            "---\nid: {KICKOFF}\npeople: [\"[[Mona Adel]]\", \"[[Ahmed Samir]]\"]\ncompanies: [\"[[Acme]]\"]\n---\nFirst meeting.\n"
        ),
    );
    h.server.remote_upsert(
        PRICING,
        "notes/Pricing.md",
        &format!(
            "---\nid: {PRICING}\npeople: [\"[[Mona Adel]]\", \"[[Ahmed Samir]]\"]\n---\nPrices for Q4.\n"
        ),
    );
    for (note, cluster) in [(KICKOFF, "c1"), (PRICING, "c1"), (MONA, "c2")] {
        h.server
            .remote_record(Record::ClusterAssignment(ClusterAssignmentRecord {
                note_id: Ulid::from_string(note).expect("id"),
                cluster_id: cluster.into(),
            }));
    }
    for (id, name) in [("c1", "Meetings"), ("c2", "Contacts"), ("c3", "Empty")] {
        h.server
            .remote_record(Record::ClusterName(ClusterNameRecord {
                cluster_id: id.into(),
                name: name.into(),
            }));
    }
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    (h, s)
}

#[tokio::test]
async fn lenses_show_co_mentions_and_the_focus_neighbourhood() {
    let (_h, s) = world().await;

    let people = map(&s, &filter(GraphLens::People));
    assert_eq!(
        nodes(&people),
        [
            (MONA, GraphNodeKind::Person, Some("c2")),
            (AHMED, GraphNodeKind::Person, None)
        ]
    );
    // Mona and Ahmed are named together in two notes: one co-mention edge between them.
    assert_eq!(
        people
            .edges
            .iter()
            .map(|e| (
                e.id.as_str(),
                e.kind.as_str(),
                e.label.as_str(),
                e.by.as_deref(),
                e.rel_type.as_deref()
            ))
            .collect::<Vec<_>>(),
        [(
            format!("{MONA}|co-mention|{AHMED}").as_str(),
            "co-mention",
            "co-mentioned in 2 notes",
            None,
            None
        )]
    );
    assert_eq!(people.neighbours, Vec::<String>::new());
    // Strength: each of the two notes names exactly two people, 1 / (2 − 1) each.
    assert_eq!(people.edges[0].confidence, Some(2.0));

    // Focusing a person lists who is connected to them.
    let focused = map(
        &s,
        &GraphFilter {
            focus: Some(AHMED.to_owned()),
            ..filter(GraphLens::People)
        },
    );
    assert_eq!(focused.neighbours, [MONA.to_owned()]);

    // Only one company: nobody to be co-mentioned with.
    let companies = map(&s, &filter(GraphLens::Companies));
    assert_eq!(
        (nodes(&companies), companies.edges.len()),
        (vec![(ACME, GraphNodeKind::Company, None)], 0)
    );
}

#[tokio::test]
async fn clusters_are_named_regions_and_one_can_be_kept_alone() {
    let (h, s) = world().await;
    let all = map(&s, &filter(GraphLens::Notes));
    // Named clusters with members, by name; "Empty" has none and is left out.
    assert_eq!(
        all.clusters
            .iter()
            .map(|c| (c.id.as_str(), c.name.as_str(), c.size))
            .collect::<Vec<_>>(),
        [("c2", "Contacts", 1), ("c1", "Meetings", 2)]
    );
    for c in &all.clusters {
        assert!(
            c.x.is_finite() && c.y.is_finite() && c.radius >= 0.0,
            "{c:?}"
        );
        let members: Vec<_> = all
            .nodes
            .iter()
            .filter(|n| n.cluster_id.as_deref() == Some(c.id.as_str()))
            .collect();
        assert_eq!(u32::try_from(members.len()).unwrap(), c.size);
    }

    let meetings = map(
        &s,
        &GraphFilter {
            cluster: Some("c1".to_owned()),
            ..filter(GraphLens::Notes)
        },
    );
    assert_eq!(
        nodes(&meetings),
        [
            (KICKOFF, GraphNodeKind::Note, Some("c1")),
            (PRICING, GraphNodeKind::Note, Some("c1"))
        ]
    );
    assert_eq!(meetings.edges, [], "no edge stays inside the cluster");
    assert_eq!(
        meetings
            .clusters
            .iter()
            .map(|c| (c.id.as_str(), c.size))
            .collect::<Vec<_>>(),
        [("c1", 2)]
    );
    // Online the similarity layer is fetched on demand; offline it is unavailable.
    assert_eq!(all.similarity, Availability::Available);
    h.server.set_offline(true);
    s.sync(Trigger::Manual).await.expect("cycle");
    assert_eq!(
        map(&s, &filter(GraphLens::Notes)).similarity,
        Availability::Offline
    );
}
