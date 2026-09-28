//! Tasks over HTTP (PLAN §6.11, §7.5 Tasks, §9.7, §16.3 Tasks): default home and month
//! heading, exact lines, views, recurring completion writing exactly two lines, cancel and
//! reopen, span edits with `If-Match`, entity filter, and the duplicate check with `force`.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod vault_harness;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use vault_harness::{H, assert_problem, not_found, plain, version};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("date")
}

fn new_task(text: &str) -> types::CreateTaskRequest {
    types::CreateTaskRequest {
        created: strata_common::clock::default_test_epoch(),
        home_id: None,
        text: text.into(),
        due: None,
        force: None,
        id: None,
        note_id: None,
        priority: None,
        recurrence: None,
        reminders: vec![],
        scheduled: None,
        start: None,
    }
}

#[tokio::test]
async fn recurring_completion_writes_exactly_two_lines_in_one_commit() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let task = ops::create_task(
        c,
        &types::CreateTaskRequest {
            text: "Make Watanya's ETA invoice [[Watanya]]".into(),
            recurrence: Some("every month on the 1st".into()),
            due: Some(d(2026, 10, 1)),
            reminders: vec![types::ReminderAt {
                date: d(2026, 10, 1),
                time: "09:00".into(),
            }],
            ..new_task("")
        },
    )
    .await
    .expect("create");
    let line = format!(
        "- [ ] Make Watanya's ETA invoice [[Watanya]] (@2026-10-01 09:00) 🔁 every month on the 1st 📅 2026-10-01 ^{}",
        task.id
    );
    assert_eq!(
        task,
        types::Task {
            done_at: None,
            due: Some(d(2026, 10, 1)),
            id: task.id.clone(),
            line: line.clone(),
            note_id: task.note_id,
            note_path: "tasks/Tasks.md".into(),
            priority: None,
            recurrence: Some("every month on the 1st".into()),
            recurrence_understood: true,
            reminders: vec!["2026-10-01T09:00:00Z".parse().expect("ts")],
            rrule: Some("FREQ=MONTHLY;BYMONTHDAY=1".into()),
            scheduled: None,
            start: None,
            status: types::TaskStatus::Open,
            text: "Make Watanya's ETA invoice [[Watanya]]".into(),
            version: version(&line),
        }
    );
    assert!(task.id.starts_with("t-"));
    let home = format!(
        "---\nid: {}\ncreated: 2026-09-27T12:00:00Z\nupdated: 2026-09-27T12:00:00Z\n---\n## September 2026\n{line}\n",
        task.note_id
    );
    assert_eq!(h.read(alice.id, "tasks/Tasks.md"), home);
    assert_eq!(h.log(alice.id)[0], "user: task create tasks/Tasks.md");
    let commits = h.log(alice.id).len();

    let done = ops::complete_task(c, &task.id, None)
        .await
        .expect("complete");
    let next = done.next.clone().expect("next occurrence");
    let next_line = format!(
        "- [ ] Make Watanya's ETA invoice [[Watanya]] (@2026-11-01 09:00) 🔁 every month on the 1st 📅 2026-11-01 ^{}",
        next.id
    );
    let done_line = format!(
        "- [x] Make Watanya's ETA invoice [[Watanya]] (@2026-10-01 09:00) 🔁 every month on the 1st 📅 2026-10-01 ✅ 2026-09-27 ^{}",
        task.id
    );
    assert_eq!(
        h.read(alice.id, "tasks/Tasks.md"),
        home.replace(&line, &format!("{next_line}\n{done_line}"))
    );
    assert_eq!(h.log(alice.id).len(), commits + 1);
    assert_eq!(h.log(alice.id)[0], "user: task complete tasks/Tasks.md");
    assert_eq!(
        (done.task.status, done.task.done_at, done.task.line.clone()),
        (types::TaskStatus::Done, Some(d(2026, 9, 27)), done_line)
    );
    assert_eq!(
        (
            next.status,
            next.due,
            next.reminders.clone(),
            next.line.clone()
        ),
        (
            types::TaskStatus::Open,
            Some(d(2026, 11, 1)),
            vec!["2026-11-01T09:00:00Z".parse().expect("ts")],
            next_line
        )
    );
    // A done task cannot be completed again.
    assert_problem(
        ops::complete_task(c, &task.id, None).await,
        &plain(
            "task_state_conflict",
            "Task state does not allow this",
            409,
            Some("the task is not open"),
        ),
    );
    h.finish().await;
}

