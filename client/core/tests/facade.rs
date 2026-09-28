//! The `flutter_rust_bridge` facade (`strata_core::api`) as Dart calls it: the process-wide
//! core opened by `init_core` with the production wiring (system clock, ULIDs, the generated
//! client) against the real server in process. Every facade function forwards to the core and
//! converts its errors into `CoreFailure`; this drives the app, intent and one-shot view
//! surfaces end to end and checks what Dart receives.
//!
//! The core is a process-wide singleton, so this binary holds one test. `init_core` runs
//! outside a Tokio runtime, which leaves the background sync loop off: every sync in the test
//! is explicit and the results are deterministic apart from the IDs and times the production
//! clock and ULID generator hand out (which are taken from the results).
//!
//! The `watch_*` streams need a Dart port (`StreamSink`) and are exercised by the app's
//! integration tests, not here.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

#[path = "support/world.rs"]
mod world;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use strata_core::api::{app, intents, views};
use strata_core::view::model::{
    AppLifecycle, CoreConfig, CoreFailure, DocumentDraft, PasswordLevel, PlaceDraft, Platform,
    SearchMode, SessionKind, SignInRequest, SignUpRequest, TaskDraft, TaskPatch,
};
use strata_index::types::UserRole;
use world::World;

fn failure(code: &str) -> CoreFailure {
    CoreFailure {
        code: code.to_owned(),
        message_key: format!("error.{code}"),
        field: None,
        reason: None,
        count: None,
        status: None,
    }
}

fn invalid(field: &str, reason: &str) -> CoreFailure {
    CoreFailure {
        field: Some(field.to_owned()),
        reason: Some(reason.to_owned()),
        ..failure("invalid_input")
    }
}

fn show<T: std::fmt::Debug>(label: &str, v: &T) {
    println!("=== {label}\n{v:#?}");
}

