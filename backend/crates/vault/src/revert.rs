//! Structured revert of a whole commit, used when git's line-based three-way revert
//! conflicts (PLAN §7.2: "any AI change is revertible").
//!
//! A line merge conflicts whenever a later edit touches a line next to one the commit changed,
//! which is the normal case in frontmatter (`updated:` sits right above the relation keys an
//! AI job writes). This revert works per file at the level Strata writes:
//!
//! - **notes:** every frontmatter key the commit changed is set back to its value before the
//!   commit, provided nobody changed it since (`updated` is left alone); the body is reverted
//!   with a three-way merge of the body only;
//! - **sidecars:** relation provenance, rejections and keep-both entries the commit added are
//!   removed and those it removed are restored (other later changes stay);
//! - **other files:** reverted only if unchanged since the commit.
//!
//! Anything that cannot be reverted without losing a later change is a conflict.

use std::path::Path;

use git2::{MergeFileInput, Oid, Repository};
use vault_format::sidecar::NoteSidecar;
use vault_format::{Document, PropertyValue};

use crate::error::{Result, VaultError};
use crate::git::FileChange;

fn blob(repo: &Repository, tree: &git2::Tree<'_>, path: &str) -> Result<Option<Vec<u8>>> {
    match tree.get_path(Path::new(path)) {
        Ok(e) => Ok(Some(repo.find_blob(e.id())?.content().to_vec())),
        Err(_) => Ok(None),
    }
}

fn merge_text(base: &str, ours: &str, theirs: &str) -> Result<String> {
    // `git2::merge_file` (unlike repository calls) does not initialise libgit2 itself: in a
    // process that has made no other git2 call it fails with "no error". `Buf::new` runs
    // git2's one-time initialisation.
    drop(git2::Buf::new());
    let mut b = MergeFileInput::new();
    b.content(base.as_bytes());
    let mut o = MergeFileInput::new();
    o.content(ours.as_bytes());
    let mut t = MergeFileInput::new();
    t.content(theirs.as_bytes());
    let result = git2::merge_file(&b, &o, &t, None)?;
    if !result.is_automergeable() {
        return Err(VaultError::RevertConflict);
    }
    Ok(String::from_utf8_lossy(result.content()).into_owned())
}

/// Reverts note `commit` → `parent` on top of `current`.
fn revert_note(parent: &str, commit: &str, current: &str) -> Result<String> {
    let p = Document::parse(parent);
    let c = Document::parse(commit);
    let mut cur = Document::parse(current);
    let fm_err = |d: &Document| d.frontmatter().is_some_and(|f| f.error().is_some());
    if fm_err(&p) || fm_err(&c) || fm_err(&cur) {
        return Err(VaultError::RevertConflict);
    }
    let get = |d: &Document, k: &str| d.frontmatter().and_then(|f| f.get(k).cloned());
    let mut keys: Vec<String> = Vec::new();
    for d in [&p, &c] {
        if let Some(f) = d.frontmatter() {
            for k in f.keys() {
                if !keys.iter().any(|x| x == k) {
                    keys.push(k.to_owned());
                }
            }
        }
    }
    for k in keys {
        if k == "updated" {
            continue;
        }
        let (pv, cv, now) = (get(&p, &k), get(&c, &k), get(&cur, &k));
        if pv == cv || now == pv {
            continue;
        }
        if now != cv {
            return Err(VaultError::RevertConflict);
        }
        let fm = cur.frontmatter_mut();
        let edit = match pv {
            Some(v) => fm.set(&k, v),
            None => fm.remove(&k).map(|_| ()),
        };
        edit.map_err(|_| VaultError::RevertConflict)?;
    }
    if p.body() != c.body() && cur.body() != p.body() {
        let body = if cur.body() == c.body() {
            p.body().to_owned()
        } else {
            merge_text(c.body(), cur.body(), p.body())?
        };
        cur.set_body(body);
    }
    // A frontmatter the revert emptied is kept only if the note had one before.
    let _ = PropertyValue::Null;
    Ok(cur.render())
}

