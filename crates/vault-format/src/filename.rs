//! File names (PLAN §6.2): forbidden characters, validation, sanitisation, titles.

use crate::resolve::link_name;

/// Characters Obsidian cannot use in note names (they break links or paths).
pub const FORBIDDEN_CHARS: &[char] = &['*', '"', '\\', '/', '<', '>', ':', '|', '?', '#', '^', '[', ']'];

/// Longest file name (bytes, extension included) Strata writes; most file systems allow 255.
pub const MAX_NAME_BYTES: usize = 255;

const RESERVED_WINDOWS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
    "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Why a file name is not allowed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileNameError {
    /// Empty or only whitespace.
    #[error("file name is empty")]
    Empty,
    /// Contains a forbidden character.
    #[error("file name contains the forbidden character `{0}`")]
    ForbiddenChar(char),
    /// Contains a control character.
    #[error("file name contains a control character (U+{0:04X})")]
    ControlChar(u32),
    /// Starts with `.` (hidden from Obsidian and the API tree).
    #[error("file name starts with a dot")]
    LeadingDot,
    /// Starts or ends with whitespace, or ends with `.` (not portable to Windows).
    #[error("file name starts or ends with whitespace or ends with a dot")]
    EdgeWhitespaceOrDot,
    /// A reserved device name on Windows.
    #[error("`{0}` is a reserved name on Windows")]
    Reserved(String),
    /// Longer than [`MAX_NAME_BYTES`].
    #[error("file name is {0} bytes; the limit is 255")]
    TooLong(usize),
}

/// Validates one file name (a single path segment, extension included).
pub fn validate_file_name(name: &str) -> Result<(), FileNameError> {
    if name.trim().is_empty() {
        return Err(FileNameError::Empty);
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN_CHARS.contains(c)) {
        return Err(FileNameError::ForbiddenChar(c));
    }
    if let Some(c) = name.chars().find(|c| c.is_control()) {
        return Err(FileNameError::ControlChar(u32::from(c)));
    }
    if name.starts_with('.') {
        return Err(FileNameError::LeadingDot);
    }
    if name.trim() != name || name.ends_with('.') {
        return Err(FileNameError::EdgeWhitespaceOrDot);
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED_WINDOWS.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(FileNameError::Reserved(stem.to_owned()));
    }
    if name.len() > MAX_NAME_BYTES {
        return Err(FileNameError::TooLong(name.len()));
    }
    Ok(())
}

/// Why a vault path is not allowed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// A segment is invalid.
    #[error("invalid path segment `{segment}`: {error}")]
    Segment {
        /// The segment.
        segment: String,
        /// Why.
        error: FileNameError,
    },
    /// Absolute paths are not vault-relative.
    #[error("vault paths are relative")]
    Absolute,
}

/// Validates a vault-relative path (`notes/Sub/Name.md`): every segment must be a valid name.
pub fn validate_vault_path(path: &str) -> Result<(), PathError> {
    if path.starts_with('/') {
        return Err(PathError::Absolute);
    }
    for segment in path.split('/') {
        validate_file_name(segment).map_err(|error| PathError::Segment {
            segment: segment.to_owned(),
            error,
        })?;
    }
    Ok(())
}

/// Turns arbitrary text (e.g. a title, possibly Arabic) into a valid note name without the
/// `.md` extension:
/// `/ \ |` become `-`; `:` becomes ` -`; other forbidden and control characters become
/// spaces; whitespace runs collapse to one space; leading dots and trailing dots/spaces are
/// removed; Windows device names get a trailing `_`; the result is cut to 200 bytes on a
/// character boundary; an empty result becomes `Untitled`.
pub fn sanitize_file_name(title: &str) -> String {
    let mut mapped = String::with_capacity(title.len());
    for c in title.chars() {
        match c {
            '/' | '\\' | '|' => mapped.push('-'),
            ':' => mapped.push_str(" -"),
            c if FORBIDDEN_CHARS.contains(&c) || c.is_control() => mapped.push(' '),
            c => mapped.push(c),
        }
    }
    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut name = collapsed.trim_start_matches('.').trim_end_matches(['.', ' ']).trim().to_owned();
    if name.len() > 200 {
        let mut cut = 200;
        while !name.is_char_boundary(cut) {
            cut -= 1;
        }
        name.truncate(cut);
        name = name.trim_end_matches(['.', ' ']).to_owned();
    }
    let stem = name.split('.').next().unwrap_or(&name).to_owned();
    if RESERVED_WINDOWS.iter().any(|r| r.eq_ignore_ascii_case(&stem)) {
        name.insert(stem.len(), '_');
    }
    if name.is_empty() {
        name = "Untitled".to_owned();
    }
    name
}

