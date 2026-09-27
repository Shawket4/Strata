//! YAML value conversion and canonical scalar rendering.

use std::fmt::Write as _;

use yaml_rust2::{Yaml, YamlLoader};

use crate::line::LineEnding;

/// A frontmatter property value, reduced to the shapes Obsidian properties use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// An empty value (`key:`, `key: null`, `key: ~`).
    Null,
    /// A scalar. Numbers and booleans keep their source text (`007` stays `007`).
    Text(String),
    /// A flat list of scalars.
    List(Vec<String>),
    /// Anything else (nested mappings, nested lists): readable only as raw YAML.
    Other,
}

impl PropertyValue {
    /// The scalar text, if this is a scalar.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a list: a scalar becomes a one-element list, `Null` an empty list
    /// (Obsidian accepts `aliases: foo` for `aliases: [foo]`).
    pub fn to_list(&self) -> Vec<String> {
        match self {
            Self::Text(s) if !s.is_empty() => vec![s.clone()],
            Self::List(v) => v.clone(),
            _ => Vec::new(),
        }
    }
}

/// Converts a parsed YAML node. `raw_scalar` supplies the source text of a top-level plain
/// scalar so numbers/booleans keep their exact spelling.
pub(crate) fn to_property(y: &Yaml, raw_scalar: Option<&str>) -> PropertyValue {
    match y {
        Yaml::Null => PropertyValue::Null,
        Yaml::String(s) => PropertyValue::Text(s.clone()),
        Yaml::Integer(_) | Yaml::Real(_) | Yaml::Boolean(_) => match raw_scalar {
            Some(raw) if !raw.is_empty() => PropertyValue::Text(raw.to_owned()),
            _ => scalar_text(y).map_or(PropertyValue::Other, PropertyValue::Text),
        },
        Yaml::Array(items) => {
            if let Some(link) = unquoted_wikilink(y) {
                return PropertyValue::Text(link);
            }
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some(link) = unquoted_wikilink(item) {
                    out.push(link);
                } else if let Some(s) = scalar_text(item) {
                    out.push(s);
                } else if matches!(item, Yaml::Null) {
                    // `- ` with nothing after it: Obsidian drops empty list entries.
                } else {
                    return PropertyValue::Other;
                }
            }
            PropertyValue::List(out)
        }
        Yaml::Hash(_) | Yaml::Alias(_) | Yaml::BadValue => PropertyValue::Other,
    }
}