#[tokio::test]
async fn views_edits_cancel_reopen_and_entity_filter() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let watanya = ops::create_entity(
        c,
        &types::CreateEntityRequest {
            created: strata_common::clock::default_test_epoch(),
            kind: types::EntityKind::Company,
            name: "Watanya".into(),
            aliases: vec![],
            fields: std::collections::HashMap::new().into_iter().collect(),
            force: None,
            id: None,
            parent_id: None,
            tags: vec![],
        },
    )
    .await
    .expect("company");
    let mk =
        |text: &str, due: Option<NaiveDate>, recurrence: Option<&str>| types::CreateTaskRequest {
            due,
            recurrence: recurrence.map(str::to_owned),
            ..new_task(text)
        };
    let today = ops::create_task(c, &mk("Pay rent", Some(d(2026, 9, 27)), None))
        .await
        .expect("t");
    let overdue = ops::create_task(c, &mk("Call bank", Some(d(2026, 9, 20)), None))
        .await
        .expect("t");
    let upcoming = ops::create_task(
        c,
        &mk("Send [[Watanya]] invoice", Some(d(2026, 10, 5)), None),
    )
    .await
    .expect("t");
    let recurring = ops::create_task(
        c,
        &mk(
            "Petrol Arrows invoice",
            Some(d(2026, 10, 4)),
            Some("every week on Sunday"),
        ),
    )
    .await
    .expect("t");
    let undated = ops::create_task(c, &mk("Read the contract", None, None))
        .await
        .expect("t");
    let ids = |l: types::TaskList| l.items.into_iter().map(|t| t.id).collect::<Vec<_>>();
    let view = |v: types::TaskViewKind| {
        let c = c.clone();
        async move {
            ids(ops::list_tasks(&c, Some(&v), None, None)
                .await
                .expect("view"))
        }
    };
    assert_eq!(
        view(types::TaskViewKind::Today).await,
        vec![today.id.clone()]
    );
    assert_eq!(
        view(types::TaskViewKind::Overdue).await,
        vec![overdue.id.clone()]
    );
    assert_eq!(
        view(types::TaskViewKind::Upcoming).await,
        vec![recurring.id.clone(), upcoming.id.clone()]
    );
    assert_eq!(
        view(types::TaskViewKind::Recurring).await,
        vec![recurring.id.clone()]
    );
    assert_eq!(view(types::TaskViewKind::Done).await, Vec::<String>::new());
    assert_eq!(
        view(types::TaskViewKind::All).await,
        vec![
            overdue.id.clone(),
            today.id.clone(),
            recurring.id.clone(),
            upcoming.id.clone(),
            undated.id.clone()
        ]
    );
    assert_eq!(
        ids(ops::list_tasks(c, None, Some(watanya.id), None)
            .await
            .expect("entity")),
        vec![upcoming.id.clone()]
    );

    // Span edit with If-Match: only the changed fields move.
    assert_problem(
        ops::patch_task(
            c,
            &undated.id,
            Some("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
            &types::PatchTaskRequest {
                clear: vec![],
                due: Some(d(2026, 9, 30)),
                priority: None,
                recurrence: None,
                reminders: None,
                scheduled: None,
                start: None,
                text: None,
            },
        )
        .await,
        &types::Problem {
            current_version: Some(undated.version.clone()),
            ..plain("version_conflict", "Version conflict", 409, None)
        },
    );
    let edited = ops::patch_task(
        c,
        &undated.id,
        Some(&undated.version),
        &types::PatchTaskRequest {
            clear: vec![],
            due: Some(d(2026, 9, 30)),
            priority: Some(types::TaskPriority::High),
            recurrence: None,
            reminders: None,
            scheduled: None,
            start: None,
            text: None,
        },
    )
    .await
    .expect("patch");
    assert_eq!(
        edited.line,
        format!("- [ ] Read the contract ⏫ 📅 2026-09-30 ^{}", undated.id)
    );
    let cleared = ops::patch_task(
        c,
        &undated.id,
        None,
        &types::PatchTaskRequest {
            clear: vec![types::TaskField::Due, types::TaskField::Priority],
            due: None,
            priority: None,
            recurrence: None,
            reminders: None,
            scheduled: None,
            start: None,
            text: Some("Read the Watanya contract".into()),
        },
    )
    .await
    .expect("patch");
    assert_eq!(
        cleared.line,
        format!("- [ ] Read the Watanya contract ^{}", undated.id)
    );

    // Cancel and reopen.
    let cancelled = ops::cancel_task(c, &overdue.id, None)
        .await
        .expect("cancel");
    assert_eq!(
        cancelled.task.line,
        format!("- [ ] Call bank 📅 2026-09-20 ^{}", overdue.id)
            .replace("- [ ]", "- [-]")
            .replace(" ^", " ❌ 2026-09-27 ^")
    );
    assert_eq!(
        view(types::TaskViewKind::Overdue).await,
        Vec::<String>::new()
    );
    assert_eq!(
        view(types::TaskViewKind::Done).await,
        vec![overdue.id.clone()]
    );
    let reopened = ops::reopen_task(c, &overdue.id, None)
        .await
        .expect("reopen");
    assert_eq!(reopened.task.line, overdue.line);
    assert_problem(
        ops::reopen_task(c, &overdue.id, None).await,
        &plain(
            "task_state_conflict",
            "Task state does not allow this",
            409,
            Some("the task is already open"),
        ),
    );
    assert_problem(ops::complete_task(c, "t-nope", None).await, &not_found());
    assert_problem(
        ops::create_task(c, &mk("Weird", None, Some("every blue moon"))).await,
        &plain(
            "invalid_body",
            "Request body is invalid",
            422,
            Some("the recurrence phrase is not understood"),
        ),
    );
    h.finish().await;
}

