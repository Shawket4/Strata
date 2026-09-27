//! YAML frontmatter ("Properties") that preserves every byte it was not asked to change.
//!
//! The frontmatter text is split into top-level entries (a key line plus its continuation
//! lines, with any comment/blank lines before it). Each entry keeps its raw text. Values are
//! read by parsing the whole block with a YAML 1.2 parser. Rendering an untouched frontmatter
//! returns the original bytes; rendering after a change re-emits only the changed entries in
//! canonical form and orders entries canonically (PLAN §6.4).

mod keys;
mod typed;
mod yaml;

use std::collections::HashMap;

use yaml_rust2::{Yaml, YamlLoader};

pub use keys::{KnownKey, RelationKey, UnknownRelation, ValueShape};
pub use typed::{CopyKind, DocType, DocumentStatus, Lang, NoteKind, format_timestamp};
pub use yaml::PropertyValue;

use crate::line::{self, LineEnding};

/// Errors reading or editing frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrontmatterError {
    /// The block is not valid YAML (includes duplicate keys). The text is kept verbatim.
    #[error("frontmatter is not valid YAML: {0}")]
    InvalidYaml(String),
    /// The block is valid YAML but not a mapping.
    #[error("frontmatter is not a YAML mapping")]
    NotAMapping,
    /// The block uses YAML structure the entry splitter cannot edit safely.
    #[error("frontmatter layout is not editable: {0}")]
    Unsupported(String),
    /// A value could not be converted to the requested type.
    #[error("invalid value for `{key}`: {reason}")]
    InvalidValue {
        /// The key.
        key: String,
        /// Why it is invalid.
        reason: String,
    },
    /// Only flat values (null, scalar, list of scalars) can be written.
    #[error("cannot write a nested value to `{0}`")]
    NestedValue(String),
}

/// One top-level entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    /// The key, or `None` for entries the splitter could not name (e.g. `? complex` keys).
    key: Option<String>,
    /// Comment and blank lines directly before the key line.
    leading: String,
    /// The key line and its continuation lines, each with its terminator.
    raw: String,
    /// Parsed value.
    value: PropertyValue,
    /// Original position (new entries get positions after all original ones).
    order: usize,
}

/// Frontmatter of one note. Construct with [`crate::Document::parse`] or [`Frontmatter::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    open: String,
    entries: Vec<Entry>,
    trailing: String,
    close: String,
    eol: LineEnding,
    error: Option<FrontmatterError>,
    dirty: bool,
    created: bool,
    next_order: usize,
}

impl Frontmatter {
    /// An empty frontmatter to be filled in (for notes that had none).
    pub fn new(eol: LineEnding) -> Self {
        Self {
            open: format!("---{}", eol.as_str()),
            entries: Vec::new(),
            trailing: String::new(),
            close: format!("---{}", eol.as_str()),
            eol,
            error: None,
            dirty: true,
            created: true,
            next_order: 0,
        }
    }

    /// Parses the text between the delimiter lines. `open` and `close` are the delimiter lines
    /// including their terminators (`close` may lack one at end of file).
    pub(crate) fn from_parts(open: &str, inner: &str, close: &str) -> Self {
        let eol = LineEnding::detect(open);
        let (entries, trailing) = split_entries(inner);
        let next_order = entries.len();
        let mut fm = Self {
            open: open.to_owned(),
            entries,
            trailing,
            close: close.to_owned(),
            eol,
            error: None,
            dirty: false,
            created: false,
            next_order,
        };
        fm.error = fm.load_values(inner).err();
        fm
    }

