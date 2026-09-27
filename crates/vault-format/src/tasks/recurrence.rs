//! The Tasks plugin's "every …" recurrence language, compiled to RFC 5545 RRULEs, with a pure
//! next-occurrence calculation.
//!
//! Supported (case-insensitive; `,` and `and` separate list items):
//!
//! | Phrase | RRULE |
//! |---|---|
//! | `every day`, `every N days`, `every other day` | `FREQ=DAILY[;INTERVAL=N]` |
//! | `every weekday` | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR` |
//! | `every week`, `every N weeks [on <weekdays>]` | `FREQ=WEEKLY[;INTERVAL=N][;BYDAY=…]` |
//! | `every <weekdays>` (e.g. `every Monday and Thursday`) | `FREQ=WEEKLY;BYDAY=MO,TH` |
//! | `every [N] month[s] [on the <days>]` (`1st`, `15th and last`) | `FREQ=MONTHLY;BYMONTHDAY=…` |
//! | `every month on the last [day]` | `BYMONTHDAY=-1` |
//! | `every month on the <nth> <weekday>` (`2nd Wednesday`, `last Friday`, `2nd last Friday`) | `BYDAY=2WE` |
//! | `every [N] year[s] [on <month> <day>]` | `FREQ=YEARLY;BYMONTH=M;BYMONTHDAY=D` |
//! | `every <months> [on the <days>/<nth weekday>]` (`every January on the 15th`) | `FREQ=YEARLY;BYMONTH=…` |
//! | … `when done` | the next date counts from the completion date |
//!
//! Days that do not exist in a month clamp to its last day ("every month on the 31st" is
//! 28/29 February, 30 April); a single such day compiles to the equivalent RFC 5545 form
//! `BYMONTHDAY=28,29,30,31;BYSETPOS=-1`.

use std::fmt;

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// How often a rule repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Frequency {
    /// `FREQ=DAILY`
    Daily,
    /// `FREQ=WEEKLY`
    Weekly,
    /// `FREQ=MONTHLY`
    Monthly,
    /// `FREQ=YEARLY`
    Yearly,
}

/// A day of the month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MonthDay {
    /// Day 1–31, clamped to the month's length.
    Day(u8),
    /// The last day.
    Last,
}

/// "The 2nd Wednesday" (`nth` 1–5) or "the 2nd last Friday" (`nth` −1…−5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NthWeekday {
    /// Position: positive from the start of the month, negative from the end.
    pub nth: i8,
    /// Weekday.
    pub weekday: Weekday,
}

/// A parsed recurrence rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecurrenceRule {
    /// Frequency.
    pub freq: Frequency,
    /// Interval (≥ 1).
    pub interval: u32,
    /// Weekdays (weekly rules), Monday first.
    pub weekdays: Vec<Weekday>,
    /// Days of the month (monthly/yearly rules), sorted.
    pub month_days: Vec<MonthDay>,
    /// An nth weekday (monthly/yearly rules).
    pub nth_weekday: Option<NthWeekday>,
    /// Months 1–12 (yearly rules), sorted.
    pub months: Vec<u32>,
    /// `when done`: count from the completion date.
    pub when_done: bool,
}

/// A recurrence phrase outside the supported grammar. The phrase stays in the file verbatim.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("recurrence not understood: `{phrase}` ({reason})")]
pub struct RecurrenceNotUnderstood {
    /// The phrase as written.
    pub phrase: String,
    /// What went wrong.
    pub reason: String,
}

const WEEKDAYS: [(Weekday, &str, &str); 7] = [
    (Weekday::Mon, "monday", "mon"),
    (Weekday::Tue, "tuesday", "tue"),
    (Weekday::Wed, "wednesday", "wed"),
    (Weekday::Thu, "thursday", "thu"),
    (Weekday::Fri, "friday", "fri"),
    (Weekday::Sat, "saturday", "sat"),
    (Weekday::Sun, "sunday", "sun"),
];

const MONTHS: [(&str, &str); 12] = [
    ("january", "jan"),
    ("february", "feb"),
    ("march", "mar"),
    ("april", "apr"),
    ("may", "may"),
    ("june", "jun"),
    ("july", "jul"),
    ("august", "aug"),
    ("september", "sep"),
    ("october", "oct"),
    ("november", "nov"),
    ("december", "dec"),
];

const ORDINAL_WORDS: [&str; 5] = ["first", "second", "third", "fourth", "fifth"];

