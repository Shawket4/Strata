//! Highlight spans for the editor (PLAN §12.1 `format/`: "render hints, highlight spans for
//! the editor"). Offsets are **UTF-16 code units** so Dart can use them on its strings
//! directly, with no index arithmetic on the Dart side.
//!
//! Besides the markdown structure the spans carry what the editor needs to act without logic:
//! the link path and anchor of wikilinks (resolved to note IDs by the view builder), the block
//! ID of task lines, heading levels, emphasis runs and the direction of every line by its
//! first strong character (PLAN §11 "per-line direction in the editor"). Every styled span
//! carries its exact marker ranges from `vault-format`'s parser, so the editor never guesses
//! marker widths.

use std::ops::Range;

use vault_format::body::InlineStyle;
use vault_format::{Anchor, Document};

use crate::format::direction::dir_of;
use crate::view::model::TextDir;

/// What a span is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanKind {
    /// The frontmatter block.
    Frontmatter,
    /// A heading line.
    Heading,
    /// `[[link]]`.
    WikiLink,
    /// `![[embed]]`.
    Embed,
    /// `#tag`.
    Tag,
    /// `^block-id`.
    BlockId,
    /// A task line.
    TaskLine,
    /// Code (inert).
    Code,
    /// `**bold**`.
    Bold,
    /// `*italic*`.
    Italic,
    /// `~~strike~~`.
    Strike,
    /// `==mark==`.
    Mark,
    /// A right-to-left line.
    RtlLine,
    /// A left-to-right line.
    LtrLine,
}

/// One span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Kind.
    pub kind: SpanKind,
    /// Start (UTF-16 units, inclusive).
    pub start: u32,
    /// End (UTF-16 units, exclusive).
    pub end: u32,
    /// Links: the link path as written (resolve with the note's path).
    pub link_path: Option<String>,
    /// Links: heading or block (without `^`).
    pub anchor: Option<String>,
    /// Task lines: the block ID.
    pub task_id: Option<String>,
    /// Headings: level.
    pub level: u8,
    /// The markdown markers of the span (UTF-16 ranges, sorted), exactly as `vault-format`'s
    /// parser matched them: emphasis delimiters, a heading's `#`s and spaces, a link's
    /// brackets (and `path|` of an aliased link).
    pub markers: Vec<(u32, u32)>,
}

impl Span {
    fn plain(kind: SpanKind, start: u32, end: u32) -> Self {
        Self {
            kind,
            start,
            end,
            link_path: None,
            anchor: None,
            task_id: None,
            level: 0,
            markers: Vec::new(),
        }
    }
}

/// Maps byte offsets of a text to UTF-16 offsets.
#[derive(Debug)]
pub struct Utf16Map<'a> {
    text: &'a str,
}

impl<'a> Utf16Map<'a> {
    /// A map over `text`.
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }

    /// The UTF-16 offset of byte `byte` (clamped).
    pub fn at(&self, byte: usize) -> u32 {
        let mut byte = byte.min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        let units: usize = self.text[..byte].chars().map(char::len_utf16).sum();
        u32::try_from(units).unwrap_or(u32::MAX)
    }

    /// The byte offset of UTF-16 offset `unit` (clamped to the text; inside a surrogate pair
    /// it rounds down).
    pub fn byte_of(&self, unit: u32) -> usize {
        let mut units = 0usize;
        for (i, c) in self.text.char_indices() {
            if units >= unit as usize {
                return i;
            }
            units += c.len_utf16();
            if units > unit as usize {
                return i;
            }
        }
        self.text.len()
    }
}

