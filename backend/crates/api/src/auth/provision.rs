//! Per-user vault directories (PLAN §5.2: `<data_root>/users/<user_id>/vault`, a git
//! repository; D22: created on approval).
//!
//! Account management only needs two things from the vault layer: create a user's vault when
//! the account becomes active, and remove the user's directory when the account is purged.
//! [`VaultProvisioner`] is that seam; the vault store (`strata-vault`) owns everything inside
//! the directory and can supply its own implementation. [`GitVaultProvisioner`] is the basic
//! one: create the directory (mode 0700) and `git init` it.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use futures_util::future::BoxFuture;
use strata_common::UserId;

/// The data root laid out per PLAN §5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    root: PathBuf,
}

impl DataRoot {
    /// A data root at `root` (e.g. `/srv/strata`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The root path.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// `<root>/users`.
    pub fn users_dir(&self) -> PathBuf {
        self.root.join("users")
    }

    /// `<root>/users/<user_id>`.
    pub fn user_dir(&self, user: UserId) -> PathBuf {
        self.users_dir().join(user.to_string())
    }

    /// `<root>/users/<user_id>/vault`.
    pub fn vault_dir(&self, user: UserId) -> PathBuf {
        self.user_dir(user).join("vault")
    }
}

/// Creates and removes per-user vault directories.
pub trait VaultProvisioner: Send + Sync + fmt::Debug {
    /// Creates the user's vault if it does not exist yet. Returns `true` if it was created
    /// now, `false` if it already existed. Idempotent.
    fn provision(&self, user: UserId) -> BoxFuture<'_, io::Result<bool>>;

    /// Removes the user's whole directory (vault, git history, anything else). Succeeds if
    /// it does not exist.
    fn deprovision(&self, user: UserId) -> BoxFuture<'_, io::Result<()>>;

    /// The user's vault directory (read by `GET /me/export`).
    fn vault_dir(&self, user: UserId) -> PathBuf;
}

/// Basic provisioner: `mkdir -m 0700` + `git init`.
#[derive(Debug, Clone)]
pub struct GitVaultProvisioner {
    root: DataRoot,
}

impl GitVaultProvisioner {
    /// A provisioner under `root`.
    pub fn new(root: DataRoot) -> Self {
        Self { root }
    }
}

async fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(path).await
}

impl VaultProvisioner for GitVaultProvisioner {
    fn provision(&self, user: UserId) -> BoxFuture<'_, io::Result<bool>> {
        Box::pin(async move {
            let vault = self.root.vault_dir(user);
            if tokio::fs::try_exists(vault.join(".git")).await? {
                return Ok(false);
            }
            create_private_dir(&vault).await?;
            let output = tokio::process::Command::new("git")
                .arg("init")
                .arg("--quiet")
                .arg("--initial-branch=main")
                .arg(&vault)
                .env("GIT_TERMINAL_PROMPT", "0")
                .kill_on_drop(true)
                .output()
                .await?;
            if !output.status.success() {
                return Err(io::Error::other(format!(
                    "git init failed with {}",
                    output.status
                )));
            }
            Ok(true)
        })
    }

    fn deprovision(&self, user: UserId) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let dir = self.root.user_dir(user);
            // Move it out of the way first so a half-removed tree is never mistaken for a
            // live vault, then delete it.
            let doomed = self.root.users_dir().join(format!(".purging-{user}"));
            match tokio::fs::rename(&dir, &doomed).await {
                Ok(()) => tokio::fs::remove_dir_all(&doomed).await,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    match tokio::fs::remove_dir_all(&doomed).await {
                        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                        other => other,
                    }
                }
                Err(e) => Err(e),
            }
        })
    }

    fn vault_dir(&self, user: UserId) -> PathBuf {
        self.root.vault_dir(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserId {
        "01M3HBS0G00000000000000001".parse().expect("ulid")
    }

    #[tokio::test]
    async fn provisioning_creates_a_private_git_repository_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let p = GitVaultProvisioner::new(DataRoot::new(tmp.path()));
        assert!(p.provision(user()).await.expect("created"));
        assert!(!p.provision(user()).await.expect("exists"));
        let vault = tmp
            .path()
            .join("users/01M3HBS0G00000000000000001/vault");
        assert_eq!(p.vault_dir(user()), vault);
        assert!(vault.join(".git/HEAD").is_file());
        assert_eq!(
            std::fs::read_to_string(vault.join(".git/HEAD")).expect("HEAD"),
            "ref: refs/heads/main\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&vault).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[tokio::test]
    async fn deprovisioning_removes_the_user_directory_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = DataRoot::new(tmp.path());
        let p = GitVaultProvisioner::new(root.clone());
        let other: UserId = "01M3HBS0G00000000000000002".parse().expect("ulid");
        p.provision(user()).await.expect("created");
        p.provision(other).await.expect("created");
        p.deprovision(user()).await.expect("removed");
        assert!(!root.user_dir(user()).exists());
        assert!(root.vault_dir(other).join(".git").is_dir());
        assert_eq!(
            std::fs::read_dir(root.users_dir()).expect("dir").count(),
            1
        );
        p.deprovision(user()).await.expect("idempotent");
    }
}