fn weekday(tok: &str) -> Option<Weekday> {
    let t = tok
        .strip_suffix('s')
        .filter(|t| t.ends_with("day"))
        .unwrap_or(tok);
    WEEKDAYS
        .iter()
        .find(|(_, full, short)| t == *full || t == *short)
        .map(|(w, _, _)| *w)
}

fn month(tok: &str) -> Option<u32> {
    MONTHS
        .iter()
        .position(|(full, short)| tok == *full || tok == *short)
        .and_then(|i| u32::try_from(i + 1).ok())
}

/// `1st`, `2nd`, `3rd`, `4th`, `21st`, bare `5`, or `first`…`fifth`.
fn ordinal(tok: &str) -> Option<u32> {
    if let Some(i) = ORDINAL_WORDS.iter().position(|w| *w == tok) {
        return u32::try_from(i + 1).ok();
    }
    let digits = tok.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &tok[digits.len()..];
    let n: u32 = digits.parse().ok()?;
    let expected = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    (suffix.is_empty() || suffix == expected).then_some(n)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map_or(31, |d| d.day())
}

fn max_days(month: u32) -> u32 {
    match month {
        2 => 29,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

struct Parser<'a> {
    toks: Vec<&'a str>,
    pos: usize,
}

type PResult<T> = Result<T, String>;

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.toks.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<&'a str> {
        self.toks.get(self.pos + n).copied()
    }

    fn next(&mut self) -> Option<&'a str> {
        let t = self.peek();
        self.pos += usize::from(t.is_some());
        t
    }

    fn eat(&mut self, word: &str) -> bool {
        if self.peek() == Some(word) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, word: &str) -> PResult<()> {
        if self.eat(word) {
            Ok(())
        } else {
            Err(format!("expected `{word}`"))
        }
    }

    /// Consumes a list separator (`,`, `and`, `, and`). Returns whether one was found.
    fn separator(&mut self) -> bool {
        let comma = self.eat(",");
        let and = self.eat("and");
        comma || and
    }

    fn weekday_list(&mut self) -> PResult<Vec<Weekday>> {
        let mut out = Vec::new();
        loop {
            let tok = self.next().ok_or("expected a weekday")?;
            out.push(weekday(tok).ok_or_else(|| format!("`{tok}` is not a weekday"))?);
            let save = self.pos;
            if !self.separator() {
                break;
            }
            if self.peek().and_then(weekday).is_none() {
                self.pos = save;
                break;
            }
        }
        out.sort_by_key(Weekday::num_days_from_monday);
        out.dedup();
        Ok(out)
    }

    /// After `on the`: day list, `last [day]`, or nth weekday.
    fn month_on(&mut self, rule: &mut RecurrenceRule) -> PResult<()> {
        self.expect("on")?;
        self.eat("the");
        // nth weekday: `2nd wednesday`, `last friday`, `2nd last friday`
        let first = self.peek().ok_or("expected a day")?;
        let (nth, consumed) = match (ordinal(first), self.peek_at(1)) {
            (Some(n), Some("last")) if self.peek_at(2).and_then(weekday).is_some() => {
                (Some(-i64::from(n)), 2)
            }
            (Some(n), Some(w)) if weekday(w).is_some() => (Some(i64::from(n)), 1),
            (None, Some(w)) if first == "last" && weekday(w).is_some() => (Some(-1), 1),
            _ => (None, 0),
        };
        if let Some(nth) = nth {
            self.pos += consumed;
            let wd = self.next().and_then(weekday).ok_or("expected a weekday")?;
            if !(1..=5).contains(&nth.abs()) {
                return Err("weekday position must be 1-5".into());
            }
            rule.nth_weekday = Some(NthWeekday {
                nth: i8::try_from(nth).map_err(|e| e.to_string())?,
                weekday: wd,
            });
            return Ok(());
        }
        loop {
            let tok = self.next().ok_or("expected a day")?;
            if tok == "last" {
                self.eat("day");
                rule.month_days.push(MonthDay::Last);
            } else {
                let n = ordinal(tok).ok_or_else(|| format!("`{tok}` is not a day of the month"))?;
                let d = u8::try_from(n)
                    .ok()
                    .filter(|d| (1..=31).contains(d))
                    .ok_or("day must be 1-31")?;
                rule.month_days.push(MonthDay::Day(d));
            }
            if !self.separator() {
                break;
            }
            self.eat("the");
        }
        rule.month_days.sort();
        rule.month_days.dedup();
        Ok(())
    }

    /// After `every year`: `on March 5[th]`, `on 5 March`, `on the 5th of March`.
    fn year_on(&mut self, rule: &mut RecurrenceRule) -> PResult<()> {
        self.expect("on")?;
        self.eat("the");
        let a = self.next().ok_or("expected a date")?;
        let (m, d) = if let Some(m) = month(a) {
            let d = self.next().and_then(ordinal).ok_or("expected a day")?;
            (m, d)
        } else {
            let d = ordinal(a).ok_or_else(|| format!("`{a}` is not a day"))?;
            self.eat("of");
            let m = self.next().and_then(month).ok_or("expected a month")?;
            (m, d)
        };
        if d == 0 || d > max_days(m) {
            return Err(format!("{} has no day {d}", MONTHS[(m - 1) as usize].0));
        }
        rule.months = vec![m];
        rule.month_days = vec![MonthDay::Day(u8::try_from(d).map_err(|e| e.to_string())?)];
        Ok(())
    }

    fn rule(&mut self) -> PResult<RecurrenceRule> {
        self.expect("every")?;
        let mut rule = RecurrenceRule {
            freq: Frequency::Daily,
            interval: 1,
            weekdays: Vec::new(),
            month_days: Vec::new(),
            nth_weekday: None,
            months: Vec::new(),
            when_done: false,
        };
        if self.toks.ends_with(&["when", "done"]) {
            rule.when_done = true;
            self.toks.truncate(self.toks.len() - 2);
        }
        let mut explicit_interval = false;
        if self.eat("other") {
            rule.interval = 2;
            explicit_interval = true;
        } else if let Some(n) = self.peek().and_then(|t| t.parse::<u32>().ok()) {
            if n == 0 {
                return Err("interval must be at least 1".into());
            }
            self.pos += 1;
            rule.interval = n;
            explicit_interval = true;
        }
        let unit = self.next().ok_or("expected a unit")?;
        match unit {
            "day" | "days" => rule.freq = Frequency::Daily,
            "weekday" | "weekdays" if !explicit_interval => {
                rule.freq = Frequency::Weekly;
                rule.weekdays = vec![
                    Weekday::Mon,
                    Weekday::Tue,
                    Weekday::Wed,
                    Weekday::Thu,
                    Weekday::Fri,
                ];
            }
            "week" | "weeks" => {
                rule.freq = Frequency::Weekly;
                if self.eat("on") {
                    rule.weekdays = self.weekday_list()?;
                }
            }
            "month" | "months" => {
                rule.freq = Frequency::Monthly;
                if self.peek() == Some("on") {
                    self.month_on(&mut rule)?;
                }
            }
            "year" | "years" => {
                rule.freq = Frequency::Yearly;
                if self.peek() == Some("on") {
                    self.year_on(&mut rule)?;
                }
            }
            t if weekday(t).is_some() && !explicit_interval => {
                self.pos -= 1;
                rule.freq = Frequency::Weekly;
                rule.weekdays = self.weekday_list()?;
            }
            t if month(t).is_some() && !explicit_interval => {
                self.pos -= 1;
                rule.freq = Frequency::Yearly;
                loop {
                    let m = self.next().and_then(month).ok_or("expected a month")?;
                    rule.months.push(m);
                    let save = self.pos;
                    if !self.separator() || self.peek().and_then(month).is_none() {
                        self.pos = save;
                        break;
                    }
                }
                rule.months.sort_unstable();
                rule.months.dedup();
                if self.peek() == Some("on") {
                    self.month_on(&mut rule)?;
                    for &m in &rule.months {
                        for d in &rule.month_days {
                            if let MonthDay::Day(d) = d
                                && u32::from(*d) > max_days(m)
                            {
                                return Err(format!(
                                    "{} has no day {d}",
                                    MONTHS[(m - 1) as usize].0
                                ));
                            }
                        }
                    }
                }
            }
            other => return Err(format!("unknown unit `{other}`")),
        }
        if let Some(t) = self.peek() {
            return Err(format!("unexpected `{t}`"));
        }
        Ok(rule)
    }
}

