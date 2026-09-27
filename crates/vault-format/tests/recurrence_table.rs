//! Recurrence grammar → RRULE → next occurrence, as a table. Weekdays in these cases were
//! checked independently (2026-09-27 is a Sunday; 2026-01-01 a Thursday; 2028 is a leap
//! year).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use vault_format::tasks::parse_recurrence;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// (phrase, reference date, RRULE, RRULE anchored at the reference, next occurrence)
const CASES: &[(&str, &str, &str, &str, &str)] = &[
    // daily
    ("every day", "2026-09-27", "FREQ=DAILY", "FREQ=DAILY", "2026-09-28"),
    ("every 3 days", "2026-09-27", "FREQ=DAILY;INTERVAL=3", "FREQ=DAILY;INTERVAL=3", "2026-09-30"),
    ("every other day", "2026-12-31", "FREQ=DAILY;INTERVAL=2", "FREQ=DAILY;INTERVAL=2", "2027-01-02"),
    ("every 10 days when done", "2026-02-25", "FREQ=DAILY;INTERVAL=10", "FREQ=DAILY;INTERVAL=10", "2026-03-07"),
    ("every day", "2028-02-28", "FREQ=DAILY", "FREQ=DAILY", "2028-02-29"),
    ("every day", "2027-02-28", "FREQ=DAILY", "FREQ=DAILY", "2027-03-01"),
    // weekdays / weekly
    ("every weekday", "2026-09-25", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "2026-09-28"),
    ("every weekday", "2026-09-26", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "2026-09-28"),
    ("every weekday", "2026-09-28", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "2026-09-29"),
    ("every week", "2026-09-27", "FREQ=WEEKLY", "FREQ=WEEKLY;BYDAY=SU", "2026-10-04"),
    ("every week on Sunday", "2026-09-27", "FREQ=WEEKLY;BYDAY=SU", "FREQ=WEEKLY;BYDAY=SU", "2026-10-04"),
    ("every week on Sunday", "2026-09-23", "FREQ=WEEKLY;BYDAY=SU", "FREQ=WEEKLY;BYDAY=SU", "2026-09-27"),
    ("every week on Tuesday, Friday", "2026-09-29", "FREQ=WEEKLY;BYDAY=TU,FR", "FREQ=WEEKLY;BYDAY=TU,FR", "2026-10-02"),
    ("every week on Friday and Tuesday", "2026-10-02", "FREQ=WEEKLY;BYDAY=TU,FR", "FREQ=WEEKLY;BYDAY=TU,FR", "2026-10-06"),
    ("every 2 weeks on Monday", "2026-09-28", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO", "2026-10-12"),
    ("every 2 weeks on Monday, Thursday", "2026-09-28", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH", "2026-10-01"),
    ("every 2 weeks on Monday, and Thursday", "2026-10-01", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TH", "2026-10-12"),
    ("every other week", "2026-09-27", "FREQ=WEEKLY;INTERVAL=2", "FREQ=WEEKLY;INTERVAL=2;BYDAY=SU", "2026-10-11"),
    ("every Monday", "2026-09-27", "FREQ=WEEKLY;BYDAY=MO", "FREQ=WEEKLY;BYDAY=MO", "2026-09-28"),
    ("every mon and fri", "2026-09-28", "FREQ=WEEKLY;BYDAY=MO,FR", "FREQ=WEEKLY;BYDAY=MO,FR", "2026-10-02"),
    ("every 3 weeks", "2026-01-01", "FREQ=WEEKLY;INTERVAL=3", "FREQ=WEEKLY;INTERVAL=3;BYDAY=TH", "2026-01-22"),
    ("every week on Sunday when done", "2026-09-30", "FREQ=WEEKLY;BYDAY=SU", "FREQ=WEEKLY;BYDAY=SU", "2026-10-04"),
    // monthly by day, with month-end clamping
    ("every month", "2026-01-15", "FREQ=MONTHLY", "FREQ=MONTHLY;BYMONTHDAY=15", "2026-02-15"),
    ("every month", "2026-01-31", "FREQ=MONTHLY", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2026-02-28"),
    ("every month", "2028-01-31", "FREQ=MONTHLY", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2028-02-29"),
    ("every month", "2026-03-31", "FREQ=MONTHLY", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2026-04-30"),
    ("every month on the 1st", "2026-09-01", "FREQ=MONTHLY;BYMONTHDAY=1", "FREQ=MONTHLY;BYMONTHDAY=1", "2026-10-01"),
    ("every month on the 1st", "2026-09-15", "FREQ=MONTHLY;BYMONTHDAY=1", "FREQ=MONTHLY;BYMONTHDAY=1", "2026-10-01"),
    ("Every Month On The 1st", "2026-12-10", "FREQ=MONTHLY;BYMONTHDAY=1", "FREQ=MONTHLY;BYMONTHDAY=1", "2027-01-01"),
    ("every month on the 31st", "2026-01-31", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2026-02-28"),
    ("every month on the 31st", "2026-02-28", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2026-03-31"),
    ("every month on the 31st", "2028-01-31", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "FREQ=MONTHLY;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2028-02-29"),
    ("every month on the 30th", "2026-01-30", "FREQ=MONTHLY;BYMONTHDAY=28,29,30;BYSETPOS=-1", "FREQ=MONTHLY;BYMONTHDAY=28,29,30;BYSETPOS=-1", "2026-02-28"),
    ("every month on the 29th", "2027-01-29", "FREQ=MONTHLY;BYMONTHDAY=28,29;BYSETPOS=-1", "FREQ=MONTHLY;BYMONTHDAY=28,29;BYSETPOS=-1", "2027-02-28"),
    ("every month on the last", "2026-01-31", "FREQ=MONTHLY;BYMONTHDAY=-1", "FREQ=MONTHLY;BYMONTHDAY=-1", "2026-02-28"),
    ("every month on the last day", "2028-02-10", "FREQ=MONTHLY;BYMONTHDAY=-1", "FREQ=MONTHLY;BYMONTHDAY=-1", "2028-02-29"),
    ("every month on the 1st and 15th", "2026-09-01", "FREQ=MONTHLY;BYMONTHDAY=1,15", "FREQ=MONTHLY;BYMONTHDAY=1,15", "2026-09-15"),
    ("every month on the 15th and last", "2026-09-15", "FREQ=MONTHLY;BYMONTHDAY=15,-1", "FREQ=MONTHLY;BYMONTHDAY=15,-1", "2026-09-30"),
    ("every 3 months", "2026-11-30", "FREQ=MONTHLY;INTERVAL=3", "FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=28,29,30;BYSETPOS=-1", "2027-02-28"),
    ("every 2 months on the 31st", "2026-12-31", "FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=28,29,30,31;BYSETPOS=-1", "2027-02-28"),
    // monthly by nth weekday
    ("every month on the 2nd Wednesday", "2026-09-09", "FREQ=MONTHLY;BYDAY=2WE", "FREQ=MONTHLY;BYDAY=2WE", "2026-10-14"),
    ("every month on the second wednesday", "2026-09-01", "FREQ=MONTHLY;BYDAY=2WE", "FREQ=MONTHLY;BYDAY=2WE", "2026-09-09"),
    ("every month on the last Friday", "2026-09-25", "FREQ=MONTHLY;BYDAY=-1FR", "FREQ=MONTHLY;BYDAY=-1FR", "2026-10-30"),
    ("every month on the 2nd last Friday", "2026-09-18", "FREQ=MONTHLY;BYDAY=-2FR", "FREQ=MONTHLY;BYDAY=-2FR", "2026-10-23"),
    ("every 6 months on the 2nd Wednesday", "2026-01-14", "FREQ=MONTHLY;INTERVAL=6;BYDAY=2WE", "FREQ=MONTHLY;INTERVAL=6;BYDAY=2WE", "2026-07-08"),
    // yearly, leap years
    ("every year", "2026-03-15", "FREQ=YEARLY", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15", "2027-03-15"),
    ("every year", "2028-02-29", "FREQ=YEARLY", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1", "2029-02-28"),
    ("every year on February 29", "2026-03-01", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1", "2027-02-28"),
    ("every year on February 29", "2027-02-28", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1", "2028-02-29"),
    ("every 2 years", "2026-06-01", "FREQ=YEARLY;INTERVAL=2", "FREQ=YEARLY;INTERVAL=2;BYMONTH=6;BYMONTHDAY=1", "2028-06-01"),
    ("every year on March 31", "2027-03-31", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=31", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=31", "2028-03-31"),
    ("every year on march 5th", "2026-01-01", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=5", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=5", "2026-03-05"),
    ("every year on 5 March", "2026-03-05", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=5", "FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=5", "2027-03-05"),
    ("every year on the 25th of December", "2026-12-25", "FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=25", "FREQ=YEARLY;BYMONTH=12;BYMONTHDAY=25", "2027-12-25"),
    ("every January on the 15th", "2026-01-15", "FREQ=YEARLY;BYMONTH=1;BYMONTHDAY=15", "FREQ=YEARLY;BYMONTH=1;BYMONTHDAY=15", "2027-01-15"),
    ("every April and December on the 1st and 24th", "2026-04-24", "FREQ=YEARLY;BYMONTH=4,12;BYMONTHDAY=1,24", "FREQ=YEARLY;BYMONTH=4,12;BYMONTHDAY=1,24", "2026-12-01"),
    ("every February on the last", "2026-03-01", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=-1", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=-1", "2027-02-28"),
    ("every February on the last", "2027-03-01", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=-1", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=-1", "2028-02-29"),
    ("every November on the 4th Thursday", "2026-11-26", "FREQ=YEARLY;BYMONTH=11;BYDAY=4TH", "FREQ=YEARLY;BYMONTH=11;BYDAY=4TH", "2027-11-25"),
];

#[test]
fn recurrence_table() {
    assert!(CASES.len() >= 40, "table has {} cases", CASES.len());
    for &(phrase, reference, rrule, anchored, next) in CASES {
        let rule = parse_recurrence(phrase).unwrap_or_else(|e| panic!("{phrase}: {e}"));
        assert_eq!(rule.to_rrule(), rrule, "rrule of {phrase:?}");
        assert_eq!(rule.to_rrule_anchored(d(reference)), anchored, "anchored rrule of {phrase:?}");
        assert_eq!(rule.next_after(d(reference)), Some(d(next)), "next of {phrase:?} after {reference}");
    }
}

#[test]
fn when_done_flag() {
    assert!(parse_recurrence("every 10 days when done").unwrap().when_done);
    assert!(parse_recurrence("every week on Sunday WHEN DONE").unwrap().when_done);
    assert!(!parse_recurrence("every day").unwrap().when_done);
}

#[test]
fn chained_occurrences_follow_month_ends() {
    // "on the last day" never drifts; plain "every month" from the 31st drifts after
    // February, exactly as the Tasks plugin does (each completion counts from the new date).
    let last = parse_recurrence("every month on the last day").unwrap();
    let mut date = d("2027-12-31");
    let mut seen = Vec::new();
    for _ in 0..4 {
        date = last.next_after(date).unwrap();
        seen.push(date.to_string());
    }
    assert_eq!(seen, ["2028-01-31", "2028-02-29", "2028-03-31", "2028-04-30"]);

    let plain = parse_recurrence("every month").unwrap();
    let mut date = d("2026-01-31");
    let mut seen = Vec::new();
    for _ in 0..3 {
        date = plain.next_after(date).unwrap();
        seen.push(date.to_string());
    }
    assert_eq!(seen, ["2026-02-28", "2026-03-28", "2026-04-28"]);
}

#[test]
fn unsupported_phrases_are_reported_verbatim() {
    for phrase in [
        "every blue moon",
        "every day at 9am",
        "كل يوم",
        "every 0 days",
        "every month on the 32nd",
        "every February on the 30th",
        "every year on June 31",
        "every week on Caturday",
        "every 2 weekdays",
        "each day",
    ] {
        let err = parse_recurrence(phrase).unwrap_err();
        assert_eq!(err.phrase, phrase);
    }
}
