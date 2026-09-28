//! The online intents of the client core against the real server (PLAN §12.6, §12.7): devices,
//! the account (`PATCH /me`, export, deletion), note history and revert, server search, vault
//! export and import, Admin → Users, the AI activity feed, similarity, saved map layouts and
//! Ask — each through the generated client to the production app served in process, with the
//! results the screens show asserted exactly.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

#[path = "support/world.rs"]
mod world;

use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::store::notes;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::model::{
    AdminUserItem, Availability, DiffLine, DiffLineKind, HighlightSpan, HistoryEntry, ImportSummary,
    NewUserRequest, NodePosition, NoteDiffView, SearchMode, SearchView, SessionKind, SignUpRequest,
    TextDir,
};
use strata_index::types::UserRole;
use world::World;

fn dbg_print<T: std::fmt::Debug>(label: &str, v: &T) {
    println!("=== {label}\n{v:#?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn devices_are_listed_renamed_muted_and_revoked() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let laptop = w.device(1);
    let phone = w.device(2);
    let s1 = laptop.sign_in("alice").await;
    let s2 = phone.sign_in("alice").await;

    s1.refresh_settings().await.expect("refresh");
    let devices = |s: &strata_core::session::Session| {
        s.read(|c, ctx| build::settings_view(c, ctx))
            .expect("settings")
            .expect("signed in")
            .device_list
            .into_iter()
            .map(|d| (d.id, d.name, d.is_this_device, d.reminders_enabled))
            .collect::<Vec<_>>()
    };
    const LAPTOP: &str = "01M3HBS0G00000000000000003";
    const PHONE: &str = "01M3HBS0G00000000000000005";
    assert_eq!(
        devices(&s1),
        [
            (LAPTOP.to_owned(), "device 1".to_owned(), true, true),
            (PHONE.to_owned(), "device 2".to_owned(), false, true),
        ]
    );

    assert_eq!(
        s1.rename_device(PHONE, "  ").await,
        Err(CoreError::InvalidInput {
            field: "name".into(),
            reason: "empty".into()
        })
    );
    s1.rename_device(PHONE, " Pixel ").await.expect("rename");
    s1.set_device_reminders(PHONE, false)
        .await
        .expect("mute the phone");
    assert_eq!(
        devices(&s1),
        [
            (LAPTOP.to_owned(), "device 1".to_owned(), true, true),
            (PHONE.to_owned(), "Pixel".to_owned(), false, false),
        ]
    );
    // This device's reminders go through the outbox (a settings op), not `PATCH /devices`.
    s1.set_device_reminders(LAPTOP, false)
        .await
        .expect("mute this device");
    s1.sync(Trigger::AfterWrite).await.expect("push");
    s1.refresh_settings().await.expect("refresh");
    assert_eq!(
        devices(&s1),
        [
            (LAPTOP.to_owned(), "device 1".to_owned(), true, false),
            (PHONE.to_owned(), "Pixel".to_owned(), false, false),
        ]
    );

    assert_eq!(
        s1.revoke_device(LAPTOP).await,
        Err(CoreError::InvalidInput {
            field: "device".into(),
            reason: "this_device".into()
        })
    );
    s1.revoke_device(PHONE).await.expect("revoke");
    assert_eq!(
        devices(&s1),
        [(LAPTOP.to_owned(), "device 1".to_owned(), true, false)]
    );
    // The revoked device's session is gone.
    assert_eq!(
        s2.refresh_settings().await,
        Err(CoreError::SessionExpired)
    );
    assert_eq!(
        phone.core.state().expect("state").kind,
        SessionKind::SignedOut
    );
    // Unknown devices are the server's 404.
    assert_eq!(
        s1.rename_device("01M3HBS0G0000000000000ZZZZ", "X").await,
        Err(CoreError::Server {
            status: 404,
            problem_type: "not_found".into()
        })
    );
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_account_is_changed_exported_and_deleted_through_me() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let d = w.device(1);
    let s = d.sign_in("alice").await;

    assert_eq!(
        d.core.set_display_name(" ").await,
        Err(CoreError::InvalidInput {
            field: "display_name".into(),
            reason: "empty".into()
        })
    );
    assert_eq!(
        d.core.set_ui_language("fr").await,
        Err(CoreError::InvalidInput {
            field: "ui_language".into(),
            reason: "unknown".into()
        })
    );
    assert_eq!(
        d.core.set_timezone("Mars/Olympus").await,
        Err(CoreError::InvalidInput {
            field: "timezone".into(),
            reason: "unknown".into()
        })
    );
    d.core.set_display_name(" Alice A. ").await.expect("name");
    d.core.set_ui_language("ar").await.expect("language");
    d.core.set_timezone("Africa/Cairo").await.expect("zone");
    let account = s
        .read(|c, ctx| build::settings_view(c, ctx))
        .expect("settings")
        .expect("signed in")
        .account;
    assert_eq!(
        (
            account.display_name.as_str(),
            account.ui_language.as_str(),
            account.timezone.as_str(),
            account.initials.as_str()
        ),
        ("Alice A.", "ar", "Africa/Cairo", "AA")
    );

    assert_eq!(
        d.core.change_password("alice-password-1", "short").await,
        Err(CoreError::InvalidInput {
            field: "new_password".into(),
            reason: "too_short".into()
        })
    );
    let wrong = d
        .core
        .change_password("not-my-password", "a-new-password-2")
        .await
        .expect_err("wrong current password");
    assert_eq!(
        wrong,
        CoreError::Server {
            status: 422,
            problem_type: "invalid_body".into()
        }
    );
    let state = d
        .core
        .change_password("alice-password-1", "a-new-password-2")
        .await
        .expect("changed");
    assert_eq!(state.kind, SessionKind::Active);

    // The account export: a zip of the vault written where the user chose.
    let id = s
        .create_note("notes/Pricing.md", "Weekly invoicing.\n", false)
        .expect("create")
        .id
        .expect("id");
    s.sync(Trigger::AfterWrite).await.expect("push");
    let path = d.dir.path().join("export.zip");
    let summary = d
        .core
        .download_export(path.to_str().expect("utf-8"))
        .await
        .expect("export");
    assert_eq!(
        (summary.note_count, summary.label.as_str()),
        (1, "2.1 ك.ب · ملاحظة واحدة"),
        "labels follow the account's language"
    );
    assert_eq!(
        std::fs::metadata(&path).expect("written").len(),
        summary.size_bytes
    );
    assert_eq!(
        d.core.download_export(" ").await,
        Err(CoreError::InvalidInput {
            field: "path".into(),
            reason: "empty".into()
        })
    );

    // "Delete now" is refused while ops are unsynced, then confirmed on the server.
    s.update_note(&id, "Weekly invoicing, net 30.\n")
        .expect("edit");
    assert_eq!(
        d.core.delete_account_now(false).await,
        Err(CoreError::PendingChanges { count: 1 })
    );
    // Only an account scheduled for deletion can confirm it.
    assert_eq!(
        d.core.delete_account_now(true).await,
        Err(CoreError::Server {
            status: 409,
            problem_type: "account_state_conflict".into()
        })
    );
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_diff_and_revert_come_from_the_server() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let d = w.device(1);
    let s = d.sign_in("alice").await;
    let id = s
        .create_note("notes/Pricing.md", "First.\nSecond.\n", false)
        .expect("create")
        .id
        .expect("id");
    s.sync(Trigger::AfterWrite).await.expect("push");
    let v1 = s.read(|c, _| notes::current(c, &id)).expect("read").expect("note").content;
    s.update_note(&id, &v1.replace("Second.", "Second, edited."))
        .expect("edit");
    s.sync(Trigger::AfterWrite).await.expect("push");

    s.refresh_history(&id).await.expect("history");
    let screen = s
        .read(|c, ctx| build::note_screen(c, ctx, &id))
        .expect("screen");
    let note = screen.note.expect("note");
    const V1: &str = "3766da684d88f7e9d3594e674bf39ad8a4bfcb36";
    const V2: &str = "7f6633140713199143c7388d60f6f35f8b63a575";
    let at: chrono::DateTime<chrono::Utc> = "2026-09-27T12:00:00Z".parse().expect("time");
    assert_eq!(
        (note.version_label.as_deref(), note.edited_by.as_deref()),
        (Some("v2"), Some("alice"))
    );
    assert_eq!(
        note.history_entries,
        [
            HistoryEntry {
                commit: V2.into(),
                version_label: "v2".into(),
                message: "user: update notes/Pricing.md".into(),
                author: "user".into(),
                at,
                at_label: "Today 12:00".into(),
                can_revert: false,
            },
            HistoryEntry {
                commit: V1.into(),
                version_label: "v1".into(),
                message: "user: create notes/Pricing.md".into(),
                author: "user".into(),
                at,
                at_label: "Today 12:00".into(),
                can_revert: true,
            },
        ]
    );
    let diff = s.note_revision_diff(&id, V1).await.expect("diff");
    let same = |n: u32, text: &str, dir: TextDir| DiffLine {
        kind: DiffLineKind::Same,
        old_line: Some(n),
        new_line: Some(n),
        text: text.into(),
        dir,
    };
    assert_eq!(
        diff,
        NoteDiffView {
            note_id: id.clone(),
            commit: V1.into(),
            summary: "1 changed".into(),
            lines: vec![
                same(1, "---", TextDir::Neutral),
                same(2, &format!("id: {id}"), TextDir::Ltr),
                same(3, "created: 2026-09-27T12:00:00Z", TextDir::Ltr),
                same(4, "updated: 2026-09-27T12:00:00Z", TextDir::Ltr),
                same(5, "---", TextDir::Neutral),
                same(6, "First.", TextDir::Ltr),
                DiffLine {
                    kind: DiffLineKind::Removed,
                    old_line: Some(7),
                    new_line: None,
                    text: "Second.".into(),
                    dir: TextDir::Ltr,
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    old_line: None,
                    new_line: Some(7),
                    text: "Second, edited.".into(),
                    dir: TextDir::Ltr,
                },
            ],
        }
    );
    let first = V1;
    s.revert_note(&id, &first).await.expect("revert");
    s.sync(Trigger::Manual).await.expect("pull");
    assert_eq!(
        s.read(|c, _| notes::current(c, &id)).expect("read").expect("note").content,
        v1
    );
    assert_eq!(
        s.note_revision_diff(&id, "0000000000000000000000000000000000000000")
            .await,
        Err(CoreError::Server {
            status: 404,
            problem_type: "not_found".into()
        })
    );
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_search_export_and_import() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    w.account("bob", UserRole::Member).await;
    let d = w.device(1);
    let s = d.sign_in("alice").await;
    s.create_note("notes/Pricing.md", "Weekly invoicing for Acme.\n", false)
        .expect("create");
    s.create_note("notes/Travel.md", "Cairo in October.\n", false)
        .expect("create");
    s.sync(Trigger::AfterWrite).await.expect("push");

    let empty = s
        .search_remote("  ", SearchMode::Hybrid, None)
        .await
        .expect("empty query");
    assert_eq!(
        empty,
        SearchView {
            query: "  ".into(),
            mode: SearchMode::Hybrid,
            results: Vec::new(),
            availability: Availability::Available,
            available_modes: vec![SearchMode::Keyword, SearchMode::Semantic, SearchMode::Hybrid],
            folder: None,
        }
    );
    let keyword = s
        .search_remote("invoicing", SearchMode::Keyword, None)
        .await
        .expect("keyword");
    assert_eq!(keyword.availability, Availability::Available);
    assert_eq!(
        keyword
            .results
            .iter()
            .map(|h| (
                h.title.as_str(),
                h.path.as_str(),
                h.kind.as_str(),
                h.snippet.as_str(),
                h.highlights.clone()
            ))
            .collect::<Vec<_>>(),
        [(
            "Pricing",
            "notes/Pricing.md",
            "note",
            "Weekly invoicing for Acme.",
            vec![HighlightSpan { start: 7, end: 16 }]
        )]
    );
    let semantic = s
        .search_remote("invoicing", SearchMode::Semantic, Some("notes".into()))
        .await
        .expect("semantic");
    // No embedding model on this server: semantic search is "not yet available", not an error.
    assert_eq!(
        (semantic.availability, semantic.results.len(), semantic.folder.as_deref()),
        (Availability::NotYetAvailable, 0, Some("notes"))
    );

    let path = d.dir.path().join("vault.zip");
    let summary = s
        .export_vault(path.to_str().expect("utf-8"))
        .await
        .expect("export");
    assert_eq!(
        (summary.note_count, summary.label.as_str()),
        (2, "1.7 KB · 2 notes")
    );
    assert_eq!(
        std::fs::metadata(&path).expect("written").len(),
        summary.size_bytes
    );
    assert_eq!(
        s.export_vault("").await,
        Err(CoreError::InvalidInput {
            field: "path".into(),
            reason: "empty".into()
        })
    );

    // Bob imports Alice's export.
    let bob_device = w.device(2);
    let b = bob_device.sign_in("bob").await;
    let imported = b
        .import_vault(path.to_str().expect("utf-8"))
        .await
        .expect("import");
    assert_eq!(
        imported,
        ImportSummary {
            imported: 2,
            skipped: 1
        }
    );
    b.sync(Trigger::Manual).await.expect("pull");
    let titles = b
        .read(|c, _| notes::live_paths(c))
        .expect("paths");
    assert_eq!(
        titles.into_iter().map(|(_, p)| p).collect::<Vec<_>>(),
        ["notes/Pricing.md", "notes/Travel.md"]
    );
    assert!(matches!(
        b.import_vault(d.dir.path().join("absent.zip").to_str().expect("utf-8"))
            .await,
        Err(CoreError::Storage(m)) if m.starts_with("io: ")
    ));
    w.finish().await;
}

/// `(username, role, status, is_self, deletion label, password change required)` of a row.
fn row(u: &AdminUserItem) -> (String, String, String, bool, Option<String>, bool) {
    (
        u.username.clone(),
        u.role.clone(),
        u.status.clone(),
        u.is_self,
        u.deletion_label.clone(),
        u.password_change_required,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admins_manage_accounts_from_the_users_screen() {
    let w = World::new().await;
    let root_id = w.account("root", UserRole::Admin).await;
    let alice_id = w.account("alice", UserRole::Member).await;
    let admin_device = w.device(1);
    let admin = admin_device.sign_in("root").await;
    let alice_device = w.device(2);
    let alice = alice_device.sign_in("alice").await;

    // Members are not allowed in.
    let denied = alice.admin_users("").await.expect("view");
    assert_eq!(
        (denied.availability, denied.users.len()),
        (Availability::NotAllowed, 0)
    );
    assert_eq!(
        alice.approve_user(&root_id).await,
        Err(CoreError::InvalidInput {
            field: "account".into(),
            reason: "not_admin".into()
        })
    );

    // A sign-up waits for approval.
    let carol_device = w.device(3);
    carol_device
        .core
        .sign_up(SignUpRequest {
            server_url: carol_device.url.clone(),
            username: "carol".into(),
            password: "carol-password-1".into(),
            display_name: "Carol".into(),
        })
        .await
        .expect("sign up");
    let dave_device = w.device(4);
    dave_device
        .core
        .sign_up(SignUpRequest {
            server_url: dave_device.url.clone(),
            username: "dave".into(),
            password: "dave-password-1".into(),
            display_name: "Dave".into(),
        })
        .await
        .expect("sign up");
    let view = admin.admin_users("").await.expect("users");
    dbg_print("users", &view);
    let carol_id = view.pending.iter().find(|u| u.username == "carol").expect("carol").id.clone();
    let dave_id = view.pending.iter().find(|u| u.username == "dave").expect("dave").id.clone();
    let filtered = admin.admin_users("ALI").await.expect("filtered");
    assert_eq!(
        filtered.users.iter().map(|u| u.username.as_str()).collect::<Vec<_>>(),
        ["alice"]
    );

    let approved = admin.approve_user(&carol_id).await.expect("approve");
    assert_eq!(row(&approved), ("carol".into(), "member".into(), "active".into(), false, None, false));
    let rejected = admin.reject_user(&dave_id).await.expect("reject");
    assert_eq!(row(&rejected), ("dave".into(), "member".into(), "rejected".into(), false, None, false));

    // Role, status and password changes; never on the admin's own account.
    assert_eq!(
        admin.set_user_role(&root_id, "member").await,
        Err(CoreError::InvalidInput {
            field: "user".into(),
            reason: "self".into()
        })
    );
    assert_eq!(
        admin.set_user_role(&alice_id, "owner").await,
        Err(CoreError::InvalidInput {
            field: "role".into(),
            reason: "unknown".into()
        })
    );
    let promoted = admin.set_user_role(&carol_id, "admin").await.expect("role");
    assert_eq!(row(&promoted), ("carol".into(), "admin".into(), "active".into(), false, None, false));
    let disabled = admin.set_user_enabled(&carol_id, false).await.expect("disable");
    assert_eq!(disabled.status, "disabled");
    let enabled = admin.set_user_enabled(&carol_id, true).await.expect("enable");
    assert_eq!(enabled.status, "active");
    let temporary = admin.reset_password(&carol_id).await.expect("reset");
    dbg_print("temporary", &temporary);

    // New accounts.
    assert_eq!(
        admin
            .create_user(NewUserRequest {
                username: "erin".into(),
                display_name: "Erin".into(),
                password: "short".into(),
                role: "member".into(),
            })
            .await,
        Err(CoreError::InvalidInput {
            field: "password".into(),
            reason: "too_short".into()
        })
    );
    assert_eq!(
        admin
            .create_user(NewUserRequest {
                username: "erin".into(),
                display_name: "Erin".into(),
                password: "erin-password-1".into(),
                role: "guest".into(),
            })
            .await,
        Err(CoreError::InvalidInput {
            field: "role".into(),
            reason: "unknown".into()
        })
    );
    let erin = admin
        .create_user(NewUserRequest {
            username: "erin".into(),
            display_name: "Erin".into(),
            password: "erin-password-1".into(),
            role: "member".into(),
        })
        .await
        .expect("create");
    assert_eq!(row(&erin), ("erin".into(), "member".into(), "active".into(), false, None, false));
    let taken = admin
        .create_user(NewUserRequest {
            username: "ERIN".into(),
            display_name: "Erin".into(),
            password: "erin-password-1".into(),
            role: "member".into(),
        })
        .await;
    dbg_print("taken", &taken);

    // Scheduled deletion: the member's session becomes export-only, and "Delete now" wipes
    // the device's copy once the server confirmed.
    assert_eq!(
        admin.schedule_deletion(&root_id).await,
        Err(CoreError::InvalidInput {
            field: "user".into(),
            reason: "self".into()
        })
    );
    let scheduled = admin.schedule_deletion(&alice_id).await.expect("schedule");
    dbg_print("scheduled", &scheduled);
    let cancelled = admin.cancel_deletion(&alice_id).await.expect("cancel");
    assert_eq!(row(&cancelled), ("alice".into(), "member".into(), "active".into(), false, None, false));
    admin.schedule_deletion(&alice_id).await.expect("schedule again");
    let state = alice_device.core.refresh_account().await;
    dbg_print("alice refresh", &state);
    dbg_print("alice state", &alice_device.core.state());
    let after = alice_device.core.delete_account_now(true).await;
    dbg_print("delete now", &after);

    let everyone = admin.admin_users("").await.expect("users");
    dbg_print("everyone", &everyone.users.iter().map(row).collect::<Vec<_>>());
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_activity_similarity_and_saved_layouts() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let d = w.device(1);
    let s = d.sign_in("alice").await;
    let a = s
        .create_note("notes/Pricing.md", "Weekly invoicing. [[Acme]]\n", false)
        .expect("create")
        .id
        .expect("id");
    s.create_note("notes/Acme.md", "A client.\n", false)
        .expect("create");
    s.sync(Trigger::AfterWrite).await.expect("push");

    s.refresh_ai_activity().await.expect("activity");
    s.refresh_similarity().await.expect("similarity");
    let unknown = "01M3HBS0G0000000000000ZZZZ";
    let e = s.reject_ai_decision(unknown).await;
    dbg_print("reject unknown", &e);
    let e = s.repoint_ai_decision(unknown, &a, Some("the client".into())).await;
    dbg_print("repoint unknown", &e);
    assert_eq!(
        s.retype_ai_decision(unknown, "likes").await,
        Err(CoreError::InvalidInput {
            field: "rel_type".into(),
            reason: "unknown_relation".into()
        })
    );
    let e = s.retype_ai_decision(unknown, "related").await;
    dbg_print("retype unknown", &e);
    let e = s.reject_ai_decision("not-an-id").await;
    dbg_print("reject bad id", &e);

    assert_eq!(
        s.save_layout(&a, " / ", &[]).await,
        Err(CoreError::InvalidInput {
            field: "name".into(),
            reason: "empty".into()
        })
    );
    let path = s
        .save_layout(
            &a,
            "Pricing map",
            &[NodePosition {
                id: a.clone(),
                x: 10.0,
                y: -20.0,
            }],
        )
        .await
        .expect("save");
    assert_eq!(path, "maps/Pricing map.canvas");
    // Saving again replaces the map at its current version.
    let again = s
        .save_layout(&a, "Pricing map", &[])
        .await
        .expect("replace");
    assert_eq!(again, "maps/Pricing map.canvas");
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_streams_an_answer_into_the_conversation_and_saves_it() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let d = w.device(1);
    let s = d.sign_in("alice").await;
    s.create_note(
        "notes/Call.md",
        "Acme prefers weekly invoicing. ^inv\n",
        false,
    )
    .expect("create");
    s.sync(Trigger::AfterWrite).await.expect("push");

    assert_eq!(
        s.ask("  ", None, "All notes").await,
        Err(CoreError::InvalidInput {
            field: "question".into(),
            reason: "empty".into()
        })
    );
    // Without a recorded answer the provider fails before streaming: the answer shows the
    // server's error.
    let failed = s.ask("What does Acme prefer?", None, "All notes").await;
    dbg_print("failed ask", &failed);
    dbg_print("entries after failure", &s.ask_entries());

    let call = w.llm.calls().last().cloned().expect("recorded call");
    w.llm.push(
        &call.prompt,
        &call.input_hash,
        strata_testkit::Fixture::stream(&["Weekly ", "invoicing [[Call#^inv]]."]),
    );
    s.new_conversation();
    let id = s
        .ask("What does Acme prefer?", None, "All notes")
        .await
        .expect("ask");
    dbg_print("entries", &s.ask_entries());
    let view = s.read(|c, ctx| build::ask(c, ctx, &s.ask_entries())).expect("view");
    dbg_print("view", &view);
    let note = s.save_answer_as_note(&id).await.expect("save");
    dbg_print("saved", &note);
    assert_eq!(
        s.save_answer_as_note("local-9").await,
        Err(CoreError::NotFound {
            what: "answer".into()
        })
    );
    s.stop_ask();
    w.finish().await;
}
