//! The recurrence editor (`TaskEdit` "Repeat"): the structured form of a Tasks-plugin
//! recurrence rule, its compiler back to the canonical phrase (`vault-format`
//! `RecurrenceRule::to_phrase`), a localised summary, and the next occurrences.

use std::fmt::Write as _;

use chrono::{NaiveDate, Weekday};
use vault_format::tasks::{Frequency, MonthDay, NthWeekday, RecurrenceRule, parse_recurrence};

use crate::error::{CoreError, CoreResult};
use crate::format::labels::{DAYS, Labels, Lang, MONTHS, Plural, WEEKS, YEARS};
use crate::view::model::{
    MonthDayMode, RecurrenceCompose, RecurrenceForm, RecurrenceFrequency, RecurrencePreviewItem,
    WeekdayKind,
};

fn weekday_kind(w: Weekday) -> WeekdayKind {
    match w {
        Weekday::Mon => WeekdayKind::Mon,
        Weekday::Tue => WeekdayKind::Tue,
        Weekday::Wed => WeekdayKind::Wed,
        Weekday::Thu => WeekdayKind::Thu,
        Weekday::Fri => WeekdayKind::Fri,
        Weekday::Sat => WeekdayKind::Sat,
        Weekday::Sun => WeekdayKind::Sun,
    }
}

fn weekday_of(k: WeekdayKind) -> Weekday {
    match k {
        WeekdayKind::Mon => Weekday::Mon,
        WeekdayKind::Tue => Weekday::Tue,
        WeekdayKind::Wed => Weekday::Wed,
        WeekdayKind::Thu => Weekday::Thu,
        WeekdayKind::Fri => Weekday::Fri,
        WeekdayKind::Sat => Weekday::Sat,
        WeekdayKind::Sun => Weekday::Sun,
    }
}

/// The form of a parsed rule.
pub fn form_of(rule: &RecurrenceRule) -> RecurrenceForm {
    let (mode, days) = if rule.nth_weekday.is_some() {
        (MonthDayMode::NthWeekday, Vec::new())
    } else if rule.month_days == [MonthDay::Last] {
        (MonthDayMode::LastDay, Vec::new())
    } else if rule.month_days.is_empty() {
        (MonthDayMode::SameDay, Vec::new())
    } else {
        (
            MonthDayMode::Days,
            rule.month_days
                .iter()
                .map(|d| match d {
                    MonthDay::Day(d) => u32::from(*d),
                    // "the 1st and last": the last day is written as 31 (clamped).
                    MonthDay::Last => 31,
                })
                .collect(),
        )
    };
    RecurrenceForm {
        frequency: match rule.freq {
            Frequency::Daily => RecurrenceFrequency::Daily,
            Frequency::Weekly => RecurrenceFrequency::Weekly,
            Frequency::Monthly => RecurrenceFrequency::Monthly,
            Frequency::Yearly => RecurrenceFrequency::Yearly,
        },
        interval: rule.interval,
        weekdays: rule.weekdays.iter().map(|w| weekday_kind(*w)).collect(),
        month_day_mode: mode,
        month_days: days,
        nth: rule.nth_weekday.map_or(0, |n| i32::from(n.nth)),
        nth_weekday: rule.nth_weekday.map(|n| weekday_kind(n.weekday)),
        months: rule.months.clone(),
        when_done: rule.when_done,
    }
}

/// The form of a phrase (`None`: outside the grammar).
pub fn form_of_phrase(phrase: &str) -> Option<RecurrenceForm> {
    parse_recurrence(phrase).ok().map(|r| form_of(&r))
}

