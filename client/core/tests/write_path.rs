//! The write path (PLAN §12.3): each intent applies optimistically, queues exactly one outbox
//! op (sync-model types, client-generated IDs, base versions) and re-emits the view-models,
//! asserted as full values and full emission sequences.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use common::{Harness, seq_id};
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::session::{NewTask, Session};
use strata_core::store::outbox::{self, OpStatus, OutboxOp};
use strata_core::sync::model::{Op, Version, ops};
use strata_core::view::hub::Recorder;
use strata_core::view::model::{
    Availability, Connectivity, CreateOutcome, HomeView, InboxFilter, InboxPreviewItem,
    NoteListItem, NoteSyncKind, NoteSyncState, SyncActivity, SyncPill, SyncPillKind, TaskItem,
    TaskSections, TaskState, TextDir,
};
use strata_core::view::{Topics, build};
use ulid::Ulid;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(common::NOW)
        .expect("now")
        .with_timezone(&Utc)
}

fn outbox(s: &Session) -> Vec<OutboxOp> {
    s.read(|c, _| outbox::all(c)).expect("outbox")
}

fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("ulid")
}

fn pill(pending: u32) -> SyncPill {
    SyncPill {
        connectivity: Connectivity::Unknown,
        activity: SyncActivity::default(),
        pending_ops: pending,
        conflicts: 0,
        duplicates: 0,
        last_sync_at: None,
        display: SyncPillKind::Synced,
        progress_done: 0,
        progress_total: 0,
        last_sync_label: None,
        label: if pending == 0 {
            "Synced".to_owned()
        } else {
            format!("Synced · {pending} queued")
        },
    }
}

fn content(s: &Session, id: &str) -> String {
    s.read(|c, _| {
        Ok(strata_core::store::notes::current(c, id)?
            .expect("note")
            .content)
    })
    .expect("read")
}

#[tokio::test]
async fn capture_queues_one_op_and_updates_home() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let home = Recorder::new();
    s.watch(Topics::ALL, build::home, home.clone())
        .expect("watch");

    let id = s.capture("كلمت أحمد النهارده").expect("capture");

    assert_eq!(id, seq_id(1));
    let created = DateTime::parse_from_rfc3339("2026-09-27T13:00:00+03:00").expect("ts");
    assert_eq!(
        outbox(&s),
        vec![OutboxOp {
            op_id: seq_id(2),
            ord: 1,
            entity_id: seq_id(1),
            local_entity: format!("note:{}", seq_id(1)),
            base_version: None,
            op: Op::Capture(ops::Capture {
                id: ulid(&seq_id(1)),
                text: "كلمت أحمد النهارده".into(),
                created,
            }),
            status: OpStatus::Pending,
            attempts: 0,
            created: "2026-09-27T10:00:00+00:00".into(),
            base_content: None,
        }]
    );
    assert_eq!(
        content(&s, &id),
        format!(
            "---\nid: {}\ncreated: 2026-09-27T13:00:00+03:00\n---\nكلمت أحمد النهارده",
            seq_id(1)
        )
    );
    let empty = HomeView {
        recent_notes: Vec::new(),
        inbox_count: 0,
        tasks: TaskSections::default(),
        sync: pill(0),
        // 13:00 in Cairo on Sunday 27 September.
        today_label: "Sunday 27 September".into(),
        greeting: "Good afternoon, shawket".into(),
        display_name: "shawket".into(),
        inbox_preview: Vec::new(),
        needs_you_count: 0,
        contradictions_count: 0,
        inbox_summary: String::new(),
        ai_activity: Availability::NotYetAvailable,
        ai_activity_items: Vec::new(),
        ai_activity_headline: String::new(),
        open_items: Availability::Available,
        open_item_list: Vec::new(),
        pinned: Vec::new(),
    };
    assert_eq!(
        home.take(),
        vec![
            empty.clone(),
            HomeView {
                inbox_count: 1,
                sync: pill(1),
                inbox_preview: vec![InboxPreviewItem {
                    note_id: id.clone(),
                    text: "كلمت أحمد النهارده".into(),
                    text_dir: TextDir::Rtl,
                    summary: String::new(),
                    needs_you: false,
                }],
                ..empty
            }
        ]
    );
    // The inbox shows it immediately, marked as not synced yet.
    let inbox = s
        .read(|c, ctx| build::inbox(c, ctx, InboxFilter::All))
        .expect("inbox");
    assert_eq!(inbox.captures.len(), 1);
    assert_eq!(inbox.captures[0].note_id, id);
    assert_eq!(inbox.captures[0].title, "2026-09-27-130000");
    assert_eq!(inbox.captures[0].text, "كلمت أحمد النهارده");
    assert!(inbox.captures[0].pending_sync);
    // Captures are never refused, even if empty-looking duplicates exist.
    assert_eq!(
        s.capture("  "),
        Err(CoreError::InvalidInput {
            field: "text".into(),
            reason: "empty".into()
        })
    );
    assert_eq!(outbox(&s).len(), 1);
}

