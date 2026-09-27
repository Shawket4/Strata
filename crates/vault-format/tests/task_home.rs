//! `tasks/Tasks.md`: inserting new task lines under the month heading of their creation date.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use vault_format::LineEnding;
use vault_format::sections::sections;
use vault_format::tasks::{
    DEFAULT_TASK_NOTE, extract_tasks, insert_under_month, month_heading, parse_month_heading,
};

const LF: LineEnding = LineEnding::Lf;
const LINE: &str = "- [ ] Petrol Arrows invoice 📅 2026-09-30 ^t-01j9a3";

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn sep(body: &str) -> String {
    insert_under_month(body, d(2026, 9, 27), LINE, LF).unwrap()
}

#[test]
fn default_home_and_heading_format() {
    assert_eq!(DEFAULT_TASK_NOTE, "tasks/Tasks.md");
    assert_eq!(month_heading(d(2026, 9, 27)), "September 2026");
    assert_eq!(month_heading(d(2027, 2, 1)), "February 2027");
}

#[test]
fn empty_file_gets_the_heading() {
    assert_eq!(sep(""), format!("## September 2026\n{LINE}\n"));
}

#[test]
fn preamble_without_month_headings() {
    assert_eq!(
        sep("# Tasks\n"),
        format!("# Tasks\n\n## September 2026\n{LINE}\n")
    );
    assert_eq!(
        sep("# Tasks"),
        format!("# Tasks\n\n## September 2026\n{LINE}\n")
    );
    assert_eq!(
        sep("# Tasks\n\n"),
        format!("# Tasks\n\n## September 2026\n{LINE}\n")
    );
}

#[test]
fn same_month_appends_after_its_last_line() {
    let body = "# Tasks\n\n## September 2026\n- [ ] one ^t-1\n- [x] two ✅ 2026-09-02 ^t-2\n\n## October 2026\n- [ ] three ^t-3\n";
    assert_eq!(
        sep(body),
        format!(
            "# Tasks\n\n## September 2026\n- [ ] one ^t-1\n- [x] two ✅ 2026-09-02 ^t-2\n{LINE}\n\n## October 2026\n- [ ] three ^t-3\n"
        )
    );
}

#[test]
fn same_month_empty_heading() {
    assert_eq!(
        sep("## September 2026\n\n## October 2026\n"),
        format!("## September 2026\n{LINE}\n\n## October 2026\n")
    );
    // Heading on the last line without a terminator.
    assert_eq!(
        sep("## September 2026"),
        format!("## September 2026\n{LINE}\n")
    );
    // Last task without a terminator.
    assert_eq!(
        sep("## September 2026\n- [ ] one ^t-1"),
        format!("## September 2026\n- [ ] one ^t-1\n{LINE}\n")
    );
}

#[test]
fn same_month_goes_before_sub_headings() {
    let body = "## September 2026\n- [ ] one ^t-1\n\n### Work\n- [ ] w ^t-w\n## October 2026\n";
    assert_eq!(
        sep(body),
        format!(
            "## September 2026\n- [ ] one ^t-1\n{LINE}\n\n### Work\n- [ ] w ^t-w\n## October 2026\n"
        )
    );
}

#[test]
fn new_month_between_earlier_and_later_headings() {
    let body = "## August 2026\n- [ ] a ^t-a\n\n## October 2026\n- [ ] o ^t-o\n";
    assert_eq!(
        sep(body),
        format!(
            "## August 2026\n- [ ] a ^t-a\n\n## September 2026\n{LINE}\n\n## October 2026\n- [ ] o ^t-o\n"
        )
    );
    // No blank line before the later heading: one is added on both sides.
    let body = "## August 2026\n- [ ] a ^t-a\n## October 2026\n- [ ] o ^t-o\n";
    assert_eq!(
        sep(body),
        format!(
            "## August 2026\n- [ ] a ^t-a\n\n## September 2026\n{LINE}\n\n## October 2026\n- [ ] o ^t-o\n"
        )
    );
}

#[test]
fn new_month_before_only_later_headings() {
    let body = "# Tasks\n\n## October 2026\n- [ ] o ^t-o\n## January 2027\n";
    assert_eq!(
        sep(body),
        format!(
            "# Tasks\n\n## September 2026\n{LINE}\n\n## October 2026\n- [ ] o ^t-o\n## January 2027\n"
        )
    );
    // Later heading on the first line.
    assert_eq!(
        sep("## October 2026\n"),
        format!("## September 2026\n{LINE}\n\n## October 2026\n")
    );
}

#[test]
fn new_month_after_only_earlier_headings() {
    let body = "## August 2025\n- [ ] a ^t-a\n\n## August 2026\n- [ ] b ^t-b";
    assert_eq!(
        sep(body),
        format!(
            "## August 2025\n- [ ] a ^t-a\n\n## August 2026\n- [ ] b ^t-b\n\n## September 2026\n{LINE}\n"
        )
    );
    // A non-month level-2 heading after the months stays after the new month.
    let body = "## August 2026\n- [ ] a ^t-a\n\n## Notes\nfree text\n";
    assert_eq!(
        sep(body),
        format!(
            "## August 2026\n- [ ] a ^t-a\n\n## September 2026\n{LINE}\n\n## Notes\nfree text\n"
        )
    );
}

