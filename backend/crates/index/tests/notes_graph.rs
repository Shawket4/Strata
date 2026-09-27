//! Notes, keyword search, tags, aliases, links, relations, rejected edges, blocks, chunks.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use chrono::Duration;
use pgvector::Vector;
use pretty_assertions::assert_eq;
use strata_common::{ChunkId, Clock, NoteId, UserId};
use strata_index::repo::graph::{self, Block, Chunk, Link, Relation};
use strata_index::repo::notes::{self, Note, SearchText};
use strata_index::types::{By, Lang, LinkKind, NoteKind};
use strata_testkit::{TestDb, TestUser};

fn note(db: &TestDb, path: &str, title: &str) -> Note {
    let t = db.clock.now();
    Note {
        id: NoteId::generate(db.ids.as_ref()),
        path: path.into(),
        title: title.into(),
        kind: NoteKind::Note,
        lang: Some(Lang::En),
        created: t,
        updated: t,
        content_hash: format!("sha256:{title}"),
        word_count: 3,
        trashed: false,
    }
}

async fn user(db: &TestDb, name: &str) -> UserId {
    TestUser::new(name).create(db).await.expect("user").id
}

#[tokio::test]
async fn notes_upsert_get_list_delete() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let mut n1 = note(&db, "notes/B.md", "B");
    let n2 = Note {
        kind: NoteKind::Person,
        lang: Some(Lang::Ar),
        ..note(&db, "people/أحمد.md", "أحمد")
    };
    notes::upsert_note(&mut tx, &n1).await.expect("n1");
    notes::upsert_note(&mut tx, &n2).await.expect("n2");
    assert_eq!(
        notes::get_note(&mut tx, n1.id).await.expect("get"),
        Some(n1.clone())
    );
    assert_eq!(
        notes::get_note_by_path(&mut tx, "people/أحمد.md")
            .await
            .expect("by path"),
        Some(n2.clone())
    );

    n1.path = "notes/Renamed.md".into();
    n1.trashed = true;
    n1.updated += Duration::minutes(1);
    notes::upsert_note(&mut tx, &n1).await.expect("update");
    assert_eq!(
        notes::get_note(&mut tx, n1.id).await.expect("get"),
        Some(n1.clone())
    );
    assert_eq!(
        notes::list_notes(&mut tx, false).await.expect("list"),
        vec![n2.clone()]
    );
    assert_eq!(
        notes::list_notes(&mut tx, true).await.expect("list"),
        vec![n1.clone(), n2.clone()]
    );

    // Path is unique per user.
    let clash = Note {
        path: "people/أحمد.md".into(),
        ..note(&db, "x", "x")
    };
    let err = notes::upsert_note(&mut tx, &clash)
        .await
        .expect_err("dup path");
    assert!(err.is_unique_violation(), "{err}");
    tx.rollback().await.expect("rollback");

    let mut tx = db.begin(a).await.expect("tx");
    notes::upsert_note(&mut tx, &n1).await.expect("n1");
    assert!(notes::delete_note(&mut tx, n1.id).await.expect("delete"));
    assert!(!notes::delete_note(&mut tx, n1.id).await.expect("again"));
    assert_eq!(notes::get_note(&mut tx, n1.id).await.expect("get"), None);
    tx.commit().await.expect("commit");

    // Another user can use the same path, and cannot see A's notes.
    let b = user(&db, "bob").await;
    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(
        notes::get_note(&mut tx, n2.id).await.expect("foreign"),
        None
    );
    notes::upsert_note(
        &mut tx,
        &Note {
            id: NoteId::generate(db.ids.as_ref()),
            ..n2.clone()
        },
    )
    .await
    .expect("same path for B");
}

#[tokio::test]
async fn keyword_search_ranks_title_over_body_and_is_per_user() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let b = user(&db, "bob").await;
    let mut tx = db.begin(a).await.expect("tx");
    let title_hit = note(&db, "notes/pricing.md", "pricing");
    let body_hit = note(&db, "notes/other.md", "other");
    let trashed = Note {
        trashed: true,
        ..note(&db, "notes/old.md", "old")
    };
    for (n, text) in [
        (
            &title_hit,
            SearchText {
                title: "pricing experiments",
                tags: "pos",
                body: "we tried tiers",
            },
        ),
        (
            &body_hit,
            SearchText {
                title: "meeting",
                tags: "",
                body: "pricing came up once",
            },
        ),
        (
            &trashed,
            SearchText {
                title: "pricing",
                tags: "",
                body: "",
            },
        ),
    ] {
        notes::upsert_note(&mut tx, n).await.expect("note");
        assert!(
            notes::set_note_search(&mut tx, n.id, text)
                .await
                .expect("search")
        );
    }
    let hits = notes::search_notes(&mut tx, "pricing", 10)
        .await
        .expect("search");
    assert_eq!(
        hits.iter().map(|h| h.note_id).collect::<Vec<_>>(),
        vec![title_hit.id, body_hit.id]
    );
    assert!(hits[0].rank > hits[1].rank);
    assert_eq!(
        notes::search_notes(&mut tx, "pricing -tiers", 10)
            .await
            .expect("search")
            .iter()
            .map(|h| h.note_id)
            .collect::<Vec<_>>(),
        vec![body_hit.id]
    );
    assert!(
        !notes::set_note_search(
            &mut tx,
            NoteId::generate(db.ids.as_ref()),
            SearchText {
                title: "",
                tags: "",
                body: ""
            }
        )
        .await
        .expect("missing")
    );
    tx.commit().await.expect("commit");

    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(
        notes::search_notes(&mut tx, "pricing", 10)
            .await
            .expect("search"),
        vec![]
    );
}

