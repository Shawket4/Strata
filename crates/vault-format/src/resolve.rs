//! Obsidian link resolution over a set of vault paths.
//!
//! Paths are vault-relative with `/` separators (`notes/Pricing.md`,
//! `attachments/2026/09/scan.pdf`). Matching is case-insensitive like Obsidian's.

use std::collections::HashMap;

/// The outcome of resolving a link path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one file matches.
    Resolved(String),
    /// Several files match and nothing disambiguates them (sorted).
    Ambiguous(Vec<String>),
    /// No file matches.
    Unresolved,
    /// The link has only an anchor (`[[#Heading]]`): it points at the note it is in.
    CurrentNote,
}

/// Index of vault paths for resolution and for choosing the shortest link to a file.
#[derive(Debug, Clone, Default)]
pub struct PathIndex {
    paths: Vec<String>,
    by_name: HashMap<String, Vec<usize>>,
    by_link_path: HashMap<String, Vec<usize>>,
}

fn is_markdown(path: &str) -> bool {
    path.len() > 3 && path.as_bytes()[path.len() - 3..].eq_ignore_ascii_case(b".md")
}

/// The path as it appears in a link: `.md` removed, other extensions kept.
pub fn link_path(path: &str) -> &str {
    if is_markdown(path) {
        &path[..path.len() - 3]
    } else {
        path
    }
}

/// The name a file is linked by: the file name without `.md` (other extensions kept).
pub fn link_name(path: &str) -> &str {
    let lp = link_path(path);
    lp.rsplit('/').next().unwrap_or(lp)
}

