//! Notes over HTTP (PLAN §7.5 Notes, Search, §15, §16.3): exact payloads, `If-Match`
//! conflicts, move/rename, trash, backlinks, history and revert, keyword search with Arabic
//! normalisation, and path traversal on every path parameter. Every response is validated
//! against the contract.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod vault_harness;

use pretty_assertions::assert_eq;
use reqwest::Method;
use strata_client::{operations as ops, types};
use vault_harness::{H, assert_problem, epoch, header, not_found, plain, version};

fn tree_folders() -> Vec<types::TreeItem> {
    [
        "_ai",
        "_ai/digests",
        "attachments",
        "companies",
        "concepts",
        "documents",
        "inbox",
        "maps",
        "notes",
        "people",
        "places",
        "tasks",
    ]
    .iter()
    .map(|p| types::TreeItem::Folder {
        path: (*p).to_owned(),
    })
    .collect()
}

fn props(id: &str, updated: &str) -> Vec<types::Property> {
    vec![
        types::Property {
            key: "id".into(),
            value: types::PropertyValueDto::Text { value: id.into() },
        },
        types::Property {
            key: "created".into(),
            value: types::PropertyValueDto::Text {
                value: "2026-09-27T12:00:00+00:00".into(),
            },
        },
        types::Property {
            key: "updated".into(),
            value: types::PropertyValueDto::Text {
                value: updated.into(),
            },
        },
    ]
}