#[tokio::test]
async fn note_create_update_move_delete_ops_and_note_view() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let created = s
        .create_note(
            "notes/sales/Pricing experiments.md",
            "# Pricing\nTry 5% off #pricing [[Churn notes]]\n",
            false,
        )
        .expect("create");
    let id = seq_id(1);
    assert_eq!(
        created,
        CreateOutcome {
            id: Some(id.clone()),
            candidates: Vec::new()
        }
    );
    let v1 = format!("---\nid: {id}\n---\n# Pricing\nTry 5% off #pricing [[Churn notes]]\n");
    assert_eq!(content(&s, &id), v1);

    let notes = Recorder::new();
    let nid = id.clone();
    s.watch(
        Topics::ALL,
        move |c, ctx| build::note_screen(c, ctx, &nid),
        notes.clone(),
    )
    .expect("watch");

    let v2 = format!("---\nid: {id}\n---\n# Pricing\nTry 10% off #pricing\n");
    s.update_note(&id, &v2).expect("update");
    s.move_note(&id, "notes/Pricing experiments.md")
        .expect("move");

    let ops: Vec<(String, Option<Version>, Op)> = outbox(&s)
        .into_iter()
        .map(|o| (o.op_id, o.base_version, o.op))
        .collect();
    assert_eq!(
        ops,
        vec![
            (
                seq_id(2),
                None,
                Op::NoteCreate(ops::NoteCreate {
                    id: ulid(&id),
                    path: "notes/sales/Pricing experiments.md".into(),
                    content: "# Pricing\nTry 5% off #pricing [[Churn notes]]\n".into(),
                    force: false,
                })
            ),
            (
                seq_id(3),
                Some(Version::of_text(&v1)),
                Op::NoteUpdate(ops::NoteUpdate {
                    id: ulid(&id),
                    content: v2.clone(),
                })
            ),
            (
                seq_id(4),
                Some(Version::of_text(&v2)),
                Op::NoteMove(ops::NoteMove {
                    id: ulid(&id),
                    new_path: "notes/Pricing experiments.md".into(),
                })
            ),
        ]
    );

    let screens = notes.take();
    assert_eq!(screens.len(), 3, "initial, after update, after move");
    let first = screens[0].note.as_ref().expect("note");
    assert_eq!(first.path, "notes/sales/Pricing experiments.md");
    assert_eq!(first.tags, vec!["pricing".to_owned()]);
    assert_eq!(first.content, v1);
    assert_eq!(first.version, None);
    assert_eq!(
        first.sync,
        NoteSyncState {
            kind: NoteSyncKind::Pending,
            pending_ops: 1,
            conflict_op_id: None,
            duplicate_op_id: None,
            label: "Saved on this device · 1 change to sync".into(),
        }
    );
    // History is fetched on demand while online (`refresh_history`).
    assert_eq!(first.history, Availability::Available);
    assert_eq!(first.history_entries, Vec::new());
    assert_eq!(first.version_label, None);
    let second = screens[1].note.as_ref().expect("note");
    assert_eq!(second.content, v2);
    assert_eq!(second.sync.pending_ops, 2);
    let third = screens[2].note.as_ref().expect("note");
    assert_eq!(third.path, "notes/Pricing experiments.md");
    assert_eq!(third.sync.pending_ops, 3);

    s.delete_note(&id).expect("delete");
    let after = notes.take();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].note, None);
    let last = outbox(&s).pop().expect("op");
    assert_eq!(last.base_version, Some(Version::of_text(&v2)));
    assert_eq!(last.op, Op::NoteDelete(ops::NoteRef { id: ulid(&id) }));
    // A note that never reached the server disappears from the table entirely once deleted.
    assert_eq!(
        s.read(|c, _| strata_core::store::notes::get(c, &id))
            .expect("read"),
        None
    );
}

