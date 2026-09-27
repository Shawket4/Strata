//! Tasks as Obsidian Tasks checklist lines (PLAN §6.11): parsing and editing lines, the
//! recurrence grammar and RRULE compilation, next-occurrence computation in the user's time
//! zone, completion of recurring tasks, and the month headings of `tasks/Tasks.md`.

mod complete;
mod home;
mod line;
mod recurrence;
mod schedule;

use std::ops::Range;

pub use complete::{TaskError, cancel, complete, complete_recurring, reopen};
pub use home::{DEFAULT_TASK_NOTE, insert_under_month, month_heading, parse_month_heading};
pub use line::{DateKind, Priority, Reminder, Spanned, TaskLine, TaskSpec, TaskStatus};
pub use recurrence::{
    Frequency, MonthDay, NthWeekday, RecurrenceNotUnderstood, RecurrenceRule, parse_recurrence,
};
pub use schedule::{next_occurrence, next_occurrence_at, reminder_instant, resolve_local};

use crate::body;

/// A task found in a note body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyTask {
    /// Span of the line (terminator excluded) in the body.
    pub line_span: Range<usize>,
    /// 0-based line number.
    pub line_number: usize,
    /// The parsed line.
    pub task: TaskLine,
}

/// Every task line in a body, skipping code blocks.
pub fn extract_tasks(body_text: &str) -> Vec<BodyTask> {
    let analysis = body::analyze(body_text);
    crate::line::lines(body_text)
        .enumerate()
        .filter(|(_, l)| !analysis.in_code(l.start))
        .filter_map(|(n, l)| {
            TaskLine::parse(l.content).map(|task| BodyTask {
                line_span: l.content_range(),
                line_number: n,
                task,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tasks_outside_code() {
        let body = "# T\n- [ ] one ^t-1\r\n```\n- [ ] fake\n```\n  - [x] two\n";
        let tasks = extract_tasks(body);
        let got: Vec<_> = tasks
            .iter()
            .map(|t| {
                (
                    t.line_number,
                    t.task.description().to_owned(),
                    &body[t.line_span.clone()],
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (1, "one".to_owned(), "- [ ] one ^t-1"),
                (5, "two".to_owned(), "  - [x] two")
            ]
        );
    }
}