#[tokio::test]
async fn notes_crud_move_trash_history_and_revert() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;

    assert_eq!(
        ops::get_tree(c).await.expect("tree"),
        types::Tree {
            entries: tree_folders()
        }
    );

    let created = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/Pricing.md".into(),
            content: "# Pricing\n\nTiers ^t1\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("create");
    let id = created.id.to_string();
    let content = format!("{}# Pricing\n\nTiers ^t1\n", header(&id));
    let expected = types::Note {
        content: content.clone(),
        created: epoch(),
        id: created.id,
        kind: types::NoteKind::Note,
        path: "notes/Pricing.md".into(),
        properties: props(&id, "2026-09-27T12:00:00+00:00"),
        title: "Pricing".into(),
        trashed: false,
        updated: epoch(),
        version: version(&content),
    };
    assert_eq!(created, expected);
    assert_eq!(h.read(alice.id, "notes/Pricing.md"), content);
    assert_eq!(ops::get_note(c, created.id).await.expect("get"), expected);
    assert_eq!(
        ops::get_note_by_path(c, "notes/Pricing.md")
            .await
            .expect("by path"),
        expected
    );
    assert_problem(
        ops::get_note_by_path(c, "notes/Nope.md").await,
        &not_found(),
    );

    // Stale If-Match → 409 with the current version; no If-Match → 422.
    h.clock.advance(chrono::Duration::minutes(10));
    assert_problem(
        ops::update_note(
            c,
            created.id,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            &types::UpdateNoteRequest {
                content: "x".into(),
            },
        )
        .await,
        &types::Problem {
            current_version: Some(expected.version.clone()),
            ..plain("version_conflict", "Version conflict", 409, None)
        },
    );
    let (status, _, body) = h
        .raw(
            &alice.token,
            Method::PUT,
            &format!("/api/v1/notes/{}", created.id),
            Some("update_note"),
            &[],
            Some((
                "application/vnd.msgpack",
                rmp_serde::to_vec_named(&types::UpdateNoteRequest {
                    content: "x".into(),
                })
                .expect("encode"),
            )),
        )
        .await;
    assert_eq!(status, 422);
    let p: types::Problem = rmp_serde::from_slice(&body).expect("problem");
    assert_eq!(
        p,
        types::Problem {
            errors: vec![types::ProblemFieldError {
                code: "missing_header".into(),
                pointer: Some("If-Match".into()),
                message: "If-Match with the current version is required".into(),
            }],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("If-Match with the current version is required")
            )
        }
    );

    let updated = ops::update_note(
        c,
        created.id,
        &expected.version,
        &types::UpdateNoteRequest {
            content: format!("{}# Pricing\n\nTiers ^t1\n\nMore.\n", header(&id)),
        },
    )
    .await
    .expect("update");
    let content2 = format!(
        "---\nid: {id}\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:10:00+00:00\n---\n# Pricing\n\nTiers ^t1\n\nMore.\n"
    );
    assert_eq!(
        updated,
        types::Note {
            content: content2.clone(),
            properties: props(&id, "2026-09-27T12:10:00+00:00"),
            updated: epoch() + chrono::Duration::minutes(10),
            version: version(&content2),
            ..expected.clone()
        }
    );

    // A second note linking to it; move rewrites the link.
    let refs = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/Refs.md".into(),
            content: "See [[Pricing#^t1]] and [[Pricing]].\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("refs");
    let moved = ops::move_note(
        c,
        created.id,
        None,
        &types::MoveNoteRequest {
            new_path: "archive/Pricing 2026.md".into(),
        },
    )
    .await
    .expect("move");
    assert_eq!(
        (moved.path.as_str(), moved.title.as_str(), moved.content == content2),
        ("archive/Pricing 2026.md", "Pricing 2026", true)
    );
    assert_eq!(
        ops::get_note(c, refs.id).await.expect("refs").content,
        format!(
            "---\nid: {}\ncreated: 2026-09-27T12:10:00+00:00\nupdated: 2026-09-27T12:10:00+00:00\n---\nSee [[Pricing 2026#^t1]] and [[Pricing 2026]].\n",
            refs.id
        )
    );
    assert_eq!(
        ops::get_backlinks(c, created.id).await.expect("backlinks"),
        types::Backlinks {
            groups: vec![types::BacklinkGroup {
                kind: "link".into(),
                items: vec![
                    types::Backlink {
                        anchor: None,
                        block_id: None,
                        by: None,
                        confidence: None,
                        source_id: refs.id,
                        source_path: "notes/Refs.md".into(),
                        source_title: "Refs".into(),
                    },
                    types::Backlink {
                        anchor: None,
                        block_id: Some("t1".into()),
                        by: None,
                        confidence: None,
                        source_id: refs.id,
                        source_path: "notes/Refs.md".into(),
                        source_title: "Refs".into(),
                    },
                ],
            }],
        }
    );
    assert_eq!(
        ops::get_tree(c).await.expect("tree").entries,
        [
            vec![tree_folders()[0].clone(), tree_folders()[1].clone()],
            vec![types::TreeItem::Folder {
                path: "archive".into()
            }],
            vec![types::TreeItem::Note {
                id: created.id,
                kind: types::NoteKind::Note,
                path: "archive/Pricing 2026.md".into(),
                title: "Pricing 2026".into(),
                updated: epoch() + chrono::Duration::minutes(10),
            }],
            tree_folders()[2..9].to_vec(),
            vec![types::TreeItem::Note {
                id: refs.id,
                kind: types::NoteKind::Note,
                path: "notes/Refs.md".into(),
                title: "Refs".into(),
                updated: epoch() + chrono::Duration::minutes(10),
            }],
            tree_folders()[9..].to_vec(),
        ]
        .concat()
    );

    // History, revision, revert.
    let history = ops::get_note_history(c, created.id).await.expect("history");
    let summary: Vec<(types::CommitAuthor, String, types::FileChange, String)> = history
        .revisions
        .iter()
        .map(|r| (r.author, r.message.clone(), r.change, r.path.clone()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                types::CommitAuthor::User,
                "user: move notes/Pricing.md -> archive/Pricing 2026.md".into(),
                types::FileChange::Renamed,
                "archive/Pricing 2026.md".into()
            ),
            (
                types::CommitAuthor::User,
                "user: update notes/Pricing.md".into(),
                types::FileChange::Modified,
                "notes/Pricing.md".into()
            ),
            (
                types::CommitAuthor::User,
                "user: create notes/Pricing.md".into(),
                types::FileChange::Added,
                "notes/Pricing.md".into()
            ),
        ]
    );
    let first = &history.revisions[2].commit;
    assert_eq!(
        ops::get_note_revision(c, created.id, first)
            .await
            .expect("revision"),
        types::NoteRevision {
            commit: first.clone(),
            content: content.clone(),
            path: "notes/Pricing.md".into(),
            version: version(&content),
        }
    );
    assert_problem(
        ops::get_note_revision(c, created.id, "0123456789012345678901234567890123456789").await,
        &not_found(),
    );
    let reverted = ops::revert_note(
        c,
        created.id,
        &types::RevertNoteRequest {
            commit: first.clone(),
        },
    )
    .await
    .expect("revert");
    assert_eq!(
        (reverted.path.as_str(), reverted.content.as_str()),
        ("archive/Pricing 2026.md", content.as_str())
    );
    assert_eq!(h.log(alice.id)[0], "user: revert archive/Pricing 2026.md");

    // Trash, restore, purge.
    let trashed = ops::delete_note(c, created.id).await.expect("delete");
    assert_eq!(
        (trashed.path.as_str(), trashed.trashed),
        (".trash/archive/Pricing 2026.md", true)
    );
    assert_eq!(
        ops::get_backlinks(c, refs.id).await.expect("refs backlinks"),
        types::Backlinks { groups: vec![] }
    );
    assert_problem(ops::get_backlinks(c, created.id).await, &not_found());
    let restored = ops::restore_note(c, created.id).await.expect("restore");
    assert_eq!(
        (restored.path.as_str(), restored.trashed),
        ("archive/Pricing 2026.md", false)
    );
    assert_problem(ops::purge_note(c, created.id).await, &not_found());
    ops::delete_note(c, created.id).await.expect("delete");
    ops::purge_note(c, created.id).await.expect("purge");
    assert_problem(ops::get_note(c, created.id).await, &not_found());
    assert_eq!(
        h.log(alice.id)[..4],
        [
            "user: purge .trash/archive/Pricing 2026.md".to_owned(),
            "user: delete archive/Pricing 2026.md".to_owned(),
            "user: restore archive/Pricing 2026.md".to_owned(),
            "user: delete archive/Pricing 2026.md".to_owned(),
        ]
    );
    h.finish().await;
}

