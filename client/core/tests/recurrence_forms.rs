//! The "Repeat" editor of a task (PLAN §11 screen 7): every shape of the structured form
//! compiles to the canonical Tasks phrase and back, invalid forms are refused with the field
//! at fault, the summary reads in both languages, and the preview lists the next dates.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use chrono::{DateTime, NaiveDate, Utc};
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::format::labels::{Labels, Lang};
use strata_core::format::recurrence::{compose, form_of_phrase, preview, rule_of, summary};
use strata_core::view::model::{
    MonthDayMode, RecurrenceCompose, RecurrenceForm, RecurrenceFrequency, WeekdayKind,
};

fn labels(lang: Lang) -> Labels {
    Labels::new(
        DateTime::parse_from_rfc3339("2026-09-27T11:32:00Z")
            .expect("ts")
            .with_timezone(&Utc),
        chrono_tz::Africa::Cairo,
        lang,
    )
}

fn form(frequency: RecurrenceFrequency) -> RecurrenceForm {
    RecurrenceForm {
        frequency,
        interval: 1,
        weekdays: Vec::new(),
        month_day_mode: MonthDayMode::SameDay,
        month_days: Vec::new(),
        nth: 0,
        nth_weekday: None,
        months: Vec::new(),
        when_done: false,
    }
}

fn invalid(field: &str, reason: &str) -> CoreError {
    CoreError::InvalidInput {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

/// `(phrase, English label, Arabic label)` of a form.
fn compiled(f: &RecurrenceForm) -> (String, String, String) {
    let en = compose(f, &labels(Lang::En)).expect("valid");
    let ar = compose(f, &labels(Lang::Ar)).expect("valid");
    assert!(en.understood, "{} parses back to the same rule", en.phrase);
    assert_eq!(en.phrase, ar.phrase);
    (en.phrase, en.label, ar.label)
}

#[test]
fn every_shape_compiles_to_the_canonical_phrase_and_reads_in_both_languages() {
    use RecurrenceFrequency::{Daily, Monthly, Weekly, Yearly};
    use WeekdayKind::{Fri, Mon, Sat, Sun, Thu, Tue, Wed};
    let cases: Vec<(RecurrenceForm, (&str, &str, &str))> = vec![
        (form(Daily), ("every day", "Every day", "كل يوم")),
        (
            RecurrenceForm {
                interval: 3,
                when_done: true,
                ..form(Daily)
            },
            (
                "every 3 days when done",
                "Every 3 days when done",
                "كل 3 أيام بعد الإنجاز",
            ),
        ),
        (
            RecurrenceForm {
                // Order and repeats in the form do not matter; weekdays of a daily rule are
                // dropped.
                weekdays: vec![Sun, Wed, Mon, Wed, Tue, Thu, Fri, Sat],
                ..form(Weekly)
            },
            (
                "every Monday, Tuesday, Wednesday, Thursday, Friday, Saturday and Sunday",
                "Every Monday, Tuesday, Wednesday, Thursday, Friday, Saturday and Sunday",
                "كل أسبوع يوم الاثنين والثلاثاء والأربعاء والخميس والجمعة والسبت والأحد",
            ),
        ),
        (
            RecurrenceForm {
                interval: 2,
                ..form(Weekly)
            },
            ("every 2 weeks", "Every 2 weeks", "كل أسبوعين"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::LastDay,
                ..form(Monthly)
            },
            (
                "every month on the last",
                "Every month on the last",
                "كل شهر في يوم الأخير",
            ),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::Days,
                month_days: vec![15, 1, 15],
                interval: 2,
                ..form(Monthly)
            },
            (
                "every 2 months on the 1st and 15th",
                "Every 2 months on the 1st and 15th",
                "كل شهرين في يوم 1 و15",
            ),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::NthWeekday,
                nth: 2,
                nth_weekday: Some(Tue),
                ..form(Monthly)
            },
            (
                "every month on the 2nd Tuesday",
                "Every month on the 2nd Tuesday",
                "كل شهر في الثلاثاء رقم 2",
            ),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::NthWeekday,
                nth: -1,
                nth_weekday: Some(Fri),
                ..form(Monthly)
            },
            (
                "every month on the last Friday",
                "Every month on the last Friday",
                "كل شهر في الجمعة رقم الأخير (1)",
            ),
        ),
        (
            RecurrenceForm {
                months: vec![9, 3],
                month_day_mode: MonthDayMode::Days,
                month_days: vec![5],
                ..form(Yearly)
            },
            (
                "every March and September on the 5th",
                "Every March and September on the 5th",
                "كل سنة في يوم 5 من مارس وسبتمبر",
            ),
        ),
        (
            RecurrenceForm {
                interval: 11,
                ..form(Yearly)
            },
            ("every 11 years", "Every 11 years", "كل 11 سنة"),
        ),
    ];
    for (f, (phrase, en, ar)) in cases {
        assert_eq!(
            compiled(&f),
            (phrase.to_owned(), en.to_owned(), ar.to_owned()),
            "{f:?}"
        );
        // The phrase opens in the same editor state (canonicalised).
        let back = form_of_phrase(phrase).expect("understood");
        assert_eq!(rule_of(&back), rule_of(&f), "{phrase}");
    }
    assert_eq!(form_of_phrase("every blue moon"), None);
}

