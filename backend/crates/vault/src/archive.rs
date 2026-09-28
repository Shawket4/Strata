//! Vault provisioning, export and import (PLAN §6.1, §6.10).
//!
//! - **Provision:** `<data_root>/users/<id>/vault` (mode 0700), a git repository with the §6.1
//!   skeleton folders (each kept by a `.gitkeep`) and one `system: initialize vault` commit.
//! - **Export:** a zip of the vault including `.meta/` and `.trash/`, excluding `.git/`, plus a
//!   minimal `.obsidian/app.json` setting the attachment folder. Entries are sorted, stored with
//!   a fixed timestamp and fixed permissions, so the same vault always exports to the same bytes.
//! - **Import:** a zip of an Obsidian vault. The whole archive is rejected if any entry is a
//!   symlink, has an absolute path, a `..` segment, a backslash or a drive prefix, or if it is
//!   too large (entries, one entry, or in total — checked while decompressing, so a zip bomb is
//!   stopped). `.git/`, `.obsidian/` and other hidden entries (except `.meta/` and `.trash/`)
//!   and entries with Obsidian-unsafe names are skipped and reported. Notes without an ID (or
//!   whose ID is taken) get one; unknown properties and every other byte are preserved. The
//!   import is one `user: import` commit, so reverting that commit undoes it.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;

use strata_common::{NoteId, UserId};
use strata_index::UserScope;
use vault_format::Document;
use vault_format::sidecar::NoteSidecar;
use zip::write::SimpleFileOptions;

use crate::error::{Result, VaultError};
use crate::model::ImportReport;
use crate::paths::{self, SKELETON};
use crate::store::{Author, Core, ImportLimits, VaultService, blocking, sidecar_id};
use crate::{fsio, git, prepare};

/// `.obsidian/app.json` added to every export.
pub const OBSIDIAN_APP_JSON: &str = "{\n  \"attachmentFolderPath\": \"attachments\"\n}\n";

fn zip_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644)
}

/// Writes the export zip of `dir` (blocking).
pub fn export_zip(dir: &Path) -> Result<Vec<u8>> {
    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for f in fsio::scan(dir)? {
        if f.starts_with(".obsidian/") {
            continue;
        }
        if let Some(bytes) = fsio::read(dir, &f)? {
            entries.insert(f, bytes);
        }
    }
    entries.insert(
        ".obsidian/app.json".to_owned(),
        OBSIDIAN_APP_JSON.as_bytes().to_vec(),
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(name, zip_options())
            .map_err(|e| VaultError::Internal(format!("zip: {e}")))?;
        zip.write_all(&bytes)?;
    }
    Ok(zip
        .finish()
        .map_err(|e| VaultError::Internal(format!("zip: {e}")))?
        .into_inner())
}

/// Why an entry path is unsafe (the whole archive is rejected).
fn unsafe_reason(name: &str) -> Option<&'static str> {
    if name.starts_with('/') || name.starts_with('\\') {
        return Some("an entry has an absolute path");
    }
    if name.contains('\\') {
        return Some("an entry path contains a backslash");
    }
    if name.len() >= 2 && name.as_bytes()[1] == b':' {
        return Some("an entry path has a drive prefix");
    }
    if name.split('/').any(|s| s == "..") {
        return Some("an entry path contains ..");
    }
    if name.contains('\0') {
        return Some("an entry path contains a NUL byte");
    }
    None
}

/// An archive entry accepted for import.
struct Entry {
    path: String,
    bytes: Vec<u8>,
}

