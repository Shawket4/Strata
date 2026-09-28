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
    AppLifecycle, CoreConfig, CoreFailure, DocumentDraft, EditorHint, HintKind, PasswordLevel,
    PlaceDraft, Platform, RecurrenceFrequency, SearchMode, SessionKind, SignInRequest,
    SignUpRequest, TagItem, TaskDraft, TaskPatch,
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
    let hint = |kind, start, end| EditorHint {
        kind,
        start,
        end,
        target_id: None,
        target_anchor: None,
        task_id: None,
        level: 0,
    };
    assert_eq!(
        views::editor_hints("See [[Acme]] #client".to_owned()),
        [
            hint(HintKind::LtrLine, 0, 20),
            hint(HintKind::WikiLink, 4, 12),
            hint(HintKind::Tag, 13, 20),
        ]
    );
    let form = views::recurrence_form("every 2 weeks".to_owned()).expect("understood");
    assert_eq!(
        (form.frequency, form.interval, form.when_done),
        (RecurrenceFrequency::Weekly, 2, false)
    );
    assert_eq!(views::recurrence_form("whenever".to_owned()), None);

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
    assert_eq!(outcome.username, "bob");
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
    assert_eq!(capture.len(), 26, "a ULID: {capture}");
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
    .expect("retype");
    assert_eq!(
        intents::retype_relation(
            note.clone(),
            company.clone(),
            "companies".to_owned(),
            "likes".to_owned(),
        ),
        Err(invalid("new_type", "unknown_relation"))
    );
    intents::remove_relation(note.clone(), company.clone(), "companies".to_owned())
        .expect("remove relation");
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
    intents::reopen_task(task.clone()).expect("reopen");
    intents::cancel_task(task.clone()).expect("cancel");
    intents::delete_task(task.clone()).expect("delete task");
    assert_eq!(
        intents::complete_task("t-unknown".to_owned()),
        Err(CoreFailure {
            field: Some("task".to_owned()),
            ..failure("not_found")
        })
    );

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
    // One-shot reads see every local change at once (nothing has synced yet).
    assert_eq!(
        views::place_options(Some(document.clone()))
            .expect("places")
            .into_iter()
            .map(|p| (p.id, p.title, p.depth, p.is_current))
            .collect::<Vec<_>>(),
        [(place.clone(), "Office safe".to_owned(), 0, false)]
    );
    assert_eq!(
        views::tags("cl".to_owned()).expect("tags"),
        [TagItem {
            tag: "client".to_owned(),
            count: 1
        }]
    );
    let blocks = views::note_blocks(note.clone()).expect("blocks");
    assert_eq!(
        blocks
            .iter()
            .map(|b| (b.text.as_str(), b.line))
            .collect::<Vec<_>>(),
        [
            ("Weekly invoicing for [[Acme]]. #client", 6),
            ("Second paragraph.", 8),
            // Completing the monthly task queued its next occurrence; the original line was
            // then reopened, cancelled and deleted.
            (
                "[ ] Send the invoice to Acme 🔁 every month 📅 2026-11-01",
                9
            ),
        ]
    );
    assert_eq!(blocks[1].block_id.as_deref(), Some("p2"));
    let relation_keys = views::relation_types()
        .expect("types")
        .into_iter()
        .map(|r| r.key)
        .collect::<Vec<_>>();
    assert_eq!(relation_keys.len(), 20);
    assert_eq!(relation_keys[0], "related");
    let merge = views::merge_preview(company.clone(), person.clone()).expect("preview");
    assert_eq!(
        (
            merge.source.title.as_str(),
            merge.into.title.as_str(),
            merge.aliases.clone(),
            merge.mention_count,
            merge.relation_count
        ),
        ("Acme", "Ahmed Fathy", vec!["Acme".to_owned()], 1, 0)
    );
    let parsed = views::parse_task_text("Pay rent tomorrow".to_owned()).expect("parsed");
    assert_eq!(
        (parsed.description.as_str(), parsed.due.is_some(), parsed.due_label.as_deref()),
        ("Pay rent", true, Some("Tomorrow"))
    );
    let completions =
        views::editor_completions(note.clone(), "[[Ah".to_owned(), 4).expect("completions");
    assert_eq!(
        completions
            .items
            .iter()
            .map(|i| (i.label.as_str(), i.insert_text.as_str(), i.target_id.clone()))
            .collect::<Vec<_>>(),
        [("Ahmed Fathy", "Ahmed Fathy]]", Some(person.clone()))]
    );
    let citation =
        views::resolve_citation(note.clone(), Some("p2".to_owned())).expect("citation");
    assert_eq!(
        (
            citation.note_id.as_deref(),
            citation.title.as_str(),
            citation.block_text.as_deref(),
            citation.tags.clone()
        ),
        (
            Some(note.as_str()),
            "Pricing",
            Some("Second paragraph."),
            vec!["client".to_owned()]
        )
    );
    let graph = views::global_graph().expect("graph");
    assert_eq!((graph.nodes.len(), graph.edges.len()), (6, 2));
    let found = rt
        .block_on(views::search("invoicing".to_owned(), SearchMode::Keyword))
        .expect("local search");
    assert_eq!(
        found
            .results
            .iter()
            .map(|h| (h.note_id.as_str(), h.snippet.as_str()))
            .collect::<Vec<_>>(),
        [(note.as_str(), "Weekly invoicing for [[Acme]]. #client")]
    );
    assert_eq!(views::ask_view().expect("ask").messages.len(), 0);

    intents::set_default_reminder_time("08:30".to_owned()).expect("default time");
    intents::set_quiet_hours(true, "23:00".to_owned(), "06:30".to_owned()).expect("quiet");
    intents::set_snooze_minutes(30).expect("snooze");
    intents::set_reminders_enabled(false).expect("reminders");

    // Signing out with unsynced changes asks first; the unsynced ops can be exported.
    let asked = rt.block_on(app::sign_out(false)).expect("sign out");
    assert_eq!(
        (asked.signed_out, asked.unsynced_ops),
        (false, 23)
    );
    let unsynced = dir.path().join("unsynced.md");
    assert_eq!(
        app::export_unsynced(unsynced.to_str().expect("utf-8").to_owned()),
        Ok(23)
    );
    let text = std::fs::read_to_string(&unsynced).expect("written");
    assert!(text.starts_with("# Unsynced changes\n\n## "), "{text}");
    assert!(text.contains("Weekly invoicing for [[Acme]]. #client"), "{text}");
    app::set_sync_paused(true).expect("pause");
    app::set_sync_paused(false).expect("resume sync");
    let done = rt.block_on(app::sign_out(true)).expect("forced sign out");
    assert_eq!((done.signed_out, done.unsynced_ops), (true, 0));
    assert_eq!(
        intents::capture("x".to_owned()),
        Err(failure("not_signed_in"))
    );
    rt.block_on(w.finish());
}
