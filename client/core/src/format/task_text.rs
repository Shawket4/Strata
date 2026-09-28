//! Natural task text ("Call Ahmed tomorrow at 9 every Monday", "كلم بابا بكرة الساعة 5") read
//! into a task draft for the new-task sheet's "Understood as" chips. Understood phrases are
//! removed from the description; everything else is kept verbatim. English and Egyptian /
//! Modern Standard Arabic keywords; recurrence phrases use the Tasks grammar of `vault-format`
//! (Arabic "كل …" phrases are mapped to it).

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use vault_format::tasks::{RecurrenceRule, parse_recurrence};

/// What was understood, before links are resolved against the vault.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedTaskText {
    /// The description without understood phrases (`@Name` mentions kept as written).
    pub description: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// 🔁 phrase (canonical).
    pub recurrence: Option<String>,
    /// Reminder times (on the due date, or today/tomorrow without one).
    pub reminders: Vec<NaiveDateTime>,
    /// `highest` … `lowest`.
    pub priority: Option<String>,
    /// `@Name` mentions (without `@`), in order.
    pub mentions: Vec<String>,
    /// Understood pieces in text order: (kind, original text).
    pub pieces: Vec<(Piece, String)>,
}

/// Kind of an understood piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    /// A date.
    Due,
    /// A recurrence.
    Recurrence,
    /// A time (reminder).
    Time,
    /// A priority.
    Priority,
    /// An `@mention`.
    Mention,
}

#[derive(Debug, Clone)]
struct Tok<'a> {
    text: &'a str,
    lower: String,
    start: usize,
    end: usize,
}

fn tokens(text: &str) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out.into_iter()
        .map(|(s, e)| {
            let t = &text[s..e];
            let trimmed = t.trim_end_matches([',', '.', '،', '?', '؟']);
            Tok {
                text: t,
                lower: trimmed.to_lowercase(),
                start: s,
                end: e,
            }
        })
        .collect()
}

const WEEKDAYS: [(&str, &str, Weekday); 7] = [
    ("monday", "الاثنين", Weekday::Mon),
    ("tuesday", "الثلاثاء", Weekday::Tue),
    ("wednesday", "الأربعاء", Weekday::Wed),
    ("thursday", "الخميس", Weekday::Thu),
    ("friday", "الجمعة", Weekday::Fri),
    ("saturday", "السبت", Weekday::Sat),
    ("sunday", "الأحد", Weekday::Sun),
];

