//! Screen view-models the other suites leave out, built headlessly from a pulled vault (fake
//! server, fake clock, deterministic ULIDs): the task screen of a recurring task (with its
//! done occurrence, reminder and preview), a missing task, task homes, the Recent filters,
//! Home's open items, and editor completions for block references and tags.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::sync::Arc;

use chrono::NaiveDate;
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::session::Session;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::extra;
use strata_core::view::model::{
    CompletionKind, RecentFilter, TaskHomeItem, TaskHomesView, TaskScreen, TaskState,
};

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const TASKS: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";
const PERSON: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1D";
const COMPANY: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1E";

/// A pulled vault: a note with a recurring task (one done occurrence) and a reminder, the
/// task home with an open task, a person with open items, a company.
async fn vault(h: &Harness) -> Arc<Session> {
    h.server.remote_upsert(
        NOTE,
        "notes/Rent.md",
        &format!(
            "---\nid: {NOTE}\ntags: [home]\n---\n# Rent\n\nPaid by transfer. ^how\n\n\
             - [ ] Pay rent (@2026-10-01 09:00) 🔁 every month 📅 2026-10-01 ^t-rent\n\
             - [x] Pay rent 🔁 every month 📅 2026-09-01 ✅ 2026-09-01 ^t-rent-sep\n\
             - [ ] Call the landlord #home 📅 2026-09-29 ^t-call\n"
        ),
    );
    h.server.remote_upsert(
        TASKS,
        "tasks/Tasks.md",
        &format!("---\nid: {TASKS}\n---\n- [ ] Buy milk ^t-milk\n"),
    );
    h.server.remote_upsert(
        PERSON,
        "people/Ahmed Fathy.md",
        &format!(
            "---\nid: {PERSON}\nkind: person\n---\n## Open items\n\n\
             - Send him the contract [[Rent#^how]]\n- Ask about the car\n"
        ),
    );
    h.server.remote_upsert(
        COMPANY,
        "companies/Acme.md",
        &format!("---\nid: {COMPANY}\nkind: company\n---\n"),
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    s
}

/// `(label, detail, insert_text, target_id)` of a completion item.
type Row = (String, String, String, Option<String>);
/// Content, cursor, `(kind, replace_start, replace_end, query)` and the items.
type Case = (
    &'static str,
    u32,
    (CompletionKind, u32, u32, &'static str),
    Vec<Row>,
);

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
}

#[tokio::test]
async fn the_task_screen_of_a_recurring_task() {
    let h = Harness::new();
    let s = vault(&h).await;
    let screen = s
        .read(|c, ctx| build::task_screen(c, ctx, "t-rent"))
        .expect("screen");
    let task = screen.task.clone().expect("task");
    assert_eq!(
        (
            task.description.as_str(),
            task.state,
            task.due,
            task.due_label.as_deref(),
            task.next_in_label.as_deref(),
            task.recurrence.as_deref(),
            task.recurrence_understood,
            task.line_number,
        ),
        (
            "Pay rent",
            TaskState::Open,
            Some(d("2026-10-01")),
            Some("Thu 1 Oct"),
            Some("next in 4 days"),
            Some("every month"),
            true,
            9
        )
    );
    // 09:00 in Cairo is 06:00 UTC.
    assert_eq!(
        task.reminders
            .iter()
            .map(|r| (
                r.local.as_str(),
                r.at.to_rfc3339(),
                r.time_label.as_str(),
                r.offset_label.as_str()
            ))
            .collect::<Vec<_>>(),
        [(
            "2026-10-01 09:00",
            "2026-10-01T06:00:00+00:00".to_owned(),
            "09:00",
            "on the day"
        )]
    );
    assert_eq!(
        screen.line,
        "- [ ] Pay rent (@2026-10-01 09:00) 🔁 every month 📅 2026-10-01 ^t-rent"
    );
    assert_eq!(
        (
            screen.location_label.as_str(),
            screen.next_occurrence_label.as_deref(),
            screen.delivery_label.as_deref()
        ),
        (
            "notes/Rent.md · line 9",
            Some("Next occurrence in 4 days"),
            None
        )
    );
    // The done occurrence is its history.
    assert_eq!(
        screen
            .history
            .iter()
            .map(|t| (
                t.id.as_str(),
                t.state,
                t.done,
                t.completion_label.as_deref()
            ))
            .collect::<Vec<_>>(),
        [(
            "t-rent-sep",
            TaskState::Done,
            Some(d("2026-09-01")),
            Some("on time")
        )]
    );
    assert_eq!(
        screen
            .recurrence_preview
            .iter()
            .map(|p| (p.date, p.label.as_str(), p.is_due))
            .collect::<Vec<_>>(),
        [
            (d("2026-10-01"), "Thu 1 Oct 2026", true),
            (d("2026-11-01"), "Sun 1 Nov", false),
            (d("2026-12-01"), "Tue 1 Dec", false),
        ]
    );

    assert_eq!(
        s.read(|c, ctx| build::task_screen(c, ctx, "t-none"))
            .expect("screen"),
        TaskScreen {
            id: "t-none".into(),
            task: None,
            line: String::new(),
            history: Vec::new(),
            location_label: String::new(),
            delivery_label: None,
            next_occurrence_label: None,
            recurrence_form: None,
            recurrence_preview: Vec::new(),
        }
    );
    assert_eq!(
        s.read(extra::task_homes).expect("homes"),
        TaskHomesView {
            homes: vec![
                TaskHomeItem {
                    note_id: Some(TASKS.into()),
                    title: "Tasks".into(),
                    path: "tasks/Tasks.md".into(),
                    is_default: true,
                    open_tasks: 1,
                },
                TaskHomeItem {
                    note_id: Some(NOTE.into()),
                    title: "Rent".into(),
                    path: "notes/Rent.md".into(),
                    is_default: false,
                    open_tasks: 2,
                },
            ],
        }
    );
}

#[tokio::test]
async fn recent_filters_and_home_open_items() {
    let h = Harness::new();
    let s = vault(&h).await;
    let titles = |f: RecentFilter| {
        s.read(|c, ctx| build::recent(c, ctx, f))
            .expect("recent")
            .notes
            .into_iter()
            .map(|n| n.title)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(RecentFilter::Edited),
        ["Acme", "Ahmed Fathy", "Tasks", "Rent"]
    );
    assert_eq!(
        titles(RecentFilter::Created),
        ["Acme", "Ahmed Fathy", "Tasks", "Rent"]
    );
    assert_eq!(titles(RecentFilter::FiledByAi), Vec::<String>::new());

    let home = s.read(build::home).expect("home");
    assert_eq!(
        (
            home.today_label.as_str(),
            home.inbox_count,
            home.needs_you_count,
            home.contradictions_count,
            home.inbox_summary.as_str()
        ),
        ("Sunday 27 September", 0, 0, 0, "")
    );
}

#[tokio::test]
async fn completions_for_block_references_and_tags() {
    let h = Harness::new();
    let s = vault(&h).await;
    let all_blocks = [
        ("Paid by transfer.", "^how"),
        (
            "[ ] Pay rent (@2026-10-01 09:00) 🔁 every month 📅 2026-10-01",
            "^t-rent",
        ),
        (
            "[x] Pay rent 🔁 every month 📅 2026-09-01 ✅ 2026-09-01",
            "^t-rent-sep",
        ),
        ("[ ] Call the landlord #home 📅 2026-09-29", "^t-call"),
    ];
    let with = |insert: &dyn Fn(&str) -> String, blocks: &[(&str, &str)]| {
        blocks
            .iter()
            .map(|(label, detail)| {
                (
                    (*label).to_owned(),
                    (*detail).to_owned(),
                    insert(detail.trim_start_matches('^')),
                    Some(NOTE.to_owned()),
                )
            })
            .collect::<Vec<_>>()
    };
    let closing = |id: &str| format!("{id}]]");
    let bare = |id: &str| id.to_owned();
    let cases: Vec<Case> = vec![
        (
            "See [[Rent#^",
            12,
            (CompletionKind::BlockRef, 12, 12, ""),
            with(&closing, &all_blocks),
        ),
        // Every block whose text or ID contains the query.
        (
            "See [[Rent#^h",
            13,
            (CompletionKind::BlockRef, 12, 13, "h"),
            with(&closing, &all_blocks),
        ),
        // Inside a closed link: no second `]]`.
        (
            "See [[Rent#^ho]] x",
            14,
            (CompletionKind::BlockRef, 12, 14, "ho"),
            with(&bare, &[all_blocks[0], all_blocks[3]]),
        ),
        (
            "See [[Nowhere#^",
            15,
            (CompletionKind::BlockRef, 15, 15, ""),
            Vec::new(),
        ),
        (
            "Tag #ho",
            7,
            (CompletionKind::Tag, 5, 7, "ho"),
            vec![("#home".into(), "1 note".into(), "home".into(), None)],
        ),
        (
            "plain text",
            5,
            (CompletionKind::None, 5, 5, ""),
            Vec::new(),
        ),
    ];
    for (content, cursor, (kind, start, end, query), items) in cases {
        let c = s
            .read(|c, ctx| extra::completions(c, ctx, NOTE, content, cursor))
            .expect("completions");
        assert_eq!(
            (
                c.kind,
                c.replace_start,
                c.replace_end,
                c.query.as_str(),
                c.items
                    .into_iter()
                    .map(|i| (i.label, i.detail, i.insert_text, i.target_id))
                    .collect::<Vec<_>>()
            ),
            (kind, start, end, query, items),
            "{content:?}"
        );
    }
}
