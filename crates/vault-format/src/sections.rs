//! Heading sections and the AI-owned sections of concept, entity and document notes
//! (PLAN §6.6, §6.7, §6.12).
//!
//! The AI rewrites only the content of its own `## ` sections. Content "owned" by a section
//! runs from the line after its heading to the next heading of any level, so user sub-headings
//! placed under an AI section, `## Notes`, and every other user heading are preserved
//! byte-for-byte.

use std::fmt;
use std::ops::Range;

use chrono::NaiveDate;

use crate::body;
use crate::custody::{self, CustodyParseError};
use crate::line::LineEnding;
use crate::wikilink;

/// A heading and the text under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Heading level.
    pub level: u8,
    /// Heading text.
    pub title: String,
    /// Span of the heading source (without its final line terminator).
    pub heading_span: Range<usize>,
    /// From the line after the heading to the next heading of the same or a higher level.
    pub content_span: Range<usize>,
    /// From the line after the heading to the next heading of any level.
    pub own_content_span: Range<usize>,
}

/// All sections of a body, one per heading (outside code), in order.
pub fn sections(body: &str) -> Vec<Section> {
    let headings = body::analyze(body).headings;
    let mut out = Vec::with_capacity(headings.len());
    for (i, h) in headings.iter().enumerate() {
        let start = after_line(body, h.span.end);
        let own_end = headings
            .get(i + 1)
            .map_or(body.len(), |n| line_begin(body, n.span.start));
        let end = headings[i + 1..]
            .iter()
            .find(|n| n.level <= h.level)
            .map_or(body.len(), |n| line_begin(body, n.span.start));
        out.push(Section {
            level: h.level,
            title: h.text.clone(),
            heading_span: h.span.clone(),
            content_span: start..end.max(start),
            own_content_span: start..own_end.max(start),
        });
    }
    out
}

fn after_line(text: &str, pos: usize) -> usize {
    text[pos..].find('\n').map_or(text.len(), |i| pos + i + 1)
}

fn line_begin(text: &str, pos: usize) -> usize {
    crate::line::line_start(text, pos)
}

/// A section the AI maintains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiSection {
    /// `## Summary` (prose).
    Summary,
    /// `## Insights` (cited bullets).
    Insights,
    /// `## Open items` (cited bullets).
    OpenItems,
    /// `## Timeline` (dated, cited bullets, newest first).
    Timeline,
    /// `## Custody` (custody event lines, newest first).
    Custody,
}

impl AiSection {
    /// The heading text.
    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Insights => "Insights",
            Self::OpenItems => "Open items",
            Self::Timeline => "Timeline",
            Self::Custody => "Custody",
        }
    }

    fn is_bullets(self) -> bool {
        !matches!(self, Self::Summary)
    }

    fn is_dated(self) -> bool {
        matches!(self, Self::Timeline | Self::Custody)
    }
}

impl fmt::Display for AiSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

/// Which AI sections a note kind has, in body order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiProfile {
    /// People and companies: Summary, Insights, Open items, Timeline.
    Entity,
    /// Documents: Summary, Custody.
    Document,
    /// Concepts: Summary.
    Concept,
}

impl AiProfile {
    /// The sections, in the order they appear in the body.
    pub fn sections(self) -> &'static [AiSection] {
        match self {
            Self::Entity => &[
                AiSection::Summary,
                AiSection::Insights,
                AiSection::OpenItems,
                AiSection::Timeline,
            ],
            Self::Document => &[AiSection::Summary, AiSection::Custody],
            Self::Concept => &[AiSection::Summary],
        }
    }
}

/// A rule broken by AI section content. Line numbers are 1-based within the content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Violation {
    /// Headings would break the section structure.
    #[error("line {0}: headings are not allowed in AI section content")]
    Heading(usize),
    /// Bullet sections contain only `- ` bullets.
    #[error("line {0}: only `- ` bullets are allowed")]
    NotABullet(usize),
    /// Every AI bullet cites a note or block.
    #[error("line {0}: bullet has no [[citation]]")]
    Uncited(usize),
    /// Timeline and custody entries start with a date.
    #[error("line {0}: entry must start with a YYYY-MM-DD date")]
    Undated(usize),
    /// Timeline and custody entries are newest first.
    #[error("line {0}: entries must be newest first")]
    NotNewestFirst(usize),
    /// A custody line does not parse.
    #[error("line {line}: {error}")]
    Custody {
        /// Line.
        line: usize,
        /// Parse error.
        error: CustodyParseError,
    },
}

