//! The UI follow-ups against the real server (in-process `TestServer`, per-test `PostgreSQL`
//! database, fake clocks): custody notes and today's date in the account's zone, list-valued
//! properties, the title rule for a `note.create` whose path is taken (on the device and on the
//! server, identical bytes), and Admin → Users' purge-date preview and export label.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;

use actix_web::web;
use chrono::{DateTime, NaiveDate, Utc};
use pretty_assertions::assert_eq;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::events::BusConfig;
use strata_api::sync::SyncConfig;
use strata_api::testing::TestServer;
use strata_common::config::Argon2Config;
use strata_common::{Config, UserId};
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::client::{BrokenSyncApi, ClientAccountApi, ClientEventsApi, ClientSyncApi};
use strata_core::net::{SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv, NotifyHub, Session};
use strata_core::store::{StorePaths, notes};
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_core::view::model::{CustodyDraft, DocumentDraft, PlaceDraft, Platform, SignInRequest};
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};
use tempfile::TempDir;

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).expect("instant").to_utc()
}

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

    async fn account(&self, name: &str, role: UserRole) -> UserId {
        self.state
            .create_account(
                &NewAccount {
                    username: name,
                    display_name: name,
                    password: "a-password-123",
                    role,
                },
                None,
            )
            .await
            .expect("account")
            .id
    }

    fn server_at(&self, s: &str) {
        self.db.clock.set(at(s));
    }

    fn device(&self, zone: &str) -> Device {
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
            default_server_url: None,
            device_timezone: Some(zone.to_owned()),
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
    /// Signs in as `name`, takes the device's zone and bootstraps.
    async fn sign_in(&self, name: &str) -> Arc<Session> {
        self.core
            .sign_in(SignInRequest {
                server_url: self.url.clone(),
                username: name.to_owned(),
                password: "a-password-123".to_owned(),
                device_name: "phone".to_owned(),
            })
            .await
            .expect("sign in");
        let s = self.core.session().expect("session");
        s.sync(Trigger::Start).await.expect("bootstrap");
        self.core.adopt_device_timezone().await.expect("zone");
        s
    }

    fn at(&self, s: &str) {
        self.clock.set(at(s));
    }
}

/// `(path, content, base content)` of a note on the device.
fn note(s: &Session, id: &str) -> (String, String, Option<String>) {
    s.read(|c, _| {
        let current = notes::current(c, id)?.expect("note");
        let base = notes::base(c, id)?;
        Ok((current.path, current.content, base.map(|b| b.content)))
    })
    .expect("read")
}

