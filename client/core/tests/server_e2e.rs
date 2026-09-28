//! The client core against the real server (PLAN §16.4): the production app (auth, vault,
//! sync, event bus; fake clock, per-test `PostgreSQL` database) served in-process by
//! `TestServer`, and real client cores — each with its own app-data directory — talking to it
//! through the generated client exactly as the app does. Covers bootstrap, incremental
//! changes, push (applied, merged, conflict, duplicate), the `/events` stream (resume and
//! `account.disabled` mid-stream), rebasing queued ops after a pull, and the admin and
//! account intents.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;
use std::time::Duration;

use actix_web::web;
use pretty_assertions::assert_eq;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::events::BusConfig;
use strata_api::sync::SyncConfig;
use strata_api::testing::TestServer;
use strata_common::Config;
use strata_common::clock::Clock as _;
use strata_common::config::Argon2Config;
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::client::{BrokenSyncApi, ClientAccountApi, ClientEventsApi, ClientSyncApi};
use strata_core::net::{EventSignal, EventStream, SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv, NotifyHub, Session};
use strata_core::store::{StorePaths, notes, outbox};
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::{Op, Version};
use strata_core::view::build;
use strata_core::view::model::{
    DuplicateChoice, HunkChoiceKind, NoteSyncKind, Platform, SessionKind, SignInRequest,
};
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};
use tempfile::TempDir;

/// The server world of one test.
struct World {
    db: TestDb,
    _data: TempDataRoot,
    state: web::Data<AuthState>,
    server: TestServer,
}

impl World {
    async fn new() -> Self {
        let db = TestDb::new().await.expect("test database");
        let data = TempDataRoot::new().expect("data root");
        let mut config = Config {
            data_root: data.path().to_path_buf(),
            ..Config::default()
        };
        config.auth.argon2 = Argon2Config {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        };
        let clock = db.clock.clone();
        let vault = VaultService::new(
            VaultConfig::new(data.path()),
            db.app_db.clone(),
            Arc::new(clock.clone()),
            db.ids.clone(),
        );
        let keys = SigningKeys::from_pem(&generate_key_pem().expect("key")).expect("key");
        let state = web::Data::new(
            AuthState::new(
                AuthDeps {
                    accounts_pool: db.accounts.clone(),
                    app_db: db.app_db.clone(),
                    issuer: db.issuer.clone(),
                    keys,
                    vaults: Arc::new(vault.clone()),
                    clock: Arc::new(clock),
                    ids: db.ids.clone(),
                },
                &config,
            )
            .expect("auth state"),
        );
        let (bus, sync) =
            strata_api::sync::install(&vault, BusConfig::default(), SyncConfig::default());
        let (for_server, vault_data) = (state.clone(), web::Data::new(vault));
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            cfg.app_data(bus.clone());
            cfg.app_data(sync.clone());
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            _data: data,
            state,
            server,
        }
    }

    /// Creates an active account; returns its ID.
    async fn account(&self, name: &str, role: UserRole) -> String {
        self.state
            .create_account(
                &NewAccount {
                    username: name,
                    display_name: name,
                    password: &password(name),
                    role,
                },
                None,
            )
            .await
            .expect("account")
            .id
            .to_string()
    }

    /// A device: a real client core with its own app-data directory, on the server's clock.
    fn device(&self, n: u64) -> Device {
        let dir = TempDir::new().expect("temp dir");
        let now = self.db.clock.now();
        let clock = FakeClock::at(&now.to_rfc3339());
        let base_ms = u64::try_from(now.timestamp_millis()).expect("ms") + n * 60_000;
        let env = CoreEnv {
            paths: StorePaths::new(dir.path()),
            platform: Platform::Linux,
            clock: Arc::new(clock),
            ids: Arc::new(SeqIds::new(base_ms)),
            account_api: Arc::new(ClientAccountApi {}),
            events_api: Arc::new(ClientEventsApi {}),
            notifications: NotifyHub::default(),
            sync_api: Arc::new(|url: &str, tokens: Tokens| -> Arc<dyn SyncApi> {
                // As in production: an address the client cannot use gets a transport that
                // fails every call.
                match ClientSyncApi::new(url, tokens) {
                    Ok(api) => Arc::new(api),
                    Err(e) => Arc::new(BrokenSyncApi(e)),
                }
            }),
            default_device_name: format!("device {n}"),
            server_url: self.server.base_url(),
            device_timezone: None,
        };
        Device {
            core: Core::open(env).expect("core"),
            url: self.server.base_url(),
            name: format!("device {n}"),
            _dir: dir,
        }
    }

    async fn finish(self) {
        drop(self.server);
        drop(self.state);
        self.db.cleanup().await.expect("cleanup");
    }
}

