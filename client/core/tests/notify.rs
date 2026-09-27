//! Reminders on the device (PLAN §12.5b, D27 = a) with a fake clock: exact notification ops
//! after sync, edit, completion, cancel, delete, device toggle and sign-out; stable IDs;
//! the 60-item rolling window; Africa/Cairo DST; platform results; Done/Snooze; Linux mode.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use chrono::{DateTime, Duration, Utc};
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::notify::stable_id;
use strata_core::session::{Session, TaskEdit};
use strata_core::store::outbox;
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::{Op, ops};
use strata_core::view::build;
use strata_core::view::hub::Recorder;
use strata_core::view::model::{
    NotificationAction, NotificationActionKind, NotificationOp, NotificationOpKind,
    NotificationPermission, NotificationResult, Platform,
};

const TASKS: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0TSK";

fn utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .expect("ts")
        .with_timezone(&Utc)
}

fn schedule(kind: NotificationOpKind, task: &str, remind_at: &str, at: &str, title: &str) -> NotificationOp {
    NotificationOp {
        kind,
        id: stable_id(task, remind_at),
        at: Some(utc(at)),
        title: title.to_owned(),
        body: format!("{} · Tasks", &remind_at[11..]),
        task_id: task.to_owned(),
    }
}

fn cancel(task: &str, remind_at: &str) -> NotificationOp {
    NotificationOp::cancel(stable_id(task, remind_at))
}

const PETROL: &str = "- [ ] Petrol Arrows invoice (@2026-09-28 09:00) 🔁 every week on Sunday 📅 2026-10-04 ^t-petrol";
const WATANYA: &str =
    "- [ ] Make Watanya's ETA invoice (@2026-10-01 09:00) 📅 2026-10-01 ^t-watanya";

async fn setup(h: &Harness, body: &str) -> (std::sync::Arc<Session>, Recorder<NotificationOp>) {
    h.server
        .remote_upsert(TASKS, "tasks/Tasks.md", &format!("---\nid: {TASKS}\n---\n{body}"));
    let s = h.sign_in_a().await;
    let rec = Recorder::new();
    s.attach_notifications(Box::new(rec.clone()));
    (s, rec)
}

#[tokio::test]
async fn sync_edit_complete_cancel_delete_produce_exact_ops() {
    let h = Harness::new();
    let (s, rec) = setup(&h, &format!("{PETROL}\n{WATANYA}\n")).await;
    assert_eq!(rec.take(), Vec::new(), "nothing before the first sync");

    // After sync: both reminders, nearest first (Cairo is UTC+3 in September).
    s.sync(Trigger::Start).await.expect("sync");
    assert_eq!(
        rec.take(),
        vec![
            schedule(NotificationOpKind::Schedule, "t-petrol", "2026-09-28 09:00", "2026-09-28T06:00:00Z", "Petrol Arrows invoice"),
            schedule(NotificationOpKind::Schedule, "t-watanya", "2026-10-01 09:00", "2026-10-01T06:00:00Z", "Make Watanya's ETA invoice"),
        ]
    );
    // Recomputing changes nothing (stable IDs, same plan).
    s.recompute_notifications().expect("recompute");
    s.sync(Trigger::Timer).await.expect("sync");
    assert_eq!(rec.take(), Vec::new());

    // Edit the text: same ID, `update`.
    s.update_task(
        "t-watanya",
        &TaskEdit {
            text: Some("Make Watanya's ETA invoice (October)".into()),
            ..TaskEdit::default()
        },
    )
    .expect("edit");
    assert_eq!(
        rec.take(),
        vec![schedule(NotificationOpKind::Update, "t-watanya", "2026-10-01 09:00", "2026-10-01T06:00:00Z", "Make Watanya's ETA invoice (October)")]
    );

    // Move the reminder: the old one is cancelled, the new one scheduled.
    s.update_task(
        "t-watanya",
        &TaskEdit {
            reminders: Some(vec![
                chrono::NaiveDateTime::parse_from_str("2026-10-01 08:30", "%Y-%m-%d %H:%M")
                    .expect("dt"),
            ]),
            ..TaskEdit::default()
        },
    )
    .expect("edit");
    assert_eq!(
        rec.take(),
        vec![
            cancel("t-watanya", "2026-10-01 09:00"),
            schedule(NotificationOpKind::Schedule, "t-watanya", "2026-10-01 08:30", "2026-10-01T05:30:00Z", "Make Watanya's ETA invoice (October)"),
        ]
    );

    // Complete the recurring task: its reminder is cancelled and the next occurrence's
    // (shifted by a week, same wall-clock time) is scheduled.
    s.complete_task("t-petrol").expect("complete");
    let next = s
        .read(|c, _| {
            Ok(outbox::all(c)?
                .into_iter()
                .find_map(|o| match o.op {
                    Op::TaskComplete(ops::TaskComplete { next_id, .. }) => next_id,
                    _ => None,
                })
                .expect("next id"))
        })
        .expect("read");
    let mut ops = rec.take();
    ops.sort_by_key(|o| (o.kind as u8, o.id));
    let mut expected = vec![
        cancel("t-petrol", "2026-09-28 09:00"),
        schedule(NotificationOpKind::Schedule, &next, "2026-10-05 09:00", "2026-10-05T06:00:00Z", "Petrol Arrows invoice"),
    ];
    expected.sort_by_key(|o| (o.kind as u8, o.id));
    assert_eq!(ops, expected);

    // Cancel and delete.
    s.cancel_task("t-watanya").expect("cancel");
    assert_eq!(rec.take(), vec![cancel("t-watanya", "2026-10-01 08:30")]);
    s.delete_task(&next).expect("delete");
    assert_eq!(rec.take(), vec![cancel(&next, "2026-10-05 09:00")]);
    assert_eq!(
        s.read(|c, _| strata_core::store::notifications::all(c))
            .expect("rows"),
        Vec::new()
    );
}

