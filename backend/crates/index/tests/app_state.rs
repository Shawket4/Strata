//! Tasks and reminders, suggestions with replies, AI decisions, dedupe keys, settings, usage.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use chrono::{Duration, NaiveDate};
use pretty_assertions::assert_eq;
use strata_common::{
    Clock, DecisionId, DeviceId, JobId, NoteId, NotificationId, ReplyId, SuggestionId, UserId,
};
use strata_index::ScopedTx;
use strata_index::repo::dedupe::{self, NearMatch};
use strata_index::repo::notes::{self, Note};
use strata_index::repo::settings::{self, AiUsage};
use strata_index::repo::suggestions::{self, Decision, Reply};
use strata_index::repo::tasks::{self, DueReminder, Notification, Task, TaskFilter};
use strata_index::types::{
    DecisionKind, NoteKind, NotifyProvider, NotifyResult, Priority, ReplyAuthor, SuggestionStatus,
    TaskStatus,
};
use strata_testkit::{TestDb, TestUser};

async fn user(db: &TestDb, name: &str) -> UserId {
    TestUser::new(name).create(db).await.expect("user").id
}

async fn a_note(db: &TestDb, tx: &mut ScopedTx, path: &str) -> NoteId {
    let t = db.clock.now();
    let id = NoteId::generate(db.ids.as_ref());
    notes::upsert_note(
        tx,
        &Note {
            id,
            path: path.into(),
            title: path.into(),
            kind: NoteKind::Note,
            lang: None,
            created: t,
            updated: t,
            content_hash: "h".into(),
            word_count: 0,
            trashed: false,
        },
    )
    .await
    .expect("note");
    id
}

fn task(id: &str, note: NoteId, due: Option<NaiveDate>, line: i32) -> Task {
    Task {
        id: id.into(),
        note_id: note,
        text: format!("task {id}"),
        status: TaskStatus::Open,
        due,
        scheduled: None,
        start: None,
        recurrence_raw: None,
        rrule: None,
        recurrence_understood: true,
        priority: None,
        done_at: None,
        line_start: line,
        line_end: line,
    }
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[tokio::test]
async fn tasks_replace_filter_and_reminders() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let inbox = a_note(&db, &mut tx, "tasks/Tasks.md").await;
    let other = a_note(&db, &mut tx, "notes/Watanya.md").await;
    let monthly = Task {
        recurrence_raw: Some("every month on the 1st".into()),
        rrule: Some("FREQ=MONTHLY;BYMONTHDAY=1".into()),
        priority: Some(Priority::High),
        ..task("t-01", inbox, Some(date(2026, 10, 1)), 3)
    };
    let weird = Task {
        recurrence_raw: Some("every blue moon".into()),
        recurrence_understood: false,
        ..task("t-02", inbox, None, 4)
    };
    tasks::replace_note_tasks(&mut tx, inbox, &[monthly.clone(), weird.clone()])
        .await
        .expect("tasks");
    tasks::replace_note_tasks(
        &mut tx,
        other,
        &[task("t-03", other, Some(date(2026, 9, 27)), 0)],
    )
    .await
    .expect("other");
    assert_eq!(
        tasks::get_task(&mut tx, "t-01").await.expect("get"),
        Some(monthly.clone())
    );

    let all: Vec<String> = tasks::list_tasks(&mut tx, TaskFilter::default())
        .await
        .expect("all")
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(all, vec!["t-03", "t-01", "t-02"]);
    let today: Vec<String> = tasks::list_tasks(
        &mut tx,
        TaskFilter {
            status: Some(TaskStatus::Open),
            due_on_or_before: Some(date(2026, 9, 27)),
            note_id: None,
        },
    )
    .await
    .expect("today")
    .into_iter()
    .map(|t| t.id)
    .collect();
    assert_eq!(today, vec!["t-03"]);
    assert_eq!(
        tasks::list_tasks(
            &mut tx,
            TaskFilter {
                note_id: Some(inbox),
                ..TaskFilter::default()
            }
        )
        .await
        .expect("by note")
        .len(),
        2
    );

    // Reminders; completing the task removes it from due reminders.
    let r1 = t0 + Duration::hours(21);
    let r2 = t0 + Duration::days(4);
    tasks::replace_reminders(&mut tx, "t-01", &[r2, r1, r1])
        .await
        .expect("reminders");
    tasks::replace_reminders(&mut tx, "t-03", &[t0])
        .await
        .expect("reminders");
    assert_eq!(
        tasks::due_reminders(&mut tx, r1).await.expect("due"),
        vec![
            DueReminder {
                task_id: "t-03".into(),
                remind_at: t0
            },
            DueReminder {
                task_id: "t-01".into(),
                remind_at: r1
            }
        ]
    );
    let done = Task {
        status: TaskStatus::Done,
        done_at: Some(date(2026, 9, 27)),
        ..task("t-03", other, Some(date(2026, 9, 27)), 0)
    };
    tasks::replace_note_tasks(&mut tx, other, std::slice::from_ref(&done))
        .await
        .expect("complete");
    assert_eq!(
        tasks::due_reminders(&mut tx, r1).await.expect("due").len(),
        1
    );

    // Re-deriving a note drops tasks no longer present (and their reminders).
    tasks::replace_note_tasks(&mut tx, inbox, &[weird])
        .await
        .expect("rederive");
    assert_eq!(tasks::get_task(&mut tx, "t-01").await.expect("gone"), None);
    assert_eq!(
        tasks::due_reminders(&mut tx, r2).await.expect("due"),
        vec![]
    );
}