/// Reads and checks the archive (blocking).
fn read_archive(bytes: &[u8], limits: ImportLimits) -> Result<(Vec<Entry>, Vec<String>)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| VaultError::InvalidArchive("the body is not a zip archive".into()))?;
    if archive.len() > limits.max_entries {
        return Err(VaultError::ArchiveTooLarge(
            format!("the archive has more than {} entries", limits.max_entries).into(),
        ));
    }
    let mut entries = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|_| VaultError::InvalidArchive("an entry cannot be read".into()))?;
        let name = file.name().to_owned();
        if let Some(reason) = unsafe_reason(&name) {
            return Err(VaultError::InvalidArchive(reason.into()));
        }
        if file.is_symlink() {
            return Err(VaultError::InvalidArchive(
                "the archive contains a symlink".into(),
            ));
        }
        if file.enclosed_name().is_none() {
            return Err(VaultError::InvalidArchive(
                "an entry path escapes the vault".into(),
            ));
        }
        if file.is_dir() {
            continue;
        }
        if file.size() > limits.max_entry_bytes {
            return Err(VaultError::ArchiveTooLarge(
                format!("an entry is larger than {} bytes", limits.max_entry_bytes).into(),
            ));
        }
        let path = name.trim_start_matches("./").to_owned();
        let first = path.split('/').next().unwrap_or_default();
        let hidden_ok = first == paths::META_DIR || first == paths::TRASH_DIR;
        let hidden = path.split('/').any(|s| s.starts_with('.'));
        if paths::file_name(&path) == ".gitkeep" {
            // Skeleton placeholders (every export has them): not content, not reported.
            continue;
        }
        if (hidden && !hidden_ok) || path.is_empty() {
            skipped.push(path);
            continue;
        }
        let checked = if hidden_ok {
            path.split('/').skip(1).collect::<Vec<_>>().join("/")
        } else {
            path.clone()
        };
        if paths::validate_path(&checked).is_err() || !seen.insert(path.clone()) {
            skipped.push(path);
            continue;
        }
        let mut buf = Vec::new();
        let read = (&mut file)
            .take(limits.max_entry_bytes + 1)
            .read_to_end(&mut buf)
            .map_err(|_| VaultError::InvalidArchive("an entry cannot be decompressed".into()))?;
        let read = u64::try_from(read).unwrap_or(u64::MAX);
        if read > limits.max_entry_bytes {
            return Err(VaultError::ArchiveTooLarge(
                format!("an entry is larger than {} bytes", limits.max_entry_bytes).into(),
            ));
        }
        total = total.saturating_add(read);
        if total > limits.max_total_bytes {
            return Err(VaultError::ArchiveTooLarge(
                format!(
                    "the archive is larger than {} bytes uncompressed",
                    limits.max_total_bytes
                )
                .into(),
            ));
        }
        entries.push(Entry { path, bytes: buf });
    }
    skipped.sort();
    skipped.dedup();
    Ok((entries, skipped))
}

/// Archive entries split into notes (live or trashed `.md` notes) and everything else.
type Files = Vec<(String, Vec<u8>)>;

fn split_notes(entries: Vec<Entry>) -> (Files, Files) {
    entries
        .into_iter()
        .fold((Vec::new(), Vec::new()), |(mut notes, mut others), e| {
            let is_note = e.path.ends_with(".md")
                && (paths::is_note(&e.path)
                    || paths::untrash_path(&e.path).is_some_and(paths::is_note));
            if is_note {
                notes.push((e.path, e.bytes));
            } else {
                others.push((e.path, e.bytes));
            }
            (notes, others)
        })
}