/// A weekday name: English full names (and 3-letter abbreviations when `short` is allowed,
/// i.e. after "on"/"next"), Arabic names with or without the article.
fn weekday_of(tok: &str, short: bool) -> Option<Weekday> {
    WEEKDAYS
        .iter()
        .find(|(en, ar, _)| {
            tok == *en
                || (short && tok.len() == 3 && en.starts_with(tok))
                || tok == *ar
                || ar.strip_prefix("ال") == Some(tok)
        })
        .map(|(_, _, w)| *w)
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn month_of(tok: &str) -> Option<u32> {
    MONTHS
        .iter()
        .position(|m| tok == *m || (tok.len() >= 3 && m.starts_with(tok)))
        .and_then(|i| u32::try_from(i + 1).ok())
}

fn day_number(tok: &str) -> Option<u32> {
    let digits = tok.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    digits.parse::<u32>().ok().filter(|d| (1..=31).contains(d))
}

/// The next date strictly after `today` falling on `w` (same weekday → next week).
fn next_weekday(today: NaiveDate, w: Weekday) -> NaiveDate {
    let ahead = (7 + i64::from(w.num_days_from_monday())
        - i64::from(today.weekday().num_days_from_monday()))
        % 7;
    today + Duration::days(if ahead == 0 { 7 } else { ahead })
}

/// `9`, `9am`, `9:30`, `21:15`, `9:30pm`, `9 pm` (am/pm as the next token).
fn time_of(tok: &str, next: Option<&str>) -> Option<(NaiveTime, bool)> {
    let (body, mut pm, mut am) = (tok, false, false);
    let body = if let Some(b) = body.strip_suffix("pm").or_else(|| body.strip_suffix("م")) {
        pm = true;
        b
    } else if let Some(b) = body.strip_suffix("am").or_else(|| body.strip_suffix("ص")) {
        am = true;
        b
    } else {
        body
    };
    let mut consumed_next = false;
    if !pm && !am {
        match next {
            Some("pm" | "م" | "مساء" | "مساءً" | "بالليل") => {
                pm = true;
                consumed_next = true;
            }
            Some("am" | "ص" | "صباحا" | "صباحًا" | "الصبح") => {
                am = true;
                consumed_next = true;
            }
            _ => {}
        }
    }
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    let h = match (h, pm, am) {
        (12, false, true) => 0,
        (h @ 1..=11, true, _) => h + 12,
        (h, _, _) => h,
    };
    NaiveTime::from_hms_opt(h, m, 0).map(|t| (t, consumed_next))
}

/// Arabic recurrence phrases mapped to the Tasks grammar.
fn arabic_recurrence(toks: &[Tok<'_>], i: usize) -> Option<(String, usize)> {
    if toks.get(i)?.lower != "كل" {
        return None;
    }
    let unit = toks.get(i + 1)?.lower.as_str();
    let phrase = match unit {
        "يوم" => "every day".to_owned(),
        "أسبوع" | "اسبوع" => "every week".to_owned(),
        "شهر" => "every month".to_owned(),
        "سنة" | "سنه" => "every year".to_owned(),
        other => {
            let w = weekday_of(other, false)?;
            let name = WEEKDAYS.iter().find(|(_, _, x)| *x == w)?.0;
            format!("every {name}")
        }
    };
    Some((phrase, 2))
}

/// The longest English recurrence phrase starting at token `i` (`every …`).
fn english_recurrence(toks: &[Tok<'_>], i: usize) -> Option<(RecurrenceRule, usize)> {
    if toks.get(i)?.lower != "every" {
        return None;
    }
    (2..=toks.len() - i).rev().find_map(|n| {
        let phrase: Vec<&str> = toks[i..i + n].iter().map(|t| t.lower.as_str()).collect();
        parse_recurrence(&phrase.join(" ")).ok().map(|r| (r, n))
    })
}

/// Reads a task text; `today` in the user's time zone.
#[allow(clippy::too_many_lines)] // one branch per phrase family
pub fn parse(text: &str, today: NaiveDate) -> ParsedTaskText {
    let toks = tokens(text);
    let mut used = vec![false; toks.len()];
    let mut out = ParsedTaskText::default();
    let mut times: Vec<NaiveTime> = Vec::new();
    let mut rule: Option<RecurrenceRule> = None;
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].lower.as_str();
        let next = toks.get(i + 1).map(|t| t.lower.as_str());
        let take = |n: usize, piece: Piece, used: &mut Vec<bool>, out: &mut ParsedTaskText| {
            for u in used.iter_mut().skip(i).take(n) {
                *u = true;
            }
            let span = &text[toks[i].start..toks[i + n - 1].end];
            out.pieces.push((piece, span.to_owned()));
        };
        // Recurrence.
        if let Some((r, n)) = english_recurrence(&toks, i) {
            out.recurrence = Some(r.to_phrase());
            rule = Some(r);
            take(n, Piece::Recurrence, &mut used, &mut out);
            i += n;
            continue;
        }
        if let Some((phrase, n)) = arabic_recurrence(&toks, i)
            && let Ok(r) = parse_recurrence(&phrase)
        {
            out.recurrence = Some(r.to_phrase());
            rule = Some(r);
            take(n, Piece::Recurrence, &mut used, &mut out);
            i += n;
            continue;
        }
        // Dates.
        let date_n: Option<(NaiveDate, usize)> = match (t, next) {
            ("today" | "النهارده" | "النهاردة" | "اليوم", _) => {
                Some((today, 1))
            }
            ("day", Some("after")) if toks.get(i + 2).is_some_and(|t| t.lower == "tomorrow") => {
                Some((today + Duration::days(2), 3))
            }
            ("بعد", Some("بكرة" | "بكره" | "غد")) => {
                Some((today + Duration::days(2), 2))
            }
            ("tomorrow" | "بكرة" | "بكره" | "غدا" | "غدًا", _) => {
                Some((today + Duration::days(1), 1))
            }
            ("in" | "بعد", Some(n)) => {
                let unit = toks.get(i + 2).map(|t| t.lower.as_str());
                match (n.parse::<i64>().ok(), unit) {
                    (Some(k), Some("day" | "days" | "يوم" | "أيام" | "ايام")) => {
                        Some((today + Duration::days(k), 3))
                    }
                    (Some(k), Some("week" | "weeks" | "أسبوع" | "اسبوع" | "أسابيع")) => {
                        Some((today + Duration::days(7 * k), 3))
                    }
                    _ => None,
                }
            }
            ("on" | "next" | "يوم", Some(w)) if weekday_of(w, true).is_some() => {
                weekday_of(w, true).map(|w| (next_weekday(today, w), 2))
            }
            (w, _) if weekday_of(w, false).is_some() => {
                weekday_of(w, false).map(|w| (next_weekday(today, w), 1))
            }
            _ => {
                // 2026-10-01, "1 Oct", "Oct 1"
                if let Ok(d) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
                    Some((d, 1))
                } else if let (Some(d), Some(m)) = (day_number(t), next.and_then(month_of)) {
                    date_this_or_next_year(today, m, d).map(|x| (x, 2))
                } else if let (Some(m), Some(d)) = (month_of(t), next.and_then(day_number)) {
                    if t.len() >= 3 {
                        date_this_or_next_year(today, m, d).map(|x| (x, 2))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };
        if let Some((d, n)) = date_n {
            out.due = Some(d);
            take(n, Piece::Due, &mut used, &mut out);
            i += n;
            continue;
        }
        // Times: "at 9", "at 9:30pm", "الساعة 5".
        if matches!(t, "at" | "الساعة" | "الساعه" | "@")
            && let Some(nx) = next
            && let Some((time, extra)) = time_of(nx, toks.get(i + 2).map(|t| t.lower.as_str()))
        {
            times.push(time);
            let n = 2 + usize::from(extra);
            take(n, Piece::Time, &mut used, &mut out);
            i += n;
            continue;
        }
        // A bare time with its meridiem attached: "3pm", "9:30am".
        if t.starts_with(|c: char| c.is_ascii_digit())
            && (t.ends_with("am") || t.ends_with("pm"))
            && let Some((time, _)) = time_of(t, None)
        {
            times.push(time);
            take(1, Piece::Time, &mut used, &mut out);
            i += 1;
            continue;
        }
        // Priority.
        let prio = match (t, next) {
            ("!!!" | "urgent" | "عاجل", _) => Some(("highest", 1)),
            ("!!" | "!high" | "important" | "مهم", _) => Some(("high", 1)),
            ("!low", _) => Some(("low", 1)),
            ("high", Some("priority")) | ("priority", Some("high")) => Some(("high", 2)),
            ("low", Some("priority")) | ("priority", Some("low")) => Some(("low", 2)),
            _ => None,
        };
        if let Some((p, n)) = prio {
            out.priority = Some(p.to_owned());
            take(n, Piece::Priority, &mut used, &mut out);
            i += n;
            continue;
        }
        // Mentions.
        if let Some(name) = toks[i].text.strip_prefix('@')
            && !name.is_empty()
        {
            let name = name.trim_end_matches([',', '.', '،']);
            out.mentions.push(name.replace('_', " "));
            out.pieces.push((Piece::Mention, toks[i].text.to_owned()));
        }
        i += 1;
    }
    // A recurrence without a date starts at its first occurrence from today.
    if out.due.is_none()
        && let Some(r) = &rule
    {
        let explicit = !r.weekdays.is_empty()
            || !r.month_days.is_empty()
            || r.nth_weekday.is_some()
            || !r.months.is_empty();
        out.due = Some(if explicit {
            r.next_after(today - Duration::days(1)).unwrap_or(today)
        } else {
            today
        });
    }
    let day = out.due.unwrap_or(today);
    out.reminders = times.into_iter().map(|t| day.and_time(t)).collect();
    // Drop connectors left dangling by removed phrases ("… on", "… at").
    let kept: Vec<&str> = toks
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(t, _)| t.text)
        .collect();
    let mut description = kept.join(" ");
    for dangling in [" on", " at", " by", " في", " يوم"] {
        if let Some(stripped) = description.strip_suffix(dangling) {
            description = stripped.to_owned();
        }
    }
    description.trim().clone_into(&mut out.description);
    out
}

fn date_this_or_next_year(today: NaiveDate, month: u32, day: u32) -> Option<NaiveDate> {
    let this = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if this >= today {
        Some(this)
    } else {
        NaiveDate::from_ymd_opt(today.year() + 1, month, day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
    }

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").expect("datetime")
    }

    // Sunday 27 Sep 2026.
    const TODAY: &str = "2026-09-27";

    #[test]
    #[allow(clippy::many_single_char_names)] // one binding per parsed example
    fn english_date_time_and_recurrence() {
        let p = parse("Call @Ahmed tomorrow at 9:30pm about invoices", d(TODAY));
        assert_eq!(p.description, "Call @Ahmed about invoices");
        assert_eq!(p.due, Some(d("2026-09-28")));
        assert_eq!(p.reminders, vec![dt("2026-09-28 21:30")]);
        assert_eq!(p.mentions, vec!["Ahmed".to_owned()]);
        assert_eq!(
            p.pieces,
            vec![
                (Piece::Mention, "@Ahmed".into()),
                (Piece::Due, "tomorrow".into()),
                (Piece::Time, "at 9:30pm".into()),
            ]
        );
        let r = parse(
            "Make Watanya's ETA invoice every month on the 1st",
            d(TODAY),
        );
        assert_eq!(r.description, "Make Watanya's ETA invoice");
        assert_eq!(r.recurrence.as_deref(), Some("every month on the 1st"));
        assert_eq!(r.due, Some(d("2026-10-01")));
        let w = parse("Standup every Monday and Thursday at 10", d(TODAY));
        assert_eq!(w.recurrence.as_deref(), Some("every Monday and Thursday"));
        assert_eq!(w.due, Some(d("2026-09-28")));
        assert_eq!(w.reminders, vec![dt("2026-09-28 10:00")]);
        let x = parse("Renew passport on Friday !!", d(TODAY));
        assert_eq!(x.description, "Renew passport");
        assert_eq!(x.due, Some(d("2026-10-02")));
        assert_eq!(x.priority.as_deref(), Some("high"));
        let y = parse("Pay rent 1 Oct", d(TODAY));
        assert_eq!(
            (y.description.as_str(), y.due),
            ("Pay rent", Some(d("2026-10-01")))
        );
        let bare = parse("Call Shady tomorrow 3pm !high #car", d(TODAY));
        assert_eq!(bare.description, "Call Shady #car");
        assert_eq!(bare.reminders, vec![dt("2026-09-28 15:00")]);
        assert_eq!(bare.priority.as_deref(), Some("high"));
        let z = parse("Submit report in 3 days", d(TODAY));
        assert_eq!(z.due, Some(d("2026-09-30")));
        let sunday = parse("Plan week sunday", d(TODAY));
        assert_eq!(
            sunday.due,
            Some(d("2026-10-04")),
            "same weekday → next week"
        );
    }

    #[test]
    fn arabic_phrases() {
        let p = parse("كلم بابا بكرة الساعة 5 مساء", d(TODAY));
        assert_eq!(p.description, "كلم بابا");
        assert_eq!(p.due, Some(d("2026-09-28")));
        assert_eq!(p.reminders, vec![dt("2026-09-28 17:00")]);
        let r = parse("راجع الحسابات كل شهر", d(TODAY));
        assert_eq!(r.description, "راجع الحسابات");
        assert_eq!(r.recurrence.as_deref(), Some("every month"));
        assert_eq!(r.due, Some(d(TODAY)));
        let w = parse("اجتماع كل اثنين", d(TODAY));
        assert_eq!(w.recurrence.as_deref(), Some("every Monday"));
        assert_eq!(w.due, Some(d("2026-09-28")));
        let t = parse("ادفع الإيجار النهارده", d(TODAY));
        assert_eq!(t.due, Some(d(TODAY)));
    }

    #[test]
    fn plain_text_is_kept() {
        let p = parse("Buy milk", d(TODAY));
        assert_eq!(
            p,
            ParsedTaskText {
                description: "Buy milk".into(),
                ..ParsedTaskText::default()
            }
        );
    }
}
