//! Isolation (PLAN §5 principle 7, §16.3 Isolation): a second user with overlapping titles
//! sees nothing of the first — no duplicate flags, no search hits, no list entries, nothing in
//! the export — and every foreign ID answers exactly like a nonexistent one (404).
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod vault_harness;

use std::collections::HashMap;

use pretty_assertions::assert_eq;
use strata_client::{Error, operations as ops, types};
use vault_harness::{H, User, not_found};

/// Everything one user owns, created with the same titles for both users.
struct Owned {
    note: types::Note,
    only: types::Note,
    person: types::Entity,
    company: types::Entity,
    doc: types::Document,
    place: types::Place,
    task: types::Task,
    suggestion: ulid::Ulid,
    commit: String,
}

async fn populate(u: &User, only_path: &str) -> Owned {
    let c = &u.client;
    let note = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "The quarterly plan for [[Watanya]].\n".into(),
            force: None,
            id: None,
            path: "notes/Plan.md".into(),
        },
    )
    .await
    .expect("note");
    let only = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "Private.\n".into(),
            force: None,
            id: None,
            path: only_path.into(),
        },
    )
    .await
    .expect("only");
    let person = ops::create_entity(
        c,
        &types::CreateEntityRequest {
            aliases: vec!["واتانيا".into()],
            fields: HashMap::new(),
            force: None,
            id: None,
            kind: types::EntityKind::Person,
            name: "Watanya".into(),
            parent_id: None,
            tags: vec![],
        },
    )
    .await
    .expect("person (never flagged against the other user)");
    let company = ops::create_entity(
        c,
        &types::CreateEntityRequest {
            aliases: vec![],
            fields: HashMap::new(),
            force: None,
            id: None,
            kind: types::EntityKind::Company,
            name: "Acme Trading".into(),
            parent_id: None,
            tags: vec![],
        },
    )
    .await
    .expect("company");
    let place = ops::create_place(
        c,
        &types::CreatePlaceRequest {
            address: None,
            aliases: vec![],
            force: None,
            id: None,
            name: "Safe".into(),
            parent_id: None,
            tags: vec![],
        },
    )
    .await
    .expect("place");
    let doc = ops::create_document(
        c,
        &types::CreateDocumentRequest {
            aliases: vec![],
            copy: None,
            doc_type: Some("passport".into()),
            expires: None,
            force: None,
            id: None,
            name: "Passport".into(),
            tags: vec![],
        },
    )
    .await
    .expect("document");
    let doc = ops::add_custody_event(
        c,
        doc.document.id,
        &types::CustodyEventRequest {
            at: chrono::NaiveDate::from_ymd_opt(2026, 9, 1).expect("date"),
            counterparty_id: None,
            person_id: None,
            place_id: Some(place.place.id),
            source_note_id: None,
            type_: types::CustodyEventKind::StoredAt,
        },
    )
    .await
    .expect("custody");
    let task = ops::create_task(
        c,
        &types::CreateTaskRequest {
            text: "Pay the rent [[Watanya]]".into(),
            due: None,
            force: None,
            id: None,
            note_id: None,
            priority: None,
            recurrence: None,
            reminders: vec![],
            scheduled: None,
            start: None,
        },
    )
    .await
    .expect("task");
    let text = "Ask Watanya about the quarterly plan";
    ops::capture(c, &types::CaptureRequest { text: text.into() })
        .await
        .expect("capture");
    let suggestion = ops::capture(c, &types::CaptureRequest { text: text.into() })
        .await
        .expect("capture")
        .suggestion_id
        .expect("suggestion");
    let commit = ops::get_note_history(c, note.id)
        .await
        .expect("history")
        .revisions[0]
        .commit
        .clone();
    Owned {
        note,
        only,
        person,
        company,
        doc,
        place,
        task,
        suggestion,
        commit,
    }
}

fn nf<T: std::fmt::Debug>(what: &str, r: Result<T, Error>) {
    let err = r.expect_err(what);
    assert_eq!(vault_harness::problem(&err), not_found(), "{what}");
}

fn paths(tree: &types::Tree) -> Vec<String> {
    tree.entries
        .iter()
        .filter_map(|e| match e {
            types::TreeItem::Note { path, .. } | types::TreeItem::File { path, .. } => {
                Some(path.clone())
            }
            types::TreeItem::Folder { .. } => None,
        })
        .collect()
}

