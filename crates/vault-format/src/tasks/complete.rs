//! Completing, cancelling and reopening task lines (Tasks plugin semantics).

use chrono::{Duration, NaiveDate};

use super::line::{DateKind, TaskLine, TaskStatus};
use super::recurrence::{RecurrenceNotUnderstood, parse_recurrence};
use crate::blocks::is_valid_block_id;

/// Why a task could not be changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskError {
    /// The line is not a checklist item.
    #[error("not a task line")]
    NotATask,
    /// The task is already done or cancelled.
    #[error("task is not open (status `{0}`)")]
    NotOpen(char),
    /// The task has no `🔁` recurrence.
    #[error("task does not recur")]
    NotRecurring,
    /// The recurrence phrase is outside the supported grammar.
    #[error(transparent)]
    RecurrenceNotUnderstood(#[from] RecurrenceNotUnderstood),
    /// A recurring task needs a due, scheduled or start date to advance.
    #[error("recurring task has no due, scheduled or start date")]
    NoReferenceDate,
    /// The rule produced no further date.
    #[error("recurrence has no next occurrence")]
    NoNextOccurrence,
    /// The new block ID is not `[a-z0-9-]+`.
    #[error("invalid block id `{0}`")]
    InvalidBlockId(String),
}

fn open_task(line: &str) -> Result<TaskLine, TaskError> {
    let t = TaskLine::parse(line).ok_or(TaskError::NotATask)?;
    if !t.status().is_open() {
        return Err(TaskError::NotOpen(t.status_char()));
    }
    Ok(t)
}

/// Marks a task done on `done_date`: `[x]` and `✅ <date>`. For recurring tasks use
/// [`complete_recurring`], which also writes the next occurrence.
pub fn complete(line: &str, done_date: NaiveDate) -> Result<String, TaskError> {
    let t = open_task(line)?;
    Ok(t.with_status(TaskStatus::Done)
        .with_date(DateKind::Done, Some(done_date))
        .as_str()
        .to_owned())
}

/// Cancels a task: `[-]` and `❌ <date>`.
pub fn cancel(line: &str, date: NaiveDate) -> Result<String, TaskError> {
    let t = open_task(line)?;
    Ok(t.with_status(TaskStatus::Cancelled)
        .with_date(DateKind::Cancelled, Some(date))
        .as_str()
        .to_owned())
}

/// Reopens a done or cancelled task: `[ ]`, `✅`/`❌` removed.
pub fn reopen(line: &str) -> Result<String, TaskError> {
    let t = TaskLine::parse(line).ok_or(TaskError::NotATask)?;
    Ok(t.with_status(TaskStatus::Todo)
        .with_date(DateKind::Done, None)
        .with_date(DateKind::Cancelled, None)
        .as_str()
        .to_owned())
}

/// Completes a recurring task the way the Tasks plugin does and returns the two lines to
/// write in place of `line`, in order: the **new occurrence** (inserted above) and the
/// **completed** line.
///
/// - The reference date is 📅 due, else ⏳ scheduled, else 🛫 start. The next reference date is
///   the first occurrence of the rule after the reference date (after `done_date` for
///   `when done` rules). Every other date moves by the same number of days; reminders move by
///   the same number of days and keep their wall-clock time.
/// - The new line is open, has no ✅/❌, carries `new_block_id`, and (if the original had
///   ➕) is created on `done_date`. Everything else is kept byte-for-byte.
/// - The completed line gets `[x]` and `✅ <done_date>` and keeps its block ID.
pub fn complete_recurring(
    line: &str,
    done_date: NaiveDate,
    new_block_id: &str,
) -> Result<[String; 2], TaskError> {
    let t = open_task(line)?;
    let phrase = t.recurrence_text().ok_or(TaskError::NotRecurring)?;
    let rule = parse_recurrence(phrase)?;
    if !is_valid_block_id(new_block_id) {
        return Err(TaskError::InvalidBlockId(new_block_id.to_owned()));
    }
    let reference = t
        .date(DateKind::Due)
        .or_else(|| t.date(DateKind::Scheduled))
        .or_else(|| t.date(DateKind::Start))
        .ok_or(TaskError::NoReferenceDate)?;
    let base = if rule.when_done { done_date } else { reference };
    let next_reference = rule.next_after(base).ok_or(TaskError::NoNextOccurrence)?;
    let offset: Duration = next_reference - reference;

    let mut next = t.with_status(TaskStatus::Todo);
    for kind in [DateKind::Start, DateKind::Scheduled, DateKind::Due] {
        if let Some(d) = t.date(kind) {
            next = next.with_date(kind, Some(d + offset));
        }
    }
    if t.date(DateKind::Created).is_some() {
        next = next.with_date(DateKind::Created, Some(done_date));
    }
    next = next
        .with_date(DateKind::Done, None)
        .with_date(DateKind::Cancelled, None);
    let shifted: Vec<_> = t
        .reminders()
        .into_iter()
        .map(|mut r| {
            r.date += offset;
            r
        })
        .collect();
    next = next.with_reminders(&shifted).with_block_id(new_block_id);

    let done = t
        .with_status(TaskStatus::Done)
        .with_date(DateKind::Done, Some(done_date));
    Ok([next.as_str().to_owned(), done.as_str().to_owned()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_default()
    }

    #[test]
    fn spec_example() {
        let line = "- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2";
        let out = complete_recurring(line, d("2026-10-01"), "t-01j9b0");
        assert_eq!(
            out,
            Ok([
                "- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-11-01 (@2026-11-01 09:00) [[Watanya]] ^t-01j9b0".to_owned(),
                "- [x] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 ✅ 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2".to_owned(),
            ])
        );
    }

    #[test]
    fn errors() {
        let day = d("2026-01-01");
        assert_eq!(
            complete_recurring("text", day, "t-1"),
            Err(TaskError::NotATask)
        );
        assert_eq!(
            complete_recurring("- [x] a 🔁 every day 📅 2026-01-01", day, "t-1"),
            Err(TaskError::NotOpen('x'))
        );
        assert_eq!(
            complete_recurring("- [ ] a 📅 2026-01-01", day, "t-1"),
            Err(TaskError::NotRecurring)
        );
        assert_eq!(
            complete_recurring("- [ ] a 🔁 every day", day, "t-1"),
            Err(TaskError::NoReferenceDate)
        );
        assert_eq!(
            complete_recurring("- [ ] a 🔁 every day 📅 2026-01-01", day, "T 1"),
            Err(TaskError::InvalidBlockId("T 1".into()))
        );
        assert!(matches!(
            complete_recurring("- [ ] a 🔁 every blue moon 📅 2026-01-01", day, "t-1"),
            Err(TaskError::RecurrenceNotUnderstood(e)) if e.phrase == "every blue moon"
        ));
        assert_eq!(cancel("- [-] a", day), Err(TaskError::NotOpen('-')));
    }

    #[test]
    fn simple_transitions() {
        let day = d("2026-02-03");
        assert_eq!(
            complete("- [ ] a ^t-1", day).as_deref(),
            Ok("- [x] a ✅ 2026-02-03 ^t-1")
        );
        assert_eq!(
            cancel("- [/] a 📅 2026-02-01", day).as_deref(),
            Ok("- [-] a 📅 2026-02-01 ❌ 2026-02-03")
        );
        assert_eq!(
            reopen("- [x] a 📅 2026-02-01 ✅ 2026-02-03 ^t").as_deref(),
            Ok("- [ ] a 📅 2026-02-01 ^t")
        );
    }
}
