//! Content direction by the first strong character (Unicode bidi rule P2, PLAN §11 "per
//! paragraph/line"). Only the direction classes that matter for Strata's content are
//! distinguished: right-to-left scripts (Hebrew, Arabic, Syriac, Thaana, N'Ko, …) and letters of
//! every other script (left-to-right). Digits, punctuation, symbols and whitespace are neutral.
//! Markdown syntax that precedes the text (`- `, `## `, `> `, `[ ] `, `[[`) is neutral by
//! construction because it is punctuation.

use crate::view::model::TextDir;

/// Whether `c` is a strong right-to-left character (bidi classes R and AL).
fn is_rtl(c: char) -> bool {
    matches!(u32::from(c),
        0x0590..=0x08FF      // Hebrew, Arabic, Syriac, Arabic Supplement, Thaana, NKo, Samaritan, Mandaic, Arabic Extended
        | 0xFB1D..=0xFDFF    // Hebrew and Arabic presentation forms A
        | 0xFE70..=0xFEFF    // Arabic presentation forms B
        | 0x10800..=0x10FFF  // historic RTL scripts
        | 0x1E800..=0x1EFFF) // Mende Kikakui, Adlam, Arabic mathematical symbols
        && !is_rtl_neutral(c)
}

/// Characters inside the RTL blocks that are not strong (Arabic-Indic digits, marks,
/// punctuation used by both directions).
fn is_rtl_neutral(c: char) -> bool {
    matches!(u32::from(c),
        0x0591..=0x05BD     // Hebrew points
        | 0x0610..=0x061A   // Arabic marks
        | 0x064B..=0x065F   // tashkeel
        | 0x0660..=0x0669   // Arabic-Indic digits (AN)
        | 0x066B..=0x066C   // Arabic decimal/thousands separators
        | 0x0670            // superscript alef
        | 0x06D6..=0x06ED   // Quranic marks
        | 0x06F0..=0x06F9   // extended Arabic-Indic digits (EN)
        | 0x060C            // Arabic comma (CS)
    )
}

/// The direction of `text` by its first strong character.
pub fn dir_of(text: &str) -> TextDir {
    for c in text.chars() {
        if is_rtl(c) {
            return TextDir::Rtl;
        }
        if c.is_alphabetic() {
            return TextDir::Ltr;
        }
    }
    TextDir::Neutral
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_strong_character_wins() {
        assert_eq!(dir_of("خصم 10% for Acme"), TextDir::Rtl);
        assert_eq!(dir_of("Acme خصم"), TextDir::Ltr);
        assert_eq!(dir_of("- خصم"), TextDir::Rtl);
        assert_eq!(dir_of("## الفرضيات"), TextDir::Rtl);
        assert_eq!(dir_of("- [ ] 10% كلم أحمد"), TextDir::Rtl);
        assert_eq!(dir_of("١٢٣ - 2026"), TextDir::Neutral);
        assert_eq!(dir_of(""), TextDir::Neutral);
        assert_eq!(dir_of("[[أحمد سمير]] called"), TextDir::Rtl);
        assert_eq!(dir_of("\u{064E}a"), TextDir::Ltr);
    }
}