    fn load_values(&mut self, inner: &str) -> Result<(), FrontmatterError> {
        if has_bare_cr(inner) {
            return Err(FrontmatterError::Unsupported("bare carriage return".into()));
        }
        let docs = YamlLoader::load_from_str(inner)
            .map_err(|e| FrontmatterError::InvalidYaml(e.to_string()))?;
        let empty = yaml_rust2::yaml::Hash::new();
        let map = match docs.as_slice() {
            [] | [Yaml::Null] => &empty,
            [Yaml::Hash(map)] => map,
            [_] => return Err(FrontmatterError::NotAMapping),
            _ => return Err(FrontmatterError::Unsupported("multiple YAML documents".into())),
        };
        if map.len() != self.entries.len() {
            return Err(FrontmatterError::Unsupported(format!(
                "found {} top-level lines but {} keys",
                self.entries.len(),
                map.len()
            )));
        }
        for entry in &mut self.entries {
            let Some(key) = entry.key.as_deref() else {
                return Err(FrontmatterError::Unsupported("complex mapping key".into()));
            };
            let Some(node) = lookup(map, key) else {
                return Err(FrontmatterError::Unsupported(format!("key `{key}` not found")));
            };
            entry.value = yaml::to_property(node, raw_scalar(&entry.raw));
        }
        Ok(())
    }

    /// The problem with this frontmatter, if any. When set, values read as absent and every
    /// edit fails; the text still renders byte-for-byte.
    pub fn error(&self) -> Option<&FrontmatterError> {
        self.error.as_ref()
    }

    /// The line ending used for lines Strata writes.
    pub fn line_ending(&self) -> LineEnding {
        self.eol
    }

    /// Whether any value was changed since parsing.
    pub fn is_modified(&self) -> bool {
        self.dirty
    }