#[tokio::test]
async fn device_toggle_and_sign_out_cancel_everything() {
    let h = Harness::new();
    let (s, rec) = setup(&h, &format!("{PETROL}\n{WATANYA}\n")).await;
    s.sync(Trigger::Start).await.expect("sync");
    rec.take();

    s.set_reminders_enabled(false).expect("off");
    let mut expected = vec![
        cancel("t-petrol", "2026-09-28 09:00"),
        cancel("t-watanya", "2026-10-01 09:00"),
    ];
    expected.sort_by_key(|o| o.id);
    assert_eq!(rec.take(), expected);
    let settings = s
        .read(build::settings_view)
        .expect("settings")
        .expect("signed in");
    assert!(!settings.reminders.enabled);
    assert_eq!(settings.reminders.scheduled, 0);
    // The toggle is synced as a device setting through the outbox.
    let last = s
        .read(|c, _| outbox::all(c))
        .expect("outbox")
        .pop()
        .expect("op");
    assert_eq!(
        last.op,
        Op::DeviceSettings(ops::DeviceSettings {
            device_id: ulid::Ulid::from_string("01K5DSSE00000000000000DEV1").expect("id"),
            reminders_enabled: Some(false),
        })
    );

    s.set_reminders_enabled(true).expect("on");
    assert_eq!(rec.take().len(), 2, "both scheduled again");

    h.core.sign_out(true).await.expect("sign out");
    assert_eq!(rec.take(), expected, "sign-out cancels everything");
}

#[tokio::test]
async fn cairo_dst_gap_and_fold() {
    let h = Harness::new();
    h.clock.set(utc("2026-04-01T00:00:00Z"));
    let body = "\
- [ ] Spring (@2026-04-24 00:30) ^t-spring
- [ ] Autumn (@2026-10-29 23:30) ^t-autumn
- [ ] Winter (@2026-11-02 09:00) ^t-winter
- [ ] Summer (@2026-07-01 09:00) ^t-summer
";
    let (s, rec) = setup(&h, body).await;
    s.sync(Trigger::Start).await.expect("sync");
    let got: Vec<(String, Option<DateTime<Utc>>)> = rec
        .take()
        .into_iter()
        .map(|o| (o.task_id, o.at))
        .collect();
    assert_eq!(
        got,
        vec![
            // 00:30 does not exist (clocks jump 00:00 → 01:00 EEST): same distance after the
            // transition, 01:30 EEST.
            ("t-spring".into(), Some(utc("2026-04-23T22:30:00Z"))),
            // Summer time, UTC+3.
            ("t-summer".into(), Some(utc("2026-07-01T06:00:00Z"))),
            // 23:30 happens twice (24:00 EEST → 23:00 EET): the first one, EEST.
            ("t-autumn".into(), Some(utc("2026-10-29T20:30:00Z"))),
            // Winter time, UTC+2.
            ("t-winter".into(), Some(utc("2026-11-02T07:00:00Z"))),
        ]
    );
}

#[tokio::test]
async fn rolling_window_of_60_refills_without_cancelling_fired_ones() {
    let h = Harness::new();
    let start = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).expect("date");
    let body: String = (0..70)
        .map(|i| {
            let d = start + Duration::days(i);
            format!("- [ ] Daily {i} (@{d} 09:00) ^t-d{i}\n")
        })
        .collect();
    let (s, rec) = setup(&h, &body).await;
    s.sync(Trigger::Start).await.expect("sync");
    let first = rec.take();
    assert_eq!(first.len(), 60);
    assert!(first.iter().all(|o| o.kind == NotificationOpKind::Schedule));
    assert_eq!(first[0].task_id, "t-d0");
    assert_eq!(first[59].task_id, "t-d59");

    // Ten days later the first ten have fired: the window refills with the next ten, and the
    // fired ones are not cancelled (that would remove them from the notification centre).
    h.clock.set(utc("2026-10-10T12:00:00Z"));
    s.recompute_notifications().expect("recompute");
    let refill = rec.take();
    assert_eq!(
        refill
            .iter()
            .map(|o| (o.kind, o.task_id.clone()))
            .collect::<Vec<_>>(),
        (60..70)
            .map(|i| (NotificationOpKind::Schedule, format!("t-d{i}")))
            .collect::<Vec<_>>()
    );
    let rows = s
        .read(|c, _| strata_core::store::notifications::all(c))
        .expect("rows");
    assert_eq!(rows.len(), 60);
    assert_eq!(rows[0].task_id, "t-d10");
}

