//! Obsidian wikilinks and embeds: `[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`,
//! `[[Note#^block]]`, `![[image.png]]`.

use std::ops::Range;

/// The part after `#` in a wikilink.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// `#Heading` (nested headings are `#H1#H2`; the text is kept as written, without the
    /// first `#`).
    Heading(String),
    /// `#^blockid` (the ID without `^`).
    Block(String),
}

/// One wikilink or embed with the spans of its parts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WikiLink {
    /// The whole link including `!`, `[[` and `]]`.
    pub span: Range<usize>,
    /// `![[…]]`
    pub embed: bool,
    /// The link path as written (may be empty for `[[#Heading]]`, may include folders and an
    /// extension).
    pub path: String,
    /// Span of `path`.
    pub path_span: Range<usize>,
    /// The anchor, if any.
    pub anchor: Option<Anchor>,
    /// Span of the anchor text after `#` (including `^` for block anchors).
    pub anchor_span: Option<Range<usize>>,
    /// Display alias after `|`.
    pub alias: Option<String>,
    /// Span of the alias text.
    pub alias_span: Option<Range<usize>>,
    /// The alias separator was written `\|` (inside a table).
    pub escaped_pipe: bool,
}

impl WikiLink {
    /// Parses a string that must consist of exactly one wikilink (e.g. a frontmatter value
    /// `"[[Note|alias]]"`). Surrounding whitespace is not allowed.
    pub fn parse_exact(s: &str) -> Option<Self> {
        let link = parse_at(s, 0)?;
        (link.span == (0..s.len())).then_some(link)
    }

    /// The link target without `.md` (the name used for resolution).
    pub fn target(&self) -> &str {
        self.path.trim()
    }

    /// Renders the link from its parts.
    pub fn to_markdown(&self) -> String {
        render(self.embed, &self.path, self.anchor.as_ref(), self.alias.as_deref(), self.escaped_pipe)
    }

    /// The same link pointing at `new_path`, keeping embed, anchor, alias and pipe style.
    pub fn with_path(&self, new_path: &str) -> String {
        render(self.embed, new_path, self.anchor.as_ref(), self.alias.as_deref(), self.escaped_pipe)
    }
}

fn render(embed: bool, path: &str, anchor: Option<&Anchor>, alias: Option<&str>, escaped: bool) -> String {
    let mut out = String::with_capacity(path.len() + 8);
    if embed {
        out.push('!');
    }
    out.push_str("[[");
    out.push_str(path);
    match anchor {
        Some(Anchor::Heading(h)) => {
            out.push('#');
            out.push_str(h);
        }
        Some(Anchor::Block(b)) => {
            out.push_str("#^");
            out.push_str(b);
        }
        None => {}
    }
    if let Some(alias) = alias {
        out.push_str(if escaped { "\\|" } else { "|" });
        out.push_str(alias);
    }
    out.push_str("]]");
    out
}

/// Tries to parse a wikilink whose `[[` (or `![[`) starts at `start`.
pub(crate) fn parse_at(text: &str, start: usize) -> Option<WikiLink> {
    let bytes = text.as_bytes();
    let (embed, open) = match bytes.get(start) {
        Some(b'!') => (true, start + 1),
        Some(b'[') => (false, start),
        _ => return None,
    };
    if !text[open..].starts_with("[[") {
        return None;
    }
    let inner_start = open + 2;
    let rest = &text[inner_start..];
    let line_len = rest.find(['\n', '\r']).unwrap_or(rest.len());
    let close_rel = rest[..line_len].find("]]")?;
    let inner = &rest[..close_rel];
    if inner.trim().is_empty() || inner.contains("[[") || inner.starts_with('[') {
        return None;
    }
    let inner_end = inner_start + close_rel;
    // A `]` right after `]]` belongs to the link only if the alias contains `[`; Obsidian
    // closes at the first `]]`, so do we.
    let (target_end, alias_range, escaped_pipe) = match inner.find('|') {
        Some(p) if p > 0 && inner.as_bytes()[p - 1] == b'\\' => {
            (inner_start + p - 1, Some(inner_start + p + 1..inner_end), true)
        }
        Some(p) => (inner_start + p, Some(inner_start + p + 1..inner_end), false),
        None => (inner_end, None, false),
    };
    let target = &text[inner_start..target_end];
    if target.contains(['[', ']']) {
        return None;
    }
    let (path_span, anchor, anchor_span) = match target.find('#') {
        Some(h) => {
            let a_start = inner_start + h + 1;
            let a_text = &text[a_start..target_end];
            let anchor = match a_text.strip_prefix('^') {
                Some(block) => Anchor::Block(block.to_owned()),
                None => Anchor::Heading(a_text.to_owned()),
            };
            (inner_start..inner_start + h, Some(anchor), Some(a_start..target_end))
        }
        None => (inner_start..target_end, None, None),
    };
    if text[path_span.clone()].trim().is_empty() && anchor.is_none() {
        return None;
    }
    Some(WikiLink {
        span: start..inner_end + 2,
        embed,
        path: text[path_span.clone()].to_owned(),
        path_span,
        anchor,
        anchor_span,
        alias: alias_range.clone().map(|r| text[r].to_owned()),
        alias_span: alias_range,
        escaped_pipe,
    })
}

