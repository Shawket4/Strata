//! Lines, line endings and the final newline.
//!
//! Every input is split into lines that keep their terminator (`\n` or `\r\n`; a bare `\r`
//! is ordinary text). Before diffing:
//!
//! - **Line endings.** When the base uses one line-ending style throughout and a side uses
//!   the other style throughout, that side changed every terminator. Its lines are converted
//!   to the base style for diffing (so the conversion does not conflict with every line of
//!   the other side) and the result is written in the converted style: a line-ending change
//!   is merged like any other one-sided change. Mixed files are compared byte-exactly.
//! - **Final newline.** A missing final newline is turned into a separate flag (and a virtual
//!   terminator on the last line), merged three-way, and re-applied to the result, so adding
//!   a line at the end of a file whose last line had no newline does not conflict with
//!   adding the newline itself.

/// Line-ending style of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    /// Every terminated line ends with `\n`.
    Lf,
    /// Every terminated line ends with `\r\n`.
    CrLf,
    /// Both kinds occur.
    Mixed,
    /// No terminated line.
    None,
}

impl Style {
    const fn terminator(self) -> &'static str {
        match self {
            Self::CrLf => "\r\n",
            Self::Lf | Self::Mixed | Self::None => "\n",
        }
    }

    const fn is_uniform(self) -> bool {
        matches!(self, Self::Lf | Self::CrLf)
    }
}

/// Splits `text` into lines that keep their terminators.
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn style_of(lines: &[&str]) -> Style {
    let mut lf = false;
    let mut crlf = false;
    for l in lines {
        if l.ends_with("\r\n") {
            crlf = true;
        } else if l.ends_with('\n') {
            lf = true;
        }
    }
    match (lf, crlf) {
        (true, true) => Style::Mixed,
        (true, false) => Style::Lf,
        (false, true) => Style::CrLf,
        (false, false) => Style::None,
    }
}

/// Returns `line` with its terminator (if any) replaced by `style`'s.
fn restyle(line: &str, style: Style) -> String {
    let content = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'));
    match content {
        Some(c) => format!("{c}{}", style.terminator()),
        None => line.to_owned(),
    }
}

/// Removes the terminator of the last line of `text`, if any.
pub(crate) fn strip_final_terminator(text: &mut String) {
    if text.ends_with("\r\n") {
        text.truncate(text.len() - 2);
    } else if text.ends_with('\n') {
        text.truncate(text.len() - 1);
    }
}

/// One input, prepared for diffing.
#[derive(Debug, Clone)]
pub(crate) struct Side {
    /// Lines in working form (converted, virtual final terminator added).
    pub lines: Vec<String>,
    /// Original style.
    style: Style,
    /// Whether the lines were converted to the working style.
    converted: bool,
    /// Whether the original text lacked a final newline (and a virtual one was added).
    pub missing_final_newline: bool,
}

impl Side {
    /// Renders `range` of this side's lines as they were in the original text.
    pub fn original(&self, range: std::ops::Range<usize>) -> String {
        let includes_last = range.end == self.lines.len() && !range.is_empty();
        let mut out = String::new();
        for line in &self.lines[range] {
            if self.converted {
                out.push_str(&restyle(line, self.style));
            } else {
                out.push_str(line);
            }
        }
        if includes_last && self.missing_final_newline {
            strip_final_terminator(&mut out);
        }
        out
    }
}

/// The three prepared inputs plus how to write the output.
#[derive(Debug, Clone)]
pub(crate) struct Prepared {
    /// Base.
    pub base: Side,
    /// Ours.
    pub ours: Side,
    /// Theirs.
    pub theirs: Side,
    working: Style,
    target: Style,
    /// Whether the merged text lacks a final newline.
    pub missing_final_newline: bool,
}

fn prepare_side(text: &str, working: Style) -> Side {
    let raw = split_lines(text);
    let style = style_of(&raw);
    let converted = working.is_uniform() && style.is_uniform() && style != working;
    let mut lines: Vec<String> = raw
        .iter()
        .map(|l| {
            if converted {
                restyle(l, working)
            } else {
                (*l).to_owned()
            }
        })
        .collect();
    let missing_final_newline = lines.last().is_some_and(|l| !l.ends_with('\n'));
    if missing_final_newline && let Some(last) = lines.last_mut() {
        last.push_str(working.terminator());
    }
    Side {
        lines,
        style,
        converted,
        missing_final_newline,
    }
}

/// Prepares base, ours and theirs.
pub(crate) fn prepare(base: &str, ours: &str, theirs: &str) -> Prepared {
    let base_lines = split_lines(base);
    let base_style = style_of(&base_lines);
    let working = if base_style.is_uniform() {
        base_style
    } else {
        Style::None
    };
    let base = prepare_side(base, working);
    let ours = prepare_side(ours, working);
    let theirs = prepare_side(theirs, working);
    let target = if ours.converted {
        ours.style
    } else if theirs.converted {
        theirs.style
    } else {
        working
    };
    let missing_final_newline = if ours.missing_final_newline == base.missing_final_newline {
        theirs.missing_final_newline
    } else {
        ours.missing_final_newline
    };
    Prepared {
        base,
        ours,
        theirs,
        working,
        target,
        missing_final_newline,
    }
}

impl Prepared {
    /// Converts working-form lines to output form (without final-newline handling).
    pub fn output(&self, lines: &[String]) -> String {
        let convert = self.target != self.working && self.target.is_uniform();
        let mut out = String::new();
        for l in lines {
            if convert {
                out.push_str(&restyle(l, self.target));
            } else {
                out.push_str(l);
            }
        }
        out
    }

    /// The output terminator for marker lines.
    pub const fn terminator(&self) -> &'static str {
        if self.target.is_uniform() {
            self.target.terminator()
        } else {
            self.working.terminator()
        }
    }

    /// Whether ours supplied the output line-ending style.
    pub fn line_endings_from_ours(&self) -> bool {
        self.ours.converted
    }

    /// Whether the output style was changed by one side.
    pub fn line_endings_changed(&self) -> bool {
        self.ours.converted || self.theirs.converted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_keeping_terminators() {
        assert_eq!(split_lines("a\nb\r\nc"), ["a\n", "b\r\n", "c"]);
        assert_eq!(split_lines(""), Vec::<&str>::new());
        assert_eq!(split_lines("a\rb\n"), ["a\rb\n"]);
    }

    #[test]
    fn styles() {
        assert_eq!(style_of(&split_lines("a\nb\n")), Style::Lf);
        assert_eq!(style_of(&split_lines("a\r\nb")), Style::CrLf);
        assert_eq!(style_of(&split_lines("a\r\nb\n")), Style::Mixed);
        assert_eq!(style_of(&split_lines("a")), Style::None);
    }

    #[test]
    fn converted_side_renders_original_bytes() {
        let p = prepare("a\nb\n", "a\r\nb", "a\nb\n");
        assert_eq!(p.ours.lines, ["a\n", "b\n"]);
        assert!(p.ours.missing_final_newline);
        assert_eq!(p.ours.original(0..2), "a\r\nb");
        assert_eq!(p.ours.original(0..1), "a\r\n");
        assert_eq!(p.output(&p.ours.lines), "a\r\nb\r\n");
        assert!(p.missing_final_newline);
    }
}
