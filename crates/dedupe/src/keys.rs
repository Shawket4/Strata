//! Keys and prepared texts: what is stored per item and what a check looks up.
//!
//! All text goes through `text-normalize` first. On top of that this module adds the
//! duplicate-specific rules (documented here because the backend's SQL candidate generation
//! and the client's `SQLite` lookup must use the very same keys):
//!
//! - **Task-intent preambles** ("remind me to", "don't forget to", "فكرني", …) are removed from
//!   the start of captures and tasks ([`prepare_text`]), so "remind me to make Watanya's
//!   invoice" compares as "make watanya invoice".
//! - **Light English plural folding** in exact keys ([`fold_plural`]): `experiments` →
//!   `experiment`, `companies` → `company`. Applied to every kind except people and
//!   companies, whose names are proper nouns.
//! - **Tasks** append the recurrence rule (parts sorted) and the linked entities (normalised,
//!   sorted) to the exact key, so the same text with a different schedule or a different
//!   client is not an exact duplicate (it can still be a near one).
//! - **Phonetic keys** (`text-normalize` transliteration keys) are produced for people,
//!   companies and aliases only, and only when at least [`crate::PHONETIC_MIN_KEY_LEN`]
//!   consonant classes long. They generate candidates; they never decide on their own.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use text_normalize::{dedupe_key, normalize_for_search, transliteration_key};

use crate::check::{Thresholds, compatible_kinds};
use crate::{DedupeKind, Item, PHONETIC_MIN_KEY_LEN};

/// Leading phrases (already normalised) that announce a task rather than describe it.
/// Longer phrases come first so the longest match wins.
pub const TASK_PREAMBLES: &[&str] = &[
    "please remind me to",
    "remind me to",
    "remind me",
    "reminder to",
    "reminder",
    "do not forget to",
    "dont forget to",
    "i need to",
    "i have to",
    "need to",
    "have to",
    "todo",
    "to do",
    "فكرني اني",
    "فكرني ان",
    "فكرني",
    "ذكرني ان",
    "ذكرني",
    "ماتنساش",
    "متنساش",
];

/// Stored duplicate keys of one item (`dedupe_keys` rows, one per name for entities).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupeKeys {
    /// Kind of the item.
    pub kind: DedupeKind,
    /// Exact keys, one per distinct name (text first, then aliases), de-duplicated.
    pub exact_keys: Vec<String>,
    /// Prepared (normalised) texts for trigram similarity, one per distinct name.
    pub trigram_texts: Vec<String>,
    /// Transliteration keys (people, companies, aliases only), de-duplicated.
    pub phonetic_keys: Vec<String>,
}

/// What a store should look up to find the candidates for a new item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateQuery {
    /// Stored kinds that can be duplicates of the new item.
    pub kinds: Vec<DedupeKind>,
    /// Candidates whose exact key equals one of these.
    pub exact_keys: Vec<String>,
    /// Candidates whose trigram text has `similarity(stored, text) >= trigram_floor` for one
    /// of these texts.
    pub trigram_texts: Vec<String>,
    /// The lowest near threshold over the compatible kinds; a store may use a lower floor,
    /// never a higher one.
    pub trigram_floor: f32,
    /// Candidates whose phonetic key equals one of these.
    pub phonetic_keys: Vec<String>,
}

impl CandidateQuery {
    /// The lookup for `item` under `thresholds`.
    pub fn for_item(item: &Item, thresholds: &Thresholds) -> Self {
        let keys = item.keys();
        let kinds = compatible_kinds(item.kind).to_vec();
        let trigram_floor = kinds
            .iter()
            .map(|&k| thresholds.near_between(item.kind, k))
            .fold(1.0_f32, f32::min);
        Self {
            kinds,
            exact_keys: keys.exact_keys,
            trigram_texts: keys.trigram_texts,
            trigram_floor,
            phonetic_keys: keys.phonetic_keys,
        }
    }
}

/// Normalises `text` and removes a leading task-intent preamble for captures and tasks
/// (only when words remain after it).
pub fn prepare_text(kind: DedupeKind, text: &str) -> String {
    let normalized = normalize_for_search(text);
    if !matches!(kind, DedupeKind::Capture | DedupeKind::Task) {
        return normalized;
    }
    for preamble in TASK_PREAMBLES {
        if let Some(rest) = normalized.strip_prefix(preamble)
            && let Some(rest) = rest.strip_prefix(' ')
            && !rest.is_empty()
        {
            return rest.to_owned();
        }
    }
    normalized
}

/// Folds a regular English plural to its singular. Only pure ASCII words of four or more
/// letters are touched: `-ies` → `-y`, `-sses` → `-ss`, and a final `s` is dropped unless the
/// word ends in `ss`, `us` or `is` (`class`, `status`, `analysis`).
pub fn fold_plural(word: &str) -> String {
    if word.len() < 4 || !word.bytes().all(|b| b.is_ascii_lowercase()) {
        return word.to_owned();
    }
    if let Some(stem) = word.strip_suffix("ies")
        && stem.len() >= 2
    {
        return format!("{stem}y");
    }
    if let Some(stem) = word.strip_suffix("sses") {
        return format!("{stem}ss");
    }
    if word.ends_with("ss") || word.ends_with("us") || word.ends_with("is") {
        return word.to_owned();
    }
    word.strip_suffix('s').unwrap_or(word).to_owned()
}

fn folds_plurals(kind: DedupeKind) -> bool {
    !matches!(kind, DedupeKind::Person | DedupeKind::Company)
}