/// "Record a move" with no date is recorded today in the account's zone (01:30 on the 29th
/// in Cairo is still the 28th in UTC), with the user's note last on the line; the server
/// writes the device's bytes, and the document page shows the note.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custody_drafts_take_a_note_and_default_to_today_in_the_account_zone() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    w.server_at("2026-09-28T22:30:00Z");
    let d = w.device("Africa/Cairo");
    let s = d.sign_in("alice").await;
    d.at("2026-09-28T22:30:00Z");
    let safe = s
        .create_place(
            &PlaceDraft {
                name: "Safe".into(),
                aliases: vec![],
                parent_id: None,
                address: None,
            },
            false,
        )
        .expect("place")
        .id
        .expect("id");
    let doc = s
        .create_document(
            &DocumentDraft {
                name: "Car license".into(),
                aliases: vec![],
                doc_type: None,
                copy: None,
                copy_of: None,
                companies: vec![],
                people: vec![],
                expires: None,
            },
            false,
        )
        .expect("document")
        .id
        .expect("id");
    s.record_custody(
        &doc,
        &CustodyDraft {
            kind: "stored-at".into(),
            place_id: Some(safe.clone()),
            person_id: None,
            counterparty_id: None,
            date: None,
            note: Some("  for the\n audit ".into()),
        },
    )
    .expect("custody");
    let (_, content, _) = note(&s, &doc);
    assert!(
        content.contains("## Custody\n- 2026-09-29 — stored-at [[Safe]] — for the audit\n"),
        "{content}"
    );
    // An explicit date is kept; a blank note is no note.
    s.record_custody(
        &doc,
        &CustodyDraft {
            kind: "moved-to".into(),
            place_id: Some(safe.clone()),
            person_id: None,
            counterparty_id: None,
            date: NaiveDate::from_ymd_opt(2026, 9, 30),
            note: Some(" ".into()),
        },
    )
    .expect("custody");
    let report = s.sync(Trigger::AfterWrite).await.expect("push");
    assert_eq!(report.pushed, 4);
    s.sync(Trigger::Manual).await.expect("pull");
    let (_, content, base) = note(&s, &doc);
    assert_eq!(
        base.as_deref(),
        Some(content.as_str()),
        "server bytes = device bytes"
    );
    assert!(
        content.contains(
            "## Custody\n- 2026-09-30 — moved-to [[Safe]]\n- 2026-09-29 — stored-at [[Safe]] — for the audit\n"
        ),
        "{content}"
    );
    let page = s
        .read(|c, ctx| build::entity_screen(c, ctx, &doc))
        .expect("page")
        .document
        .expect("document");
    assert_eq!(
        page.custody
            .iter()
            .map(|c| (c.date.to_string(), c.kind.as_str(), c.note.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("2026-09-30".to_owned(), "moved-to", None),
            ("2026-09-29".to_owned(), "stored-at", Some("for the audit")),
        ]
    );
    // A note made of links only would read back as citations: refused, nothing queued.
    let refused = s.record_custody(
        &doc,
        &CustodyDraft {
            kind: "lost".into(),
            place_id: None,
            person_id: None,
            counterparty_id: None,
            date: None,
            note: Some("[[Safe]]".into()),
        },
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(note(&s, &doc).1, content);
    w.finish().await;
}

/// `set_property_values` replaces a list (several phone numbers, aliases, tags) on the device
/// and the server alike; an empty list removes the key; relation keys are refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_properties_are_set_as_lists() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    w.server_at("2026-09-28T07:00:00Z");
    let d = w.device("Africa/Cairo");
    let s = d.sign_in("alice").await;
    let ahmed = s
        .create_entity(
            domain::NoteKind::Person,
            "Ahmed",
            &["Ahmed S.".into()],
            false,
        )
        .expect("person")
        .id
        .expect("id");
    let v = |xs: &[&str]| xs.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    s.set_property_values(&ahmed, "phone", &v(&["+20 100", " +20 101 ", "+20 100"]))
        .expect("phones");
    s.set_property_values(&ahmed, "aliases", &v(&["أحمد سمير", "Ahmed S."]))
        .expect("aliases");
    s.set_property_values(&ahmed, "tags", &v(&["#client", "vip"]))
        .expect("tags");
    let (_, local, _) = note(&s, &ahmed);
    assert!(
        local.contains("aliases: [أحمد سمير, Ahmed S.]\ntags: [client, vip]\n")
            && local.contains("phone: [+20 100, +20 101]\n"),
        "{local}"
    );
    assert!(
        s.set_property_values(&ahmed, "works-at", &v(&["[[Acme]]"]))
            .is_err()
    );
    s.sync(Trigger::AfterWrite).await.expect("push");
    s.sync(Trigger::Manual).await.expect("pull");
    let (_, content, base) = note(&s, &ahmed);
    assert_eq!(base.as_deref(), Some(content.as_str()));
    assert!(content.contains("phone: [+20 100, +20 101]\n"), "{content}");
    assert!(content.contains("tags: [client, vip]\n"), "{content}");
    // An empty list removes the key.
    s.set_property_values(&ahmed, "phone", &[]).expect("clear");
    s.sync(Trigger::AfterWrite).await.expect("push");
    s.sync(Trigger::Manual).await.expect("pull");
    let (_, content, base) = note(&s, &ahmed);
    assert_eq!(base.as_deref(), Some(content.as_str()));
    assert!(!content.contains("phone:"), "{content}");
    w.finish().await;
}

