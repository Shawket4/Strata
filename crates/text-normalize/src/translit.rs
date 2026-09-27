//! Coarse Arabic↔Latin phonetic key for alias *candidate* matching (§6.7, §7.4
//! `entity_aliases.alias_normalized`).
//!
//! The key is a **consonant skeleton**: vowels are dropped and each consonant maps to a
//! phonetic class shared by both scripts, so common spellings of the same name collide:
//!
//! ```
//! use text_normalize::transliteration_key;
//! assert_eq!(transliteration_key("أحمد سمير"), "hmdsmr");
//! assert_eq!(transliteration_key("Ahmed Samir"), "hmdsmr");
//! assert_eq!(transliteration_key("Ahmad Sameer"), "hmdsmr");
//! ```
//!
//! It is deliberately lossy and only proposes candidates; the final decision uses aliases,
//! context and embeddings (§6.7). Scheme, applied to [`normalize_for_search`] output word by
//! word, with the word keys concatenated **without separators** (so spacing differences such as
//! `عبد الرحمن` / `Abdelrahman` do not matter):
//!
//! | Class | Arabic | Latin |
//! |---|---|---|
//! | *(dropped)* | `ا ء ع و ي` | `a e i o u y w`; Franco-Arabic digits `2 3` |
//! | `b` | `ب پ` | `b p` |
//! | `t` | `ت ط ث` | `t`, `th` |
//! | `j` | `ج گ` | `j`, `g` (Egyptian `ج` is /g/) |
//! | `h` | `ح ه` | `h`; Franco `7` |
//! | `k` | `خ ق ك` | `k q`, `kh`, `c` (not before `e i y`); Franco `5 9` |
//! | `d` | `د ض` | `d` |
//! | `z` | `ذ ز ظ` | `z`, `dh` (Egyptian `ذ ظ` are /z/) |
//! | `r` `l` `m` `n` | `ر ل م ن` | `r l m n` |
//! | `s` | `س ص` | `s`, `c` before `e i y` |
//! | `x` | `ش` | `sh ch` |
//! | `g` | `غ` | `gh`; Franco `8` |
//! | `f` | `ف ڤ` | `f v ph` |
//! | *(two classes)* `ks` | | `x` |
//!
//! Further rules:
//! - Runs of the same class collapse (`mm`, `ّ`-less `محمد` vs `Mohammed` → `mhmd`).
//! - A word-final `h` class is dropped (ta marbuta / final `-ah`: `فاطمة`, `Fatmah`, `Fatma`
//!   → `ftm`).
//! - The article is ignored: Arabic `ال` at the start of a word (when two letters remain),
//!   standalone Latin `al`/`el`/`ul`, and the `abd-el`/`abd-ul`/`abd-al`/`abd-ol`/`abd-il`
//!   and `عبدال` compounds (`Abdelrahman`, `Abd El Rahman`, `عبد الرحمن` → `bdrhmn`).
//! - Franco-Arabic digits are only interpreted inside words that contain Latin letters;
//!   other digits, and letters of other scripts, are kept unchanged.
//!
//! Known limitations: `ث` is always `t` (Egyptian `Osman` for `عثمان` does not collide), `ق`
//! pronounced as a glottal stop (`2`) does not collide with `k`, and vowels never matter
//! (`Samir` and `Samar` share `smr`).

use crate::normalize_for_search;

/// Returns the coarse phonetic consonant-skeleton key of `input`. See the module docs.
pub fn transliteration_key(input: &str) -> String {
    let normalized = normalize_for_search(input);
    let mut key = String::with_capacity(normalized.len());
    for word in normalized.split(' ').filter(|w| !w.is_empty()) {
        if matches!(word, "al" | "el" | "ul") {
            continue;
        }
        key.push_str(&word_key(&strip_article(word)));
    }
    key
}