fn has_phonetic_keys(kind: DedupeKind) -> bool {
    matches!(
        kind,
        DedupeKind::Person | DedupeKind::Company | DedupeKind::Alias
    )
}

/// The exact key of one name of an item of `kind` (without the task facets).
pub fn name_key(kind: DedupeKind, name: &str) -> String {
    let key = dedupe_key(&prepare_text(kind, name));
    if !folds_plurals(kind) {
        return key;
    }
    let words: BTreeSet<String> = key.split(' ').map(fold_plural).collect();
    words.into_iter().collect::<Vec<_>>().join(" ")
}

/// Canonical form of an RRULE for keys: upper-cased, `RRULE:` prefix removed, parts sorted.
pub fn canonical_rrule(rrule: &str) -> String {
    let upper = rrule.trim().to_ascii_uppercase();
    let body = upper.strip_prefix("RRULE:").unwrap_or(&upper);
    let parts: BTreeSet<&str> = body
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    parts.into_iter().collect::<Vec<_>>().join(";")
}

/// The exact keys of `item` (one per distinct name). Task keys carry the recurrence and the
/// linked entities: `<text key> | <rrule> | <entities>`.
pub fn exact_keys(item: &Item) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in item.names() {
        let mut key = name_key(item.kind, name);
        if item.kind == DedupeKind::Task {
            let rrule = item
                .rrule
                .as_deref()
                .map(canonical_rrule)
                .unwrap_or_default();
            let entities: BTreeSet<String> = item
                .entities
                .iter()
                .map(|e| normalize_for_search(e))
                .filter(|e| !e.is_empty())
                .collect();
            let entities = entities.into_iter().collect::<Vec<_>>().join(",");
            key = format!("{key} | {rrule} | {entities}");
        }
        if !key.is_empty() && !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

pub(crate) fn keys_for(item: &Item) -> DedupeKeys {
    let mut trigram_texts: Vec<String> = Vec::new();
    let mut phonetic_keys: Vec<String> = Vec::new();
    for name in item.names() {
        let text = prepare_text(item.kind, name);
        if !text.is_empty() && !trigram_texts.contains(&text) {
            trigram_texts.push(text);
        }
        if has_phonetic_keys(item.kind) {
            let key = transliteration_key(name);
            if key.chars().count() >= PHONETIC_MIN_KEY_LEN && !phonetic_keys.contains(&key) {
                phonetic_keys.push(key);
            }
        }
    }
    DedupeKeys {
        kind: item.kind,
        exact_keys: exact_keys(item),
        trigram_texts,
        phonetic_keys,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_folding_rules() {
        let cases = [
            ("experiments", "experiment"),
            ("companies", "company"),
            ("classes", "class"),
            ("class", "class"),
            ("status", "status"),
            ("analysis", "analysis"),
            ("invoices", "invoice"),
            ("bus", "bus"),
            ("ties", "tie"),
            ("notes2", "notes2"),
            ("فواتير", "فواتير"),
        ];
        for (input, want) in cases {
            assert_eq!(fold_plural(input), want, "{input}");
        }
    }

    #[test]
    fn preamble_stripping() {
        assert_eq!(
            prepare_text(DedupeKind::Capture, "Remind me to make Watanya's invoice"),
            "make watanya invoice"
        );
        assert_eq!(
            prepare_text(DedupeKind::Task, "Don't forget to call Shady"),
            "call shady"
        );
        assert_eq!(
            prepare_text(DedupeKind::Capture, "فكّرني أعمل فاتورة وطنية"),
            "اعمل فاتوره وطنيه"
        );
        // Only the preamble: kept as is.
        assert_eq!(prepare_text(DedupeKind::Capture, "Remind me"), "remind me");
        // Not stripped for notes, nor in the middle of a word.
        assert_eq!(
            prepare_text(DedupeKind::Note, "Remind me to call"),
            "remind me to call"
        );
        assert_eq!(
            prepare_text(DedupeKind::Task, "reminders list"),
            "reminders list"
        );
    }

    #[test]
    fn rrule_canonical_form() {
        assert_eq!(
            canonical_rrule("RRULE:bymonthday=1;FREQ=MONTHLY"),
            "BYMONTHDAY=1;FREQ=MONTHLY"
        );
        assert_eq!(
            canonical_rrule("FREQ=MONTHLY;BYMONTHDAY=1;"),
            "BYMONTHDAY=1;FREQ=MONTHLY"
        );
    }

    #[test]
    fn task_exact_key_includes_rrule_and_entities() {
        let item = Item::task(
            Some("t-1"),
            "Make Watanya's ETA invoice",
            Some("FREQ=MONTHLY;BYMONTHDAY=1"),
            &["Watanya", "watanya"],
        );
        assert_eq!(
            exact_keys(&item),
            ["eta invoice make watanya | BYMONTHDAY=1;FREQ=MONTHLY | watanya"]
        );
    }

    #[test]
    fn entity_keys_cover_every_name() {
        let item = Item::entity(
            DedupeKind::Person,
            Some("p"),
            "Ahmed Samir",
            &["أحمد سمير", "Ahmed Samir", "A. Samir"],
        );
        let keys = item.keys();
        assert_eq!(keys.exact_keys, ["ahmed samir", "احمد سمير", "samir"]);
        assert_eq!(keys.trigram_texts, ["ahmed samir", "احمد سمير", "a samir"]);
        assert_eq!(keys.phonetic_keys, ["hmdsmr", "smr"]);
    }
}
