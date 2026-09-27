//! Task lines: parsing keeps the text, edits never panic and stay parseable, recurrence
//! parsing and completion never panic.
#![no_main]

use chrono::NaiveDate;
use libfuzzer_sys::fuzz_target;
use vault_format::tasks::{self, DateKind, TaskLine};

fuzz_target!(|line: &str| {
    let _ = tasks::parse_recurrence(line);
    let Some(t) = TaskLine::parse(line) else {
        return;
    };
    assert_eq!(t.as_str(), line);
    let day = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    for kind in DateKind::ALL {
        let edited = t.with_date(kind, Some(day));
        assert_eq!(edited.date(kind), Some(day));
        assert!(TaskLine::parse(edited.as_str()).is_some());
        assert_eq!(edited.with_date(kind, None).date(kind), None);
    }
    let _ = t.with_block_id("t-fuzz").with_reminders(&[]);
    if let Some(Ok(rule)) = t.recurrence() {
        let _ = rule.to_rrule();
        let _ = rule.next_after(day);
    }
    if let Ok([next, done]) = tasks::complete_recurring(line, day, "t-next") {
        assert!(TaskLine::parse(&next).is_some());
        assert!(TaskLine::parse(&done).is_some());
    }
});