fn strip_article(word: &str) -> String {
    if let Some(rest) = word.strip_prefix("عبدال")
        && rest.chars().count() >= 2
    {
        return format!("عبد{rest}");
    }
    if let Some(rest) = word.strip_prefix("ال")
        && rest.chars().count() >= 2
    {
        return rest.to_owned();
    }
    if let Some(rest) = word.strip_prefix("abd") {
        for joiner in ["el", "al", "ul", "ol", "il"] {
            if let Some(tail) = rest.strip_prefix(joiner) {
                return format!("abd{tail}");
            }
        }
    }
    word.to_owned()
}

fn word_key(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let has_latin = chars.iter().any(char::is_ascii_lowercase);
    let mut classes: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c.is_ascii_lowercase() {
            let (class, consumed) = latin_class(c, next);
            classes.extend(class.chars());
            i += consumed;
            continue;
        }
        if let Some(class) = franco_class(c).filter(|_| has_latin) {
            classes.extend(class.chars());
        } else if let Some(class) = arabic_class(c) {
            classes.extend(class.chars());
        } else {
            classes.push(c);
        }
        i += 1;
    }
    classes.dedup();
    if classes.len() > 1 && classes.last() == Some(&'h') {
        classes.pop();
    }
    classes.into_iter().collect()
}

/// Class of a Latin letter, with the number of characters consumed (2 for digraphs).
fn latin_class(c: char, next: Option<char>) -> (&'static str, usize) {
    if next == Some('h') {
        let digraph = match c {
            'k' => Some("k"),
            'g' => Some("g"),
            's' | 'c' => Some("x"),
            't' => Some("t"),
            'd' => Some("z"),
            'p' => Some("f"),
            _ => None,
        };
        if let Some(class) = digraph {
            return (class, 2);
        }
    }
    let class = match c {
        'b' | 'p' => "b",
        't' => "t",
        'd' => "d",
        'j' | 'g' => "j",
        'h' => "h",
        'c' if matches!(next, Some('e' | 'i' | 'y')) => "s",
        'k' | 'q' | 'c' => "k",
        'x' => "ks",
        's' => "s",
        'z' => "z",
        'f' | 'v' => "f",
        'l' => "l",
        'm' => "m",
        'n' => "n",
        'r' => "r",
        // a e i o u w y
        _ => "",
    };
    (class, 1)
}

/// Franco-Arabic ("Arabizi") digits used as letters; `None` for other characters.
fn franco_class(c: char) -> Option<&'static str> {
    Some(match c {
        '2' | '3' => "",
        '5' | '9' => "k",
        '7' => "h",
        '8' => "g",
        _ => return None,
    })
}

/// Class of a normalised Arabic letter; `None` for characters outside the table.
fn arabic_class(c: char) -> Option<&'static str> {
    Some(match c {
        'ا' | 'ء' | 'ع' | 'و' | 'ي' => "",
        'ب' | 'پ' => "b",
        'ت' | 'ط' | 'ث' => "t",
        'ج' | 'گ' => "j",
        'ح' | 'ه' => "h",
        'خ' | 'ق' | 'ك' => "k",
        'د' | 'ض' => "d",
        'ذ' | 'ز' | 'ظ' => "z",
        'ر' => "r",
        'س' | 'ص' => "s",
        'ش' => "x",
        'غ' => "g",
        'ف' | 'ڤ' => "f",
        'ل' => "l",
        'م' => "m",
        'ن' => "n",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digraphs_consume_two_chars() {
        assert_eq!(latin_class('s', Some('h')), ("x", 2));
        assert_eq!(latin_class('c', Some('h')), ("x", 2));
        assert_eq!(latin_class('c', Some('e')), ("s", 1));
        assert_eq!(latin_class('c', Some('a')), ("k", 1));
        assert_eq!(latin_class('h', Some('h')), ("h", 1));
    }

    #[test]
    fn franco_digits_only_inside_latin_words() {
        assert_eq!(word_key("3amr"), "mr");
        assert_eq!(word_key("7amada"), "hmd");
        assert_eq!(word_key("2024"), "2024");
    }

    #[test]
    fn single_h_is_kept() {
        assert_eq!(word_key("ه"), "h");
        assert_eq!(word_key("taha"), "t");
    }
}
