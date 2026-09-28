//! Writes of vault files that are not notes (hook for the graph component, PLAN §6.5, §6.8):
//! saved JSON Canvas layouts in `maps/` (`user:` commits) and `.meta/clusters.json` (`ai:`
//! commits of the `cluster` job). Each write runs on the user's writer actor, checks the
//! expected version of the file there (so read-check-write cannot race another write), runs
//! the caller's index work in the same scoped transaction as the index update of the commit,
//! and is one git commit.
//!
//! Notes, sidecars and trashed files never go through here; they have their own operations.

use futures_util::future::BoxFuture;
use strata_index::{ScopedTx, UserScope};

use crate::error::{Result, VaultError};
use crate::store::{Author, Core, VaultService};
use crate::{fsio, paths};

/// Vault path of the cluster assignment (`vault_format::clusters::CLUSTERS_PATH`).
const CLUSTERS_PATH: &str = vault_format::clusters::CLUSTERS_PATH;

/// What the file must look like before the write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// Anything (create or overwrite).
    Any,
    /// It must not exist (`409 version_conflict` with its version otherwise).
    Absent,
    /// It must exist with this version (`404` when missing, `409` when different).
    Version(String),
}

/// Index work run in the write's scoped transaction, before the commit of that transaction.
pub type IndexHook = Box<dyn for<'a> FnOnce(&'a mut ScopedTx) -> BoxFuture<'a, Result<()>> + Send>;

/// One file write.
pub struct FileWrite {
    /// Vault-relative path: a non-note content file (e.g. `maps/Plan.canvas`) or
    /// `.meta/clusters.json`.
    pub path: String,
    /// New bytes, or `None` to remove the file.
    pub content: Option<Vec<u8>>,
    /// Precondition.
    pub expect: Expect,
    /// Commit author (`user:` / `ai:`).
    pub author: Author,
    /// Operation word of the commit message (`save map`, `cluster`, …).
    pub op: String,
    /// Index work in the same transaction (only run when the file changes).
    pub index: Option<IndexHook>,
}

impl std::fmt::Debug for FileWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWrite")
            .field("path", &self.path)
            .field("bytes", &self.content.as_ref().map(Vec::len))
            .field("expect", &self.expect)
            .field("author", &self.author)
            .field("op", &self.op)
            .field("index", &self.index.is_some())
            .finish()
    }
}

/// The result of a [`FileWrite`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWritten {
    /// The commit, or `None` when the file already had this content (nothing written, the
    /// index hook not run).
    pub commit: Option<String>,
    /// The file's version now (`None` when removed).
    pub version: Option<String>,
}

/// Whether `path` is a note's thread file (`.meta/threads/<ulid>.json`).
fn is_thread_path(path: &str) -> bool {
    path.strip_prefix(".meta/threads/")
        .and_then(|p| p.strip_suffix(".json"))
        .is_some_and(|id| id.parse::<ulid::Ulid>().is_ok())
}

/// Whether `path` may be written by [`VaultService::write_file`].
fn writable(path: &str) -> Result<()> {
    if path == CLUSTERS_PATH || is_thread_path(path) {
        return Ok(());
    }
    paths::validate_path(path)?;
    if path.ends_with(".md") {
        return Err(VaultError::InvalidName(
            "notes are written through the note operations",
        ));
    }
    Ok(())
}

impl Core {
    /// See [`VaultService::write_file`].
    pub async fn write_file(&mut self, scope: UserScope, w: FileWrite) -> Result<FileWritten> {
        writable(&w.path)?;
        let current = self.read(&w.path).await?;
        let current_version = current.as_deref().map(fsio::version_of);
        match (&w.expect, &current_version) {
            (Expect::Absent, Some(v)) => {
                return Err(VaultError::VersionConflict { current: v.clone() });
            }
            (Expect::Version(_), None) => return Err(VaultError::NotFound),
            (Expect::Version(want), Some(v)) if want != v => {
                return Err(VaultError::VersionConflict { current: v.clone() });
            }
            _ => {}
        }
        let version = w.content.as_deref().map(fsio::version_of);
        if current == w.content {
            return Ok(FileWritten {
                commit: None,
                version,
            });
        }
        let mut tx = self.begin(&scope).await?;
        if let Some(hook) = w.index {
            hook(&mut tx).await?;
        }
        let message = w.author.message(&w.op, &w.path);
        let commit = self.finish(tx, vec![(w.path, w.content)], message).await?;
        Ok(FileWritten { commit, version })
    }
}

impl VaultService {
    /// Writes (or removes) one non-note file in one commit, on the user's writer actor; see
    /// the module docs.
    pub async fn write_file(&self, scope: &UserScope, w: FileWrite) -> Result<FileWritten> {
        self.exec(scope, move |core, s| Box::pin(core.write_file(s, w)))
            .await
    }

    /// Reads a vault file (any path inside the vault; `None` when missing). Reads do not
    /// wait for the writer; files are replaced atomically.
    pub async fn read_file_bytes(&self, scope: &UserScope, path: &str) -> Result<Option<Vec<u8>>> {
        self.ready(scope).await?;
        let dir = self.vault_dir(scope.user_id());
        let path = path.to_owned();
        crate::store::blocking(move || Ok(fsio::read(&dir, &path)?)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_non_note_content_files_and_the_clusters_file_are_writable() {
        assert!(writable("maps/Plan.canvas").is_ok());
        assert!(writable(".meta/clusters.json").is_ok());
        assert!(matches!(
            writable("notes/a.md"),
            Err(VaultError::InvalidName(_))
        ));
        for bad in [
            ".meta/notes/x.json",
            ".trash/maps/a.canvas",
            "../x.canvas",
            ".git/config",
        ] {
            assert!(
                matches!(writable(bad), Err(VaultError::InvalidName(_))),
                "{bad}"
            );
        }
    }
}