#[tokio::test]
async fn notifications_are_recorded_once_per_task_time_device() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let device = DeviceId::generate(db.ids.as_ref());
    let n = Notification {
        id: NotificationId::generate(db.ids.as_ref()),
        task_id: "t-01".into(),
        remind_at: t0,
        device_id: device,
        provider: NotifyProvider::Fcm,
        sent_at: t0 + Duration::seconds(2),
        result: NotifyResult::Sent,
    };
    assert!(
        tasks::record_notification(&mut tx, &n)
            .await
            .expect("first")
    );
    let retry = Notification {
        id: NotificationId::generate(db.ids.as_ref()),
        sent_at: t0 + Duration::seconds(9),
        ..n.clone()
    };
    assert!(
        !tasks::record_notification(&mut tx, &retry)
            .await
            .expect("retry is a no-op")
    );
    let other_device = Notification {
        id: NotificationId::generate(db.ids.as_ref()),
        device_id: DeviceId::generate(db.ids.as_ref()),
        provider: NotifyProvider::EventStream,
        ..n.clone()
    };
    assert!(
        tasks::record_notification(&mut tx, &other_device)
            .await
            .expect("other device")
    );
    let mut expected = vec![n, other_device];
    expected.sort_by_key(|x| x.device_id);
    assert_eq!(
        tasks::notifications_for(&mut tx, "t-01")
            .await
            .expect("log"),
        expected
    );
}

#[tokio::test]
async fn suggestions_decide_once_and_thread_replies() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let note = a_note(&db, &mut tx, "inbox/2026-09-27-120000.md").await;
    let s1 = suggestions::create_suggestion(
        &mut tx,
        SuggestionId::generate(db.ids.as_ref()),
        Some(note),
        "filing",
        &[0x80],
        t0,
    )
    .await
    .expect("s1");
    let s2 = suggestions::create_suggestion(
        &mut tx,
        SuggestionId::generate(db.ids.as_ref()),
        None,
        "entity_link",
        &[0x81],
        t0 + Duration::seconds(1),
    )
    .await
    .expect("s2");
    assert_eq!(
        (s1.status, s1.decided_at, s1.payload.as_slice()),
        (SuggestionStatus::Pending, None, &[0x80u8][..])
    );
    assert_eq!(
        suggestions::list_suggestions(&mut tx, SuggestionStatus::Pending)
            .await
            .expect("pending"),
        vec![s1.clone(), s2.clone()]
    );

    let t1 = t0 + Duration::minutes(3);
    let accepted = suggestions::decide_suggestion(&mut tx, s1.id, SuggestionStatus::Accepted, t1)
        .await
        .expect("accept")
        .expect("pending");
    assert_eq!(
        (accepted.status, accepted.decided_at, accepted.updated),
        (SuggestionStatus::Accepted, Some(t1), t1)
    );
    assert_eq!(
        suggestions::decide_suggestion(&mut tx, s1.id, SuggestionStatus::Rejected, t1)
            .await
            .expect("again"),
        None
    );
    let superseded =
        suggestions::decide_suggestion(&mut tx, s2.id, SuggestionStatus::Superseded, t1)
            .await
            .expect("supersede")
            .expect("pending");
    assert_eq!(
        (superseded.status, superseded.decided_at),
        (SuggestionStatus::Superseded, None)
    );
    let err = suggestions::decide_suggestion(&mut tx, s2.id, SuggestionStatus::Pending, t1)
        .await
        .expect_err("pending is not a decision");
    assert_eq!(
        err.to_string(),
        "invalid argument: a suggestion cannot be decided as pending"
    );

    let r1 = Reply {
        id: ReplyId::generate(db.ids.as_ref()),
        suggestion_id: s2.id,
        author: ReplyAuthor::User,
        body: "no, the Petrol Arrows one".into(),
        created: t1,
    };
    let r2 = Reply {
        id: ReplyId::generate(db.ids.as_ref()),
        suggestion_id: s2.id,
        author: ReplyAuthor::Ai,
        body: "Linked to Ahmed Fathy instead.".into(),
        created: t1 + Duration::seconds(5),
    };
    suggestions::add_reply(&mut tx, &r2).await.expect("r2");
    suggestions::add_reply(&mut tx, &r1).await.expect("r1");
    assert_eq!(
        suggestions::replies(&mut tx, s2.id).await.expect("thread"),
        vec![r1, r2]
    );
    assert_eq!(
        suggestions::get_suggestion(&mut tx, s2.id)
            .await
            .expect("get"),
        Some(superseded)
    );
}