impl Core {
    /// Imports a zip archive (see the module docs).
    pub async fn import(&mut self, scope: UserScope, bytes: Vec<u8>) -> Result<ImportReport> {
        let limits = self.inner.config.import;
        let (entries, skipped) = blocking(move || read_archive(&bytes, limits)).await?;
        let mut tx = self.begin(&scope).await?;
        let state = self.state()?;
        let mut used: BTreeSet<NoteId> = BTreeSet::new();
        let mut kept_ids: BTreeSet<NoteId> = BTreeSet::new();
        let mut assigned = Vec::new();
        let (notes, others) = split_notes(entries);
        let mut changes: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        let mut seen_tasks: BTreeSet<String> = BTreeSet::new();
        for (path, bytes) in notes {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let mut doc = Document::parse(&text);
            let id = doc
                .frontmatter()
                .and_then(|f| f.id().ok().flatten())
                .map(NoteId::from_ulid);
            let existing_here = |i: NoteId| {
                state.note(i).is_some_and(|(p, _)| p == path)
                    || state.trash_by_id.get(&i).is_some_and(|p| *p == path)
            };
            let free =
                id.filter(|i| !used.contains(i) && (!state.contains_id(*i) || existing_here(*i)));
            let editable = doc.frontmatter().is_none_or(|f| f.error().is_none());
            // The note already at this path keeps its ID (§6.4: an ID never changes), whatever
            // ID the entry carries; the entry replaces its content.
            let occupant = state
                .notes
                .get(&path)
                .or_else(|| state.trash.get(&path))
                .map(|m| m.id)
                .filter(|o| !used.contains(o));
            let final_id = match (occupant, free) {
                (Some(o), f) if f != Some(o) && editable => {
                    prepare::stamp(&mut doc, o, None, None)?;
                    assigned.push(path.clone());
                    kept_ids.insert(o);
                    o
                }
                (_, Some(i)) => {
                    kept_ids.insert(i);
                    i
                }
                (_, None) if editable => {
                    let i = NoteId::generate(self.ids());
                    prepare::stamp(&mut doc, i, None, None)?;
                    assigned.push(path.clone());
                    i
                }
                (_, None) => {
                    // Unreadable frontmatter: imported verbatim, indexed after an ID is given
                    // by reconciliation.
                    changes.push((path, Some(bytes)));
                    continue;
                }
            };
            used.insert(final_id);
            if editable {
                let body = prepare::assign_task_ids(doc.body(), self.ids(), &seen_tasks);
                if body != doc.body() {
                    doc.set_body(body);
                    if !assigned.contains(&path) {
                        assigned.push(path.clone());
                    }
                }
                for t in vault_format::tasks::extract_tasks(doc.body()) {
                    if let Some(b) = t.task.block_id() {
                        seen_tasks.insert(b.to_owned());
                    }
                }
            }
            let out = doc.render();
            changes.push((path, Some(out.into_bytes())));
        }
        for (path, bytes) in others {
            if let Some(id) = sidecar_id(&path) {
                // Keep a sidecar only with the note it describes.
                if !kept_ids.contains(&id)
                    || NoteSidecar::from_json(&String::from_utf8_lossy(&bytes)).is_err()
                {
                    continue;
                }
            }
            changes.push((path, Some(bytes)));
        }
        changes.sort_by(|a, b| a.0.cmp(&b.0));
        let imported: Vec<String> = changes.iter().map(|(p, _)| p.clone()).collect();
        assigned.sort();
        let _ = &mut tx;
        let message = Author::User.message("import", &format!("{} files", imported.len()));
        let commit = self.finish(tx, changes, message).await?;
        Ok(ImportReport {
            commit,
            imported,
            ids_assigned: assigned,
            skipped,
        })
    }
}

/// Finishes a vault whose repository has no commit yet (one created by this store, or by
/// an older provisioner that only ran `git init`): repository options, the §6.1 skeleton
/// and the `system: initialize vault` commit. Does nothing once a commit exists.
pub fn complete_init(dir: &Path, at: chrono::DateTime<chrono::Utc>) -> Result<bool> {
    if git::head(dir)?.is_some() {
        return Ok(false);
    }
    git::configure(dir)?;
    for folder in SKELETON {
        let keep = format!("{folder}/.gitkeep");
        if fsio::read(dir, &keep)?.is_none() {
            fsio::atomic_write(dir, &keep, b"")?;
        }
    }
    git::commit_all(dir, "system: initialize vault", at)?;
    Ok(true)
}

impl VaultService {
    /// Creates the user's vault if missing (§6.1 skeleton, one initial commit). Returns
    /// whether it was created now.
    pub async fn provision(&self, user: UserId) -> io::Result<bool> {
        let dir = self.vault_dir(user);
        let user_dir = self.user_dir(user);
        let at = self.inner.clock.now();
        blocking(move || {
            if dir.join(".git").is_dir() {
                return Ok(false);
            }
            fsio::create_dir_private(&user_dir)?;
            fsio::create_dir_private(&dir)?;
            git::init(&dir)?;
            complete_init(&dir, at)?;
            Ok(true)
        })
        .await
        .map_err(|e| match e {
            VaultError::Io(e) => e,
            other => io::Error::other(other.to_string()),
        })
    }