#[test]
fn the_facade_drives_the_core_end_to_end() {
    // Before `init_core`: every call that needs the core says so; pure helpers still work.
    assert_eq!(
        intents::capture("x".to_owned()),
        Err(failure("not_initialised"))
    );
    assert_eq!(views::global_graph(), Err(failure("not_initialised")));
    assert_eq!(app::sync_now(), Err(failure("not_initialised")));
    assert_eq!(
        app::app_lifecycle(AppLifecycle::Resumed),
        Err(failure("not_initialised"))
    );
    let weak = app::password_strength("abc".to_owned());
    assert_eq!(
        (weak.level, weak.length, weak.min_length),
        (PasswordLevel::TooShort, 3, 10)
    );
    show("hints", &views::editor_hints("See [[Acme]] #client".to_owned()));
    show("recurrence form", &views::recurrence_form("every 2 weeks".to_owned()));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let w = rt.block_on(World::new());
    rt.block_on(w.account("alice", UserRole::Admin));
    let dir = tempfile::TempDir::new().expect("app data");
    let url = w.server.base_url();

    // Outside a runtime: no background loop.
    let state = futures::executor::block_on(app::init_core(CoreConfig {
        app_data_dir: dir.path().to_str().expect("utf-8").to_owned(),
        platform: Platform::Linux,
        default_device_name: "Facade laptop".to_owned(),
        default_server_url: Some(url.clone()),
    }))
    .expect("init");
    assert_eq!(state.kind, SessionKind::SignedOut);
    assert_eq!(state.server_url.as_deref(), Some(url.as_str()));
    assert_eq!(state.device_name, "Facade laptop");
    assert_eq!(
        intents::capture("x".to_owned()),
        Err(failure("not_signed_in"))
    );

    // Sign-up waits for approval; a wrong password is typed.
    let outcome = rt
        .block_on(app::sign_up(SignUpRequest {
            server_url: url.clone(),
            username: "bob".to_owned(),
            password: "bob-password-1".to_owned(),
            display_name: "Bob".to_owned(),
        }))
        .expect("sign up");
    show("sign up", &outcome);
    let wrong = rt.block_on(app::sign_in(SignInRequest {
        server_url: url.clone(),
        username: "alice".to_owned(),
        password: "nope-nope-nope".to_owned(),
        device_name: "Facade laptop".to_owned(),
    }));
    assert_eq!(wrong, Err(failure("invalid_credentials")));
    let active = rt
        .block_on(app::sign_in(SignInRequest {
            server_url: url.clone(),
            username: "alice".to_owned(),
            password: world::password("alice"),
            device_name: "Facade laptop".to_owned(),
        }))
        .expect("sign in");
    assert_eq!(active.kind, SessionKind::Active);
    assert_eq!(
        active.account.as_ref().map(|a| a.username.as_str()),
        Some("alice")
    );
    app::app_lifecycle(AppLifecycle::Resumed).expect("resume");
    app::sync_now().expect("sync now");
    rt.block_on(app::refresh_account()).expect("refresh account");

    // Local intents.
    let note = intents::create_note(
        "notes/Pricing.md".to_owned(),
        "Weekly invoicing for [[Acme]]. #client\n\nSecond paragraph. ^p2\n".to_owned(),
        false,
    )
    .expect("create note")
    .id
    .expect("created");
    let capture = intents::capture("Call the bank".to_owned()).expect("capture");
    show("capture", &capture);
    let person = intents::create_entity(
        "person".to_owned(),
        "Ahmed Fathy".to_owned(),
        vec!["Ahmed".to_owned()],
        false,
    )
    .expect("person")
    .id
    .expect("created");
    assert_eq!(
        intents::create_entity("robot".to_owned(), "R2".to_owned(), vec![], false),
        Err(invalid("kind", "unknown"))
    );
    let company = intents::create_entity("company".to_owned(), "Acme".to_owned(), vec![], false)
        .expect("company")
        .id
        .expect("created");
    intents::add_relation(note.clone(), company.clone(), "related".to_owned())
        .expect("relate");
    intents::retype_relation(
        note.clone(),
        company.clone(),
        "related".to_owned(),
        "companies".to_owned(),
    )
    .map(|_| ())
    .unwrap_or_else(|e| show("retype", &e));
    intents::remove_relation(note.clone(), company.clone(), "companies".to_owned())
        .map(|_| ())
        .unwrap_or_else(|e| show("remove relation", &e));
    intents::set_property(person.clone(), "phone".to_owned(), "+20 100".to_owned())
        .expect("property");
    intents::remove_property(person.clone(), "phone".to_owned()).expect("remove property");
    intents::add_alias(person.clone(), "Fathy".to_owned()).expect("alias");
    intents::remove_alias(person.clone(), "Fathy".to_owned()).expect("remove alias");
    intents::update_user_notes(person.clone(), "Met in Cairo.".to_owned()).expect("notes");
    intents::pin_note(note.clone(), true).expect("pin");

    let due = NaiveDate::from_ymd_opt(2026, 10, 1).expect("date");
    let task = intents::create_task(
        TaskDraft {
            note_id: Some(note.clone()),
            description: "Send the invoice".to_owned(),
            due: Some(due),
            scheduled: None,
            recurrence: None,
            reminders: vec![],
            priority: Some("high".to_owned()),
        },
        false,
    )
    .expect("task")
    .id
    .expect("created");
    assert_eq!(
        intents::create_task(
            TaskDraft {
                note_id: None,
                description: "x".to_owned(),
                due: None,
                scheduled: None,
                recurrence: None,
                reminders: vec![],
                priority: Some("urgent-ish".to_owned()),
            },
            false
        ),
        Err(invalid("priority", "unknown"))
    );
    let at = due.and_hms_opt(9, 0, 0).expect("time");
    intents::add_reminder(task.clone(), at).expect("reminder");
    intents::remove_reminder(task.clone(), at).expect("remove reminder");
    intents::update_task(
        task.clone(),
        TaskPatch {
            text: Some("Send the invoice to Acme".to_owned()),
            due: None,
            clear_due: false,
            recurrence: Some("every month".to_owned()),
            clear_recurrence: false,
            reminders: None,
            priority: Some("normal".to_owned()),
        },
    )
    .expect("update task");
    intents::complete_task(task.clone()).expect("complete");
    intents::reopen_task(task.clone())
        .map(|_| ())
        .unwrap_or_else(|e| show("reopen", &e));
    intents::cancel_task(task.clone())
        .map(|_| ())
        .unwrap_or_else(|e| show("cancel", &e));
    intents::delete_task(task.clone())
        .map(|_| ())
        .unwrap_or_else(|e| show("delete task", &e));

    let place = intents::create_place(
        PlaceDraft {
            name: "Office safe".to_owned(),
            aliases: vec![],
            parent_id: None,
            address: None,
        },
        false,
    )
    .expect("place")
    .id
    .expect("created");
    let document = intents::create_document(
        DocumentDraft {
            name: "Passport".to_owned(),
            aliases: vec![],
            doc_type: Some("passport".to_owned()),
            copy: None,
            copy_of: None,
            companies: vec![],
            people: vec![person.clone()],
            expires: Some(NaiveDate::from_ymd_opt(2030, 1, 1).expect("date")),
        },
        false,
    )
    .expect("document")
    .id
    .expect("created");
    show("place options", &views::place_options(Some(document.clone())));
    show("tags", &views::tags("cl".to_owned()));
    show("blocks", &views::note_blocks(note.clone()));
    show("relation types", &views::relation_types().map(|r| r.len()));
    show("merge preview", &views::merge_preview(company.clone(), person.clone()));
    show("task text", &views::parse_task_text("Pay rent tomorrow".to_owned()));
    show("completions", &views::editor_completions(note.clone(), "[[Ah".to_owned(), 4));
    show("citation", &views::resolve_citation(note.clone(), Some("p2".to_owned())));
    show("global graph", &views::global_graph().map(|g| (g.nodes.len(), g.edges.len())));
    show(
        "local search",
        &rt.block_on(views::search("invoicing".to_owned(), SearchMode::Keyword)),
    );
    show("ask view", &views::ask_view().map(|a| a.messages.len()));
    show("place id", &place);

    intents::set_default_reminder_time("08:30".to_owned()).expect("default time");
    intents::set_quiet_hours(true, "23:00".to_owned(), "06:30".to_owned()).expect("quiet");
    intents::set_snooze_minutes(30).expect("snooze");
    intents::set_reminders_enabled(false).expect("reminders");

    // Signing out with unsynced changes asks first; the unsynced ops can be exported.
    let asked = rt.block_on(app::sign_out(false)).expect("sign out");
    show("sign out asked", &asked);
    let unsynced = dir.path().join("unsynced.md");
    show(
        "export unsynced",
        &app::export_unsynced(unsynced.to_str().expect("utf-8").to_owned()),
    );
    app::set_sync_paused(true).expect("pause");
    app::set_sync_paused(false).expect("resume sync");
    let done = rt.block_on(app::sign_out(true)).expect("forced sign out");
    show("sign out forced", &done);
    rt.block_on(w.finish());
}
