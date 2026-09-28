//! User settings as synced records (§7.5 Sync, §12.5): how a stored setting value becomes
//! [`SettingRecord`]s, and which records a change touches.
//!
//! The server stores each setting as one `MessagePack` value; devices receive
//! `SettingRecord { key, value }` with the value as text:
//!
//! - text → itself; boolean → `true` / `false`; integer → decimal; finite float → the
//!   shortest decimal that parses back to the same `f64` (`0.7`, `12.5`);
//! - a map with text keys (e.g. `preferences`) → one record per entry, keyed
//!   `<setting>.<entry>` ([`entry_key`]), whose value is the entry's scalar text; the map
//!   itself has no record, so removing an entry is a tombstone of its key;
//! - anything else (arrays, nil, binary, non-finite floats, nested maps or non-scalar entries)
//!   is not synced.
//!
//! A record key is looked up as a setting key first, then as `<setting>.<entry>` split at the
//! first `.` ([`split_entry_key`]).

use std::collections::BTreeMap;

use crate::changes::SettingRecord;

/// A stored setting value, decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    /// Text.
    Text(String),
    /// Boolean.
    Bool(bool),
    /// Integer (signed or unsigned `MessagePack` integers).
    Integer(i128),
    /// Float.
    Float(f64),
    /// Map with text keys, in key order.
    Map(BTreeMap<String, SettingValue>),
    /// Anything that is not synced.
    Other,
}

impl SettingValue {
    /// The text of a scalar value (`None` for maps and values that are not synced).
    pub fn scalar_text(&self) -> Option<String> {
        match self {
            Self::Text(s) => Some(s.clone()),
            Self::Bool(b) => Some(b.to_string()),
            Self::Integer(i) => Some(i.to_string()),
            Self::Float(f) if f.is_finite() => Some(f.to_string()),
            Self::Float(_) | Self::Map(_) | Self::Other => None,
        }
    }
}

/// The record key of entry `entry` of map setting `setting`.
pub fn entry_key(setting: &str, entry: &str) -> String {
    format!("{setting}.{entry}")
}

/// `(setting, entry)` of a record key that names a map entry (split at the first `.`).
pub fn split_entry_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('.')
        .filter(|(s, e)| !s.is_empty() && !e.is_empty())
}

/// The records of setting `key` holding `value`, keyed by record key.
pub fn records(key: &str, value: &SettingValue) -> BTreeMap<String, SettingRecord> {
    let mut out = BTreeMap::new();
    match value {
        SettingValue::Map(entries) => {
            for (entry, v) in entries {
                if let Some(text) = v.scalar_text() {
                    let k = entry_key(key, entry);
                    out.insert(
                        k.clone(),
                        SettingRecord {
                            key: k,
                            value: text,
                        },
                    );
                }
            }
        }
        scalar => {
            if let Some(text) = scalar.scalar_text() {
                out.insert(
                    key.to_owned(),
                    SettingRecord {
                        key: key.to_owned(),
                        value: text,
                    },
                );
            }
        }
    }
    out
}

/// The record for record key `record_key` of setting `key` holding `value`, if any.
pub fn record(key: &str, value: &SettingValue, record_key: &str) -> Option<SettingRecord> {
    records(key, value).remove(record_key)
}

/// A change to one synced record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordChange {
    /// The record is new or its value changed.
    Upsert(String),
    /// The record no longer exists (tombstone).
    Delete(String),
}