    /// Removes the user's whole directory (vault and history) and stops their writer.
    pub async fn deprovision(&self, user: UserId) -> io::Result<()> {
        self.evict(user);
        let dir = self.user_dir(user);
        let doomed = self
            .inner
            .config
            .data_root
            .join("users")
            .join(format!(".purging-{user}"));
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
    }

    /// `GET /export`: the vault as a zip (taken on the writer, so it is a consistent
    /// snapshot).
    pub async fn export(&self, scope: &UserScope) -> Result<Vec<u8>> {
        self.exec(scope, |core, _| {
            Box::pin(async move {
                let dir = core.dir().to_path_buf();
                blocking(move || export_zip(&dir)).await
            })
        })
        .await
    }

    /// `POST /import`.
    pub async fn import(&self, scope: &UserScope, bytes: Vec<u8>) -> Result<ImportReport> {
        self.exec(scope, move |core, s| {
            Box::pin(async move { core.import(s, bytes).await })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip_options()).expect("start");
            zip.write_all(bytes).expect("write");
        }
        zip.finish().expect("finish").into_inner()
    }

    #[test]
    fn unsafe_paths_reject_the_archive() {
        for bad in [
            "../evil.md",
            "notes/../../x.md",
            "/etc/passwd",
            "C:/x.md",
            "a\\b.md",
        ] {
            let z = zip_of(&[("notes/ok.md", b"x"), (bad, b"x")]);
            assert!(
                matches!(
                    read_archive(&z, ImportLimits::default()),
                    Err(VaultError::InvalidArchive(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn hidden_and_invalid_names_are_skipped() {
        let z = zip_of(&[
            ("notes/ok.md", b"x"),
            (".obsidian/app.json", b"{}"),
            (".git/HEAD", b"ref"),
            ("notes/.DS_Store", b"x"),
            ("notes/a?b.md", b"x"),
            (".meta/notes/x.json", b"{}"),
        ]);
        let (entries, skipped) = read_archive(&z, ImportLimits::default()).expect("ok");
        let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, vec!["notes/ok.md", ".meta/notes/x.json"]);
        assert_eq!(
            skipped,
            vec![
                ".git/HEAD",
                ".obsidian/app.json",
                "notes/.DS_Store",
                "notes/a?b.md"
            ]
        );
    }

    #[test]
    fn size_limits_apply_while_decompressing() {
        let big = vec![b'a'; 4096];
        let z = zip_of(&[("notes/big.md", &big)]);
        let limits = ImportLimits {
            max_entries: 10,
            max_entry_bytes: 1000,
            max_total_bytes: 10_000,
        };
        assert!(matches!(
            read_archive(&z, limits),
            Err(VaultError::ArchiveTooLarge(_))
        ));
        let z = zip_of(&[("a.md", &big[..900]), ("b.md", &big[..900])]);
        let limits = ImportLimits {
            max_entries: 10,
            max_entry_bytes: 1000,
            max_total_bytes: 1000,
        };
        assert!(matches!(
            read_archive(&z, limits),
            Err(VaultError::ArchiveTooLarge(_))
        ));
        let limits = ImportLimits {
            max_entries: 1,
            ..ImportLimits::default()
        };
        assert!(matches!(
            read_archive(&z, limits),
            Err(VaultError::ArchiveTooLarge(_))
        ));
        assert!(matches!(
            read_archive(b"not a zip", ImportLimits::default()),
            Err(VaultError::InvalidArchive(_))
        ));
    }

    #[test]
    fn exports_are_deterministic_and_skip_git() {
        let tmp = tempfile::tempdir().expect("tmp");
        fsio::atomic_write(tmp.path(), "notes/a.md", b"A").expect("w");
        fsio::atomic_write(tmp.path(), ".meta/notes/x.json", b"{}").expect("w");
        git::init(tmp.path()).expect("git");
        let one = export_zip(tmp.path()).expect("zip");
        let two = export_zip(tmp.path()).expect("zip");
        assert_eq!(one, two);
        let archive = zip::ZipArchive::new(Cursor::new(one)).expect("zip");
        let names: Vec<&str> = archive.file_names().collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![".meta/notes/x.json", ".obsidian/app.json", "notes/a.md"]
        );
    }
}
