//! The per-vault git repository (PLAN §7.2, L10): one commit per logical operation, file
//! history across renames, files at a revision, and reverts. Blocking (libgit2); callers run
//! these on the blocking pool. The repository has no remotes.

use std::path::Path;

use chrono::{DateTime, Utc};
use git2::{
    DiffFindOptions, IndexAddOption, Oid, Repository, RepositoryInitOptions, Signature,
    StatusOptions, Time,
};

use crate::error::{Result, VaultError};
use crate::paths::TEMP_PREFIX;

/// Author and committer of every commit (the message prefix says who acted).
const NAME: &str = "Strata";
const EMAIL: &str = "strata@localhost";

/// One commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// Full hex ID.
    pub id: String,
    /// Message (`user: …`, `ai: …`, `system: …`), without the `Strata-*` trailers of a
    /// pushed op (see [`crate::receipt`]).
    pub message: String,
    /// Commit time.
    pub at: DateTime<Utc>,
}

/// How a commit changed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Created.
    Added,
    /// Content changed.
    Modified,
    /// Moved or renamed (the content may also have changed).
    Renamed,
    /// Removed.
    Deleted,
}

/// A commit that touched a file, with the file's path in that commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRevision {
    /// The commit.
    pub commit: CommitInfo,
    /// Path of the file in that commit (for `Deleted`: the path it was deleted from).
    pub path: String,
    /// Kind of change.
    pub change: Change,
}

fn info(c: &git2::Commit<'_>) -> CommitInfo {
    CommitInfo {
        id: c.id().to_string(),
        message: crate::receipt::strip_trailers(c.message().unwrap_or_default()).to_owned(),
        at: DateTime::from_timestamp(c.time().seconds(), 0).unwrap_or_default(),
    }
}

fn open(dir: &Path) -> Result<Repository> {
    Ok(Repository::open(dir)?)
}

/// Creates the repository (branch `main`) if missing and excludes temporary files. Returns
/// whether it was created now.
pub fn init(dir: &Path) -> Result<bool> {
    if dir.join(".git").is_dir() {
        return Ok(false);
    }
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main").mkdir(true).no_reinit(true);
    Repository::init_opts(dir, &opts)?;
    configure(dir)?;
    Ok(true)
}

/// Sets the repository options the store relies on (no CRLF conversion, no mode tracking,
/// temp files excluded). Idempotent; also applied to repositories created elsewhere.
pub fn configure(dir: &Path) -> Result<()> {
    let repo = open(dir)?;
    let mut config = repo.config()?;
    config.set_bool("core.autocrlf", false)?;
    config.set_bool("core.filemode", false)?;
    let info = dir.join(".git").join("info");
    std::fs::create_dir_all(&info)?;
    std::fs::write(info.join("exclude"), format!("{TEMP_PREFIX}*\n"))?;
    Ok(())
}

fn signature(at: DateTime<Utc>) -> Result<Signature<'static>> {
    Ok(Signature::new(NAME, EMAIL, &Time::new(at.timestamp(), 0))?)
}

fn head_commit(repo: &Repository) -> Option<git2::Commit<'_>> {
    repo.head().ok()?.peel_to_commit().ok()
}

fn commit_index(
    repo: &Repository,
    index: &mut git2::Index,
    message: &str,
    at: DateTime<Utc>,
) -> Result<Option<String>> {
    index.write()?;
    let tree_id = index.write_tree()?;
    let parent = head_commit(repo);
    if parent.as_ref().is_some_and(|p| p.tree_id() == tree_id) {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id)?;
    let sig = signature(at)?;
    let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
    Ok(Some(oid.to_string()))
}

/// Stages exactly `paths` (added/modified if the file exists, removed otherwise) and
/// commits. Returns the commit ID, or `None` if nothing changed.
pub fn commit_paths(
    dir: &Path,
    paths: &[String],
    message: &str,
    at: DateTime<Utc>,
) -> Result<Option<String>> {
    let repo = open(dir)?;
    let mut index = repo.index()?;
    for p in paths {
        let full = crate::fsio::resolve(dir, p);
        let is_file = std::fs::symlink_metadata(&full).is_ok_and(|m| m.is_file());
        if is_file {
            index.add_path(Path::new(p))?;
        } else if index.get_path(Path::new(p), 0).is_some() {
            index.remove_path(Path::new(p))?;
        }
    }
    commit_index(&repo, &mut index, message, at)
}