#[tokio::test]
async fn duplicate_tasks_are_refused_until_forced_and_keep_both_is_remembered() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let first = ops::create_task(
        c,
        &types::CreateTaskRequest {
            recurrence: Some("every month on the 1st".into()),
            due: Some(d(2026, 10, 1)),
            ..new_task("Make Watanya's ETA invoice")
        },
    )
    .await
    .expect("first");
    let again = types::CreateTaskRequest {
        recurrence: Some("every month on the 1st".into()),
        due: Some(d(2026, 11, 1)),
        ..new_task("ETA invoice for Watanya")
    };
    let ulid = strata_vault::prepare::task_ulid(&first.id).expect("t-<ulid>");
    assert_problem(
        ops::create_task(c, &again).await,
        &types::Problem {
            candidates: vec![types::DuplicateCandidate {
                id: ulid,
                kind: "task".into(),
                // Same recurrence, same people, re-ordered words: a near match per the
                // dedupe crate's scoring (20/29 in f32).
                match_level: types::MatchLevel::Near,
                score: f64::from(20.0_f32 / 29.0),
                snippet: Some("due 2026-10-01".into()),
                title: "Make Watanya's ETA invoice".into(),
            }],
            ..plain(
                "duplicate_candidates",
                "Possible duplicate",
                409,
                Some("resend with force = true to create it anyway"),
            )
        },
    );
    let forced = ops::create_task(
        c,
        &types::CreateTaskRequest {
            force: Some(true),
            ..again.clone()
        },
    )
    .await
    .expect("forced");
    // The pair is remembered (sidecar extension + table) and never flagged again.
    let sidecar = h.read(alice.id, &format!(".meta/notes/{}.json", first.note_id));
    let (a, b) = if first.id < forced.id {
        (&first.id, &forced.id)
    } else {
        (&forced.id, &first.id)
    };
    assert_eq!(
        sidecar,
        format!(
            "{{\n  \"id\": \"{}\",\n  \"relations\": [],\n  \"rejected\": [],\n  \"keep_both_items\": [\n    {{\n      \"a\": \"{a}\",\n      \"at\": \"2026-09-27T12:00:00Z\",\n      \"b\": \"{b}\",\n      \"kind\": \"task\"\n    }}\n  ]\n}}\n",
            first.note_id
        )
    );
    let scope = h.db.scope(alice.id);
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    let keep = strata_index::repo::dedupe::is_keep_both(&mut tx, "task", &first.id, &forced.id)
        .await
        .expect("query");
    let text = std::fs::read_to_string(h.dir(alice.id).join("tasks/Tasks.md")).expect("read");
    let doc = vault_format::Document::parse(&text);
    let (_, line) = strata_vault::ops::tasks::find_task(doc.body(), &forced.id).expect("line");
    let item = strata_vault::dup::task_item(&forced.id, &line);
    let rechecked = strata_vault::dup::find(&mut tx, &item, &std::collections::BTreeMap::new())
        .await
        .expect("recheck");
    tx.commit().await.expect("commit");
    assert!(keep);
    assert_eq!(rechecked, vec![]);
    // A third one is still flagged against both.
    let third = ops::create_task(c, &again).await;
    let flagged: Vec<String> = vault_harness::problem(&third.expect_err("dup"))
        .candidates
        .into_iter()
        .map(|c| c.title)
        .collect();
    assert_eq!(flagged.len(), 2);
    let _ = scope;
    h.finish().await;
}
