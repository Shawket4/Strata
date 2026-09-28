//! The write journal: makes one vault write all-or-nothing across a crash (PLAN §7.2, §7.3).
//!
//! Before a write touches any file, the writer records the `HEAD` commit and the paths it is
//! about to change in `.git/strata-write-journal` (atomically, outside the working tree); the
//! journal is removed right after the write's git commit. Finding a journal on load means the
//! process stopped in between:
//!
//! - `HEAD` is still the recorded commit: the write never committed. Its paths are restored to
//!   their `HEAD` content (or removed if `HEAD` has none), so a half-applied write — some files
//!   written, others not, nothing committed, the index not updated — disappears and the
//!   replayed request applies it once, from the start.
//! - `HEAD` moved: the commit happened (reconciliation then brings the index and the op
//!   results up to date, see [`crate::receipt`]); the journal is simply removed.
//!
//! Changes made outside the API are not journaled and are still committed as
//! `system: recovered changes`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, VaultError};
use crate::{fsio, git};

/// Journal location, relative to the vault (inside `.git/`, never part of the tree).
pub const JOURNAL: &str = ".git/strata-write-journal";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Journal {
    /// `HEAD` before the write (`None` in an empty repository).
    head: Option<String>,
    /// Vault-relative paths the write changes.
    paths: Vec<String>,
}

/// Records the write about to change `paths` (blocking).
pub fn begin(dir: &Path, paths: &[String]) -> Result<()> {
    let journal = Journal {
        head: git::head(dir)?.map(|c| c.id),
        paths: paths.to_vec(),
    };
    let bytes = serde_json::to_vec(&journal)
        .map_err(|e| VaultError::Internal(format!("write journal: {e}")))?;
    fsio::atomic_write(dir, JOURNAL, &bytes)?;
    Ok(())
}

/// Removes the journal after the write's commit (blocking).
pub fn end(dir: &Path) -> Result<()> {
    fsio::remove(dir, JOURNAL)?;
    Ok(())
}

/// Recovers from an interrupted write, if a journal is present (blocking): restores the
/// paths of a write that never committed and returns those whose content changed back, sorted.
/// An unreadable journal is discarded (nothing can be attributed to it).
pub fn recover(dir: &Path) -> Result<Vec<String>> {
    let Some(bytes) = fsio::read(dir, JOURNAL)? else {
        return Ok(Vec::new());
    };
    let mut restored = Vec::new();
    if let Ok(journal) = serde_json::from_slice::<Journal>(&bytes) {
        let head = git::head(dir)?.map(|c| c.id);
        if head == journal.head {
            for path in &journal.paths {
                let committed = match &head {
                    Some(h) => git::blob_at(dir, h, path)?,
                    None => None,
                };
                let now = fsio::read(dir, path)?;
                if now == committed {
                    continue;
                }
                match &committed {
                    Some(b) => fsio::atomic_write(dir, path, b)?,
                    None => fsio::remove(dir, path)?,
                }
                restored.push(path.clone());
            }
        }
    }
    end(dir)?;
    restored.sort();
    restored.dedup();
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use pretty_assertions::assert_eq;

    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        git::init(tmp.path()).expect("init");
        fsio::atomic_write(tmp.path(), "notes/a.md", b"one").expect("write");
        let at = Utc
            .with_ymd_and_hms(2026, 9, 27, 12, 0, 0)
            .single()
            .expect("ts");
        git::commit_paths(
            tmp.path(),
            &["notes/a.md".into()],
            "user: create notes/a.md",
            at,
        )
        .expect("commit");
        tmp
    }

    #[test]
    fn an_uncommitted_write_is_rolled_back() {
        let tmp = repo();
        let dir = tmp.path();
        let paths = vec![
            "notes/a.md".to_owned(),
            "notes/b.md".to_owned(),
            "notes/c.md".to_owned(),
        ];
        begin(dir, &paths).expect("journal");
        fsio::atomic_write(dir, "notes/a.md", b"two").expect("write");
        fsio::atomic_write(dir, "notes/b.md", b"new").expect("write");
        // notes/c.md was never written (the crash came first).
        assert_eq!(
            recover(dir).expect("recover"),
            vec!["notes/a.md".to_owned(), "notes/b.md".to_owned()]
        );
        assert_eq!(
            fsio::read(dir, "notes/a.md").expect("read"),
            Some(b"one".to_vec())
        );
        assert_eq!(fsio::read(dir, "notes/b.md").expect("read"), None);
        assert_eq!(fsio::read(dir, JOURNAL).expect("read"), None);
        assert_eq!(git::dirty_paths(dir).expect("dirty"), Vec::<String>::new());
    }

    #[test]
    fn a_committed_write_is_kept() {
        let tmp = repo();
        let dir = tmp.path();
        begin(dir, &["notes/a.md".to_owned()]).expect("journal");
        fsio::atomic_write(dir, "notes/a.md", b"two").expect("write");
        let at = Utc
            .with_ymd_and_hms(2026, 9, 27, 12, 1, 0)
            .single()
            .expect("ts");
        git::commit_paths(dir, &["notes/a.md".into()], "user: update notes/a.md", at)
            .expect("commit");
        assert_eq!(recover(dir).expect("recover"), Vec::<String>::new());
        assert_eq!(
            fsio::read(dir, "notes/a.md").expect("read"),
            Some(b"two".to_vec())
        );
        assert_eq!(fsio::read(dir, JOURNAL).expect("read"), None);
    }

    #[test]
    fn no_journal_and_a_garbled_one_change_nothing() {
        let tmp = repo();
        let dir = tmp.path();
        assert_eq!(recover(dir).expect("recover"), Vec::<String>::new());
        fsio::atomic_write(dir, "notes/a.md", b"edited out of band").expect("write");
        fsio::atomic_write(dir, JOURNAL, b"not json").expect("write");
        assert_eq!(recover(dir).expect("recover"), Vec::<String>::new());
        assert_eq!(
            fsio::read(dir, "notes/a.md").expect("read"),
            Some(b"edited out of band".to_vec())
        );
        assert_eq!(fsio::read(dir, JOURNAL).expect("read"), None);
    }
}