#[test]
fn look_alike_headings_are_not_month_headings() {
    let body =
        "### September 2026\n- [ ] deep ^t-d\n```\n## September 2026\n```\n## september 2026\n";
    assert_eq!(sep(body), format!("{body}\n## September 2026\n{LINE}\n"));
}

#[test]
fn arabic_content_and_crlf_are_kept() {
    let body = "# المهام\r\n\r\nملاحظة قبل العناوين\r\n\r\n## August 2026\r\n- [ ] فاتورة وطنية 📅 2026-08-01 ^t-a\r\n\r\nنص عربي بعد المهام\r\n\r\n## October 2026\r\n- [ ] اتصل بـ [[أحمد سمير]] ^t-o\r\n";
    let line = "- [ ] ادفع الإيجار 🔁 every month on the 1st 📅 2026-10-01 ^t-01j9z9";
    let got = insert_under_month(body, d(2026, 9, 3), line, LineEnding::CrLf).unwrap();
    assert_eq!(
        got,
        format!(
            "# المهام\r\n\r\nملاحظة قبل العناوين\r\n\r\n## August 2026\r\n- [ ] فاتورة وطنية 📅 2026-08-01 ^t-a\r\n\r\nنص عربي بعد المهام\r\n\r\n## September 2026\r\n{line}\r\n\r\n## October 2026\r\n- [ ] اتصل بـ [[أحمد سمير]] ^t-o\r\n"
        )
    );
    // Same month again: after the Arabic text the month section ends with.
    let again = insert_under_month(&got, d(2026, 9, 30), LINE, LineEnding::CrLf).unwrap();
    assert_eq!(
        again,
        got.replace(&format!("{line}\r\n"), &format!("{line}\r\n{LINE}\r\n"))
    );
}

// ---------------------------------------------------------------------------------------
// Properties

fn month() -> impl Strategy<Value = (i32, u32)> {
    (2024i32..2028, 1u32..=12)
}

fn filler() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        "[a-z ]{1,12}",
        "[ء-ي ]{1,12}",
        "[a-zء-ي ]{1,8}".prop_map(|s| format!("- [ ] {s} ^t-x")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Inserting only adds the line (and, for a new month, its heading and blank lines);
    /// month headings stay unique and chronological; the line lands under its month.
    #[test]
    fn insertion_is_minimal_and_ordered(
        preamble in filler(),
        mut months in prop::collection::btree_set(month(), 0..5),
        contents in prop::collection::vec(prop::collection::vec(filler(), 0..3), 5),
        target in month(),
        crlf in any::<bool>(),
        final_newline in any::<bool>(),
    ) {
        let (eol, eol_s) = if crlf { (LineEnding::CrLf, "\r\n") } else { (LineEnding::Lf, "\n") };
        let mut body = String::new();
        if !preamble.is_empty() {
            body.push_str(&preamble);
            body.push_str(eol_s);
        }
        for ((y, m), lines) in months.iter().zip(&contents) {
            body.push_str("## ");
            body.push_str(&month_heading(d(*y, *m, 1)));
            body.push_str(eol_s);
            for l in lines {
                body.push_str(l);
                body.push_str(eol_s);
            }
        }
        if !final_newline && body.ends_with(eol_s) {
            body.truncate(body.len() - eol_s.len());
        }
        let date = d(target.0, target.1, 15);
        let out = insert_under_month(&body, date, LINE, eol).unwrap();

        // Everything else is byte-identical: `out` = body[..p] + inserted + body[p..].
        let p = body.bytes().zip(out.bytes()).take_while(|(a, b)| a == b).count();
        let q = body.bytes().rev().zip(out.bytes().rev()).take_while(|(a, b)| a == b).count();
        prop_assert!(p + q >= body.len(), "{body:?} -> {out:?}");
        // Some split point of that shape inserts only the heading, the line and terminators.
        let inserted_len = out.len() - body.len();
        let heading = format!("## {}", month_heading(date));
        let only_new = (body.len().saturating_sub(q)..=p.min(body.len()))
            .filter(|&at| out.is_char_boundary(at) && out.is_char_boundary(at + inserted_len))
            .any(|at| {
                let inserted = &out[at..at + inserted_len];
                inserted.contains(LINE)
                    && inserted.replace(LINE, "").replace(&heading, "").replace(eol_s, "").is_empty()
            });
        prop_assert!(only_new, "{body:?} -> {out:?}");

        // Month headings unique and ascending; the line is in its month's section.
        let secs = sections(&out);
        let found: Vec<(i32, u32)> = secs.iter().filter(|s| s.level == 2)
            .filter_map(|s| parse_month_heading(&s.title)).collect();
        months.insert(target);
        prop_assert_eq!(found, months.into_iter().collect::<Vec<_>>());
        let own = secs.iter().find(|s| s.level == 2 && s.title == month_heading(date)).unwrap();
        let task = extract_tasks(&out).into_iter()
            .find(|t| t.task.block_id() == Some("t-01j9a3")).unwrap();
        prop_assert!(own.own_content_span.contains(&task.line_span.start));
    }
}
