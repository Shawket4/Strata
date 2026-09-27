//! Highlight spans for the editor (PLAN §12.1 `format/`: "render hints, highlight spans for
//! the editor"). Offsets are **UTF-16 code units** so Dart can use them on its strings
//! directly, with no index arithmetic on the Dart side.

use vault_format::Document;

/// What a span is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HintKind {
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
}

/// One span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// Kind.
    pub kind: HintKind,
    /// Start (UTF-16 units, inclusive).
    pub start: u32,
    /// End (UTF-16 units, exclusive).
    pub end: u32,
}

/// Maps byte offsets of `text` to UTF-16 offsets.
struct Utf16Map<'a> {
    text: &'a str,
}

impl Utf16Map<'_> {
    fn at(&self, byte: usize) -> u32 {
        let byte = byte.min(self.text.len());
        let units: usize = self.text[..byte].chars().map(char::len_utf16).sum();
        u32::try_from(units).unwrap_or(u32::MAX)
    }
}

/// Highlight spans of a whole note, sorted by start then kind.
pub fn editor_hints(content: &str) -> Vec<Hint> {
    let doc = Document::parse(content);
    let map = Utf16Map { text: content };
    let off = doc.body_offset();
    let mut out = Vec::new();
    let mut push = |kind, start: usize, end: usize| {
        out.push(Hint {
            kind,
            start: map.at(start),
            end: map.at(end),
        });
    };
    if doc.frontmatter().is_some() && off > 0 {
        push(HintKind::Frontmatter, 0, off);
    }
    let a = doc.analyze_body();
    for h in &a.headings {
        push(HintKind::Heading, off + h.span.start, off + h.span.end);
    }
    for l in &a.links {
        let kind = if l.embed {
            HintKind::Embed
        } else {
            HintKind::WikiLink
        };
        push(kind, off + l.span.start, off + l.span.end);
    }
    for t in &a.tags {
        push(HintKind::Tag, off + t.span.start, off + t.span.end);
    }
    for b in &a.blocks {
        if let Some(id) = &b.id {
            push(HintKind::BlockId, off + id.span.start, off + id.span.end);
        }
    }
    for c in &a.code_spans {
        push(HintKind::Code, off + c.start, off + c.end);
    }
    for t in vault_format::tasks::extract_tasks(doc.body()) {
        push(
            HintKind::TaskLine,
            off + t.line_span.start,
            off + t.line_span.end,
        );
    }
    out.sort_by_key(|h| (h.start, h.end, h.kind as u8));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_are_utf16() {
        // "أ" is 2 bytes / 1 UTF-16 unit; "😀" is 4 bytes / 2 units.
        let hints = editor_hints("أ😀 [[Note]] #tag\n");
        assert_eq!(
            hints,
            vec![
                Hint {
                    kind: HintKind::WikiLink,
                    start: 4,
                    end: 12
                },
                Hint {
                    kind: HintKind::Tag,
                    start: 13,
                    end: 17
                },
            ]
        );
    }

    #[test]
    fn frontmatter_heading_task_and_block_id() {
        let text = "---\nid: X\n---\n# Title\n- [ ] do ^t-1\n";
        let kinds: Vec<(HintKind, u32, u32)> = editor_hints(text)
            .into_iter()
            .map(|h| (h.kind, h.start, h.end))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (HintKind::Frontmatter, 0, 14),
                (HintKind::Heading, 14, 21),
                (HintKind::TaskLine, 22, 36),
                (HintKind::BlockId, 32, 36),
            ]
        );
    }
}