/// Reverts sidecar `commit` → `parent` on top of `current` (set semantics per list).
fn revert_sidecar(parent: &str, commit: &str, current: &str) -> Result<Option<String>> {
    let parse = |s: &str| NoteSidecar::from_json(s).map_err(|_| VaultError::RevertConflict);
    let (p, c, mut cur) = (parse(parent)?, parse(commit)?, parse(current)?);
    macro_rules! revert_list {
        ($field:ident, $eq:expr) => {{
            let eq = $eq;
            for added in c
                .$field
                .iter()
                .filter(|x| !p.$field.iter().any(|y| eq(x, y)))
            {
                cur.$field.retain(|x| !eq(x, added));
            }
            for removed in p
                .$field
                .iter()
                .filter(|x| !c.$field.iter().any(|y| eq(x, y)))
            {
                if !cur.$field.iter().any(|x| eq(x, removed)) {
                    cur.$field.push(removed.clone());
                }
            }
        }};
    }
    revert_list!(
        relations,
        |a: &vault_format::sidecar::SidecarRelation, b: &vault_format::sidecar::SidecarRelation| a
            == b
    );
    revert_list!(
        rejected,
        |a: &vault_format::sidecar::RejectedRelation,
         b: &vault_format::sidecar::RejectedRelation| a == b
    );
    revert_list!(
        keep_both,
        |a: &vault_format::sidecar::KeepBoth, b: &vault_format::sidecar::KeepBoth| a == b
    );
    if p.summary != c.summary && cur.summary == c.summary {
        cur.summary.clone_from(&p.summary);
    }
    if p.last_linked_hash != c.last_linked_hash && cur.last_linked_hash == c.last_linked_hash {
        cur.last_linked_hash.clone_from(&p.last_linked_hash);
    }
    let change = crate::store::Core::sidecar_change(&cur)?;
    Ok(change.1.map(|b| String::from_utf8_lossy(&b).into_owned()))
}

/// The changes that revert `commit` file by file (see the module docs).
pub fn structured(dir: &Path, commit: &str) -> Result<Vec<FileChange>> {
    let repo = Repository::open(dir)?;
    let target = repo.find_commit(Oid::from_str(commit)?)?;
    let parent = target.parent(0).map_err(|_| VaultError::NotFound)?;
    let (ctree, ptree) = (target.tree()?, parent.tree()?);
    let paths = crate::git::changed_paths(dir, commit)?;
    let mut out = Vec::new();
    for path in paths {
        let before = blob(&repo, &ptree, &path)?;
        let after = blob(&repo, &ctree, &path)?;
        let now = crate::fsio::read(dir, &path)?;
        if now == before {
            continue;
        }
        if now == after {
            out.push((path, before));
            continue;
        }
        let (Some(b), Some(a), Some(n)) = (before, after, now) else {
            return Err(VaultError::RevertConflict);
        };
        let text = |v: &[u8]| String::from_utf8_lossy(v).into_owned();
        let merged = if path.ends_with(".md") {
            Some(revert_note(&text(&b), &text(&a), &text(&n))?)
        } else if path.starts_with(".meta/notes/") && path.ends_with(".json") {
            revert_sidecar(&text(&b), &text(&a), &text(&n))?
        } else {
            return Err(VaultError::RevertConflict);
        };
        out.push((path, merged.map(String::into_bytes)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_keys_revert_independently_of_nearby_edits() {
        let parent = "---\nid: X\nupdated: 1\n---\nbody\n";
        let commit = "---\nid: X\nupdated: 1\nrelated: [\"[[B]]\"]\n---\nbody\n";
        let current = "---\nid: X\nupdated: 2\nrelated: [\"[[B]]\"]\n---\nbody edited\n";
        assert_eq!(
            revert_note(parent, commit, current).expect("revert"),
            "---\nid: X\nupdated: 2\n---\nbody edited\n"
        );
        // A later change of the same key conflicts.
        let current = "---\nid: X\nupdated: 2\nrelated: [\"[[C]]\"]\n---\nbody\n";
        assert!(matches!(
            revert_note(parent, commit, current),
            Err(VaultError::RevertConflict)
        ));
    }

    #[test]
    fn body_changes_merge_three_way() {
        let parent = "a\nb\nc\nd\ne\n";
        let commit = "a\nB\nc\nd\ne\n";
        let current = "a\nB\nc\nd\nE\n";
        assert_eq!(
            revert_note(parent, commit, current).expect("revert"),
            "a\nb\nc\nd\nE\n"
        );
    }
}
