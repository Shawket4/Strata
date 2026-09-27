//! Shared harness for the auth/account API tests: a fresh database, a temp data root, the
//! production app with a fake clock on an in-process server, and generated clients whose
//! every response is validated against the production contract.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)] // shared by several test binaries

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use actix_web::web;
use strata_api::auth::service::NewAccount;
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{AuthDeps, AuthState, DataRoot, GitVaultProvisioner, SigningKeys};
use strata_api::contract::Contract;
use strata_api::testing::TestServer;
use strata_client::{
    ApiError, Client, Error, ObservedResponse, ResponseObserver, StaticToken, operations as ops,
    types,
};
use strata_common::config::Argon2Config;
use strata_common::{Clock, Config, FakeClock, UserId};
use strata_index::types::UserRole;
use strata_testkit::{TempDataRoot, TestDb};

/// Validates every response against the production contract.
#[derive(Debug)]
pub struct Conformance {
    contract: Contract,
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

/// One test's world.
pub struct Harness {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub clock: FakeClock,
    pub state: web::Data<AuthState>,
    pub config: Config,
    server: TestServer,
    conformance: Arc<Conformance>,
}

/// Cheap Argon2 so tests stay fast (the production default is 19 MiB × 2).
pub fn test_config(data_root: PathBuf) -> Config {
    let mut config = Config {
        data_root,
        ..Config::default()
    };
    config.auth.argon2 = Argon2Config {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };
    config
}

impl Harness {
    /// A harness with the default test configuration.
    pub async fn new() -> Self {
        Self::with_config(|_| {}).await
    }

    /// A harness whose configuration `tweak` adjusts.
    pub async fn with_config(tweak: impl FnOnce(&mut Config)) -> Self {
        let db = TestDb::new().await.expect("test database");
        let data = TempDataRoot::new().expect("data root");
        let mut config = test_config(data.path().to_path_buf());
        tweak(&mut config);
        let clock = db.clock.clone();
        let keys = SigningKeys::from_pem(&generate_key_pem().expect("key")).expect("key");
        let state = AuthState::new(
            AuthDeps {
                accounts_pool: db.accounts.clone(),
                app_db: db.app_db.clone(),
                issuer: db.issuer.clone(),
                keys,
                vaults: Arc::new(GitVaultProvisioner::new(DataRoot::new(data.path()))),
                clock: Arc::new(clock.clone()),
                ids: db.ids.clone(),
            },
            &config,
        )
        .expect("auth state");
        let state = web::Data::new(state);
        let for_server = state.clone();
        let server = TestServer::start(move |cfg| {
            cfg.app_data(for_server.clone());
            strata_api::app::configure(cfg);
        })
        .expect("server");
        Self {
            db,
            data,
            clock,
            state,
            config,
            server,
            conformance: Arc::new(Conformance {
                contract: Contract::production(),
                violations: Mutex::new(Vec::new()),
                seen: Mutex::new(Vec::new()),
            }),
        }
    }

    /// An unauthenticated client.
    pub fn anon(&self) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .build()
            .expect("client")
    }

    /// A client sending `access_token`.
    pub fn with_token(&self, access_token: &str) -> Client {
        Client::builder(&self.server.base_url())
            .observer(self.conformance.clone())
            .tokens(Arc::new(StaticToken(access_token.to_owned())))
            .build()
            .expect("client")
    }

    /// A client sending `access_token` whose responses are not contract-checked (for probing
    /// paths that are not operations at all).
    pub fn unchecked_with_token(&self, access_token: &str) -> Client {
        Client::builder(&self.server.base_url())
            .tokens(Arc::new(StaticToken(access_token.to_owned())))
            .build()
            .expect("client")
    }

    /// Base URL of the server.
    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    /// Creates an active account directly (as `stratad create-user` does).
    pub async fn create_user(&self, username: &str, password: &str, role: UserRole) -> UserId {
        self.state
            .create_account(
                &NewAccount {
                    username,
                    display_name: username,
                    password,
                    role,
                },
                None,
            )
            .await
            .expect("account created")
            .id
    }

    /// Logs in and returns the session.
    pub async fn login(&self, username: &str, password: &str) -> types::AuthSession {
        self.try_login(username, password).await.expect("login")
    }

    /// Logs in as device `test-device` on Linux.
    pub async fn try_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<types::AuthSession, Error> {
        ops::login(
            &self.anon(),
            &types::LoginRequest {
                username: username.to_owned(),
                password: password.to_owned(),
                device_name: "test-device".to_owned(),
                platform: types::DevicePlatform::Linux,
            },
        )
        .await
    }

    /// An admin account `admin` / `admin-password-1` and a client for it.
    pub async fn admin(&self) -> (UserId, Client) {
        let id = self
            .create_user("admin", "admin-password-1", UserRole::Admin)
            .await;
        let session = self.login("admin", "admin-password-1").await;
        (id, self.with_token(&session.access_token))
    }

    /// `<data_root>/users/<id>/vault`.
    pub fn vault_dir(&self, user: UserId) -> PathBuf {
        self.data
            .path()
            .join("users")
            .join(user.to_string())
            .join("vault")
    }

    /// Current fake time.
    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }

    /// Every contract violation seen so far must be none.
    pub fn assert_conformant(&self) {
        assert_eq!(
            *self.conformance.violations.lock().expect("lock"),
            Vec::<String>::new()
        );
        assert!(!self.conformance.seen.lock().expect("lock").is_empty());
    }

    /// Checks conformance, stops the server and drops the database.
    pub async fn finish(self) {
        self.assert_conformant();
        drop(self.server);
        drop(self.state);
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

/// `422 invalid_body` with one field error.
pub fn invalid_field(code: &str, pointer: &str, message: &str) -> types::Problem {
    types::Problem {
        errors: vec![types::ProblemFieldError {
            code: code.to_owned(),
            message: message.to_owned(),
            pointer: Some(pointer.to_owned()),
        }],
        ..plain(
            "invalid_body",
            "Request body is invalid",
            422,
            Some(message),
        )
    }
}

/// Asserts `result` failed with exactly `expected`.
pub fn assert_problem<T: std::fmt::Debug>(result: Result<T, Error>, expected: &types::Problem) {
    let err = result.expect_err("expected a problem");
    assert_eq!(&problem(&err), expected);
}

/// `401 unauthorized` with `detail`.
pub fn unauthorized(detail: &str) -> types::Problem {
    plain("unauthorized", "Authentication required", 401, Some(detail))
}

/// True if the error is the given typed API error variant.
pub fn is_api(err: &Error, f: impl Fn(&ApiError) -> bool) -> bool {
    err.api().is_some_and(f)
}
