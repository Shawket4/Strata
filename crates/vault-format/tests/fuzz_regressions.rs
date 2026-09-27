//! Inputs found by the fuzz targets (`fuzz/`), kept as regression tests (PLAN §16.2).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use vault_format::frontmatter::FrontmatterError;
use vault_format::tasks::{DateKind, TaskLine};
use vault_format::{Document, KnownKey, PropertyValue};

/// fuzz/document: canonical rendering reordered the entries of unreadable YAML (control
/// characters), producing a different document. Such frontmatter must stay verbatim.
#[test]
fn invalid_frontmatter_is_never_reordered() {
    let text = "---\n# Written by- \"[[Discount\npublish: true\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nmultiline: |\n  lin\u{1}\0\0]e one\n\n  line three\naliases: pricing tests\n---\nBody with [[Churn notes]] and a #tag.\n";
    let doc = Document::parse(text);
    // yaml-rust2 stops reading at the NUL, so the key count no longer matches the lines.
    assert_eq!(
        doc.frontmatter().unwrap().error(),
        Some(&FrontmatterError::Unsupported("found 4 top-level lines but 3 keys".into()))
    );
    assert_eq!(doc.render(), text);
    assert_eq!(doc.render_canonical(), text);
}

/// fuzz/frontmatter: a bare `\r` is a line break to YAML but not to the entry splitter.
#[test]
fn bare_carriage_returns_make_frontmatter_read_only() {
    let text = "---\n#\r\r\t~\t>\rfalse\n?+2\n---\n";
    let mut doc = Document::parse(text);
    assert_eq!(
        doc.frontmatter().unwrap().error(),
        Some(&FrontmatterError::Unsupported("bare carriage return".into()))
    );
    assert!(doc.frontmatter_mut().set_text(KnownKey::Title, "x").is_err());
    assert_eq!(doc.render(), text);
}

/// Canonical order would put an alias before its anchor; the original order is kept.
#[test]
fn reordering_that_breaks_anchors_falls_back_to_file_order() {
    let text = "---\nbase: &b hello\ntitle: *b\n---\n";
    let mut doc = Document::parse(text);
    let fm = doc.frontmatter_mut();
    assert_eq!(fm.get("title"), Some(&PropertyValue::Text("hello".into())));
    fm.set_text(KnownKey::Lang, "en").unwrap();
    assert_eq!(doc.render(), "---\nbase: &b hello\ntitle: *b\nlang: en\n---\n");
    // Canonical rendering writes the resolved value, which needs no anchor.
    assert_eq!(
        Document::parse(text).render_canonical(),
        "---\ntitle: hello\nbase: &b hello\n---\n"
    );
}

/// Edits that would change how another entry reads are refused.
#[test]
fn edits_that_would_break_other_entries_are_refused() {
    let text = "---\nbase: &b hello\ntitle: *b\n---\n";
    let mut doc = Document::parse(text);
    let err = doc.frontmatter_mut().remove("base").unwrap_err();
    assert!(matches!(err, FrontmatterError::Unsupported(_)));
    assert_eq!(doc.render(), text);
}

/// fuzz/task_line: a date inserted right after a recurrence phrase was glued to the
/// following text and did not parse back.
#[test]
fn inserted_fields_stay_separated() {
    let line = "- [ ] Odd recurrence 🔁 Every blue[moon 📅 9248-1on the 1st ";
    let t = TaskLine::parse(line).unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    for kind in DateKind::ALL {
        let edited = t.with_date(kind, Some(day));
        assert_eq!(edited.date(kind), Some(day), "{kind:?}: {}", edited.as_str());
    }
    assert_eq!(
        t.with_date(DateKind::Due, Some(day)).as_str(),
        "- [ ] Odd recurrence 🔁 Every blue 📅 2026-01-31 [moon 📅 9248-1on the 1st "
    );
}
