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
        matches!(self, Self::BlockQuote | Self::CodeBlock | Self::Table | Self::Html)
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
    let mut s = Structure::collect(body);
    s.code_spans.sort_by_key(|r| r.start);
    let code_spans = merge(s.code_spans);
    let links = wikilink::scan(body, &code_spans);
    let mut skip: Vec<Range<usize>> = code_spans.clone();
    skip.extend(links.iter().map(|l| l.span.clone()));
    skip.sort_by_key(|r| r.start);
    let tags = scan_tags(body, &merge(skip));
    BodyAnalysis {
        links,
        tags,
        headings: s.headings,
        blocks: s.blocks,
        code_spans,
    }
}

/// Wikilinks and embeds outside code.
pub fn links(body: &str) -> Vec<WikiLink> {
    analyze(body).links
}

/// Inline tags outside code.
pub fn tags(body: &str) -> Vec<InlineTag> {
    analyze(body).tags
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
}

impl Structure {
    fn collect(body: &str) -> Self {
        let mut s = Self::default();
        let mut stack: Vec<Container> = Vec::new();
        let mut path: Vec<(u8, String)> = Vec::new();
        let parser = Parser::new_ext(body, markdown_options()).into_offset_iter();
        for (event, range) in parser {
            let nested = stack
                .iter()
                .any(|c| matches!(c, Container::Item { .. } | Container::Quote | Container::Footnote));
            match event {
                Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                    s.code_spans.push(range);
                }
                Event::Start(tag) => s.start(body, tag, range, nested, &mut stack, &mut path),
                Event::End(end) => {
                    if matches!(
                        end,
                        TagEnd::Item | TagEnd::BlockQuote(_) | TagEnd::FootnoteDefinition | TagEnd::List(_)
                    ) {
                        stack.pop();
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn start(
        &mut self,
        body: &str,
        tag: Tag<'_>,
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
                let level = level as u8;
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

    fn push_block(&mut self, body: &str, kind: BlockKind, range: Range<usize>, heading_path: Vec<String>) {
        let span = trim_end_range(body, range);
        let id = if matches!(kind, BlockKind::Paragraph | BlockKind::ListItem | BlockKind::Footnote) {
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
        content.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ")
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
        && !name.chars().all(|c| c.is_ascii_digit() || c == '/' || c == '_' || c == '-')
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
        assert_eq!(t, vec![InlineTag { span: 2..6, name: "b/c".into() }]);
    }

    #[test]
    fn headings_and_paths() {
        let body = "# A\n\n## B ##\n\ntext\n\n### C\n\n## D\nSetext\n---\n";
        let h = analyze(body).headings;
        let got: Vec<_> = h.iter().map(|h| (h.level, h.text.as_str(), h.path.join(">"))).collect();
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
            .map(|b| (b.kind, &body[b.span.clone()], b.id.as_ref().map(|i| i.id.as_str())))
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
        assert_eq!(p1, Some(Some(BlockId { id: "p1".into(), span: 9..12 })));
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
    fn heading_paths_on_blocks() {
        let body = "## S\n\ntext\n";
        let a = analyze(body);
        assert_eq!(a.blocks[1].heading_path, ["S"]);
        assert_eq!(a.blocks[0].kind, BlockKind::Heading);
        assert!(a.blocks[0].heading_path.is_empty());
    }
}