#[tokio::test]
async fn a_second_user_sees_nothing_and_foreign_ids_are_not_found() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let a = populate(&alice, "notes/Alice only.md").await;
    let b = populate(&bob, "notes/Bob only.md").await;
    let c = &bob.client;

    // Lists and searches hold only bob's own items.
    let found = ops::search(c, "quarterly plan", None, None).await.expect("search");
    let mut hit_ids: Vec<_> = found.hits.iter().map(|x| x.id).collect();
    hit_ids.sort();
    assert!(!hit_ids.contains(&a.note.id), "{hit_ids:?}");
    assert!(hit_ids.contains(&b.note.id));
    let people = ops::list_entities(c, None, Some("واتانيا"), None, None)
        .await
        .expect("entities");
    assert_eq!(
        people.items.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![b.person.id]
    );
    let all = ops::list_entities(c, None, None, None, None).await.expect("all");
    assert!(all.items.iter().all(|e| e.id != a.person.id && e.id != a.company.id));
    assert_eq!(
        ops::list_documents(c, None, None, None, None, None)
            .await
            .expect("docs")
            .items
            .iter()
            .map(|d| d.id)
            .collect::<Vec<_>>(),
        vec![b.doc.document.id]
    );
    assert_eq!(
        ops::list_places(c, None)
            .await
            .expect("places")
            .items
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![b.place.place.id]
    );
    assert_eq!(
        ops::list_tasks(c, None, None, None)
            .await
            .expect("tasks")
            .items
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        vec![b.task.id.clone()]
    );
    let inbox = ops::get_inbox(c).await.expect("inbox");
    assert_eq!(inbox.items.len(), 2);
    assert!(inbox.items.iter().all(|i| i.suggestions.iter().all(|s| s.id != a.suggestion)));
    let pending = ops::list_suggestions(c, None).await.expect("suggestions");
    assert_eq!(
        pending.items.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![b.suggestion]
    );
    let tree = paths(&ops::get_tree(c).await.expect("tree"));
    assert!(tree.contains(&"notes/Bob only.md".to_owned()));
    assert!(!tree.contains(&"notes/Alice only.md".to_owned()));
    assert_eq!(ops::get_integrity(c).await.expect("integrity").warnings, vec![]);
    let export = ops::export_vault(c).await.expect("export");
    let names: Vec<String> = {
        let z = zip::ZipArchive::new(std::io::Cursor::new(export.to_vec())).expect("zip");
        z.file_names().map(str::to_owned).collect()
    };
    assert!(names.contains(&"notes/Bob only.md".to_owned()));
    assert!(!names.contains(&"notes/Alice only.md".to_owned()));
    assert!(
        !names.iter().any(|n| n.contains(&a.note.id.to_string())),
        "{names:?}"
    );
    // Her mention counts and backlinks never include alice's notes.
    let backlinks = ops::get_backlinks(c, b.person.id).await.expect("backlinks");
    let from: Vec<_> = backlinks
        .groups
        .iter()
        .flat_map(|g| g.items.iter().map(|i| i.source_id))
        .collect();
    assert!(!from.contains(&a.note.id));
    nf("by path", ops::get_note_by_path(c, "notes/Alice only.md").await);

    // Every endpoint taking an ID answers 404 for alice's IDs.
    let an = a.note.id;
    nf("get_note", ops::get_note(c, an).await);
    nf(
        "update_note",
        ops::update_note(
            c,
            an,
            &a.note.version,
            &types::UpdateNoteRequest {
                content: "x\n".into(),
            },
        )
        .await,
    );
    nf(
        "move_note",
        ops::move_note(
            c,
            an,
            None,
            &types::MoveNoteRequest {
                new_path: "notes/Stolen.md".into(),
            },
        )
        .await,
    );
    nf("delete_note", ops::delete_note(c, an).await);
    nf("restore_note", ops::restore_note(c, an).await);
    nf("purge_note", ops::purge_note(c, a.only.id).await);
    nf("get_backlinks", ops::get_backlinks(c, an).await);
    nf("get_note_history", ops::get_note_history(c, an).await);
    nf("get_note_revision", ops::get_note_revision(c, an, &a.commit).await);
    nf(
        "get_note_revision (own note, foreign commit)",
        ops::get_note_revision(c, b.note.id, &a.commit).await,
    );
    nf(
        "revert_note",
        ops::revert_note(c, an, &types::RevertNoteRequest { commit: a.commit.clone() }).await,
    );
    nf("revert_commit", ops::revert_commit(c, &a.commit).await);
    nf("get_entity", ops::get_entity(c, a.person.id).await);
    nf(
        "patch_entity",
        ops::patch_entity(
            c,
            a.person.id,
            None,
            &types::PatchEntityRequest {
                name: Some("Mine".into()),
                ..Default::default()
            },
        )
        .await,
    );
    nf("get_entity_notes", ops::get_entity_notes(c, a.person.id).await);
    nf("get_entity_documents", ops::get_entity_documents(c, a.person.id).await);
    nf(
        "merge into foreign",
        ops::merge_entity(c, b.person.id, &types::MergeRequest { into_id: a.person.id }).await,
    );
    nf(
        "merge foreign",
        ops::merge_entity(c, a.person.id, &types::MergeRequest { into_id: b.person.id }).await,
    );
    nf("get_document", ops::get_document(c, a.doc.document.id).await);
    nf(
        "patch_document",
        ops::patch_document(
            c,
            a.doc.document.id,
            None,
            &types::PatchDocumentRequest {
                doc_type: Some("visa".into()),
                ..Default::default()
            },
        )
        .await,
    );
    let custody = |doc: ulid::Ulid, place: ulid::Ulid| types::CustodyEventRequest {
        at: chrono::NaiveDate::from_ymd_opt(2026, 9, 2).expect("date"),
        counterparty_id: None,
        person_id: None,
        place_id: Some(place),
        source_note_id: None,
        type_: types::CustodyEventKind::MovedTo,
    };
    nf(
        "custody on foreign document",
        ops::add_custody_event(c, a.doc.document.id, &custody(a.doc.document.id, b.place.place.id))
            .await,
    );
    nf(
        "custody with foreign place",
        ops::add_custody_event(c, b.doc.document.id, &custody(b.doc.document.id, a.place.place.id))
            .await,
    );
    nf("get_place", ops::get_place(c, a.place.place.id).await);
    nf(
        "patch_place",
        ops::patch_place(
            c,
            a.place.place.id,
            None,
            &types::PatchPlaceRequest {
                name: Some("Mine".into()),
                ..Default::default()
            },
        )
        .await,
    );
    nf(
        "patch_task",
        ops::patch_task(
            c,
            &a.task.id,
            None,
            &types::PatchTaskRequest {
                text: Some("Mine".into()),
                ..Default::default()
            },
        )
        .await,
    );
    nf("complete_task", ops::complete_task(c, &a.task.id, None).await);
    nf("cancel_task", ops::cancel_task(c, &a.task.id, None).await);
    nf("reopen_task", ops::reopen_task(c, &a.task.id, None).await);
    nf("accept_suggestion", ops::accept_suggestion(c, a.suggestion).await);
    nf("reject_suggestion", ops::reject_suggestion(c, a.suggestion).await);
    nf(
        "reply_suggestion",
        ops::reply_suggestion(c, a.suggestion, &types::ReplyRequest { body: "hi".into() }).await,
    );
    let rel = |src, dst| types::RelationRef {
        dst_id: dst,
        src_id: src,
        type_: "related".into(),
    };
    nf("relation to foreign", ops::add_relation(c, &rel(b.note.id, an)).await);
    nf("relation from foreign", ops::add_relation(c, &rel(an, b.note.id)).await);
    nf("remove foreign relation", ops::remove_relation(c, &rel(an, b.note.id)).await);
    nf(
        "retype foreign relation",
        ops::retype_relation(
            c,
            &types::RetypeRelation {
                dst_id: b.note.id,
                src_id: an,
                type_: "related".into(),
                new_type: "supports".into(),
            },
        )
        .await,
    );
    // Filters naming a foreign ID: like a nonexistent one.
    nf(
        "tasks of a foreign entity",
        ops::list_tasks(c, None, Some(a.person.id), None).await,
    );
    nf("tasks of a foreign note", ops::list_tasks(c, None, None, Some(an)).await);
    assert_eq!(
        ops::list_documents(c, None, Some(a.place.place.id), None, None, None)
            .await
            .expect("docs")
            .items,
        vec![]
    );

    // Alice's vault is untouched by all of it.
    assert_eq!(
        ops::get_note(&alice.client, an).await.expect("hers").version,
        a.note.version
    );
    assert_eq!(
        ops::list_tasks(&alice.client, None, None, None)
            .await
            .expect("tasks")
            .items
            .len(),
        1,
        "alice still has her task"
    );
    h.finish().await;
}