#[tokio::test]
async fn ai_decisions_recent_first_and_revert_once() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    let mut made = Vec::new();
    for i in 0..3 {
        let d = Decision {
            id: DecisionId::generate(db.ids.as_ref()),
            kind: if i == 1 {
                DecisionKind::EntityMention
            } else {
                DecisionKind::Relation
            },
            source_note_id: None,
            source_block_id: Some(format!("b{i}")),
            target_type: "entity".into(),
            target_id: format!("e{i}"),
            summary: format!("decision {i}"),
            confidence: Some(0.8),
            job_id: Some(JobId::generate(db.ids.as_ref())),
            suggestion_id: None,
            git_commit: Some(format!("c{i}")),
            created: t0 + Duration::seconds(i),
            reverted_at: None,
        };
        suggestions::record_decision(&mut tx, &d)
            .await
            .expect("record");
        made.push(d);
    }
    assert_eq!(
        suggestions::recent_decisions(&mut tx, 2)
            .await
            .expect("recent"),
        vec![made[2].clone(), made[1].clone()]
    );
    assert!(
        suggestions::mark_decision_reverted(&mut tx, made[0].id, t0 + Duration::minutes(1))
            .await
            .expect("revert")
    );
    assert!(
        !suggestions::mark_decision_reverted(&mut tx, made[0].id, t0 + Duration::minutes(2))
            .await
            .expect("again")
    );
    let all = suggestions::recent_decisions(&mut tx, 50)
        .await
        .expect("all");
    assert_eq!(all[2].reverted_at, Some(t0 + Duration::minutes(1)));
}

