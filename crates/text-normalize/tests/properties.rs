//! Property tests (PLAN §16.2): normalisation is idempotent, keys are invariant under
//! normalisation, and similarity is symmetric and bounded.

use proptest::prelude::*;
use text_normalize::{
    dedupe_key, normalize_for_search, raw_similarity, transliteration_key, trigram_similarity,
    trigrams,
};

/// Strings biased towards the characters this crate cares about: Arabic letters and marks,
/// digits of all three families, Latin with accents, punctuation, apostrophes, whitespace and
/// invisible bidi/format characters.
fn arabic_latin_text() -> impl Strategy<Value = String> {
    let pieces = prop_oneof![
        "[\u{0600}-\u{06FF}]{1,6}",
        "[\u{08A0}-\u{08FF}\u{FB50}-\u{FDFF}\u{FE70}-\u{FEFF}]{1,3}",
        "[a-zA-Z]{1,8}",
        "[À-ɏ]{1,4}",
        "[\u{0300}-\u{036F}]{1,2}",
        "[0-9٠-٩۰-۹]{1,4}",
        "[ \t\n.,;:!?'’`\"()\\-/،؛؟ـ]{1,3}",
        "[\u{200B}-\u{200F}\u{202A}-\u{202E}\u{2066}-\u{2069}\u{00AD}\u{FEFF}]",
        "'s ",
    ];
    prop::collection::vec(pieces, 0..12).prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn normalize_is_idempotent_on_arabic_latin(s in arabic_latin_text()) {
        let once = normalize_for_search(&s);
        prop_assert_eq!(normalize_for_search(&once), once);
    }

    #[test]
    fn normalize_is_idempotent_on_any_unicode(s in any::<String>()) {
        let once = normalize_for_search(&s);
        prop_assert_eq!(normalize_for_search(&once), once);
    }

    #[test]
    fn normalized_output_is_clean(s in arabic_latin_text()) {
        let out = normalize_for_search(&s);
        prop_assert!(!out.starts_with(' ') && !out.ends_with(' ') && !out.contains("  "));
        prop_assert!(out.chars().all(|c| c == ' ' || c.is_alphanumeric()), "{:?}", out);
    }

    #[test]
    fn keys_are_invariant_under_normalization(s in arabic_latin_text()) {
        let n = normalize_for_search(&s);
        prop_assert_eq!(dedupe_key(&n), dedupe_key(&s));
        prop_assert_eq!(transliteration_key(&n), transliteration_key(&s));
        prop_assert_eq!(trigrams(&n), trigrams(&s));
    }

    #[test]
    fn dedupe_key_ignores_word_order(words in prop::collection::vec("[a-z\u{0628}-\u{063A}]{1,6}", 1..6)) {
        let mut reversed = words.clone();
        reversed.reverse();
        prop_assert_eq!(dedupe_key(&words.join(" ")), dedupe_key(&reversed.join(" ")));
    }

    #[test]
    fn dedupe_key_is_idempotent_on_latin(s in "[a-zA-Z ',.]{0,40}") {
        let once = dedupe_key(&s);
        prop_assert_eq!(dedupe_key(&once), once);
    }

    #[test]
    fn similarity_is_symmetric_and_bounded(a in arabic_latin_text(), b in arabic_latin_text()) {
        let ab = trigram_similarity(&a, &b);
        prop_assert_eq!(ab.to_bits(), trigram_similarity(&b, &a).to_bits());
        prop_assert!((0.0..=1.0).contains(&ab));
    }

    #[test]
    fn similarity_to_self_is_one_unless_empty(a in arabic_latin_text()) {
        let expected: f32 = if trigrams(&a).is_empty() { 0.0 } else { 1.0 };
        prop_assert_eq!(trigram_similarity(&a, &a).to_bits(), expected.to_bits());
        let n = normalize_for_search(&a);
        prop_assert_eq!(raw_similarity(&n, &n).to_bits(), expected.to_bits());
    }
}