#[tokio::test]
async fn platform_results_and_retries() {
    let h = Harness::new();
    let (s, rec) = setup(&h, &format!("{PETROL}\n")).await;
    s.sync(Trigger::Start).await.expect("sync");
    let op = rec.take().pop().expect("op");

    s.notification_result(op.id, NotificationResult::PermissionDenied)
        .expect("result");
    let settings = s
        .read(build::settings_view)
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.reminders.permission, NotificationPermission::Denied);
    // A refused schedule is retried on the next computation.
    s.recompute_notifications().expect("recompute");
    assert_eq!(rec.take(), vec![op.clone()]);
    s.notification_result(op.id, NotificationResult::Ok)
        .expect("result");
    let settings = s
        .read(build::settings_view)
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.reminders.permission, NotificationPermission::Granted);
    assert_eq!(settings.reminders.scheduled, 1);
    s.recompute_notifications().expect("recompute");
    assert_eq!(rec.take(), Vec::new());
}

#[tokio::test]
async fn done_and_snooze_go_through_the_outbox() {
    let h = Harness::new();
    let (s, rec) = setup(&h, &format!("{PETROL}\n{WATANYA}\n")).await;
    s.sync(Trigger::Start).await.expect("sync");
    rec.take();
    let watanya_id = stable_id("t-watanya", "2026-10-01 09:00");

    h.clock.set(utc("2026-10-01T06:00:00Z"));
    s.notification_action(
        watanya_id,
        NotificationAction {
            kind: NotificationActionKind::Snooze,
            minutes: 10,
        },
    )
    .expect("snooze");
    let op = s
        .read(|c, _| outbox::all(c))
        .expect("outbox")
        .pop()
        .expect("op");
    assert_eq!(
        op.op,
        Op::TaskUpdate(ops::TaskUpdate {
            id: "t-watanya".into(),
            reminders: Some(vec![
                chrono::NaiveDateTime::parse_from_str("2026-10-01 09:10", "%Y-%m-%d %H:%M")
                    .expect("dt")
            ]),
            ..ops::TaskUpdate::default()
        })
    );
    assert_eq!(
        rec.take(),
        vec![schedule(NotificationOpKind::Schedule, "t-watanya", "2026-10-01 09:10", "2026-10-01T06:10:00Z", "Make Watanya's ETA invoice")]
    );

    let snoozed_id = stable_id("t-watanya", "2026-10-01 09:10");
    s.notification_action(
        snoozed_id,
        NotificationAction {
            kind: NotificationActionKind::Done,
            minutes: 0,
        },
    )
    .expect("done");
    let op = s
        .read(|c, _| outbox::all(c))
        .expect("outbox")
        .pop()
        .expect("op");
    assert_eq!(
        op.op,
        Op::TaskComplete(ops::TaskComplete {
            id: "t-watanya".into(),
            done: chrono::NaiveDate::from_ymd_opt(2026, 10, 1).expect("date"),
            next_id: None,
        })
    );
    assert_eq!(rec.take(), vec![cancel("t-watanya", "2026-10-01 09:10")]);
}

#[tokio::test]
async fn linux_shows_due_reminders_while_running() {
    let h = Harness::with_platform(Platform::Linux);
    let (s, rec) = setup(&h, &format!("{PETROL}\n{WATANYA}\n")).await;
    s.sync(Trigger::Start).await.expect("sync");
    assert_eq!(rec.take(), Vec::new(), "nothing is scheduled with the OS");
    assert_eq!(
        s.tick_notifications().expect("tick"),
        Some(utc("2026-09-28T06:00:00Z"))
    );
    assert_eq!(rec.take(), Vec::new());

    h.clock.set(utc("2026-09-28T06:00:00Z"));
    assert_eq!(
        s.tick_notifications().expect("tick"),
        Some(utc("2026-10-01T06:00:00Z"))
    );
    assert_eq!(
        rec.take(),
        vec![NotificationOp {
            kind: NotificationOpKind::ShowNow,
            id: stable_id("t-petrol", "2026-09-28 09:00"),
            at: None,
            title: "Petrol Arrows invoice".into(),
            body: "09:00 · Tasks".into(),
            task_id: "t-petrol".into(),
        }]
    );
    // Shown once.
    s.tick_notifications().expect("tick");
    assert_eq!(rec.take(), Vec::new());
}
