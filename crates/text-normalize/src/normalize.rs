//! Search normalisation.
//!
//! [`normalize_for_search`] applies, in order:
//!
//! 1. **Unicode NFKC** — folds compatibility forms: Arabic presentation forms (`ﻻ` → `لا`),
//!    full-width Latin, ligatures (`ﬁ` → `fi`), superscripts.
//! 2. **Case folding** — `char::to_lowercase`, plus the full case folds `ß`/`ẞ` → `ss` and final
//!    sigma `ς` → `σ`.
//! 3. **Canonical decomposition (NFD)** so every accent and every Arabic hamza/madda is a
//!    separate combining mark, then per character:
//!    - drop Arabic tashkeel and Quranic marks (U+0610–U+061A, U+064B–U+065F, U+0670,
//!      U+0674, U+06D6–U+06ED, U+0898–U+089F, U+08CA–U+08FF) and tatweel (U+0640);
//!    - drop Latin/Greek/Cyrillic combining diacritics (U+0300–U+036F, U+1AB0–U+1AFF,
//!      U+1DC0–U+1DFF, U+20D0–U+20FF, U+FE20–U+FE2F), so `é` → `e`;
//!    - drop invisible format characters (bidi marks and embeddings, ZWJ/ZWNJ, BOM, soft hyphen,
//!      variation selectors), which are common in mixed-direction text;
//!    - unify letters: `أ إ آ ٱ` → `ا`, `ى ی` → `ي`, `ة` (and Urdu/Kurdish heh forms) → `ه`,
//!      `ؤ` → `و`, `ئ` → `ي`, `ک ڪ` → `ك`;
//!    - map Arabic-Indic (U+0660–U+0669) and Eastern Arabic-Indic (U+06F0–U+06F9) digits to
//!      ASCII;
//!    - apostrophes (`'`, `’`, `‘`, `ʼ`, `` ` ``) are removed, and an English possessive `'s`
//!      at the end of a word is removed entirely (`Watanya's` → `watanya`), so a query for the
//!      bare word finds the possessive and vice versa;
//!    - every other character that is not alphanumeric (punctuation including `، ؛ ؟`,
//!      symbols, emoji, whitespace, controls) becomes a word separator.
//! 4. **Canonical composition (NFC)** and whitespace collapsing: words are separated by exactly
//!    one ASCII space, with no leading or trailing space.
//!
//! The output only contains alphanumeric characters (plus combining marks of scripts other
//! than Latin/Greek/Cyrillic/Arabic, e.g. Devanagari signs) and single spaces. The function is
//! idempotent: `normalize_for_search(normalize_for_search(x)) == normalize_for_search(x)`.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Normalises text for search, alias matching and duplicate detection. See the module docs
/// for the exact rules.
///
/// ```
/// use text_normalize::normalize_for_search;
/// assert_eq!(normalize_for_search("أَحْمَد  سَمِيرة"), "احمد سميره");
/// assert_eq!(normalize_for_search("Watanya's ETA invoice!"), "watanya eta invoice");
/// assert_eq!(normalize_for_search("Café ١٢٣"), "cafe 123");
/// ```
pub fn normalize_for_search(input: &str) -> String {
    let mut folded = String::with_capacity(input.len());
    for c in input.nfkc() {
        fold_case(c, &mut folded);
    }

    let mut mapped = String::with_capacity(folded.len());
    for c in folded.nfd() {
        match classify(c) {
            Class::Keep(k) => mapped.push(k),
            Class::Mark => {
                // Keep marks of other scripts only when attached to a letter; a stray mark after
                // a separator carries no meaning.
                if mapped
                    .chars()
                    .next_back()
                    .is_some_and(|p| p.is_alphanumeric() || is_combining_mark(p))
                {
                    mapped.push(c);
                }
            }
            Class::Drop => {}
            Class::Apostrophe => mapped.push(APOSTROPHE),
            Class::Separator => mapped.push(' '),
        }
    }

    let without_apostrophes = remove_apostrophes(&mapped);
    let composed: String = without_apostrophes.nfc().collect();
    let mut out = String::with_capacity(composed.len());
    for word in composed.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Placeholder used between classification and apostrophe handling.
const APOSTROPHE: char = '\'';

enum Class {
    Keep(char),
    Mark,
    Drop,
    Apostrophe,
    Separator,
}

fn fold_case(c: char, out: &mut String) {
    match c {
        'ß' | 'ẞ' => out.push_str("ss"),
        'ς' => out.push('σ'),
        _ => out.extend(c.to_lowercase()),
    }
}

fn classify(c: char) -> Class {
    if is_arabic_mark(c) || c == '\u{0640}' || is_latin_mark(c) || is_invisible_format(c) {
        return Class::Drop;
    }
    if let Some(mapped) = unify_arabic_letter(c) {
        return Class::Keep(mapped);
    }
    if let Some(digit) = arabic_digit_to_ascii(c) {
        return Class::Keep(digit);
    }
    if matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '\u{02BC}' | '`') {
        return Class::Apostrophe;
    }
    if c.is_alphanumeric() {
        return Class::Keep(c);
    }
    if is_combining_mark(c) {
        return Class::Mark;
    }
    Class::Separator
}

