//! Creation times and time zones against the real server (owner decision 2026-09-28 "Times,
//! creation stamps and titles"): items created offline keep the device's creation time however
//! late they sync, the device and the server write identical bytes for them, a device clock
//! far in the future is refused, times are written in UTC, and an account without a time
//! zone of its own takes the device's.
//!
//! The production app runs in-process (`TestServer`, per-test `PostgreSQL` database, fake
//! clock); the device has its own fake clock. Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL`
//! or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;

use actix_web::web;
use chrono::{DateTime, Utc};
use pretty_assertions::assert_eq;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::events::BusConfig;
use strata_api::sync::SyncConfig;
use strata_api::testing::TestServer;
use strata_common::Config;
use strata_common::config::Argon2Config;
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::client::{BrokenSyncApi, ClientAccountApi, ClientEventsApi, ClientSyncApi};
use strata_core::net::{SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv, NewTask, NotifyHub, Session};
use strata_core::store::conflicts::{self, RejectionRow};
use strata_core::store::{StorePaths, notes, outbox};
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::Version;
use strata_core::view::model::{Platform, SignInRequest};
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};
use tempfile::TempDir;

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).expect("instant").to_utc()
}

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
        let w = Self {
            db,
            _data: data,
            state,
            server,
        };
        w.state
            .create_account(
                &NewAccount {
                    username: "alice",
                    display_name: "alice",
                    password: "alice-password-1",
                    role: UserRole::Member,
                },
                None,
            )
            .await
            .expect("account");
        w
    }

    /// Sets the server's clock.
    fn server_at(&self, s: &str) {
        self.db.clock.set(at(s));
    }

    /// A device on its own clock (starting at the server's), in time zone `zone`.
    fn device(&self, zone: Option<&str>) -> Device {
        let dir = TempDir::new().expect("temp dir");
        let now = strata_common::clock::Clock::now(&self.db.clock);
        let clock = FakeClock::new(now);
        let base_ms = u64::try_from(now.timestamp_millis()).expect("ms");
        let env = CoreEnv {
            paths: StorePaths::new(dir.path()),
            platform: Platform::Linux,
            clock: Arc::new(clock.clone()),
            ids: Arc::new(SeqIds::new(base_ms)),
            account_api: Arc::new(ClientAccountApi {}),
            events_api: Arc::new(ClientEventsApi {}),
            notifications: NotifyHub::default(),
            sync_api: Arc::new(|url: &str, tokens: Tokens| -> Arc<dyn SyncApi> {
                match ClientSyncApi::new(url, tokens) {
                    Ok(api) => Arc::new(api),
                    Err(e) => Arc::new(BrokenSyncApi(e)),
                }
            }),
            default_device_name: "phone".to_owned(),
            server_url: self.server.base_url(),
            device_timezone: zone.map(str::to_owned),
        };
        Device {
            core: Core::open(env).expect("core"),
            clock,
            url: self.server.base_url(),
            _dir: dir,
        }
    }

    async fn finish(self) {
        drop(self.server);
        drop(self.state);
        self.db.cleanup().await.expect("cleanup");
    }
}

struct Device {
    core: Core,
    clock: FakeClock,
    url: String,
    _dir: TempDir,
}

impl Device {
    async fn sign_in(&self) -> Arc<Session> {
        self.core
            .sign_in(SignInRequest {
                username: "alice".to_owned(),
                password: "alice-password-1".to_owned(),
                device_name: "phone".to_owned(),
            })
            .await
            .expect("sign in");
        let s = self.core.session().expect("session");
        s.sync(Trigger::Start).await.expect("bootstrap");
        s
    }

    fn at(&self, s: &str) {
        self.clock.set(at(s));
    }
}

/// `(current content, base content, base version)` of a note on the device.
fn note(s: &Session, id: &str) -> (String, Option<String>, Option<String>) {
    s.read(|c, _| {
        let current = notes::current(c, id)?.expect("note").content;
        let base = notes::base(c, id)?;
        Ok((
            current,
            base.as_ref().map(|b| b.content.clone()),
            base.map(|b| b.version),
        ))
    })
    .expect("read")
}