fn tokenize(phrase: &str) -> Vec<String> {
    phrase
        .to_lowercase()
        .replace(',', " , ")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Parses a recurrence phrase (the text after `🔁`).
pub fn parse_recurrence(phrase: &str) -> Result<RecurrenceRule, RecurrenceNotUnderstood> {
    let toks = tokenize(phrase);
    let mut p = Parser {
        toks: toks.iter().map(String::as_str).collect(),
        pos: 0,
    };
    p.rule().map_err(|reason| RecurrenceNotUnderstood {
        phrase: phrase.to_owned(),
        reason,
    })
}

fn rrule_day(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "MO",
        Weekday::Tue => "TU",
        Weekday::Wed => "WE",
        Weekday::Thu => "TH",
        Weekday::Fri => "FR",
        Weekday::Sat => "SA",
        Weekday::Sun => "SU",
    }
}

fn join<T: fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    items
        .into_iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

impl RecurrenceRule {
    /// The RRULE (without `RRULE:` prefix). Parts implied by the start date (the weekday of a
    /// plain weekly rule, the day of a plain monthly rule, …) are omitted, as RFC 5545 takes
    /// them from `DTSTART`; use [`RecurrenceRule::to_rrule_anchored`] to spell them out.
    pub fn to_rrule(&self) -> String {
        let mut parts = vec![format!(
            "FREQ={}",
            match self.freq {
                Frequency::Daily => "DAILY",
                Frequency::Weekly => "WEEKLY",
                Frequency::Monthly => "MONTHLY",
                Frequency::Yearly => "YEARLY",
            }
        )];
        if self.interval > 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }
        if !self.months.is_empty() {
            parts.push(format!("BYMONTH={}", join(&self.months)));
        }
        let mut setpos = false;
        match self.month_days.as_slice() {
            [] => {}
            [MonthDay::Day(d)] if self.needs_clamp(*d) => {
                parts.push(format!("BYMONTHDAY={}", join(28..=*d)));
                setpos = true;
            }
            days => parts.push(format!(
                "BYMONTHDAY={}",
                join(days.iter().map(|d| match d {
                    MonthDay::Day(n) => i32::from(*n),
                    MonthDay::Last => -1,
                }))
            )),
        }
        if let Some(n) = self.nth_weekday {
            parts.push(format!("BYDAY={}{}", n.nth, rrule_day(n.weekday)));
        } else if !self.weekdays.is_empty() {
            parts.push(format!(
                "BYDAY={}",
                self.weekdays
                    .iter()
                    .map(|w| rrule_day(*w))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if setpos {
            parts.push("BYSETPOS=-1".into());
        }
        parts.join(";")
    }

    /// Whether day `d` does not exist in some month the rule covers.
    fn needs_clamp(&self, d: u8) -> bool {
        if d < 29 {
            return false;
        }
        match self.freq {
            Frequency::Yearly if !self.months.is_empty() => self
                .months
                .iter()
                .any(|&m| u32::from(d) > days_in_month(2001, m)),
            _ => true,
        }
    }

    /// The rule with parts implied by `anchor` (the reference date) made explicit.
    #[must_use]
    pub fn anchored(&self, anchor: NaiveDate) -> Self {
        let mut r = self.clone();
        let day = u8::try_from(anchor.day()).unwrap_or(1);
        match r.freq {
            Frequency::Daily => {}
            Frequency::Weekly => {
                if r.weekdays.is_empty() {
                    r.weekdays = vec![anchor.weekday()];
                }
            }
            Frequency::Monthly => {
                if r.month_days.is_empty() && r.nth_weekday.is_none() {
                    r.month_days = vec![MonthDay::Day(day)];
                }
            }
            Frequency::Yearly => {
                if r.months.is_empty() {
                    r.months = vec![anchor.month()];
                }
                if r.month_days.is_empty() && r.nth_weekday.is_none() {
                    r.month_days = vec![MonthDay::Day(day)];
                }
            }
        }
        r
    }

    /// The RRULE with every part spelled out for a series starting at `anchor`.
    pub fn to_rrule_anchored(&self, anchor: NaiveDate) -> String {
        self.anchored(anchor).to_rrule()
    }

    /// The first occurrence strictly after `reference` in the series that starts at
    /// `reference` (Tasks plugin semantics: the reference is the due/scheduled/start date, or
    /// the completion date for `when done`). `None` only if no occurrence exists within
    /// 400 periods (e.g. "every February on the 5th Monday" never matches often enough).
    pub fn next_after(&self, reference: NaiveDate) -> Option<NaiveDate> {
        let r = self.anchored(reference);
        match r.freq {
            Frequency::Daily => reference.checked_add_signed(Duration::days(i64::from(r.interval))),
            Frequency::Weekly => {
                let start_monday = reference
                    - Duration::days(i64::from(reference.weekday().num_days_from_monday()));
                (1..=7 * i64::from(r.interval) + 7).find_map(|i| {
                    let day = reference + Duration::days(i);
                    let weeks = (day - start_monday).num_days() / 7;
                    (weeks % i64::from(r.interval) == 0 && r.weekdays.contains(&day.weekday()))
                        .then_some(day)
                })
            }
            Frequency::Monthly => {
                let base = reference.year() * 12 + i32::try_from(reference.month0()).ok()?;
                (0..400).find_map(|k| {
                    let idx = base + k * i32::try_from(r.interval).ok()?;
                    let (y, m) = (
                        idx.div_euclid(12),
                        u32::try_from(idx.rem_euclid(12)).ok()? + 1,
                    );
                    r.dates_in_month(y, m).into_iter().find(|d| *d > reference)
                })
            }
            Frequency::Yearly => (0..400).find_map(|k| {
                let y = reference.year() + k * i32::try_from(r.interval).ok()?;
                r.months
                    .iter()
                    .flat_map(|&m| r.dates_in_month(y, m))
                    .find(|d| *d > reference)
            }),
        }
    }

    fn dates_in_month(&self, y: i32, m: u32) -> Vec<NaiveDate> {
        let len = days_in_month(y, m);
        let mut out: Vec<NaiveDate> = if let Some(n) = self.nth_weekday {
            nth_weekday(y, m, n).into_iter().collect()
        } else {
            self.month_days
                .iter()
                .filter_map(|d| {
                    let day = match d {
                        MonthDay::Day(d) => u32::from(*d).min(len),
                        MonthDay::Last => len,
                    };
                    NaiveDate::from_ymd_opt(y, m, day)
                })
                .collect()
        };
        out.sort();
        out.dedup();
        out
    }
}

fn nth_weekday(y: i32, m: u32, n: NthWeekday) -> Option<NaiveDate> {
    let len = days_in_month(y, m);
    let days: Vec<NaiveDate> = (1..=len)
        .filter_map(|d| NaiveDate::from_ymd_opt(y, m, d))
        .filter(|d| d.weekday() == n.weekday)
        .collect();
    let idx = if n.nth > 0 {
        usize::try_from(n.nth - 1).ok()?
    } else {
        days.len().checked_sub(usize::try_from(-n.nth).ok()?)?
    };
    days.get(idx).copied()
}

/// `1st`, `2nd`, `3rd`, `11th`, `21st`.
fn ordinal_text(n: u32) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    }
}

