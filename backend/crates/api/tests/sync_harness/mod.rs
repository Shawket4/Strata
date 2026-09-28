//! Harness for the sync and events tests: the production app (auth, vault store, sync, event
//! bus; fake clock) on an in-process server, signed-in users, generated clients whose every
//! response is validated against the contract, and a small device model built on
//! `sync-model` (records keyed by entity, a cursor, raw MessagePack requests whose responses
//! are validated with the `Contract` helper too).
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use actix_web::web;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::contract::Contract;
use strata_api::events::{BusConfig, EventBus};
use strata_api::sync::{SyncConfig, SyncState};
use strata_api::testing::TestServer;
use strata_client::{
    Client, ObservedResponse, ResponseObserver, StaticToken, operations as ops, types,
};
use strata_common::config::Argon2Config;
use strata_common::{Config, FakeClock, UserId};
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};
use sync_model::{
    BootstrapPage, Change, ChangesPage, EntityType, PushRequest, PushResponse, Record, SyncCursor,
    SyncOp,
};

/// Validates every response against the production contract.
#[derive(Debug)]
pub struct Conformance {
    pub contract: Contract,
    violations: Mutex<Vec<String>>,
    seen: Mutex<Vec<(String, u16)>>,
}

impl Conformance {
    fn check(&self, op: &str, status: u16, ct: Option<&str>, body: &[u8]) {
        self.seen
            .lock()
            .expect("lock")
            .push((op.to_owned(), status));
        if let Err(e) = self.contract.validate_response(op, status, ct, body) {
            self.violations
                .lock()
                .expect("lock")
                .push(format!("{op} {status}: {e}"));
        }
    }
}

impl ResponseObserver for Conformance {
    fn observe(&self, r: &ObservedResponse<'_>) {
        self.check(r.operation_id, r.status, r.content_type, r.body);
    }
}

/// One signed-in user.
#[derive(Clone)]
pub struct User {
    pub id: UserId,
    pub client: Client,
    pub token: String,
    pub device: ulid::Ulid,
}

/// One test's world.
pub struct H {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub clock: FakeClock,
    pub vault: VaultService,
    pub state: web::Data<AuthState>,
    pub bus: web::Data<EventBus>,
    server: TestServer,
    pub conformance: Arc<Conformance>,
}