/// The note title for a vault path: the file name without `.md` (PLAN §6.2).
pub fn title_from_path(path: &str) -> &str {
    link_name(path)
}

/// A name based on `base` that is not in `taken` (compared case-insensitively):
/// `base`, `base 2`, `base 3`, …
pub fn unique_name<'a, I>(base: &str, taken: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let taken: std::collections::HashSet<String> = taken.into_iter().map(str::to_lowercase).collect();
    if !taken.contains(&base.to_lowercase()) {
        return base.to_owned();
    }
    (2u32..)
        .map(|n| format!("{base} {n}"))
        .find(|c| !taken.contains(&c.to_lowercase()))
        .unwrap_or_else(|| base.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert_eq!(validate_file_name("Pricing experiments.md"), Ok(()));
        assert_eq!(validate_file_name("أحمد سمير.md"), Ok(()));
        assert_eq!(validate_file_name("Safe — Nasr City office.md"), Ok(()));
        for c in FORBIDDEN_CHARS {
            assert_eq!(validate_file_name(&format!("a{c}b.md")), Err(FileNameError::ForbiddenChar(*c)));
        }
        assert_eq!(validate_file_name(""), Err(FileNameError::Empty));
        assert_eq!(validate_file_name("   "), Err(FileNameError::Empty));
        assert_eq!(validate_file_name("a\tb"), Err(FileNameError::ControlChar(9)));
        assert_eq!(validate_file_name(".hidden.md"), Err(FileNameError::LeadingDot));
        assert_eq!(validate_file_name(" a.md"), Err(FileNameError::EdgeWhitespaceOrDot));
        assert_eq!(validate_file_name("a."), Err(FileNameError::EdgeWhitespaceOrDot));
        assert_eq!(validate_file_name("con.md"), Err(FileNameError::Reserved("con".into())));
        assert_eq!(validate_file_name(&"a".repeat(256)), Err(FileNameError::TooLong(256)));
    }

    #[test]
    fn path_validation() {
        assert_eq!(validate_vault_path("notes/sub/x.md"), Ok(()));
        assert_eq!(validate_vault_path("/x.md"), Err(PathError::Absolute));
        assert!(matches!(
            validate_vault_path("notes//x.md"),
            Err(PathError::Segment { error: FileNameError::Empty, .. })
        ));
        assert!(matches!(
            validate_vault_path("../x.md"),
            Err(PathError::Segment { error: FileNameError::LeadingDot, .. })
        ));
    }

    #[test]
    fn sanitisation() {
        assert_eq!(sanitize_file_name("Q3: plan / review"), "Q3 - plan - review");
        assert_eq!(sanitize_file_name("What? #1 [draft]"), "What 1 draft");
        assert_eq!(sanitize_file_name("  ..hidden..  "), "hidden");
        assert_eq!(sanitize_file_name("عقد وطنية: نسخة"), "عقد وطنية - نسخة");
        assert_eq!(sanitize_file_name("***"), "Untitled");
        assert_eq!(sanitize_file_name("CON"), "CON_");
        assert_eq!(sanitize_file_name("aux.notes"), "aux_.notes");
        let long = "ب".repeat(150);
        let s = sanitize_file_name(&long);
        assert_eq!(s.len(), 200);
        assert!(validate_file_name(&format!("{s}.md")).is_ok());
    }

    #[test]
    fn sanitised_names_are_valid() {
        for t in ["a:b", "x|y", "..", "a\u{0}b", "é/è", "#", "[[x]]", "a.", "NUL.txt"] {
            let s = sanitize_file_name(t);
            assert_eq!(validate_file_name(&format!("{s}.md")), Ok(()), "{t:?} -> {s:?}");
        }
    }

    #[test]
    fn titles_and_unique_names() {
        assert_eq!(title_from_path("people/أحمد سمير.md"), "أحمد سمير");
        assert_eq!(title_from_path("x/v1.2 plan.md"), "v1.2 plan");
        assert_eq!(unique_name("Note", ["note", "Note 2"]), "Note 3");
        assert_eq!(unique_name("Fresh", ["note"]), "Fresh");
    }
}
