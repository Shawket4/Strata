//! Body syntax: wikilinks, embeds, inline tags, headings, blocks and block IDs, with byte
//! spans. Code blocks, inline code and math are skipped (links and tags there are text).

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::line::trim_end_range;
use crate::wikilink::{self, WikiLink};

/// An inline `#tag`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InlineTag {
    /// Span including `#`.
    pub span: Range<usize>,
    /// Tag name without `#` (nested tags keep `/`: `project/strata`).
    pub name: String,
}

/// A heading (ATX `## x` or setext).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Heading {
    /// 1–6.
    pub level: u8,
    /// Heading text (markers and closing `#`s removed, trimmed).
    pub text: String,
    /// Span of the heading source (without the final line terminator).
    pub span: Range<usize>,
    /// Texts of the enclosing headings followed by this heading's text.
    pub path: Vec<String>,
    /// The heading's markdown markers: an ATX heading's opening `#`s with the whitespace
    /// around them and its closing `#`s with the whitespace before them; a setext heading's
    /// underline line. Sorted, never overlapping the text.
    pub markers: Vec<Range<usize>>,
}

/// An inline style (`**strong**`, `*emphasis*`, `~~strike~~`, `==mark==`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InlineStyle {
    /// `**x**` / `__x__`.
    Strong,
    /// `*x*` / `_x_`.
    Emphasis,
    /// `~~x~~` / `~x~`.
    Strikethrough,
    /// `==x==` (Obsidian highlight).
    Mark,
}

/// A styled run with its exact delimiter ranges, as the `CommonMark` parser matched them
/// (`***x***` is an emphasis around a strong run: `*` + `**` … `**` + `*`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InlineSpan {
    /// Style.
    pub style: InlineStyle,
    /// The whole run, delimiters included.
    pub span: Range<usize>,
    /// The opening delimiter.
    pub open: Range<usize>,
    /// The closing delimiter.
    pub close: Range<usize>,
}

/// The kind of a citable block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// A paragraph.
    Paragraph,
    /// A heading.
    Heading,
    /// One list item (its own lines, without nested lists).
    ListItem,
    /// A whole blockquote or callout.
    BlockQuote,
    /// A fenced or indented code block.
    CodeBlock,
    /// A table.
    Table,
    /// An HTML block.
    Html,
    /// A footnote definition.
    Footnote,
}

impl BlockKind {
    /// Whether a block ID for this kind goes on its own line after the block (Obsidian's rule
    /// for structured blocks) rather than at the end of its last line.
    pub fn id_on_own_line(self) -> bool {
        matches!(
            self,
            Self::BlockQuote | Self::CodeBlock | Self::Table | Self::Html
        )
    }
}

/// A block ID `^id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId {
    /// The ID without `^`.
    pub id: String,
    /// Span of `^id`.
    pub span: Range<usize>,
}

/// A block: the unit that `[[Note#^id]]` links cite.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    /// Kind.
    pub kind: BlockKind,
    /// Span of the block content (trailing whitespace excluded; for list items, nested lists
    /// excluded).
    pub span: Range<usize>,
    /// Block ID, at the end of the block or on a standalone line right after it.
    pub id: Option<BlockId>,
    /// Texts of the headings enclosing the block.
    pub heading_path: Vec<String>,
}

/// Everything extracted from a body in one pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyAnalysis {
    /// Wikilinks and embeds in source order.
    pub links: Vec<WikiLink>,
    /// Inline tags in source order.
    pub tags: Vec<InlineTag>,
    /// Headings in source order.
    pub headings: Vec<Heading>,
    /// Blocks in source order.
    pub blocks: Vec<Block>,
    /// Spans where markdown syntax is inert (code blocks, inline code, math), sorted.
    pub code_spans: Vec<Range<usize>>,
    /// Styled runs (strong, emphasis, strikethrough, mark) outside code, sorted by start then
    /// by end descending (outer runs first).
    pub inline: Vec<InlineSpan>,
}

