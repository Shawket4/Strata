//! Harness for the hardening suites (PLAN §15 security checklist, §16.3 schema-driven fuzzer,
//! §16.7 performance): the complete production composition — auth, vault store, sync and
//! events, AI (fake LLM provider, fake embedder), graph and maps — behind `stratad`'s request
//! logger, on an in-process server with the fake clock; users with a fixture of every kind of
//! object; a raw HTTP client whose every response is validated against the contract; and
//! scanners for leaked paths and content.
#![allow(
    dead_code,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

pub mod generate;
pub mod http;
pub mod ops;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use actix_web::middleware::from_fn;
use actix_web::web;
use strata_ai::embed::fake::FakeEmbedder;
use strata_ai::{AiService, BudgetGuard, BudgetLimits, Embedder, MemoryUsageStore, ProviderRouter};
use strata_api::ai::AiApi;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::contract::Contract;
use strata_api::events::{BusConfig, EventBus};
use strata_api::graph::GraphApi;
use strata_api::sync::{SyncConfig, SyncState};
use strata_api::testing::TestServer;
use strata_client::{
    Client, ObservedResponse, ResponseObserver, StaticToken, operations as api, types,
};
use strata_common::config::{AiProviderKind, Argon2Config, RateLimit};
use strata_common::{Config, FakeClock, SequentialIdGenerator, UserId};
use strata_graph::GraphService;
use strata_index::types::UserRole;
use strata_jobs::ask::{AskConfig, AskEngine};
use strata_jobs::retrieval::Retriever;
use strata_jobs::{RecordedEvents, Runner, RunnerConfig};
use strata_testkit::{FakeLlmProvider, TempDataRoot, TestDb};
use strata_vault::{ImportLimits, VaultConfig, VaultService};

use self::generate::Pools;
use self::http::{Req, Resp};
use self::ops::Op;

/// Every response seen, validated against the contract.
#[derive(Debug)]
pub struct Conformance {
    /// The production contract.
    pub contract: Contract,
    violations: Mutex<Vec<String>>,
    seen: Mutex<BTreeMap<String, BTreeMap<u16, u32>>>,
}

impl Conformance {
    /// Validates one response of `op`; returns the violation, if any, and records it.
    pub fn check(&self, op: &str, status: u16, ct: Option<&str>, body: &[u8]) -> Option<String> {
        *self
            .seen
            .lock()
            .expect("lock")
            .entry(op.to_owned())
            .or_default()
            .entry(status)
            .or_default() += 1;
        let violation = self
            .contract
            .validate_response(op, status, ct, body)
            .err()
            .map(|e| format!("{op} {status}: {e}"));
        if let Some(v) = &violation {
            self.violations.lock().expect("lock").push(v.clone());
        }
        violation
    }

    /// Statuses seen per operation.
    pub fn seen(&self) -> BTreeMap<String, BTreeMap<u16, u32>> {
        self.seen.lock().expect("lock").clone()
    }

    /// Violations so far.
    pub fn violations(&self) -> Vec<String> {
        self.violations.lock().expect("lock").clone()
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
    pub name: String,
    pub password: String,
    pub client: Client,
    pub token: String,
}

/// Knobs for [`H::with`].
pub struct Options {
    /// Adjusts the server configuration (rate limits, ...).
    pub config: Box<dyn FnOnce(&mut Config)>,
    /// Import limits of the vault store.
    pub import: ImportLimits,
    /// `POST /graph/recluster` limit.
    pub recluster: RateLimit,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            config: Box::new(|_| {}),
            import: ImportLimits::default(),
            recluster: RateLimit {
                max: 1000,
                window_secs: 3600,
            },
        }
    }
}

/// Rate limits high enough that sweeps and the fuzzer never meet them.
pub fn generous_limits(config: &mut Config) {
    let many = RateLimit {
        max: 1_000_000,
        window_secs: 60,
    };
    let limits = &mut config.auth.rate_limits;
    limits.login_per_ip = many;
    limits.login_per_username = many;
    limits.signup_per_ip = many;
    limits.signup_global = many;
    limits.capture_per_user = many;
    limits.ask_per_user = many;
}

