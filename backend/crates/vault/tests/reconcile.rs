//! Reconciliation, verify and reindex (PLAN §7.3, §16.3): crash leftovers, out-of-band
//! edits, uncommitted state, sidecar drift, and full rebuild = incremental state.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

mod common;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use common::World;
use domain::{NoteKind, RelationType};
use pretty_assertions::assert_eq;
use strata_vault::ops::entities::{NewCustodyEvent, NewEntity};
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::relations::AiEdge;
use strata_vault::ops::tasks::{NewTask, Transition};
use strata_vault::reconcile::Report;
use vault_format::RelationKey;
use vault_format::custody::CustodyEventType;
use vault_format::tasks::Reminder;

fn note(path: &str, content: &str) -> CreateNote {
    CreateNote {
        path: path.to_owned(),
        content: content.to_owned(),
        id: None,
        force: false,
    }
}

async fn warnings(w: &World, s: &strata_index::UserScope) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = w
        .vault
        .integrity(s, 100)
        .await
        .expect("warnings")
        .into_iter()
        .map(|w| (w.kind, w.path))
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn crash_leftovers_never_become_notes() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    w.vault
        .create_note(&s, note("notes/A.md", "a\n"))
        .await
        .expect("a");
    // A crash between write and rename leaves a temporary file holding a whole note.
    let tmp = w.dir(u).join("notes/.strata-tmp-4242-7");
    std::fs::write(
        &tmp,
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n---\nhalf-written\n",
    )
    .expect("write");
    let log_before = w.log(u);
    w.vault.evict(u);
    let report = w.vault.verify(&s).await.expect("verify");
    assert_eq!(
        report,
        Report {
            temp_files_removed: vec!["notes/.strata-tmp-4242-7".into()],
            ..Report::default()
        }
    );
    assert!(!tmp.exists());
    assert_eq!(w.log(u), log_before);
    assert_eq!(
        warnings(&w, &s).await,
        vec![(
            "temp_file_removed".into(),
            Some("notes/.strata-tmp-4242-7".into())
        )]
    );
    let id: strata_common::NoteId = "01J8ZK3M4X7Q9W2E5R6T8Y0V1H".parse().expect("id");
    assert!(matches!(
        w.vault.note(&s, id).await,
        Err(strata_vault::VaultError::NotFound)
    ));
    let paths: Vec<String> = w
        .vault
        .tree(&s)
        .await
        .expect("tree")
        .into_iter()
        .filter_map(|e| match e {
            strata_vault::model::TreeEntry::Note { path, .. } => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(paths, vec!["notes/A.md"]);
    w.finish().await;
}

#[tokio::test]
async fn reconciliation_repairs_out_of_band_edits_uncommitted_state_and_sidecar_drift() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let a = w
        .vault
        .create_note(&s, note("notes/A.md", "alpha [[B]]\n"))
        .await
        .expect("a");
    let b = w
        .vault
        .create_note(&s, note("notes/B.md", "beta\n"))
        .await
        .expect("b");
    let dir = w.dir(u);
    // Out of band: edit A, add a note without an id, delete B, and a sidecar whose relation
    // is not in A's frontmatter.
    std::fs::write(dir.join("notes/A.md"), format!("{}zeta line\n", a.content)).expect("edit");
    std::fs::write(dir.join("notes/New.md"), "fresh #idea\n").expect("new");
    std::fs::remove_file(dir.join("notes/B.md")).expect("rm");
    std::fs::create_dir_all(dir.join(".meta/notes")).expect("mkdir");
    let sidecar = format!(".meta/notes/{}.json", a.id);
    std::fs::write(
        dir.join(&sidecar),
        format!(
            "{{\n  \"id\": \"{}\",\n  \"relations\": [\n    {{\n      \"type\": \"related\",\n      \"target_id\": \"{}\",\n      \"by\": \"ai\",\n      \"created\": \"2026-09-27T12:00:00Z\"\n    }}\n  ],\n  \"rejected\": []\n}}\n",
            a.id, b.id
        ),
    )
    .expect("sidecar");
    w.vault.evict(u);
    let report = w.vault.verify(&s).await.expect("verify");
    assert_eq!(
        report,
        Report {
            temp_files_removed: vec![],
            recovered: vec![
                sidecar.clone(),
                "notes/A.md".into(),
                "notes/B.md".into(),
                "notes/New.md".into()
            ],
            ids_assigned: vec!["notes/New.md".into()],
            recovery_commit: report.recovery_commit.clone(),
            sidecars_repaired: vec![sidecar.clone()],
            out_of_band: vec!["notes/A.md".into()],
            missing: vec!["notes/B.md".into()],
            reindexed: 2,
        }
    );
    assert!(report.recovery_commit.is_some());
    assert_eq!(
        w.log(u)[..3],
        [
            "system: repair sidecars".to_owned(),
            "system: recovered changes".to_owned(),
            "user: create notes/B.md".to_owned(),
        ]
    );
    // The sidecar lost the relation the frontmatter does not have, so it is gone.
    assert!(!w.exists(u, &sidecar));
    let new = w
        .vault
        .note_by_path(&s, "notes/New.md")
        .await
        .expect("indexed");
    assert_eq!(
        new.content,
        format!("---\nid: {}\n---\nfresh #idea\n", new.id)
    );
    let hits = w
        .vault
        .search(&s, "zeta", strata_vault::ops::read::SearchMode::Keyword, 10)
        .await
        .expect("search");
    assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![a.id]);
    assert!(matches!(
        w.vault.note(&s, b.id).await,
        Err(strata_vault::VaultError::NotFound)
    ));
    assert_eq!(
        warnings(&w, &s).await,
        vec![
            ("id_assigned".into(), Some("notes/New.md".into())),
            ("missing_file".into(), Some("notes/B.md".into())),
            ("out_of_band_edit".into(), Some("notes/A.md".into())),
            ("sidecar_repaired".into(), Some(sidecar.clone())),
            ("uncommitted_changes".into(), None),
        ]
    );
    // Now clean.
    w.vault.evict(u);
    assert_eq!(w.vault.verify(&s).await.expect("verify"), Report::default());
    w.finish().await;
}

