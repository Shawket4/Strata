//! The default home of tasks created without a note (PLAN §6.11): `tasks/Tasks.md`, one
//! `## <Month> <YYYY>` heading per month of creation, in chronological order.

use chrono::{Datelike, NaiveDate};

use super::complete::TaskError;
use super::line::TaskLine;
use crate::line::{LineEnding, line_start};
use crate::sections::{Section, sections};

/// Vault path of the note that holds tasks created without a home note.
pub const DEFAULT_TASK_NOTE: &str = "tasks/Tasks.md";

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The title of the month heading for `date`, e.g. `September 2026` (written as
/// `## September 2026`).
pub fn month_heading(date: NaiveDate) -> String {
    format!("{} {:04}", MONTHS[date.month0() as usize], date.year())
}

/// Reads a month heading title (`September 2026`) as `(year, month)`. Only the exact form
/// [`month_heading`] writes is recognised: English month name, one space, the year with at
/// least four digits.
pub fn parse_month_heading(title: &str) -> Option<(i32, u32)> {
    let (name, year) = title.split_once(' ')?;
    let month = MONTHS.iter().position(|m| *m == name)?;
    let y: i32 = year.parse().ok()?;
    if format!("{y:04}") != year {
        return None;
    }
    Some((y, u32::try_from(month).ok()? + 1))
}

/// Whether the text before an insertion point already ends with a blank line (or is empty).
fn ends_with_blank_line(pre: &str) -> bool {
    let Some(without_eol) = pre.strip_suffix('\n') else {
        return pre.is_empty();
    };
    let last = &without_eol[line_start(without_eol, without_eol.len())..];
    last.trim().is_empty()
}

/// The separator that puts a blank line between `pre` and new text.
fn blank_line_after(pre: &str, eol: &str) -> String {
    if ends_with_blank_line(pre) {
        String::new()
    } else if pre.ends_with('\n') {
        eol.to_owned()
    } else {
        format!("{eol}{eol}")
    }
}

/// Inserts `task_line` into a `tasks/Tasks.md` body under the heading for the month of
/// `created` (see `docs/VAULT_FORMAT.md` §9.5). Every other byte is kept:
///
/// - The month heading exists (the first level-2 heading titled [`month_heading`]): the line
///   goes after the last non-blank line the heading directly owns (before any sub-heading),
///   or directly under the heading when it owns none.
/// - It does not: `## <Month> <YYYY>` and the line are inserted before the first month
///   heading of a later month, or else after the last month section (or at the end of the
///   body when there is none), separated from the text around it by a blank line.
///
/// New lines use `eol`; a last line without a terminator gets one before the insertion.
/// Fails with [`TaskError::NotATask`] unless `task_line` is a single task line.
pub fn insert_under_month(
    body: &str,
    created: NaiveDate,
    task_line: &str,
    eol: LineEnding,
) -> Result<String, TaskError> {
    if task_line.contains(['\n', '\r']) || TaskLine::parse(task_line).is_none() {
        return Err(TaskError::NotATask);
    }
    let eol = eol.as_str();
    let target = (created.year(), created.month());
    let all = sections(body);
    let months: Vec<(&Section, (i32, u32))> = all
        .iter()
        .filter(|s| s.level == 2)
        .filter_map(|s| Some((s, parse_month_heading(&s.title)?)))
        .collect();

    if let Some((s, _)) = months.iter().find(|(_, ym)| *ym == target) {
        let own = &body[s.own_content_span.clone()];
        let kept = own.trim_end_matches([' ', '\t', '\r', '\n']);
        let at = if kept.is_empty() {
            s.own_content_span.start
        } else {
            let end = s.own_content_span.start + kept.len();
            body[end..].find('\n').map_or(body.len(), |i| end + i + 1)
        };
        let pre = &body[..at];
        let sep = if pre.is_empty() || pre.ends_with('\n') {
            ""
        } else {
            eol
        };
        return Ok(format!("{pre}{sep}{task_line}{eol}{}", &body[at..]));
    }

    let block = format!("## {}{eol}{task_line}{eol}", month_heading(created));
    let later = months.iter().find(|(_, ym)| *ym > target);
    let at = match (later, months.last()) {
        (Some((s, _)), _) => line_start(body, s.heading_span.start),
        (None, Some((s, _))) => s.content_span.end,
        (None, None) => body.len(),
    };
    let pre = &body[..at];
    let sep = blank_line_after(pre, eol);
    let after = if at < body.len() { eol } else { "" };
    Ok(format!("{pre}{sep}{block}{after}{}", &body[at..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid test date")
    }

    #[test]
    fn heading_titles_round_trip() {
        assert_eq!(month_heading(d(2026, 9, 27)), "September 2026");
        assert_eq!(month_heading(d(987, 1, 1)), "January 0987");
        assert_eq!(parse_month_heading("September 2026"), Some((2026, 9)));
        assert_eq!(parse_month_heading("January 0987"), Some((987, 1)));
        for bad in [
            "september 2026",
            "Sept 2026",
            "September  2026",
            "September 26",
            "September 2026 ",
            "September +2026",
            "2026 September",
            "September",
        ] {
            assert_eq!(parse_month_heading(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn blank_line_detection() {
        assert!(ends_with_blank_line(""));
        assert!(ends_with_blank_line("\n"));
        assert!(ends_with_blank_line("a\n\n"));
        assert!(ends_with_blank_line("a\r\n\r\n"));
        assert!(ends_with_blank_line("a\n  \n"));
        assert!(!ends_with_blank_line("a\n"));
        assert!(!ends_with_blank_line("a"));
    }

    #[test]
    fn rejects_non_task_text() {
        for bad in ["plain", "- [ ] a\n- [ ] b", "- [ ] a\r"] {
            assert_eq!(
                insert_under_month("", d(2026, 9, 1), bad, LineEnding::Lf),
                Err(TaskError::NotATask),
                "{bad:?}"
            );
        }
    }
}