fn password(name: &str) -> String {
    format!("{name}-password-1")
}

struct Device {
    core: Core,
    url: String,
    name: String,
    _dir: TempDir,
}

impl Device {
    async fn sign_in_with(&self, name: &str, password: &str) -> Arc<Session> {
        self.core
            .sign_in(SignInRequest {
                username: name.to_owned(),
                password: password.to_owned(),
                device_name: self.name.clone(),
            })
            .await
            .expect("sign in");
        self.core.session().expect("session")
    }

    async fn sign_in(&self, name: &str) -> Arc<Session> {
        let s = self.sign_in_with(name, &password(name)).await;
        s.sync(Trigger::Start).await.expect("bootstrap");
        s
    }
}

fn content(s: &Session, id: &str) -> String {
    s.read(|c, _| notes::current(c, id))
        .expect("read")
        .expect("note exists")
        .content
}

async fn next_signal(stream: &mut Box<dyn EventStream>) -> EventSignal {
    tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("an event within 10 s")
        .expect("stream open")
        .expect("event")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_devices_sync_through_the_real_server() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let a = w.device(1);
    let b = w.device(2);
    let sa = a.sign_in("alice").await;

    // Push: a created note is applied and becomes the verified base.
    let id = sa
        .create_note("notes/Pricing.md", "First.\nSecond.\nThird.\n", false)
        .expect("create")
        .id
        .expect("created");
    let report = sa.sync(Trigger::AfterWrite).await.expect("push");
    assert_eq!(report.pushed, 1);
    let base = content(&sa, &id);
    assert!(base.starts_with(&format!("---\nid: {id}\n")), "{base}");
    assert!(base.ends_with("First.\nSecond.\nThird.\n"), "{base}");
    assert_eq!(sa.read(|c, _| outbox::all(c)).expect("outbox"), []);

    // Bootstrap: the second device gets the note.
    let sb = b.sign_in("alice").await;
    assert_eq!(content(&sb, &id), base);

    // Stale, non-overlapping edits: the server merges (D19) and both converge via changes.
    sb.update_note(&id, &base.replace("Third.", "Third (B)."))
        .expect("edit b");
    sb.sync(Trigger::AfterWrite).await.expect("sync b");
    sa.update_note(&id, &base.replace("First.", "First (A)."))
        .expect("edit a");
    sa.sync(Trigger::AfterWrite).await.expect("sync a");
    let merged = base
        .replace("First.", "First (A).")
        .replace("Third.", "Third (B).");
    assert_eq!(content(&sa, &id), merged);
    sb.sync(Trigger::Manual).await.expect("pull b");
    assert_eq!(content(&sb, &id), merged);

    // Overlapping edits: the late device gets a conflict (its text stays visible).
    sb.update_note(&id, &merged.replace("Second.", "Second (B)."))
        .expect("edit b");
    sb.sync(Trigger::AfterWrite).await.expect("sync b");
    let mine = merged.replace("Second.", "Second (A).");
    let op = sa.update_note(&id, &mine).expect("edit a");
    sa.sync(Trigger::AfterWrite).await.expect("sync a");
    let note = sa
        .read(|c, ctx| build::note_screen(c, ctx, &id))
        .expect("screen")
        .note
        .expect("note");
    assert_eq!(note.sync.kind, NoteSyncKind::Conflict);
    assert_eq!(content(&sa, &id), mine);
    let detail = sa
        .read(|c, ctx| build::conflict_screen(c, ctx, &op))
        .expect("conflict")
        .conflict
        .expect("conflict exists");
    assert_eq!(
        detail.server.as_deref(),
        Some(merged.replace("Second.", "Second (B).").as_str())
    );
    assert_eq!(detail.hunks.len(), 1);
    assert!(
        detail
            .hunks
            .iter()
            .all(|h| h.allowed_choices.contains(&HunkChoiceKind::Ours))
    );

    // Duplicate: B creates a note matching one it has not pulled yet; the server answers with
    // candidates, and "create anyway" retries with force.
    sa.create_note(
        "notes/ETA invoice for Watanya.md",
        "Monthly invoice\n",
        true,
    )
    .expect("create a");
    sa.sync(Trigger::AfterWrite).await.expect("sync a");
    let dup = sb
        .create_note("notes/Watanya ETA invoice.md", "Monthly invoice\n", false)
        .expect("create b")
        .id
        .expect("no local candidate");
    sb.sync(Trigger::AfterWrite).await.expect("sync b");
    let prompts = sb.read(build::duplicate_prompts).expect("prompts").prompts;
    assert_eq!(prompts.len(), 1);
    assert_eq!(
        prompts[0]
            .candidates
            .iter()
            .map(|c| c.title.as_str())
            .collect::<Vec<_>>(),
        ["ETA invoice for Watanya"]
    );
    sb.resolve_duplicate(&prompts[0].op_id, DuplicateChoice::CreateAnyway)
        .expect("create anyway");
    sb.sync(Trigger::Manual).await.expect("sync b");
    assert!(sb.note_exists(&dup).expect("exists"));
    assert_eq!(
        sb.read(build::duplicate_prompts).expect("prompts").prompts,
        []
    );
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_edits_are_rebased_after_a_pull_and_fast_forward() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let a = w.device(1);
    let b = w.device(2);
    let sa = a.sign_in("alice").await;
    let id = sa
        .create_note("notes/Plan.md", "First.\nSecond.\nThird.\n", false)
        .expect("create")
        .id
        .expect("created");
    sa.sync(Trigger::AfterWrite).await.expect("push");
    let base = content(&sa, &id);
    let sb = b.sign_in("alice").await;

    // A queues two chained edits without pushing; B changes the third line meanwhile.
    let e1 = base.replace("First.", "First (A).");
    let e2 = e1.replace("Second.", "Second (A).");
    sa.update_note(&id, &e1).expect("edit 1");
    sa.update_note(&id, &e2).expect("edit 2");
    let theirs = base.replace("Third.", "Third (B).");
    sb.update_note(&id, &theirs).expect("edit b");
    sb.sync(Trigger::AfterWrite).await.expect("sync b");

    // A pull rewrites A's queue on top of B's version.
    sa.pull().await.expect("pull");
    let queued = sa.read(|c, _| outbox::all(c)).expect("outbox");
    let expected = theirs
        .replace("First.", "First (A).")
        .replace("Second.", "Second (A).");
    let last = match &queued.last().expect("queued").op {
        Op::NoteUpdate(u) => u.content.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(last, expected);
    assert_eq!(
        queued[0].base_version,
        Some(Version::of_text(&content(&sb, &id)))
    );
    assert_eq!(content(&sa, &id), expected);

    // The push applies without a conflict; B converges.
    sa.sync(Trigger::Manual).await.expect("push");
    assert_eq!(sa.read(|c, _| outbox::all(c)).expect("outbox"), []);
    let note = sa
        .read(|c, ctx| build::note_screen(c, ctx, &id))
        .expect("screen")
        .note
        .expect("note");
    assert_eq!(note.sync.kind, NoteSyncKind::Synced);
    sb.sync(Trigger::Manual).await.expect("pull b");
    assert_eq!(content(&sb, &id), expected);
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn events_resume_from_the_saved_seq_and_account_disabled_ends_the_session() {
    let w = World::new().await;
    let alice_id = w.account("alice", UserRole::Member).await;
    w.account("root", UserRole::Admin).await;
    let a = w.device(1);
    let b = w.device(2);
    let admin = w.device(3);
    let sa = a.sign_in("alice").await;
    let sb = b.sign_in("alice").await;

    let mut stream = sa.subscribe_events().expect("subscribe");
    let id = sb
        .create_note("notes/From B.md", "Hello.\n", false)
        .expect("create")
        .id
        .expect("created");
    sb.sync(Trigger::AfterWrite).await.expect("push b");
    let first = next_signal(&mut stream).await;
    let EventSignal::Changed { seq } = first else {
        panic!("expected a change, got {first:?}");
    };
    assert!(sa.handle_event(&first).expect("handled"), "a change pulls");
    sa.pull().await.expect("pull");
    assert!(sa.note_exists(&id).expect("exists"));
    drop(stream);

    // Missed while disconnected: the resumed stream delivers it (after the saved seq only).
    sb.update_note(&id, &content(&sb, &id).replace("Hello.", "Hello again."))
        .expect("edit");
    sb.sync(Trigger::AfterWrite).await.expect("push b");
    let mut stream = sa.subscribe_events().expect("resubscribe");
    let resumed = next_signal(&mut stream).await;
    match &resumed {
        EventSignal::Changed { seq: s } | EventSignal::Reset { seq: s } => assert!(*s > seq),
        other @ EventSignal::AccountClosed { .. } => panic!("expected a change, got {other:?}"),
    }
    assert!(sa.handle_event(&resumed).expect("handled"));
    sa.pull().await.expect("pull");
    assert!(content(&sa, &id).ends_with("Hello again.\n"));

    // The admin disables Alice while her stream is open.
    let sadmin = admin.sign_in("root").await;
    let item = sadmin
        .set_user_enabled(&alice_id, false)
        .await
        .expect("disable");
    assert_eq!(item.status, "disabled");
    let closed = next_signal(&mut stream).await;
    assert_eq!(
        closed,
        EventSignal::AccountClosed {
            seq: match closed {
                EventSignal::AccountClosed { seq, .. } => seq,
                _ => 0,
            },
            reason: "disabled".into(),
        }
    );
    assert!(!sa.handle_event(&closed).expect("handled"));
    assert_eq!(a.core.state().expect("state").kind, SessionKind::Disabled);
    w.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admin_and_account_intents_against_the_real_server() {
    let w = World::new().await;
    let alice_id = w.account("alice", UserRole::Member).await;
    w.account("root", UserRole::Admin).await;
    let admin = w.device(1);
    let a = w.device(2);
    let sadmin = admin.sign_in("root").await;

    let users = sadmin.admin_users("").await.expect("users");
    let mut names: Vec<&str> = users.users.iter().map(|u| u.username.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["alice", "root"]);
    let alice = sadmin.admin_users("ali").await.expect("filtered");
    assert_eq!(
        alice
            .users
            .iter()
            .map(|u| u.username.as_str())
            .collect::<Vec<_>>(),
        ["alice"]
    );

    // Role change, then a password reset: a one-time password that forces a change.
    let item = sadmin
        .set_user_role(&alice_id, "admin")
        .await
        .expect("role");
    assert_eq!(item.role, "admin");
    let item = sadmin
        .set_user_role(&alice_id, "member")
        .await
        .expect("role");
    assert_eq!(item.role, "member");
    let one_time = sadmin.reset_password(&alice_id).await.expect("reset");
    assert!(one_time.len() >= 10, "{one_time}");

    a.sign_in_with("alice", &one_time).await;
    assert_eq!(
        a.core.state().expect("state").kind,
        SessionKind::PasswordChangeRequired
    );
    let state = a
        .core
        .change_password(&one_time, "a-new-password-2")
        .await
        .expect("change");
    assert_eq!(state.kind, SessionKind::Active);

    // Timezone and devices.
    a.core.set_timezone("Africa/Cairo").await.expect("timezone");
    let sa = a.core.session().expect("session");
    assert_eq!(
        a.core
            .state()
            .expect("state")
            .account
            .expect("account")
            .timezone,
        "Africa/Cairo"
    );
    sa.refresh_settings().await.expect("settings");
    let settings = sa
        .read(|c, ctx| build::settings_view(c, ctx, common::SERVER))
        .expect("settings")
        .expect("signed in");
    let this: Vec<(&str, bool)> = settings
        .device_list
        .iter()
        .map(|d| (d.name.as_str(), d.is_this_device))
        .collect();
    assert_eq!(this, [("device 2", true)]);
    let device_id = settings.device_list[0].id.clone();
    sa.rename_device(&device_id, "Alice's laptop")
        .await
        .expect("rename");
    let settings = sa
        .read(|c, ctx| build::settings_view(c, ctx, common::SERVER))
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.device_list[0].name, "Alice's laptop");
    w.finish().await;
}