/// One test's world.
pub struct H {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub clock: FakeClock,
    pub vault: VaultService,
    pub state: web::Data<AuthState>,
    pub bus: web::Data<EventBus>,
    pub llm: FakeLlmProvider,
    pub embedder: FakeEmbedder,
    pub ai: Arc<AiService>,
    pub config: Config,
    server: TestServer,
    pub conformance: Arc<Conformance>,
}

impl H {
    /// The default composition.
    pub async fn new() -> Self {
        Self::with(Options::default()).await
    }

    /// A composition adjusted by `opts`.
    pub async fn with(opts: Options) -> Self {
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
        (opts.config)(&mut config);
        let clock = db.clock.clone();
        let mut vault_config = VaultConfig::new(data.path());
        vault_config.import = opts.import;
        let vault = VaultService::new(
            vault_config,
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
        let (bus, sync) =
            strata_api::sync::install(&vault, BusConfig::default(), SyncConfig::default());
        let llm = FakeLlmProvider::new().named("claude_cli");
        let router = ProviderRouter::new(AiProviderKind::ClaudeCli, BTreeMap::new())
            .with_provider(AiProviderKind::ClaudeCli, Arc::new(llm.clone()));
        let budget = BudgetGuard::new(
            BudgetLimits::default(),
            chrono_tz::UTC,
            Arc::new(clock.clone()),
            Arc::new(MemoryUsageStore::default()),
        );
        let embedder = FakeEmbedder::new("fake-embed@1", 384);
        let embedder_arc: Arc<dyn Embedder> = Arc::new(embedder.clone());
        vault.set_semantic(Arc::new(strata_jobs::semantic_dup::SemanticDupSource::new(
            embedder_arc.clone(),
            dedupe::Thresholds::new(),
        )));
        let ai = Arc::new(AiService::new(router, budget).with_embedder(embedder_arc.clone()));
        let retriever = Retriever::new(db.app_db.clone(), embedder_arc);
        let ask = AskEngine::new(
            db.app_db.clone(),
            vault.clone(),
            ai.clone(),
            Some(retriever.clone()),
            Arc::new(SequentialIdGenerator::default()),
            Arc::new(clock.clone()),
            AskConfig {
                top_k: 3,
                ..AskConfig::default()
            },
        );
        let ai_api = web::Data::new(AiApi::new(
            ai.clone(),
            db.app_db.clone(),
            Some(retriever),
            ask,
            Arc::new(clock.clone()),
            Arc::new(SequentialIdGenerator::new(1_790_000_000_000)),
        ));
        let graph = web::Data::new(GraphApi::with_limit(
            GraphService::new(db.app_db.clone(), vault.clone(), None),
            Arc::new(clock.clone()),
            db.ids.clone(),
            opts.recluster,
        ));
        let (for_server, vault_data, bus_data) =
            (state.clone(), web::Data::new(vault.clone()), bus.clone());
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            cfg.app_data(bus_data.clone());
            cfg.app_data(sync.clone());
            cfg.app_data(ai_api.clone());
            cfg.app_data(graph.clone());
            cfg.service(
                web::scope("")
                    .wrap(from_fn(stratad::logging::log_request))
                    .configure(strata_api::app::configure),
            );
        })
        .expect("server");
        Self {
            db,
            data,
            clock,
            vault,
            state,
            bus,
            llm,
            embedder,
            ai,
            config,
            server,
            conformance: Arc::new(Conformance {
                contract: Contract::production(),
                violations: Mutex::new(Vec::new()),
                seen: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    /// An account (vault provisioned) signed in as device `<name>-device`.
    pub async fn account(&self, name: &str, role: UserRole) -> User {
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
        let token = self.login(name, &password).await;
        User {
            id,
            name: name.to_owned(),
            password,
            client: self.client(&token),
            token,
        }
    }

    /// An active member.
    pub async fn user(&self, name: &str) -> User {
        self.account(name, UserRole::Member).await
    }

    /// An active admin.
    pub async fn admin(&self, name: &str) -> User {
        self.account(name, UserRole::Admin).await
    }

    /// A new session's access token.
    pub async fn login(&self, name: &str, password: &str) -> String {
        api::login(
            &self.anon(),
            &types::LoginRequest {
                username: name.to_owned(),
                password: password.to_owned(),
                device_name: format!("{name}-device"),
                platform: types::DevicePlatform::Linux,
            },
        )
        .await
        .expect("login")
        .access_token
    }

    /// A contract-checked client sending `token`.
    pub fn client(&self, token: &str) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .tokens(Arc::new(StaticToken(token.to_owned())))
            .build()
            .expect("client")
    }

    /// A contract-checked client without a token.
    pub fn anon(&self) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .build()
            .expect("client")
    }

    pub fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// `<data_root>/users/<id>/vault`.
    pub fn dir(&self, user: UserId) -> PathBuf {
        self.vault.vault_dir(user)
    }

    /// Commit messages, newest first.
    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    /// Sends a raw request; when `op` is given the response is validated as that operation.
    pub async fn send(&self, op: Option<&str>, req: &Req) -> Resp {
        let resp = http::send(self.addr(), req).await;
        if let Some(op) = op
            && resp.status != 101
        {
            self.conformance
                .check(op, resp.status, resp.content_type(), &resp.body);
        }
        resp
    }

    /// Runs the embedding jobs until idle (builds the vectors search and Ask use).
    pub async fn embed_all(&self) {
        let embedder: Arc<dyn Embedder> = Arc::new(self.embedder.clone());
        Runner::new(
            self.db.app_db.clone(),
            self.db.issuer.clone(),
            Arc::new(self.clock.clone()),
            Arc::new(RecordedEvents::default()),
            RunnerConfig {
                idle_recheck: chrono::Duration::zero(),
                ..RunnerConfig::default()
            },
            vec![Arc::new(strata_jobs::embed::EmbedHandler::new(
                self.db.app_db.clone(),
                self.vault.clone(),
                Some(embedder),
                Arc::new(self.clock.clone()),
                self.db.ids.clone(),
                false,
            ))],
        )
        .run_until_idle()
        .await;
    }

    pub fn assert_conformant(&self) {
        assert_eq!(self.conformance.violations(), Vec::<String>::new());
    }

    pub async fn finish(self) {
        self.assert_conformant();
        drop(self.server);
        drop(self.state);
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// Everything one user owns: at least one object of every kind an operation can address.
#[derive(Debug, Clone)]
pub struct Fixtures {
    pub note: types::Note,
    pub other: types::Note,
    pub trashed: types::Note,
    pub person: ulid::Ulid,
    pub company: ulid::Ulid,
    pub document: ulid::Ulid,
    pub place: ulid::Ulid,
    pub task: types::Task,
    pub suggestion: ulid::Ulid,
    pub commit: String,
    pub device: ulid::Ulid,
    pub map: String,
    pub ask: ulid::Ulid,
    /// Text only this user wrote (must never reach anyone else).
    pub secret: String,
}

impl Fixtures {
    /// The IDs as generator pools (kinds as in [`path_kind`]).
    pub fn pools(&self) -> Pools {
        let mut p = Pools::default();
        p.add("note", self.note.id.to_string());
        p.add("note", self.other.id.to_string());
        p.add("trash", self.trashed.id.to_string());
        p.add("entity", self.person.to_string());
        p.add("entity", self.company.to_string());
        p.add("document", self.document.to_string());
        p.add("place", self.place.to_string());
        p.add("task", self.task.id.clone());
        p.add("suggestion", self.suggestion.to_string());
        p.add("commit", self.commit.clone());
        p.add("device", self.device.to_string());
        p.add("map", self.map.clone());
        p.add("ask", self.ask.to_string());
        p
    }

    /// The fixture for a path parameter of `kind`.
    pub fn id_of(&self, kind: &str) -> String {
        self.pools()
            .get(kind)
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("no fixture of kind {kind}"))
    }

    /// Every ID and the secret text: none may appear in another user's responses.
    pub fn private_strings(&self) -> Vec<String> {
        let mut out = self.pools().all();
        out.push(self.secret.clone());
        out.push(self.note.path.clone());
        out
    }
}

/// The fixture kind a path parameter addresses, from the operation's path template (so an
/// operation added later under an existing resource is covered automatically).
pub fn path_kind(op: &Op, param: &str) -> Option<&'static str> {
    let p = op.path.as_str();
    if param == "commit" {
        return Some("commit");
    }
    let prefixes: [(&str, &str); 13] = [
        ("/api/v1/notes/{id}", "note"),
        ("/api/v1/graph/local/{id}", "note"),
        ("/api/v1/trash/{id}", "trash"),
        ("/api/v1/entities/{id}", "entity"),
        ("/api/v1/documents/{id}", "document"),
        ("/api/v1/places/{id}", "place"),
        ("/api/v1/tasks/{id}", "task"),
        ("/api/v1/suggestions/{id}", "suggestion"),
        ("/api/v1/devices/{id}", "device"),
        ("/api/v1/admin/users/{id}", "user"),
        ("/api/v1/maps/{id}", "map"),
        ("/api/v1/ask/{id}", "ask"),
        ("/api/v1/commits/{commit}", "commit"),
    ];
    prefixes
        .iter()
        .find(|(prefix, _)| p.starts_with(prefix))
        .map(|(_, kind)| *kind)
}

/// A valid JSON Canvas with one file node pointing at `path`.
pub fn canvas(path: &str) -> String {
    format!(
        "{{\"nodes\":[{{\"id\":\"n1\",\"type\":\"file\",\"file\":\"{path}\",\"x\":0,\"y\":0,\"width\":400,\"height\":200}}],\"edges\":[]}}"
    )
}

/// Creates the fixtures of `u` (its secret text is `secret`).
pub async fn populate(h: &H, u: &User, secret: &str) -> Fixtures {
    let c = &u.client;
    let note = api::create_note(
        c,
        &types::CreateNoteRequest {
            content: format!("The plan for [[Watanya]]. {secret}\n\nSecond block ^b1\n"),
            force: None,
            id: None,
            path: "notes/Plan.md".into(),
        },
    )
    .await
    .expect("note");
    let other = api::create_note(
        c,
        &types::CreateNoteRequest {
            content: format!("Follow-up to [[Plan]]. {secret}\n"),
            force: None,
            id: None,
            path: "notes/Follow up.md".into(),
        },
    )
    .await
    .expect("other");
    let trashed = api::create_note(
        c,
        &types::CreateNoteRequest {
            content: format!("Old {secret}\n"),
            force: None,
            id: None,
            path: "notes/Old.md".into(),
        },
    )
    .await
    .expect("trashed");
    api::delete_note(c, trashed.id).await.expect("trash");
    api::add_relation(
        c,
        &types::RelationRef {
            dst_id: other.id,
            src_id: note.id,
            type_: "related".into(),
        },
    )
    .await
    .expect("relation");
    let entity = |kind, name: &str| types::CreateEntityRequest {
        aliases: vec![],
        fields: HashMap::new(),
        force: None,
        id: None,
        kind,
        name: name.into(),
        parent_id: None,
        tags: vec![],
    };
    let person = api::create_entity(c, &entity(types::EntityKind::Person, "Watanya"))
        .await
        .expect("person")
        .id;
    let company = api::create_entity(c, &entity(types::EntityKind::Company, "Acme Trading"))
        .await
        .expect("company")
        .id;
    let place = api::create_place(
        c,
        &types::CreatePlaceRequest {
            address: None,
            aliases: vec![],
            force: None,
            id: None,
            name: "Safe".into(),
            parent_id: None,
            tags: vec![],
        },
    )
    .await
    .expect("place")
    .place
    .id;
    let document = api::create_document(
        c,
        &types::CreateDocumentRequest {
            aliases: vec![],
            copy: None,
            doc_type: Some("passport".into()),
            expires: None,
            force: None,
            id: None,
            name: "Passport".into(),
            tags: vec![],
        },
    )
    .await
    .expect("document")
    .document
    .id;
    api::add_custody_event(
        c,
        document,
        &types::CustodyEventRequest {
            at: chrono::NaiveDate::from_ymd_opt(2026, 9, 1).expect("date"),
            counterparty_id: None,
            person_id: None,
            place_id: Some(place),
            source_note_id: None,
            type_: types::CustodyEventKind::StoredAt,
        },
    )
    .await
    .expect("custody");
    let task = api::create_task(
        c,
        &types::CreateTaskRequest {
            text: "Pay the rent [[Watanya]]".into(),
            due: None,
            force: None,
            id: None,
            note_id: None,
            priority: None,
            recurrence: None,
            reminders: vec![],
            scheduled: None,
            start: None,
        },
    )
    .await
    .expect("task");
    let text = format!("Ask Watanya about the plan {secret}");
    api::capture(c, &types::CaptureRequest { text: text.clone() })
        .await
        .expect("capture");
    let suggestion = api::capture(c, &types::CaptureRequest { text })
        .await
        .expect("capture")
        .suggestion_id
        .expect("duplicate suggestion");
    let commit = api::get_note_history(c, note.id)
        .await
        .expect("history")
        .revisions[0]
        .commit
        .clone();
    let device = api::list_devices(c)
        .await
        .expect("devices")
        .iter()
        .find(|d| d.current)
        .map(|d| d.id)
        .expect("current device");
    let map = "Overview".to_owned();
    api::put_map(
        c,
        &map,
        None,
        &types::PutMapRequest {
            content: canvas("notes/Plan.md"),
        },
    )
    .await
    .expect("map");
    h.embed_all().await;
    let ask = api::ask(
        c,
        &types::AskRequest {
            question: format!("What is the plan? {secret}"),
            scope: None,
        },
    )
    .await
    .expect("ask")
    .id;
    Fixtures {
        note,
        other,
        trashed,
        person,
        company,
        document,
        place,
        task,
        suggestion,
        commit,
        device,
        map,
        ask,
        secret: secret.to_owned(),
    }
}

/// Every string (keys and values) inside a MessagePack document; empty if it is not one.
pub fn strings_in(body: &[u8]) -> Vec<String> {
    fn walk(v: &rmpv::Value, out: &mut Vec<String>) {
        match v {
            rmpv::Value::String(s) => {
                if let Some(s) = s.as_str() {
                    out.push(s.to_owned());
                }
            }
            rmpv::Value::Binary(b) => out.push(String::from_utf8_lossy(b).into_owned()),
            rmpv::Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            rmpv::Value::Map(entries) => entries.iter().for_each(|(k, v)| {
                walk(k, out);
                walk(v, out);
            }),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Ok(v) = rmpv::decode::read_value(&mut &body[..]) {
        walk(&v, &mut out);
    }
    out
}

/// Filesystem paths in a string: the data root, absolute Unix/Windows paths, `..` segments.
pub fn path_leaks(s: &str, data_root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let root = data_root.to_string_lossy();
    if s.contains(root.as_ref()) {
        out.push(format!("data root in {s:?}"));
    }
    for prefix in [
        "/home/", "/tmp/", "/srv/", "/etc/", "/root/", "/var/", "/usr/", "/proc/", "/opt/",
    ] {
        if s.contains(prefix) {
            out.push(format!("absolute path {prefix} in {s:?}"));
        }
    }
    if s.contains("../") || s.contains("..\\") || s.contains(":\\") {
        out.push(format!("path escape in {s:?}"));
    }
    out
}

/// Files under `root` (relative paths), skipping the given directories.
pub fn files_under(root: &Path, skip: &[PathBuf]) -> BTreeSet<PathBuf> {
    fn walk(dir: &Path, root: &Path, skip: &[PathBuf], out: &mut BTreeSet<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if skip.contains(&p) {
                continue;
            }
            if p.is_dir() {
                walk(&p, root, skip, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap_or(&p).to_path_buf());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(root, root, skip, &mut out);
    out
}