/// Finds every wikilink in `text` whose start is not inside one of `excluded` (sorted,
/// non-overlapping ranges). A `[[` preceded by a backslash is not a link.
pub(crate) fn scan(text: &str, excluded: &[Range<usize>]) -> Vec<WikiLink> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut pos = 0;
    let mut ex = 0;
    while let Some(rel) = text[pos..].find("[[") {
        let open = pos + rel;
        let start = if open > 0 && bytes[open - 1] == b'!' { open - 1 } else { open };
        while ex < excluded.len() && excluded[ex].end <= open {
            ex += 1;
        }
        if ex < excluded.len() && excluded[ex].start <= open {
            pos = excluded[ex].end.max(open + 2);
            continue;
        }
        let escaped = open > 0 && bytes[open - 1] == b'\\';
        if !escaped && let Some(link) = parse_at(text, start) {
            pos = link.span.end;
            out.push(link);
            continue;
        }
        pos = open + 1;
    }
    out
}

/// Every wikilink in `text` (no code awareness; use [`crate::body::analyze`] for bodies).
pub fn find_all(text: &str) -> Vec<WikiLink> {
    scan(text, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(s: &str) -> WikiLink {
        let links = find_all(s);
        assert_eq!(links.len(), 1, "{s}");
        links.into_iter().next().unwrap_or_else(|| unreachable!())
    }

    #[test]
    fn plain_link() {
        let l = one("see [[Note]] now");
        assert_eq!(l.span, 4..12);
        assert!(!l.embed);
        assert_eq!(l.path, "Note");
        assert_eq!(l.path_span, 6..10);
        assert_eq!(l.anchor, None);
        assert_eq!(l.alias, None);
    }

    #[test]
    fn alias_heading_block_embed() {
        let l = one("[[Note|the note]]");
        assert_eq!(l.alias.as_deref(), Some("the note"));
        assert_eq!(l.alias_span, Some(7..15));
        let l = one("[[Note#Intro#Sub]]");
        assert_eq!(l.anchor, Some(Anchor::Heading("Intro#Sub".into())));
        assert_eq!(l.anchor_span, Some(7..16));
        let l = one("x ![[Call 2026-09-12#^a1b2|c]]");
        assert!(l.embed);
        assert_eq!(l.span, 2..30);
        assert_eq!(l.path, "Call 2026-09-12");
        assert_eq!(l.anchor, Some(Anchor::Block("a1b2".into())));
        assert_eq!(l.alias.as_deref(), Some("c"));
        let l = one("![[image.png]]");
        assert_eq!(l.path, "image.png");
        let l = one("[[#Heading]]");
        assert_eq!(l.path, "");
        assert_eq!(l.anchor, Some(Anchor::Heading("Heading".into())));
    }

    #[test]
    fn escaped_pipe_in_tables() {
        let l = one("| [[Note\\|alias]] |");
        assert!(l.escaped_pipe);
        assert_eq!(l.path, "Note");
        assert_eq!(l.alias.as_deref(), Some("alias"));
        assert_eq!(l.to_markdown(), "[[Note\\|alias]]");
        assert_eq!(l.with_path("x/New"), "[[x/New\\|alias]]");
    }

    #[test]
    fn arabic_links() {
        let l = one("راجع [[أحمد سمير|أحمد]] اليوم");
        assert_eq!(l.path, "أحمد سمير");
        assert_eq!(l.alias.as_deref(), Some("أحمد"));
        assert_eq!(&"راجع [[أحمد سمير|أحمد]] اليوم"[l.span.clone()], "[[أحمد سمير|أحمد]]");
    }

    #[test]
    fn not_links() {
        for s in ["[[]]", "[[ ]]", "[[a\nb]]", "[[a", "\\[[x]]", "[[a[b]]", "[[|x]]", "[ [x]]"] {
            assert_eq!(find_all(s), vec![], "{s:?}");
        }
    }

    #[test]
    fn multiple_and_adjacent() {
        let links = find_all("[[a]][[b]] ![[c]]");
        let paths: Vec<_> = links.iter().map(|l| (l.path.as_str(), l.embed)).collect();
        assert_eq!(paths, [("a", false), ("b", false), ("c", true)]);
    }

    #[test]
    fn nested_bracket_start() {
        // `[[[x]]` — Obsidian treats `[[x]]` after the stray `[` as the link.
        let links = find_all("[[[x]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].span, 1..6);
    }

    #[test]
    fn parse_exact_requires_whole_string() {
        assert!(WikiLink::parse_exact("[[A]]").is_some());
        assert!(WikiLink::parse_exact(" [[A]]").is_none());
        assert!(WikiLink::parse_exact("[[A]] b").is_none());
        assert!(WikiLink::parse_exact("A").is_none());
    }

    #[test]
    fn excluded_ranges_skip_links() {
        let t = "`[[a]]` [[b]]";
        let links = scan(t, &[0..7]);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].path, "b");
    }
}
