//! A note file: optional frontmatter plus body, rendered back byte-exactly.

use crate::body::{self, BodyAnalysis};
use crate::frontmatter::{Frontmatter, FrontmatterError};
use crate::line::{self, LineEnding};
use crate::rewrite::{MoveSet, Rewrite};

const BOM: &str = "\u{feff}";

/// A parsed note file. Parsing never fails: text without a well-formed frontmatter block is
/// all body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    bom: bool,
    frontmatter: Option<Frontmatter>,
    body: String,
    body_offset: usize,
    eol: LineEnding,
}

impl Document {
    /// Splits `text` into frontmatter and body.
    ///
    /// Frontmatter is recognised when the file (after an optional BOM) starts with a line that
    /// is exactly `---` and a later line is exactly `---` (trailing spaces/tabs allowed on
    /// both, like Obsidian). Everything after the closing line is the body.
    pub fn parse(text: &str) -> Self {
        let (bom, rest) = match text.strip_prefix(BOM) {
            Some(r) => (true, r),
            None => (false, text),
        };
        let base = if bom { BOM.len() } else { 0 };
        let eol = LineEnding::detect(rest);
        let mut lines = line::lines(rest);
        if let Some(first) = lines.next()
            && is_delimiter(first.content)
            && !first.eol.is_empty()
        {
            let inner_start = first.full_range().end;
            for l in lines {
                if is_delimiter(l.content) {
                    let open = &rest[..inner_start];
                    let inner = &rest[inner_start..l.start];
                    let close = &rest[l.full_range()];
                    let body_start = l.full_range().end;
                    return Self {
                        bom,
                        frontmatter: Some(Frontmatter::from_parts(open, inner, close)),
                        body: rest[body_start..].to_owned(),
                        body_offset: base + body_start,
                        eol,
                    };
                }
            }
        }
        Self {
            bom,
            frontmatter: None,
            body: rest.to_owned(),
            body_offset: base,
            eol,
        }
    }

    /// The frontmatter, if the file has one.
    pub fn frontmatter(&self) -> Option<&Frontmatter> {
        self.frontmatter.as_ref()
    }

    /// The frontmatter for editing; an empty one is created (with the file's line ending) if
    /// the file has none. A created frontmatter that stays empty is not rendered.
    pub fn frontmatter_mut(&mut self) -> &mut Frontmatter {
        let eol = self.eol;
        self.frontmatter.get_or_insert_with(|| Frontmatter::new(eol))
    }

    /// The body text.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Replaces the body.
    pub fn set_body(&mut self, body: impl Into<String>) {
        self.body = body.into();
    }

    /// Byte offset of the body in the parsed file (add it to body spans to get file spans).
    pub fn body_offset(&self) -> usize {
        self.body_offset
    }

    /// The line ending of the file's first line.
    pub fn line_ending(&self) -> LineEnding {
        self.eol
    }

    /// Whether the file started with a UTF-8 byte order mark.
    pub fn has_bom(&self) -> bool {
        self.bom
    }

    /// Links, tags, headings and blocks of the body.
    pub fn analyze_body(&self) -> BodyAnalysis {
        body::analyze(&self.body)
    }

    /// Renders the file. An unmodified document renders to exactly the bytes it was parsed
    /// from; a modified frontmatter is re-emitted per [`Frontmatter::render`].
    pub fn render(&self) -> String {
        self.render_with(Frontmatter::render)
    }

    /// Renders with every known frontmatter key in canonical form and order.
    pub fn render_canonical(&self) -> String {
        self.render_with(Frontmatter::render_canonical)
    }

    fn render_with(&self, fm: impl Fn(&Frontmatter) -> String) -> String {
        let mut out = String::with_capacity(self.body.len() + 256);
        if self.bom {
            out.push_str(BOM);
        }
        if let Some(f) = &self.frontmatter {
            out.push_str(&fm(f));
        }
        out.push_str(&self.body);
        out
    }

    /// Applies a rename/move to this note (which was at `source` before the moves): body links
    /// and frontmatter link values. Returns the number of changed links.
    pub fn rewrite_links(&mut self, moves: &MoveSet<'_>, source: &str) -> Result<usize, FrontmatterError> {
        let mut changed = 0;
        if let Some(fm) = self.frontmatter.as_mut()
            && fm.error().is_none()
        {
            changed += moves.rewrite_frontmatter(fm, source)?;
        }
        let Rewrite { text, changed: n } = moves.rewrite_body(&self.body, source);
        if n > 0 {
            self.body = text;
        }
        Ok(changed + n)
    }
}

fn is_delimiter(line: &str) -> bool {
    line.trim_end_matches([' ', '\t']) == "---"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::PropertyValue;

    #[test]
    fn splits_frontmatter() {
        let d = Document::parse("---\nid: x\n---\nbody\n");
        assert_eq!(d.body(), "body\n");
        assert_eq!(d.body_offset(), 14);
        assert_eq!(d.frontmatter().map(Frontmatter::len), Some(1));
        assert_eq!(d.render(), "---\nid: x\n---\nbody\n");
    }

    #[test]
    fn no_frontmatter_cases() {
        for t in ["", "body", "---\nno close\n", "--- \n", "----\na: b\n----\n", "x\n---\na: b\n---\n", "---"] {
            let d = Document::parse(t);
            assert!(d.frontmatter().is_none(), "{t:?}");
            assert_eq!(d.body(), t);
            assert_eq!(d.render(), t);
        }
    }

    #[test]
    fn empty_frontmatter_and_eof_close() {
        let d = Document::parse("---\n---\n");
        assert_eq!(d.frontmatter().map(Frontmatter::is_empty), Some(true));
        assert_eq!(d.body(), "");
        let d = Document::parse("---\na: b\n---");
        assert_eq!(d.body(), "");
        assert_eq!(d.render(), "---\na: b\n---");
    }

    #[test]
    fn bom_and_crlf() {
        let t = "\u{feff}---\r\nid: x\r\n---  \r\nbody\r\n";
        let d = Document::parse(t);
        assert!(d.has_bom());
        assert_eq!(d.line_ending(), LineEnding::CrLf);
        assert_eq!(d.body(), "body\r\n");
        assert_eq!(d.body_offset(), t.len() - 6);
        assert_eq!(d.render(), t);
    }

    #[test]
    fn creating_frontmatter() {
        let mut d = Document::parse("hello\r\n");
        assert!(d.frontmatter_mut().is_empty());
        assert_eq!(d.render(), "hello\r\n");
        d.frontmatter_mut()
            .set("id", PropertyValue::Text("01J".into()))
            .unwrap_or_default();
        assert_eq!(d.render(), "---\r\nid: 01J\r\n---\r\nhello\r\n");
    }
}
