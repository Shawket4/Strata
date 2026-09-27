//! Harness for the AI API tests: the production app (auth, vault store, AI features) on an
//! in-process server with the fake clock, the fake LLM provider behind a real `AiService`, a
//! deterministic fake embedder, a job runner to build vectors, and contract-checked clients.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use actix_web::web;
use strata_ai::embed::fake::FakeEmbedder;
use strata_ai::{AiService, BudgetGuard, BudgetLimits, Embedder, MemoryUsageStore, ProviderRouter};
use strata_api::ai::AiApi;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::contract::Contract;
use strata_api::testing::TestServer;
use strata_client::{
    Client, Error, ObservedResponse, ResponseObserver, StaticToken, operations as ops, types,
};
use strata_common::config::{AiProviderKind, Argon2Config};
use strata_common::{Config, FakeClock, SequentialIdGenerator, UserId};
use strata_index::types::UserRole;
use strata_jobs::ask::{AskConfig, AskEngine};
use strata_jobs::retrieval::Retriever;
use strata_jobs::{Deps, RecordedEvents, Runner, RunnerConfig};
use strata_testkit::{FakeLlmProvider, TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};

pub const MODEL: &str = "fake-embed@1";

#[derive(Debug)]
pub struct Conformance {
    pub contract: Contract,
    violations: Mutex<Vec<String>>,
}

impl ResponseObserver for Conformance {
    fn observe(&self, r: &ObservedResponse<'_>) {
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

#[derive(Clone)]
pub struct User {
    pub id: UserId,
    pub client: Client,
    pub token: String,
}

pub struct H {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub clock: FakeClock,
    pub vault: VaultService,
    pub state: web::Data<AuthState>,
    pub llm: FakeLlmProvider,
    pub embedder: FakeEmbedder,
    pub ai: Arc<AiService>,
    server: TestServer,
    pub conformance: Arc<Conformance>,
}

pub struct Options {
    pub limits: BudgetLimits,
    /// Register the AI features at all.
    pub ai: bool,
    /// Configure an embedding model.
    pub embeddings: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            limits: BudgetLimits::default(),
            ai: true,
            embeddings: true,
        }
    }
}

impl H {
    pub async fn new() -> Self {
        Self::with(Options::default()).await
    }

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
        let llm = FakeLlmProvider::new().named("claude_cli");
        let per_user: BTreeMap<String, AiProviderKind> =
            [("noai".to_owned(), AiProviderKind::Disabled)].into();
        let router = ProviderRouter::new(AiProviderKind::ClaudeCli, per_user)
            .with_provider(AiProviderKind::ClaudeCli, Arc::new(llm.clone()));
        let budget = BudgetGuard::new(
            opts.limits,
            chrono_tz::UTC,
            Arc::new(clock.clone()),
            Arc::new(MemoryUsageStore::default()),
        );
        let embedder = FakeEmbedder::new(MODEL, 384);
        let embedder_arc: Arc<dyn Embedder> = Arc::new(embedder.clone());
        let mut service = AiService::new(router, budget);
        if opts.embeddings {
            service = service.with_embedder(embedder_arc.clone());
            vault.set_semantic(Arc::new(strata_jobs::semantic_dup::SemanticDupSource::new(
                embedder_arc.clone(),
                dedupe::Thresholds::new(),
            )));
        }
        let ai = Arc::new(service);
        let retriever = opts
            .embeddings
            .then(|| Retriever::new(db.app_db.clone(), embedder_arc.clone()));
        let ask = AskEngine::new(
            db.app_db.clone(),
            vault.clone(),
            ai.clone(),
            retriever.clone(),
            Arc::new(SequentialIdGenerator::default()),
            Arc::new(clock.clone()),
            AskConfig {
                top_k: 3,
                ..AskConfig::default()
            },
        );
        let api = opts.ai.then(|| {
            web::Data::new(AiApi::new(
                ai.clone(),
                db.app_db.clone(),
                retriever,
                ask,
                Arc::new(clock.clone()),
                Arc::new(SequentialIdGenerator::new(1_790_000_000_000)),
            ))
        });
        let for_server = state.clone();
        let vault_data = web::Data::new(vault.clone());
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            if let Some(api) = &api {
                cfg.app_data(api.clone());
            }
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            data,
            clock,
            vault,
            state,
            llm,
            embedder,
            ai,
            server,
            conformance: Arc::new(Conformance {
                contract: Contract::production(),
                violations: Mutex::new(Vec::new()),
            }),
        }
    }

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
            client: self.client(&session.access_token),
            token: session.access_token,
        }
    }

    pub fn client(&self, token: &str) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .tokens(Arc::new(StaticToken(token.to_owned())))
            .build()
            .expect("client")
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.server.addr()
    }

    /// Runs the embedding jobs (and whatever else is queued) until idle.
    pub async fn embed_all(&self) {
        let deps = Deps {
            db: self.db.app_db.clone(),
            vault: self.vault.clone(),
            ai: self.ai.clone(),
            embedder: Some(Arc::new(self.embedder.clone())),
            clock: Arc::new(self.clock.clone()),
            ids: self.db.ids.clone(),
            thresholds: dedupe::Thresholds::new(),
        };
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
                deps.db.clone(),
                deps.vault.clone(),
                deps.embedder.clone(),
                deps.clock.clone(),
                deps.ids.clone(),
                false,
            ))],
        )
        .run_until_idle()
        .await;
    }

    pub fn dir(&self, user: UserId) -> std::path::PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("vault file")
    }

    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    pub fn assert_conformant(&self) {
        assert_eq!(
            *self.conformance.violations.lock().expect("lock"),
            Vec::<String>::new()
        );
    }

    pub async fn finish(self) {
        self.assert_conformant();
        drop(self.server);
        drop(self.state);
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

pub fn problem(err: &Error) -> types::Problem {
    err.api()
        .unwrap_or_else(|| panic!("expected a problem, got {err:?}"))
        .problem()
        .clone()
}

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