#[tokio::test]
async fn failed_intents_write_nothing() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let missing = seq_id(99);
    assert_eq!(
        s.update_note(&missing, "x"),
        Err(CoreError::NotFound {
            what: "note".into()
        })
    );
    assert_eq!(
        s.create_note("notes/bad:name.md", "x", false),
        Err(CoreError::InvalidInput {
            field: "path".into(),
            reason: "forbidden_char".into()
        })
    );
    assert_eq!(
        s.complete_task("t-missing"),
        Err(CoreError::NotFound {
            what: "task".into()
        })
    );
    assert_eq!(outbox(&s), Vec::new());
}

fn task(
    id: &str,
    note_id: &str,
    description: &str,
    due: &str,
    line_number: u32,
    due_label: &str,
) -> TaskItem {
    TaskItem {
        description_dir: TextDir::Ltr,
        note_path: "tasks/Tasks.md".to_owned(),
        line_number,
        due_label: Some(due_label.to_owned()),
        lateness_label: None,
        completion_label: None,
        next_in_label: None,
        origin_label: None,
        is_overdue: false,
        id: id.to_owned(),
        note_id: note_id.to_owned(),
        note_title: "Tasks".to_owned(),
        description: description.to_owned(),
        state: TaskState::Open,
        priority: "normal".to_owned(),
        due: NaiveDate::parse_from_str(due, "%Y-%m-%d").ok(),
        scheduled: None,
        done: None,
        recurrence: None,
        recurrence_understood: true,
        reminders: Vec::new(),
        links: Vec::new(),
        pending_sync: true,
    }
}