impl BodyAnalysis {
    /// The block with the given ID.
    pub fn block_by_id(&self, id: &str) -> Option<&Block> {
        self.blocks
            .iter()
            .find(|b| b.id.as_ref().is_some_and(|b| b.id == id))
    }

    /// Whether byte `pos` is inside code or math.
    pub fn in_code(&self, pos: usize) -> bool {
        in_ranges(&self.code_spans, pos)
    }
}

fn in_ranges(ranges: &[Range<usize>], pos: usize) -> bool {
    let i = ranges.partition_point(|r| r.end <= pos);
    ranges.get(i).is_some_and(|r| r.start <= pos)
}

pub(crate) fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
}

/// Analyses a note body (the text after the frontmatter).
pub fn analyze(body: &str) -> BodyAnalysis {
    let mut s = collect_guarded(body);
    s.code_spans.sort_by_key(|r| r.start);
    let code_spans = merge(s.code_spans);
    let links = wikilink::scan(body, &code_spans);
    let mut skip: Vec<Range<usize>> = code_spans.clone();
    skip.extend(links.iter().map(|l| l.span.clone()));
    skip.sort_by_key(|r| r.start);
    let skip = merge(skip);
    let tags = scan_tags(body, &skip);
    let mut inline = s.inline;
    inline.extend(scan_marks(body, &skip));
    inline.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(b.span.end.cmp(&a.span.end))
    });
    BodyAnalysis {
        links,
        tags,
        headings: s.headings,
        blocks: s.blocks,
        code_spans,
        inline,
    }
}

/// `==mark==` runs (not part of `CommonMark`): on one line, the text between the delimiters is
/// not empty and neither starts nor ends with whitespace, and no delimiter is inside `skip`
/// (code, links). Runs are matched left to right.
fn scan_marks(body: &str, skip: &[Range<usize>]) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let mut base = 0;
    for (line, eol) in markdown_lines(body) {
        let mut from = 0;
        while let Some(open) = line[from..].find("==").map(|i| from + i) {
            let content_start = open + 2;
            let Some(close) = line[content_start..]
                .find("==")
                .map(|i| content_start + i)
            else {
                break;
            };
            let content = &line[content_start..close];
            let (o, c) = (base + open..base + content_start, base + close..base + close + 2);
            let free = |r: &Range<usize>| !skip.iter().any(|k| r.start < k.end && k.start < r.end);
            if !content.is_empty()
                && !content.starts_with(char::is_whitespace)
                && !content.ends_with(char::is_whitespace)
                && !content.starts_with('=')
                && free(&o)
                && free(&c)
            {
                out.push(InlineSpan {
                    style: InlineStyle::Mark,
                    span: o.start..c.end,
                    open: o,
                    close: c,
                });
                from = close + 2;
            } else {
                from = open + 1;
            }
        }
        base += line.len() + eol.len();
    }
    out
}

/// Wikilinks and embeds outside code.
pub fn links(body: &str) -> Vec<WikiLink> {
    analyze(body).links
}

/// Inline tags outside code.
pub fn tags(body: &str) -> Vec<InlineTag> {
    analyze(body).tags
}

/// Runs the `CommonMark` parser defensively.
///
/// `pulldown-cmark` 0.13.4 panics when a list item that starts with a link reference
/// definition is followed by certain whitespace-only lines (found by fuzzing, e.g.
/// `"- [a]: b\n        "`). A whitespace-only line is a blank line to `CommonMark`, so the parser
/// is given a copy with those lines emptied (and vertical tab / form feed, which `CommonMark`
/// treats as whitespace, turned into spaces); every range it reports is mapped back to the
/// original text. Should the parser still panic on some other input, the body is treated as
/// plain text (no code spans, headings or blocks) instead of taking the caller down.
fn collect_guarded(body: &str) -> Structure {
    let prepared = Prepared::new(body);
    std::panic::catch_unwind(|| Structure::collect(body, &prepared)).unwrap_or_default()
}