/// The rule of a form (validated).
pub fn rule_of(form: &RecurrenceForm) -> CoreResult<RecurrenceRule> {
    if form.interval == 0 {
        return Err(CoreError::invalid("interval", "zero"));
    }
    let monthly = matches!(
        form.frequency,
        RecurrenceFrequency::Monthly | RecurrenceFrequency::Yearly
    );
    let mut weekdays: Vec<Weekday> = if form.frequency == RecurrenceFrequency::Weekly {
        form.weekdays.iter().map(|w| weekday_of(*w)).collect()
    } else {
        Vec::new()
    };
    weekdays.sort_by_key(Weekday::num_days_from_monday);
    weekdays.dedup();
    let mut month_days = Vec::new();
    let mut nth_weekday = None;
    if monthly {
        match form.month_day_mode {
            MonthDayMode::SameDay => {}
            MonthDayMode::LastDay => month_days.push(MonthDay::Last),
            MonthDayMode::Days => {
                for d in &form.month_days {
                    let d = u8::try_from(*d)
                        .ok()
                        .filter(|d| (1..=31).contains(d))
                        .ok_or_else(|| CoreError::invalid("month_days", "out_of_range"))?;
                    month_days.push(MonthDay::Day(d));
                }
                if month_days.is_empty() {
                    return Err(CoreError::invalid("month_days", "empty"));
                }
                month_days.sort();
                month_days.dedup();
            }
            MonthDayMode::NthWeekday => {
                let nth = i8::try_from(form.nth)
                    .ok()
                    .filter(|n| (1..=5).contains(&n.abs()))
                    .ok_or_else(|| CoreError::invalid("nth", "out_of_range"))?;
                let weekday = form
                    .nth_weekday
                    .map(weekday_of)
                    .ok_or_else(|| CoreError::invalid("nth_weekday", "missing"))?;
                nth_weekday = Some(NthWeekday { nth, weekday });
            }
        }
    }
    let mut months: Vec<u32> = if form.frequency == RecurrenceFrequency::Yearly {
        form.months.clone()
    } else {
        Vec::new()
    };
    if months.iter().any(|m| !(1..=12).contains(m)) {
        return Err(CoreError::invalid("months", "out_of_range"));
    }
    months.sort_unstable();
    months.dedup();
    Ok(RecurrenceRule {
        freq: match form.frequency {
            RecurrenceFrequency::Daily => Frequency::Daily,
            RecurrenceFrequency::Weekly => Frequency::Weekly,
            RecurrenceFrequency::Monthly => Frequency::Monthly,
            RecurrenceFrequency::Yearly => Frequency::Yearly,
        },
        interval: form.interval,
        weekdays,
        month_days,
        nth_weekday,
        months,
        when_done: form.when_done,
    })
}

fn every(n: u32, p: Plural, en_one: &str, ar_one: &str, lang: Lang) -> String {
    match (lang, n) {
        (Lang::En, 1) => format!("Every {en_one}"),
        (Lang::Ar, 1) => format!("كل {ar_one}"),
        (Lang::En, _) => format!("Every {}", p.of(i64::from(n), lang)),
        (Lang::Ar, _) => format!("كل {}", p.of(i64::from(n), lang)),
    }
}