#[tokio::test]
async fn tasks_create_complete_recurring_and_sections() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let tasks = Recorder::new();
    s.watch(
        Topics::TASKS | Topics::NOTES | Topics::SYNC,
        build::tasks_view,
        tasks.clone(),
    )
    .expect("watch");

    // No task home yet: a local `tasks/Tasks.md` placeholder is created under a new ID.
    let one_off = s
        .create_task(
            &NewTask {
                description: "Send weekly invoicing proposal to Ahmed".into(),
                due: NaiveDate::from_ymd_opt(2026, 9, 29),
                ..NewTask::default()
            },
            false,
        )
        .expect("create");
    let t1 = format!("t-{}", seq_id(1).to_ascii_lowercase());
    assert_eq!(one_off.id.as_deref(), Some(t1.as_str()));
    let home_id = seq_id(2);
    let petrol = s
        .create_task(
            &NewTask {
                description: "Petrol Arrows invoice".into(),
                due: NaiveDate::from_ymd_opt(2026, 9, 27),
                recurrence: Some("every week on Sunday".into()),
                reminders: vec![
                    NaiveDateTime::parse_from_str("2026-09-27 10:00", "%Y-%m-%d %H:%M")
                        .expect("dt"),
                ],
                ..NewTask::default()
            },
            false,
        )
        .expect("create");
    let t2 = format!("t-{}", seq_id(4).to_ascii_lowercase());
    assert_eq!(petrol.id.as_deref(), Some(t2.as_str()));
    assert_eq!(
        content(&s, &home_id),
        format!(
            "- [ ] Send weekly invoicing proposal to Ahmed 📅 2026-09-29 ^{t1}\n\
             - [ ] Petrol Arrows invoice (@2026-09-27 10:00) 🔁 every week on Sunday 📅 2026-09-27 ^{t2}\n"
        )
    );

    s.complete_task(&t2).expect("complete");
    let t3 = format!("t-{}", seq_id(6).to_ascii_lowercase());
    assert_eq!(
        content(&s, &home_id),
        format!(
            "- [ ] Send weekly invoicing proposal to Ahmed 📅 2026-09-29 ^{t1}\n\
             - [ ] Petrol Arrows invoice (@2026-10-04 10:00) 🔁 every week on Sunday 📅 2026-10-04 ^{t3}\n\
             - [x] Petrol Arrows invoice (@2026-09-27 10:00) 🔁 every week on Sunday 📅 2026-09-27 ✅ 2026-09-27 ^{t2}\n"
        )
    );

    let ops: Vec<(String, Option<Version>, Op)> = outbox(&s)
        .into_iter()
        .map(|o| (o.local_entity, o.base_version, o.op))
        .collect();
    let petrol_line = format!(
        "- [ ] Petrol Arrows invoice (@2026-09-27 10:00) 🔁 every week on Sunday 📅 2026-09-27 ^{t2}"
    );
    assert_eq!(
        ops,
        vec![
            (
                format!("note:{home_id}"),
                None,
                Op::TaskCreate(ops::TaskCreate {
                    id: t1.clone(),
                    note_id: None,
                    text: "Send weekly invoicing proposal to Ahmed".into(),
                    due: NaiveDate::from_ymd_opt(2026, 9, 29),
                    scheduled: None,
                    start: None,
                    recurrence: None,
                    reminders: Vec::new(),
                    priority: None,
                    force: false,
                })
            ),
            (
                format!("note:{home_id}"),
                None,
                Op::TaskCreate(ops::TaskCreate {
                    id: t2.clone(),
                    note_id: None,
                    text: "Petrol Arrows invoice".into(),
                    due: NaiveDate::from_ymd_opt(2026, 9, 27),
                    scheduled: None,
                    start: None,
                    recurrence: Some("every week on Sunday".into()),
                    reminders: vec![
                        NaiveDateTime::parse_from_str("2026-09-27 10:00", "%Y-%m-%d %H:%M")
                            .expect("dt")
                    ],
                    priority: None,
                    force: false,
                })
            ),
            (
                format!("note:{home_id}"),
                Some(Version::of_text(&petrol_line)),
                Op::TaskComplete(ops::TaskComplete {
                    id: t2.clone(),
                    done: NaiveDate::from_ymd_opt(2026, 9, 27).expect("date"),
                    next_id: Some(t3.clone()),
                })
            ),
        ]
    );

    // Emissions: initial (empty), after each create, after completion.
    let views = tasks.take();
    assert_eq!(views.len(), 4);
    assert_eq!(views[0].sections, TaskSections::default());
    assert_eq!(
        views[1].sections.upcoming,
        vec![task(
            &t1,
            &home_id,
            "Send weekly invoicing proposal to Ahmed",
            "2026-09-29",
            1,
            "Tue 29 Sep"
        )]
    );
    let reminder_at = DateTime::parse_from_rfc3339("2026-09-27T07:00:00Z")
        .expect("ts")
        .with_timezone(&Utc);
    let petrol_today = TaskItem {
        recurrence: Some("every week on Sunday".into()),
        reminders: vec![strata_core::view::model::ReminderItem {
            local: "2026-09-27 10:00".into(),
            at: reminder_at,
            local_at: NaiveDateTime::parse_from_str("2026-09-27 10:00", "%Y-%m-%d %H:%M")
                .expect("dt"),
            time_label: "10:00".into(),
            offset_label: "on the day".into(),
        }],
        next_in_label: Some("next today".into()),
        ..task(
            &t2,
            &home_id,
            "Petrol Arrows invoice",
            "2026-09-27",
            2,
            "Today",
        )
    };
    assert_eq!(views[2].sections.today, vec![petrol_today.clone()]);
    assert_eq!(views[2].sections.recurring, vec![petrol_today]);
    let last = &views[3];
    assert_eq!(last.sections.today, Vec::new());
    assert_eq!(
        last.sections
            .upcoming
            .iter()
            .map(|t| (t.id.clone(), t.due))
            .collect::<Vec<_>>(),
        vec![
            (t1.clone(), NaiveDate::from_ymd_opt(2026, 9, 29)),
            (t3.clone(), NaiveDate::from_ymd_opt(2026, 10, 4)),
        ]
    );
    assert_eq!(
        last.done
            .iter()
            .map(|t| (t.id.clone(), t.state, t.done))
            .collect::<Vec<_>>(),
        vec![(
            t2.clone(),
            TaskState::Done,
            NaiveDate::from_ymd_opt(2026, 9, 27)
        )]
    );
    // Completing a done task is refused by the shared task rules and writes nothing.
    assert_eq!(
        s.complete_task(&t2),
        Err(CoreError::TaskChange {
            reason: "not_open".into()
        })
    );
    assert_eq!(outbox(&s).len(), 3);
}

