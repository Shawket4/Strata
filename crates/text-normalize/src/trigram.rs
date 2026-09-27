//! Trigram similarity with PostgreSQL `pg_trgm` semantics (§9.7 "Near"), so the client core's
//! offline near-duplicate check returns exactly the score the server computes with
//! `similarity(trigram_text, $1)`.
//!
//! The server stores [`normalize_for_search`] output in `dedupe_keys.trigram_text` and
//! `entity_aliases.alias_normalized`; [`trigrams`] and [`trigram_similarity`] normalise their
//! input first, so `trigram_similarity(a, b)` equals
//! `similarity(normalize_for_search(a), normalize_for_search(b))` in PostgreSQL.
//! [`raw_trigrams`] / [`raw_similarity`] skip normalisation (use them for text that is already
//! normalised).
//!
//! `pg_trgm` algorithm (PostgreSQL 16, `trgm_op.c`, default build with `KEEPONLYALNUM`,
//! `IGNORECASE` and `DIVUNION`):
//! - words are maximal runs of alphanumeric characters; everything else separates words;
//! - each word is lower-cased and padded with two spaces in front and one behind
//!   (`"  word "`), and every run of three consecutive characters is a trigram;
//! - the trigram set is de-duplicated;
//! - `similarity = |A ∩ B| / (|A| + |B| - |A ∩ B|)` computed in `float4`, and `0` when either
//!   set is empty.
//!
//! Parity caveats (documented, covered by the parity tests):
//! - The server database must use a UTF-8 `LC_CTYPE` other than plain `C`/`POSIX` (e.g.
//!   `C.UTF-8`, `en_US.UTF-8`); under plain `C`, PostgreSQL does not treat Arabic letters as
//!   word characters.
//! - PostgreSQL stores trigrams of multi-byte characters as 3-byte CRC hashes; a hash
//!   collision could in theory make it count one more shared trigram. Normalised text only
//!   contains letters and digits, for which Rust's `char::is_alphanumeric` agrees with glibc.

use std::collections::BTreeSet;

use crate::normalize_for_search;

/// The sorted, de-duplicated `pg_trgm` trigrams of `normalize_for_search(input)`.
///
/// ```
/// use text_normalize::trigrams;
/// assert_eq!(trigrams("Cat!"), ["  c", " ca", "at ", "cat"]);
/// ```
pub fn trigrams(input: &str) -> Vec<String> {
    raw_trigrams(&normalize_for_search(input))
}

/// `pg_trgm` `similarity()` of the normalised inputs, in `0.0..=1.0`.
///
/// ```
/// use text_normalize::trigram_similarity;
/// assert_eq!(trigram_similarity("word", "Word!"), 1.0);
/// assert_eq!(trigram_similarity("", "word"), 0.0);
/// ```
pub fn trigram_similarity(a: &str, b: &str) -> f32 {
    raw_similarity(&normalize_for_search(a), &normalize_for_search(b))
}

/// The sorted, de-duplicated `pg_trgm` trigrams of `input` exactly as given (only the
/// lower-casing `pg_trgm` itself performs).
pub fn raw_trigrams(input: &str) -> Vec<String> {
    trigram_set(input).into_iter().collect()
}

/// `pg_trgm` `similarity()` of `a` and `b` exactly as given.
pub fn raw_similarity(a: &str, b: &str) -> f32 {
    let left = trigram_set(a);
    let right = trigram_set(b);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let common = left.intersection(&right).count();
    let union = left.len() + right.len() - common;
    // Same float4 arithmetic as `cnt_sml`: convert both counts, then divide.
    #[expect(
        clippy::cast_precision_loss,
        reason = "trigram counts are far below 2^24, so the conversion is exact"
    )]
    let (common, union) = (common as f32, union as f32);
    common / union
}

fn trigram_set(input: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for word in input.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let mut padded: Vec<char> = vec![' ', ' '];
        padded.extend(word.chars().flat_map(char::to_lowercase));
        padded.push(' ');
        for window in padded.windows(3) {
            set.insert(window.iter().collect());
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_letter_word_has_two_trigrams() {
        assert_eq!(raw_trigrams("a"), ["  a", " a "]);
    }

    #[test]
    fn repeated_words_count_once() {
        assert_eq!(raw_trigrams("ab ab"), raw_trigrams("ab"));
        assert_eq!(raw_trigrams("ab"), ["  a", " ab", "ab "]);
    }

    #[test]
    fn raw_lowercases_and_splits_on_punctuation() {
        assert_eq!(raw_trigrams("A_b"), ["  a", "  b", " a ", " b "]);
    }
}