/// A localised summary of a rule ("Every month on the 1st", "كل شهر في يوم 1").
pub fn summary(rule: &RecurrenceRule, labels: &Labels) -> String {
    match labels.lang {
        Lang::En => {
            let p = rule.to_phrase();
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
        Lang::Ar => {
            let lang = Lang::Ar;
            let n = rule.interval;
            let ref_day = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap_or_default(); // a Monday
            let wd = |w: Weekday| {
                labels.weekday_long(
                    ref_day + chrono::Duration::days(i64::from(w.num_days_from_monday())),
                )
            };
            let mut s = match rule.freq {
                Frequency::Daily => every(n, DAYS, "day", "يوم", lang),
                Frequency::Weekly => {
                    let mut s = every(n, WEEKS, "week", "أسبوع", lang);
                    if !rule.weekdays.is_empty() {
                        let names: Vec<String> = rule.weekdays.iter().map(|w| wd(*w)).collect();
                        let _ = write!(s, " يوم {}", names.join(" و"));
                    }
                    s
                }
                Frequency::Monthly => every(n, MONTHS, "month", "شهر", lang),
                Frequency::Yearly => every(n, YEARS, "year", "سنة", lang),
            };
            if let Some(nw) = rule.nth_weekday {
                let pos = if nw.nth < 0 {
                    format!("الأخير ({})", nw.nth.unsigned_abs())
                } else {
                    nw.nth.to_string()
                };
                let _ = write!(s, " في {} رقم {pos}", wd(nw.weekday));
            } else if !rule.month_days.is_empty() {
                let days: Vec<String> = rule
                    .month_days
                    .iter()
                    .map(|d| match d {
                        MonthDay::Day(d) => d.to_string(),
                        MonthDay::Last => "الأخير".to_owned(),
                    })
                    .collect();
                let _ = write!(s, " في يوم {}", days.join(" و"));
            }
            if !rule.months.is_empty() {
                let names: Vec<String> = rule
                    .months
                    .iter()
                    .filter_map(|m| NaiveDate::from_ymd_opt(2026, *m, 1))
                    .map(|d| {
                        labels
                            .day_month(d)
                            .split_once(' ')
                            .map(|(_, m)| m.to_owned())
                            .unwrap_or_default()
                    })
                    .collect();
                let _ = write!(s, " من {}", names.join(" و"));
            }
            if rule.when_done {
                s.push_str(" بعد الإنجاز");
            }
            s
        }
    }
}

/// Compiles a form to its phrase.
pub fn compose(form: &RecurrenceForm, labels: &Labels) -> CoreResult<RecurrenceCompose> {
    let rule = rule_of(form)?;
    let phrase = rule.to_phrase();
    let understood = parse_recurrence(&phrase).is_ok_and(|r| r == rule);
    Ok(RecurrenceCompose {
        label: summary(&rule, labels),
        phrase,
        understood,
    })
}

/// The first `count` occurrences starting at `from` (the current due date is the first).
pub fn preview(
    rule: &RecurrenceRule,
    from: NaiveDate,
    count: u32,
    labels: &Labels,
) -> Vec<RecurrencePreviewItem> {
    let mut out = Vec::new();
    let mut date = Some(from);
    while let Some(d) = date {
        if out.len() >= count as usize {
            break;
        }
        let first = out.is_empty();
        out.push(RecurrencePreviewItem {
            date: d,
            label: if first {
                labels.weekday_date_year(d)
            } else {
                labels.weekday_date(d)
            },
            is_due: first,
        });
        date = rule.next_after(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn labels(lang: Lang) -> Labels {
        Labels::new(
            DateTime::parse_from_rfc3339("2026-09-27T11:32:00Z")
                .expect("ts")
                .with_timezone(&Utc),
            chrono_tz::Africa::Cairo,
            lang,
        )
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
    }

    #[test]
    fn form_round_trips_through_the_phrase() {
        let form = form_of_phrase("every month on the 1st").expect("understood");
        assert_eq!(
            form,
            RecurrenceForm {
                frequency: RecurrenceFrequency::Monthly,
                interval: 1,
                weekdays: vec![],
                month_day_mode: MonthDayMode::Days,
                month_days: vec![1],
                nth: 0,
                nth_weekday: None,
                months: vec![],
                when_done: false,
            }
        );
        let c = compose(&form, &labels(Lang::En)).expect("valid");
        assert_eq!(
            c,
            RecurrenceCompose {
                phrase: "every month on the 1st".into(),
                understood: true,
                label: "Every month on the 1st".into(),
            }
        );
        let weekly = RecurrenceForm {
            frequency: RecurrenceFrequency::Weekly,
            interval: 2,
            weekdays: vec![WeekdayKind::Thu, WeekdayKind::Mon, WeekdayKind::Mon],
            month_day_mode: MonthDayMode::SameDay,
            month_days: vec![],
            nth: 0,
            nth_weekday: None,
            months: vec![],
            when_done: true,
        };
        assert_eq!(
            compose(&weekly, &labels(Lang::En)).expect("valid").phrase,
            "every 2 weeks on Monday and Thursday when done"
        );
        assert_eq!(
            compose(&weekly, &labels(Lang::Ar)).expect("valid").label,
            "كل أسبوعين يوم الاثنين والخميس بعد الإنجاز"
        );
        let nth = RecurrenceForm {
            frequency: RecurrenceFrequency::Monthly,
            month_day_mode: MonthDayMode::NthWeekday,
            nth: -1,
            nth_weekday: Some(WeekdayKind::Fri),
            ..weekly.clone()
        };
        assert_eq!(
            compose(&nth, &labels(Lang::En)).expect("valid").phrase,
            "every 2 months on the last Friday when done"
        );
        assert_eq!(form_of_phrase("every blue moon"), None);
        assert_eq!(
            compose(
                &RecurrenceForm {
                    interval: 0,
                    ..weekly
                },
                &labels(Lang::En)
            ),
            Err(CoreError::invalid("interval", "zero"))
        );
    }

    #[test]
    fn preview_lists_next_dates() {
        let rule = parse_recurrence("every month on the 1st").expect("rule");
        let items = preview(&rule, d("2026-10-01"), 3, &labels(Lang::En));
        assert_eq!(
            items,
            vec![
                RecurrencePreviewItem {
                    date: d("2026-10-01"),
                    label: "Thu 1 Oct 2026".into(),
                    is_due: true
                },
                RecurrencePreviewItem {
                    date: d("2026-11-01"),
                    label: "Sun 1 Nov".into(),
                    is_due: false
                },
                RecurrencePreviewItem {
                    date: d("2026-12-01"),
                    label: "Tue 1 Dec".into(),
                    is_due: false
                },
            ]
        );
        let clamp = parse_recurrence("every month on the 31st").expect("rule");
        let dates: Vec<NaiveDate> = preview(&clamp, d("2027-01-31"), 3, &labels(Lang::En))
            .into_iter()
            .map(|i| i.date)
            .collect();
        assert_eq!(
            dates,
            vec![d("2027-01-31"), d("2027-02-28"), d("2027-03-31")]
        );
    }
}
