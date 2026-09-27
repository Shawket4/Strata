//! Rewriting wikilinks when notes are renamed or moved (PLAN §7.2): body links and frontmatter
//! relation values, preserving embed markers, anchors, aliases and escaped pipes.

use std::collections::HashMap;

use crate::body;
use crate::frontmatter::{Frontmatter, FrontmatterError, KnownKey, PropertyValue, ValueShape};
use crate::resolve::{PathIndex, Resolution};
use crate::wikilink::WikiLink;

/// Result of rewriting a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    /// The new text.
    pub text: String,
    /// How many links were changed.
    pub changed: usize,
}

/// Replaces links in `text`: `f` returns the complete new markdown for a link, or `None` to
/// keep it. `links` must be sorted by position and non-overlapping (as returned by the
/// extraction APIs).
pub fn replace_links<F>(text: &str, links: &[WikiLink], mut f: F) -> Rewrite
where
    F: FnMut(&WikiLink) -> Option<String>,
{
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut changed = 0;
    for link in links {
        if let Some(new) = f(link)
            && new != text[link.span.clone()]
        {
            out.push_str(&text[last..link.span.start]);
            out.push_str(&new);
            last = link.span.end;
            changed += 1;
        }
    }
    out.push_str(&text[last..]);
    Rewrite { text: out, changed }
}

/// Rewrites links in a note body (code-aware).
pub fn rewrite_body<F>(body: &str, f: F) -> Rewrite
where
    F: FnMut(&WikiLink) -> Option<String>,
{
    let links = body::links(body);
    replace_links(body, &links, f)
}

/// A set of renames/moves applied to a vault in one operation.
#[derive(Debug, Clone)]
pub struct MoveSet<'a> {
    before: &'a PathIndex,
    after: &'a PathIndex,
    moves: HashMap<String, String>,
}

impl<'a> MoveSet<'a> {
    /// `before`/`after` index the vault before and after the moves; `moves` maps old paths to
    /// new paths (one entry per moved file; a folder move lists every file in it).
    pub fn new<I>(before: &'a PathIndex, after: &'a PathIndex, moves: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self {
            before,
            after,
            moves: moves.into_iter().collect(),
        }
    }

    /// The path a file has after the moves.
    pub fn moved<'p>(&'p self, path: &'p str) -> &'p str {
        self.moves.get(path).map_or(path, String::as_str)
    }

    /// The new markdown for `link` written in the note that was at `source` before the moves,
    /// or `None` when the link still points where it did. Links that did not resolve to a
    /// single file before are left alone. A link that resolved before but would resolve
    /// differently afterwards (because its target moved, the source moved, or a new file now
    /// shares its name) is rewritten to the shortest unambiguous form.
    pub fn rewrite_link(&self, link: &WikiLink, source: &str) -> Option<String> {
        let Resolution::Resolved(target) = self.before.resolve(&link.path, Some(source)) else {
            return None;
        };
        let new_target = self.moved(&target);
        let new_source = self.moved(source);
        if self.after.resolve(&link.path, Some(new_source)) == Resolution::Resolved(new_target.to_owned()) {
            return None;
        }
        let mut text = self.after.link_text_for(new_target);
        let wrote_md = link.path.trim_end().to_ascii_lowercase().ends_with(".md");
        if wrote_md && new_target.to_ascii_lowercase().ends_with(".md") {
            text.push_str(".md");
        }
        Some(link.with_path(&text))
    }

    /// Rewrites the links in a body written in the note that was at `source`.
    pub fn rewrite_body(&self, body: &str, source: &str) -> Rewrite {
        rewrite_body(body, |l| self.rewrite_link(l, source))
    }