#[tokio::test]
async fn tags_aliases_links_and_backlinks() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let src = note(&db, "notes/src.md", "src");
    let dst = note(&db, "notes/dst.md", "dst");
    notes::upsert_note(&mut tx, &src).await.expect("src");
    notes::upsert_note(&mut tx, &dst).await.expect("dst");

    graph::replace_tags(
        &mut tx,
        src.id,
        &["pos".into(), "pricing".into(), "pos".into()],
    )
    .await
    .expect("tags");
    assert_eq!(
        graph::tags_for(&mut tx, src.id).await.expect("tags"),
        vec!["pos", "pricing"]
    );
    graph::replace_tags(&mut tx, src.id, &["loyalty".into()])
        .await
        .expect("retag");
    assert_eq!(
        graph::tags_for(&mut tx, src.id).await.expect("tags"),
        vec!["loyalty"]
    );
    assert_eq!(
        graph::notes_with_tag(&mut tx, "loyalty")
            .await
            .expect("by tag"),
        vec![src.id]
    );

    graph::replace_aliases(&mut tx, dst.id, &["target".into(), "هدف".into()])
        .await
        .expect("aliases");
    assert_eq!(
        graph::aliases_for(&mut tx, dst.id).await.expect("aliases"),
        vec!["target", "هدف"]
    );

    let links = vec![
        Link {
            src_id: src.id,
            ord: 0,
            dst_id: Some(dst.id),
            dst_raw: "dst".into(),
            kind: LinkKind::Link,
            anchor: Some("Heading".into()),
            block_id: None,
        },
        Link {
            src_id: src.id,
            ord: 1,
            dst_id: None,
            dst_raw: "missing".into(),
            kind: LinkKind::Embed,
            anchor: None,
            block_id: Some("abc".into()),
        },
    ];
    graph::replace_links(&mut tx, src.id, &links)
        .await
        .expect("links");
    assert_eq!(
        graph::links_from(&mut tx, src.id).await.expect("from"),
        links
    );
    assert_eq!(
        graph::backlinks(&mut tx, dst.id).await.expect("back"),
        vec![links[0].clone()]
    );

    // Deleting the target unresolves (not deletes) the link.
    notes::delete_note(&mut tx, dst.id)
        .await
        .expect("delete dst");
    let after = graph::links_from(&mut tx, src.id).await.expect("from");
    assert_eq!(after[0].dst_id, None);
    assert_eq!(after[0].dst_raw, "dst");
}