/// `a`, `a and b`, `a, b and c`.
fn list_phrase(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

impl RecurrenceRule {
    /// The canonical Tasks-plugin phrase of the rule (`every month on the 1st`,
    /// `every 2 weeks on Monday and Thursday`, `every March on the 2nd Wednesday when done`).
    /// [`parse_recurrence`] reads it back to the same rule for every rule the grammar can
    /// express (yearly rules with an interval keep one month and day, as the grammar has no
    /// longer form for them).
    pub fn to_phrase(&self) -> String {
        let n = self.interval.max(1);
        let every = |one: &str, many: &str| {
            if n == 1 {
                format!("every {one}")
            } else {
                format!("every {n} {many}")
            }
        };
        let weekdays: Vec<String> = self
            .weekdays
            .iter()
            .map(|w| weekday_name(*w).to_owned())
            .collect();
        let on_part = || -> String {
            if let Some(nw) = self.nth_weekday {
                let pos = match nw.nth {
                    -1 => "last".to_owned(),
                    k if k < 0 => format!("{} last", ordinal_text(u32::from(k.unsigned_abs()))),
                    k => ordinal_text(u32::from(k.unsigned_abs())),
                };
                format!(" on the {pos} {}", weekday_name(nw.weekday))
            } else if self.month_days.is_empty() {
                String::new()
            } else {
                let days: Vec<String> = self
                    .month_days
                    .iter()
                    .map(|d| match d {
                        MonthDay::Day(d) => ordinal_text(u32::from(*d)),
                        MonthDay::Last => "last".to_owned(),
                    })
                    .collect();
                format!(" on the {}", list_phrase(&days))
            }
        };
        let mut phrase = match self.freq {
            Frequency::Daily => every("day", "days"),
            Frequency::Weekly => {
                let workdays = [
                    Weekday::Mon,
                    Weekday::Tue,
                    Weekday::Wed,
                    Weekday::Thu,
                    Weekday::Fri,
                ];
                if n == 1 && self.weekdays == workdays {
                    "every weekday".to_owned()
                } else if n == 1 && !weekdays.is_empty() {
                    format!("every {}", list_phrase(&weekdays))
                } else if weekdays.is_empty() {
                    every("week", "weeks")
                } else {
                    format!("{} on {}", every("week", "weeks"), list_phrase(&weekdays))
                }
            }
            Frequency::Monthly => format!("{}{}", every("month", "months"), on_part()),
            Frequency::Yearly => match (self.months.as_slice(), self.month_days.as_slice()) {
                ([m], [MonthDay::Day(d)]) if self.nth_weekday.is_none() => format!(
                    "{} on {} {}",
                    every("year", "years"),
                    month_name(*m),
                    ordinal_text(u32::from(*d))
                ),
                ([], _) => every("year", "years"),
                (months, _) if n == 1 => {
                    let names: Vec<String> =
                        months.iter().map(|m| month_name(*m).to_owned()).collect();
                    format!("every {}{}", list_phrase(&names), on_part())
                }
                _ => every("year", "years"),
            },
        };
        if self.when_done {
            phrase.push_str(" when done");
        }
        phrase
    }
}

#[cfg(test)]
mod phrase_tests {
    use super::*;

    #[test]
    fn phrases_round_trip() {
        for phrase in [
            "every day",
            "every 3 days",
            "every weekday",
            "every week",
            "every 2 weeks",
            "every Monday",
            "every Monday and Thursday",
            "every Monday, Wednesday and Friday",
            "every 2 weeks on Tuesday",
            "every month",
            "every month on the 1st",
            "every month on the 1st, 15th and last",
            "every month on the last",
            "every 3 months on the 2nd Wednesday",
            "every month on the last Friday",
            "every month on the 2nd last Friday",
            "every year",
            "every year on March 5th",
            "every 2 years on February 29th",
            "every January and July on the 15th",
            "every March on the 2nd Wednesday",
            "every December",
            "every month on the 1st when done",
        ] {
            let rule = parse_recurrence(phrase).expect(phrase);
            assert_eq!(rule.to_phrase(), phrase, "canonical form of {phrase}");
            assert_eq!(parse_recurrence(&rule.to_phrase()).expect("reparses"), rule);
        }
    }

    #[test]
    fn non_canonical_phrases_normalise() {
        let cases = [
            ("Every other day", "every 2 days"),
            ("every mon, thu", "every Monday and Thursday"),
            ("every month on the first", "every month on the 1st"),
            ("every year on the 5th of march", "every year on March 5th"),
            ("every month on the last day", "every month on the last"),
        ];
        for (input, canonical) in cases {
            let rule = parse_recurrence(input).expect(input);
            assert_eq!(rule.to_phrase(), canonical);
            assert_eq!(parse_recurrence(canonical).expect("reparses"), rule);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals() {
        assert_eq!(ordinal("1st"), Some(1));
        assert_eq!(ordinal("2nd"), Some(2));
        assert_eq!(ordinal("3rd"), Some(3));
        assert_eq!(ordinal("11th"), Some(11));
        assert_eq!(ordinal("21st"), Some(21));
        assert_eq!(ordinal("22nd"), Some(22));
        assert_eq!(ordinal("5"), Some(5));
        assert_eq!(ordinal("second"), Some(2));
        assert_eq!(ordinal("2st"), None);
        assert_eq!(ordinal("x"), None);
    }

    #[test]
    fn errors_keep_phrase() {
        let e = parse_recurrence("every fortnight");
        assert_eq!(
            e,
            Err(RecurrenceNotUnderstood {
                phrase: "every fortnight".into(),
                reason: "unknown unit `fortnight`".into()
            })
        );
        for bad in [
            "",
            "daily",
            "every",
            "every 0 days",
            "every day at 9",
            "every month on the 32nd",
            "every year on February 30",
            "every April on the 31st",
            "every 2 weekdays",
            "every month on the 6th monday",
            "every week on funday",
        ] {
            assert!(parse_recurrence(bad).is_err(), "{bad}");
        }
    }
}
