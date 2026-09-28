//! The sync screen's list of unsynced changes (PLAN §11 screen 11, §12.3): every kind of
//! queued op is described by its note and a one-line detail, in English and Arabic, while
//! the device is offline. Against the fake server.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use chrono::NaiveDate;
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::session::TaskEdit;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::model::{CustodyDraft, OutboxStatus};

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const MONA: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";
const MONA2: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1D";
const ACME: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1E";
const DOC: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1F";
const OLD: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1G";

const BODY: &str = "First.\n- [ ] Send the offer ^t1\n- [ ] Call Mona ^t2\n- [ ] Book a room ^t3\n- [ ] Print slides ^t4\n- [ ] Order lunch ^t5\n";

/// `(kind, title, detail)` of every unsynced op after queueing one of each kind.
async fn rows(lang: &str) -> Vec<(String, Option<String>, String)> {
    let h = Harness::new();
    h.accounts
        .update_user("shawket", |m| m.ui_language = lang.to_owned());
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing.md",
        &format!("---\nid: {NOTE}\nrelated: [\"[[Acme]]\"]\n---\n{BODY}"),
    );
    for (id, path, kind) in [
        (MONA, "people/Mona Adel.md", "person"),
        (MONA2, "people/Mona A.md", "person"),
        (ACME, "companies/Acme.md", "company"),
        (DOC, "documents/Car license.md", "document"),
        (OLD, "notes/Old ideas.md", "note"),
    ] {
        h.server
            .remote_upsert(id, path, &format!("---\nid: {id}\nkind: {kind}\n---\n"));
    }
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    h.server.set_offline(true);

    let content = |body: &str| format!("---\nid: {NOTE}\nrelated: [\"[[Acme]]\"]\n---\n{body}");
    s.update_note(NOTE, &content(&BODY.replace("First.", "First, revised.\nSecond.")))
        .expect("update");
    s.add_relation(NOTE, MONA, "people").expect("add");
    s.retype_relation(NOTE, ACME, "related", "part-of")
        .expect("retype");
    s.remove_relation(NOTE, ACME, "part-of").expect("remove");
    s.update_task(
        "t1",
        &TaskEdit {
            text: Some("Send the revised offer".to_owned()),
            ..TaskEdit::default()
        },
    )
    .expect("task text");
    s.update_task(
        "t5",
        &TaskEdit {
            due: Some(NaiveDate::from_ymd_opt(2026, 10, 1)),
            ..TaskEdit::default()
        },
    )
    .expect("task due");
    s.complete_task("t2").expect("complete");
    s.cancel_task("t3").expect("cancel");
    s.delete_task("t4").expect("delete task");
    s.merge_entities(MONA2, MONA).expect("merge");
    s.record_custody(
        DOC,
        &CustodyDraft {
            kind: "handed-to".to_owned(),
            place_id: None,
            person_id: Some(MONA.to_owned()),
            counterparty_id: None,
            date: NaiveDate::from_ymd_opt(2026, 9, 26),
            note: None,
        },
    )
    .expect("custody");
    s.move_note(OLD, "archive/Old ideas.md").expect("move");
    s.delete_note(OLD).expect("delete note");
    s.capture("Call the landlord about the lease\nand the parking spot")
        .expect("capture");

    let status = s.read(build::sync_status).expect("status");
    assert!(status.outbox.iter().all(|o| o.status == OutboxStatus::Pending));
    status
        .outbox
        .into_iter()
        .map(|o| (o.kind, o.title, o.detail))
        .collect()
}

fn row(kind: &str, title: Option<&str>, detail: &str) -> (String, Option<String>, String) {
    (kind.to_owned(), title.map(str::to_owned), detail.to_owned())
}

#[tokio::test]
async fn every_queued_change_is_described_in_english() {
    let pricing = Some("Pricing");
    assert_eq!(
        rows("en").await,
        [
            row("note.update", pricing, "1 line changed, 1 line added"),
            row("relation.add", pricing, "People · Mona Adel"),
            row(
                "relation.retype",
                pricing,
                "Changed from Related to Part of · Acme"
            ),
            row("relation.remove", pricing, "Removed Part of · Acme"),
            row("task.update", pricing, "Send the revised offer"),
            row("task.update", pricing, "Order lunch"),
            row("task.complete", pricing, "✓ Call Mona"),
            row("task.cancel", pricing, "✕ Book a room"),
            row("task.delete", pricing, "Print slides"),
            row("entity.merge", Some("Mona A"), "Merged into Mona Adel"),
            row("document.custody", Some("Car license"), "handed-to"),
            row("note.move", Some("Old ideas"), "Moved to archive/Old ideas.md"),
            row("note.delete", None, "Deleted"),
            row("capture", None, "Call the landlord about the lease"),
        ]
    );
}

#[tokio::test]
async fn every_queued_change_is_described_in_arabic() {
    let details: Vec<String> = rows("ar").await.into_iter().map(|r| r.2).collect();
    assert_eq!(
        details,
        [
            "تغيّر سطر واحد، أُضيف سطر واحد",
            "الأشخاص · Mona Adel",
            "تغيّرت من مرتبطة إلى جزء من · Acme",
            "أُزيلت جزء من · Acme",
            "Send the revised offer",
            "Order lunch",
            "✓ Call Mona",
            "✕ Book a room",
            "Print slides",
            "دُمج في Mona Adel",
            "handed-to",
            "نُقلت إلى archive/Old ideas.md",
            "حذف",
            "Call the landlord about the lease",
        ]
    );
}
