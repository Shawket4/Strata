//! Dates of AI-extracted events (PLAN §6.7 "Timeline dates", §6.12, §6.11): an explicit date
//! in the text wins; a relative phrase ("tomorrow", "next Sunday", "بكرة") is resolved against
//! the note's `created` date; otherwise the date is the note's `created` date. The resolved
//! date is stored, never the phrase.
//!
//! The model is asked to follow the same rules; this module makes them deterministic for the
//! phrases it knows, overriding the model's date whenever the cited text contains an explicit
//! date or a known relative phrase. Phrases it does not know keep the model's date.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use strata_ai::outputs::DateSource;
use text_normalize::normalize_for_search;

/// A known relative phrase: its words (normalised with `text-normalize`) and the day offset.
const RELATIVE: &[(&str, i64)] = &[
    ("day after tomorrow", 2),
    ("بعد بكره", 2),
    ("بعد بكرة", 2),
    ("tomorrow", 1),
    ("بكره", 1),
    ("بكرة", 1),
    ("غدا", 1),
    ("yesterday", -1),
    ("امبارح", -1),
    ("إمبارح", -1),
    ("امس", -1),
    ("أمس", -1),
    ("today", 0),
    ("tonight", 0),
    ("النهارده", 0),
    ("النهاردة", 0),
    ("اليوم", 0),
];

/// Weekday names (English, Arabic with and without the article).
const WEEKDAYS: &[(&str, Weekday)] = &[
    ("monday", Weekday::Mon),
    ("tuesday", Weekday::Tue),
    ("wednesday", Weekday::Wed),
    ("thursday", Weekday::Thu),
    ("friday", Weekday::Fri),
    ("saturday", Weekday::Sat),
    ("sunday", Weekday::Sun),
    ("الاتنين", Weekday::Mon),
    ("الاثنين", Weekday::Mon),
    ("التلات", Weekday::Tue),
    ("الثلاثاء", Weekday::Tue),
    ("الاربع", Weekday::Wed),
    ("الاربعاء", Weekday::Wed),
    ("الخميس", Weekday::Thu),
    ("الجمعة", Weekday::Fri),
    ("السبت", Weekday::Sat),
    ("الحد", Weekday::Sun),
    ("الاحد", Weekday::Sun),
];

fn words(text: &str) -> Vec<String> {
    normalize_for_search(text)
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

fn find_phrase(haystack: &[String], phrase: &str) -> Option<usize> {
    let needle = words(phrase);
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == needle[..])
}

