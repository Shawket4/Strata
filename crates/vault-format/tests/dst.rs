//! Reminder and occurrence times across DST transitions in Africa/Cairo and Europe/London.
//!
//! 2026 transitions:
//! - London: 29 March 01:00 GMT → 02:00 BST (01:00–01:59 skipped); 25 October 02:00 BST →
//!   01:00 GMT (01:00–01:59 repeated).
//! - Cairo: 24 April (last Friday) 00:00 EET → 01:00 EEST (00:00–00:59 skipped); 29 October
//!   (last Thursday) 24:00 EEST → 23:00 EET (23:00–23:59 repeated).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Africa::Cairo;
use chrono_tz::Europe::London;
use chrono_tz::Tz;
use pretty_assertions::assert_eq;
use vault_format::tasks::{
    TaskLine, complete_recurring, next_occurrence_at, parse_recurrence, reminder_instant,
    resolve_local,
};

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
}

fn resolved(s: &str, tz: Tz) -> String {
    resolve_local(at(s), tz).to_rfc3339()
}

#[test]
fn london_resolution() {
    assert_eq!(resolved("2026-03-29 00:59", London), "2026-03-29T00:59:00+00:00");
    assert_eq!(resolved("2026-03-29 01:00", London), "2026-03-29T02:00:00+01:00");
    assert_eq!(resolved("2026-03-29 01:30", London), "2026-03-29T02:30:00+01:00");
    assert_eq!(resolved("2026-03-29 02:00", London), "2026-03-29T02:00:00+01:00");
    assert_eq!(resolved("2026-10-25 00:59", London), "2026-10-25T00:59:00+01:00");
    assert_eq!(resolved("2026-10-25 01:30", London), "2026-10-25T01:30:00+01:00");
    assert_eq!(resolved("2026-10-25 02:00", London), "2026-10-25T02:00:00+00:00");
}

#[test]
fn cairo_resolution() {
    assert_eq!(resolved("2026-04-23 23:59", Cairo), "2026-04-23T23:59:00+02:00");
    assert_eq!(resolved("2026-04-24 00:30", Cairo), "2026-04-24T01:30:00+03:00");
    assert_eq!(resolved("2026-04-24 01:00", Cairo), "2026-04-24T01:00:00+03:00");
    assert_eq!(resolved("2026-10-29 22:59", Cairo), "2026-10-29T22:59:00+03:00");
    assert_eq!(resolved("2026-10-29 23:30", Cairo), "2026-10-29T23:30:00+03:00");
    assert_eq!(resolved("2026-10-30 00:00", Cairo), "2026-10-30T00:00:00+02:00");
}

#[test]
fn weekly_reminder_keeps_wall_clock_time_across_transitions() {
    let weekly = parse_recurrence("every week").unwrap();
    let next = |s: &str, tz| next_occurrence_at(&weekly, at(s), tz).unwrap().to_rfc3339();
    assert_eq!(next("2026-03-22 09:00", London), "2026-03-29T09:00:00+01:00");
    assert_eq!(next("2026-10-18 09:00", London), "2026-10-25T09:00:00+00:00");
    assert_eq!(next("2026-04-17 09:00", Cairo), "2026-04-24T09:00:00+03:00");
    assert_eq!(next("2026-10-22 09:00", Cairo), "2026-10-29T09:00:00+03:00");
    assert_eq!(next("2026-10-23 09:00", Cairo), "2026-10-30T09:00:00+02:00");
}

#[test]
fn occurrences_landing_in_gaps_and_folds() {
    let daily = parse_recurrence("every day").unwrap();
    let monthly = parse_recurrence("every month on the 24th").unwrap();
    let next = |r, s: &str, tz| next_occurrence_at(r, at(s), tz).unwrap().to_rfc3339();
    assert_eq!(next(&daily, "2026-03-28 01:30", London), "2026-03-29T02:30:00+01:00");
    assert_eq!(next(&daily, "2026-10-24 01:30", London), "2026-10-25T01:30:00+01:00");
    assert_eq!(next(&monthly, "2026-03-24 00:30", Cairo), "2026-04-24T01:30:00+03:00");
    assert_eq!(next(&daily, "2026-10-28 23:30", Cairo), "2026-10-29T23:30:00+03:00");
}

#[test]
fn completed_task_reminders_fire_at_the_same_local_time() {
    let line = "- [ ] Weekly call 🔁 every week on Sunday 📅 2026-10-18 (@2026-10-18 09:00) ^t-a";
    let done = NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
    let [next, _] = complete_recurring(line, done, "t-b").unwrap();
    assert_eq!(
        next,
        "- [ ] Weekly call 🔁 every week on Sunday 📅 2026-10-25 (@2026-10-25 09:00) ^t-b"
    );
    let reminder = TaskLine::parse(&next).unwrap().reminders()[0];
    let nine = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let before = reminder_instant(NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(), Some(nine), nine, London);
    let after = reminder_instant(reminder.date, reminder.time, nine, London);
    assert_eq!(before.to_rfc3339(), "2026-10-18T09:00:00+01:00");
    assert_eq!(after.to_rfc3339(), "2026-10-25T09:00:00+00:00");
    // Exactly 7 days + 1 hour apart in absolute time.
    assert_eq!((after - before).num_minutes(), 7 * 24 * 60 + 60);
}

#[test]
fn date_only_reminders_use_the_default_time() {
    let eight = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
    let d = NaiveDate::from_ymd_opt(2026, 4, 24).unwrap();
    assert_eq!(reminder_instant(d, None, eight, Cairo).to_rfc3339(), "2026-04-24T08:00:00+03:00");
    let midnight = NaiveTime::from_hms_opt(0, 15, 0).unwrap();
    assert_eq!(reminder_instant(d, None, midnight, Cairo).to_rfc3339(), "2026-04-24T01:15:00+03:00");
}
