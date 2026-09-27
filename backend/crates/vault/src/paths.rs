//! Vault-relative paths: validation of client-supplied paths (PLAN §6.2, §15 path traversal)
//! and the reserved locations inside a vault.

use vault_format::filename::{FileNameError, PathError, validate_vault_path};

use crate::error::{Result, VaultError};

/// Sidecar folder (hidden from Obsidian and the tree).
pub const META_DIR: &str = ".meta";
/// Soft-deleted notes.
pub const TRASH_DIR: &str = ".trash";
/// Git repository.
pub const GIT_DIR: &str = ".git";
/// Prefix of in-flight temporary files (never notes; removed by reconciliation).
pub const TEMP_PREFIX: &str = ".strata-tmp-";
/// Inbox folder for captures (§6.9).
pub const INBOX_DIR: &str = "inbox";
/// Default home of tasks without a note (§6.11).
pub const TASKS_NOTE: &str = "tasks/Tasks.md";

/// Folders created in every new vault (§6.1). `attachments/` gets its dated subfolders on
/// demand.
pub const SKELETON: &[&str] = &[
    "inbox",
    "notes",
    "concepts",
    "people",
    "companies",
    "documents",
    "places",
    "attachments",
    "tasks",
    "maps",
    "_ai/digests",
];

fn name_error(e: &FileNameError) -> &'static str {
    match e {
        FileNameError::Empty => "a path segment is empty",
        FileNameError::ForbiddenChar(_) => "the name contains a character Obsidian forbids",
        FileNameError::ControlChar(_) => "the name contains a control character",
        FileNameError::LeadingDot => "a path segment starts with a dot",
        FileNameError::EdgeWhitespaceOrDot => {
            "a path segment starts or ends with whitespace or ends with a dot"
        }
        FileNameError::Reserved => "a path segment is a reserved device name",
        FileNameError::TooLong => "a path segment is longer than 255 bytes",
    }
}

fn path_error(e: &PathError) -> &'static str {
    match e {
        PathError::Absolute => "the path must be relative to the vault",
        PathError::Segment { error, .. } => name_error(error),
    }
}

/// Validates a vault-relative folder or file path written by a client: relative, `/`
/// separators, every segment Obsidian-safe (no `.`/`..`, no hidden segments, no forbidden
/// characters). Hidden segments rule out `.meta`, `.trash` and `.git`.
pub fn validate_path(path: &str) -> Result<()> {
    validate_vault_path(path).map_err(|e| VaultError::InvalidName(path_error(&e)))
}

/// Validates a note path: [`validate_path`] and a `.md` extension.
pub fn validate_note_path(path: &str) -> Result<()> {
    validate_path(path)?;
    if !path.ends_with(".md") || path.len() <= 3 || path.ends_with("/.md") {
        return Err(VaultError::InvalidName("a note path must end in .md"));
    }
    Ok(())
}

/// Validates a single file name (no `/`) as a note title.
pub fn validate_title(title: &str) -> Result<()> {
    vault_format::filename::validate_file_name(title)
        .map_err(|e| VaultError::InvalidName(name_error(&e)))
}

/// Whether a scanned file path is user content (a note or attachment), i.e. not inside a
/// hidden folder and not hidden itself.
pub fn is_content(path: &str) -> bool {
    !path.split('/').any(|seg| seg.starts_with('.'))
}

/// Whether a scanned path is a markdown note.
pub fn is_note(path: &str) -> bool {
    is_content(path) && path.ends_with(".md")
}

/// `.trash/<path>`.
pub fn trash_path(path: &str) -> String {
    format!("{TRASH_DIR}/{path}")
}

/// The original path of a trashed file.
pub fn untrash_path(trash: &str) -> Option<&str> {
    trash.strip_prefix(TRASH_DIR)?.strip_prefix('/')
}

/// The folder of a path (`""` at the root).
pub fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// The file name of a path.
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Joins a folder and a name.
pub fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_and_hidden_paths_are_rejected() {
        for bad in [
            "../x.md",
            "notes/../../etc/passwd.md",
            "/etc/passwd.md",
            ".meta/notes/x.md",
            ".trash/x.md",
            ".git/config.md",
            "notes/./x.md",
            "notes//x.md",
            "notes\\x.md",
            "",
            "notes/x",
            "notes/.md",
            "notes/a:b.md",
        ] {
            assert!(validate_note_path(bad).is_err(), "{bad:?}");
        }
        for good in ["notes/x.md", "people/أحمد سمير.md", "a.md", "inbox/2026-09-27-120000.md"] {
            assert_eq!(validate_note_path(good).ok(), Some(()), "{good:?}");
        }
    }

    #[test]
    fn content_classification() {
        assert!(is_note("notes/a.md"));
        assert!(!is_note(".meta/notes/x.json"));
        assert!(!is_note("notes/.strata-tmp-1"));
        assert!(!is_note(".trash/notes/a.md"));
        assert!(is_content("attachments/2026/09/x.png"));
        assert_eq!(untrash_path(".trash/notes/a.md"), Some("notes/a.md"));
        assert_eq!(parent("notes/a/b.md"), "notes/a");
        assert_eq!(parent("b.md"), "");
        assert_eq!(join("", "b.md"), "b.md");
    }
}