#[test]
fn invalid_forms_name_the_field_at_fault() {
    use RecurrenceFrequency::{Daily, Monthly, Yearly};
    let cases = [
        (
            RecurrenceForm {
                interval: 0,
                ..form(Daily)
            },
            invalid("interval", "zero"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::Days,
                month_days: vec![32],
                ..form(Monthly)
            },
            invalid("month_days", "out_of_range"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::Days,
                month_days: vec![0],
                ..form(Monthly)
            },
            invalid("month_days", "out_of_range"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::Days,
                ..form(Monthly)
            },
            invalid("month_days", "empty"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::NthWeekday,
                nth: 6,
                nth_weekday: Some(WeekdayKind::Mon),
                ..form(Monthly)
            },
            invalid("nth", "out_of_range"),
        ),
        (
            RecurrenceForm {
                month_day_mode: MonthDayMode::NthWeekday,
                nth: 1,
                ..form(Yearly)
            },
            invalid("nth_weekday", "missing"),
        ),
        (
            RecurrenceForm {
                months: vec![13],
                ..form(Yearly)
            },
            invalid("months", "out_of_range"),
        ),
    ];
    for (f, err) in cases {
        assert_eq!(compose(&f, &labels(Lang::En)), Err(err), "{f:?}");
    }
    // Month settings of a daily rule are ignored rather than refused.
    assert_eq!(
        compose(
            &RecurrenceForm {
                month_day_mode: MonthDayMode::Days,
                month_days: vec![40],
                ..form(Daily)
            },
            &labels(Lang::En)
        ),
        Ok(RecurrenceCompose {
            phrase: "every day".to_owned(),
            understood: true,
            label: "Every day".to_owned(),
        })
    );
}

#[test]
fn the_preview_lists_the_due_date_then_the_next_ones() {
    let rule = rule_of(&RecurrenceForm {
        month_day_mode: MonthDayMode::LastDay,
        ..form(RecurrenceFrequency::Monthly)
    })
    .expect("rule");
    let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date");
    let items = preview(&rule, d("2026-10-31"), 3, &labels(Lang::En));
    assert_eq!(
        items
            .iter()
            .map(|i| (i.date, i.label.as_str(), i.is_due))
            .collect::<Vec<_>>(),
        [
            // The due date carries its year; the next ones are in this year.
            (d("2026-10-31"), "Sat 31 Oct 2026", true),
            (d("2026-11-30"), "Mon 30 Nov", false),
            (d("2026-12-31"), "Thu 31 Dec", false),
        ]
    );
    assert_eq!(preview(&rule, d("2027-01-31"), 0, &labels(Lang::En)), []);
    assert_eq!(summary(&rule, &labels(Lang::Ar)), "كل شهر في يوم الأخير");
}