    /// Rewrites wikilink values of the link-holding known keys (relation lists, `source`,
    /// `location`, `holder`, `last-holder`). Unknown keys are never touched. Returns how many
    /// values changed.
    pub fn rewrite_frontmatter(&self, fm: &mut Frontmatter, source: &str) -> Result<usize, FrontmatterError> {
        let mut changed = 0;
        for &key in KnownKey::ALL.iter().filter(|k| k.holds_links()) {
            let Some(value) = fm.get(key.as_str()).cloned() else {
                continue;
            };
            let map = |s: &str| {
                WikiLink::parse_exact(s).and_then(|l| self.rewrite_link(&l, source))
            };
            match (key.shape(), value) {
                (ValueShape::Link, PropertyValue::Text(s)) => {
                    if let Some(new) = map(&s) {
                        fm.set(key.as_str(), PropertyValue::Text(new))?;
                        changed += 1;
                    }
                }
                (ValueShape::LinkList, PropertyValue::List(items)) => {
                    let mut n = 0;
                    let new: Vec<String> = items
                        .iter()
                        .map(|s| {
                            map(s).map_or_else(
                                || s.clone(),
                                |v| {
                                    n += 1;
                                    v
                                },
                            )
                        })
                        .collect();
                    if n > 0 {
                        fm.set(key.as_str(), PropertyValue::List(new))?;
                        changed += n;
                    }
                }
                (ValueShape::LinkList, PropertyValue::Text(s)) => {
                    if let Some(new) = map(&s) {
                        fm.set(key.as_str(), PropertyValue::Text(new))?;
                        changed += 1;
                    }
                }
                _ => {}
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexes() -> (PathIndex, PathIndex) {
        let before = PathIndex::new(["notes/Old.md", "notes/Other.md", "x/Thing.md", "img/a.png"]);
        let after = PathIndex::new(["archive/New.md", "notes/Other.md", "x/Thing.md", "img/a.png"]);
        (before, after)
    }

    #[test]
    fn rewrites_every_form() {
        let (b, a) = indexes();
        let set = MoveSet::new(&b, &a, [("notes/Old.md".to_owned(), "archive/New.md".to_owned())]);
        let body = "[[Old]] [[Old|alias]] [[Old#Head]] [[Old#^blk|c]] ![[Old]] | [[Old\\|t]] | [[notes/Old]] [[Old.md]] [[Other]] `[[Old]]`\n";
        let r = set.rewrite_body(body, "notes/Other.md");
        assert_eq!(
            r.text,
            "[[New]] [[New|alias]] [[New#Head]] [[New#^blk|c]] ![[New]] | [[New\\|t]] | [[New]] [[New.md]] [[Other]] `[[Old]]`\n"
        );
        assert_eq!(r.changed, 8);
    }

    #[test]
    fn new_name_collision_uses_full_path() {
        let before = PathIndex::new(["a/Old.md", "b/Thing.md"]);
        let after = PathIndex::new(["a/Thing.md", "b/Thing.md"]);
        let set = MoveSet::new(&before, &after, [("a/Old.md".to_owned(), "a/Thing.md".to_owned())]);
        // Link to the moved note gets the full path; the existing `[[Thing]]` link that used to
        // resolve to b/Thing.md would now be ambiguous, so it is made explicit too.
        let r = set.rewrite_body("[[Old]] [[Thing]]", "c/Note.md");
        assert_eq!(r.text, "[[a/Thing]] [[b/Thing]]");
        assert_eq!(r.changed, 2);
    }

    #[test]
    fn unresolved_and_ambiguous_are_left_alone() {
        let before = PathIndex::new(["a/X.md", "b/X.md", "c/Old.md"]);
        let after = PathIndex::new(["a/X.md", "b/X.md", "c/New.md"]);
        let set = MoveSet::new(&before, &after, [("c/Old.md".to_owned(), "c/New.md".to_owned())]);
        let r = set.rewrite_body("[[X]] [[Missing]] [[#Local]]", "d/n.md");
        assert_eq!(r.changed, 0);
        assert_eq!(r.text, "[[X]] [[Missing]] [[#Local]]");
    }

    #[test]
    fn relative_links_follow_moved_source() {
        let before = PathIndex::new(["a/S.md", "a/T.md"]);
        let after = PathIndex::new(["b/S.md", "a/T.md"]);
        let set = MoveSet::new(&before, &after, [("a/S.md".to_owned(), "b/S.md".to_owned())]);
        let r = set.rewrite_body("[[./T]]", "a/S.md");
        assert_eq!(r.text, "[[T]]");
    }

    #[test]
    fn frontmatter_relations_follow() {
        let (b, a) = indexes();
        let set = MoveSet::new(&b, &a, [("notes/Old.md".to_owned(), "archive/New.md".to_owned())]);
        let mut fm = Frontmatter::from_parts(
            "---\n",
            "related: [\"[[Old]]\", \"[[Other]]\"]\nlocation: \"[[Old|safe]]\"\nmine: \"[[Old]]\"\npeople: \"[[Old]]\"\n",
            "---\n",
        );
        assert_eq!(set.rewrite_frontmatter(&mut fm, "x/Thing.md"), Ok(3));
        assert_eq!(
            fm.render(),
            "---\nlocation: \"[[New|safe]]\"\nrelated: [\"[[New]]\", \"[[Other]]\"]\npeople: \"[[New]]\"\nmine: \"[[Old]]\"\n---\n"
        );
    }

    #[test]
    fn replace_links_with_closure() {
        let text = "a [[x]] b [[y]]";
        let links = crate::wikilink::find_all(text);
        let r = replace_links(text, &links, |l| (l.path == "y").then(|| "[[z]]".to_owned()));
        assert_eq!(r, Rewrite { text: "a [[x]] b [[z]]".into(), changed: 1 });
    }
}
