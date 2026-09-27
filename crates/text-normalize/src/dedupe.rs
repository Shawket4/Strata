//! Exact-duplicate keys (§9.7, level "Exact").
//!
//! [`dedupe_key`] turns a title/text into the **sorted set of its content words**:
//!
//! 1. [`normalize_for_search`] (letter variants, tashkeel, case, spacing, punctuation, and the
//!    English possessive `'s`).
//! 2. The Arabic definite article `ال` is removed from the start of words that keep at least
//!    two letters without it (`الفاتوره` → `فاتوره`), mirroring the removal of English `the`.
//! 3. Stopwords ([`ENGLISH_STOPWORDS`], [`ARABIC_STOPWORDS`]) are removed — unless *every* word
//!    is a stopword, in which case all words are kept so the key is never empty for non-empty
//!    text.
//! 4. Remaining words are de-duplicated, sorted (by code point) and joined with one space.
//!
//! Word order, repetition, case, possessives and function words therefore do not matter,
//! while any difference in the set of content words does:
//!
//! ```
//! use text_normalize::dedupe_key;
//! assert_eq!(dedupe_key("Watanya's ETA invoice"), dedupe_key("watanya ETA invoice"));
//! assert_eq!(dedupe_key("ETA invoice for Watanya"), "eta invoice watanya");
//! assert_ne!(dedupe_key("Watanya ETA invoice"), dedupe_key("Watanya invoice"));
//! ```
//!
//! Stopword lists are deliberately minimal: only words that never distinguish two items.
//! Pronoun-like or ambiguous words are *not* listed — in particular `على` is kept, because it
//! normalises to `علي`, which is also the name Ali.
//!
//! For tasks the caller combines this key with the recurrence rule and linked entities
//! (§9.7); this function only handles text.

use std::collections::BTreeSet;

use crate::normalize_for_search;

/// English stopwords removed by [`dedupe_key`] (already in normalised form).
pub const ENGLISH_STOPWORDS: &[&str] = &[
    "a", "an", "and", "at", "for", "in", "of", "on", "the", "to", "with",
];

/// Arabic (MSA + Egyptian) stopwords removed by [`dedupe_key`], in normalised form:
/// `في` (in), `من` (from), `عن` (about), `الي` (= `إلى`, to), `مع` (with), `و` (and),
/// `بتاع` / `بتاعه` (= `بتاعة`) / `بتوع` (Egyptian "of/belonging to").
pub const ARABIC_STOPWORDS: &[&str] =
    &["في", "من", "عن", "الي", "مع", "و", "بتاع", "بتاعه", "بتوع"];

/// Returns the exact-duplicate key of `input`. See the module docs for the rules.
pub fn dedupe_key(input: &str) -> String {
    let normalized = normalize_for_search(input);
    let words: Vec<&str> = normalized
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(strip_article)
        .collect();

    let content: BTreeSet<&str> = words.iter().copied().filter(|w| !is_stopword(w)).collect();
    let chosen = if content.is_empty() {
        words.into_iter().collect()
    } else {
        content
    };
    chosen.into_iter().collect::<Vec<_>>().join(" ")
}

fn is_stopword(word: &str) -> bool {
    ENGLISH_STOPWORDS.contains(&word) || ARABIC_STOPWORDS.contains(&word)
}

/// Removes a leading Arabic definite article when at least two letters remain.
fn strip_article(word: &str) -> &str {
    match word.strip_prefix("ال") {
        Some(rest) if rest.chars().count() >= 2 => rest,
        _ => word,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_needs_two_remaining_letters() {
        assert_eq!(strip_article("الفاتوره"), "فاتوره");
        assert_eq!(strip_article("الي"), "الي");
        assert_eq!(strip_article("ال"), "ال");
        assert_eq!(strip_article("الله"), "له");
    }

    #[test]
    fn stopword_lists_are_normalised() {
        for w in ENGLISH_STOPWORDS.iter().chain(ARABIC_STOPWORDS) {
            assert_eq!(normalize_for_search(w), *w);
        }
    }
}
