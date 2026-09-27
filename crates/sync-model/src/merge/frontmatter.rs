//! Key-by-key frontmatter merge.
//!
//! Rules (PLAN §6.4, D19):
//! - A key changed on one side only takes that side's value (a deleted key stays deleted).
//! - Both sides changed a key identically: one copy.
//! - **List keys** Strata knows (`aliases`, `tags`, every relation key) are merged as sets
//!   with deletions respected: ours' list, minus the entries theirs removed, plus the entries
//!   theirs added (in theirs' order). A scalar is read as a one-entry list, an empty value as
//!   an empty list.
//! - `updated` changed on both sides takes the later timestamp.
//! - Any other key changed differently on both sides is a conflict; the merged frontmatter
//!   keeps ours' value until the conflict is resolved.
//! - Unknown keys are preserved in their original order (the result is built on ours'
//!   frontmatter, so untouched entries keep their exact bytes).
//! - A value that is not a flat Obsidian property (nested YAML) is compared and written as
//!   its raw entry text: taking it from theirs writes theirs' entry verbatim (in ours' line
//!   endings, `Frontmatter::set_raw_entry`). Only an entry that cannot be written without
//!   changing how other keys read is reported as a conflict.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use vault_format::frontmatter::ValueShape;
use vault_format::{Document, Frontmatter, KnownKey, PropertyValue};

use super::{AutoResolution, AutoResolved, Location};

/// A frontmatter value on one side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FmValue {
    /// The key is not there.
    Absent,
    /// `key:` with no value.
    Null,
    /// A scalar.
    Text(String),
    /// A flat list.
    List(Vec<String>),
    /// A nested YAML value, as its raw entry text (key line and continuation lines).
    Raw(String),
}

impl FmValue {
    pub(crate) fn read(fm: Option<&Frontmatter>, key: &str) -> Self {
        let Some(fm) = fm else {
            return Self::Absent;
        };
        match fm.get(key) {
            None => Self::Absent,
            Some(PropertyValue::Null) => Self::Null,
            Some(PropertyValue::Text(s)) => Self::Text(s.clone()),
            Some(PropertyValue::List(v)) => Self::List(v.clone()),
            Some(PropertyValue::Other) => {
                Self::Raw(fm.raw_entry(key).unwrap_or_default().to_owned())
            }
        }
    }

    fn as_list(&self) -> Option<Vec<String>> {
        match self {
            Self::Absent | Self::Null => Some(Vec::new()),
            Self::Text(s) if s.is_empty() => Some(Vec::new()),
            Self::Text(s) => Some(vec![s.clone()]),
            Self::List(v) => Some(v.clone()),
            Self::Raw(_) => None,
        }
    }

    /// Writes this value to `fm` (`Absent` removes the key).
    pub(crate) fn write(&self, fm: &mut Frontmatter, key: &str) -> Result<(), ()> {
        let r = match self {
            Self::Absent => fm.remove(key).map(|_| ()),
            Self::Null => fm.set(key, PropertyValue::Null),
            Self::Text(s) => fm.set(key, PropertyValue::Text(s.clone())),
            Self::List(v) => fm.set(key, PropertyValue::List(v.clone())),
            Self::Raw(raw) => fm.set_raw_entry(key, raw),
        };
        r.map_err(|_| ())
    }
}

/// A key both sides changed differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FmConflict {
    pub key: String,
    pub base: FmValue,
    pub ours: FmValue,
    pub theirs: FmValue,
    /// Raw entries for display.
    pub base_raw: String,
    pub ours_raw: String,
    pub theirs_raw: String,
}

/// Result of a frontmatter merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FmMerge {
    /// Frontmatter block as it should be written (ours' values for conflicts); the BOM is
    /// merged separately.
    pub prefix: String,
    pub conflicts: Vec<FmConflict>,
    pub auto: Vec<AutoResolved>,
}

fn is_list_key(key: &str) -> bool {
    KnownKey::from_name(key)
        .is_some_and(|k| matches!(k.shape(), ValueShape::List | ValueShape::LinkList))
}

/// Set merge with deletions respected (see module docs).
pub(crate) fn merge_list(base: &[String], ours: &[String], theirs: &[String]) -> Vec<String> {
    let removed_by_theirs: Vec<&String> = base.iter().filter(|x| !theirs.contains(x)).collect();
    let mut out: Vec<String> = ours
        .iter()
        .filter(|x| !removed_by_theirs.contains(x))
        .cloned()
        .collect();
    for x in theirs {
        if !base.contains(x) && !out.contains(x) {
            out.push(x.clone());
        }
    }
    out
}