/// Why AI sections could not be replaced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SectionError {
    /// The section does not belong to the note's profile.
    #[error("`{section}` is not an AI section of this note kind")]
    NotInProfile {
        /// The section.
        section: AiSection,
    },
    /// The body has the heading more than once; which one is the AI's is ambiguous.
    #[error("`## {0}` appears more than once")]
    Duplicate(AiSection),
    /// The same section was given twice.
    #[error("`{0}` was given twice")]
    RepeatedUpdate(AiSection),
    /// The content breaks the rules.
    #[error("invalid content for `{section}`: {violations:?}")]
    InvalidContent {
        /// The section.
        section: AiSection,
        /// Every violation found.
        violations: Vec<Violation>,
    },
}

fn leading_date(text: &str) -> Option<NaiveDate> {
    let d = text.get(..10)?;
    let rest = &text[10..];
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// Checks AI content for a section: no headings; bullet sections hold only `- ` bullets, each
/// with at least one (non-embed) wikilink; timeline/custody bullets start with a date and are
/// newest first; custody bullets parse as custody events and cite at least one note (the
/// argument links do not count; citation-free lines are for user-recorded events only).
pub fn validate_content(section: AiSection, content: &str) -> Result<(), Vec<Violation>> {
    let mut v = Vec::new();
    let mut last_date: Option<NaiveDate> = None;
    for (i, line) in content.lines().enumerate() {
        let n = i + 1;
        if line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim_start();
        let after_hashes = trimmed.trim_start_matches('#');
        let is_heading = trimmed.starts_with('#')
            && (after_hashes.is_empty() || after_hashes.starts_with([' ', '\t']));
        if is_heading {
            v.push(Violation::Heading(n));
            continue;
        }
        if !section.is_bullets() {
            continue;
        }
        let Some(item) = line.strip_prefix("- ") else {
            v.push(Violation::NotABullet(n));
            continue;
        };
        // Custody lines' argument links are not citations: an AI custody event must carry its
        // own citation part (only user-recorded events may omit it, and they are never
        // written through here).
        let custody = (section == AiSection::Custody).then(|| custody::CustodyEvent::parse(line));
        let uncited_event = matches!(&custody, Some(Ok(e)) if e.require_citation().is_err());
        if uncited_event || !wikilink::find_all(item).iter().any(|l| !l.embed) {
            v.push(Violation::Uncited(n));
        }
        if section.is_dated() {
            match leading_date(item) {
                Some(d) => {
                    if last_date.is_some_and(|prev| d > prev) {
                        v.push(Violation::NotNewestFirst(n));
                    }
                    last_date = Some(d);
                }
                None => v.push(Violation::Undated(n)),
            }
        }
        if let Some(Err(error)) = custody {
            v.push(Violation::Custody { line: n, error });
        }
    }
    if v.is_empty() { Ok(()) } else { Err(v) }
}

enum Edit {
    Replace(Range<usize>, String),
    Insert(usize, usize, String),
}

impl Edit {
    fn key(&self) -> (usize, u8, usize) {
        match self {
            Self::Replace(r, _) => (r.start, 0, 0),
            Self::Insert(p, order, _) => (*p, 1, *order),
        }
    }
}

/// Replaces (or inserts) the given AI sections of `body`.
///
/// Existing AI sections keep their heading line; only their own content changes. Missing
/// sections are inserted in profile order: before the next AI section that exists, else after
/// the previous one, else before the first `## ` heading, else at the end. Content is
/// validated with [`validate_content`] and written with the body's line ending, followed by
/// one blank line when another heading follows.
pub fn replace_ai_sections(
    body: &str,
    profile: AiProfile,
    updates: &[(AiSection, &str)],
) -> Result<String, SectionError> {
    let order = profile.sections();
    for (i, (s, content)) in updates.iter().enumerate() {
        if !order.contains(s) {
            return Err(SectionError::NotInProfile { section: *s });
        }
        if updates[..i].iter().any(|(p, _)| p == s) {
            return Err(SectionError::RepeatedUpdate(*s));
        }
        validate_content(*s, content).map_err(|violations| SectionError::InvalidContent {
            section: *s,
            violations,
        })?;
    }
    let eol = LineEnding::detect(body);
    let all = sections(body);
    let find = |s: AiSection| -> Result<Option<&Section>, SectionError> {
        let mut hits = all
            .iter()
            .filter(|x| x.level == 2 && x.title.trim().eq_ignore_ascii_case(s.title()));
        let first = hits.next();
        if hits.next().is_some() {
            return Err(SectionError::Duplicate(s));
        }
        Ok(first)
    };
    let mut present = Vec::with_capacity(order.len());
    for &s in order {
        present.push(find(s)?);
    }
    let mut edits = Vec::new();
    for (s, content) in updates {
        let idx = order.iter().position(|o| o == s).unwrap_or(0);
        let content = eol.apply(content.trim_end_matches([' ', '\t', '\r', '\n']));
        if let Some(sec) = present[idx] {
            let at_end = sec.own_content_span.end == body.len();
            edits.push(Edit::Replace(
                sec.own_content_span.clone(),
                content_block(&content, eol, at_end),
            ));
        } else {
            let pos = insertion_point(body, &all, &present, idx);
            let at_end = pos == body.len();
            let text = format!(
                "## {}{}{}",
                s.title(),
                eol.as_str(),
                content_block(&content, eol, at_end)
            );
            edits.push(Edit::Insert(pos, idx, text));
        }
    }
    edits.sort_by_key(Edit::key);
    let mut out = String::with_capacity(body.len() + 512);
    let mut last = 0;
    for e in edits {
        match e {
            Edit::Replace(r, text) => {
                out.push_str(&body[last..r.start]);
                out.push_str(&text);
                last = r.end;
            }
            Edit::Insert(pos, _, text) => {
                out.push_str(&body[last..pos]);
                last = pos;
                if pos == body.len() {
                    separate(&mut out, eol);
                }
                out.push_str(&text);
            }
        }
    }
    out.push_str(&body[last..]);
    Ok(out)
}

fn content_block(content: &str, eol: LineEnding, at_end: bool) -> String {
    let e = eol.as_str();
    match (content.is_empty(), at_end) {
        (true, true) => String::new(),
        (true, false) => e.to_owned(),
        (false, true) => format!("{content}{e}"),
        (false, false) => format!("{content}{e}{e}"),
    }
}

/// Ensures `out` ends with a blank line before appending a section (unless it is empty).
fn separate(out: &mut String, eol: LineEnding) {
    if out.trim().is_empty() {
        out.clear();
        return;
    }
    let e = eol.as_str();
    if !out.ends_with(e) {
        out.push_str(e);
    }
    if !out.ends_with(&format!("{e}{e}")) {
        out.push_str(e);
    }
}

fn insertion_point(body: &str, all: &[Section], present: &[Option<&Section>], idx: usize) -> usize {
    if let Some(next) = present[idx + 1..].iter().flatten().next() {
        return line_begin(body, next.heading_span.start);
    }
    if let Some(prev) = present[..idx].iter().rev().flatten().next() {
        return prev.content_span.end;
    }
    all.iter()
        .find(|s| s.level <= 2)
        .map_or(body.len(), |s| line_begin(body, s.heading_span.start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_spans() {
        let body = "intro\n## A\na text\n### A1\nsub\n## B\nb\n";
        let s = sections(body);
        assert_eq!(s.len(), 3);
        assert_eq!(&body[s[0].content_span.clone()], "a text\n### A1\nsub\n");
        assert_eq!(&body[s[0].own_content_span.clone()], "a text\n");
        assert_eq!(&body[s[2].content_span.clone()], "b\n");
        assert_eq!(s[1].level, 3);
    }

    #[test]
    fn replaces_existing_and_preserves_user_text() {
        let body = "## Summary\nold summary\n\n## Insights\n- old [[a]]\n### My own\nkeep me\n\n## Notes\nuser **text**\n";
        let out = replace_ai_sections(
            body,
            AiProfile::Entity,
            &[
                (AiSection::Summary, "New summary."),
                (
                    AiSection::Insights,
                    "- Prefers weekly invoicing [[Call 2026-09-12#^a1b2]]\n",
                ),
            ],
        );
        assert_eq!(
            out.as_deref(),
            Ok(
                "## Summary\nNew summary.\n\n## Insights\n- Prefers weekly invoicing [[Call 2026-09-12#^a1b2]]\n\n### My own\nkeep me\n\n## Notes\nuser **text**\n"
            )
        );
    }

    #[test]
    fn inserts_missing_sections_in_order() {
        let body = "## Notes\nmine\n";
        let out = replace_ai_sections(
            body,
            AiProfile::Entity,
            &[
                (AiSection::Timeline, "- 2026-09-12 — call [[Call]]"),
                (AiSection::Summary, "S."),
            ],
        );
        assert_eq!(
            out.as_deref(),
            Ok("## Summary\nS.\n\n## Timeline\n- 2026-09-12 — call [[Call]]\n\n## Notes\nmine\n")
        );
        let body2 = out.unwrap_or_default();
        let out = replace_ai_sections(
            &body2,
            AiProfile::Entity,
            &[(AiSection::Insights, "- x [[y]]")],
        );
        assert_eq!(
            out.as_deref(),
            Ok(
                "## Summary\nS.\n\n## Insights\n- x [[y]]\n\n## Timeline\n- 2026-09-12 — call [[Call]]\n\n## Notes\nmine\n"
            )
        );
    }

    #[test]
    fn appends_to_empty_or_plain_bodies() {
        let out = replace_ai_sections("", AiProfile::Concept, &[(AiSection::Summary, "Def.")]);
        assert_eq!(out.as_deref(), Ok("## Summary\nDef.\n"));
        let out = replace_ai_sections(
            "user text",
            AiProfile::Concept,
            &[(AiSection::Summary, "Def.")],
        );
        assert_eq!(out.as_deref(), Ok("user text\n\n## Summary\nDef.\n"));
        let out = replace_ai_sections(
            "## Summary\nold",
            AiProfile::Document,
            &[(AiSection::Custody, "- 2026-01-01 — lost — [[c]]")],
        );
        assert_eq!(
            out.as_deref(),
            Ok("## Summary\nold\n\n## Custody\n- 2026-01-01 — lost — [[c]]\n")
        );
    }

    #[test]
    fn crlf_bodies_stay_crlf() {
        let body = "## Summary\r\nold\r\n\r\n## Notes\r\nn\r\n";
        let out = replace_ai_sections(body, AiProfile::Concept, &[(AiSection::Summary, "a\nb")]);
        assert_eq!(
            out.as_deref(),
            Ok("## Summary\r\na\r\nb\r\n\r\n## Notes\r\nn\r\n")
        );
    }

    #[test]
    fn empty_content() {
        let body = "## Summary\nold\n## Notes\nn\n";
        let out = replace_ai_sections(body, AiProfile::Concept, &[(AiSection::Summary, "")]);
        assert_eq!(out.as_deref(), Ok("## Summary\n\n## Notes\nn\n"));
    }

    #[test]
    fn errors() {
        assert_eq!(
            replace_ai_sections(
                "",
                AiProfile::Concept,
                &[(AiSection::Insights, "- a [[b]]")]
            ),
            Err(SectionError::NotInProfile {
                section: AiSection::Insights
            })
        );
        assert_eq!(
            replace_ai_sections(
                "## Summary\n## Summary\n",
                AiProfile::Concept,
                &[(AiSection::Summary, "x")]
            ),
            Err(SectionError::Duplicate(AiSection::Summary))
        );
        assert_eq!(
            replace_ai_sections(
                "",
                AiProfile::Concept,
                &[(AiSection::Summary, "x"), (AiSection::Summary, "y")]
            ),
            Err(SectionError::RepeatedUpdate(AiSection::Summary))
        );
    }

    #[test]
    fn validation_rules() {
        assert_eq!(
            validate_content(AiSection::Summary, "Plain prose, no cite.\n\nMore."),
            Ok(())
        );
        assert_eq!(
            validate_content(AiSection::Summary, "## Sneaky"),
            Err(vec![Violation::Heading(1)])
        );
        assert_eq!(
            validate_content(
                AiSection::Insights,
                "- cited [[a]]\n- uncited\ntext\n- embed only ![[img.png]]\n  - nested [[x]]"
            ),
            Err(vec![
                Violation::Uncited(2),
                Violation::NotABullet(3),
                Violation::Uncited(4),
                Violation::NotABullet(5)
            ])
        );
        assert_eq!(
            validate_content(
                AiSection::Timeline,
                "- 2026-09-12 — a [[x]]\n- 2026-09-20 — b [[y]]\n- soon [[z]]"
            ),
            Err(vec![Violation::NotNewestFirst(2), Violation::Undated(3)])
        );
        assert_eq!(
            validate_content(AiSection::Custody, "- 2026-09-12 — misplaced — [[x]]"),
            Err(vec![Violation::Custody {
                line: 1,
                error: CustodyParseError::UnknownType("misplaced".into())
            }])
        );
        // AI custody events must cite: the argument links are not citations.
        assert_eq!(
            validate_content(
                AiSection::Custody,
                "- 2026-09-21 — handed-to [[Shady]] — [[Capture#^c1]]\n\
                 - 2026-09-20 — handed-to [[Shady]]\n\
                 - 2026-09-19 — lost"
            ),
            Err(vec![Violation::Uncited(2), Violation::Uncited(3)])
        );
        assert_eq!(
            validate_content(AiSection::OpenItems, "- Send the quote [[Call#^b1]]"),
            Ok(())
        );
    }
}