/// Lines as `CommonMark` sees them: ended by `\r\n`, `\n` or a lone `\r`.
fn markdown_lines(text: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let (content, eol_len) = match rest.find(['\r', '\n']) {
            Some(i) if rest[i..].starts_with("\r\n") => (&rest[..i], 2),
            Some(i) => (&rest[..i], 1),
            None => (rest, 0),
        };
        let eol = &rest[content.len()..content.len() + eol_len];
        rest = &rest[content.len() + eol_len..];
        Some((content, eol))
    })
}

/// The text given to the markdown parser and the map back to the original offsets.
struct Prepared<'a> {
    text: std::borrow::Cow<'a, str>,
    /// (offset in `text`, bytes removed from the original before that offset), ascending.
    removed: Vec<(usize, usize)>,
}

impl<'a> Prepared<'a> {
    fn new(body: &'a str) -> Self {
        let blank = |l: &str| {
            !l.is_empty()
                && l.chars()
                    .all(|c| matches!(c, ' ' | '\t' | '\u{b}' | '\u{c}'))
        };
        let needs_work =
            body.contains(['\u{b}', '\u{c}']) || markdown_lines(body).any(|(l, _)| blank(l));
        if !needs_work {
            return Self {
                text: body.into(),
                removed: Vec::new(),
            };
        }
        let mut text = String::with_capacity(body.len());
        let mut removed = Vec::new();
        let mut total = 0;
        for (content, eol) in markdown_lines(body) {
            if blank(content) {
                total += content.len();
                removed.push((text.len(), total));
            } else {
                text.extend(content.chars().map(|c| {
                    if matches!(c, '\u{b}' | '\u{c}') {
                        ' '
                    } else {
                        c
                    }
                }));
            }
            text.push_str(eol);
        }
        Self {
            text: text.into(),
            removed,
        }
    }

    /// Maps an offset in the parsed text to the original (a position at an emptied line maps
    /// to that line's start).
    fn map(&self, pos: usize) -> usize {
        let i = self.removed.partition_point(|(at, _)| *at < pos);
        pos + i.checked_sub(1).map_or(0, |j| self.removed[j].1)
    }

    fn map_range(&self, r: Range<usize>) -> Range<usize> {
        self.map(r.start)..self.map(r.end)
    }
}

fn merge(ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Item { block: Option<usize> },
    Quote,
    Footnote,
    Other,
}

#[derive(Default)]
struct Structure {
    code_spans: Vec<Range<usize>>,
    headings: Vec<Heading>,
    blocks: Vec<Block>,
    inline: Vec<InlineSpan>,
}

/// A styled run whose end the parser has not reached yet: its range, and the extent of the
/// events inside it so far (their first start and last end are the delimiters' inner edges).
struct OpenStyle {
    style: InlineStyle,
    range: Range<usize>,
    first: Option<usize>,
    last: usize,
}

