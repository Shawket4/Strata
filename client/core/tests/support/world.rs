//! The production server in process for client-core tests: auth, vault, sync and events, the
//! AI API (fake LLM provider, no embedding model) and the graph API, on the test database's
//! fake clock; plus devices (real client cores with their own app-data directories) that
//! talk to it through the generated client exactly as the app does.

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use actix_web::web;
use strata_ai::{AiService, BudgetGuard, BudgetLimits, MemoryUsageStore, ProviderRouter};
use strata_api::ai::AiApi;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, SigningKeys};
use strata_api::events::BusConfig;
use strata_api::graph::GraphApi;
use strata_api::sync::SyncConfig;
use strata_api::testing::TestServer;
use strata_common::Config;
use strata_common::clock::Clock as _;
use strata_common::config::{AiProviderKind, Argon2Config};
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::client::{BrokenSyncApi, ClientAccountApi, ClientEventsApi, ClientSyncApi};
use strata_core::net::{SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv, NotifyHub, Session};
use strata_core::store::StorePaths;
use strata_core::sync::engine::Trigger;
use strata_core::view::model::{Platform, SignInRequest};
use strata_index::types::UserRole;
use strata_jobs::ask::{AskConfig, AskEngine};
use strata_testkit::{FakeLlmProvider, TempDataRoot, TestDb};
use strata_vault::{VaultConfig, VaultService};
use tempfile::TempDir;

/// The server world of one test.
pub struct World {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub state: web::Data<AuthState>,
    pub vault: VaultService,
    pub llm: FakeLlmProvider,
    pub server: TestServer,
}

impl World {
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
        let llm = FakeLlmProvider::new().named("claude_cli");
        let router = ProviderRouter::new(AiProviderKind::ClaudeCli, BTreeMap::new())
            .with_provider(AiProviderKind::ClaudeCli, Arc::new(llm.clone()));
        let budget = BudgetGuard::new(
            BudgetLimits::default(),
            chrono_tz::UTC,
            Arc::new(clock.clone()),
            Arc::new(MemoryUsageStore::default()),
        );
        let ai = Arc::new(AiService::new(router, budget));
        let ask = AskEngine::new(
            db.app_db.clone(),
            vault.clone(),
            ai.clone(),
            None,
            db.ids.clone(),
            Arc::new(clock.clone()),
            AskConfig::default(),
        );
        let ai_api = web::Data::new(AiApi::new(
            ai,
            db.app_db.clone(),
            None,
            ask,
            Arc::new(clock.clone()),
            db.ids.clone(),
        ));
        let graph = web::Data::new(GraphApi::new(
            strata_graph::GraphService::new(db.app_db.clone(), vault.clone(), None),
            Arc::new(clock.clone()),
            db.ids.clone(),
        ));
        let (bus, sync) =
            strata_api::sync::install(&vault, BusConfig::default(), SyncConfig::default());
        let (for_server, vault_data) = (state.clone(), web::Data::new(vault.clone()));
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            cfg.app_data(vault_data.clone());
            cfg.app_data(bus.clone());
            cfg.app_data(sync.clone());
            cfg.app_data(ai_api.clone());
            cfg.app_data(graph.clone());
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            data,
            state,
            vault,
            llm,
            server,
        }
    }

    /// Creates an active account; returns its ID.
    pub async fn account(&self, name: &str, role: UserRole) -> String {
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
    pub fn device(&self, n: u64) -> Device {
        let dir = TempDir::new().expect("temp dir");
        let now = self.db.clock.now();
        let clock = FakeClock::at(&now.to_rfc3339());
        let base_ms = u64::try_from(now.timestamp_millis()).expect("ms") + n * 60_000;
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
            default_device_name: format!("device {n}"),
            server_url: self.server.base_url(),
            device_timezone: None,
        };
        Device {
            core: Core::open(env).expect("core"),
            name: format!("device {n}"),
            clock,
            dir,
        }
    }

    pub async fn finish(self) {
        drop(self.server);
        drop(self.state);
        self.db.cleanup().await.expect("cleanup");
    }
}

pub fn password(name: &str) -> String {
    format!("{name}-password-1")
}

pub struct Device {
    pub core: Core,
    pub name: String,
    pub clock: FakeClock,
    pub dir: TempDir,
}

impl Device {
    pub async fn sign_in_with(&self, name: &str, password: &str) -> Arc<Session> {
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

    /// Signs in and bootstraps.
    pub async fn sign_in(&self, name: &str) -> Arc<Session> {
        let s = self.sign_in_with(name, &password(name)).await;
        s.sync(Trigger::Start).await.expect("bootstrap");
        s
    }
}