impl H {
    pub async fn new() -> Self {
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
                    clock: Arc::new(clock.clone()),
                    ids: db.ids.clone(),
                },
                &config,
            )
            .expect("auth state"),
        );
        let (bus, sync): (web::Data<EventBus>, web::Data<SyncState>) =
            strata_api::sync::install(&vault, BusConfig::default(), SyncConfig::default());
        let (for_server, vault_data, bus_data) =
            (state.clone(), web::Data::new(vault.clone()), bus.clone());
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            cfg.app_data(bus_data.clone());
            cfg.app_data(sync.clone());
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            data,
            clock,
            vault,
            state,
            bus,
            server,
            conformance: Arc::new(Conformance {
                contract: Contract::production(),
                violations: Mutex::new(Vec::new()),
                seen: Mutex::new(Vec::new()),
            }),
        }
    }

    async fn account(&self, name: &str, role: UserRole) -> User {
        let password = format!("{name}-password-1");
        let id = self
            .state
            .create_account(
                &NewAccount {
                    username: name,
                    display_name: name,
                    password: &password,
                    role,
                },
                None,
            )
            .await
            .expect("account")
            .id;
        let anon = Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .build()
            .expect("client");
        let session = ops::login(
            &anon,
            &types::LoginRequest {
                username: name.to_owned(),
                password,
                device_name: format!("{name}-device"),
                platform: types::DevicePlatform::Linux,
            },
        )
        .await
        .expect("login");
        let client = self.client(&session.access_token);
        let devices = ops::list_devices(&client).await.expect("devices");
        User {
            id,
            client,
            token: session.access_token,
            device: devices
                .iter()
                .find(|d| d.current)
                .map(|d| d.id)
                .expect("current device"),
        }
    }

    /// Another device of `name` (signed in with the harness password).
    pub async fn login(&self, user: &User, name: &str, device_name: &str) -> User {
        let anon = Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .build()
            .expect("client");
        let session = ops::login(
            &anon,
            &types::LoginRequest {
                username: name.to_owned(),
                password: format!("{name}-password-1"),
                device_name: device_name.to_owned(),
                platform: types::DevicePlatform::Android,
            },
        )
        .await
        .expect("login");
        let client = self.client(&session.access_token);
        let devices = ops::list_devices(&client).await.expect("devices");
        let device = devices
            .iter()
            .find(|d| d.current)
            .map(|d| d.id)
            .expect("current device");
        User {
            id: user.id,
            client,
            token: session.access_token,
            device,
        }
    }

    /// An active member with a provisioned vault, signed in.
    pub async fn user(&self, name: &str) -> User {
        self.account(name, UserRole::Member).await
    }

    /// An active admin, signed in.
    pub async fn admin(&self, name: &str) -> User {
        self.account(name, UserRole::Admin).await
    }

    /// A contract-checked client sending `token`.
    pub fn client(&self, token: &str) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .tokens(Arc::new(StaticToken(token.to_owned())))
            .build()
            .expect("client")
    }

    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.server.addr()
    }

    pub fn dir(&self, user: UserId) -> PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("vault file")
    }

    pub fn exists(&self, user: UserId, rel: &str) -> bool {
        self.dir(user).join(rel).exists()
    }

    /// Commit messages, newest first.
    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    /// A raw request validated against the contract as `operation`: (status, content type,
    /// body).
    pub async fn raw(
        &self,
        token: &str,
        method: reqwest::Method,
        path_and_query: &str,
        operation: &str,
        body: Option<Vec<u8>>,
    ) -> (u16, Option<String>, Vec<u8>) {
        let client = reqwest::Client::new();
        let mut req = client
            .request(method, format!("{}{path_and_query}", self.base_url()))
            .header("Authorization", format!("Bearer {token}"));
        if let Some(bytes) = body {
            req = req
                .header("Content-Type", strata_client::MSGPACK)
                .body(bytes);
        }
        let resp = req.send().await.expect("response");
        let status = resp.status().as_u16();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = resp.bytes().await.expect("body").to_vec();
        self.conformance
            .check(operation, status, ct.as_deref(), &bytes);
        (status, ct, bytes)
    }

    /// `POST /sync/push` with `ops` (encoded with `sync-model`): the decoded response and its
    /// raw bytes.
    pub async fn push(&self, user: &User, ops: Vec<SyncOp>) -> (PushResponse, Vec<u8>) {
        let request = PushRequest { ops };
        let body = rmp_serde::to_vec_named(&request).expect("encode");
        let (status, _, bytes) = self
            .raw(
                &user.token,
                reqwest::Method::POST,
                "/api/v1/sync/push",
                "sync_push",
                Some(body),
            )
            .await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let response: PushResponse = rmp_serde::from_slice(&bytes).expect("decode push");
        response.check_answers(&request).expect("one answer per op");
        (response, bytes)
    }

    /// One bootstrap page (raw, contract-checked).
    pub async fn bootstrap_page(
        &self,
        user: &User,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<BootstrapPage, (u16, Vec<u8>)> {
        let mut q = Vec::new();
        if let Some(c) = cursor {
            q.push(format!("cursor={c}"));
        }
        if let Some(l) = limit {
            q.push(format!("limit={l}"));
        }
        let path = format!("/api/v1/sync/bootstrap?{}", q.join("&"));
        let (status, _, bytes) = self
            .raw(
                &user.token,
                reqwest::Method::GET,
                &path,
                "sync_bootstrap",
                None,
            )
            .await;
        if status == 200 {
            Ok(rmp_serde::from_slice(&bytes).expect("decode bootstrap"))
        } else {
            Err((status, bytes))
        }
    }

    /// One changes page (raw, contract-checked).
    pub async fn changes_page(
        &self,
        user: &User,
        since: u64,
        epoch: u64,
        limit: Option<u32>,
    ) -> Result<ChangesPage, (u16, Vec<u8>)> {
        let mut path = format!("/api/v1/sync/changes?since={since}&epoch={epoch}");
        if let Some(l) = limit {
            use std::fmt::Write as _;
            let _ = write!(path, "&limit={l}");
        }
        let (status, _, bytes) = self
            .raw(
                &user.token,
                reqwest::Method::GET,
                &path,
                "sync_changes",
                None,
            )
            .await;
        if status == 200 {
            Ok(rmp_serde::from_slice(&bytes).expect("decode changes"))
        } else {
            Err((status, bytes))
        }
    }

    /// A complete bootstrap (all pages) into a fresh device model.
    pub async fn bootstrap(&self, user: &User, limit: Option<u32>) -> Device {
        let mut device = Device::default();
        let mut cursor: Option<String> = None;
        loop {
            let page = self
                .bootstrap_page(user, cursor.as_deref(), limit)
                .await
                .expect("bootstrap page");
            device.apply_page(&page);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        device
    }

    /// Pulls every change after the device's cursor.
    pub async fn pull(&self, user: &User, device: &mut Device) {
        loop {
            let cursor = device.cursor.expect("bootstrapped");
            let page = self
                .changes_page(user, cursor.seq, cursor.epoch, None)
                .await
                .expect("changes page");
            device.apply_changes(&page);
            if !page.has_more {
                break;
            }
        }
    }

    pub fn assert_conformant(&self) {
        assert_eq!(
            *self.conformance.violations.lock().expect("lock"),
            Vec::<String>::new()
        );
        assert!(!self.conformance.seen.lock().expect("lock").is_empty());
    }

    pub async fn finish(self) {
        self.assert_conformant();
        drop(self.server);
        drop(self.state);
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// A device's cache: every record by (type, ID), and its sync position.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Device {
    pub records: BTreeMap<(EntityType, String), Record>,
    pub cursor: Option<SyncCursor>,
}

impl Device {
    /// Applies a bootstrap page (records are full states).
    pub fn apply_page(&mut self, page: &BootstrapPage) {
        for r in &page.records {
            self.records
                .insert((r.entity_type(), r.entity_id()), r.clone());
        }
        let cursor = SyncCursor::after_bootstrap(page);
        if let Some(c) = self.cursor {
            assert_eq!(
                (c.epoch, c.seq),
                (cursor.epoch, cursor.seq),
                "stable snapshot"
            );
        }
        self.cursor = Some(cursor);
    }

    /// Applies a changes page (validated with `SyncCursor::advance`).
    pub fn apply_changes(&mut self, page: &ChangesPage) {
        let cursor = self.cursor.expect("bootstrapped");
        let next = cursor.advance(page).expect("valid changes page");
        for c in &page.changes {
            let key = (c.entity_type, c.entity_id.clone());
            match &c.change {
                Change::Upsert { record } => {
                    self.records.insert(key, record.clone());
                }
                Change::Delete => {
                    self.records.remove(&key);
                }
            }
        }
        self.cursor = Some(next);
    }

    /// The note records by path.
    pub fn notes(&self) -> BTreeMap<String, sync_model::changes::NoteRecord> {
        self.records
            .values()
            .filter_map(|r| match r {
                Record::Note(n) => Some((n.path.clone(), n.clone())),
                _ => None,
            })
            .collect()
    }

    /// The records without the cursor.
    pub fn state(&self) -> &BTreeMap<(EntityType, String), Record> {
        &self.records
    }
}

/// The frontmatter of a note created at the test epoch (UTC).
pub fn header(id: &str) -> String {
    format!("---\nid: {id}\ncreated: 2026-09-27T12:00:00Z\nupdated: 2026-09-27T12:00:00Z\n---\n")
}

/// `sha256:` version of a text.
pub fn version(text: &str) -> sync_model::Version {
    sync_model::Version::of_text(text)
}