/// Stages every change in the working tree (new, modified, deleted) and commits.
pub fn commit_all(dir: &Path, message: &str, at: DateTime<Utc>) -> Result<Option<String>> {
    let repo = open(dir)?;
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    commit_index(&repo, &mut index, message, at)
}

/// Paths whose working-tree state differs from `HEAD` (modified, new, deleted), sorted.
pub fn dirty_paths(dir: &Path) -> Result<Vec<String>> {
    let repo = open(dir)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    let mut out: Vec<String> = statuses
        .iter()
        .filter(|s| !s.status().is_ignored())
        .filter_map(|s| s.path().ok().map(str::to_owned))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// The `HEAD` commit, if any.
pub fn head(dir: &Path) -> Result<Option<CommitInfo>> {
    let repo = open(dir)?;
    Ok(head_commit(&repo).map(|c| info(&c)))
}

/// Every commit reachable from `HEAD`, newest first.
pub fn log(dir: &Path) -> Result<Vec<CommitInfo>> {
    let repo = open(dir)?;
    if head_commit(&repo).is_none() {
        return Ok(Vec::new());
    }
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    let mut out = Vec::new();
    for oid in walk {
        out.push(info(&repo.find_commit(oid?)?));
    }
    Ok(out)
}

/// The commits after `stop` up to `HEAD` along first parents, oldest first, as (ID, full raw
/// message). With `stop` absent or not found, at most `limit` commits are walked (the newest
/// ones).
pub fn commits_since(
    dir: &Path,
    stop: Option<&str>,
    limit: usize,
) -> Result<Vec<(String, String)>> {
    let repo = open(dir)?;
    if head_commit(&repo).is_none() {
        return Ok(Vec::new());
    }
    let stop = stop.and_then(|s| Oid::from_str(s).ok());
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.simplify_first_parent()?;
    let mut out = Vec::new();
    for oid in walk {
        let oid = oid?;
        if Some(oid) == stop || out.len() == limit {
            break;
        }
        let commit = repo.find_commit(oid)?;
        out.push((
            oid.to_string(),
            commit.message().unwrap_or_default().to_owned(),
        ));
    }
    out.reverse();
    Ok(out)
}

/// Parses a full 40-character hex commit ID of a commit in this repository.
pub fn find_commit(dir: &Path, id: &str) -> Result<Option<CommitInfo>> {
    if id.len() != 40 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(None);
    }
    let repo = open(dir)?;
    let Ok(oid) = Oid::from_str(id) else {
        return Ok(None);
    };
    Ok(repo.find_commit(oid).ok().map(|c| info(&c)))
}

fn entry_id(tree: &git2::Tree<'_>, path: &str) -> Option<Oid> {
    tree.get_path(Path::new(path)).ok().map(|e| e.id())
}

/// The commits that touched the file currently at `path`, newest first, following renames
/// (moves, soft delete to `.trash/`, restore).
pub fn history(dir: &Path, path: &str) -> Result<Vec<FileRevision>> {
    let repo = open(dir)?;
    if head_commit(&repo).is_none() {
        return Ok(Vec::new());
    }
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.simplify_first_parent()?;
    let mut current = path.to_owned();
    let mut out = Vec::new();
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        let tree = commit.tree()?;
        let parent_tree = match commit.parent(0) {
            Ok(p) => Some(p.tree()?),
            Err(_) => None,
        };
        let now = entry_id(&tree, &current);
        let before = parent_tree.as_ref().and_then(|t| entry_id(t, &current));
        if now == before {
            continue;
        }
        match (before, now) {
            (Some(_), Some(_)) => out.push(FileRevision {
                commit: info(&commit),
                path: current.clone(),
                change: Change::Modified,
            }),
            (Some(_), None) => out.push(FileRevision {
                commit: info(&commit),
                path: current.clone(),
                change: Change::Deleted,
            }),
            (None, Some(_)) => {
                let renamed_from = parent_tree
                    .as_ref()
                    .map(|pt| rename_source(&repo, pt, &tree, &current))
                    .transpose()?
                    .flatten();
                if let Some(old) = renamed_from {
                    out.push(FileRevision {
                        commit: info(&commit),
                        path: current.clone(),
                        change: Change::Renamed,
                    });
                    current = old;
                } else {
                    out.push(FileRevision {
                        commit: info(&commit),
                        path: current.clone(),
                        change: Change::Added,
                    });
                    break;
                }
            }
            (None, None) => {}
        }
    }
    Ok(out)
}