    /// Keys in file order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|e| e.key.as_deref())
    }

    /// Number of top-level entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The value of `key`. `None` when absent or when the frontmatter is invalid.
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        if self.error.is_some() {
            return None;
        }
        self.entry(key).map(|e| &e.value)
    }

    /// The raw YAML text of `key`'s entry (key line and continuation lines).
    pub fn raw_entry(&self, key: &str) -> Option<&str> {
        self.entry(key).map(|e| e.raw.as_str())
    }

    fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key.as_deref() == Some(key))
    }

    fn ensure_editable(&self) -> Result<(), FrontmatterError> {
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    /// Sets `key` to `value`, rendering the entry canonically. Unchanged values leave the
    /// entry (and the file) untouched. Fails (and changes nothing) if the edited block would
    /// not read back as exactly the expected values.
    pub fn set(&mut self, key: &str, value: PropertyValue) -> Result<(), FrontmatterError> {
        self.ensure_editable()?;
        if value == PropertyValue::Other {
            return Err(FrontmatterError::NestedValue(key.to_owned()));
        }
        let raw = render_entry(key, &value, self.eol);
        let before = self.entries.clone();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.key.as_deref() == Some(key))
        {
            if entry.value == value {
                return Ok(());
            }
            entry.raw = raw;
            entry.value = value;
        } else {
            self.entries.push(Entry {
                key: Some(key.to_owned()),
                leading: String::new(),
                raw,
                value,
                order: self.next_order,
            });
            self.next_order += 1;
        }
        self.commit(before)
    }

    /// Removes `key` (and comment lines directly above it). Returns whether it existed.
    pub fn remove(&mut self, key: &str) -> Result<bool, FrontmatterError> {
        self.ensure_editable()?;
        let before = self.entries.clone();
        self.entries.retain(|e| e.key.as_deref() != Some(key));
        if self.entries.len() == before.len() {
            return Ok(false);
        }
        self.commit(before).map(|()| true)
    }

    /// Keeps an edit only if some layout of the edited entries reads back correctly.
    fn commit(&mut self, before: Vec<Entry>) -> Result<(), FrontmatterError> {
        if self.valid(&self.layout(false, true)) || self.valid(&self.layout(false, false)) {
            self.dirty = true;
            Ok(())
        } else {
            self.entries = before;
            Err(FrontmatterError::Unsupported(
                "the edit would change how the other properties are read".into(),
            ))
        }
    }

    /// Renders the frontmatter block including both delimiter lines. Untouched (or invalid)
    /// → original bytes. Modified → entries in canonical order; untouched entries keep their
    /// raw text. If canonical order would change how the YAML reads (e.g. an alias used
    /// before its anchor), the original order is kept.
    pub fn render(&self) -> String {
        if !self.dirty || self.error.is_some() {
            return self.wrap(&self.layout(false, false));
        }
        let ordered = self.layout(false, true);
        if self.valid(&ordered) {
            return self.wrap(&ordered);
        }
        self.wrap(&self.layout(false, false))
    }

    /// Like [`Frontmatter::render`] but also re-renders every known key canonically and always
    /// applies canonical order. Unknown keys keep their raw text and original order. Invalid
    /// frontmatter is returned verbatim.
    pub fn render_canonical(&self) -> String {
        if self.error.is_some() {
            return self.wrap(&self.layout(false, false));
        }
        for (canonical, reorder) in [(true, true), (false, true)] {
            let layout = self.layout(canonical, reorder);
            if self.valid(&layout) {
                return self.wrap(&layout);
            }
        }
        self.wrap(&self.layout(false, false))
    }

    /// Entries in output order with the raw text each would be written with.
    fn layout(&self, canonical: bool, reorder: bool) -> Vec<(&Entry, String)> {
        let mut ordered: Vec<&Entry> = self.entries.iter().collect();
        if reorder {
            ordered.sort_by_key(|e| (sort_rank(e), e.order));
        }
        ordered
            .into_iter()
            .map(|e| {
                let known = e.key.as_deref().and_then(KnownKey::from_name);
                let raw = match known.filter(|_| canonical) {
                    Some(k) => canonical_value(k, &e.value)
                        .map_or_else(|| e.raw.clone(), |v| render_entry(k.as_str(), &v, self.eol)),
                    None => e.raw.clone(),
                };
                (e, raw)
            })
            .collect()
    }

    fn inner(&self, layout: &[(&Entry, String)]) -> String {
        let mut out = String::new();
        for (e, raw) in layout {
            out.push_str(&e.leading);
            out.push_str(raw);
        }
        out.push_str(&self.trailing);
        out
    }

    fn wrap(&self, layout: &[(&Entry, String)]) -> String {
        if self.created && self.entries.is_empty() && self.trailing.is_empty() {
            return String::new();
        }
        format!("{}{}{}", self.open, self.inner(layout), self.close)
    }

    /// Whether `layout` parses back to exactly the entries' keys and values.
    fn valid(&self, layout: &[(&Entry, String)]) -> bool {
        let inner = self.inner(layout);
        if has_bare_cr(&inner) {
            return false;
        }
        let Ok(docs) = YamlLoader::load_from_str(&inner) else {
            return false;
        };
        let empty = yaml_rust2::yaml::Hash::new();
        let map = match docs.as_slice() {
            [] | [Yaml::Null] => &empty,
            [Yaml::Hash(map)] => map,
            _ => return false,
        };
        map.len() == layout.len()
            && layout.iter().all(|(e, raw)| {
                e.key
                    .as_deref()
                    .and_then(|k| lookup(map, k))
                    .is_some_and(|node| yaml::to_property(node, raw_scalar(raw)) == e.value)
            })
    }
}

/// A `\r` not followed by `\n` is a line break to YAML but not to the entry splitter.
fn has_bare_cr(text: &str) -> bool {
    text.match_indices('\r')
        .any(|(i, _)| text.as_bytes().get(i + 1) != Some(&b'\n'))
}

fn lookup<'a>(map: &'a yaml_rust2::yaml::Hash, key: &str) -> Option<&'a Yaml> {
    map.get(&Yaml::String(key.to_owned())).or_else(|| {
        map.iter()
            .find(|(k, _)| yaml_key_text(k).as_deref() == Some(key))
            .map(|(_, v)| v)
    })
}

fn sort_rank(e: &Entry) -> usize {
    e.key
        .as_deref()
        .and_then(KnownKey::from_name)
        .map_or(KnownKey::ALL.len(), KnownKey::rank)
}

/// The canonical value of a known key, or `None` when it has no canonical form (nested).
fn canonical_value(key: KnownKey, value: &PropertyValue) -> Option<PropertyValue> {
    match (key.shape(), value) {
        (_, PropertyValue::Other)
        | (ValueShape::Text | ValueShape::Link, PropertyValue::List(_)) => None,
        (ValueShape::List | ValueShape::LinkList, v) => Some(PropertyValue::List(v.to_list())),
        (ValueShape::Text | ValueShape::Link, v) => Some(v.clone()),
    }
}

