//! Shared harness of the headless client-core tests: a temp app-data directory, a fake clock,
//! deterministic ULIDs, the fake sync server and the fake account API, wired into a real
//! [`Core`] exactly as production wires the real ones.

#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;

use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::{SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv, Session};
use strata_core::store::StorePaths;
use strata_core::testing::{FakeAccountApi, FakeServer};
use strata_core::view::model::{Platform, SignInRequest};
use tempfile::TempDir;

/// Account A (Shawket).
pub const USER_A: &str = "01K5DSSE00000000000000AAAA";
/// Account B (Mona).
pub const USER_B: &str = "01K5DSSE00000000000000BBBB";
/// The server URL the harness signs in to.
pub const SERVER: &str = "https://strata.example";
/// Base timestamp of the deterministic ULIDs (2026-09-27T10:00:00Z).
pub const ID_BASE_MS: u64 = 1_790_503_200_000;
/// "Now" of every test unless moved.
pub const NOW: &str = "2026-09-27T10:00:00Z";

/// A test core.
pub struct Harness {
    pub dir: TempDir,
    pub server: FakeServer,
    pub accounts: Arc<FakeAccountApi>,
    pub clock: FakeClock,
    pub ids: Arc<SeqIds>,
    pub core: Core,
    pub platform: Platform,
}

pub fn env(
    dir: &Path,
    server: &FakeServer,
    accounts: &Arc<FakeAccountApi>,
    clock: &FakeClock,
    ids: &Arc<SeqIds>,
    platform: Platform,
) -> CoreEnv {
    let server = server.clone();
    let events = server.clone();
    CoreEnv {
        paths: StorePaths::new(dir),
        platform,
        clock: Arc::new(clock.clone()),
        ids: ids.clone(),
        account_api: accounts.clone(),
        events_api: Arc::new(events),
        notifications: strata_core::session::NotifyHub::default(),
        sync_api: Arc::new(move |_url: &str, _tokens: Tokens| -> Arc<dyn SyncApi> {
            Arc::new(server.clone())
        }),
        default_device_name: "Shawket's laptop".to_owned(),
        default_server_url: Some(SERVER.to_owned()),
    }
}

impl Harness {
    /// A fresh install with users A (Africa/Cairo) and B (Europe/London) on the server.
    pub fn new() -> Self {
        Self::with_platform(Platform::Android)
    }

    /// A fresh install on `platform`.
    pub fn with_platform(platform: Platform) -> Self {
        let dir = TempDir::new().expect("temp dir");
        let server = FakeServer::new();
        let accounts = Arc::new(FakeAccountApi::default());
        accounts.add_user("shawket", "pw-a", USER_A, "Africa/Cairo");
        accounts.add_user("mona", "pw-b", USER_B, "Europe/London");
        let clock = FakeClock::at(NOW);
        let ids = Arc::new(SeqIds::new(ID_BASE_MS));
        let core = Core::open(env(dir.path(), &server, &accounts, &clock, &ids, platform))
            .expect("core opens");
        Self {
            dir,
            server,
            accounts,
            clock,
            ids,
            core,
            platform,
        }
    }

    /// Simulates an app restart (or a crash): the core and its connections are dropped and a
    /// new core opens the same files.
    pub fn restart(&mut self) {
        let env = env(
            self.dir.path(),
            &self.server,
            &self.accounts,
            &self.clock,
            &self.ids,
            self.platform,
        );
        // Drop the old core (and its database connections) before opening the files again.
        let fresh = Core::open(env).expect("core reopens");
        self.core = fresh;
    }

    /// Signs in as `username`.
    pub async fn sign_in(&self, username: &str, password: &str) -> Arc<Session> {
        self.core
            .sign_in(SignInRequest {
                server_url: SERVER.to_owned(),
                username: username.to_owned(),
                password: password.to_owned(),
                device_name: "Shawket's laptop".to_owned(),
            })
            .await
            .expect("sign in");
        self.core.session().expect("session")
    }

    /// Signs in as A.
    pub async fn sign_in_a(&self) -> Arc<Session> {
        self.sign_in("shawket", "pw-a").await
    }
}

/// The ULID `SeqIds` hands out as its `n`-th ID (1-based).
pub fn seq_id(n: u128) -> String {
    ulid::Ulid::from_parts(ID_BASE_MS, n).to_string()
}
