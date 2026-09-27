//! Filesystem primitives: atomic writes (temp file → fsync → rename → fsync directory),
//! removals, content hashes and vault scans. All functions are blocking; the actor runs them
//! on the blocking pool.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::paths::{GIT_DIR, TEMP_PREFIX};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `sha256:<hex>` of `bytes`: a note's version (PLAN §7.2).
pub fn version_of(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Creates `dir` and its parents with mode 0700.
pub fn create_dir_private(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    // Directories can be opened read-only and fsynced on Unix; elsewhere this is a no-op.
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
    Ok(())
}

/// The absolute path of vault-relative `rel` under `root`. `rel` must already be validated
/// (no `..`, not absolute); this only joins `/`-separated segments.
pub fn resolve(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for seg in rel.split('/') {
        p.push(seg);
    }
    p
}

/// Writes `bytes` to `root/rel` atomically: a temporary file in the same directory is
/// written and fsynced, renamed over the target, then the directory is fsynced. A crash at
/// any point leaves either the old file or the new one, plus at most a temporary file whose
/// name starts with [`TEMP_PREFIX`] (a hidden name: never a note, removed by reconciliation).
pub fn atomic_write(root: &Path, rel: &str, bytes: &[u8]) -> io::Result<()> {
    let target = resolve(root, rel);
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("vault path has no parent"))?
        .to_path_buf();
    create_dir_private(&dir)?;
    let tmp = dir.join(format!(
        "{TEMP_PREFIX}{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &target)?;
        sync_dir(&dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Removes `root/rel` (missing is fine) and fsyncs its directory.
pub fn remove(root: &Path, rel: &str) -> io::Result<()> {
    let target = resolve(root, rel);
    match fs::remove_file(&target) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    }
    if let Some(dir) = target.parent() {
        sync_dir(dir)?;
    }
    Ok(())
}

/// Reads `root/rel`; `None` if it does not exist or is not a regular file.
pub fn read(root: &Path, rel: &str) -> io::Result<Option<Vec<u8>>> {
    let path = resolve(root, rel);
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() => Ok(Some(fs::read(&path)?)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Every regular file under `root` as sorted `/`-separated relative paths, excluding `.git/`
/// at the root and temporary files. Symlinks and special files are skipped. Paths that are
/// not UTF-8 are skipped.
pub fn scan(root: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    walk(root, root, "", &mut |rel, kind| {
        if kind == Kind::File {
            out.push(rel.to_owned());
        }
    })?;
    out.sort();
    Ok(out)
}

/// Every directory under `root` (relative, sorted), excluding hidden ones.
pub fn scan_dirs(root: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    walk(root, root, "", &mut |rel, kind| {
        if kind == Kind::Dir && crate::paths::is_content(rel) {
            out.push(rel.to_owned());
        }
    })?;
    out.sort();
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Dir,
}

fn walk(root: &Path, dir: &Path, prefix: &str, f: &mut dyn FnMut(&str, Kind)) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let kind = entry.file_type()?;
        let rel = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        if kind.is_dir() {
            if prefix.is_empty() && name == GIT_DIR {
                continue;
            }
            f(&rel, Kind::Dir);
            walk(root, &entry.path(), &rel, f)?;
        } else if kind.is_file() && !name.starts_with(TEMP_PREFIX) {
            f(&rel, Kind::File);
        }
    }
    let _ = root;
    Ok(())
}

/// Removes leftover temporary files (from a crash between write and rename). Returns their
/// relative paths, sorted.
pub fn remove_temp_files(root: &Path) -> io::Result<Vec<String>> {
    fn go(dir: &Path, prefix: &str, out: &mut Vec<String>) -> io::Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let kind = entry.file_type()?;
            let rel = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            if kind.is_dir() {
                if !(prefix.is_empty() && name == GIT_DIR) {
                    go(&entry.path(), &rel, out)?;
                }
            } else if name.starts_with(TEMP_PREFIX) {
                fs::remove_file(entry.path())?;
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    go(root, "", &mut out)?;
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        atomic_write(tmp.path(), "notes/a.md", b"one").expect("write");
        atomic_write(tmp.path(), "notes/a.md", b"two").expect("write");
        assert_eq!(
            read(tmp.path(), "notes/a.md").expect("read"),
            Some(b"two".to_vec())
        );
        assert_eq!(
            scan(tmp.path()).expect("scan"),
            vec!["notes/a.md".to_owned()]
        );
        assert_eq!(
            remove_temp_files(tmp.path()).expect("clean"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn temp_files_are_invisible_and_cleaned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("notes")).expect("mkdir");
        fs::write(
            tmp.path().join("notes/.strata-tmp-9-9"),
            "---\nid: x\n---\n",
        )
        .expect("w");
        fs::create_dir_all(tmp.path().join(".git")).expect("mkdir");
        fs::write(tmp.path().join(".git/HEAD"), "x").expect("w");
        assert_eq!(scan(tmp.path()).expect("scan"), Vec::<String>::new());
        assert_eq!(
            remove_temp_files(tmp.path()).expect("clean"),
            vec!["notes/.strata-tmp-9-9".to_owned()]
        );
        assert!(!tmp.path().join("notes/.strata-tmp-9-9").exists());
    }

    #[test]
    fn versions_are_sha256() {
        assert_eq!(
            version_of(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