/// Arabic tashkeel (harakat, tanween, shadda, sukun, hamza/madda marks), superscript alef and
/// Quranic annotation marks.
pub(crate) fn is_arabic_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{0674}'
            | '\u{06D6}'..='\u{06ED}'
            | '\u{0898}'..='\u{089F}'
            | '\u{08CA}'..='\u{08FF}'
    )
}

fn is_latin_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Invisible characters that must not split words: soft hyphen, Arabic letter mark, Mongolian
/// vowel separator, zero-width space/joiners, bidi marks/embeddings/isolates, word joiner and
/// invisible operators, variation selectors, BOM.
fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
    )
}

/// Letter unification for Arabic-script variants. Composed forms (`أ`, `ؤ`, `ۀ`, …) are listed
/// for clarity even though NFD already splits them into base letter + (dropped) mark.
fn unify_arabic_letter(c: char) -> Option<char> {
    Some(match c {
        'أ' | 'إ' | 'آ' | 'ٱ' | '\u{0672}' | '\u{0673}' | '\u{0675}' => 'ا',
        'ى' | 'ی' | 'ئ' => 'ي',
        'ة' | '\u{06C3}' | '\u{06D5}' | '\u{06C1}' | '\u{06BE}' | '\u{06C0}' | '\u{06C2}' => 'ه',
        'ؤ' => 'و',
        'ک' | 'ڪ' => 'ك',
        _ => return None,
    })
}

fn arabic_digit_to_ascii(c: char) -> Option<char> {
    let base = match c {
        '\u{0660}'..='\u{0669}' => 0x0660,
        '\u{06F0}'..='\u{06F9}' => 0x06F0,
        _ => return None,
    };
    char::from_digit(u32::from(c) - base, 10)
}

/// Removes apostrophes; a possessive `'s` (apostrophe after a letter/digit, followed by `s` and
/// then a word end) is removed together with its `s`.
fn remove_apostrophes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == APOSTROPHE {
            let after_word = i > 0 && chars[i - 1].is_alphanumeric();
            let possessive = after_word
                && chars.get(i + 1) == Some(&'s')
                && chars.get(i + 2).is_none_or(|n| !n.is_alphanumeric());
            i += if possessive { 2 } else { 1 };
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn possessive_only_at_word_end() {
        assert_eq!(remove_apostrophes("a's b"), "a b");
        assert_eq!(remove_apostrophes("a'sb"), "asb");
        assert_eq!(remove_apostrophes("'s"), "s");
        assert_eq!(remove_apostrophes("it's"), "it");
        assert_eq!(remove_apostrophes("don't"), "dont");
    }

    #[test]
    fn digits_map_exactly() {
        assert_eq!(arabic_digit_to_ascii('٠'), Some('0'));
        assert_eq!(arabic_digit_to_ascii('٩'), Some('9'));
        assert_eq!(arabic_digit_to_ascii('۰'), Some('0'));
        assert_eq!(arabic_digit_to_ascii('۹'), Some('9'));
        assert_eq!(arabic_digit_to_ascii('9'), None);
    }
}
