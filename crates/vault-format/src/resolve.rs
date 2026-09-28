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
///
/// Built once with [`PathIndex::new`] and kept current with [`PathIndex::insert`] and
/// [`PathIndex::remove`], which cost the same whatever the number of paths (bar moving the
/// sorted path list), so a writer can maintain one index across writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathIndex {
    /// Every path, sorted and unique.
    paths: Vec<String>,
    /// Lower-cased link name → paths with it, sorted.
    by_name: HashMap<String, Vec<String>>,
    /// Lower-cased link path (and, for notes, the lower-cased path with `.md`) → paths,
    /// sorted.
    by_link_path: HashMap<String, Vec<String>>,
}

/// Adds `path` to the sorted `list` of `key` (no duplicates).
fn add_to(map: &mut HashMap<String, Vec<String>>, key: String, path: &str) {
    let list = map.entry(key).or_default();
    if let Err(at) = list.binary_search_by(|p| p.as_str().cmp(path)) {
        list.insert(at, path.to_owned());
    }
}

/// Removes `path` from the list of `key`, dropping the key when the list empties.
fn remove_from(map: &mut HashMap<String, Vec<String>>, key: &str, path: &str) {
    if let Some(list) = map.get_mut(key) {
        list.retain(|p| p != path);
        if list.is_empty() {
            map.remove(key);
        }
    }
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
        let mut index = Self::default();
        for p in &paths {
            index.add_keys(p);
        }
        index.paths = paths;
        index
    }

    fn keys(path: &str) -> [Option<String>; 3] {
        [
            Some(link_name(path).to_lowercase()),
            Some(link_path(path).to_lowercase()),
            // `[[notes/x.md]]` also names the file.
            is_markdown(path).then(|| path.to_lowercase()),
        ]
    }

    fn add_keys(&mut self, path: &str) {
        let [name, lp, full] = Self::keys(path);
        if let Some(k) = name {
            add_to(&mut self.by_name, k, path);
        }
        for k in [lp, full].into_iter().flatten() {
            add_to(&mut self.by_link_path, k, path);
        }
    }

    /// Adds `path` (no-op if present).
    pub fn insert(&mut self, path: &str) {
        if let Err(at) = self.paths.binary_search_by(|p| p.as_str().cmp(path)) {
            self.paths.insert(at, path.to_owned());
            self.add_keys(path);
        }
    }

    /// Removes `path` (no-op if absent).
    pub fn remove(&mut self, path: &str) {
        if let Ok(at) = self.paths.binary_search_by(|p| p.as_str().cmp(path)) {
            self.paths.remove(at);
            let [name, lp, full] = Self::keys(path);
            if let Some(k) = name {
                remove_from(&mut self.by_name, &k, path);
            }
            for k in [lp, full].into_iter().flatten() {
                remove_from(&mut self.by_link_path, &k, path);
            }
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
            let mut c: Vec<&str> = self
                .by_name
                .get(&strip_md(t).to_lowercase())
                .map(|v| v.iter().map(String::as_str).collect())
                .unwrap_or_default();
            if is_markdown(t) {
                c.retain(|p| is_markdown(p));
            }
            c
        };
        self.narrow(candidates, t, source)
    }

    fn exact(&self, t: &str) -> Vec<&str> {
        self.by_link_path
            .get(&t.to_lowercase())
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    fn suffix(&self, t: &str) -> Vec<&str> {
        let needle = format!("/{}", strip_md(t).to_lowercase());
        self.paths
            .iter()
            .filter(|p| link_path(p).to_lowercase().ends_with(&needle))
            .map(String::as_str)
            .collect()
    }

    #[allow(clippy::unused_self)] // kept a method beside `exact` and `suffix`
    fn narrow(&self, mut c: Vec<&str>, written: &str, source: Option<&str>) -> Resolution {
        c.sort_unstable();
        c.dedup();
        if c.len() > 1 {
            let written = strip_md(written);
            let exact_case: Vec<&str> = c
                .iter()
                .copied()
                .filter(|p| {
                    let lp = link_path(p);
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
            let same: Vec<&str> = c.iter().copied().filter(|p| folder(p) == dir).collect();
            if same.len() == 1 {
                c = same;
            }
        }
        match c.as_slice() {
            [] => Resolution::Unresolved,
            [p] => Resolution::Resolved((*p).to_owned()),
            many => Resolution::Ambiguous(many.iter().map(|p| (*p).to_owned()).collect()),
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
    fn insert_and_remove_match_a_rebuild() {
        let mut i = idx();
        i.insert("notes/c/Meeting.md");
        i.insert("notes/Pricing.md");
        i.remove("notes/a/Meeting.md");
        i.remove("notes/missing.md");
        i.remove("Readme.md");
        let rebuilt = PathIndex::new([
            "notes/Pricing.md",
            "notes/b/Meeting.md",
            "notes/c/Meeting.md",
            "people/أحمد سمير.md",
            "attachments/2026/09/scan.pdf",
            "notes/v1.2 plan.md",
            "notes/readme.md",
        ]);
        assert_eq!(i, rebuilt);
        assert_eq!(
            i.resolve("Meeting", None),
            Resolution::Ambiguous(vec![
                "notes/b/Meeting.md".into(),
                "notes/c/Meeting.md".into()
            ])
        );
        assert_eq!(i.resolve("README", None), r("notes/readme.md"));
        for p in rebuilt.paths().to_vec() {
            i.remove(&p);
        }
        assert_eq!(i, PathIndex::default());
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