/// Highlight spans of a whole note, sorted by start, then end, then kind.
pub fn editor_hints(content: &str) -> Vec<Span> {
    let doc = Document::parse(content);
    let map = Utf16Map::new(content);
    let off = doc.body_offset();
    let mut out = Vec::new();
    let mut push = |span: Span| out.push(span);
    let u = |b: usize| map.at(b);
    if doc.frontmatter().is_some() && off > 0 {
        push(Span::plain(SpanKind::Frontmatter, 0, u(off)));
    }
    let a = doc.analyze_body();
    let range = |r: &Range<usize>| (u(off + r.start), u(off + r.end));
    for h in &a.headings {
        push(Span {
            level: h.level,
            markers: h.markers.iter().map(range).collect(),
            ..Span::plain(
                SpanKind::Heading,
                u(off + h.span.start),
                u(off + h.span.end),
            )
        });
    }
    for l in &a.links {
        let kind = if l.embed {
            SpanKind::Embed
        } else {
            SpanKind::WikiLink
        };
        // `[[` (or `![[`, with `path|` when an alias is shown instead) and `]]`.
        let open_end = l.alias_span.as_ref().map_or(l.path_span.start, |a| a.start);
        push(Span {
            link_path: Some(l.path.clone()),
            anchor: l.anchor.as_ref().map(|a| match a {
                Anchor::Heading(h) => h.clone(),
                Anchor::Block(b) => b.clone(),
            }),
            markers: vec![
                range(&(l.span.start..open_end)),
                range(&(l.span.end.saturating_sub(2)..l.span.end)),
            ],
            ..Span::plain(kind, u(off + l.span.start), u(off + l.span.end))
        });
    }
    for t in &a.tags {
        push(Span::plain(
            SpanKind::Tag,
            u(off + t.span.start),
            u(off + t.span.end),
        ));
    }
    // Styled runs with the delimiters the shared parser matched (`***x***` is an emphasis
    // around a strong run; code and link paths are never styled).
    for i in &a.inline {
        let kind = match i.style {
            InlineStyle::Strong => SpanKind::Bold,
            InlineStyle::Emphasis => SpanKind::Italic,
            InlineStyle::Strikethrough => SpanKind::Strike,
            InlineStyle::Mark => SpanKind::Mark,
        };
        push(Span {
            markers: vec![range(&i.open), range(&i.close)],
            ..Span::plain(kind, u(off + i.span.start), u(off + i.span.end))
        });
    }
    for b in &a.blocks {
        if let Some(id) = &b.id {
            push(Span::plain(
                SpanKind::BlockId,
                u(off + id.span.start),
                u(off + id.span.end),
            ));
        }
    }
    for c in &a.code_spans {
        push(Span::plain(
            SpanKind::Code,
            u(off + c.start),
            u(off + c.end),
        ));
    }
    for t in vault_format::tasks::extract_tasks(doc.body()) {
        push(Span {
            task_id: t.task.block_id().map(str::to_owned),
            ..Span::plain(
                SpanKind::TaskLine,
                u(off + t.line_span.start),
                u(off + t.line_span.end),
            )
        });
    }
    // Line directions over the body, line by line.
    let body = doc.body();
    let mut pos = 0usize;
    for line in body.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        let base = off + pos;
        let kind = match dir_of(text) {
            TextDir::Rtl => Some(SpanKind::RtlLine),
            TextDir::Ltr => Some(SpanKind::LtrLine),
            TextDir::Neutral => None,
        };
        if let Some(kind) = kind {
            push(Span::plain(kind, u(base), u(base + text.len())));
        }
        pos += line.len();
    }
    out.sort_by_key(|h| (h.start, h.end, h.kind as u8));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(SpanKind, u32, u32)> {
        editor_hints(text)
            .into_iter()
            .map(|h| (h.kind, h.start, h.end))
            .collect()
    }

    #[test]
    fn spans_are_utf16() {
        // "أ" is 2 bytes / 1 UTF-16 unit; "😀" is 4 bytes / 2 units.
        let hints = editor_hints("أ😀 [[Note]] #tag\n");
        assert_eq!(
            hints,
            vec![
                Span::plain(SpanKind::RtlLine, 0, 17),
                Span {
                    link_path: Some("Note".into()),
                    markers: vec![(4, 6), (10, 12)],
                    ..Span::plain(SpanKind::WikiLink, 4, 12)
                },
                Span::plain(SpanKind::Tag, 13, 17),
            ]
        );
    }

    #[test]
    fn frontmatter_heading_task_and_block_id() {
        let text = "---\nid: X\n---\n# Title\n- [ ] do ^t-1\n";
        assert_eq!(
            kinds(text),
            vec![
                (SpanKind::Frontmatter, 0, 14),
                (SpanKind::Heading, 14, 21),
                (SpanKind::LtrLine, 14, 21),
                (SpanKind::TaskLine, 22, 35),
                (SpanKind::LtrLine, 22, 35),
                (SpanKind::BlockId, 31, 35),
            ]
        );
        let hints = editor_hints(text);
        assert_eq!(hints[1].level, 1);
        assert_eq!(hints[3].task_id.as_deref(), Some("t-1"));
    }

    #[test]
    fn emphasis_runs_skip_code_and_links() {
        let text = "**bold** _it_ ~~gone~~ ==hi== `**no**` [[a_b_c]] snake_case_word *x*\n";
        let spans: Vec<(SpanKind, u32, u32)> = kinds(text)
            .into_iter()
            .filter(|(k, _, _)| {
                matches!(
                    k,
                    SpanKind::Bold | SpanKind::Italic | SpanKind::Strike | SpanKind::Mark
                )
            })
            .collect();
        assert_eq!(
            spans,
            vec![
                (SpanKind::Bold, 0, 8),
                (SpanKind::Italic, 9, 13),
                (SpanKind::Strike, 14, 22),
                (SpanKind::Mark, 23, 29),
                (SpanKind::Italic, 65, 68),
            ]
        );
    }

    #[test]
    fn line_directions_and_heading_levels() {
        let text = "## الفرضيات\n- خصم 10%\n\n123\nAcme\n";
        assert_eq!(
            kinds(text),
            vec![
                (SpanKind::Heading, 0, 11),
                (SpanKind::RtlLine, 0, 11),
                (SpanKind::RtlLine, 12, 21),
                (SpanKind::LtrLine, 27, 31),
            ]
        );
        assert_eq!(editor_hints(text)[0].level, 2);
    }

    #[test]
    fn link_anchor_is_carried() {
        let hints = editor_hints("See [[Call 2026-09-12#^a1b2|call]]\n");
        let link = hints
            .iter()
            .find(|h| h.kind == SpanKind::WikiLink)
            .expect("link");
        assert_eq!(link.link_path.as_deref(), Some("Call 2026-09-12"));
        assert_eq!(link.anchor.as_deref(), Some("a1b2"));
    }

    /// Every styled span with its markers, as the text they cover.
    fn marked(text: &str) -> Vec<(SpanKind, String, Vec<String>)> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let s = |a: u32, b: u32| String::from_utf16_lossy(&units[a as usize..b as usize]);
        editor_hints(text)
            .into_iter()
            .filter(|h| !h.markers.is_empty())
            .map(|h| {
                (
                    h.kind,
                    s(h.start, h.end),
                    h.markers.iter().map(|&(a, b)| s(a, b)).collect(),
                )
            })
            .collect()
    }

    fn m(kind: SpanKind, span: &str, markers: &[&str]) -> (SpanKind, String, Vec<String>) {
        (
            kind,
            span.to_owned(),
            markers.iter().map(|x| (*x).to_owned()).collect(),
        )
    }

    #[test]
    fn markers_are_exact_for_every_variant() {
        use SpanKind::{Bold, Embed, Heading, Italic, Mark, Strike, WikiLink};
        // Nested emphasis: `***` is an emphasis around a strong run, each with its own
        // markers; `__`/`_` and `~`/`~~` keep their widths.
        assert_eq!(
            marked("***both*** __u__ _e_ ~one~ ~~two~~ ==hi==\n"),
            vec![
                m(Italic, "***both***", &["*", "*"]),
                m(Bold, "**both**", &["**", "**"]),
                m(Bold, "__u__", &["__", "__"]),
                m(Italic, "_e_", &["_", "_"]),
                m(Strike, "~one~", &["~", "~"]),
                m(Strike, "~~two~~", &["~~", "~~"]),
                m(Mark, "==hi==", &["==", "=="]),
            ]
        );
        assert_eq!(
            marked("**bold *it* more**\n"),
            vec![
                m(Bold, "**bold *it* more**", &["**", "**"]),
                m(Italic, "*it*", &["*", "*"]),
            ]
        );
        // Headings: several spaces, closing hashes, setext; Arabic after frontmatter.
        assert_eq!(
            marked("---\nid: X\n---\n###   Three ##\nSetext\n===\n## عنوان\n"),
            vec![
                m(Heading, "###   Three ##", &["###   ", " ##"]),
                m(Heading, "Setext\n===", &["==="]),
                m(Heading, "## عنوان", &["## "]),
            ]
        );
        // Links: brackets, an aliased link hides its path, embeds.
        assert_eq!(
            marked("[[Plan]] [[Plan#^a1|the plan]] ![[img.png]] 😀[[x]]\n"),
            vec![
                m(WikiLink, "[[Plan]]", &["[[", "]]"]),
                m(WikiLink, "[[Plan#^a1|the plan]]", &["[[Plan#^a1|", "]]"]),
                m(Embed, "![[img.png]]", &["![[", "]]"]),
                m(WikiLink, "[[x]]", &["[[", "]]"]),
            ]
        );
        // Code and delimiters inside a link path are never styled.
        assert_eq!(marked("`**no**` a_b_c\n"), vec![]);
        assert_eq!(
            marked("[[*draft*]]\n"),
            vec![m(WikiLink, "[[*draft*]]", &["[[", "]]"])]
        );
    }

    #[test]
    fn utf16_map_round_trips() {
        let text = "أ😀b";
        let m = Utf16Map::new(text);
        assert_eq!(m.at(text.len()), 4);
        assert_eq!(m.byte_of(0), 0);
        assert_eq!(m.byte_of(1), 2);
        assert_eq!(m.byte_of(3), 6);
        assert_eq!(m.byte_of(4), 7);
        assert_eq!(m.byte_of(99), 7);
    }
}
