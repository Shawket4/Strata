//! Neighbourhoods, filters, co-mentions and cluster matching.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)] // test helpers; exact values

use domain::{EntityRelationType, GraphEdgeKind as E, GraphNodeKind as K, RelationType};
use graph_algo::{
    ClusterMatching, CoMention, EdgeInput, Filter, Graph, GraphError, LeidenConfig, MatchConfig,
    MatchedCluster, NeighbourhoodError, PreviousCluster, WeightedGraph, co_mentions, leiden,
    match_clusters, neighbourhood,
};
use pretty_assertions::assert_eq;

/// n0 —link→ n1 —related→ n2 —link→ n3 —link→ n4 ; n0 mentions p (person) ; n1 → c (concept)
/// n2 ← n5 (link, incoming) ; p —works-at→ co (company)
fn sample() -> Graph {
    let nodes = [
        ("n0", K::Note),
        ("n1", K::Note),
        ("n2", K::Note),
        ("n3", K::Note),
        ("n4", K::Note),
        ("n5", K::Note),
        ("p", K::Person),
        ("c", K::Concept),
        ("co", K::Company),
    ];
    let edges = vec![
        EdgeInput::user("n0", "n1", E::Link),
        EdgeInput::ai("n1", "n2", E::Relation(RelationType::Related), 0.8),
        EdgeInput::user("n2", "n3", E::Link),
        EdgeInput::user("n3", "n4", E::Link),
        EdgeInput::user("n0", "p", E::Mention),
        EdgeInput::user("n1", "c", E::Concept),
        EdgeInput::user("n5", "n2", E::Link),
        EdgeInput::user("p", "co", E::Entity(EntityRelationType::WorksAt)),
    ];
    Graph::from_edge_list(nodes, edges).unwrap()
}

fn keys(g: &Graph, nodes: &[(u32, u8)]) -> Vec<(String, u8)> {
    nodes
        .iter()
        .map(|&(n, d)| (g.node(n).key.clone(), d))
        .collect()
}

fn s(k: &str, d: u8) -> (String, u8) {
    (k.to_owned(), d)
}