/// The title rule for plain notes: a `note.create` whose path is taken lands at
/// `<stem> 2.md` with `title: <stem>` — on the device when it knows the path is taken (and
/// the server writes the same bytes), and on the server when only the server knows.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_taken_note_path_keeps_its_name_as_the_title() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    w.server_at("2026-09-28T07:00:00Z");
    let phone = w.device("Africa/Cairo");
    // Another ULID base, so the two devices never mint the same IDs.
    w.server_at("2026-09-28T07:00:00.500Z");
    let laptop = w.device("Africa/Cairo");
    w.server_at("2026-09-28T07:00:00Z");
    let s = phone.sign_in("alice").await;
    let l = laptop.sign_in("alice").await;

    let first = s
        .create_note("notes/Plan.md", "# Plan\n", false)
        .expect("note")
        .id
        .expect("id");
    s.sync(Trigger::AfterWrite).await.expect("push");
    // The phone knows `notes/Plan.md`: its second note lands at `Plan 2.md` with the title.
    let second = s
        .create_note("notes/Plan.md", "Second\n", true)
        .expect("note")
        .id
        .expect("id");
    let stamp = "created: 2026-09-28T07:00:00Z\nupdated: 2026-09-28T07:00:00Z\n";
    let expected = format!("---\nid: {second}\ntitle: Plan\n{stamp}---\nSecond\n");
    assert_eq!(
        note(&s, &second),
        ("notes/Plan 2.md".to_owned(), expected.clone(), None)
    );
    s.sync(Trigger::AfterWrite).await.expect("push");
    assert_eq!(
        note(&s, &second),
        (
            "notes/Plan 2.md".to_owned(),
            expected.clone(),
            Some(expected)
        ),
        "the server wrote the device's bytes"
    );

    // The laptop has not pulled: it writes `notes/Plan.md` locally, the server moves it to
    // the first free name (`Plan 3.md`) with the title, and the pull brings that back.
    let third = l
        .create_note("notes/Plan.md", "Third\n", true)
        .expect("note")
        .id
        .expect("id");
    assert_eq!(note(&l, &third).0, "notes/Plan.md");
    l.sync(Trigger::AfterWrite).await.expect("push");
    l.sync(Trigger::Manual).await.expect("pull");
    let expected = format!("---\nid: {third}\ntitle: Plan\n{stamp}---\nThird\n");
    assert_eq!(
        note(&l, &third),
        (
            "notes/Plan 3.md".to_owned(),
            expected.clone(),
            Some(expected)
        )
    );
    // The first note keeps its path and has no title.
    let (path, content, _) = note(&l, &first);
    assert_eq!(
        (path.as_str(), content.contains("title:")),
        ("notes/Plan.md", false)
    );
    w.finish().await;
}

/// Admin → Users shows the purge date before a deletion is scheduled (`GET /admin/settings`)
/// and when a user downloaded their export, both in the admin's zone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admin_users_show_the_purge_date_and_the_export_download() {
    let w = World::new().await;
    w.account("root", UserRole::Admin).await;
    let bob = w.account("bob", UserRole::Member).await;
    w.server_at("2026-09-28T21:30:00Z");
    let d = w.device("Africa/Cairo");
    let s = d.sign_in("root").await;
    d.at("2026-09-28T21:30:00Z");
    let view = s.admin_users("").await.expect("users");
    // 21:30 UTC is the 29th in Cairo; plus the default 14 days.
    assert_eq!(
        view.deletion_preview_label.as_deref(),
        Some("Deleted on 13 Oct 2026")
    );
    let scheduled = s
        .schedule_deletion(&bob.as_ulid().to_string())
        .await
        .expect("scheduled");
    assert_eq!(
        scheduled.deletion_label.as_deref(),
        view.deletion_preview_label.as_deref()
    );
    assert_eq!(scheduled.export_downloaded_label, None);
    w.db.accounts_db
        .mark_export_downloaded(bob, at("2026-09-28T21:40:00Z"))
        .await
        .expect("mark")
        .expect("pending");
    let view = s.admin_users("bob").await.expect("users");
    assert_eq!(
        view.users
            .iter()
            .map(|u| (u.username.as_str(), u.export_downloaded_label.as_deref()))
            .collect::<Vec<_>>(),
        // 21:40 UTC is 00:40 on the 29th in Cairo: "today" for the admin.
        vec![("bob", Some("Export downloaded 00:40"))]
    );
    w.finish().await;
}
