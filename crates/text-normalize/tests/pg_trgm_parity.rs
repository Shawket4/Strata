//! Parity with PostgreSQL `pg_trgm` (§9.7): the client core's offline near-duplicate scores
//! must equal the server's.
//!
//! # How the expected values were obtained
//!
//! Every `expected` value below is the `real` returned by PostgreSQL 16.13 (`pg_trgm` 1.6,
//! database `LC_CTYPE = C.UTF-8`) for
//! `SELECT similarity(normalize_for_search(a), normalize_for_search(b))` — the normalised
//! strings were computed by this crate and sent as literals by the ignored test
//! [`live_postgres_matches_hardcoded_values`], which is how the table was produced and how it
//! is re-verified:
//!
//! ```sh
//! STRATA_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres \
//!   cargo test -p text-normalize --test pg_trgm_parity -- --ignored --nocapture
//! ```
//!
//! It needs `psql` on `PATH`, creates `pg_trgm` inside a transaction and rolls it back, and
//! prints the live values as a ready-to-paste table when they differ.

use std::process::Command;

use text_normalize::{normalize_for_search, raw_similarity, raw_trigrams, trigram_similarity};

/// (a, b, PostgreSQL `similarity()` of the normalised strings)
const NORMALIZED_PAIRS: &[(&str, &str, f32)] = &[
    ("Watanya's ETA invoice", "ETA invoice for Watanya", 0.8333333),
    ("Make Watanya's ETA invoice", "Make Watanya ETA invoice monthly", 0.78125),
    ("Petrol Arrows invoice", "Watanya's ETA invoice", 0.23529412),
    ("Ahmed Samir", "Ahmad Sameer", 0.31578946),
    ("Ahmed Samir", "Ahmed Fathy", 0.33333334),
    ("أحمد سمير", "احمد سمير", 1.0),
    ("أحمد سمير", "أحمد فتحي", 0.33333334),
    ("محمد", "م\u{64f}ح\u{64e}م\u{64e}\u{651}د", 1.0),
    ("فاتورة وطنية", "فاتورة الوطنية", 0.64705884),
    ("عقد وطنية", "عقد شركة وطنية", 0.6666667),
    ("مكتب مدينة نصر", "Nasr City office", 0.0),
    ("Nasr City office", "Safe — Nasr City office", 0.77272725),
    ("Acme Logistics", "Acme Logistic", 0.8125),
    ("Acme", "أكمي", 0.0),
    ("invoice 2026", "invoice ٢٠٢٦", 1.0),
    ("Pricing experiments", "pricing tests", 0.36),
    ("Loyalty", "Loyality", 0.54545456),
    ("a", "ab", 0.25),
    ("word", "words", 0.5714286),
    ("Café crème", "cafe creme", 1.0),
    ("Churn notes", "Churn note", 0.7692308),
    ("Discount policy", "Discount policies", 0.7),
    ("المستشفى", "مستشفي", 0.45454547),
    ("مدرسة", "مدرسه", 1.0),
    ("مسئول المشتريات", "مسؤول المشتريات", 0.68421054),
    ("اجتماع مع Acme يوم الأحد", "اجتماع Acme الأحد", 0.7083333),
    ("Watanya", "Watania", 0.45454547),
    ("Petrol Arrows invoice every week", "Petrol Arrows weekly invoice", 0.7222222),
    ("x", "y", 0.0),
    ("Subscription tiers", "subscription tier pricing", 0.60714287),
    ("", "abc", 0.0),
    ("Shady", "Shadi", 0.5),
    ("وطنية", "ووتانيا", 0.07692308),
    ("كتاب", "كتب", 0.2857143),
    ("Ahmed (أحمد) called", "أحمد called Ahmed", 1.0),
];

/// (a, b, PostgreSQL `similarity(a, b)` on the raw strings) — checks the word splitting and
/// lower-casing of the raw algorithm itself.
const RAW_PAIRS: &[(&str, &str, f32)] = &[
    ("Hello, World!", "hello world", 1.0),
    ("foo_bar-baz", "foo bar baz", 1.0),
    ("ABC abc", "abc", 1.0),
    ("e-mail", "email", 0.44444445),
    ("Watanya's", "watanya", 0.8),
];

/// ASCII strings whose raw trigram sets are compared with `show_trgm()` (PostgreSQL prints
/// multi-byte trigrams as hashes, so only ASCII sets are compared literally).
const SHOW_TRGM_INPUTS: &[&str] = &[
    "Hello, World!",
    "a",
    "ab ab",
    "foo_bar-baz",
    "Watanya's ETA invoice",
    "  x  ",
    "2026-10-01",
];

/// `show_trgm()` output for [`SHOW_TRGM_INPUTS`], same order.
const SHOW_TRGM_EXPECTED: &[&[&str]] = &[
    &["  h", "  w", " he", " wo", "ell", "hel", "ld ", "llo", "lo ", "orl", "rld", "wor"],
    &["  a", " a "],
    &["  a", " ab", "ab "],
    &["  b", "  f", " ba", " fo", "ar ", "az ", "bar", "baz", "foo", "oo "],
    &["  e", "  i", "  s", "  w", " et", " in", " s ", " wa", "any", "ata", "ce ", "eta", "ice", "inv", "nvo", "nya", "oic", "ta ", "tan", "voi", "wat", "ya "],
    &["  x", " x "],
    &["  0", "  1", "  2", " 01", " 10", " 20", "01 ", "026", "10 ", "202", "26 "],
];