#[tokio::test]
async fn relations_and_rejections() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let (n1, n2, n3) = (
        note(&db, "a.md", "a"),
        note(&db, "b.md", "b"),
        note(&db, "c.md", "c"),
    );
    for n in [&n1, &n2, &n3] {
        notes::upsert_note(&mut tx, n).await.expect("note");
    }
    let ai = Relation {
        src_id: n1.id,
        dst_id: n2.id,
        rel_type: "contradicts".into(),
        by: By::Ai,
        confidence: Some(0.72),
        reason: Some("caps differ".into()),
        created: t,
        decision_id: None,
    };
    let user_rel = Relation {
        src_id: n3.id,
        dst_id: n2.id,
        rel_type: "related".into(),
        by: By::User,
        confidence: None,
        reason: None,
        created: t,
        decision_id: None,
    };
    graph::upsert_relation(&mut tx, &ai).await.expect("ai");
    graph::upsert_relation(&mut tx, &user_rel)
        .await
        .expect("user");
    assert_eq!(
        graph::relations_from(&mut tx, n1.id).await.expect("from"),
        vec![ai.clone()]
    );
    assert_eq!(
        graph::relations_to(&mut tx, n2.id).await.expect("to"),
        vec![ai.clone(), user_rel.clone()]
    );

    let retyped = Relation {
        confidence: Some(0.9),
        ..ai.clone()
    };
    graph::upsert_relation(&mut tx, &retyped)
        .await
        .expect("update");
    assert_eq!(
        graph::remove_relation(&mut tx, n1.id, n2.id, "contradicts")
            .await
            .expect("remove"),
        Some(retyped)
    );
    assert_eq!(
        graph::remove_relation(&mut tx, n1.id, n2.id, "contradicts")
            .await
            .expect("again"),
        None
    );

    // Self-relations are rejected by the schema.
    let err = graph::upsert_relation(
        &mut tx,
        &Relation {
            dst_id: n1.id,
            ..ai.clone()
        },
    )
    .await
    .expect_err("self");
    assert_eq!(err.sqlstate().as_deref(), Some("23514"));
    tx.rollback().await.expect("rollback");

    let mut tx = db.begin(a).await.expect("tx");
    for n in [&n1, &n2] {
        notes::upsert_note(&mut tx, n).await.expect("note");
    }
    graph::add_rejected(&mut tx, n1.id, n2.id, "supports", t)
        .await
        .expect("reject");
    graph::add_rejected(&mut tx, n1.id, n2.id, "supports", t)
        .await
        .expect("idempotent");
    assert!(
        graph::is_rejected(&mut tx, n1.id, n2.id, "supports")
            .await
            .expect("exact")
    );
    assert!(
        graph::is_rejected(&mut tx, n1.id, n2.id, "related")
            .await
            .expect("related blocked too")
    );
    assert!(
        !graph::is_rejected(&mut tx, n1.id, n2.id, "contradicts")
            .await
            .expect("other type")
    );
    assert!(
        !graph::is_rejected(&mut tx, n2.id, n1.id, "supports")
            .await
            .expect("direction matters")
    );
}

#[tokio::test]
async fn blocks_and_chunks_with_nearest_neighbour_search() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let mut tx = db.begin(a).await.expect("tx");
    let n = note(&db, "n.md", "n");
    notes::upsert_note(&mut tx, &n).await.expect("note");
    let blocks = vec![
        Block {
            note_id: n.id,
            block_id: "b2".into(),
            heading_path: "Summary".into(),
            text: "second".into(),
            start_offset: 10,
            end_offset: 16,
        },
        Block {
            note_id: n.id,
            block_id: "b1".into(),
            heading_path: String::new(),
            text: "first".into(),
            start_offset: 0,
            end_offset: 5,
        },
    ];
    graph::replace_blocks(&mut tx, n.id, &blocks)
        .await
        .expect("blocks");
    assert_eq!(
        graph::blocks_for(&mut tx, n.id).await.expect("blocks"),
        vec![blocks[1].clone(), blocks[0].clone()]
    );

    let axis = |i: usize| {
        let mut v = vec![0.0_f32; 384];
        v[i] = 1.0;
        Vector::from(v)
    };
    let chunks = vec![
        Chunk {
            id: ChunkId::generate(db.ids.as_ref()),
            note_id: n.id,
            block_id: Some("b1".into()),
            text: "first".into(),
            token_count: 1,
            embedding: Some(axis(0)),
            model: Some("granite-r2".into()),
        },
        Chunk {
            id: ChunkId::generate(db.ids.as_ref()),
            note_id: n.id,
            block_id: Some("b2".into()),
            text: "second".into(),
            token_count: 1,
            embedding: Some(axis(1)),
            model: Some("granite-r2".into()),
        },
        Chunk {
            id: ChunkId::generate(db.ids.as_ref()),
            note_id: n.id,
            block_id: None,
            text: "pending".into(),
            token_count: 1,
            embedding: None,
            model: None,
        },
    ];
    graph::replace_chunks(&mut tx, n.id, &chunks)
        .await
        .expect("chunks");
    assert_eq!(
        graph::chunks_for(&mut tx, n.id).await.expect("chunks"),
        chunks
    );
    let hits = graph::nearest_chunks(&mut tx, &axis(1), "granite-r2", 5)
        .await
        .expect("knn");
    assert_eq!(
        hits.iter()
            .map(|h| (h.chunk_id, h.distance))
            .collect::<Vec<_>>(),
        vec![(chunks[1].id, 0.0), (chunks[0].id, 1.0)]
    );
    assert_eq!(
        graph::nearest_chunks(&mut tx, &axis(1), "other-model", 5)
            .await
            .expect("knn"),
        vec![]
    );

    // A vector must have exactly 384 dimensions.
    let bad = Chunk {
        embedding: Some(Vector::from(vec![1.0_f32; 3])),
        ..chunks[0].clone()
    };
    let err = graph::replace_chunks(&mut tx, n.id, &[bad])
        .await
        .expect_err("dims");
    assert_eq!(err.sqlstate().as_deref(), Some("22000"));
}
