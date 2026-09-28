//! Paths of new items (PLAN §6.1, §6.2, §6.9) and conflict copies (D19).
//!
//! A new item's file name is free when no `.md` note directly in the same folder has the
//! same stem, compared case-insensitively (`vault_format::filename::unique_name`): the first
//! free one of `stem`, `stem 2`, `stem 3`, …

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use domain::NoteKind;
use vault_format::filename::{sanitize_file_name, unique_name};

/// The inbox folder of captures (§6.9).
pub const INBOX_DIR: &str = "inbox";

/// The note tasks without a home note go to (§6.11).
pub use vault_format::tasks::DEFAULT_TASK_NOTE;

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

/// Whether the note at `path` is an inbox item: a file directly in `inbox/` (notes in
/// subfolders of the inbox are ordinary notes).
pub fn is_inbox_path(path: &str) -> bool {
    parent(path) == INBOX_DIR
}

/// The first free note path `folder/<stem>[ N].md` given the vault paths already `taken`
/// (only `.md` files directly in `folder` count).
pub fn free_note_path<'a>(
    folder: &str,
    stem: &str,
    taken: impl IntoIterator<Item = &'a str>,
) -> String {
    let stems: Vec<&str> = taken
        .into_iter()
        .filter(|p| parent(p) == folder)
        .filter_map(|p| file_name(p).strip_suffix(".md"))
        .collect();
    join(folder, &format!("{}.md", unique_name(stem, stems)))
}

/// The file stem of a capture made at `created` (`YYYY-MM-DD-HHmmss`, in the offset the
/// time carries — the user's time zone).
pub fn capture_stem(created: &DateTime<FixedOffset>) -> String {
    created.format("%Y-%m-%d-%H%M%S").to_string()
}

/// The inbox path of a capture made at `created`: `inbox/YYYY-MM-DD-HHmmss.md`, or
/// `… 2.md`, `… 3.md` when taken (§6.9).
pub fn capture_path<'a>(
    created: &DateTime<FixedOffset>,
    taken: impl IntoIterator<Item = &'a str>,
) -> String {
    free_note_path(INBOX_DIR, &capture_stem(created), taken)
}

/// The file stem of an entity named `name`: the trimmed name, sanitised (§6.2).
pub fn entity_stem(name: &str) -> String {
    sanitize_file_name(name.trim())
}

/// The path of a new note of `kind` named `name`: `<kind folder>/<sanitised name>[ N].md`
/// (`people/`, `companies/`, `documents/`, `places/`, `concepts/`).
pub fn entity_path<'a>(
    kind: NoteKind,
    name: &str,
    taken: impl IntoIterator<Item = &'a str>,
) -> String {
    free_note_path(kind.default_folder(), &entity_stem(name), taken)
}

/// The path of the `n`-th conflict copy of the note at `path` made at `at` (D19):
/// `<stem> (conflict YYYY-MM-DD HHmmss).md`, and `<stem> (conflict YYYY-MM-DD HHmmss N).md`
/// for `n >= 2`.
pub fn conflict_copy_path(path: &str, at: NaiveDateTime, n: u32) -> String {
    let stem = path.strip_suffix(".md").unwrap_or(path);
    let stamp = at.format("%Y-%m-%d %H%M%S");
    if n <= 1 {
        format!("{stem} (conflict {stamp}).md")
    } else {
        format!("{stem} (conflict {stamp} {n}).md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn path_helpers() {
        assert_eq!(parent("notes/a/b.md"), "notes/a");
        assert_eq!(parent("b.md"), "");
        assert_eq!(file_name("notes/a/b.md"), "b.md");
        assert_eq!(join("", "b.md"), "b.md");
        assert_eq!(join("x", "b.md"), "x/b.md");
        assert!(is_inbox_path("inbox/2026-09-27-120000.md"));
        assert!(!is_inbox_path("inbox/sub/x.md"));
        assert!(!is_inbox_path("notes/inbox/x.md"));
        assert!(!is_inbox_path("inbox.md"));
    }

    #[test]
    fn free_paths_ignore_other_folders_and_non_notes() {
        let taken = [
            "people/Ahmed.md",
            "people/ahmed 2.md",
            "people/sub/Ahmed 3.md",
            "companies/Ahmed 3.md",
            "people/Ahmed 3.png",
        ];
        assert_eq!(
            free_note_path("people", "Ahmed", taken),
            "people/Ahmed 3.md"
        );
        assert_eq!(free_note_path("", "x", ["x.md"]), "x 2.md");
    }
}