#[tokio::test]
async fn dedupe_exact_near_and_keep_both() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let t0 = db.clock.now();
    let mut tx = db.begin(a).await.expect("tx");
    dedupe::upsert_key(
        &mut tx,
        "task",
        "t-1",
        "make watanya eta invoice|monthly",
        "make watanyas eta invoice",
    )
    .await
    .expect("k1");
    dedupe::upsert_key(
        &mut tx,
        "task",
        "t-2",
        "petrol arrows invoice|weekly",
        "petrol arrows invoice",
    )
    .await
    .expect("k2");
    dedupe::upsert_key(
        &mut tx,
        "note",
        "n-1",
        "make watanya eta invoice|monthly",
        "make watanyas eta invoice",
    )
    .await
    .expect("other kind");

    assert_eq!(
        dedupe::exact_matches(&mut tx, "task", "make watanya eta invoice|monthly")
            .await
            .expect("exact"),
        vec!["t-1"]
    );
    assert_eq!(
        dedupe::exact_matches(&mut tx, "task", "nothing")
            .await
            .expect("exact"),
        Vec::<String>::new()
    );

    let near = dedupe::near_matches(&mut tx, "task", "eta invoice for watanya", 0.3, 10)
        .await
        .expect("near");
    assert_eq!(
        near.iter().map(|m| m.item_id.as_str()).collect::<Vec<_>>(),
        vec!["t-1"]
    );
    assert!(near[0].score >= 0.3 && near[0].score < 1.0);
    assert_eq!(
        dedupe::near_matches(&mut tx, "task", "make watanyas eta invoice", 0.9, 10)
            .await
            .expect("identical"),
        vec![NearMatch {
            item_id: "t-1".into(),
            score: 1.0
        }]
    );

    // Keep-both is order-insensitive and idempotent.
    assert!(
        !dedupe::is_keep_both(&mut tx, "task", "t-2", "t-1")
            .await
            .expect("not yet")
    );
    dedupe::add_keep_both(&mut tx, "task", "t-2", "t-1", t0)
        .await
        .expect("keep");
    dedupe::add_keep_both(&mut tx, "task", "t-1", "t-2", t0)
        .await
        .expect("again");
    dedupe::add_keep_both(&mut tx, "task", "t-1", "t-9", t0)
        .await
        .expect("another");
    assert!(
        dedupe::is_keep_both(&mut tx, "task", "t-1", "t-2")
            .await
            .expect("yes")
    );
    assert!(
        !dedupe::is_keep_both(&mut tx, "note", "t-1", "t-2")
            .await
            .expect("per kind")
    );
    assert_eq!(
        dedupe::keep_both_partners(&mut tx, "task", "t-1")
            .await
            .expect("partners"),
        vec!["t-2", "t-9"]
    );
    let err = dedupe::add_keep_both(&mut tx, "task", "t-1", "t-1", t0)
        .await
        .expect_err("self pair");
    assert_eq!(err.sqlstate().as_deref(), Some("23514"));
    tx.rollback().await.expect("rollback");

    let mut tx = db.begin(a).await.expect("tx");
    dedupe::upsert_key(&mut tx, "task", "t-1", "a", "a")
        .await
        .expect("k");
    assert!(
        dedupe::remove_key(&mut tx, "task", "t-1")
            .await
            .expect("remove")
    );
    assert!(
        !dedupe::remove_key(&mut tx, "task", "t-1")
            .await
            .expect("again")
    );
}

#[tokio::test]
async fn settings_and_ai_usage_accumulate_per_user() {
    let db = TestDb::new().await.expect("db");
    let a = user(&db, "alice").await;
    let b = user(&db, "bob").await;
    let t0 = db.clock.now();
    let day = date(2026, 9, 27);
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(
        settings::get_setting(&mut tx, "timezone")
            .await
            .expect("get"),
        None
    );
    settings::put_setting(&mut tx, "timezone", b"\xacAfrica/Cairo", t0)
        .await
        .expect("put");
    settings::put_setting(&mut tx, "auto_file", &[0xc3], t0)
        .await
        .expect("put");
    settings::put_setting(&mut tx, "auto_file", &[0xc2], t0)
        .await
        .expect("overwrite");
    assert_eq!(
        settings::list_settings(&mut tx).await.expect("list"),
        vec![
            ("auto_file".to_owned(), vec![0xc2]),
            ("timezone".to_owned(), b"\xacAfrica/Cairo".to_vec())
        ]
    );
    let call = AiUsage {
        day,
        provider: "claude_cli".into(),
        model: "m".into(),
        calls: 1,
        input_tokens: 100,
        output_tokens: 20,
        est_cost_micros: 5,
    };
    settings::add_ai_usage(&mut tx, &call).await.expect("first");
    let total = settings::add_ai_usage(&mut tx, &call)
        .await
        .expect("second");
    assert_eq!(
        total,
        AiUsage {
            calls: 2,
            input_tokens: 200,
            output_tokens: 40,
            est_cost_micros: 10,
            ..call.clone()
        }
    );
    assert_eq!(
        settings::ai_usage_for_day(&mut tx, day).await.expect("day"),
        vec![total]
    );
    tx.commit().await.expect("commit");

    let mut tx = db.begin(b).await.expect("tx");
    assert_eq!(
        settings::get_setting(&mut tx, "timezone")
            .await
            .expect("foreign"),
        None
    );
    assert_eq!(
        settings::ai_usage_for_day(&mut tx, day)
            .await
            .expect("foreign"),
        vec![]
    );
}
