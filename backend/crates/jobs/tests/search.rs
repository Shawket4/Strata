//! Semantic and hybrid retrieval on an Arabic/English fixture vault (PLAN §7.5 Search, §9.6):
//! exact rankings and scores with the deterministic fake embedder, reciprocal-rank fusion
//! with Arabic normalisation on the keyword side, similarity edges, and isolation (a user
//! never retrieves another user's chunks).
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

mod common;

use common::World;
use pretty_assertions::assert_eq;
use strata_ai::embed::fake::bag_of_words;
use strata_common::NoteId;
use strata_index::UserScope;
use strata_jobs::retrieval::{Mode, RRF_K, Retriever};

const NOTES: [(&str, &str); 4] = [
    (
        "notes/Watanya contract.md",
        "The Watanya contract is stored in the safe at the Nasr City office.",
    ),
    ("notes/عقد وطنية.md", "عقد وطنية في الخزنة في مكتب مدينة نصر"),
    (
        "notes/Banana bread.md",
        "Banana bread recipe with walnuts and cinnamon.",
    ),
    (
        "notes/ETA invoice.md",
        "Monthly ETA invoice for Watanya is due on the first.",
    ),
];

fn title(path: &str) -> &str {
    path.trim_start_matches("notes/").trim_end_matches(".md")
}

async fn fixture(w: &World, scope: &UserScope) -> Vec<NoteId> {
    let mut ids = Vec::new();
    for (path, body) in NOTES {
        ids.push(w.create(scope, path, &format!("{body}\n")).await);
    }
    w.standard_runner().run_until_idle().await;
    ids
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    f64::from(a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>())
}

#[tokio::test]
async fn semantic_search_ranks_by_the_best_chunk_and_hybrid_fuses_with_arabic_keyword_hits() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let ids = fixture(&w, &sa).await;
    let retriever = Retriever::new(w.db.app_db.clone(), w.embedder_arc());

    // Semantic: cosine of the query and each note's chunk ("<title>\n<body>"), ties by chunk.
    let query = "Watanya contract safe";
    let q = bag_of_words(query, 384);
    let mut expected: Vec<(usize, f64)> = NOTES
        .iter()
        .enumerate()
        .map(|(i, (p, b))| (i, cosine(&q, &bag_of_words(&format!("{}\n{b}", title(p)), 384))))
        .collect();
    expected.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
    let hits = retriever
        .search(&sa, query, Mode::Semantic, 10)
        .await
        .expect("search");
    assert_eq!(
        hits.iter().map(|h| h.note.id).collect::<Vec<_>>(),
        expected.iter().map(|(i, _)| ids[*i]).collect::<Vec<_>>()
    );
    assert_eq!(hits[0].note.path, "notes/Watanya contract.md");
    for (h, (_, score)) in hits.iter().zip(&expected) {
        assert!((h.score - score).abs() < 1e-6, "{} vs {score}", h.score);
    }
    assert!(expected[0].1 > expected[1].1, "the contract note is clearly first");
    assert_eq!(
        hits[0].snippet.as_deref(),
        Some("The Watanya contract is stored in the safe at the Nasr City office.")
    );

    // Hybrid: "وطنيه" (ta marbuta written as ha) matches "وطنية" only through the normalised
    // full-text index; the fake embedder sees an unknown word (all cosines 0, chunk order).
    let hits = retriever
        .search(&sa, "وطنيه", Mode::Hybrid, 10)
        .await
        .expect("search");
    let r = |rank: f64| 1.0 / (RRF_K + rank);
    let want = [
        (ids[1], r(1.0) + r(2.0)),
        (ids[0], r(1.0)),
        (ids[2], r(3.0)),
        (ids[3], r(4.0)),
    ];
    assert_eq!(
        hits.iter().map(|h| h.note.id).collect::<Vec<_>>(),
        want.iter().map(|(id, _)| *id).collect::<Vec<_>>()
    );
    for (h, (_, s)) in hits.iter().zip(want) {
        assert!((h.score - s).abs() < 1e-12, "{} vs {s}", h.score);
    }
    assert_eq!(hits[0].note.title, "عقد وطنية");
    // The limit applies after fusion.
    let top = retriever
        .search(&sa, "وطنيه", Mode::Hybrid, 1)
        .await
        .expect("search");
    assert_eq!(top.len(), 1);
    assert!(matches!(
        retriever.search(&sa, "   ", Mode::Semantic, 5).await,
        Err(strata_jobs::retrieval::RetrievalError::EmptyQuery)
    ));
    w.finish().await;
}

#[tokio::test]
async fn similarity_edges_are_top_neighbours_above_the_floor() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let ids = fixture(&w, &sa).await;
    let retriever = Retriever::new(w.db.app_db.clone(), w.embedder_arc());
    let v = |i: usize| bag_of_words(&format!("{}\n{}", title(NOTES[i].0), NOTES[i].1), 384);
    let expected = cosine(&v(0), &v(3));
    assert!(expected > 0.1);
    let edges = retriever.similar(&sa, ids[0], 5, 0.05).await.expect("edges");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0, ids[3]);
    assert!((edges[0].1 - expected).abs() < 1e-6);
    // n bounds the list; floor 0 admits the unrelated notes (cosine 0), most similar first.
    let edges = retriever.similar(&sa, ids[0], 2, 0.0).await.expect("edges");
    assert_eq!(
        edges.iter().map(|e| e.0).collect::<Vec<_>>(),
        vec![ids[3], ids[1]]
    );
    // A note without a vector has no edges.
    let fresh = w.create(&sa, "notes/Fresh.md", "new\n").await;
    assert_eq!(retriever.similar(&sa, fresh, 5, 0.0).await.expect("edges"), vec![]);
    w.finish().await;
}

#[tokio::test]
async fn retrieval_never_crosses_users() {
    let w = World::new().await;
    let (_, sa) = w.user("alice").await;
    let (_, sb) = w.user("bob").await;
    let alice = fixture(&w, &sa).await;
    let bob = fixture(&w, &sb).await;
    let retriever = Retriever::new(w.db.app_db.clone(), w.embedder_arc());
    for (scope, own, other) in [(&sa, &alice, &bob), (&sb, &bob, &alice)] {
        for mode in [Mode::Semantic, Mode::Hybrid] {
            let hits = retriever
                .search(scope, "Watanya contract وطنية", mode, 20)
                .await
                .expect("search");
            assert_eq!(hits.len(), 4, "{mode:?}");
            assert!(hits.iter().all(|h| own.contains(&h.note.id)));
            assert!(hits.iter().all(|h| !other.contains(&h.note.id)));
        }
        let chunks = retriever
            .ask_chunks(scope, "Watanya contract", 10, None)
            .await
            .expect("chunks");
        assert!(chunks.iter().all(|c| own.contains(&c.note.id)));
        // Another user's note ID has no vector in this scope: no edges, no error.
        assert_eq!(
            retriever.similar(scope, other[0], 5, 0.0).await.expect("edges"),
            vec![]
        );
    }
    w.finish().await;
}
