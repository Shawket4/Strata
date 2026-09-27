//! Entities, documents and places over HTTP (PLAN §6.6–§6.8, §7.5, §16.3 Entities /
//! Documents): creation in both scripts, the duplicate check (409 payloads, `force`,
//! keep-both never re-flagged), field edits, merge, custody sequences and nested places.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod vault_harness;

use std::collections::HashMap;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use vault_harness::{H, assert_problem, not_found, plain};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("date")
}

fn entity(kind: types::EntityKind, name: &str, aliases: &[&str]) -> types::CreateEntityRequest {
    types::CreateEntityRequest {
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        fields: HashMap::new(),
        force: None,
        id: None,
        kind,
        name: name.to_owned(),
        parent_id: None,
        tags: vec![],
    }
}

fn place(name: &str, parent: Option<ulid::Ulid>) -> types::CreatePlaceRequest {
    types::CreatePlaceRequest {
        address: None,
        aliases: vec![],
        force: None,
        id: None,
        name: name.to_owned(),
        parent_id: parent,
        tags: vec![],
    }
}

fn duplicate(candidates: Vec<types::DuplicateCandidate>) -> types::Problem {
    types::Problem {
        candidates,
        ..plain(
            "duplicate_candidates",
            "Possible duplicate",
            409,
            Some("resend with force = true to create it anyway"),
        )
    }
}

fn exact(id: ulid::Ulid, kind: &str, title: &str) -> types::DuplicateCandidate {
    types::DuplicateCandidate {
        id,
        kind: kind.to_owned(),
        match_level: types::MatchLevel::Exact,
        score: 1.0,
        snippet: None,
        title: title.to_owned(),
    }
}