const TOLERANCE: f32 = 1e-6;

#[test]
fn normalized_pairs_match_postgres() {
    assert!(NORMALIZED_PAIRS.len() >= 30);
    for (a, b, expected) in NORMALIZED_PAIRS {
        let actual = trigram_similarity(a, b);
        assert!(
            (actual - expected).abs() <= TOLERANCE,
            "{a:?} vs {b:?}: rust {actual}, postgres {expected}"
        );
    }
}

#[test]
fn raw_pairs_match_postgres() {
    for (a, b, expected) in RAW_PAIRS {
        let actual = raw_similarity(a, b);
        assert!(
            (actual - expected).abs() <= TOLERANCE,
            "{a:?} vs {b:?}: rust {actual}, postgres {expected}"
        );
    }
}

#[test]
fn trigram_sets_match_show_trgm() {
    assert_eq!(SHOW_TRGM_INPUTS.len(), SHOW_TRGM_EXPECTED.len());
    for (input, expected) in SHOW_TRGM_INPUTS.iter().zip(SHOW_TRGM_EXPECTED) {
        assert_eq!(raw_trigrams(input), *expected, "input {input:?}");
    }
}

fn sql_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn run_psql(select_list: &[String]) -> Vec<String> {
    let url = std::env::var("STRATA_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres@127.0.0.1:5432/postgres".to_owned());
    let mut sql = String::from("BEGIN;\nCREATE EXTENSION IF NOT EXISTS pg_trgm;\n");
    for item in select_list {
        sql.push_str(&format!("SELECT {item};\n"));
    }
    sql.push_str("ROLLBACK;\n");
    let output = Command::new("psql")
        .args([url.as_str(), "-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1", "-c", &sql])
        .output()
        .expect("psql must be installed to run the live parity test");
    assert!(
        output.status.success(),
        "psql failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("psql output is UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
#[ignore = "needs a live PostgreSQL with pg_trgm (STRATA_TEST_DATABASE_URL) and psql"]
fn live_postgres_matches_hardcoded_values() {
    let pairs: Vec<(&str, &str)> = NORMALIZED_PAIRS.iter().map(|(a, b, _)| (*a, *b)).collect();
    let normalized_sql: Vec<String> = pairs
        .iter()
        .map(|(a, b)| {
            format!(
                "similarity({}, {})",
                sql_literal(&normalize_for_search(a)),
                sql_literal(&normalize_for_search(b))
            )
        })
        .collect();
    let raw_sql: Vec<String> = RAW_PAIRS
        .iter()
        .map(|(a, b, _)| format!("similarity({}, {})", sql_literal(a), sql_literal(b)))
        .collect();
    let show_sql: Vec<String> = SHOW_TRGM_INPUTS
        .iter()
        .map(|s| format!("array_to_string(show_trgm({}), '|')", sql_literal(s)))
        .collect();

    let norm_live = run_psql(&normalized_sql);
    let raw_live = run_psql(&raw_sql);
    let show_live = run_psql(&show_sql);

    println!("NORMALIZED_PAIRS (live):");
    for ((a, b), v) in pairs.iter().zip(&norm_live) {
        println!("    ({a:?}, {b:?}, {v}),");
    }
    println!("RAW_PAIRS (live):");
    for ((a, b, _), v) in RAW_PAIRS.iter().zip(&raw_live) {
        println!("    ({a:?}, {b:?}, {v}),");
    }
    println!("SHOW_TRGM_EXPECTED (live):");
    for v in &show_live {
        let items: Vec<String> = v.split('|').map(|t| format!("{t:?}")).collect();
        println!("    &[{}],", items.join(", "));
    }

    for ((a, b, expected), live) in NORMALIZED_PAIRS.iter().zip(&norm_live) {
        let live: f32 = live.parse().expect("similarity() prints a float");
        assert!((live - expected).abs() <= TOLERANCE, "{a:?} vs {b:?}: live {live}");
        assert!((trigram_similarity(a, b) - live).abs() <= TOLERANCE, "{a:?} vs {b:?}");
    }
    for ((a, b, expected), live) in RAW_PAIRS.iter().zip(&raw_live) {
        let live: f32 = live.parse().expect("similarity() prints a float");
        assert!((live - expected).abs() <= TOLERANCE, "{a:?} vs {b:?}: live {live}");
    }
    for (expected, live) in SHOW_TRGM_EXPECTED.iter().zip(&show_live) {
        let live: Vec<&str> = live.split('|').collect();
        assert_eq!(*expected, live.as_slice());
    }
    assert_eq!(norm_live.len(), NORMALIZED_PAIRS.len());
    assert_eq!(raw_live.len(), RAW_PAIRS.len());
    assert_eq!(show_live.len(), SHOW_TRGM_INPUTS.len());
}