/// Created offline on Monday, pushed on Wednesday: every new note keeps Monday (UTC), and the
/// server's bytes are the device's (the push answer's version is the hash of the device's
/// content, so it becomes the verified base without waiting for a pull).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_creates_keep_the_device_creation_time() {
    let w = World::new().await;
    w.server_at("2026-09-28T07:00:00Z");
    let d = w.device(Some("Africa/Cairo"));
    let s = d.sign_in().await;

    // Monday 08:15:30 UTC (11:15:30 in Cairo), offline: nothing is pushed.
    d.at("2026-09-28T08:15:30.400Z");
    let note_id = s
        .create_note("notes/Pricing.md", "# Pricing\n", false)
        .expect("note")
        .id
        .expect("id");
    let capture_id = s.capture("كلمت أحمد").expect("capture");
    let person_id = s
        .create_entity(domain::NoteKind::Person, "Ahmed", &[], false)
        .expect("person")
        .id
        .expect("id");
    let task = s
        .create_task(
            &NewTask {
                description: "Send the invoice".into(),
                ..NewTask::default()
            },
            false,
        )
        .expect("task")
        .id
        .expect("id");
    let home_id = s
        .read(|c, _| notes::id_by_path(c, "tasks/Tasks.md"))
        .expect("read")
        .expect("home");
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox").len(), 4);

    // Wednesday: back online.
    w.server_at("2026-09-30T09:00:00Z");
    d.at("2026-09-30T09:00:00Z");
    let report = s.sync(Trigger::Resume).await.expect("push");
    assert_eq!(report.pushed, 4);
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), Vec::new());

    let stamp = "created: 2026-09-28T08:15:30Z\nupdated: 2026-09-28T08:15:30Z\n";
    let expected = [
        (
            note_id.clone(),
            format!("---\nid: {note_id}\n{stamp}---\n# Pricing\n"),
        ),
        (
            capture_id.clone(),
            format!("---\nid: {capture_id}\ncreated: 2026-09-28T08:15:30Z\n---\nكلمت أحمد\n"),
        ),
        (
            person_id.clone(),
            format!("---\nid: {person_id}\nkind: person\n{stamp}---\n## Notes\n"),
        ),
        (
            home_id.clone(),
            format!(
                "---\nid: {home_id}\n{stamp}---\n## September 2026\n- [ ] Send the invoice ^{task}\n"
            ),
        ),
    ];
    for (id, content) in expected {
        let (current, base, version) = note(&s, &id);
        assert_eq!(current, content, "{id}");
        // The server wrote the same bytes: its version is the device content's hash, and the
        // pulled base is the device's content.
        assert_eq!(
            (base.as_deref(), version),
            (
                Some(content.as_str()),
                Some(Version::of_text(&content).as_str().to_owned())
            ),
            "{id}"
        );
    }
    // The inbox file is named by the capture time in UTC.
    assert_eq!(
        s.read(|c, _| notes::current(c, &capture_id))
            .expect("read")
            .expect("capture")
            .path,
        "inbox/2026-09-28-081530.md"
    );
    w.finish().await;
}

/// A device clock more than 5 minutes ahead of the server is refused (`422
/// created_in_future`): the create is rolled back and reported, nothing is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_creation_time_in_the_future_is_refused() {
    let w = World::new().await;
    w.server_at("2026-09-28T07:00:00Z");
    let d = w.device(None);
    let s = d.sign_in().await;

    d.at("2026-09-28T07:05:00Z");
    let ok = s.capture("on time").expect("capture");
    d.at("2026-09-28T07:05:01Z");
    let late = s.capture("too early").expect("capture");
    let report = s.sync(Trigger::AfterWrite).await.expect("push");
    assert_eq!(report.pushed, 2);
    assert_eq!(
        s.read(|c, _| notes::current(c, &ok))
            .expect("read")
            .map(|n| n.path),
        Some("inbox/2026-09-28-070500.md".to_owned())
    );
    assert_eq!(s.read(|c, _| notes::current(c, &late)).expect("read"), None);
    let rejections = s.read(|c, _| conflicts::rejections(c)).expect("rejections");
    assert_eq!(
        rejections
            .iter()
            .map(|r: &RejectionRow| (
                r.kind.as_str(),
                r.entity_id.as_str(),
                r.problem_type.as_str(),
                r.status
            ))
            .collect::<Vec<_>>(),
        vec![("capture", late.as_str(), "created_in_future", 422)]
    );
    w.finish().await;
}

/// An account without a time zone of its own takes the device's after the first bootstrap;
/// a zone the user chose is never replaced.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_device_time_zone_is_the_default() {
    let w = World::new().await;
    let tokyo = w.device(Some("Asia/Tokyo"));
    let s = tokyo.sign_in().await;
    let zone = |core: &Core| {
        core.state()
            .expect("state")
            .account
            .expect("account")
            .timezone
    };
    // Before `/me` knew better the server's default applied.
    assert_eq!(zone(&tokyo.core), "UTC");
    assert_eq!(tokyo.core.adopt_device_timezone().await, Ok(true));
    assert_eq!(zone(&tokyo.core), "Asia/Tokyo");
    // The setting syncs back; from then on it is the account's own.
    s.sync(Trigger::Manual).await.expect("pull");
    assert_eq!(tokyo.core.adopt_device_timezone().await, Ok(false));

    // Another device elsewhere keeps the account's zone.
    let cairo = w.device(Some("Africa/Cairo"));
    cairo.sign_in().await;
    assert_eq!(cairo.core.adopt_device_timezone().await, Ok(false));
    assert_eq!(zone(&cairo.core), "Asia/Tokyo");
    w.finish().await;
}