fn rename_source(
    repo: &Repository,
    old: &git2::Tree<'_>,
    new: &git2::Tree<'_>,
    path: &str,
) -> Result<Option<String>> {
    let mut diff = repo.diff_tree_to_tree(Some(old), Some(new), None)?;
    let mut find = DiffFindOptions::new();
    find.renames(true).rename_threshold(30);
    diff.find_similar(Some(&mut find))?;
    for delta in diff.deltas() {
        if delta.status() == git2::Delta::Renamed
            && delta.new_file().path().and_then(Path::to_str) == Some(path)
        {
            return Ok(delta
                .old_file()
                .path()
                .and_then(Path::to_str)
                .map(str::to_owned));
        }
    }
    Ok(None)
}

/// The content of `path` in `commit`.
pub fn blob_at(dir: &Path, commit: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let repo = open(dir)?;
    let Ok(oid) = Oid::from_str(commit) else {
        return Ok(None);
    };
    let Ok(c) = repo.find_commit(oid) else {
        return Ok(None);
    };
    let tree = c.tree()?;
    let Ok(entry) = tree.get_path(Path::new(path)) else {
        return Ok(None);
    };
    let Ok(blob) = repo.find_blob(entry.id()) else {
        return Ok(None);
    };
    Ok(Some(blob.content().to_vec()))
}

