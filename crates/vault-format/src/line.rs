//! Line-ending detection and line iteration with byte offsets.

use std::ops::Range;

/// The line terminator used by a file. Strata preserves whatever a file uses and writes new
/// lines with the same terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineEnding {
    /// `\n`
    #[default]
    Lf,
    /// `\r\n`
    CrLf,
}

impl LineEnding {
    /// The terminator as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// The terminator of the first line of `text`; [`LineEnding::Lf`] when there is none.
    pub fn detect(text: &str) -> Self {
        match text.find('\n') {
            Some(i) if i > 0 && text.as_bytes()[i - 1] == b'\r' => Self::CrLf,
            _ => Self::Lf,
        }
    }

    /// Converts every `\n` (that is not already part of `\r\n`) in `text` to this terminator.
    pub fn apply(self, text: &str) -> String {
        let normalized = text.replace("\r\n", "\n");
        match self {
            Self::Lf => normalized,
            Self::CrLf => normalized.replace('\n', "\r\n"),
        }
    }
}

/// One line of a text with its byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    /// Offset of the first byte of the line.
    pub start: usize,
    /// The line content without its terminator.
    pub content: &'a str,
    /// The terminator (`""`, `"\n"` or `"\r\n"`).
    pub eol: &'a str,
}

impl Line<'_> {
    /// Byte range of the content (terminator excluded).
    pub fn content_range(&self) -> Range<usize> {
        self.start..self.start + self.content.len()
    }

    /// Byte range of the whole line including its terminator.
    pub fn full_range(&self) -> Range<usize> {
        self.start..self.start + self.content.len() + self.eol.len()
    }
}

/// Iterates the lines of `text`, keeping terminators. A trailing terminator does not start an
/// extra empty line.
pub fn lines(text: &str) -> impl Iterator<Item = Line<'_>> {
    let mut pos = 0;
    std::iter::from_fn(move || {
        if pos >= text.len() {
            return None;
        }
        let rest = &text[pos..];
        let start = pos;
        let (content, eol) = match rest.find('\n') {
            Some(i) => {
                if i > 0 && rest.as_bytes()[i - 1] == b'\r' {
                    (&rest[..i - 1], &rest[i - 1..=i])
                } else {
                    (&rest[..i], &rest[i..=i])
                }
            }
            None => (rest, ""),
        };
        pos += content.len() + eol.len();
        Some(Line {
            start,
            content,
            eol,
        })
    })
}

/// Start offset of the line containing byte `pos`.
pub(crate) fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// Returns `range` with trailing ASCII whitespace (spaces, tabs, CR, LF) removed.
pub(crate) fn trim_end_range(text: &str, range: Range<usize>) -> Range<usize> {
    let trimmed = text[range.clone()].trim_end_matches([' ', '\t', '\r', '\n']);
    range.start..range.start + trimmed.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_line_endings() {
        assert_eq!(LineEnding::detect("a\r\nb\n"), LineEnding::CrLf);
        assert_eq!(LineEnding::detect("a\nb\r\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("abc"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("\nx"), LineEnding::Lf);
    }

    #[test]
    fn applies_line_endings() {
        assert_eq!(LineEnding::CrLf.apply("a\nb\r\nc"), "a\r\nb\r\nc");
        assert_eq!(LineEnding::Lf.apply("a\r\nb\nc"), "a\nb\nc");
    }

    #[test]
    fn iterates_lines_with_offsets() {
        let got: Vec<_> = lines("ab\r\n\ncd")
            .map(|l| (l.start, l.content, l.eol))
            .collect();
        assert_eq!(got, vec![(0, "ab", "\r\n"), (4, "", "\n"), (5, "cd", "")]);
        assert_eq!(lines("x\n").count(), 1);
        assert_eq!(lines("").count(), 0);
    }

    #[test]
    fn line_bounds() {
        let t = "ab\r\ncd\nef";
        assert_eq!(line_start(t, 5), 4);
        assert_eq!(trim_end_range("ab \r\n", 0..5), 0..2);
    }
}
