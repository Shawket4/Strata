//! Harness for the graph API tests: the production app (auth, vault store, graph API with a
//! configurable recluster limit, fake clock) on an in-process server, users with provisioned
//! vaults, and generated clients whose every response is validated against the production
//! contract.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use actix_web::web;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::contract::Contract;
use strata_api::graph::GraphApi;
use strata_api::testing::TestServer;
use strata_client::{
    Client, Error, ObservedResponse, ResponseObserver, StaticToken, operations as ops, types,
};
use strata_common::config::{Argon2Config, RateLimit};
use strata_common::{Clock, Config, FakeClock, NoteId, UserId};
use strata_graph::GraphService;
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};
use strata_vault::ops::notes::CreateNote;
use strata_vault::{VaultConfig, VaultService};

/// Validates every response against the production contract.
#[derive(Debug)]
pub struct Conformance {
    pub contract: Contract,
    violations: Mutex<Vec<String>>,
    seen: Mutex<Vec<(String, u16)>>,
}

impl ResponseObserver for Conformance {
    fn observe(&self, r: &ObservedResponse<'_>) {
        self.seen
            .lock()
            .expect("lock")
            .push((r.operation_id.to_owned(), r.status));
        if let Err(e) =
            self.contract
                .validate_response(r.operation_id, r.status, r.content_type, r.body)
        {
            self.violations
                .lock()
                .expect("lock")
                .push(format!("{} {}: {e}", r.operation_id, r.status));
        }
    }
}

/// One signed-in user.
#[derive(Clone)]
pub struct User {
    pub id: UserId,
    pub client: Client,
    pub token: String,
}

/// One test's world.
pub struct H {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub clock: FakeClock,
    pub vault: VaultService,
    pub state: web::Data<AuthState>,
    server: TestServer,
    conformance: Arc<Conformance>,
}

impl H {
    /// With the given `POST /graph/recluster` limit.
    pub async fn new(recluster: RateLimit) -> Self {
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
        let graph = web::Data::new(GraphApi::with_limit(
            GraphService::new(db.app_db.clone(), vault.clone(), None),
            Arc::new(clock.clone()),
            db.ids.clone(),
            recluster,
        ));
        let for_server = state.clone();
        let vault_data = web::Data::new(vault.clone());
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            cfg.app_data(graph.clone());
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            data,
            clock,
            vault,
            state,
            server,
            conformance: Arc::new(Conformance {
                contract: Contract::production(),
                violations: Mutex::new(Vec::new()),
                seen: Mutex::new(Vec::new()),
            }),
        }
    }

    /// An active member with a provisioned vault, signed in.
    pub async fn user(&self, name: &str) -> User {
        let password = format!("{name}-password-1");
        let id = self
            .state
            .create_account(
                &NewAccount {
                    username: name,
                    display_name: name,
                    password: &password,
                    role: UserRole::Member,
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
                device_name: "test-device".to_owned(),
                platform: types::DevicePlatform::Linux,
            },
        )
        .await
        .expect("login");
        User {
            id,
            client: Client::builder(&self.server.base_url())
                .observer(self.conformance.clone())
                .tokens(Arc::new(StaticToken(session.access_token.clone())))
                .build()
                .expect("client"),
            token: session.access_token,
        }
    }

    /// Creates a note in `user`'s vault through the vault store.
    pub async fn note(&self, user: UserId, path: &str, content: &str) -> NoteId {
        self.vault
            .create_note(
                &self.db.scope(user),
                CreateNote {
                    path: path.to_owned(),
                    content: content.to_owned(),
                    id: None,
                    force: true,
                },
            )
            .await
            .expect("create note")
            .id
    }

    pub fn dir(&self, user: UserId) -> PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("vault file")
    }

    /// Commit messages, newest first.
    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }

    /// A raw request, validated against the contract as `operation`.
    pub async fn raw(
        &self,
        token: &str,
        method: reqwest::Method,
        path_and_query: &str,
        operation: &str,
    ) -> (u16, Option<String>, Vec<u8>) {
        let resp = reqwest::Client::new()
            .request(
                method,
                format!("{}{path_and_query}", self.server.base_url()),
            )
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.msgpack")
            .send()
            .await
            .expect("response");
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = resp.bytes().await.expect("body").to_vec();
        if let Err(e) =
            self.conformance
                .contract
                .validate_response(operation, status, ct.as_deref(), &bytes)
        {
            self.conformance
                .violations
                .lock()
                .expect("lock")
                .push(format!("{operation} {status}: {e}"));
        }
        (status, retry_after, bytes)
    }

    pub async fn finish(self) {
        assert_eq!(
            *self.conformance.violations.lock().expect("lock"),
            Vec::<String>::new()
        );
        assert!(!self.conformance.seen.lock().expect("lock").is_empty());
        drop(self.server);
        drop(self.state);
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// The problem of an API error (panics on other errors).
pub fn problem(err: &Error) -> types::Problem {
    err.api()
        .unwrap_or_else(|| panic!("expected a problem, got {err:?}"))
        .problem()
        .clone()
}

/// Asserts `result` failed with exactly `expected`.
pub fn assert_problem<T: std::fmt::Debug>(result: Result<T, Error>, expected: &types::Problem) {
    let err = result.expect_err("expected a problem");
    assert_eq!(&problem(&err), expected);
}

/// A problem with only the standard members.
pub fn plain(slug: &str, title: &str, status: u32, detail: Option<&str>) -> types::Problem {
    types::Problem {
        candidates: vec![],
        current_version: None,
        detail: detail.map(str::to_owned),
        errors: vec![],
        instance: None,
        status,
        title: title.to_owned(),
        type_: slug.to_owned(),
    }
}

/// One field error.
pub fn field(code: &str, pointer: &str, message: &str) -> types::ProblemFieldError {
    types::ProblemFieldError {
        code: code.to_owned(),
        pointer: Some(pointer.to_owned()),
        message: message.to_owned(),
    }
}