impl Structure {
    fn collect(body: &str, prepared: &Prepared<'_>) -> Self {
        let mut s = Self::default();
        let mut stack: Vec<Container> = Vec::new();
        let mut path: Vec<(u8, String)> = Vec::new();
        let parser = Parser::new_ext(&prepared.text, markdown_options()).into_offset_iter();
        let mut styles: Vec<OpenStyle> = Vec::new();
        for (event, range) in parser {
            let range = prepared.map_range(range);
            let style = match &event {
                Event::Start(Tag::Strong) => Some(InlineStyle::Strong),
                Event::Start(Tag::Emphasis) => Some(InlineStyle::Emphasis),
                Event::Start(Tag::Strikethrough) => Some(InlineStyle::Strikethrough),
                _ => None,
            };
            if let Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough) = event {
                if let Some(open) = styles.pop() {
                    s.close_style(open);
                }
                continue;
            }
            for open in &mut styles {
                open.first.get_or_insert(range.start);
                open.last = open.last.max(range.end);
            }
            if let Some(style) = style {
                styles.push(OpenStyle {
                    style,
                    last: range.start,
                    range,
                    first: None,
                });
                continue;
            }
            let nested = stack.iter().any(|c| {
                matches!(
                    c,
                    Container::Item { .. } | Container::Quote | Container::Footnote
                )
            });
            match event {
                Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                    s.code_spans.push(range);
                }
                Event::Start(tag) => s.start(body, &tag, range, nested, &mut stack, &mut path),
                Event::End(end) => {
                    if matches!(
                        end,
                        TagEnd::Item
                            | TagEnd::BlockQuote(_)
                            | TagEnd::FootnoteDefinition
                            | TagEnd::List(_)
                    ) {
                        stack.pop();
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn close_style(&mut self, open: OpenStyle) {
        let OpenStyle {
            style,
            range,
            first,
            last,
        } = open;
        let Some(first) = first else { return };
        if first <= range.start || last >= range.end || first > last {
            return;
        }
        self.inline.push(InlineSpan {
            style,
            open: range.start..first,
            close: last..range.end,
            span: range,
        });
    }

    fn start(
        &mut self,
        body: &str,
        tag: &Tag<'_>,
        range: Range<usize>,
        nested: bool,
        stack: &mut Vec<Container>,
        path: &mut Vec<(u8, String)>,
    ) {
        match tag {
            Tag::CodeBlock(_) => {
                self.code_spans.push(range.clone());
                if !nested {
                    self.push_block(body, BlockKind::CodeBlock, range, names(path));
                }
            }
            Tag::Heading { level, .. } => {
                let level = *level as u8;
                let span = trim_end_range(body, range);
                let text = heading_text(&body[span.clone()]);
                if !nested {
                    while path.last().is_some_and(|(l, _)| *l >= level) {
                        path.pop();
                    }
                    let parent = names(path);
                    path.push((level, text.clone()));
                    let mut full = parent.clone();
                    full.push(text.clone());
                    self.headings.push(Heading {
                        level,
                        text,
                        markers: heading_markers(body, &span),
                        span: span.clone(),
                        path: full,
                    });
                    self.push_block(body, BlockKind::Heading, span, parent);
                }
            }
            Tag::Paragraph => {
                if !nested {
                    self.paragraph(body, range, names(path));
                }
            }
            Tag::Item => {
                let quoted = stack
                    .iter()
                    .any(|c| matches!(c, Container::Quote | Container::Footnote));
                let block = (!quoted).then(|| {
                    self.push_block(body, BlockKind::ListItem, range, names(path));
                    self.blocks.len() - 1
                });
                stack.push(Container::Item { block });
            }
            Tag::List(_) => {
                // A nested list ends its parent item's own content.
                if let Some(Container::Item { block: Some(i) }) = stack.last().copied() {
                    self.truncate_block(body, i, range.start);
                }
                stack.push(Container::Other);
            }
            Tag::BlockQuote(_) => {
                if !nested {
                    self.push_block(body, BlockKind::BlockQuote, range, names(path));
                }
                stack.push(Container::Quote);
            }
            Tag::FootnoteDefinition(_) => {
                if !nested {
                    self.push_block(body, BlockKind::Footnote, range, names(path));
                }
                stack.push(Container::Footnote);
            }
            Tag::Table(_) if !nested => {
                self.push_block(body, BlockKind::Table, range, names(path));
            }
            Tag::HtmlBlock if !nested => {
                self.push_block(body, BlockKind::Html, range, names(path));
            }
            _ => {}
        }
    }

    fn paragraph(&mut self, body: &str, range: Range<usize>, heading_path: Vec<String>) {
        let span = trim_end_range(body, range);
        let text = &body[span.clone()];
        // A paragraph that is only `^id` labels the previous block (Obsidian's rule for
        // tables, quotes, code and lists).
        if let Some(id) = standalone_block_id(text)
            && let Some(prev) = self.blocks.last_mut()
            && prev.id.is_none()
            && prev.kind != BlockKind::Heading
        {
            prev.id = Some(BlockId {
                id: id.to_owned(),
                span,
            });
            return;
        }
        self.push_block(body, BlockKind::Paragraph, span, heading_path);
    }

    fn push_block(
        &mut self,
        body: &str,
        kind: BlockKind,
        range: Range<usize>,
        heading_path: Vec<String>,
    ) {
        let span = trim_end_range(body, range);
        let id = if matches!(
            kind,
            BlockKind::Paragraph | BlockKind::ListItem | BlockKind::Footnote
        ) {
            trailing_block_id(body, &span)
        } else {
            None
        };
        self.blocks.push(Block {
            kind,
            span,
            id,
            heading_path,
        });
    }

    fn truncate_block(&mut self, body: &str, index: usize, end: usize) {
        if let Some(block) = self.blocks.get_mut(index)
            && end < block.span.end
        {
            block.span = trim_end_range(body, block.span.start..end);
            block.id = trailing_block_id(body, &block.span);
        }
    }
}

fn names(path: &[(u8, String)]) -> Vec<String> {
    path.iter().map(|(_, t)| t.clone()).collect()
}

fn is_block_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

/// `^id` alone.
fn standalone_block_id(text: &str) -> Option<&str> {
    let id = text.strip_prefix('^')?;
    (!id.is_empty() && id.chars().all(is_block_id_char)).then_some(id)
}

/// A `^id` at the end of the last line of `span`, preceded by whitespace.
pub(crate) fn trailing_block_id(text: &str, span: &Range<usize>) -> Option<BlockId> {
    let slice = &text[span.clone()];
    let trimmed = slice.trim_end_matches([' ', '\t']);
    let caret = trimmed.rfind('^')?;
    let id = &trimmed[caret + 1..];
    if id.is_empty() || !id.chars().all(is_block_id_char) {
        return None;
    }
    let before = trimmed[..caret].chars().next_back();
    if caret > 0 && !matches!(before, Some(' ' | '\t')) {
        return None;
    }
    let start = span.start + caret;
    Some(BlockId {
        id: id.to_owned(),
        span: start..start + 1 + id.len(),
    })
}

/// The marker ranges of the heading at `span` (see [`Heading::markers`]).
fn heading_markers(body: &str, span: &Range<usize>) -> Vec<Range<usize>> {
    let src = &body[span.clone()];
    let first_line_len = src.find(['\r', '\n']).unwrap_or(src.len());
    let line = &src[..first_line_len];
    let indent = line.len() - line.trim_start().len();
    let after_indent = &line[indent..];
    if !after_indent.starts_with('#') {
        // Setext: the underline is the last line.
        let start = src
            .trim_end_matches(['\r', '\n'])
            .rfind('\n')
            .map_or(src.len(), |i| i + 1);
        return (start < src.len())
            .then(|| span.start + start..span.end)
            .into_iter()
            .collect();
    }
    let hashes = after_indent.len() - after_indent.trim_start_matches('#').len();
    let rest = &after_indent[hashes..];
    let text_start = indent + hashes + (rest.len() - rest.trim_start().len());
    let mut out = vec![span.start..span.start + text_start];
    let tail = line[text_start..].trim_end();
    let without = tail.trim_end_matches('#');
    if without.len() < tail.len() && without.ends_with([' ', '\t']) {
        let close_start = text_start + without.trim_end().len();
        out.push(span.start + close_start..span.start + line.len());
    } else if tail.len() < line.len() - text_start {
        // Trailing whitespace after the text.
        out.push(span.start + text_start + tail.len()..span.start + line.len());
    }
    out.retain(|r| r.start < r.end);
    out
}

fn heading_text(src: &str) -> String {
    let first_line = src.lines().next().unwrap_or("");
    let t = first_line.trim_start();
    if t.starts_with('#') {
        let t = t.trim_start_matches('#');
        let t = t.trim();
        // Closing sequence: spaces then only `#`s.
        let without = t.trim_end_matches('#');
        if without.len() < t.len() && (without.is_empty() || without.ends_with([' ', '\t'])) {
            without.trim_end().to_owned()
        } else {
            t.to_owned()
        }
    } else {
        // Setext: every line except the underline, joined.
        let lines: Vec<&str> = src.lines().collect();
        let content = &lines[..lines.len().saturating_sub(1)];
        content
            .iter()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Characters allowed in a tag (after `#`): letters and digits of any script, combining
/// marks used by Arabic and Latin, `_`, `-`, `/`.
pub fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(c, '_' | '-' | '/')
        || matches!(c as u32,
            0x0300..=0x036F   // combining diacritics
            | 0x0610..=0x061A // Arabic signs
            | 0x064B..=0x065F // tashkeel
            | 0x0670          // superscript alef
            | 0x06D6..=0x06ED // Quranic marks
            | 0x200C..=0x200D // ZWNJ / ZWJ
        )
}

/// Whether `name` (without `#`) is a valid tag: tag characters only, not all digits.
pub fn is_valid_tag(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(is_tag_char)
        && !name
            .chars()
            .all(|c| c.is_ascii_digit() || c == '/' || c == '_' || c == '-')
}

fn scan_tags(text: &str, skip: &[Range<usize>]) -> Vec<InlineTag> {
    let mut out = Vec::new();
    let mut prev: Option<char> = None;
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if c == '#' && prev.is_none_or(char::is_whitespace) && !in_ranges(skip, i) {
            let start = i + 1;
            let mut end = start;
            while let Some(&(j, n)) = iter.peek() {
                if !is_tag_char(n) {
                    break;
                }
                end = j + n.len_utf8();
                iter.next();
            }
            let name = &text[start..end];
            if is_valid_tag(name) {
                out.push(InlineTag {
                    span: i..end,
                    name: name.to_owned(),
                });
            }
            prev = text[..end].chars().next_back();
            continue;
        }
        prev = Some(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_names(body: &str) -> Vec<String> {
        tags(body).into_iter().map(|t| t.name).collect()
    }

    #[test]
    fn tags_follow_obsidian_rules() {
        assert_eq!(tag_names("#a b #c/d e#f #123 #1a"), ["a", "c/d", "1a"]);
        assert_eq!(tag_names("# Heading #inhead\n#start"), ["inhead", "start"]);
        assert_eq!(tag_names("x #عربي و #مشروع/قديم."), ["عربي", "مشروع/قديم"]);
        assert_eq!(tag_names("`#code` #real"), ["real"]);
        assert_eq!(tag_names("```\n#fenced\n```\n#after"), ["after"]);
        assert_eq!(tag_names("[[Note#Heading]] #t"), ["t"]);
        assert_eq!(tag_names("http://x.com/#frag \\#esc"), Vec::<String>::new());
        assert_eq!(tag_names("#tag, #other!"), ["tag", "other"]);
        assert_eq!(tag_names("$#math$ #ok"), ["ok"]);
    }

    #[test]
    fn tag_spans() {
        let t = tags("a #b/c d");
        assert_eq!(
            t,
            vec![InlineTag {
                span: 2..6,
                name: "b/c".into()
            }]
        );
    }

    #[test]
    fn headings_and_paths() {
        let body = "# A\n\n## B ##\n\ntext\n\n### C\n\n## D\nSetext\n---\n";
        let h = analyze(body).headings;
        let got: Vec<_> = h
            .iter()
            .map(|h| (h.level, h.text.as_str(), h.path.join(">")))
            .collect();
        assert_eq!(
            got,
            [
                (1, "A", "A".to_owned()),
                (2, "B", "A>B".to_owned()),
                (3, "C", "A>B>C".to_owned()),
                (2, "D", "A>D".to_owned()),
                (2, "Setext", "A>Setext".to_owned()),
            ]
        );
        assert_eq!(&body[h[1].span.clone()], "## B ##");
        assert_eq!(heading_text("## C#"), "C#");
    }

    #[test]
    fn blocks_with_ids() {
        let body = "Para one ^p1\n\n- item a ^i1\n  - nested ^i2\n- item b\n\n> quote\n\n^q1\n\n| a |\n|---|\n| 1 |\n\n^t1\n";
        let a = analyze(body);
        let got: Vec<_> = a
            .blocks
            .iter()
            .map(|b| {
                (
                    b.kind,
                    &body[b.span.clone()],
                    b.id.as_ref().map(|i| i.id.as_str()),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (BlockKind::Paragraph, "Para one ^p1", Some("p1")),
                (BlockKind::ListItem, "- item a ^i1", Some("i1")),
                (BlockKind::ListItem, "- nested ^i2", Some("i2")),
                (BlockKind::ListItem, "- item b", None),
                (BlockKind::BlockQuote, "> quote", Some("q1")),
                (BlockKind::Table, "| a |\n|---|\n| 1 |", Some("t1")),
            ]
        );
        let p1 = a.block_by_id("p1").map(|b| b.id.clone());
        assert_eq!(
            p1,
            Some(Some(BlockId {
                id: "p1".into(),
                span: 9..12
            }))
        );
    }

    #[test]
    fn block_id_needs_whitespace_before() {
        let a = analyze("price^2\n");
        assert_eq!(a.blocks[0].id, None);
        let a = analyze("x ^a_b\n");
        assert_eq!(a.blocks[0].id, None);
    }

    #[test]
    fn code_is_inert() {
        let body = "```md\n[[fake]] #fake ^fake\n```\n\n    [[indented]]\n\nreal [[yes]] `[[no]]` $[[m]]$\n";
        let a = analyze(body);
        let paths: Vec<_> = a.links.iter().map(|l| l.path.as_str()).collect();
        assert_eq!(paths, ["yes"]);
        assert!(a.tags.is_empty());
        assert!(a.in_code(8));
        assert!(!a.in_code(body.find("real").unwrap_or(0)));
    }

    #[test]
    fn prepared_text_maps_offsets_back() {
        let body = "a\n  \t\nb\n\n   \r\nc\u{b}d";
        let p = Prepared::new(body);
        assert_eq!(p.text, "a\n\nb\n\n\r\nc d");
        // Positions at an emptied line map to the start of that line.
        assert_eq!(p.map(2), 2);
        assert_eq!(p.map(6), 9);
        for (parsed, orig) in [
            (0, 0),
            (1, 1),
            (3, 6),
            (4, 7),
            (5, 8),
            (7, 13),
            (8, 14),
            (9, 15),
            (10, 16),
        ] {
            assert_eq!(p.map(parsed), orig, "{parsed}");
            let expect = if &body[orig..=orig] == "\u{b}" {
                " "
            } else {
                &body[orig..=orig]
            };
            assert_eq!(expect, &p.text[parsed..=parsed], "{parsed}");
        }
        assert!(Prepared::new("plain\n").removed.is_empty());
    }

    #[test]
    fn whitespace_lines_after_link_definitions() {
        // Fuzz regressions: pulldown-cmark 0.13.4 panics on these unless blank lines are emptied.
        for body in [
            "- [a]: b\n        \n[[x]]",
            "- [\0]::\n\t\t\n[[x]]",
            "1. [a]:x\n\u{b}\n[[x]]",
            "+ [\0]::\r\t\t\r[[x]]",
        ] {
            let a = analyze(body);
            assert_eq!(a.links.len(), 1, "{body:?}");
            assert_eq!(a.blocks.len(), 2, "{body:?}");
        }
    }

    #[test]
    fn parser_panic_inputs_are_survived() {
        // Fuzz regression: pulldown-cmark 0.13.4 panics on this input.
        let a = analyze("- [n]:`\n\u{b}");
        assert_eq!(a.blocks.len(), 1);
        let a = analyze("- [n]: `\n\u{c}[[x]]");
        assert_eq!(a.links.len(), 1);
    }

    #[test]
    fn heading_paths_on_blocks() {
        let body = "## S\n\ntext\n";
        let a = analyze(body);
        assert_eq!(a.blocks[1].heading_path, ["S"]);
        assert_eq!(a.blocks[0].kind, BlockKind::Heading);
        assert!(a.blocks[0].heading_path.is_empty());
    }

    fn styles(body: &str) -> Vec<(InlineStyle, &str, &str, &str)> {
        analyze(body)
            .inline
            .into_iter()
            .map(|i| {
                (
                    i.style,
                    &body[i.span.clone()],
                    &body[i.open.clone()],
                    &body[i.close.clone()],
                )
            })
            .collect()
    }

    #[test]
    fn inline_styles_carry_exact_delimiters() {
        use InlineStyle::{Emphasis, Mark, Strikethrough, Strong};
        assert_eq!(
            styles("a ***both*** b\n"),
            vec![
                (Emphasis, "***both***", "*", "*"),
                (Strong, "**both**", "**", "**")
            ]
        );
        assert_eq!(
            styles("**bold *it* more** and __u__ _e_\n"),
            vec![
                (Strong, "**bold *it* more**", "**", "**"),
                (Emphasis, "*it*", "*", "*"),
                (Strong, "__u__", "__", "__"),
                (Emphasis, "_e_", "_", "_"),
            ]
        );
        assert_eq!(
            styles("~~gone~~ ~one~ ==hi== x==y==z\n"),
            vec![
                (Strikethrough, "~~gone~~", "~~", "~~"),
                (Strikethrough, "~one~", "~", "~"),
                (Mark, "==hi==", "==", "=="),
                (Mark, "==y==", "==", "=="),
            ]
        );
        // Code, links and unmatched delimiters are not styled.
        assert_eq!(styles("`**no**` [[a==b==c]] ** x ** == y ==\n"), vec![]);
        // A run around a link keeps it; Arabic text keeps byte offsets.
        assert_eq!(
            styles("**[[Ahmed]]** و **مهم**\n"),
            vec![
                (Strong, "**[[Ahmed]]**", "**", "**"),
                (Strong, "**مهم**", "**", "**"),
            ]
        );
    }

    #[test]
    fn heading_markers_are_exact() {
        let markers = |body: &str| -> Vec<Vec<String>> {
            analyze(body)
                .headings
                .iter()
                .map(|h| h.markers.iter().map(|m| body[m.clone()].to_owned()).collect())
                .collect()
        };
        assert_eq!(markers("# One\n"), vec![vec!["# ".to_owned()]]);
        assert_eq!(markers("###   Three\n"), vec![vec!["###   ".to_owned()]]);
        assert_eq!(
            markers("## Closed ##\n"),
            vec![vec!["## ".to_owned(), " ##".to_owned()]]
        );
        assert_eq!(markers("  ## Indented\n"), vec![vec!["## ".to_owned()]]);
        assert_eq!(
            markers("Setext\n===\n"),
            vec![vec!["===".to_owned()]]
        );
        assert_eq!(markers("#\n"), vec![vec!["#".to_owned()]]);
        let h = &analyze("x\n\n## عنوان\n").headings[0];
        assert_eq!((h.text.as_str(), h.markers.clone()), ("عنوان", vec![3..6]));
    }
}