fn render_entry(key: &str, value: &PropertyValue, eol: LineEnding) -> String {
    let key = if KnownKey::from_name(key).is_some() {
        key.to_owned()
    } else {
        yaml::render_scalar(key, true)
    };
    let eol = eol.as_str();
    match value {
        PropertyValue::Null | PropertyValue::Other => format!("{key}:{eol}"),
        PropertyValue::Text(s) => format!("{key}: {}{eol}", yaml::render_scalar(s, false)),
        PropertyValue::List(items) => format!("{key}: {}{eol}", yaml::render_flow_list(items)),
    }
}

fn yaml_key_text(k: &Yaml) -> Option<String> {
    match k {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The source text of a single-line plain scalar value (`key: <this> # comment`).
fn raw_scalar(raw: &str) -> Option<&str> {
    let first = line::lines(raw).next()?.content;
    let colon = key_line(first)?.1;
    let mut value = first[colon + 1..].trim();
    if let Some(i) = value.find(" #") {
        value = value[..i].trim_end();
    }
    Some(value)
}

/// If `line` starts a top-level mapping entry, returns its key and the byte index of the `:`.
fn key_line(line: &str) -> Option<(String, usize)> {
    let first = line.chars().next()?;
    match first {
        ' ' | '\t' | '#' | '[' | '{' | '?' | '|' | '>' | '&' | '*' | '!' | '%' | '@' | '`' => None,
        '-' if line == "-" || line.starts_with("- ") || line.starts_with("-\t") => None,
        '"' | '\'' => {
            let (key, end) = quoted_key(line, first)?;
            let rest = &line[end..];
            let ws = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            let after = &rest[ws..];
            if after.starts_with(':') && separator_follows(&after[1..]) {
                Some((key, end + ws))
            } else {
                None
            }
        }
        _ => {
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b':' && separator_follows(&line[i + 1..]) {
                    let key = line[..i].trim_end();
                    if key.is_empty() || key.contains(" #") {
                        return None;
                    }
                    return Some((key.to_owned(), i));
                }
                i += 1;
            }
            None
        }
    }
}

fn separator_follows(rest: &str) -> bool {
    rest.is_empty() || rest.starts_with([' ', '\t'])
}

fn quoted_key(line: &str, quote: char) -> Option<(String, usize)> {
    let mut key = String::new();
    let mut chars = line.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if c == quote {
            if quote == '\'' && chars.peek().is_some_and(|&(_, n)| n == '\'') {
                chars.next();
                key.push('\'');
                continue;
            }
            return Some((key, i + 1));
        }
        if quote == '"' && c == '\\' {
            let (_, esc) = chars.next()?;
            key.push(match esc {
                'n' => '\n',
                't' => '\t',
                other => other,
            });
            continue;
        }
        key.push(c);
    }
    None
}

/// Splits the frontmatter text into entries plus trailing trivia.
fn split_entries(inner: &str) -> (Vec<Entry>, String) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut pending = String::new();
    for l in line::lines(inner) {
        let full = &inner[l.full_range()];
        let content = l.content;
        let is_trivia = content.trim().is_empty() || content.starts_with('#');
        if is_trivia {
            pending.push_str(full);
            continue;
        }
        let starts_entry = key_line(content).map(|(k, _)| k);
        let complex = content.starts_with("? ") || content == "?";
        if starts_entry.is_some() || complex || entries.is_empty() {
            let order = entries.len();
            entries.push(Entry {
                key: starts_entry,
                leading: std::mem::take(&mut pending),
                raw: full.to_owned(),
                value: PropertyValue::Null,
                order,
            });
        } else if let Some(last) = entries.last_mut() {
            last.raw.push_str(&pending);
            pending.clear();
            last.raw.push_str(full);
        }
    }
    (entries, pending)
}

