//! Shared harness for strata-jobs integration tests: a fresh database, a temp data root with
//! provisioned vaults, the fake LLM provider behind a real `AiService`, a deterministic fake
//! embedder, and a runner over the standard handlers.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeMap;
use std::sync::Arc;

use strata_ai::embed::fake::FakeEmbedder;
use strata_ai::{AiService, BudgetGuard, BudgetLimits, Embedder, MemoryUsageStore, ProviderRouter};
use strata_common::config::AiProviderKind;
use strata_common::{NoteId, SequentialIdGenerator, UserId};
use strata_index::UserScope;
use strata_jobs::{Deps, JobHandler, RecordedEvents, Runner, RunnerConfig};
use strata_testkit::{FakeLlmProvider, TempDataRoot, TestDb, TestUser};
use strata_vault::ops::notes::CreateNote;
use strata_vault::{VaultConfig, VaultService};

/// The embedding model ID of the fake embedder.
pub const MODEL: &str = "fake-embed@1";

/// One test's world.
pub struct World {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub vault: VaultService,
    pub llm: FakeLlmProvider,
    pub embedder: FakeEmbedder,
    pub ai: Arc<AiService>,
    pub events: Arc<RecordedEvents>,
}

impl World {
    pub async fn new() -> Self {
        Self::with_limits(BudgetLimits::default()).await
    }

    pub async fn with_limits(limits: BudgetLimits) -> Self {
        let db = TestDb::new().await.expect("db");
        let data = TempDataRoot::new().expect("data root");
        let vault = VaultService::new(
            VaultConfig::new(data.path()),
            db.app_db.clone(),
            Arc::new(db.clock.clone()),
            db.ids.clone(),
        );
        let llm = FakeLlmProvider::new().named("claude_cli");
        let per_user: BTreeMap<String, AiProviderKind> =
            [("noai".to_owned(), AiProviderKind::Disabled)].into();
        let router = ProviderRouter::new(AiProviderKind::ClaudeCli, per_user)
            .with_provider(AiProviderKind::ClaudeCli, Arc::new(llm.clone()));
        let budget = BudgetGuard::new(
            limits,
            chrono_tz::UTC,
            Arc::new(db.clock.clone()),
            Arc::new(MemoryUsageStore::default()),
        );
        let embedder = FakeEmbedder::new(MODEL, 384);
        let ai = Arc::new(AiService::new(router, budget).with_embedder(Arc::new(embedder.clone())));
        Self {
            db,
            data,
            vault,
            llm,
            embedder,
            ai,
            events: Arc::new(RecordedEvents::default()),
        }
    }

    /// A user with a provisioned vault.
    pub async fn user(&self, name: &str) -> (UserId, UserScope) {
        let user = TestUser::new(name).create(&self.db).await.expect("user").id;
        assert!(self.vault.provision(user).await.expect("provision"));
        (user, self.db.scope(user))
    }

    pub fn embedder_arc(&self) -> Arc<dyn Embedder> {
        Arc::new(self.embedder.clone())
    }

    pub fn deps(&self) -> Deps {
        self.deps_with(Some(self.embedder_arc()))
    }

    pub fn deps_with(&self, embedder: Option<Arc<dyn Embedder>>) -> Deps {
        Deps {
            db: self.db.app_db.clone(),
            vault: self.vault.clone(),
            ai: self.ai.clone(),
            embedder,
            clock: Arc::new(self.db.clock.clone()),
            ids: self.db.ids.clone(),
            thresholds: dedupe::Thresholds::new(),
        }
    }

    pub fn runner(&self, config: RunnerConfig, handlers: Vec<Arc<dyn JobHandler>>) -> Runner {
        Runner::new(
            self.db.app_db.clone(),
            self.db.issuer.clone(),
            Arc::new(self.db.clock.clone()),
            self.events.clone(),
            config,
            handlers,
        )
    }

    /// A runner over the standard handlers with the fake embedder.
    pub fn standard_runner(&self) -> Runner {
        self.runner(
            RunnerConfig::default(),
            strata_jobs::standard_handlers(&self.deps()),
        )
    }

    pub async fn create(&self, scope: &UserScope, path: &str, content: &str) -> NoteId {
        self.vault
            .create_note(
                scope,
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

    pub fn dir(&self, user: UserId) -> std::path::PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("read vault file")
    }

    /// Commit messages, newest first.
    pub fn log(&self, user: UserId) -> Vec<String> {
        strata_vault::git::log(&self.dir(user))
            .expect("log")
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    /// Paths changed by the newest commit.
    pub fn last_commit_paths(&self, user: UserId) -> Vec<String> {
        let head = strata_vault::git::head(&self.dir(user))
            .expect("head")
            .expect("a commit");
        strata_vault::git::changed_paths(&self.dir(user), &head.id).expect("paths")
    }

    /// `(kind, status, attempts)` of every job of `user`, by kind then creation.
    pub async fn jobs(&self, user: UserId) -> Vec<(String, String, i32)> {
        let mut tx = self.db.begin(user).await.expect("tx");
        let rows: Vec<(String, String, i32)> =
            sqlx::query_as("SELECT kind, status, attempts FROM jobs ORDER BY kind, created, id")
                .fetch_all(tx.conn())
                .await
                .expect("jobs");
        tx.commit().await.expect("commit");
        rows
    }

    pub async fn finish(self) {
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// Runner settings for tests that run only some kinds: users whose other due jobs this
/// runner does not handle are not skipped.
pub fn eager() -> RunnerConfig {
    RunnerConfig {
        idle_recheck: chrono::Duration::zero(),
        ..RunnerConfig::default()
    }
}

/// A fresh ID generator (so a test can reproduce a sequence of IDs).
pub fn fresh_ids() -> Arc<SequentialIdGenerator> {
    Arc::new(SequentialIdGenerator::default())
}