#[tokio::test]
async fn whole_commit_revert_over_http() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let t = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/T.md".into(),
            content: "t\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("t");
    let n = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/N.md".into(),
            content: "n\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("n");
    let scope = h.db.scope(alice.id);
    let ai = h
        .vault
        .ai_add_relations(
            &scope,
            "link".into(),
            strata_common::NoteId::from_ulid(n.id),
            vec![strata_vault::ops::relations::AiEdge {
                rel: vault_format::RelationKey::Note(domain::RelationType::Related),
                dst: strata_common::NoteId::from_ulid(t.id),
                confidence: 0.9,
                reason: "same".into(),
                model: "fake".into(),
            }],
        )
        .await
        .expect("ai")
        .expect("commit");
    let reverted = ops::revert_commit(c, &ai).await.expect("revert");
    assert_eq!(
        reverted.paths,
        vec![format!(".meta/notes/{}.json", n.id), "notes/N.md".to_owned()]
    );
    assert_eq!(ops::get_note(c, n.id).await.expect("n").content, n.content);
    assert_problem(
        ops::revert_commit(c, "0123456789012345678901234567890123456789").await,
        &not_found(),
    );
    h.finish().await;
}

#[tokio::test]
async fn keyword_search_normalises_arabic_and_semantic_needs_ai() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let n = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/عقد.md".into(),
            content: "عَقْد شركة وطنيّة\nسطر آخر\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("note");
    let hits = ops::search(c, "وطنيه", None, None).await.expect("search");
    assert_eq!(
        hits.hits
            .iter()
            .map(|h| (h.id, h.path.clone(), h.snippet.clone()))
            .collect::<Vec<_>>(),
        vec![(
            n.id,
            "notes/عقد.md".to_owned(),
            Some("عَقْد شركة وطنيّة".to_owned())
        )]
    );
    assert!(hits.hits[0].score > 0.0);
    assert_eq!(
        ops::search(c, "missing", None, None).await.expect("search"),
        types::SearchResults { hits: vec![] }
    );
    for mode in [types::SearchMode::Semantic, types::SearchMode::Hybrid] {
        assert_problem(
            ops::search(c, "x", Some(&mode), None).await,
            &plain(
                "ai_unavailable",
                "AI unavailable",
                503,
                Some(
                    "semantic and hybrid search need the AI subsystem, which is not available yet; use mode=keyword",
                ),
            ),
        );
    }
    assert_problem(
        ops::search(c, "  ؟ ", None, None).await,
        &types::Problem {
            errors: vec![types::ProblemFieldError {
                code: "empty_query".into(),
                pointer: Some("q".into()),
                message: "the query is empty after normalisation".into(),
            }],
            ..plain(
                "invalid_parameter",
                "Request parameter is invalid",
                422,
                Some("the query is empty after normalisation"),
            )
        },
    );
    h.finish().await;
}