/// The record changes when setting `key` goes from `before` to `after` (`None` = absent),
/// in record-key order.
pub fn changes(
    key: &str,
    before: Option<&SettingValue>,
    after: Option<&SettingValue>,
) -> Vec<RecordChange> {
    let old = before.map(|v| records(key, v)).unwrap_or_default();
    let new = after.map(|v| records(key, v)).unwrap_or_default();
    let mut keys: Vec<&String> = old.keys().chain(new.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| match (old.get(k), new.get(k)) {
            (Some(a), Some(b)) if a == b => None,
            (_, Some(_)) => Some(RecordChange::Upsert(k.clone())),
            (Some(_), None) => Some(RecordChange::Delete(k.clone())),
            (None, None) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn rec(key: &str, value: &str) -> SettingRecord {
        SettingRecord {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    fn map(entries: &[(&str, SettingValue)]) -> SettingValue {
        SettingValue::Map(
            entries
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn scalars_are_one_record_with_their_text() {
        let cases = [
            (
                SettingValue::Text("Africa/Cairo".into()),
                Some("Africa/Cairo"),
            ),
            (SettingValue::Bool(true), Some("true")),
            (SettingValue::Bool(false), Some("false")),
            (SettingValue::Integer(-42), Some("-42")),
            (
                SettingValue::Integer(i128::from(u64::MAX)),
                Some("18446744073709551615"),
            ),
            (SettingValue::Float(0.7), Some("0.7")),
            (SettingValue::Float(12.5), Some("12.5")),
            (SettingValue::Float(f64::NAN), None),
            (SettingValue::Float(f64::INFINITY), None),
            (SettingValue::Other, None),
        ];
        for (value, text) in cases {
            let expected: BTreeMap<String, SettingRecord> = text
                .map(|t| ("k".to_owned(), rec("k", t)))
                .into_iter()
                .collect();
            assert_eq!(records("k", &value), expected, "{value:?}");
        }
    }

    #[test]
    fn maps_become_one_record_per_scalar_entry() {
        let prefs = map(&[
            ("theme", SettingValue::Text("dark".into())),
            ("compact", SettingValue::Bool(true)),
            ("font_scale", SettingValue::Float(1.25)),
            ("nested", map(&[("x", SettingValue::Bool(true))])),
            ("list", SettingValue::Other),
        ]);
        assert_eq!(
            records("preferences", &prefs)
                .into_values()
                .collect::<Vec<_>>(),
            vec![
                rec("preferences.compact", "true"),
                rec("preferences.font_scale", "1.25"),
                rec("preferences.theme", "dark"),
            ]
        );
        assert_eq!(
            record("preferences", &prefs, "preferences.theme"),
            Some(rec("preferences.theme", "dark"))
        );
        assert_eq!(record("preferences", &prefs, "preferences.nested"), None);
        assert_eq!(
            split_entry_key("preferences.font.size"),
            Some(("preferences", "font.size"))
        );
        assert_eq!(split_entry_key("timezone"), None);
        assert_eq!(split_entry_key(".x"), None);
        assert_eq!(split_entry_key("x."), None);
    }

    #[test]
    fn changes_list_upserts_and_tombstones_in_key_order() {
        let before = map(&[
            ("a", SettingValue::Text("1".into())),
            ("b", SettingValue::Text("2".into())),
            ("c", SettingValue::Text("3".into())),
        ]);
        let after = map(&[
            ("a", SettingValue::Text("1".into())),
            ("c", SettingValue::Integer(3)),
            ("d", SettingValue::Bool(false)),
        ]);
        // `c` has the same text ("3") in both: no change.
        assert_eq!(
            changes("p", Some(&before), Some(&after)),
            vec![
                RecordChange::Delete("p.b".into()),
                RecordChange::Upsert("p.d".into()),
            ]
        );
        assert_eq!(
            changes("tz", None, Some(&SettingValue::Text("UTC".into()))),
            vec![RecordChange::Upsert("tz".into())]
        );
        assert_eq!(
            changes("tz", Some(&SettingValue::Text("UTC".into())), None),
            vec![RecordChange::Delete("tz".into())]
        );
        assert_eq!(
            changes(
                "tz",
                Some(&SettingValue::Text("UTC".into())),
                Some(&SettingValue::Text("UTC".into()))
            ),
            vec![]
        );
    }
}