#[tokio::test]
async fn a_full_reindex_equals_the_incremental_state() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let v = &w.vault;
    let office = v
        .create_entity(
            &s,
            NewEntity {
                kind: NoteKind::Place,
                name: "Nasr City office".into(),
                aliases: vec!["مكتب مدينة نصر".into()],
                tags: vec![],
                fields: BTreeMap::new(),
                parent: None,
                id: None,
                force: false,
            },
        )
        .await
        .expect("office");
    let safe = v
        .create_entity(
            &s,
            NewEntity {
                kind: NoteKind::Place,
                name: "Safe — Nasr City office".into(),
                aliases: vec![],
                tags: vec![],
                fields: BTreeMap::new(),
                parent: Some(office.id),
                id: None,
                force: false,
            },
        )
        .await
        .expect("safe");
    let shady = v
        .create_entity(
            &s,
            NewEntity {
                kind: NoteKind::Person,
                name: "Shady".into(),
                aliases: vec!["شادي".into()],
                tags: vec!["client".into()],
                fields: [("role".to_owned(), "Accountant".to_owned())].into(),
                parent: None,
                id: None,
                force: false,
            },
        )
        .await
        .expect("shady");
    let watanya = v
        .create_entity(
            &s,
            NewEntity {
                kind: NoteKind::Company,
                name: "Watanya".into(),
                aliases: vec!["وطنية".into()],
                tags: vec![],
                fields: [("industry".to_owned(), "Fuel".to_owned())].into(),
                parent: None,
                id: None,
                force: false,
            },
        )
        .await
        .expect("watanya");
    let contract = v
        .create_entity(
            &s,
            NewEntity {
                kind: NoteKind::Document,
                name: "Watanya contract".into(),
                aliases: vec!["عقد وطنية".into()],
                tags: vec![],
                fields: [
                    ("doc-type".to_owned(), "contract".to_owned()),
                    ("expires".to_owned(), "2027-03-31".to_owned()),
                ]
                .into(),
                parent: None,
                id: None,
                force: false,
            },
        )
        .await
        .expect("contract");
    let capture = v
        .capture(
            &s,
            "Watanya's contract is at the Nasr City office in the safe, last with Shady".into(),
        )
        .await
        .expect("capture");
    for (kind, day, place, person) in [
        (CustodyEventType::HandedTo, 10, None, Some(shady.id)),
        (
            CustodyEventType::ReturnedBy,
            20,
            Some(safe.id),
            Some(shady.id),
        ),
    ] {
        v.add_custody_event(
            &s,
            contract.id,
            NewCustodyEvent {
                kind,
                date: NaiveDate::from_ymd_opt(2026, 9, day).expect("date"),
                place,
                person,
                counterparty: None,
                source: Some(capture.note.id),
            },
        )
        .await
        .expect("custody");
    }
    let meeting = v
        .create_note(
            &s,
            note(
                "notes/Meeting.md",
                "---\ntags: [pos]\naliases: [sync]\npeople: [\"[[Shady]]\"]\ncompanies: [\"[[Watanya]]\"]\n---\n# Call\n\nPrices up ^p1\n\n- [ ] Send the invoice [[Watanya]] 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00)\n",
            ),
        )
        .await
        .expect("meeting");
    let other = v
        .create_note(
            &s,
            note("notes/Other.md", "links [[Meeting#^p1]] ![[Meeting]]\n"),
        )
        .await
        .expect("other");
    v.ai_add_relations(
        &s,
        "link".into(),
        other.id,
        vec![
            AiEdge {
                rel: RelationKey::Note(RelationType::Supports),
                dst: meeting.id,
                confidence: 0.8,
                reason: "r".into(),
                model: "m".into(),
            },
            AiEdge {
                rel: RelationKey::Note(RelationType::Related),
                dst: safe.id,
                confidence: 0.75,
                reason: "r2".into(),
                model: "m".into(),
            },
        ],
    )
    .await
    .expect("ai");
    v.remove_relation(
        &s,
        other.id,
        safe.id,
        RelationKey::Note(RelationType::Related),
    )
    .await
    .expect("reject");
    let task_id = v
        .create_task(
            &s,
            NewTask {
                text: "Petrol Arrows invoice".into(),
                recurrence: Some("every week on Sunday".into()),
                due: NaiveDate::from_ymd_opt(2026, 9, 27),
                reminders: vec![Reminder {
                    date: NaiveDate::from_ymd_opt(2026, 9, 27).expect("d"),
                    time: None,
                }],
                ..NewTask::default()
            },
        )
        .await
        .expect("task");
    v.transition_task(&s, task_id, Transition::Complete, None)
        .await
        .expect("complete");
    v.create_note(
        &s,
        CreateNote {
            force: true,
            ..note("notes/Sub/Meeting.md", "dup\n")
        },
    )
    .await
    .expect("forced");
    let gone = v
        .create_note(&s, note("notes/Gone.md", "bye [[Meeting]]\n"))
        .await
        .expect("gone");
    v.delete_note(&s, gone.id).await.expect("trash");
    let _ = watanya;

    let incremental = w.snapshot(u).await;
    // Every derived table is populated.
    for (table, rows) in &incremental {
        if !matches!(table.as_str(), "chunks" | "clusters") {
            assert!(!rows.is_empty(), "{table} has no rows");
        }
    }
    let n = v.reindex(&s).await.expect("reindex");
    assert_eq!(n, 11);
    assert_eq!(w.snapshot(u).await, incremental);
    w.finish().await;
}