fn folder(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

fn strip_md(s: &str) -> &str {
    if is_markdown(s) { &s[..s.len() - 3] } else { s }
}

/// Normalises `a/./b/../c` to `a/c`. Returns `None` if `..` escapes the root.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

impl PathIndex {
    /// Builds an index. Duplicate paths are collapsed.
    pub fn new<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut paths: Vec<String> = paths.into_iter().map(Into::into).collect();
        paths.sort();
        paths.dedup();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_link_path: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in paths.iter().enumerate() {
            by_name
                .entry(link_name(p).to_lowercase())
                .or_default()
                .push(i);
            by_link_path
                .entry(link_path(p).to_lowercase())
                .or_default()
                .push(i);
            if is_markdown(p) {
                // `[[notes/x.md]]` also names the file.
                by_link_path.entry(p.to_lowercase()).or_default().push(i);
            }
        }
        Self {
            paths,
            by_name,
            by_link_path,
        }
    }

    /// All indexed paths, sorted.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Whether `path` is indexed (exact).
    pub fn contains(&self, path: &str) -> bool {
        self.paths
            .binary_search_by(|p| p.as_str().cmp(path))
            .is_ok()
    }

    /// Resolves the path part of a link (`Note`, `folder/Note`, `image.png`, `./x`) written in
    /// the note at `source` (vault path, used for relative links and tie-breaking).
    ///
    /// Rules: a bare name matches files whose link name equals it; a path with `/` matches a
    /// vault path exactly, else by path suffix; `./` and `../` are relative to the source
    /// folder. Several matches are narrowed first to exact-case matches, then to files in the
    /// source's folder; if more than one remains the link is [`Resolution::Ambiguous`].
    pub fn resolve(&self, link: &str, source: Option<&str>) -> Resolution {
        let t = link.trim();
        if t.is_empty() {
            return Resolution::CurrentNote;
        }
        if t.starts_with("./") || t.starts_with("../") {
            let base = source.map_or("", folder);
            let joined = if base.is_empty() {
                t.to_owned()
            } else {
                format!("{base}/{t}")
            };
            return match normalize(&joined) {
                Some(p) => self.narrow(self.exact(&p), &p, source),
                None => Resolution::Unresolved,
            };
        }
        let t = t.trim_start_matches('/');
        let candidates = if t.contains('/') {
            let exact = self.exact(t);
            if exact.is_empty() {
                self.suffix(t)
            } else {
                exact
            }
        } else {
            let mut c = self
                .by_name
                .get(&strip_md(t).to_lowercase())
                .cloned()
                .unwrap_or_default();
            if is_markdown(t) {
                c.retain(|&i| is_markdown(&self.paths[i]));
            }
            c
        };
        self.narrow(candidates, t, source)
    }

    fn exact(&self, t: &str) -> Vec<usize> {
        self.by_link_path
            .get(&t.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    fn suffix(&self, t: &str) -> Vec<usize> {
        let needle = format!("/{}", strip_md(t).to_lowercase());
        self.paths
            .iter()
            .enumerate()
            .filter(|(_, p)| link_path(p).to_lowercase().ends_with(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    fn narrow(&self, mut c: Vec<usize>, written: &str, source: Option<&str>) -> Resolution {
        c.sort_unstable();
        c.dedup();
        if c.len() > 1 {
            let written = strip_md(written);
            let exact_case: Vec<usize> = c
                .iter()
                .copied()
                .filter(|&i| {
                    let lp = link_path(&self.paths[i]);
                    lp == written || lp.ends_with(&format!("/{written}"))
                })
                .collect();
            if exact_case.len() == 1 {
                c = exact_case;
            }
        }
        if c.len() > 1
            && let Some(src) = source
        {
            let dir = folder(src);
            let same: Vec<usize> = c
                .iter()
                .copied()
                .filter(|&i| folder(&self.paths[i]) == dir)
                .collect();
            if same.len() == 1 {
                c = same;
            }
        }
        match c.as_slice() {
            [] => Resolution::Unresolved,
            [i] => Resolution::Resolved(self.paths[*i].clone()),
            many => Resolution::Ambiguous(many.iter().map(|&i| self.paths[i].clone()).collect()),
        }
    }

    /// The path to write in a link to `target` ("shortest path when possible"): the bare link
    /// name when no other file shares it, otherwise the full vault path (without `.md`).
    pub fn link_text_for(&self, target: &str) -> String {
        let name = link_name(target);
        let unique = self
            .by_name
            .get(&name.to_lowercase())
            .is_none_or(|v| v.len() <= 1);
        if unique {
            name.to_owned()
        } else {
            link_path(target).to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx() -> PathIndex {
        PathIndex::new([
            "notes/Pricing.md",
            "notes/a/Meeting.md",
            "notes/b/Meeting.md",
            "people/أحمد سمير.md",
            "attachments/2026/09/scan.pdf",
            "notes/v1.2 plan.md",
            "Readme.md",
            "notes/readme.md",
        ])
    }

    fn r(s: &str) -> Resolution {
        Resolution::Resolved(s.into())
    }

    #[test]
    fn bare_names() {
        let i = idx();
        assert_eq!(i.resolve("Pricing", None), r("notes/Pricing.md"));
        assert_eq!(i.resolve("pricing", None), r("notes/Pricing.md"));
        assert_eq!(i.resolve("Pricing.md", None), r("notes/Pricing.md"));
        assert_eq!(i.resolve(" Pricing ", None), r("notes/Pricing.md"));
        assert_eq!(i.resolve("أحمد سمير", None), r("people/أحمد سمير.md"));
        assert_eq!(
            i.resolve("scan.pdf", None),
            r("attachments/2026/09/scan.pdf")
        );
        assert_eq!(i.resolve("scan", None), Resolution::Unresolved);
        assert_eq!(i.resolve("v1.2 plan", None), r("notes/v1.2 plan.md"));
        assert_eq!(i.resolve("", None), Resolution::CurrentNote);
    }

    #[test]
    fn ambiguity() {
        let i = idx();
        assert_eq!(
            i.resolve("Meeting", None),
            Resolution::Ambiguous(vec![
                "notes/a/Meeting.md".into(),
                "notes/b/Meeting.md".into()
            ])
        );
        assert_eq!(
            i.resolve("Meeting", Some("notes/b/Other.md")),
            r("notes/b/Meeting.md")
        );
        assert_eq!(i.resolve("a/Meeting", None), r("notes/a/Meeting.md"));
        assert_eq!(i.resolve("notes/b/Meeting", None), r("notes/b/Meeting.md"));
        assert_eq!(i.resolve("Readme", None), r("Readme.md"));
        assert_eq!(i.resolve("readme", None), r("notes/readme.md"));
        assert_eq!(
            i.resolve("README", None),
            Resolution::Ambiguous(vec!["Readme.md".into(), "notes/readme.md".into()])
        );
    }

    #[test]
    fn relative_links() {
        let i = idx();
        assert_eq!(
            i.resolve("./Meeting", Some("notes/a/x.md")),
            r("notes/a/Meeting.md")
        );
        assert_eq!(
            i.resolve("../b/Meeting", Some("notes/a/x.md")),
            r("notes/b/Meeting.md")
        );
        assert_eq!(
            i.resolve("../../../x", Some("notes/a/x.md")),
            Resolution::Unresolved
        );
    }

    #[test]
    fn link_text() {
        let i = idx();
        assert_eq!(i.link_text_for("notes/Pricing.md"), "Pricing");
        assert_eq!(i.link_text_for("notes/a/Meeting.md"), "notes/a/Meeting");
        assert_eq!(i.link_text_for("attachments/2026/09/scan.pdf"), "scan.pdf");
        assert_eq!(i.link_text_for("new/Thing.md"), "Thing");
        assert!(i.contains("Readme.md"));
        assert!(!i.contains("readme.md"));
        assert_eq!(link_name("a/b.c.md"), "b.c");
        assert_eq!(link_path("a/B.MD"), "a/B");
    }
}
