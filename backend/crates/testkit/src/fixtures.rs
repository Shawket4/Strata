//! Builders for users and data directories.

use std::path::{Path, PathBuf};

use strata_common::UserId;
use strata_index::accounts::{NewUser, User};
use strata_index::types::{UserRole, UserStatus};

use crate::db::TestDb;
use crate::error::TestkitError;

/// Builds a user row through the accounts pool. Defaults: active member, display name equal to
/// the username, a placeholder password hash, ID from the database's deterministic generator,
/// created at the fake clock's time.
#[derive(Debug, Clone)]
pub struct TestUser {
    username: String,
    display_name: Option<String>,
    role: UserRole,
    status: UserStatus,
    password_hash: String,
    id: Option<UserId>,
}

impl TestUser {
    /// An active member named `username`.
    pub fn new(username: &str) -> Self {
        Self {
            username: username.to_owned(),
            display_name: None,
            role: UserRole::Member,
            status: UserStatus::Active,
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$dGVzdA$dGVzdA".to_owned(),
            id: None,
        }
    }

    /// Makes the user an admin.
    #[must_use]
    pub fn admin(mut self) -> Self {
        self.role = UserRole::Admin;
        self
    }

    /// Creates the user in `pending` state (self-signup, D22).
    #[must_use]
    pub fn pending(mut self) -> Self {
        self.status = UserStatus::Pending;
        self
    }

    /// Sets the display name.
    #[must_use]
    pub fn display_name(mut self, name: &str) -> Self {
        self.display_name = Some(name.to_owned());
        self
    }

    /// Sets the stored password hash.
    #[must_use]
    pub fn password_hash(mut self, hash: &str) -> Self {
        self.password_hash = hash.to_owned();
        self
    }

    /// Uses a fixed ID instead of the next deterministic one.
    #[must_use]
    pub fn id(mut self, id: UserId) -> Self {
        self.id = Some(id);
        self
    }

    /// Inserts the user and returns the stored row.
    pub async fn create(self, db: &TestDb) -> Result<User, TestkitError> {
        let id = self.id.unwrap_or_else(|| UserId::generate(db.ids.as_ref()));
        let new = NewUser {
            id,
            username_normalized: self.username.to_lowercase(),
            display_name: self.display_name.unwrap_or_else(|| self.username.clone()),
            username: self.username,
            password_hash: self.password_hash,
            role: self.role,
            status: self.status,
            approved_by: None,
        };
        Ok(db
            .accounts_db
            .create_user(&new, strata_common::Clock::now(&db.clock))
            .await?)
    }
}

/// A temporary data root laid out like `/srv/strata` (PLAN §5.2), removed on drop.
#[derive(Debug)]
pub struct TempDataRoot {
    dir: tempfile::TempDir,
}

impl TempDataRoot {
    /// A new empty data root.
    pub fn new() -> Result<Self, TestkitError> {
        Ok(Self {
            dir: tempfile::Builder::new().prefix("strata-data-").tempdir()?,
        })
    }

    /// The root path.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// `users/<user_id>/vault`, created if missing.
    pub fn vault_dir(&self, user: UserId) -> Result<PathBuf, TestkitError> {
        let path = self.dir.path().join("users").join(user.to_string()).join("vault");
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }
}
