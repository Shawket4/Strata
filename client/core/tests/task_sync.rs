//! Task edits, relations and entity creates round-trip through a sync (PLAN §12.3, L16): the
//! device applies each op locally, the server applies the pushed op with the same shared
//! rules, and afterwards both hold the same bytes; a task the server no longer has is
//! refused and rolled back on the device. Against the fake server.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::session::{NewTask, Session, TaskEdit};
use strata_core::store::{notes, outbox};
use strata_core::sync::engine::Trigger;
use strata_core::view::build;

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const MONA: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";

fn local(s: &Session) -> BTreeMap<String, (String, String)> {
    s.read(|c, _| {
        let mut out = BTreeMap::new();
        for (id, path) in notes::live_paths(c)? {
            let content = notes::current(c, &id)?.expect("current").content;
            out.insert(id, (path, content));
        }
        Ok(out)
    })
    .expect("local notes")
}

fn server(h: &Harness) -> BTreeMap<String, (String, String)> {
    h.server
        .notes()
        .into_iter()
        .map(|(id, n)| (id.to_string(), (n.path, n.content)))
        .collect()
}

#[tokio::test]
async fn task_edits_relations_and_creates_converge_after_a_sync() {
    let h = Harness::new();
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing.md",
        &format!(
            "---\nid: {NOTE}\n---\nQ4.\n- [ ] Send the offer ^t1\n- [ ] Call Mona ^t2\n- [x] Old ✅ 2026-09-01 ^t3\n"
        ),
    );
    h.server.remote_upsert(
        MONA,
        "people/Mona Adel.md",
        &format!("---\nid: {MONA}\nkind: person\n---\n"),
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");

    s.update_task(
        "t1",
        &TaskEdit {
            text: Some("Send the revised offer".into()),
            due: Some(NaiveDate::from_ymd_opt(2026, 10, 2)),
            ..TaskEdit::default()
        },
    )
    .expect("edit");
    s.complete_task("t2").expect("complete");
    s.reopen_task("t3").expect("reopen");
    s.add_relation(NOTE, MONA, "people").expect("relation");
    // A task without a note goes to the task home, which the device creates.
    let home_task = s
        .create_task(
            &NewTask {
                note_id: None,
                description: "Renew the lease".into(),
                due: NaiveDate::from_ymd_opt(2026, 11, 1),
                scheduled: None,
                recurrence: None,
                reminders: Vec::new(),
                priority: None,
            },
            true,
        )
        .expect("home task");
    let in_note = s
        .create_task(
            &NewTask {
                note_id: Some(NOTE.into()),
                description: "Ask [[Mona Adel]] for the numbers".into(),
                due: None,
                scheduled: None,
                recurrence: None,
                reminders: Vec::new(),
                priority: None,
            },
            true,
        )
        .expect("note task");
    // A second person with the same name gets the next free file name, as on the server.
    let twin = s
        .create_entity(domain::NoteKind::Person, "Mona Adel", &[], true)
        .expect("twin")
        .id
        .expect("created");
    let before_push = local(&s);
    assert_eq!(before_push[&twin].0, "people/Mona Adel 2.md");

    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report.pushed, 7);
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), []);
    assert_eq!(local(&s), server(&h), "device and server hold the same bytes");
    assert_eq!(local(&s), before_push, "the server wrote what the device showed");

    let pricing = &server(&h)[NOTE].1;
    assert_eq!(
        pricing,
        &format!(
            "---\nid: {NOTE}\ncreated: 2026-09-27T10:00:00Z\npeople: [\"[[Mona Adel]]\"]\n---\nQ4.\n- [ ] Send the revised offer 📅 2026-10-02 ^t1\n- [x] Call Mona ✅ 2026-09-27 ^t2\n- [ ] Old ^t3\n- [ ] Ask [[Mona Adel]] for the numbers ^{}\n",
            in_note.id.expect("id")
        )
    );
    assert_eq!(server(&h)[&twin].0, "people/Mona Adel 2.md");
    let home = server(&h)
        .into_values()
        .find(|(p, _)| p == "tasks/Tasks.md")
        .expect("task home");
    assert!(
        home.1.contains(&format!(
            "- [ ] Renew the lease 📅 2026-11-01 ^{}\n",
            home_task.id.expect("id")
        )),
        "{}",
        home.1
    );
}

#[tokio::test]
async fn an_edit_of_a_task_the_server_no_longer_has_is_rolled_back() {
    let h = Harness::new();
    let content = format!("---\nid: {NOTE}\n---\n- [ ] Send the offer ^t1\n");
    h.server.remote_upsert(NOTE, "notes/Pricing.md", &content);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let op = s.complete_task("t1").expect("complete");
    // Another device removed the line; this device has not pulled yet.
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing.md",
        &format!("---\nid: {NOTE}\n---\nNo tasks left.\n"),
    );
    // The push is refused (the fake server answers `404` for an unknown task).
    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report.pushed, 1);
    let status = s.read(build::sync_status).expect("status");
    assert_eq!(
        status
            .rejections
            .iter()
            .map(|r| (r.op_id.as_str(), r.kind.as_str(), r.problem_type.as_str()))
            .collect::<Vec<_>>(),
        [(op.as_str(), "task.complete", "not_found")]
    );
    assert_eq!(local(&s), server(&h));
}
