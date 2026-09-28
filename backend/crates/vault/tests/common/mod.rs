//! Shared harness: a fresh database, a temp data root and a provisioned vault per user.
#![allow(dead_code, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::sync::Arc;

use strata_common::UserId;
use strata_index::UserScope;
use strata_testkit::{TempDataRoot, TestDb, TestUser};
use strata_vault::{VaultConfig, VaultService};

/// One test's world.
pub struct World {
    pub db: TestDb,
    pub data: TempDataRoot,
    pub vault: VaultService,
}

impl World {
    pub async fn new() -> Self {
        let db = TestDb::new().await.expect("db");
        let data = TempDataRoot::new().expect("data root");
        let vault = VaultService::new(
            VaultConfig::new(data.path()),
            db.app_db.clone(),
            Arc::new(db.clock.clone()),
            db.ids.clone(),
        );
        Self { db, data, vault }
    }

    /// A user with a provisioned vault.
    pub async fn user(&self, name: &str) -> (UserId, UserScope) {
        let user = TestUser::new(name).create(&self.db).await.expect("user").id;
        assert!(self.vault.provision(user).await.expect("provision"));
        (user, self.db.scope(user))
    }

    pub fn dir(&self, user: UserId) -> PathBuf {
        self.vault.vault_dir(user)
    }

    pub fn read(&self, user: UserId, rel: &str) -> String {
        std::fs::read_to_string(self.dir(user).join(rel)).expect("read vault file")
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

    /// Paths changed by the newest commit.
    pub fn last_commit_paths(&self, user: UserId) -> Vec<String> {
        let head = strata_vault::git::head(&self.dir(user))
            .expect("head")
            .expect("a commit");
        strata_vault::git::changed_paths(&self.dir(user), &head.id).expect("paths")
    }

    /// Every derived row of the user.
    pub async fn snapshot(&self, user: UserId) -> Vec<(String, Vec<String>)> {
        let mut tx = self.db.begin(user).await.expect("tx");
        let s = strata_index::repo::vault::derived_snapshot(&mut tx)
            .await
            .expect("snapshot");
        tx.commit().await.expect("commit");
        s
    }

    pub async fn finish(self) {
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// The frontmatter Strata writes for a new note created at the test epoch in UTC.
pub fn header(id: &str) -> String {
    format!(
        "---\nid: {id}\ncreated: 2026-09-27T12:00:00Z\nupdated: 2026-09-27T12:00:00Z\n---\n"
    )
}
