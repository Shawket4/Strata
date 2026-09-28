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
    AdminUserItem, AppLifecycle, AskScope, AskScopeKind, Availability, ConflictResolution,
    CoreConfig, CoreFailure, CustodyDraft, DocumentDraft, DuplicateChoice, EditorHint, GraphFilter,
    GraphLens, HintKind, ImportSummary, LinkOrCreateChoice, LinkOrCreateKind, MentionEdit,
    NewUserRequest, NodePosition, NotificationAction, NotificationActionKind, NotificationResult,
    PasswordLevel, PlaceDraft, Platform, RecurrenceCompose, RecurrenceFrequency,
    RecurrencePreviewItem, ResolutionKind, SearchMode, SessionKind, SignInRequest, SignUpRequest,
    SuggestionEdits, TagItem, TaskDraft, TaskPatch, TextRange,
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
        markers: Vec::new(),
    };
    assert_eq!(
        views::editor_hints("See [[Acme]] #client".to_owned()),
        [
            hint(HintKind::LtrLine, 0, 20),
            EditorHint {
                markers: vec![
                    TextRange { start: 4, end: 6 },
                    TextRange { start: 10, end: 12 },
                ],
                ..hint(HintKind::WikiLink, 4, 12)
            },
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
    rt.block_on(app::refresh_account())
        .expect("refresh account");

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
    intents::add_relation(note.clone(), company.clone(), "related".to_owned()).expect("relate");
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
        (
            parsed.description.as_str(),
            parsed.due.is_some(),
            parsed.due_label.as_deref()
        ),
        ("Pay rent", true, Some("Tomorrow"))
    );
    let completions =
        views::editor_completions(note.clone(), "[[Ah".to_owned(), 4).expect("completions");
    assert_eq!(
        completions
            .items
            .iter()
            .map(|i| (
                i.label.as_str(),
                i.insert_text.as_str(),
                i.target_id.clone()
            ))
            .collect::<Vec<_>>(),
        [("Ahmed Fathy", "Ahmed Fathy]]", Some(person.clone()))]
    );
    let citation = views::resolve_citation(note.clone(), Some("p2".to_owned())).expect("citation");
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

    // Edits, moves, merges and relation fixes of local items.
    let edited = intents::update_note(
        note.clone(),
        "Weekly invoicing for [[Acme]]. #client\n\nSecond paragraph. ^p2\n\nThird.\n".to_owned(),
        None,
    )
    .expect("edit");
    assert_eq!(edited.len(), 26, "an op ID: {edited}");
    assert_eq!(
        intents::update_note(note.clone(), "x".to_owned(), Some("v-unknown".to_owned())),
        Err(failure("stale_edit"))
    );
    let scratch = intents::create_note("notes/Scratch.md".to_owned(), "Draft.\n".to_owned(), false)
        .expect("scratch")
        .id
        .expect("created");
    intents::move_note(scratch.clone(), "archive/Scratch.md".to_owned()).expect("move");
    intents::delete_note(scratch.clone()).expect("delete note");
    assert_eq!(
        intents::insert_mention(note.clone(), "Hi @Ah".to_owned(), 3, 6, person.clone()),
        Ok(MentionEdit {
            content: "---\npeople: [\"[[Ahmed Fathy]]\"]\n---\nHi [[Ahmed Fathy]]".to_owned(),
            cursor: 54,
        })
    );
    let other = intents::create_entity("company".to_owned(), "Acme Corp".to_owned(), vec![], true)
        .expect("company")
        .id
        .expect("created");
    intents::add_relation(person.clone(), other.clone(), "works-at".to_owned()).expect("works at");
    intents::repoint_relation(
        person.clone(),
        other.clone(),
        "works-at".to_owned(),
        company.clone(),
    )
    .expect("repoint");
    intents::reject_relation(person.clone(), company.clone(), "works-at".to_owned())
        .expect("reject relation");
    intents::merge_entities(other.clone(), company.clone()).expect("merge");
    intents::record_custody(
        document.clone(),
        CustodyDraft {
            kind: "stored-at".to_owned(),
            place_id: Some(place.clone()),
            person_id: None,
            counterparty_id: None,
            date: NaiveDate::from_ymd_opt(2026, 9, 20),
            note: None,
        },
    )
    .expect("custody");
    intents::request_relink(note.clone()).expect("relink");

    // Inbox and sync items that do not exist answer `not_found` (what: the item looked up).
    let unknown = "01M3HBS0G0000000000000ZZZZ".to_owned();
    let not_found = |what: &str| CoreFailure {
        field: Some(what.to_owned()),
        ..failure("not_found")
    };
    let no_suggestion = Err(not_found("suggestion"));
    assert_eq!(intents::accept_suggestion(unknown.clone()), no_suggestion);
    assert_eq!(intents::reject_suggestion(unknown.clone()), no_suggestion);
    assert_eq!(
        intents::accept_suggestion_with(unknown.clone(), SuggestionEdits::default()),
        no_suggestion
    );
    assert_eq!(
        intents::accept_suggestion_choice(unknown.clone(), document.clone()),
        no_suggestion
    );
    assert_eq!(intents::undo_suggestion(unknown.clone()), no_suggestion);
    assert_eq!(
        intents::reply_to_suggestion(unknown.clone(), "Why?".to_owned()),
        no_suggestion
    );
    assert_eq!(
        intents::resolve_capture_duplicate(unknown.clone(), DuplicateChoice::Discard),
        no_suggestion
    );
    assert_eq!(
        intents::acknowledge_suggestion(unknown.clone()),
        Err(not_found("suggestion"))
    );
    assert_eq!(
        intents::resolve_link_or_create(
            unknown.clone(),
            LinkOrCreateChoice {
                kind: LinkOrCreateKind::Link,
                entity_id: Some(person.clone()),
                name: None,
                entity_kind: None,
                force: false,
            }
        ),
        Err(not_found("suggestion"))
    );
    assert_eq!(
        intents::resolve_conflict(
            unknown.clone(),
            ConflictResolution {
                kind: ResolutionKind::KeepMine,
                content: None,
                choices: vec![],
            }
        ),
        Err(not_found("conflict"))
    );
    assert_eq!(
        intents::resolve_duplicate(unknown.clone(), DuplicateChoice::CreateAnyway),
        Err(not_found("op"))
    );
    // Dismissing a rejection that is not there is a no-op (the banner may be stale).
    assert_eq!(intents::dismiss_rejection(unknown.clone()), Ok(()));
    // The capture has no AI proposal yet: nothing to accept or reject.
    assert_eq!(
        intents::accept_capture(capture.clone()),
        Err(invalid("capture", "needs_choice"))
    );
    assert_eq!(intents::reject_capture(capture.clone()), Ok(vec![]));
    assert_eq!(intents::accept_captures(vec![capture.clone()]), Ok(vec![]));
    assert_eq!(intents::accept_all_ready(), Ok(vec![]));

    // Recurrence helpers and filtered maps.
    assert_eq!(
        views::compose_recurrence(form.clone()),
        Ok(RecurrenceCompose {
            phrase: "every 2 weeks".to_owned(),
            understood: true,
            label: "Every 2 weeks".to_owned(),
        })
    );
    let monday = NaiveDate::from_ymd_opt(2026, 10, 5).expect("date");
    let item = |d: u32, label: &str, is_due: bool| RecurrencePreviewItem {
        date: NaiveDate::from_ymd_opt(2026, 10, d).expect("date"),
        label: label.to_owned(),
        is_due,
    };
    assert_eq!(
        views::recurrence_preview("every week".to_owned(), monday, 3),
        Ok(vec![
            item(5, "Mon 5 Oct 2026", true),
            item(12, "Mon 12 Oct", false),
            item(19, "Mon 19 Oct", false),
        ])
    );
    assert_eq!(
        views::recurrence_preview("sometimes".to_owned(), monday, 3),
        Err(invalid("recurrence", "not_understood"))
    );
    let people = views::global_graph_filtered(GraphFilter {
        edge_kinds: vec![],
        node_kinds: vec!["person".to_owned()],
        similarity: false,
        cluster: None,
        lens: GraphLens::People,
        focus: None,
        include_tags: false,
    })
    .expect("people map");
    assert_eq!((people.nodes.len(), people.edges.len()), (1, 0));

    // Reminders reported by the platform adapter.
    assert_eq!(
        strata_core::api::reminders::report_notification_result(7, NotificationResult::Ok),
        Ok(())
    );
    assert_eq!(
        strata_core::api::reminders::notification_action(
            7,
            NotificationAction {
                kind: NotificationActionKind::Done
            }
        ),
        Err(not_found("notification"))
    );

    // Online calls through the facade: server problems keep their status and type.
    let server = |status: u16, problem: &str| CoreFailure {
        status: Some(status),
        reason: Some(problem.to_owned()),
        ..failure("server")
    };
    rt.block_on(intents::refresh_settings()).expect("settings");
    assert_eq!(
        rt.block_on(intents::rename_device(unknown.clone(), "X".to_owned())),
        Err(server(404, "not_found"))
    );
    assert_eq!(
        rt.block_on(intents::revoke_device(unknown.clone())),
        Err(server(404, "not_found"))
    );
    assert_eq!(
        rt.block_on(intents::set_device_reminders(unknown.clone(), false)),
        Err(server(404, "not_found"))
    );
    // The note has not synced: the server does not know it yet.
    assert_eq!(
        rt.block_on(intents::refresh_history(note.clone())),
        Err(server(404, "not_found"))
    );
    let no_commit = "0000000000000000000000000000000000000000".to_owned();
    assert_eq!(
        rt.block_on(intents::revert_note(note.clone(), no_commit.clone())),
        Err(server(404, "not_found"))
    );
    assert_eq!(
        rt.block_on(views::note_revision_diff(note.clone(), no_commit)),
        Err(server(404, "not_found"))
    );
    // A map whose notes the server has not seen is refused.
    assert_eq!(
        rt.block_on(intents::save_layout(
            note.clone(),
            "Pricing".to_owned(),
            vec![NodePosition {
                id: note.clone(),
                x: 0.0,
                y: 0.0
            }]
        )),
        Err(server(422, "invalid_body"))
    );
    let zip = dir.path().join("vault.zip");
    let exported = rt
        .block_on(intents::export_vault(
            zip.to_str().expect("utf-8").to_owned(),
        ))
        .expect("export");
    assert_eq!(
        (exported.note_count, exported.label.as_str()),
        (0, "1.4 KB · 0 notes")
    );
    assert_eq!(
        rt.block_on(intents::import_vault(
            zip.to_str().expect("utf-8").to_owned()
        )),
        Ok(ImportSummary {
            imported: 0,
            skipped: 1
        })
    );

    // Admin → Users: Bob's sign-up waits; approve, promote, disable, reset, schedule, cancel.
    let users = rt
        .block_on(views::load_admin_users(String::new()))
        .expect("users");
    assert_eq!(
        users
            .pending
            .iter()
            .map(|u| (u.username.as_str(), u.status.as_str()))
            .collect::<Vec<_>>(),
        [("bob", "pending")]
    );
    let bob = users.pending[0].id.clone();
    let state_of = |r: Result<AdminUserItem, CoreFailure>| {
        r.map(|u| (u.role, u.status, u.password_change_required))
    };
    let st = |role: &str, status: &str, pcr: bool| Ok((role.to_owned(), status.to_owned(), pcr));
    assert_eq!(
        state_of(rt.block_on(intents::approve_user(bob.clone()))),
        st("member", "active", false)
    );
    assert_eq!(
        state_of(rt.block_on(intents::set_user_role(bob.clone(), "admin".to_owned()))),
        st("admin", "active", false)
    );
    assert_eq!(
        state_of(rt.block_on(intents::set_user_enabled(bob.clone(), false))),
        st("admin", "disabled", false)
    );
    assert_eq!(
        rt.block_on(intents::reset_password(bob.clone()))
            .map(|p| p.chars().count()),
        Ok(16)
    );
    assert_eq!(
        state_of(rt.block_on(intents::schedule_deletion(bob.clone()))),
        st("admin", "deletion_pending", true)
    );
    assert_eq!(
        state_of(rt.block_on(intents::cancel_deletion(bob.clone()))),
        st("admin", "active", true)
    );
    assert_eq!(
        rt.block_on(intents::reject_user(bob.clone())),
        Err(server(409, "account_state_conflict"))
    );
    assert_eq!(
        state_of(rt.block_on(intents::create_user(NewUserRequest {
            username: "carol".to_owned(),
            display_name: "Carol".to_owned(),
            password: "carol-password-1".to_owned(),
            role: "member".to_owned(),
        }))),
        st("member", "active", false)
    );

    // Remote search without an embedding model, Ask without the provider.
    let remote = rt
        .block_on(views::search_in_folder(
            "invoicing".to_owned(),
            SearchMode::Hybrid,
            Some("notes".to_owned()),
        ))
        .expect("search");
    assert_eq!(
        (remote.availability, remote.results.len()),
        (Availability::NotYetAvailable, 0)
    );
    assert_eq!(
        rt.block_on(intents::ask(
            "Anything?".to_owned(),
            AskScope {
                kind: AskScopeKind::All,
                value: None,
                label: "All notes".to_owned()
            }
        )),
        Err(server(503, "ai_unavailable"))
    );
    assert_eq!(
        views::ask_view()
            .expect("ask")
            .messages
            .iter()
            .map(|m| (m.role.as_str(), m.error_key.as_deref()))
            .collect::<Vec<_>>(),
        [("user", None), ("assistant", Some("error.server"))]
    );
    intents::stop_ask().expect("stop");
    intents::new_conversation().expect("new conversation");
    assert_eq!(views::ask_view().expect("ask").messages.len(), 0);
    assert_eq!(
        rt.block_on(intents::save_answer_as_note(unknown.clone())),
        Err(not_found("answer"))
    );
    rt.block_on(intents::refresh_ai_activity())
        .expect("activity");
    assert_eq!(
        rt.block_on(intents::reject_ai_decision(unknown.clone())),
        Err(server(404, "not_found"))
    );
    assert_eq!(
        rt.block_on(intents::repoint_ai_decision(
            unknown.clone(),
            person.clone(),
            None
        )),
        Err(server(404, "not_found"))
    );
    assert_eq!(
        rt.block_on(intents::retype_ai_decision(
            unknown.clone(),
            "related".to_owned()
        )),
        Err(server(404, "not_found"))
    );
    rt.block_on(intents::refresh_similarity())
        .expect("similarity");

    // Account settings through the facade.
    rt.block_on(app::set_display_name("Alice".to_owned()))
        .expect("name");
    rt.block_on(app::set_ui_language("en".to_owned()))
        .expect("language");
    rt.block_on(app::set_timezone("Africa/Cairo".to_owned()))
        .expect("zone");
    let export = dir.path().join("me.zip");
    assert_eq!(
        rt.block_on(app::download_export(
            export.to_str().expect("utf-8").to_owned()
        ))
        .map(|e| e.note_count),
        Ok(0)
    );
    let pending_ops = match rt.block_on(app::delete_account_now(false)) {
        Err(CoreFailure {
            code,
            count: Some(n),
            ..
        }) if code == "pending_changes" => n,
        other => panic!("{other:?}"),
    };
    // "Check again" is only for a sign-up kept in memory; this account signed in normally.
    assert_eq!(
        rt.block_on(app::check_approval()),
        Err(invalid("password", "required"))
    );
    assert_eq!(
        app::dismiss_pending().map(|s| s.kind),
        Ok(SessionKind::Active)
    );
    assert_eq!(
        app::switch_account(unknown.clone()).map(|s| s.kind),
        Err(not_found("account"))
    );
    assert_eq!(
        rt.block_on(app::change_password(
            world::password("alice"),
            "alice-password-2".to_owned()
        ))
        .map(|s| s.kind),
        Ok(SessionKind::Active)
    );

    intents::set_default_reminder_time("08:30".to_owned()).expect("default time");
    intents::set_quiet_hours(true, "23:00".to_owned(), "06:30".to_owned()).expect("quiet");
    intents::set_snooze_minutes(30).expect("snooze");
    intents::set_reminders_enabled(false).expect("reminders");

    // Signing out with unsynced changes asks first; the unsynced ops can be exported.
    let asked = rt.block_on(app::sign_out(false)).expect("sign out");
    // The four reminder settings add one settings op.
    assert_eq!(
        (asked.signed_out, asked.unsynced_ops),
        (false, pending_ops + 1)
    );
    let unsynced = dir.path().join("unsynced.md");
    assert_eq!(
        app::export_unsynced(unsynced.to_str().expect("utf-8").to_owned()),
        Ok(pending_ops + 1)
    );
    let text = std::fs::read_to_string(&unsynced).expect("written");
    assert!(text.starts_with("# Unsynced changes\n\n## "), "{text}");
    assert!(
        text.contains("Weekly invoicing for [[Acme]]. #client"),
        "{text}"
    );
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