fn later_timestamp(ours: &FmValue, theirs: &FmValue) -> Option<FmValue> {
    let (FmValue::Text(o), FmValue::Text(t)) = (ours, theirs) else {
        return None;
    };
    let (od, td) = (
        DateTime::parse_from_rfc3339(o).ok()?,
        DateTime::parse_from_rfc3339(t).ok()?,
    );
    Some(if td > od {
        theirs.clone()
    } else {
        ours.clone()
    })
}

enum KeyOutcome {
    KeepOurs,
    Take(FmValue, AutoResolution),
    Conflict,
}

fn merge_key(key: &str, b: &FmValue, o: &FmValue, t: &FmValue) -> KeyOutcome {
    if o == t || t == b {
        return KeyOutcome::KeepOurs;
    }
    if o == b {
        return KeyOutcome::Take(t.clone(), AutoResolution::TookTheirs);
    }
    if is_list_key(key)
        && let (Some(bl), Some(ol), Some(tl)) = (b.as_list(), o.as_list(), t.as_list())
    {
        let merged = merge_list(&bl, &ol, &tl);
        let value = match (merged.is_empty(), o) {
            (true, FmValue::Absent) => FmValue::Absent,
            (true, FmValue::Null) => FmValue::Null,
            _ => FmValue::List(merged),
        };
        return KeyOutcome::Take(value, AutoResolution::ListUnion);
    }
    if key == KnownKey::Updated.as_str()
        && let Some(v) = later_timestamp(o, t)
    {
        return KeyOutcome::Take(v, AutoResolution::LatestTimestamp);
    }
    KeyOutcome::Conflict
}

fn raw(fm: Option<&Frontmatter>, key: &str) -> String {
    fm.and_then(|f| f.raw_entry(key))
        .unwrap_or_default()
        .to_owned()
}

/// Merges the frontmatter of three parsed documents. Returns `None` when any frontmatter is
/// invalid YAML (the caller then merges the whole file as text).
pub(crate) fn merge_frontmatter(
    base: &Document,
    ours: &Document,
    theirs: &Document,
) -> Option<FmMerge> {
    if [base, ours, theirs]
        .iter()
        .any(|d| d.frontmatter().is_some_and(|f| f.error().is_some()))
    {
        return None;
    }
    let (bf, of, tf) = (base.frontmatter(), ours.frontmatter(), theirs.frontmatter());
    let mut keys: Vec<String> = Vec::new();
    for fm in [of, tf, bf].into_iter().flatten() {
        for k in fm.keys() {
            if !keys.iter().any(|x| x == k) {
                keys.push(k.to_owned());
            }
        }
    }
    let mut result = ours.clone();
    result.set_body("");
    let mut conflicts = Vec::new();
    let mut auto = Vec::new();
    for key in keys {
        let (b, o, t) = (
            FmValue::read(bf, &key),
            FmValue::read(of, &key),
            FmValue::read(tf, &key),
        );
        let conflict = |conflicts: &mut Vec<FmConflict>| {
            conflicts.push(FmConflict {
                key: key.clone(),
                base: b.clone(),
                ours: o.clone(),
                theirs: t.clone(),
                base_raw: raw(bf, &key),
                ours_raw: raw(of, &key),
                theirs_raw: raw(tf, &key),
            });
        };
        match merge_key(&key, &b, &o, &t) {
            KeyOutcome::KeepOurs => {
                if o != b {
                    auto.push(AutoResolved {
                        location: Location::Frontmatter { key: key.clone() },
                        resolution: if o == t {
                            AutoResolution::Identical
                        } else {
                            AutoResolution::TookOurs
                        },
                    });
                }
            }
            KeyOutcome::Take(value, resolution) => {
                if value == o || value.write(result.frontmatter_mut(), &key).is_ok() {
                    auto.push(AutoResolved {
                        location: Location::Frontmatter { key: key.clone() },
                        resolution,
                    });
                } else {
                    conflict(&mut conflicts);
                }
            }
            KeyOutcome::Conflict => conflict(&mut conflicts),
        }
    }
    Some(FmMerge {
        prefix: result.render(),
        conflicts,
        auto,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn list_merge_respects_deletions_on_both_sides() {
        let base = v(&["a", "b", "c"]);
        let ours = v(&["a", "c", "o"]); // removed b, added o
        let theirs = v(&["b", "c", "t"]); // removed a, added t
        assert_eq!(merge_list(&base, &ours, &theirs), v(&["c", "o", "t"]));
        // Both add the same entry: once.
        assert_eq!(merge_list(&v(&[]), &v(&["x"]), &v(&["x"])), v(&["x"]));
    }

    #[test]
    fn list_keys_are_known_list_shapes() {
        assert!(is_list_key("tags"));
        assert!(is_list_key("related"));
        assert!(is_list_key("works-at"));
        assert!(!is_list_key("title"));
        assert!(!is_list_key("my-list"));
    }
}
