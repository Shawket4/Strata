//! Pairwise scores between two items.

use serde::{Deserialize, Serialize};
use text_normalize::{raw_similarity, transliteration_key};

use crate::keys::{exact_keys, prepare_text};
use crate::{DedupeKind, Item};

/// Minimum number of consonant classes a transliteration key needs before it may propose a
/// candidate (`Ali` → `l` would collide with far too much).
pub const PHONETIC_MIN_KEY_LEN: usize = 3;

/// Same-script names with equal transliteration keys are near duplicates only when their
/// spellings are at least this similar ([`edit_similarity`]): `Ahmed Sameer` / `Ahmed Samir`
/// = 0.83 passes, while the key alone would also pair `Samir` with `Somar`.
pub const PHONETIC_MIN_SPELLING_SIMILARITY: f32 = 0.75;

/// Score given to a cross-script pair (Arabic vs Latin) whose transliteration keys are equal.
/// Spelling cannot be compared across scripts, so the key is the only evidence; the score is
/// fixed at the spelling bar so such pairs rank below every stronger same-script match.
pub const PHONETIC_CROSS_SCRIPT_SCORE: f32 = PHONETIC_MIN_SPELLING_SIMILARITY;

/// How two items compare.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairScore {
    /// An exact key is shared.
    pub exact: bool,
    /// Best `pg_trgm` similarity over all name pairs of the prepared texts.
    pub trigram: f32,
    /// Best confirmed phonetic score over all name pairs (people, companies, aliases), if
    /// any pair has equal transliteration keys and passes the spelling rule.
    pub phonetic: Option<f32>,
}

impl PairScore {
    /// The overall score in `0.0..=1.0`: `1.0` for exact, else the best of trigram and
    /// phonetic.
    pub fn score(&self) -> f32 {
        if self.exact {
            1.0
        } else {
            self.trigram.max(self.phonetic.unwrap_or(0.0))
        }
    }
}

fn phonetic_kind(kind: DedupeKind) -> bool {
    matches!(
        kind,
        DedupeKind::Person | DedupeKind::Company | DedupeKind::Alias
    )
}

/// Levenshtein similarity of two strings by `char`: `1 - distance / max(len)`; `1.0` when
/// both are empty.
pub fn edit_similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(ca != cb);
            cur[j + 1] = substitution.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "name lengths are far below 2^24, so the conversion is exact"
    )]
    let (distance, longest) = (prev[b.len()] as f32, longest as f32);
    1.0 - distance / longest
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Latin,
    Arabic,
    Other,
}

fn script(normalized: &str) -> Script {
    let latin = normalized.chars().any(|c| c.is_ascii_alphabetic());
    let arabic = normalized
        .chars()
        .any(|c| ('\u{0600}'..='\u{06ff}').contains(&c));
    match (latin, arabic) {
        (true, false) => Script::Latin,
        (false, true) => Script::Arabic,
        _ => Script::Other,
    }
}

fn phonetic_score(a: &str, b: &str, a_text: &str, b_text: &str) -> Option<f32> {
    let key = transliteration_key(a);
    if key.chars().count() < PHONETIC_MIN_KEY_LEN || key != transliteration_key(b) {
        return None;
    }
    let (sa, sb) = (script(a_text), script(b_text));
    if sa != sb && sa != Script::Other && sb != Script::Other {
        return Some(PHONETIC_CROSS_SCRIPT_SCORE);
    }
    let similarity = edit_similarity(a_text, b_text);
    (similarity >= PHONETIC_MIN_SPELLING_SIMILARITY).then_some(similarity)
}

/// Scores `a` against `b`. Symmetric: `score_pair(a, b) == score_pair(b, a)`.
pub fn score_pair(a: &Item, b: &Item) -> PairScore {
    let a_keys = exact_keys(a);
    let exact = exact_keys(b).iter().any(|k| a_keys.contains(k));
    let phonetic_pair = phonetic_kind(a.kind) && phonetic_kind(b.kind);
    let mut trigram = 0.0_f32;
    let mut phonetic: Option<f32> = None;
    for an in a.names() {
        let at = prepare_text(a.kind, an);
        for bn in b.names() {
            let bt = prepare_text(b.kind, bn);
            trigram = trigram.max(raw_similarity(&at, &bt));
            if phonetic_pair && let Some(p) = phonetic_score(an, bn, &at, &bt) {
                phonetic = Some(phonetic.map_or(p, |q| q.max(p)));
            }
        }
    }
    PairScore {
        exact,
        trigram,
        phonetic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_similarity_values() {
        assert!((edit_similarity("ahmed sameer", "ahmed samir") - (1.0 - 2.0 / 12.0)).abs() < 1e-6);
        assert_eq!(edit_similarity("", ""), 1.0);
        assert_eq!(edit_similarity("abc", ""), 0.0);
        assert_eq!(edit_similarity("وطنيه", "وطنيه"), 1.0);
    }

    #[test]
    fn scripts() {
        assert_eq!(script("ahmed"), Script::Latin);
        assert_eq!(script("احمد"), Script::Arabic);
        assert_eq!(script("احمد ahmed"), Script::Other);
        assert_eq!(script("123"), Script::Other);
    }
}
