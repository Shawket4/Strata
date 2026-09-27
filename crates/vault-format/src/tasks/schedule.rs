//! Turning dates and wall-clock reminder times into instants in the user's time zone.

use chrono::{
    DateTime, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone,
};
use chrono_tz::Tz;

use super::recurrence::RecurrenceRule;

/// Resolves a wall-clock time in `tz`:
/// - a time that exists once maps to it;
/// - a time repeated by a DST fall-back maps to its **first** occurrence;
/// - a time skipped by a DST spring-forward maps to the same distance after the transition
///   (London 01:30 on the spring-forward day becomes 02:30 BST), i.e. the wall-clock time
///   interpreted with the offset in force before the gap.
pub fn resolve_local(naive: NaiveDateTime, tz: Tz) -> DateTime<Tz> {
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(earliest, _) => earliest,
        LocalResult::None => {
            // Offset before the gap: look a day earlier (transitions are never a day apart).
            let before = naive - Duration::days(1);
            let offset = match tz.from_local_datetime(&before) {
                LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.offset().fix(),
                LocalResult::None => tz.offset_from_utc_datetime(&before).fix(),
            };
            let utc = naive - Duration::seconds(i64::from(offset.local_minus_utc()));
            tz.from_utc_datetime(&utc)
        }
    }
}

/// The next occurrence date after `reference` (see [`RecurrenceRule::next_after`]).
pub fn next_occurrence(rule: &RecurrenceRule, reference: NaiveDate) -> Option<NaiveDate> {
    rule.next_after(reference)
}

/// The next occurrence of a timed item: the date advances by `rule`, the wall-clock time stays
/// the same, and the result is resolved in `tz` with [`resolve_local`] (so a 09:00 reminder
/// stays at 09:00 local across DST changes).
pub fn next_occurrence_at(
    rule: &RecurrenceRule,
    reference: NaiveDateTime,
    tz: Tz,
) -> Option<DateTime<Tz>> {
    let date = rule.next_after(reference.date())?;
    Some(resolve_local(date.and_time(reference.time()), tz))
}

/// The instant a reminder fires: its date at its time (or `default_time` for date-only
/// reminders) in `tz`.
pub fn reminder_instant(
    date: NaiveDate,
    time: Option<NaiveTime>,
    default_time: NaiveTime,
    tz: Tz,
) -> DateTime<Tz> {
    resolve_local(date.and_time(time.unwrap_or(default_time)), tz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap_or_default()
    }

    #[test]
    fn london_gap_and_fold() {
        let tz = chrono_tz::Europe::London;
        assert_eq!(
            resolve_local(dt("2026-03-29 01:30"), tz).to_rfc3339(),
            "2026-03-29T02:30:00+01:00"
        );
        assert_eq!(
            resolve_local(dt("2026-10-25 01:30"), tz).to_rfc3339(),
            "2026-10-25T01:30:00+01:00"
        );
        assert_eq!(
            resolve_local(dt("2026-07-01 09:00"), tz).to_rfc3339(),
            "2026-07-01T09:00:00+01:00"
        );
    }
}