#[tokio::test]
async fn relations_edit_frontmatter_through_shared_rules() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.create_note("notes/Churn notes.md", "Churn is up.\n", false)
        .expect("create");
    s.create_note("notes/Pricing experiments.md", "Try 5% off.\n", false)
        .expect("create");
    let churn = seq_id(1);
    let pricing = seq_id(3);

    s.add_relation(&pricing, &churn, "related").expect("add");
    assert_eq!(
        content(&s, &pricing),
        format!("---\nid: {pricing}\nrelated: [\"[[Churn notes]]\"]\n---\nTry 5% off.\n")
    );
    let screen = s
        .read(|c, ctx| build::note_screen(c, ctx, &churn))
        .expect("screen");
    let backlinks = screen.note.expect("note").backlinks;
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].kind, "related");
    assert_eq!(backlinks[0].items[0].note_id, pricing);

    s.retype_relation(&pricing, &churn, "related", "supports")
        .expect("retype");
    assert_eq!(
        content(&s, &pricing),
        format!("---\nid: {pricing}\nsupports: [\"[[Churn notes]]\"]\n---\nTry 5% off.\n")
    );
    s.remove_relation(&pricing, &churn, "supports")
        .expect("remove");
    assert_eq!(
        content(&s, &pricing),
        format!("---\nid: {pricing}\n---\nTry 5% off.\n")
    );
    let kinds: Vec<&str> = outbox(&s).iter().map(|o| o.op.kind().as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "note.create",
            "note.create",
            "relation.add",
            "relation.retype",
            "relation.remove"
        ]
    );
    assert_eq!(
        s.add_relation(&pricing, &churn, "likes"),
        Err(CoreError::InvalidInput {
            field: "rel_type".into(),
            reason: "unknown_relation".into()
        })
    );
}

#[tokio::test]
async fn offline_duplicate_check_prompts_before_queuing_a_create() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.create_note("notes/Pricing experiments.md", "x\n", false)
        .expect("create");
    let existing = seq_id(1);

    let outcome = s
        .create_note("notes/Pricing experiment.md", "y\n", false)
        .expect("checked");
    assert_eq!(outcome.id, None);
    assert_eq!(outcome.candidates.len(), 1);
    let c = &outcome.candidates[0];
    assert_eq!(
        (
            c.id.as_str(),
            c.kind.as_str(),
            c.title.as_str(),
            c.match_level.as_str(),
            c.score
        ),
        (
            existing.as_str(),
            "note",
            "Pricing experiments",
            "exact",
            1.0
        )
    );
    assert_eq!(outbox(&s).len(), 1, "nothing queued");

    let forced = s
        .create_note("notes/Pricing experiment.md", "y\n", true)
        .expect("forced");
    assert_eq!(forced.id, Some(seq_id(4)));
    let last = outbox(&s).pop().expect("op");
    assert_eq!(
        last.op,
        Op::NoteCreate(ops::NoteCreate {
            id: ulid(&seq_id(4)),
            path: "notes/Pricing experiment.md".into(),
            content: "y\n".into(),
            force: true,
        })
    );
}

#[tokio::test]
async fn recent_notes_on_home_are_newest_first() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.create_note("notes/A.md", "first line\n", false)
        .expect("a");
    h.clock.advance(chrono::Duration::minutes(5));
    s.create_note("notes/B.md", "# Title\n\nsecond #tag\n", false)
        .expect("b");
    let home = s.read(build::home).expect("home");
    assert_eq!(
        home.recent_notes,
        vec![
            NoteListItem {
                id: seq_id(3),
                title: "B".into(),
                path: "notes/B.md".into(),
                kind: "note".into(),
                snippet: "second #tag".into(),
                tags: vec!["tag".into()],
                updated_at: now() + chrono::Duration::minutes(5),
                pending_sync: true,
                title_dir: TextDir::Ltr,
                snippet_dir: TextDir::Ltr,
                updated_label: "13:05".into(),
                link_count: 0,
                highlights: Vec::new(),
            },
            NoteListItem {
                id: seq_id(1),
                title: "A".into(),
                path: "notes/A.md".into(),
                kind: "note".into(),
                snippet: "first line".into(),
                tags: Vec::new(),
                updated_at: now(),
                pending_sync: true,
                title_dir: TextDir::Ltr,
                snippet_dir: TextDir::Ltr,
                updated_label: "13:00".into(),
                link_count: 0,
                highlights: Vec::new(),
            },
        ]
    );
}