#[test]
fn depths_one_to_three() {
    let g = sample();
    let n1 = g.index_of("n1").unwrap();
    let h1 = neighbourhood(&g, n1, 1, &Filter::default()).unwrap();
    assert_eq!(
        keys(&g, &h1.nodes),
        [s("n1", 0), s("n0", 1), s("n2", 1), s("c", 1)]
    );
    assert_eq!(h1.edges, [0, 1, 5]);
    let h2 = neighbourhood(&g, n1, 2, &Filter::default()).unwrap();
    assert_eq!(
        keys(&g, &h2.nodes),
        [
            s("n1", 0),
            s("n0", 1),
            s("n2", 1),
            s("c", 1),
            s("n3", 2),
            s("n5", 2),
            s("p", 2)
        ]
    );
    assert_eq!(h2.edges, [0, 1, 2, 4, 5, 6]);
    let h3 = neighbourhood(&g, n1, 3, &Filter::default()).unwrap();
    assert_eq!(h3.nodes.len(), 9);
    assert_eq!(h3.edges, [0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn type_filters() {
    let g = sample();
    let n1 = g.index_of("n1").unwrap();
    let links_only = Filter {
        edge_kinds: vec![E::Link],
        node_kinds: vec![],
    };
    let h = neighbourhood(&g, n1, 3, &links_only).unwrap();
    assert_eq!(keys(&g, &h.nodes), [s("n1", 0), s("n0", 1)]);
    let notes_only = Filter {
        edge_kinds: vec![],
        node_kinds: vec![K::Note],
    };
    let h = neighbourhood(&g, n1, 2, &notes_only).unwrap();
    assert_eq!(
        keys(&g, &h.nodes),
        [s("n1", 0), s("n0", 1), s("n2", 1), s("n3", 2), s("n5", 2)]
    );
    // The focus is included even when its kind is filtered out.
    let p = g.index_of("p").unwrap();
    let h = neighbourhood(&g, p, 1, &notes_only).unwrap();
    assert_eq!(keys(&g, &h.nodes), [s("p", 0), s("n0", 1)]);
}

#[test]
fn neighbourhood_errors() {
    let g = sample();
    assert_eq!(
        neighbourhood(&g, 0, 0, &Filter::default()),
        Err(NeighbourhoodError::InvalidDepth(0))
    );
    assert_eq!(
        neighbourhood(&g, 0, 4, &Filter::default()),
        Err(NeighbourhoodError::InvalidDepth(4))
    );
    assert_eq!(
        neighbourhood(&g, 99, 1, &Filter::default()),
        Err(NeighbourhoodError::UnknownNode(99))
    );
}

#[test]
fn builder_errors_and_accessors() {
    assert_eq!(
        Graph::from_edge_list([("a", K::Note)], [EdgeInput::user("a", "b", E::Link)]).unwrap_err(),
        GraphError::UnknownNode("b".into())
    );
    assert_eq!(
        Graph::from_edge_list([("a", K::Note), ("a", K::Person)], []).unwrap_err(),
        GraphError::KindMismatch {
            key: "a".into(),
            first: K::Note,
            second: K::Person
        }
    );
    let g = sample();
    let n2 = g.index_of("n2").unwrap();
    assert_eq!(
        (g.degree(n2), g.out_edges(n2), g.in_edges(n2)),
        (3, &[2_u32][..], &[1_u32, 6][..])
    );
    assert_eq!((g.node_count(), g.edge_count()), (9, 8));
}

#[test]
fn co_mention_strength() {
    let nodes = [
        ("m1", K::Note),
        ("m2", K::Note),
        ("m3", K::Note),
        ("ahmed", K::Person),
        ("shady", K::Person),
        ("sara", K::Person),
        ("acme", K::Company),
    ];
    let mut edges = Vec::new();
    for (note, who) in [
        ("m1", "ahmed"),
        ("m1", "shady"),
        ("m2", "ahmed"),
        ("m2", "shady"),
        ("m2", "sara"),
        ("m2", "acme"),
        ("m3", "sara"),
        ("m3", "sara"),
    ] {
        edges.push(EdgeInput::user(note, who, E::Mention));
    }
    edges.push(EdgeInput::user("m3", "acme", E::Link)); // not a mention
    let g = Graph::from_edge_list(nodes, edges).unwrap();
    let ix = |k| g.index_of(k).unwrap();
    assert_eq!(
        co_mentions(&g, &[K::Person]),
        [
            CoMention {
                a: ix("ahmed"),
                b: ix("shady"),
                notes: 2,
                strength: 1.5
            },
            CoMention {
                a: ix("ahmed"),
                b: ix("sara"),
                notes: 1,
                strength: 0.5
            },
            CoMention {
                a: ix("shady"),
                b: ix("sara"),
                notes: 1,
                strength: 0.5
            },
        ]
    );
    let with_companies = co_mentions(&g, &[K::Person, K::Company]);
    assert_eq!(with_companies.len(), 6);
    assert_eq!(
        with_companies[0],
        CoMention {
            a: ix("ahmed"),
            b: ix("shady"),
            notes: 2,
            strength: 1.0 + 1.0 / 3.0
        }
    );
}

fn prev(id: &str, members: &[&str]) -> PreviousCluster {
    PreviousCluster {
        id: id.into(),
        members: members.iter().map(|&m| m.to_owned()).collect(),
    }
}

fn cl(members: &[&str]) -> Vec<String> {
    members.iter().map(|&m| m.to_owned()).collect()
}

fn fresh() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("new-{n}")
    }
}

#[test]
fn cluster_matching_keeps_ids_through_growth_split_and_merge() {
    let previous = vec![
        prev("pricing", &["a", "b", "c", "d"]),
        prev("people", &["e", "f", "g"]),
        prev("travel", &["h", "i"]),
        prev("misc", &["j", "k"]),
    ];
    let new = vec![
        cl(&["a", "b", "x"]),           // split of pricing: J = 2/5
        cl(&["c", "d"]), // other half of the split: J = 2/4, the better match keeps "pricing"
        cl(&["e", "f", "g", "h", "i"]), // merge of people (J = 3/5) and travel (J = 2/5)
        cl(&["y", "z"]), // brand new
        cl(&["j", "q", "r", "s", "t"]), // J(misc) = 1/6 < 0.25 → fresh
    ];
    let m = match_clusters(&previous, &new, &MatchConfig::default(), fresh());
    let got: Vec<(&str, Option<&str>)> = m
        .clusters
        .iter()
        .map(|c| (c.id.as_str(), c.previous.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            ("new-1", None),
            ("pricing", Some("pricing")),
            ("people", Some("people")),
            ("new-2", None),
            ("new-3", None),
        ]
    );
    assert_eq!(m.retired, ["misc", "travel"]);
    assert_eq!(m.clusters[1].jaccard, 0.5);
    assert_eq!(m.clusters[2].members, cl(&["e", "f", "g", "h", "i"]));
}

#[test]
fn identical_clusters_keep_every_id() {
    let previous = vec![prev("x", &["1", "2"]), prev("y", &["3"])];
    let new = vec![cl(&["3"]), cl(&["2", "1"])];
    let m = match_clusters(&previous, &new, &MatchConfig::default(), || {
        panic!("no fresh IDs needed")
    });
    assert_eq!(
        m,
        ClusterMatching {
            clusters: vec![
                MatchedCluster {
                    id: "y".into(),
                    members: cl(&["3"]),
                    previous: Some("y".into()),
                    jaccard: 1.0
                },
                MatchedCluster {
                    id: "x".into(),
                    members: cl(&["1", "2"]),
                    previous: Some("x".into()),
                    jaccard: 1.0
                },
            ],
            retired: vec![],
        }
    );
}

#[test]
fn leiden_clusters_keep_their_ids_after_a_small_edit() {
    let mut edges = Vec::new();
    for c in 0..6_u32 {
        for i in 0..6 {
            for j in i + 1..6 {
                edges.push((c * 6 + i, c * 6 + j, 1.0));
            }
        }
        edges.push((c * 6 + 5, ((c + 1) % 6) * 6, 1.0));
    }
    let key = |i: u32| format!("n{i}");
    let run = |edges: &[(u32, u32, f64)], n: usize| {
        let p = leiden(
            &WeightedGraph::from_edges(n, edges.iter().copied()),
            &LeidenConfig::default(),
        );
        p.communities()
            .into_iter()
            .map(|c| c.into_iter().map(key).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let first = run(&edges, 36);
    let named = match_clusters(&[], &first, &MatchConfig::default(), fresh());
    let previous: Vec<PreviousCluster> = named
        .clusters
        .iter()
        .map(|c| PreviousCluster {
            id: c.id.clone(),
            members: c.members.clone(),
        })
        .collect();
    // A new note linked into the third clique.
    let mut edited = edges.clone();
    edited.push((36, 12, 1.0));
    edited.push((36, 13, 1.0));
    let second = run(&edited, 37);
    let rematched = match_clusters(&previous, &second, &MatchConfig::default(), || {
        "unexpected".into()
    });
    let ids: Vec<&str> = rematched.clusters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["new-1", "new-2", "new-3", "new-4", "new-5", "new-6"]);
    assert!(rematched.clusters[2].members.contains(&"n36".to_owned()));
    assert_eq!(rematched.retired, Vec::<String>::new());
}