fn ids(ids: &[ulid::Ulid]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

fn names(list: &types::EntityList) -> Vec<String> {
    list.items.iter().map(|e| e.name.clone()).collect()
}

#[tokio::test]
async fn entities_in_both_scripts_duplicates_force_keep_both_and_merge() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let mut req = entity(types::EntityKind::Person, "Watanya", &["واتانيا"]);
    req.fields.insert("role".into(), "Accountant".into());
    let watanya = ops::create_entity(c, &req).await.expect("create");
    assert_eq!(watanya.path, "people/Watanya.md");
    assert_eq!(watanya.kind, types::EntityKind::Person);
    assert_eq!(watanya.aliases, vec!["واتانيا".to_owned()]);
    assert_eq!(
        h.read(alice.id, "people/Watanya.md"),
        format!(
            "---\nid: {}\nkind: person\naliases: [واتانيا]\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:00:00+00:00\nrole: Accountant\n---\n## Notes\n",
            watanya.id
        )
    );
    assert_eq!(h.log(alice.id)[0], "user: create people/Watanya.md");

    // Found by either script.
    for q in ["واتانيا", "watanya", "WATANYA"] {
        let found = ops::list_entities(c, None, Some(q), None, None)
            .await
            .expect("list");
        assert_eq!(names(&found), vec!["Watanya".to_owned()], "{q}");
    }
    let acme = ops::create_entity(c, &entity(types::EntityKind::Company, "Acme Trading", &[]))
        .await
        .expect("company");
    let people = ops::list_entities(c, Some(&types::EntityKind::Person), None, None, None)
        .await
        .expect("people");
    assert_eq!(names(&people), vec!["Watanya".to_owned()]);

    // Same name in the other script → 409 exact; the file is not written.
    let arabic = entity(types::EntityKind::Person, "واتانيا", &[]);
    assert_problem(
        ops::create_entity(c, &arabic).await,
        &duplicate(vec![exact(watanya.id, "person", "Watanya")]),
    );
    assert_problem(
        ops::create_entity(c, &entity(types::EntityKind::Company, "ACME trading", &[])).await,
        &duplicate(vec![exact(acme.id, "company", "Acme Trading")]),
    );
    assert_eq!(h.log(alice.id).len(), 3, "a refused create writes nothing");

    // Forced: created, the pair remembered, and an alias edit is not re-flagged.
    let forced = ops::create_entity(
        c,
        &types::CreateEntityRequest {
            force: Some(true),
            ..arabic.clone()
        },
    )
    .await
    .expect("forced");
    assert_eq!(forced.path, "people/واتانيا.md");
    let patched = ops::patch_entity(
        c,
        forced.id,
        Some(&forced.version),
        &types::PatchEntityRequest {
            aliases: Some(vec!["Watanya".into(), "W.".into()]),
            ..Default::default()
        },
    )
    .await
    .expect("alias edit is not re-flagged");
    assert_eq!(patched.aliases, vec!["W.".to_owned(), "Watanya".to_owned()]);
    // A third one is flagged against both.
    let third = ops::create_entity(c, &arabic).await.expect_err("dup");
    let mut flagged: Vec<ulid::Ulid> = vault_harness::problem(&third)
        .candidates
        .into_iter()
        .map(|c| c.id)
        .collect();
    flagged.sort();
    let mut both = vec![watanya.id, forced.id];
    both.sort();
    assert_eq!(flagged, both);

    // Field edits: set and unset; stale If-Match → 409 with the current version.
    let edited = ops::patch_entity(
        c,
        watanya.id,
        Some(&watanya.version),
        &types::PatchEntityRequest {
            set_fields: HashMap::from([("email".to_owned(), "w@example.com".to_owned())]),
            unset_fields: vec!["role".into()],
            tags: Some(vec!["client".into()]),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    assert_eq!(
        h.read(alice.id, "people/Watanya.md"),
        format!(
            "---\nid: {}\nkind: person\naliases: [واتانيا]\ntags: [client]\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:00:00+00:00\nemail: w@example.com\n---\n## Notes\n",
            watanya.id
        )
    );
    let stale = ops::patch_entity(
        c,
        watanya.id,
        Some(&watanya.version),
        &types::PatchEntityRequest {
            name: Some("Wat".into()),
            ..Default::default()
        },
    )
    .await
    .expect_err("stale");
    let p = vault_harness::problem(&stale);
    assert_eq!(p.type_, "version_conflict");
    assert_eq!(p.current_version.as_deref(), Some(edited.version.as_str()));
    let unknown = ops::patch_entity(
        c,
        acme.id,
        None,
        &types::PatchEntityRequest {
            set_fields: HashMap::from([("phone".to_owned(), "1".to_owned())]),
            ..Default::default()
        },
    )
    .await
    .expect_err("phone is not a company field");
    assert_eq!(vault_harness::problem(&unknown).status, 422);

    // A note mentions the forced duplicate; merging it into Watanya rewrites the link.
    let note = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "Met [[واتانيا]] about the invoice.\n".into(),
            force: None,
            id: None,
            path: "notes/Meeting.md".into(),
        },
    )
    .await
    .expect("note");
    let mentions = ops::get_entity_notes(c, forced.id).await.expect("mentions");
    assert_eq!(
        mentions.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![note.id]
    );
    let merged = ops::merge_entity(
        c,
        forced.id,
        &types::MergeRequest {
            into_id: watanya.id,
        },
    )
    .await
    .expect("merge");
    assert_eq!(merged.id, watanya.id);
    assert_eq!(merged.aliases, vec!["W.".to_owned(), "واتانيا".to_owned()]);
    assert_eq!(
        h.log(alice.id)[0],
        "user: merge people/واتانيا.md -> people/Watanya.md"
    );
    let meeting = ops::get_note(c, note.id).await.expect("note");
    assert!(
        meeting
            .content
            .ends_with("Met [[Watanya]] about the invoice.\n"),
        "{}",
        meeting.content
    );
    assert_problem(ops::get_entity(c, forced.id).await, &not_found());
    let mentions = ops::get_entity_notes(c, watanya.id)
        .await
        .expect("mentions");
    assert_eq!(
        mentions.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![note.id]
    );
    // Merging across kinds is refused.
    let refused = ops::merge_entity(
        c,
        acme.id,
        &types::MergeRequest {
            into_id: watanya.id,
        },
    )
    .await
    .expect_err("kinds differ");
    assert_eq!(vault_harness::problem(&refused).status, 422);
    h.finish().await;
}

