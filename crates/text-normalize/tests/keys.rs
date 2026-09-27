//! Table tests for `dedupe_key` and `transliteration_key`.

use text_normalize::{dedupe_key, transliteration_key};

#[test]
fn dedupe_key_exact_values() {
    let cases: &[(&str, &str)] = &[
        ("Watanya's ETA invoice", "eta invoice watanya"),
        ("watanya ETA invoice", "eta invoice watanya"),
        ("ETA invoice for Watanya", "eta invoice watanya"),
        ("Make Watanya’s ETA invoice", "eta invoice make watanya"),
        ("  The   Invoice, the invoice! ", "invoice"),
        ("the", "the"),
        ("The Of", "of the"),
        ("", ""),
        ("فاتورة وطنية", "فاتوره وطنيه"),
        ("الفاتورة الوطنية", "فاتوره وطنيه"),
        ("فاتورة بتاعة وطنية", "فاتوره وطنيه"),
        ("فاتورة الضرائب في وطنية", "ضرايب فاتوره وطنيه"),
        ("مكالمة مع أحمد", "احمد مكالمه"),
        ("مكالمه مع علي", "علي مكالمه"),
        ("مكالمة على الطريق", "طريق علي مكالمه"),
        ("عقد وطنية ٢٠٢٦", "2026 عقد وطنيه"),
        ("عقد وطنية 2026", "2026 عقد وطنيه"),
        ("Invoice فاتورة", "invoice فاتوره"),
    ];
    for (input, expected) in cases {
        assert_eq!(dedupe_key(input), *expected, "input {input:?}");
    }
}

#[test]
fn dedupe_key_keeps_word_set_differences() {
    let distinct: &[(&str, &str)] = &[
        ("Watanya ETA invoice", "Watanya invoice"),
        ("Watanya ETA invoice", "Petrol Arrows ETA invoice"),
        ("invoice 2026", "invoice 2025"),
        ("فاتورة وطنية", "فاتورة بترول"),
        ("call Ahmed", "call Ahmed Samir"),
        ("مكالمة مع علي", "مكالمة"),
    ];
    for (a, b) in distinct {
        assert_ne!(dedupe_key(a), dedupe_key(b), "{a:?} vs {b:?}");
    }
}

#[test]
fn transliteration_key_exact_values() {
    let cases: &[(&str, &str)] = &[
        ("أحمد سمير", "hmdsmr"),
        ("Ahmed Samir", "hmdsmr"),
        ("Ahmad Sameer", "hmdsmr"),
        ("AHMED SAMEER", "hmdsmr"),
        ("أَحْمَد سَمِير", "hmdsmr"),
        ("محمد", "mhmd"),
        ("Mohamed", "mhmd"),
        ("Mohammed", "mhmd"),
        ("Muhammad", "mhmd"),
        ("فاطمة", "ftm"),
        ("Fatma", "ftm"),
        ("Fatmah", "ftm"),
        ("خالد", "kld"),
        ("Khaled", "kld"),
        ("Khalid", "kld"),
        ("شادي", "xd"),
        ("Shady", "xd"),
        ("Shadi", "xd"),
        ("جمال", "jml"),
        ("Gamal", "jml"),
        ("Jamal", "jml"),
        ("عبد الرحمن", "bdrhmn"),
        ("عبدالرحمن", "bdrhmn"),
        ("Abdelrahman", "bdrhmn"),
        ("Abd El-Rahman", "bdrhmn"),
        ("Abdul Rahman", "bdrhmn"),
        ("Abdurrahman", "bdrhmn"),
        ("عبدالله", "bdl"),
        ("Abdallah", "bdl"),
        ("Abdullah", "bdl"),
        ("السيد", "sd"),
        ("El Sayed", "sd"),
        ("El-Sayed", "sd"),
        ("هشام", "hxm"),
        ("Hesham", "hxm"),
        ("Hisham", "hxm"),
        ("يوسف", "sf"),
        ("Youssef", "sf"),
        ("Yousef", "sf"),
        ("عمرو", "mr"),
        ("Amr", "mr"),
        ("3amr", "mr"),
        ("حمادة", "hmd"),
        ("7amada", "hmd"),
        ("غادة", "gd"),
        ("Ghada", "gd"),
        ("وطنية", "tn"),
        ("Watanya", "tn"),
        ("Watania", "tn"),
        ("Acme", "km"),
        ("أكمي", "km"),
        ("ظافر", "zfr"),
        ("Zafer", "zfr"),
        ("Dhafer", "zfr"),
        ("Max", "mks"),
        ("Cecile", "sl"),
        ("Philip", "flb"),
        ("Room 12", "rm12"),
        ("", ""),
    ];
    for (input, expected) in cases {
        assert_eq!(transliteration_key(input), *expected, "input {input:?}");
    }
}

#[test]
fn transliteration_key_separates_different_names() {
    let distinct: &[(&str, &str)] = &[
        ("Ahmed Samir", "Ahmed Fathy"),
        ("أحمد سمير", "أحمد فتحي"),
        ("Shady", "Sady"),
        ("Khaled", "Walid"),
        ("Watanya", "Petrol Arrows"),
    ];
    for (a, b) in distinct {
        assert_ne!(transliteration_key(a), transliteration_key(b), "{a:?} vs {b:?}");
    }
}
