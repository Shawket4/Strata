//! Table tests for `normalize_for_search` (PLAN §16.2 "Arabic normalisation cases").

use text_normalize::normalize_for_search;

/// (input, expected output, what the case covers)
const CASES: &[(&str, &str, &str)] = &[
    // --- alef forms ---
    ("أحمد", "احمد", "alef with hamza above"),
    ("إسلام", "اسلام", "alef with hamza below"),
    ("آمال", "امال", "alef with madda"),
    ("ٱلرحمن", "الرحمن", "alef wasla"),
    ("ٲ", "ا", "alef with wavy hamza above"),
    ("\u{0627}\u{0653}مال", "امال", "decomposed alef + madda"),
    (
        "\u{0627}\u{0654}حمد",
        "احمد",
        "decomposed alef + hamza above",
    ),
    ("ﺃﺣﻤﺪ", "احمد", "presentation forms (NFKC)"),
    ("ﻻ", "لا", "lam-alef ligature"),
    ("ﻷ", "لا", "lam-alef with hamza ligature"),
    // --- ta marbuta / heh ---
    ("مدرسة", "مدرسه", "ta marbuta"),
    (
        "فاطمة الزهراء",
        "فاطمه الزهراء",
        "ta marbuta mid-phrase, standalone hamza kept",
    ),
    ("ﺓ", "ه", "ta marbuta presentation form"),
    ("ۀ", "ه", "heh with yeh above"),
    ("ھ", "ه", "heh doachashmee"),
    // --- ya / alef maqsura / hamza carriers ---
    ("مستشفى", "مستشفي", "alef maqsura"),
    ("على", "علي", "alef maqsura in a preposition"),
    ("علی", "علي", "Persian yeh"),
    ("مسئول", "مسيول", "yeh with hamza"),
    ("مؤتمر", "موتمر", "waw with hamza"),
    ("شئون", "شيون", "yeh with hamza mid-word"),
    ("کتاب", "كتاب", "Persian keheh"),
    // --- tashkeel ---
    ("مُحَمَّد", "محمد", "damma, fatha, shadda"),
    ("كِتَابٌ", "كتاب", "kasra, tanween damm"),
    ("شُكْرًا", "شكرا", "sukun, tanween fath"),
    ("رَحْمَٰن", "رحمن", "superscript alef"),
    ("بِسْمِ ٱللَّهِ", "بسم الله", "full basmala marks and alef wasla"),
    (
        "صلى الله عليه وسلمۖ",
        "صلي الله عليه وسلم",
        "Quranic pause mark",
    ),
    (
        "ﷺ",
        "صلي الله عليه وسلم",
        "honorific ligature expands via NFKC",
    ),
    // --- tatweel ---
    ("مـحـمـد", "محمد", "tatweel between letters"),
    ("جمـــيل", "جميل", "repeated tatweel"),
    ("ـــ", "", "only tatweel"),
    // --- digits ---
    ("٠١٢٣٤٥٦٧٨٩", "0123456789", "Arabic-Indic digits"),
    ("۰۱۲۳۴۵۶۷۸۹", "0123456789", "Eastern Arabic-Indic digits"),
    ("فاتورة رقم ١٢٣", "فاتوره رقم 123", "digits in Arabic text"),
    ("٣٫٥", "3 5", "Arabic decimal separator splits like '.'"),
    ("１２３", "123", "full-width digits"),
    ("x²", "x2", "superscript via NFKC"),
    // --- punctuation ---
    (
        "مرحبا، كيف الحال؟",
        "مرحبا كيف الحال",
        "Arabic comma and question mark",
    ),
    ("أولاً؛ ثانياً", "اولا ثانيا", "Arabic semicolon and tanween"),
    ("Hello, world!", "hello world", "Latin punctuation"),
    (
        "e-mail/fax",
        "e mail fax",
        "hyphen and slash separate words",
    ),
    ("«اقتباس»", "اقتباس", "guillemets"),
    ("#tag @mention", "tag mention", "hash and at"),
    ("50%", "50", "percent sign"),
    ("🔁 every month", "every month", "emoji"),
    // --- apostrophes / possessive ---
    (
        "Watanya's ETA invoice",
        "watanya eta invoice",
        "possessive 's",
    ),
    ("Watanya’s", "watanya", "typographic apostrophe possessive"),
    ("don't", "dont", "contraction apostrophe removed"),
    ("'quoted'", "quoted", "quote marks"),
    (
        "rock 'n' roll",
        "rock n roll",
        "apostrophes around a letter",
    ),
    // --- case and Latin diacritics ---
    ("HELLO World", "hello world", "lower-casing"),
    ("Café Crème", "cafe creme", "acute and grave"),
    ("naïve façade", "naive facade", "diaeresis and cedilla"),
    ("Straße", "strasse", "sharp s folds to ss"),
    ("ÅNGSTRÖM", "angstrom", "ring and diaeresis upper-case"),
    ("İstanbul", "istanbul", "dotted capital I"),
    ("ﬁle", "file", "fi ligature"),
    ("Ｆｕｌｌ", "full", "full-width Latin"),
    ("Ελληνικά", "ελληνικα", "Greek tonos stripped"),
    ("ΟΔΟΣ", "οδοσ", "final sigma folds"),
    ("e\u{0301}te\u{0301}", "ete", "decomposed accents"),
    // --- whitespace ---
    ("  a \t b\n\nc  ", "a b c", "collapse and trim"),
    ("a\u{00A0}b", "a b", "no-break space"),
    ("a\u{3000}b", "a b", "ideographic space"),
    ("", "", "empty"),
    ("   ", "", "only spaces"),
    ("!!!", "", "only punctuation"),
    // --- invisible format characters / mixed direction ---
    ("\u{200F}مرحبا\u{200F} Ahmed", "مرحبا ahmed", "RLM marks"),
    ("\u{202B}نص\u{202C} text", "نص text", "RLE/PDF embedding"),
    (
        "\u{2067}أحمد\u{2069} said hi",
        "احمد said hi",
        "RLI/PDI isolates",
    ),
    ("می\u{200C}خواهم", "ميخواهم", "ZWNJ joins"),
    ("\u{FEFF}start", "start", "BOM"),
    ("soft\u{00AD}hyphen", "softhyphen", "soft hyphen"),
    (
        "Invoice فاتورة ٢٠٢٦",
        "invoice فاتوره 2026",
        "mixed scripts and digits",
    ),
    (
        "اجتماع مع Acme Logistics يوم ١/١٠",
        "اجتماع مع acme logistics يوم 1 10",
        "mixed sentence",
    ),
    ("Ahmed(أحمد)", "ahmed احمد", "parentheses between scripts"),
    // --- other scripts are left intact ---
    ("日本語", "日本語", "CJK kept"),
    ("हिन्दी", "हिन्दी", "Devanagari signs kept"),
];

#[test]
fn table_cases() {
    let mut failures = Vec::new();
    for (input, expected, what) in CASES {
        let actual = normalize_for_search(input);
        if actual != *expected {
            failures.push(format!(
                "{what}: {input:?} -> {actual:?}, expected {expected:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "failures:\n{}", failures.join("\n"));
}

#[test]
fn table_has_at_least_sixty_cases() {
    assert!(CASES.len() >= 60, "only {} cases", CASES.len());
}

#[test]
fn every_expected_value_is_a_fixed_point() {
    for (_, expected, what) in CASES {
        assert_eq!(normalize_for_search(expected), *expected, "{what}");
    }
}