/// Collects the values of every key (for diagnostics and tests).
pub fn values(fm: &Frontmatter) -> HashMap<String, PropertyValue> {
    fm.entries
        .iter()
        .filter_map(|e| Some((e.key.clone()?, e.value.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(inner: &str) -> Frontmatter {
        Frontmatter::from_parts("---\n", inner, "---\n")
    }

    #[test]
    fn splits_entries_with_trivia() {
        let f = fm("# c\nid: 1\ntags:\n  - a\n\n  - b\nx: |\n  line\n\n# end\n");
        assert_eq!(f.error(), None);
        assert_eq!(f.keys().collect::<Vec<_>>(), ["id", "tags", "x"]);
        assert_eq!(f.entries[0].leading, "# c\n");
        assert_eq!(f.entries[1].raw, "tags:\n  - a\n\n  - b\n");
        assert_eq!(f.trailing, "\n# end\n");
        assert_eq!(
            f.get("tags"),
            Some(&PropertyValue::List(vec!["a".into(), "b".into()]))
        );
        assert_eq!(f.get("id"), Some(&PropertyValue::Text("1".into())));
        assert_eq!(f.get("x"), Some(&PropertyValue::Text("line\n".into())));
    }

    #[test]
    fn key_line_detection() {
        assert_eq!(key_line("id: x"), Some(("id".into(), 2)));
        assert_eq!(key_line("url: http://a:b"), Some(("url".into(), 3)));
        assert_eq!(key_line("http://a: b"), Some(("http://a".into(), 8)));
        assert_eq!(key_line("\"a: b\": c"), Some(("a: b".into(), 6)));
        assert_eq!(key_line("'it''s': c"), Some(("it's".into(), 7)));
        assert_eq!(key_line("- a"), None);
        assert_eq!(key_line("  a: b"), None);
        assert_eq!(key_line("plain"), None);
        assert_eq!(key_line("k:"), Some(("k".into(), 1)));
        assert_eq!(key_line("-k: v"), Some(("-k".into(), 2)));
    }

    #[test]
    fn invalid_yaml_is_kept_and_read_only() {
        let mut f = fm("a: [\nb: c\n");
        assert!(matches!(f.error(), Some(FrontmatterError::InvalidYaml(_))));
        assert_eq!(f.get("b"), None);
        assert!(f.set("b", PropertyValue::Text("d".into())).is_err());
        assert_eq!(f.render(), "---\na: [\nb: c\n---\n");
    }

    #[test]
    fn duplicate_keys_are_invalid() {
        let f = fm("a: 1\na: 2\n");
        assert!(matches!(f.error(), Some(FrontmatterError::InvalidYaml(_))));
    }

    #[test]
    fn non_mapping_is_rejected() {
        let f = fm("- a\n- b\n");
        assert_eq!(f.error(), Some(&FrontmatterError::NotAMapping));
        let f = fm("just text\n");
        assert_eq!(f.error(), Some(&FrontmatterError::NotAMapping));
    }

    #[test]
    fn comments_only_is_empty_mapping() {
        let f = fm("# nothing\n");
        assert_eq!(f.error(), None);
        assert!(f.is_empty());
    }

    #[test]
    fn set_same_value_is_noop() {
        let mut f = fm("title:   'x'\n");
        f.set("title", PropertyValue::Text("x".into()))
            .unwrap_or_default();
        assert!(!f.is_modified());
        assert_eq!(f.render(), "---\ntitle:   'x'\n---\n");
    }

    #[test]
    fn nested_values_cannot_be_written() {
        let mut f = fm("");
        assert_eq!(
            f.set("a", PropertyValue::Other),
            Err(FrontmatterError::NestedValue("a".into()))
        );
    }

    #[test]
    fn unknown_new_keys_are_quoted_when_needed() {
        let mut f = fm("");
        f.set("my key: x", PropertyValue::Null).unwrap_or_default();
        assert_eq!(f.render(), "---\n\"my key: x\":\n---\n");
    }

    #[test]
    fn values_map() {
        let f = fm("a: 1\nb: [x]\n");
        let v = values(&f);
        assert_eq!(v.len(), 2);
        assert_eq!(v["b"], PropertyValue::List(vec!["x".into()]));
    }
}