#[tokio::test]
async fn path_traversal_is_rejected_on_every_path_parameter() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let invalid = |detail: &str| plain("invalid_name", "Invalid name", 422, Some(detail));
    for (path, detail) in [
        ("../evil.md", "a path segment starts with a dot"),
        ("notes/../../etc/passwd.md", "a path segment starts with a dot"),
        ("/etc/passwd.md", "the path must be relative to the vault"),
        (".meta/notes/x.md", "a path segment starts with a dot"),
        (".git/config.md", "a path segment starts with a dot"),
        ("notes\\..\\x.md", "the name contains a character Obsidian forbids"),
        ("notes/x.txt", "a note path must end in .md"),
    ] {
        assert_problem(
            ops::create_note(
                c,
                &types::CreateNoteRequest {
                    path: path.into(),
                    content: "x".into(),
                    id: None,
                    force: None,
                },
            )
            .await,
            &invalid(detail),
        );
        assert_problem(ops::get_note_by_path(c, path).await, &invalid(detail));
    }
    let n = ops::create_note(
        c,
        &types::CreateNoteRequest {
            path: "notes/A.md".into(),
            content: "a\n".into(),
            id: None,
            force: None,
        },
    )
    .await
    .expect("a");
    assert_problem(
        ops::move_note(
            c,
            n.id,
            None,
            &types::MoveNoteRequest {
                new_path: "../../outside.md".into(),
            },
        )
        .await,
        &invalid("a path segment starts with a dot"),
    );
    // Path segments that are not IDs (or not commit IDs) are 404 on every route.
    let bad_ids = ["..%2F..%2Fetc%2Fpasswd", "%2E%2E", "notes%2FA.md", "01J8ZK3M4X7Q9W2E5R6T8Y0V1H%2F.."];
    let routes: [(Method, &str, &str); 9] = [
        (Method::GET, "/api/v1/notes/{}", "get_note"),
        (Method::GET, "/api/v1/notes/{}/history", "get_note_history"),
        (Method::GET, "/api/v1/notes/{}/backlinks", "get_backlinks"),
        (Method::DELETE, "/api/v1/trash/{}", "purge_note"),
        (Method::POST, "/api/v1/trash/{}/restore", "restore_note"),
        (Method::GET, "/api/v1/entities/{}", "get_entity"),
        (Method::GET, "/api/v1/documents/{}", "get_document"),
        (Method::GET, "/api/v1/places/{}", "get_place"),
        (Method::POST, "/api/v1/tasks/{}/complete", "complete_task"),
    ];
    for bad in bad_ids {
        for (method, route, op) in &routes {
            let (status, _, body) = h
                .raw(
                    &alice.token,
                    method.clone(),
                    &route.replace("{}", bad),
                    Some(op),
                    &[],
                    None,
                )
                .await;
            let p: types::Problem = rmp_serde::from_slice(&body).expect("problem");
            // `%2E%2E` is normalised away by the URL parser before the request is sent, so
            // that probe ends at no route at all.
            assert!(
                status == 404 && matches!(p.type_.as_str(), "not_found" | "route_not_found"),
                "{route} {bad}: {status} {}",
                p.type_
            );
        }
        for (method, route, op) in [
            (Method::GET, format!("/api/v1/notes/{}/history/{bad}", n.id), "get_note_revision"),
            (Method::POST, format!("/api/v1/commits/{bad}/revert"), "revert_commit"),
        ] {
            let (status, _, body) = h.raw(&alice.token, method, &route, Some(op), &[], None).await;
            let p: types::Problem = rmp_serde::from_slice(&body).expect("problem");
            assert!(
                status == 404 && matches!(p.type_.as_str(), "not_found" | "route_not_found"),
                "{route}: {status} {}",
                p.type_
            );
        }
    }
    // Nothing escaped the vault.
    assert!(!h.data.path().join("users").join("outside.md").exists());
    assert!(!h.dir(alice.id).join("../evil.md").exists());
    h.finish().await;
}