/// The first `YYYY-MM-DD` date written in `text`.
pub fn explicit_date(text: &str) -> Option<NaiveDate> {
    let bytes = text.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    for i in 0..=bytes.len() - 10 {
        let w = &bytes[i..i + 10];
        let shape = w.iter().enumerate().all(|(j, b)| match j {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
        let bounded = (i == 0 || !bytes[i - 1].is_ascii_digit())
            && bytes.get(i + 10).is_none_or(|b| !b.is_ascii_digit());
        if shape
            && bounded
            && let Some(d) = std::str::from_utf8(w)
                .ok()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        {
            return Some(d);
        }
    }
    None
}

/// The first known relative date phrase in `text`, resolved against `created`.
pub fn relative_date(text: &str, created: NaiveDate) -> Option<NaiveDate> {
    let hay = words(text);
    let mut best: Option<(usize, NaiveDate)> = None;
    let mut consider = |pos: usize, date: NaiveDate| {
        if best.is_none_or(|(p, _)| pos < p) {
            best = Some((pos, date));
        }
    };
    for (phrase, days) in RELATIVE {
        if let Some(pos) = find_phrase(&hay, phrase) {
            consider(pos, created + Duration::days(*days));
        }
    }
    for (name, day) in WEEKDAYS {
        let Some(pos) = find_phrase(&hay, name) else {
            continue;
        };
        let before = pos
            .checked_sub(1)
            .and_then(|p| hay.get(p))
            .map(String::as_str);
        let after = hay.get(pos + 1).map(String::as_str);
        let next = matches!(before, Some("next" | "this" | "on" | "coming"))
            || matches!(after, Some("الجاي" | "الجايه" | "القادم"))
            || matches!(before, Some("يوم"));
        if next {
            let ahead = (7 + i64::from(day.num_days_from_monday())
                - i64::from(created.weekday().num_days_from_monday()))
                % 7;
            let ahead = if ahead == 0 { 7 } else { ahead };
            consider(pos, created + Duration::days(ahead));
        }
    }
    best.map(|(_, d)| d)
}

/// The date of an event stated in `text` of a note created on `created`: explicit, else a
/// known relative phrase, else `None` (the caller keeps the model's date or uses `created`).
pub fn resolve(text: &str, created: NaiveDate) -> Option<(NaiveDate, DateSource)> {
    if let Some(d) = explicit_date(text) {
        return Some((d, DateSource::Explicit));
    }
    relative_date(text, created).map(|d| (d, DateSource::Relative))
}

/// The date to store for an event the model dated `model_date` (`YYYY-MM-DD`,
/// `source`), stated in `text` of a note created on `created` (§6.7 rules, deterministic for
/// the phrases this module knows).
pub fn event_date(
    text: &str,
    created: NaiveDate,
    model_date: Option<&str>,
    source: Option<DateSource>,
) -> NaiveDate {
    if let Some((d, _)) = resolve(text, created) {
        return d;
    }
    if source == Some(DateSource::Created) {
        return created;
    }
    model_date
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
    }

    #[test]
    fn relative_phrases_resolve_against_created() {
        let created = d("2026-09-27"); // a Sunday
        assert_eq!(
            relative_date("هبعت العقد لشادي بكرة", created),
            Some(d("2026-09-28"))
        );
        assert_eq!(
            relative_date("هبعت العقد بكره", created),
            Some(d("2026-09-28"))
        );
        assert_eq!(
            relative_date("send it tomorrow", created),
            Some(d("2026-09-28"))
        );
        assert_eq!(
            relative_date("met him yesterday", created),
            Some(d("2026-09-26"))
        );
        assert_eq!(
            relative_date("امبارح قابلته", created),
            Some(d("2026-09-26"))
        );
        assert_eq!(
            relative_date("the day after tomorrow", created),
            Some(d("2026-09-29"))
        );
        assert_eq!(relative_date("بعد بكرة", created), Some(d("2026-09-29")));
        assert_eq!(
            relative_date("call next Sunday", created),
            Some(d("2026-10-04"))
        );
        assert_eq!(
            relative_date("call on Thursday", created),
            Some(d("2026-10-01"))
        );
        assert_eq!(
            relative_date("الخميس الجاي", created),
            Some(d("2026-10-01"))
        );
        assert_eq!(relative_date("Sunday meetings are long", created), None);
        assert_eq!(relative_date("no date here", created), None);
    }

    #[test]
    fn explicit_dates_win_and_the_model_date_is_the_fallback() {
        let created = d("2026-09-27");
        assert_eq!(
            explicit_date("due 2026-10-01, not 12026-10-011"),
            Some(d("2026-10-01"))
        );
        assert_eq!(
            resolve("on 2026-10-05 tomorrow", created),
            Some((d("2026-10-05"), DateSource::Explicit))
        );
        assert_eq!(
            event_date(
                "gave it to Shady",
                created,
                Some("2026-09-01"),
                Some(DateSource::Created)
            ),
            created
        );
        assert_eq!(
            event_date(
                "gave it to Shady",
                created,
                Some("2026-09-01"),
                Some(DateSource::Relative)
            ),
            d("2026-09-01")
        );
        assert_eq!(
            event_date(
                "بكرة",
                created,
                Some("2026-09-27"),
                Some(DateSource::Created)
            ),
            d("2026-09-28")
        );
        assert_eq!(event_date("x", created, Some("bad"), None), created);
    }
}