/// Paths a commit changed relative to its first parent (both sides of renames), sorted.
pub fn changed_paths(dir: &Path, commit: &str) -> Result<Vec<String>> {
    let repo = open(dir)?;
    let c = repo.find_commit(Oid::from_str(commit)?)?;
    let tree = c.tree()?;
    let parent = match c.parent(0) {
        Ok(p) => Some(p.tree()?),
        Err(_) => None,
    };
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;
    let mut out = Vec::new();
    for d in diff.deltas() {
        for f in [d.old_file(), d.new_file()] {
            if let Some(p) = f.path().and_then(Path::to_str) {
                out.push(p.to_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// A file change to apply to the working tree: new content, or `None` to delete.
pub type FileChange = (String, Option<Vec<u8>>);

/// The working-tree changes that revert `commit` on top of `HEAD` (a three-way revert, so
/// later unrelated edits to the same files survive). Fails with
/// [`VaultError::RevertConflict`] when the revert does not merge cleanly, and with
/// [`VaultError::NotFound`] for a root commit (nothing to revert to).
pub fn revert_changes(dir: &Path, commit: &str) -> Result<Vec<FileChange>> {
    let repo = open(dir)?;
    let target = repo.find_commit(Oid::from_str(commit)?)?;
    if target.parent_count() == 0 {
        return Err(VaultError::NotFound);
    }
    let ours = head_commit(&repo).ok_or(VaultError::NotFound)?;
    let mut index = repo.revert_commit(&target, &ours, 0, None)?;
    if index.has_conflicts() {
        return Err(VaultError::RevertConflict);
    }
    let new_tree = repo.find_tree(index.write_tree_to(&repo)?)?;
    let head_tree = ours.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&head_tree), Some(&new_tree), None)?;
    let mut out = Vec::new();
    for d in diff.deltas() {
        let new = d.new_file();
        let old = d.old_file();
        if new.exists() {
            let path = new
                .path()
                .and_then(Path::to_str)
                .ok_or_else(|| VaultError::Internal("non-UTF-8 path in revert".into()))?;
            let blob = repo.find_blob(new.id())?;
            out.push((path.to_owned(), Some(blob.content().to_vec())));
        }
        if old.exists()
            && let Some(path) = old.path().and_then(Path::to_str)
            && !new.exists()
        {
            out.push((path.to_owned(), None));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsio::{atomic_write, remove};

    fn t(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).expect("ts")
    }

    #[test]
    fn commits_history_and_revert() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        assert!(init(dir).expect("init"));
        assert!(!init(dir).expect("again"));
        atomic_write(dir, "notes/a.md", b"one\ntwo\nthree\n").expect("w");
        let c1 = commit_paths(
            dir,
            &["notes/a.md".into()],
            "user: create notes/a.md",
            t(100),
        )
        .expect("commit")
        .expect("changed");
        assert_eq!(
            commit_paths(dir, &["notes/a.md".into()], "noop", t(101)).expect("commit"),
            None
        );
        atomic_write(dir, "notes/a.md", b"one\nTWO\nthree\n").expect("w");
        let c2 = commit_paths(dir, &["notes/a.md".into()], "ai: link notes/a.md", t(200))
            .expect("commit")
            .expect("changed");
        // A later unrelated edit survives reverting the ai commit.
        atomic_write(dir, "notes/a.md", b"zero\n\none\nTWO\nthree\n").expect("w");
        let c3 = commit_paths(
            dir,
            &["notes/a.md".into()],
            "user: update notes/a.md",
            t(300),
        )
        .expect("commit")
        .expect("changed");
        assert_eq!(
            revert_changes(dir, &c2).expect("revert"),
            vec![(
                "notes/a.md".into(),
                Some(b"zero\n\none\ntwo\nthree\n".to_vec())
            )]
        );
        // rename
        let content = std::fs::read(dir.join("notes/a.md")).expect("read");
        atomic_write(dir, "notes/b.md", &content).expect("w");
        remove(dir, "notes/a.md").expect("rm");
        let c4 = commit_paths(
            dir,
            &["notes/a.md".into(), "notes/b.md".into()],
            "user: move notes/a.md -> notes/b.md",
            t(400),
        )
        .expect("commit")
        .expect("changed");
        let h = history(dir, "notes/b.md").expect("history");
        let got: Vec<(String, String, Change)> = h
            .iter()
            .map(|r| (r.commit.id.clone(), r.path.clone(), r.change))
            .collect();
        assert_eq!(
            got,
            vec![
                (c4.clone(), "notes/b.md".into(), Change::Renamed),
                (c3.clone(), "notes/a.md".into(), Change::Modified),
                (c2.clone(), "notes/a.md".into(), Change::Modified),
                (c1.clone(), "notes/a.md".into(), Change::Added),
            ]
        );
        assert_eq!(
            blob_at(dir, &c1, "notes/a.md").expect("blob"),
            Some(b"one\ntwo\nthree\n".to_vec())
        );
        assert_eq!(dirty_paths(dir).expect("status"), Vec::<String>::new());
        assert_eq!(log(dir).expect("log").len(), 4);
        assert_eq!(
            find_commit(dir, &c1).expect("find").map(|c| c.message),
            Some("user: create notes/a.md".into())
        );
        assert_eq!(find_commit(dir, "zz").expect("find"), None);
        assert!(matches!(
            revert_changes(dir, &c1),
            Err(VaultError::NotFound)
        ));
    }

    #[test]
    fn dirty_paths_ignore_temp_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        init(dir).expect("init");
        std::fs::create_dir_all(dir.join("notes")).expect("mkdir");
        std::fs::write(dir.join("notes/.strata-tmp-1-1"), "x").expect("w");
        std::fs::write(dir.join("notes/new.md"), "x").expect("w");
        assert_eq!(
            dirty_paths(dir).expect("status"),
            vec!["notes/new.md".to_owned()]
        );
        let id = commit_all(dir, "system: recovered changes", t(5))
            .expect("commit")
            .expect("changed");
        assert_eq!(
            changed_paths(dir, &id).expect("paths"),
            vec!["notes/new.md"]
        );
        assert_eq!(dirty_paths(dir).expect("status"), Vec::<String>::new());
    }
}