fn scalar_text(y: &Yaml) -> Option<String> {
    match y {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// `[[Note]]` written without quotes parses as a list holding a list holding `Note`.
/// Obsidian users do this; read it back as the wikilink the user meant.
fn unquoted_wikilink(y: &Yaml) -> Option<String> {
    if let Yaml::Array(outer) = y
        && let [Yaml::Array(inner)] = outer.as_slice()
        && let [inner_item] = inner.as_slice()
    {
        return scalar_text(inner_item).map(|s| format!("[[{s}]]"));
    }
    None
}

/// Renders a scalar for a block value (`key: <here>`) or a flow list item (`[<here>]`).
/// Plain style when the text reads back unchanged, double-quoted otherwise.
pub(crate) fn render_scalar(s: &str, in_flow: bool) -> String {
    if plain_is_safe(s, in_flow) {
        s.to_owned()
    } else {
        double_quote(s)
    }
}

fn plain_is_safe(s: &str, in_flow: bool) -> bool {
    if s.is_empty()
        || s.trim() != s
        || s.chars().any(|c| c.is_control() || c == '\u{feff}')
        || s.contains(": ")
        || s.contains(" #")
    {
        return false;
    }
    let first = s.chars().next().unwrap_or(' ');
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return false;
    }
    if in_flow && s.contains([',', '[', ']', '{', '}']) {
        return false;
    }
    // YAML 1.1 booleans are still read as booleans by some tools; keep them strings.
    let lower = s.to_ascii_lowercase();
    if matches!(lower.as_str(), "yes" | "no" | "on" | "off" | "y" | "n") {
        return false;
    }
    // Final authority: the plain text must parse back as exactly this string.
    let doc = if in_flow {
        format!("k: [{s}]")
    } else {
        format!("k: {s}")
    };
    let Ok(parsed) = YamlLoader::load_from_str(&doc) else {
        return false;
    };
    let Some(Yaml::Hash(map)) = parsed.first() else {
        return false;
    };
    let value = map.get(&Yaml::String("k".into()));
    match (in_flow, value) {
        (true, Some(Yaml::Array(items))) => {
            matches!(items.as_slice(), [Yaml::String(v)] if v == s)
        }
        (false, Some(Yaml::String(v))) => v == s,
        _ => false,
    }
}

/// YAML double-quoted scalar.
pub(crate) fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() || c == '\u{feff}' || c == '\u{2028}' || c == '\u{2029}' => {
                let code = u32::from(c);
                if code <= 0xff {
                    let _ = write!(out, "\\x{code:02x}");
                } else {
                    let _ = write!(out, "\\u{code:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Renders a whole entry (`key: value` or `key:` plus indented block lines) for any YAML
/// value. `key` is already rendered. Errors name what cannot be written.
pub(crate) fn render_entry_yaml(key: &str, value: &Yaml, eol: LineEnding) -> Result<String, String> {
    let eol = eol.as_str();
    let mut out = String::new();
    match value {
        Yaml::Null => {
            let _ = write!(out, "{key}:{eol}");
        }
        v if is_block(v) => {
            let _ = write!(out, "{key}:{eol}");
            render_block(v, 2, false, eol, &mut out)?;
        }
        v => {
            let _ = write!(out, "{key}: {}{eol}", inline(v)?);
        }
    }
    Ok(out)
}

/// A non-empty mapping or sequence (written as block lines).
fn is_block(v: &Yaml) -> bool {
    match v {
        Yaml::Hash(h) => !h.is_empty(),
        Yaml::Array(a) => !a.is_empty(),
        _ => false,
    }
}

/// A scalar or empty collection on the current line.
fn inline(v: &Yaml) -> Result<String, String> {
    Ok(match v {
        Yaml::Null => "null".to_owned(),
        Yaml::String(s) => render_scalar(s, false),
        Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Hash(_) => "{}".to_owned(),
        Yaml::Array(_) => "[]".to_owned(),
        Yaml::Alias(_) => return Err("aliases cannot be written".into()),
        Yaml::BadValue => return Err("bad value".into()),
    })
}

/// A mapping key.
fn render_key(k: &Yaml) -> Result<String, String> {
    match k {
        Yaml::String(s) => Ok(render_scalar(s, false)),
        Yaml::Integer(_) | Yaml::Real(_) | Yaml::Boolean(_) => inline(k),
        _ => Err("mapping keys must be scalars".into()),
    }
}

/// Writes a non-empty collection as block lines at `indent` spaces. With `compact`, the
/// first line continues the current one (after `- `).
fn render_block(
    v: &Yaml,
    indent: usize,
    compact: bool,
    eol: &str,
    out: &mut String,
) -> Result<(), String> {
    let pad = " ".repeat(indent);
    let mut first = true;
    let mut start = |out: &mut String| {
        if !(compact && first) {
            out.push_str(&pad);
        }
        first = false;
    };
    match v {
        Yaml::Hash(h) => {
            for (k, item) in h {
                start(out);
                out.push_str(&render_key(k)?);
                out.push(':');
                if is_block(item) {
                    out.push_str(eol);
                    render_block(item, indent + 2, false, eol, out)?;
                } else {
                    out.push(' ');
                    out.push_str(&inline(item)?);
                    out.push_str(eol);
                }
            }
        }
        Yaml::Array(items) => {
            for item in items {
                start(out);
                out.push_str("- ");
                if is_block(item) {
                    render_block(item, indent + 2, true, eol, out)?;
                } else {
                    out.push_str(&inline(item)?);
                    out.push_str(eol);
                }
            }
        }
        other => {
            out.push_str(&inline(other)?);
            out.push_str(eol);
        }
    }
    Ok(())
}

/// Renders a flow list `[a, "b, c"]`.
pub(crate) fn render_flow_list(items: &[String]) -> String {
    let mut out = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&render_scalar(item, true));
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_block(s: &str) -> Option<String> {
        let doc = format!("k: {}", render_scalar(s, false));
        let parsed = YamlLoader::load_from_str(&doc).ok()?;
        parsed.first()?["k"].as_str().map(str::to_owned)
    }

    fn roundtrip_flow(items: &[String]) -> Option<Vec<String>> {
        let doc = format!("k: {}", render_flow_list(items));
        let parsed = YamlLoader::load_from_str(&doc).ok()?;
        match to_property(&parsed.first()?["k"], None) {
            PropertyValue::List(v) => Some(v),
            _ => None,
        }
    }

    #[test]
    fn plain_when_safe() {
        assert_eq!(
            render_scalar("Pricing experiments", false),
            "Pricing experiments"
        );
        assert_eq!(render_scalar("أحمد سمير", true), "أحمد سمير");
        assert_eq!(render_scalar("A. Samir", true), "A. Samir");
        assert_eq!(
            render_scalar("2026-09-27T14:32:00+03:00", false),
            "2026-09-27T14:32:00+03:00"
        );
        assert_eq!(render_scalar("2027-03-31", false), "2027-03-31");
        assert_eq!(render_scalar("01J8ZK3M4X7Q", false), "01J8ZK3M4X7Q");
    }

    #[test]
    fn quoted_when_needed() {
        assert_eq!(
            render_scalar("[[Churn notes]]", true),
            "\"[[Churn notes]]\""
        );
        assert_eq!(render_scalar("", false), "\"\"");
        assert_eq!(render_scalar("2024", false), "\"2024\"");
        assert_eq!(render_scalar("true", false), "\"true\"");
        assert_eq!(render_scalar("yes", false), "\"yes\"");
        assert_eq!(render_scalar("a: b", false), "\"a: b\"");
        assert_eq!(render_scalar("a, b", true), "\"a, b\"");
        assert_eq!(render_scalar("a, b", false), "a, b");
        assert_eq!(render_scalar("x #y", false), "\"x #y\"");
        assert_eq!(render_scalar(" lead", false), "\" lead\"");
        assert_eq!(
            render_scalar("say \"hi\"\n", false),
            "\"say \\\"hi\\\"\\n\""
        );
        assert_eq!(render_scalar("null", false), "\"null\"");
        assert_eq!(render_scalar("~", false), "\"~\"");
        assert_eq!(render_scalar("#tag", true), "\"#tag\"");
    }

    #[test]
    fn rendered_scalars_read_back() {
        for s in [
            "a\tb",
            "\u{7}",
            "back\\slash",
            "q'uote",
            "é",
            "x\u{2028}y",
            "-1",
            "0x10",
        ] {
            assert_eq!(roundtrip_block(s).as_deref(), Some(s), "{s:?}");
        }
        let items: Vec<String> = ["[[A|b]]", "c,d", "", "e"].map(String::from).to_vec();
        assert_eq!(roundtrip_flow(&items), Some(items));
    }

    #[test]
    fn converts_unquoted_wikilinks() {
        let parsed =
            YamlLoader::load_from_str("a: [[X]]\nb:\n  - [[Y]]\n  - z\n").unwrap_or_default();
        let doc = &parsed[0];
        assert_eq!(
            to_property(&doc["a"], None),
            PropertyValue::Text("[[X]]".into())
        );
        assert_eq!(
            to_property(&doc["b"], None),
            PropertyValue::List(vec!["[[Y]]".into(), "z".into()])
        );
    }

    #[test]
    fn converts_scalars_with_raw_text() {
        assert_eq!(
            to_property(&Yaml::Integer(7), Some("007")),
            PropertyValue::Text("007".into())
        );
        assert_eq!(
            to_property(&Yaml::Integer(7), None),
            PropertyValue::Text("7".into())
        );
        assert_eq!(to_property(&Yaml::Null, None), PropertyValue::Null);
        assert_eq!(
            PropertyValue::Text("x".into()).to_list(),
            vec!["x".to_owned()]
        );
        assert_eq!(PropertyValue::Null.to_list(), Vec::<String>::new());
    }
}