#[tokio::test]
async fn custody_sequences_nested_places_and_document_duplicates() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let home = ops::create_place(c, &place("Home", None))
        .await
        .expect("home");
    let study = ops::create_place(c, &place("Study", Some(home.place.id)))
        .await
        .expect("study");
    let safe = ops::create_place(c, &place("Safe", Some(study.place.id)))
        .await
        .expect("safe");
    let office = ops::create_place(c, &place("Office", None))
        .await
        .expect("office");
    assert_eq!(safe.ancestors, ids(&[study.place.id, home.place.id]));
    let home_view = ops::get_place(c, home.place.id).await.expect("home");
    assert_eq!(home_view.children, ids(&[study.place.id]));
    // A place nested in a person is refused; a place duplicate is refused unless forced.
    assert_problem(
        ops::create_place(c, &place("safe", None)).await,
        &duplicate(vec![exact(safe.place.id, "place", "Safe")]),
    );
    let watanya = ops::create_entity(c, &entity(types::EntityKind::Person, "Watanya", &[]))
        .await
        .expect("person");
    let bank = ops::create_entity(c, &entity(types::EntityKind::Company, "Bank Misr", &[]))
        .await
        .expect("company");

    let new_doc = |name: &str| types::CreateDocumentRequest {
        aliases: vec![],
        copy: Some(types::CopyKind::Original),
        doc_type: Some("passport".into()),
        expires: Some(d(2027, 1, 15)),
        force: None,
        id: None,
        name: name.to_owned(),
        tags: vec![],
    };
    let passport = ops::create_document(c, &new_doc("Passport"))
        .await
        .expect("doc");
    assert_eq!(passport.document.path, "documents/Passport.md");
    assert_eq!(passport.document.status, types::DocumentStatus::Stored);
    assert_eq!(passport.document.location_id, None);
    assert_problem(
        ops::create_document(c, &new_doc("passport")).await,
        &duplicate(vec![exact(passport.document.id, "document", "Passport")]),
    );
    let deed = ops::create_document(
        c,
        &types::CreateDocumentRequest {
            doc_type: Some("deed".into()),
            expires: None,
            ..new_doc("House deed")
        },
    )
    .await
    .expect("deed");

    let event = |kind, at, place: Option<ulid::Ulid>, person: Option<ulid::Ulid>, cp| {
        types::CustodyEventRequest {
            at,
            counterparty_id: cp,
            person_id: person,
            place_id: place,
            source_note_id: None,
            type_: kind,
        }
    };
    let pid = passport.document.id;
    // stored-at Safe: stored, in the safe; found recursively under Home.
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::StoredAt,
            d(2026, 1, 10),
            Some(safe.place.id),
            None,
            None,
        ),
    )
    .await
    .expect("stored");
    assert_eq!(doc.document.status, types::DocumentStatus::Stored);
    assert_eq!(doc.document.location_id, Some(safe.place.id));
    assert_eq!(doc.document.holder_id, None);
    assert_eq!(
        doc.location_path,
        ids(&[home.place.id, study.place.id, safe.place.id])
    );
    assert_eq!(doc.custody.len(), 1);
    assert_eq!(doc.custody[0].by, "user");
    assert_eq!(h.log(alice.id)[0], "user: custody documents/Passport.md");
    let under_home = ops::list_documents(c, None, Some(home.place.id), None, None, None)
        .await
        .expect("under home");
    assert_eq!(
        under_home.items.iter().map(|d| d.id).collect::<Vec<_>>(),
        vec![pid]
    );
    let home_view = ops::get_place(c, home.place.id).await.expect("home");
    assert_eq!(home_view.documents, ids(&[pid]));
    let in_office = ops::list_documents(c, None, Some(office.place.id), None, None, None)
        .await
        .expect("office");
    assert_eq!(in_office.items, vec![]);

    // handed-to Watanya: checked out, held by her.
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::HandedTo,
            d(2026, 3, 1),
            None,
            Some(watanya.id),
            None,
        ),
    )
    .await
    .expect("handed");
    assert_eq!(doc.document.status, types::DocumentStatus::CheckedOut);
    assert_eq!(doc.document.holder_id, Some(watanya.id));
    let held = ops::get_entity_documents(c, watanya.id)
        .await
        .expect("held");
    assert_eq!(held.holds, ids(&[pid]));
    let by_holder = ops::list_documents(c, None, None, Some(watanya.id), None, None)
        .await
        .expect("by holder");
    assert_eq!(by_holder.items.len(), 1);
    assert_eq!(
        ops::list_documents(c, None, Some(home.place.id), None, None, None)
            .await
            .expect("home")
            .items,
        vec![]
    );

    // returned-by Watanya into the safe: stored, she is the last holder.
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::ReturnedBy,
            d(2026, 3, 15),
            Some(safe.place.id),
            Some(watanya.id),
            None,
        ),
    )
    .await
    .expect("returned");
    assert_eq!(doc.document.status, types::DocumentStatus::Stored);
    assert_eq!(doc.document.holder_id, None);
    assert_eq!(doc.document.last_holder_id, Some(watanya.id));
    assert_eq!(doc.document.location_id, Some(safe.place.id));
    let held = ops::get_entity_documents(c, watanya.id)
        .await
        .expect("held");
    assert_eq!(held.holds, Vec::<String>::new());
    assert_eq!(held.last_handled, ids(&[pid]));
    // sent-to the bank: with a third party, the bank holds it.
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::SentTo,
            d(2026, 4, 2),
            None,
            None,
            Some(bank.id),
        ),
    )
    .await
    .expect("sent");
    assert_eq!(doc.document.status, types::DocumentStatus::WithThirdParty);
    assert_eq!(doc.document.holder_id, Some(bank.id));
    assert_eq!(doc.document.location_id, None);
    let with_bank = ops::get_entity_documents(c, bank.id).await.expect("bank");
    assert_eq!(with_bank.holds, ids(&[pid]));
    assert_eq!(
        ops::get_entity_documents(c, watanya.id)
            .await
            .expect("w")
            .last_handled,
        Vec::<String>::new()
    );
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::StoredAt,
            d(2026, 5, 20),
            Some(office.place.id),
            None,
            None,
        ),
    )
    .await
    .expect("stored again");
    assert_eq!(doc.document.status, types::DocumentStatus::Stored);
    assert_eq!(doc.document.location_id, Some(office.place.id));
    assert_eq!(doc.custody.len(), 5);
    // An out-of-order (older) event is recorded but the state follows the newest.
    let doc = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::Lost,
            d(2025, 12, 1),
            None,
            None,
            None,
        ),
    )
    .await
    .expect("older");
    assert_eq!(doc.document.status, types::DocumentStatus::Stored);
    assert_eq!(doc.document.location_id, Some(office.place.id));
    assert_eq!(doc.custody.len(), 6);
    // An event with a source note cites it; events recorded without one carry no citation
    // (`by: user`), rather than an invented one.
    let call = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "Found the passport at the office.\n".into(),
            force: None,
            id: None,
            path: "notes/Call.md".into(),
        },
    )
    .await
    .expect("call note");
    let doc = ops::add_custody_event(
        c,
        pid,
        &types::CustodyEventRequest {
            source_note_id: Some(call.id),
            ..event(
                types::CustodyEventKind::Found,
                d(2026, 5, 21),
                Some(office.place.id),
                None,
                None,
            )
        },
    )
    .await
    .expect("found");
    assert_eq!(
        doc.custody
            .iter()
            .map(|e| (e.by.as_str(), e.source_note_id))
            .collect::<Vec<_>>(),
        vec![
            ("user", Some(call.id)),
            ("user", None),
            ("user", None),
            ("user", None),
            ("user", None),
            ("user", None),
            ("user", None),
        ]
    );
    let text = h.read(alice.id, "documents/Passport.md");
    let custody = text
        .split("## Custody\n")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("custody section");
    assert_eq!(
        custody,
        "- 2026-05-21 — found at [[Office]] — [[Call]]\n\
         - 2026-05-20 — stored-at [[Office]]\n\
         - 2026-04-02 — sent-to [[Bank Misr]]\n\
         - 2026-03-15 — returned-by [[Watanya]] to [[Safe]]\n\
         - 2026-03-01 — handed-to [[Watanya]]\n\
         - 2026-01-10 — stored-at [[Safe]]\n\
         - 2025-12-01 — lost\n"
    );

    // Wrong-kind arguments are refused.
    let wrong = ops::add_custody_event(
        c,
        pid,
        &event(
            types::CustodyEventKind::StoredAt,
            d(2026, 6, 1),
            Some(watanya.id),
            None,
            None,
        ),
    )
    .await
    .expect_err("a person is not a place");
    assert_eq!(vault_harness::problem(&wrong).status, 422);

    // Expiry filter and status filter.
    let expiring = ops::list_documents(c, None, None, None, None, Some("2027-02-01"))
        .await
        .expect("expiring");
    assert_eq!(
        expiring.items.iter().map(|d| d.id).collect::<Vec<_>>(),
        vec![pid]
    );
    let stored = ops::list_documents(
        c,
        None,
        None,
        None,
        Some(&types::DocumentStatus::Stored),
        None,
    )
    .await
    .expect("stored");
    let mut got: Vec<_> = stored.items.iter().map(|d| d.id).collect();
    got.sort();
    let mut want = vec![pid, deed.document.id];
    want.sort();
    assert_eq!(got, want);

    // Nesting follows a moved place: Safe moves to the office.
    ops::patch_place(
        c,
        safe.place.id,
        None,
        &types::PatchPlaceRequest {
            parent_id: Some(office.place.id),
            ..Default::default()
        },
    )
    .await
    .expect("move safe");
    let safe_view = ops::get_place(c, safe.place.id).await.expect("safe");
    assert_eq!(safe_view.ancestors, ids(&[office.place.id]));
    let office_view = ops::get_place(c, office.place.id).await.expect("office");
    assert_eq!(office_view.children, ids(&[safe.place.id]));
    let study_view = ops::get_place(c, study.place.id).await.expect("study");
    assert_eq!(study_view.children, Vec::<String>::new());
    // A cycle is refused.
    let cycle = ops::patch_place(
        c,
        office.place.id,
        None,
        &types::PatchPlaceRequest {
            parent_id: Some(safe.place.id),
            ..Default::default()
        },
    )
    .await
    .expect_err("cycle");
    assert_eq!(vault_harness::problem(&cycle).status, 422);
    h.finish().await;
}
