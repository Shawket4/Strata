//! Display labels (L15: Dart never formats dates, counts or relative times). Every label is
//! computed from an injected "now" in the account's IANA time zone (DST resolved by
//! `chrono-tz`) and written in the account's UI language (English or Arabic, PLAN §11).
//! Digits are Western in both languages (the brand typography's figures); Arabic plurals
//! follow the CLDR categories (one, two, few, many, other).

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc, Weekday};
use chrono_tz::Tz;

/// UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// English.
    #[default]
    En,
    /// Arabic.
    Ar,
}

impl Lang {
    /// The language of a UI language code (`ar…` is Arabic, everything else English).
    pub fn from_code(code: &str) -> Self {
        if code.starts_with("ar") {
            Self::Ar
        } else {
            Self::En
        }
    }
}

const WEEKDAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const WEEKDAYS_AR: [&str; 7] = [
    "الاثنين",
    "الثلاثاء",
    "الأربعاء",
    "الخميس",
    "الجمعة",
    "السبت",
    "الأحد",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTHS_AR: [&str; 12] = [
    "يناير",
    "فبراير",
    "مارس",
    "أبريل",
    "مايو",
    "يونيو",
    "يوليو",
    "أغسطس",
    "سبتمبر",
    "أكتوبر",
    "نوفمبر",
    "ديسمبر",
];

/// Plural forms of one counted word: English `(one, other)`, Arabic `(one, two, few, many,
/// other)`. `{n}` is replaced by the number.
#[derive(Debug, Clone, Copy)]
pub struct Plural {
    /// English singular.
    pub en_one: &'static str,
    /// English plural.
    pub en_other: &'static str,
    /// Arabic 1.
    pub ar_one: &'static str,
    /// Arabic 2.
    pub ar_two: &'static str,
    /// Arabic 3–10.
    pub ar_few: &'static str,
    /// Arabic 11–99.
    pub ar_many: &'static str,
    /// Arabic 0, 100+.
    pub ar_other: &'static str,
}

/// Days.
pub const DAYS: Plural = Plural {
    en_one: "1 day",
    en_other: "{n} days",
    ar_one: "يوم واحد",
    ar_two: "يومان",
    ar_few: "{n} أيام",
    ar_many: "{n} يومًا",
    ar_other: "{n} يوم",
};
/// Minutes.
pub const MINUTES: Plural = Plural {
    en_one: "1 minute",
    en_other: "{n} minutes",
    ar_one: "دقيقة",
    ar_two: "دقيقتين",
    ar_few: "{n} دقائق",
    ar_many: "{n} دقيقة",
    ar_other: "{n} دقيقة",
};
/// Hours.
pub const HOURS: Plural = Plural {
    en_one: "1 hour",
    en_other: "{n} hours",
    ar_one: "ساعة",
    ar_two: "ساعتين",
    ar_few: "{n} ساعات",
    ar_many: "{n} ساعة",
    ar_other: "{n} ساعة",
};
/// Weeks.
pub const WEEKS: Plural = Plural {
    en_one: "1 week",
    en_other: "{n} weeks",
    ar_one: "أسبوع",
    ar_two: "أسبوعين",
    ar_few: "{n} أسابيع",
    ar_many: "{n} أسبوعًا",
    ar_other: "{n} أسبوع",
};
/// Months.
pub const MONTHS: Plural = Plural {
    en_one: "1 month",
    en_other: "{n} months",
    ar_one: "شهر",
    ar_two: "شهرين",
    ar_few: "{n} أشهر",
    ar_many: "{n} شهرًا",
    ar_other: "{n} شهر",
};
/// Years.
pub const YEARS: Plural = Plural {
    en_one: "1 year",
    en_other: "{n} years",
    ar_one: "سنة",
    ar_two: "سنتين",
    ar_few: "{n} سنوات",
    ar_many: "{n} سنة",
    ar_other: "{n} سنة",
};
/// Notes.
pub const NOTES: Plural = Plural {
    en_one: "1 note",
    en_other: "{n} notes",
    ar_one: "ملاحظة واحدة",
    ar_two: "ملاحظتان",
    ar_few: "{n} ملاحظات",
    ar_many: "{n} ملاحظة",
    ar_other: "{n} ملاحظة",
};
/// Relations.
pub const RELATIONS: Plural = Plural {
    en_one: "1 relation",
    en_other: "{n} relations",
    ar_one: "علاقة واحدة",
    ar_two: "علاقتان",
    ar_few: "{n} علاقات",
    ar_many: "{n} علاقة",
    ar_other: "{n} علاقة",
};
/// Lines.
pub const LINES: Plural = Plural {
    en_one: "1 line",
    en_other: "{n} lines",
    ar_one: "سطر واحد",
    ar_two: "سطران",
    ar_few: "{n} أسطر",
    ar_many: "{n} سطرًا",
    ar_other: "{n} سطر",
};
/// Conflicts.
pub const CONFLICTS: Plural = Plural {
    en_one: "1 conflict",
    en_other: "{n} conflicts",
    ar_one: "تعارض واحد",
    ar_two: "تعارضان",
    ar_few: "{n} تعارضات",
    ar_many: "{n} تعارضًا",
    ar_other: "{n} تعارض",
};
/// Changes.
pub const CHANGES: Plural = Plural {
    en_one: "1 change",
    en_other: "{n} changes",
    ar_one: "تغيير واحد",
    ar_two: "تغييران",
    ar_few: "{n} تغييرات",
    ar_many: "{n} تغييرًا",
    ar_other: "{n} تغيير",
};

impl Plural {
    /// The phrase for `n` in `lang`.
    pub fn of(&self, n: i64, lang: Lang) -> String {
        let form = match lang {
            Lang::En => {
                if n == 1 {
                    self.en_one
                } else {
                    self.en_other
                }
            }
            Lang::Ar => match n.rem_euclid(100) {
                _ if n == 1 => self.ar_one,
                _ if n == 2 => self.ar_two,
                3..=10 => self.ar_few,
                11..=99 => self.ar_many,
                _ => self.ar_other,
            },
        };
        form.replace("{n}", &n.to_string())
    }
}

/// Picks the English or Arabic text.
pub fn tr(lang: Lang, en: &str, ar: &str) -> String {
    match lang {
        Lang::En => en.to_owned(),
        Lang::Ar => ar.to_owned(),
    }
}

fn weekday_index(w: Weekday) -> usize {
    w.num_days_from_monday() as usize
}

/// Label maker for one view build: now, time zone, language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Labels {
    /// Now.
    pub now: DateTime<Utc>,
    /// The account's time zone.
    pub tz: Tz,
    /// UI language.
    pub lang: Lang,
}

impl Labels {
    /// Labels for `now` in `tz`, `lang`.
    pub fn new(now: DateTime<Utc>, tz: Tz, lang: Lang) -> Self {
        Self { now, tz, lang }
    }

    fn t(&self, en: &str, ar: &str) -> String {
        tr(self.lang, en, ar)
    }

    /// An instant as wall-clock time in the account's zone.
    pub fn local(&self, t: DateTime<Utc>) -> NaiveDateTime {
        t.with_timezone(&self.tz).naive_local()
    }

    /// Today in the account's zone.
    pub fn today(&self) -> NaiveDate {
        self.local(self.now).date()
    }

    /// "Sat" / "السبت".
    pub fn weekday_short(&self, d: NaiveDate) -> String {
        let i = weekday_index(d.weekday());
        match self.lang {
            Lang::En => WEEKDAYS_EN[i][..3].to_owned(),
            Lang::Ar => WEEKDAYS_AR[i].to_owned(),
        }
    }

    /// "Saturday" / "السبت".
    pub fn weekday_long(&self, d: NaiveDate) -> String {
        let i = weekday_index(d.weekday());
        match self.lang {
            Lang::En => WEEKDAYS_EN[i].to_owned(),
            Lang::Ar => WEEKDAYS_AR[i].to_owned(),
        }
    }

    fn month_short(&self, m: u32) -> String {
        let i = (m.clamp(1, 12) - 1) as usize;
        match self.lang {
            Lang::En => MONTHS_EN[i][..3].to_owned(),
            Lang::Ar => MONTHS_AR[i].to_owned(),
        }
    }

    fn month_long(&self, m: u32) -> String {
        let i = (m.clamp(1, 12) - 1) as usize;
        match self.lang {
            Lang::En => MONTHS_EN[i].to_owned(),
            Lang::Ar => MONTHS_AR[i].to_owned(),
        }
    }

    /// "14:32".
    pub fn hm(&self, t: DateTime<Utc>) -> String {
        let l = self.local(t);
        format!("{:02}:{:02}", l.hour(), l.minute())
    }

    /// "14:47:30".
    pub fn hms(&self, t: DateTime<Utc>) -> String {
        let l = self.local(t);
        format!("{:02}:{:02}:{:02}", l.hour(), l.minute(), l.second())
    }

    /// "21 Sep" / "21 سبتمبر".
    pub fn day_month(&self, d: NaiveDate) -> String {
        format!("{} {}", d.day(), self.month_short(d.month()))
    }

    /// "11 Oct 2026".
    pub fn date_long(&self, d: NaiveDate) -> String {
        format!("{} {} {}", d.day(), self.month_short(d.month()), d.year())
    }

    /// "Tue 29 Sep" (with the year when it is not this year: "Tue 29 Sep 2027").
    pub fn weekday_date(&self, d: NaiveDate) -> String {
        let base = format!("{} {}", self.weekday_short(d), self.day_month(d));
        if d.year() == self.today().year() {
            base
        } else {
            format!("{base} {}", d.year())
        }
    }

    /// "Thu 1 Oct 2026" (always with the year).
    pub fn weekday_date_year(&self, d: NaiveDate) -> String {
        format!("{} {}", self.weekday_short(d), self.date_long(d))
    }

    /// "Sunday 27 September".
    pub fn today_long(&self) -> String {
        let d = self.today();
        format!(
            "{} {} {}",
            self.weekday_long(d),
            d.day(),
            self.month_long(d.month())
        )
    }

    /// A date in a list: "Today", "Sat" (the last five days), "21 Sep", "21 Sep 2025".
    pub fn date_in_list(&self, d: NaiveDate) -> String {
        let today = self.today();
        let back = (today - d).num_days();
        if back == 0 {
            self.t("Today", "اليوم")
        } else if (1..6).contains(&back) {
            self.weekday_short(d)
        } else if d.year() == today.year() {
            self.day_month(d)
        } else {
            self.date_long(d)
        }
    }

    /// An instant in a list: "14:31" today, "Sat" this week, "21 Sep", "21 Sep 2025".
    pub fn list_label(&self, t: DateTime<Utc>) -> String {
        let d = self.local(t).date();
        if d == self.today() {
            self.hm(t)
        } else {
            self.date_in_list(d)
        }
    }

    /// An instant with its time: "09:47" today, "Sat 18:40" this week, "21 Sep 18:40",
    /// "21 Sep 2025 18:40".
    pub fn moment_label(&self, t: DateTime<Utc>) -> String {
        let d = self.local(t).date();
        let today = self.today();
        let back = (today - d).num_days();
        let time = self.hm(t);
        if back == 0 {
            time
        } else if (1..6).contains(&back) {
            format!("{} {time}", self.weekday_short(d))
        } else if d.year() == today.year() {
            format!("{} {time}", self.day_month(d))
        } else {
            format!("{} {time}", self.date_long(d))
        }
    }

    /// "Today 14:31", "Sat 18:40", "12 Sep 14:31".
    pub fn moment_with_day(&self, t: DateTime<Utc>) -> String {
        let d = self.local(t).date();
        if d == self.today() {
            format!("{} {}", self.t("Today", "اليوم"), self.hm(t))
        } else {
            self.moment_label(t)
        }
    }

    /// Compact age: "now", "5m", "3h", "2d", "1w", "3mo", "1y".
    pub fn age(&self, t: DateTime<Utc>) -> String {
        let secs = (self.now - t).num_seconds().max(0);
        let (n, en, ar) = match secs {
            s if s < 60 => return self.t("now", "الآن"),
            s if s < 3600 => (s / 60, "m", " د"),
            s if s < 86_400 => (s / 3600, "h", " س"),
            s if s < 7 * 86_400 => (s / 86_400, "d", " ي"),
            s if s < 30 * 86_400 => (s / (7 * 86_400), "w", " أ"),
            s if s < 365 * 86_400 => (s / (30 * 86_400), "mo", " ش"),
            s => (s / (365 * 86_400), "y", " سنة"),
        };
        match self.lang {
            Lang::En => format!("{n}{en}"),
            Lang::Ar => format!("{n}{ar}"),
        }
    }

    /// "2 hours ago" / "منذ ساعتين"; "just now".
    pub fn ago(&self, t: DateTime<Utc>) -> String {
        let secs = (self.now - t).num_seconds().max(0);
        let phrase = match secs {
            s if s < 60 => return self.t("just now", "الآن"),
            s if s < 3600 => MINUTES.of(s / 60, self.lang),
            s if s < 86_400 => HOURS.of(s / 3600, self.lang),
            s if s < 7 * 86_400 => DAYS.of(s / 86_400, self.lang),
            s if s < 30 * 86_400 => WEEKS.of(s / (7 * 86_400), self.lang),
            s if s < 365 * 86_400 => MONTHS.of(s / (30 * 86_400), self.lang),
            s => YEARS.of(s / (365 * 86_400), self.lang),
        };
        match self.lang {
            Lang::En => format!("{phrase} ago"),
            Lang::Ar => format!("منذ {phrase}"),
        }
    }

    /// "Today", "Tomorrow", "Yesterday", else "Tue 29 Sep".
    pub fn relative_day(&self, d: NaiveDate) -> String {
        match (d - self.today()).num_days() {
            0 => self.t("Today", "اليوم"),
            1 => self.t("Tomorrow", "غدًا"),
            -1 => self.t("Yesterday", "أمس"),
            _ => self.weekday_date(d),
        }
    }

    /// "3 days late" for a date before today.
    pub fn lateness(&self, due: NaiveDate) -> Option<String> {
        let n = (self.today() - due).num_days();
        (n > 0).then(|| match self.lang {
            Lang::En => format!("{} late", DAYS.of(n, self.lang)),
            Lang::Ar => format!("متأخر {}", DAYS.of(n, self.lang)),
        })
    }

    /// "on time" / "2 days late" for a completion.
    pub fn completion(&self, done: NaiveDate, due: NaiveDate) -> String {
        let n = (done - due).num_days();
        if n <= 0 {
            self.t("on time", "في الموعد")
        } else {
            match self.lang {
                Lang::En => format!("{} late", DAYS.of(n, self.lang)),
                Lang::Ar => format!("متأخر {}", DAYS.of(n, self.lang)),
            }
        }
    }

    fn in_days(&self, d: NaiveDate) -> String {
        match (d - self.today()).num_days() {
            n if n <= 0 => self.t("today", "اليوم"),
            1 => self.t("tomorrow", "غدًا"),
            n => match self.lang {
                Lang::En => format!("in {}", DAYS.of(n, self.lang)),
                Lang::Ar => format!("بعد {}", DAYS.of(n, self.lang)),
            },
        }
    }

    /// "next in 4 days", "next today", "next tomorrow".
    pub fn next_in(&self, d: NaiveDate) -> String {
        match self.lang {
            Lang::En => format!("next {}", self.in_days(d)),
            Lang::Ar => format!("التالي {}", self.in_days(d)),
        }
    }

    /// "Next occurrence in 4 days".
    pub fn next_occurrence(&self, d: NaiveDate) -> String {
        match self.lang {
            Lang::En => format!("Next occurrence {}", self.in_days(d)),
            Lang::Ar => format!("الموعد التالي {}", self.in_days(d)),
        }
    }

    /// Upcoming group header: "TUE 29 SEP" (English upper-cased).
    pub fn group_header(&self, d: NaiveDate) -> String {
        match self.lang {
            Lang::En => self.weekday_date(d).to_uppercase(),
            Lang::Ar => self.weekday_date(d),
        }
    }

    /// "LATER".
    pub fn later(&self) -> String {
        self.t("LATER", "لاحقًا")
    }

    /// "Good afternoon, Shawket".
    pub fn greeting(&self, name: &str) -> String {
        let h = self.local(self.now).hour();
        match self.lang {
            Lang::En => {
                let part = match h {
                    5..=11 => "Good morning",
                    12..=16 => "Good afternoon",
                    _ => "Good evening",
                };
                format!("{part}, {name}")
            }
            Lang::Ar => {
                let part = if (5..=11).contains(&h) {
                    "صباح الخير"
                } else {
                    "مساء الخير"
                };
                format!("{part}، {name}")
            }
        }
    }

    /// A reminder relative to the due date: "on the day", "30 days before", "2 days after";
    /// without a due date the reminder's date ("Thu 1 Oct").
    pub fn reminder_offset(&self, due: Option<NaiveDate>, reminder: NaiveDate) -> String {
        let Some(due) = due else {
            return self.weekday_date(reminder);
        };
        match (due - reminder).num_days() {
            0 => self.t("on the day", "في نفس اليوم"),
            n if n > 0 => match self.lang {
                Lang::En => format!("{} before", DAYS.of(n, self.lang)),
                Lang::Ar => format!("قبل {}", DAYS.of(n, self.lang)),
            },
            n => match self.lang {
                Lang::En => format!("{} after", DAYS.of(-n, self.lang)),
                Lang::Ar => format!("بعد {}", DAYS.of(-n, self.lang)),
            },
        }
    }

    /// "18.4 MB".
    pub fn bytes(&self, n: u64) -> String {
        #[allow(clippy::cast_precision_loss)] // display rounding
        let f = n as f64;
        let (v, en, ar) = if n < 1024 {
            return match self.lang {
                Lang::En => format!("{n} B"),
                Lang::Ar => format!("{n} بايت"),
            };
        } else if n < 1024 * 1024 {
            (f / 1024.0, "KB", "ك.ب")
        } else if n < 1024 * 1024 * 1024 {
            (f / (1024.0 * 1024.0), "MB", "م.ب")
        } else {
            (f / (1024.0 * 1024.0 * 1024.0), "GB", "ج.ب")
        };
        match self.lang {
            Lang::En => format!("{v:.1} {en}"),
            Lang::Ar => format!("{v:.1} {ar}"),
        }
    }

    /// "Sat 18:40"-style label of a local wall-clock time (reminders).
    pub fn local_time_label(&self, t: NaiveDateTime) -> String {
        format!("{:02}:{:02}", t.hour(), t.minute())
    }

    /// Whole days from today to `d` (negative: past).
    pub fn days_from_today(&self, d: NaiveDate) -> i64 {
        (d - self.today()).num_days()
    }

    /// Monday of the current week.
    pub fn week_start(&self) -> NaiveDate {
        let t = self.today();
        t - Duration::days(i64::from(t.weekday().num_days_from_monday()))
    }
}

/// Avatar initials: first letters of the first and last words ("Sara Nabil" → "SN",
/// "أحمد سمير" → "أس", "acme" → "A"). Upper-cased where the script has case.
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.')
        .filter(|w| w.chars().next().is_some_and(char::is_alphanumeric))
        .collect();
    let first = |w: &str| w.chars().next().map(|c| c.to_uppercase().to_string());
    match words.as_slice() {
        [] => String::new(),
        [one] => first(one).unwrap_or_default(),
        [a, .., b] => format!(
            "{}{}",
            first(a).unwrap_or_default(),
            first(b).unwrap_or_default()
        ),
    }
}

/// Localised label of a relation type (`works-at` → "works at").
pub fn relation_label(rel: &str, lang: Lang) -> String {
    let (en, ar) = match rel {
        "related" => ("related", "مرتبط"),
        "part-of" => ("part of", "جزء من"),
        "supports" => ("supports", "يدعم"),
        "contradicts" => ("contradicts", "يتعارض مع"),
        "follows-up" => ("follows up", "متابعة لـ"),
        "duplicates" => ("duplicates", "مكرر لـ"),
        "concepts" => ("concepts", "مفاهيم"),
        "people" => ("people", "أشخاص"),
        "companies" => ("companies", "شركات"),
        "works-at" => ("works at", "يعمل في"),
        "worked-at" => ("worked at", "عمل في"),
        "reports-to" => ("reports to", "يتبع"),
        "knows" => ("knows", "يعرف"),
        "introduced-by" => ("introduced by", "عرّفه"),
        "client-of" => ("client of", "عميل لدى"),
        "supplier-of" => ("supplier of", "مورد لـ"),
        "partner-of" => ("partner of", "شريك لـ"),
        "competitor-of" => ("competitor of", "منافس لـ"),
        "subsidiary-of" => ("subsidiary of", "تابعة لـ"),
        "copy-of" => ("copy of", "نسخة من"),
        "link" => ("Links", "روابط"),
        "embed" => ("Embeds", "تضمينات"),
        "mention" => ("mentions", "يذكر"),
        "concept" => ("concept", "مفهوم"),
        "similarity" => ("similar", "مشابه"),
        "part-of-place" => ("inside", "داخل"),
        "custody" => ("custody", "العهدة"),
        "entity" => ("entity relation", "علاقة كيان"),
        "relation" => ("relation", "علاقة"),
        other => return other.replace('-', " "),
    };
    tr(lang, en, ar)
}

/// Localised label of a note kind.
pub fn kind_label(kind: &str, lang: Lang) -> String {
    let (en, ar) = match kind {
        "note" => ("Notes", "الملاحظات"),
        "concept" => ("Concepts", "المفاهيم"),
        "person" => ("People", "الأشخاص"),
        "company" => ("Companies", "الشركات"),
        "document" => ("Documents", "المستندات"),
        "place" => ("Places", "الأماكن"),
        "capture" => ("Captures", "الالتقاطات"),
        other => return other.to_owned(),
    };
    tr(lang, en, ar)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
    }

    fn cairo(now: &str, lang: Lang) -> Labels {
        Labels::new(at(now), chrono_tz::Africa::Cairo, lang)
    }

    #[test]
    fn list_and_moment_labels_are_local() {
        // Sun 27 Sep 2026 14:32 in Cairo (UTC+3, summer time).
        let l = cairo("2026-09-27T11:32:00Z", Lang::En);
        assert_eq!(l.today(), d("2026-09-27"));
        assert_eq!(l.list_label(at("2026-09-27T11:31:00Z")), "14:31");
        // 23:30 UTC on the 26th is already the 27th in Cairo.
        assert_eq!(l.list_label(at("2026-09-26T22:30:00Z")), "01:30");
        assert_eq!(l.list_label(at("2026-09-26T15:40:00Z")), "Sat");
        assert_eq!(l.list_label(at("2026-09-21T09:00:00Z")), "21 Sep");
        assert_eq!(l.list_label(at("2025-09-21T09:00:00Z")), "21 Sep 2025");
        assert_eq!(l.moment_label(at("2026-09-27T06:47:00Z")), "09:47");
        assert_eq!(l.moment_label(at("2026-09-26T15:40:00Z")), "Sat 18:40");
        assert_eq!(l.moment_label(at("2026-09-12T11:31:00Z")), "12 Sep 14:31");
        assert_eq!(l.moment_with_day(at("2026-09-27T11:31:00Z")), "Today 14:31");
        assert_eq!(l.today_long(), "Sunday 27 September");
        assert_eq!(l.greeting("Shawket"), "Good afternoon, Shawket");
    }

    #[test]
    fn cairo_dst_end_moves_the_wall_clock() {
        // Egypt's summer time ends at 24:00 on the last Thursday of October (29 Oct 2026):
        // 20:59 UTC is 23:59 (UTC+3), 22:30 UTC is 00:30 on the 30th (UTC+2).
        let l = cairo("2026-10-29T20:59:00Z", Lang::En);
        assert_eq!(l.hm(at("2026-10-29T20:59:00Z")), "23:59");
        assert_eq!(l.hm(at("2026-10-29T22:30:00Z")), "00:30");
        // A future day is shown as its date.
        assert_eq!(l.list_label(at("2026-10-29T22:30:00Z")), "30 Oct");
        let after = cairo("2026-10-30T10:00:00Z", Lang::En);
        assert_eq!(after.today(), d("2026-10-30"));
        assert_eq!(after.hm(at("2026-10-30T10:00:00Z")), "12:00");
        // Summer time starts on the last Friday of April (24 Apr 2026): 00:00 → 01:00.
        let spring = cairo("2026-04-23T22:30:00Z", Lang::En);
        assert_eq!(spring.hm(at("2026-04-23T22:30:00Z")), "01:30");
        assert_eq!(spring.hm(at("2026-04-23T21:30:00Z")), "23:30");
    }

    #[test]
    fn relative_labels() {
        let l = cairo("2026-09-27T11:32:00Z", Lang::En);
        assert_eq!(l.relative_day(d("2026-09-27")), "Today");
        assert_eq!(l.relative_day(d("2026-09-28")), "Tomorrow");
        assert_eq!(l.relative_day(d("2026-09-26")), "Yesterday");
        assert_eq!(l.relative_day(d("2026-09-29")), "Tue 29 Sep");
        assert_eq!(l.relative_day(d("2027-10-01")), "Fri 1 Oct 2027");
        assert_eq!(l.lateness(d("2026-09-24")), Some("3 days late".into()));
        assert_eq!(l.lateness(d("2026-09-26")), Some("1 day late".into()));
        assert_eq!(l.lateness(d("2026-09-27")), None);
        assert_eq!(l.next_in(d("2026-10-01")), "next in 4 days");
        assert_eq!(l.next_in(d("2026-09-28")), "next tomorrow");
        assert_eq!(
            l.next_occurrence(d("2026-10-01")),
            "Next occurrence in 4 days"
        );
        assert_eq!(l.group_header(d("2026-09-29")), "TUE 29 SEP");
        assert_eq!(
            l.reminder_offset(Some(d("2026-10-01")), d("2026-09-01")),
            "30 days before"
        );
        assert_eq!(
            l.reminder_offset(Some(d("2026-10-01")), d("2026-10-01")),
            "on the day"
        );
        assert_eq!(l.reminder_offset(None, d("2026-10-01")), "Thu 1 Oct");
        assert_eq!(
            l.completion(d("2026-10-03"), d("2026-10-01")),
            "2 days late"
        );
        assert_eq!(l.completion(d("2026-10-01"), d("2026-10-01")), "on time");
        assert_eq!(l.ago(at("2026-09-27T09:32:00Z")), "2 hours ago");
        assert_eq!(l.age(at("2026-09-25T11:32:00Z")), "2d");
        assert_eq!(l.age(at("2026-09-20T11:32:00Z")), "1w");
        assert_eq!(l.bytes(19_293_798), "18.4 MB");
        assert_eq!(l.weekday_date_year(d("2026-10-01")), "Thu 1 Oct 2026");
    }

    #[test]
    fn arabic_labels_and_plurals() {
        let l = cairo("2026-09-27T11:32:00Z", Lang::Ar);
        assert_eq!(l.today_long(), "الأحد 27 سبتمبر");
        assert_eq!(l.relative_day(d("2026-09-28")), "غدًا");
        assert_eq!(l.relative_day(d("2026-09-29")), "الثلاثاء 29 سبتمبر");
        assert_eq!(l.lateness(d("2026-09-24")), Some("متأخر 3 أيام".into()));
        assert_eq!(l.lateness(d("2026-09-25")), Some("متأخر يومان".into()));
        assert_eq!(l.lateness(d("2026-09-15")), Some("متأخر 12 يومًا".into()));
        assert_eq!(l.greeting("شوكت"), "مساء الخير، شوكت");
        assert_eq!(l.ago(at("2026-09-27T09:32:00Z")), "منذ ساعتين");
        assert_eq!(DAYS.of(100, Lang::Ar), "100 يوم");
        assert_eq!(l.moment_label(at("2026-09-26T15:40:00Z")), "السبت 18:40");
    }

    #[test]
    fn initials_of_names() {
        assert_eq!(initials("Sara Nabil"), "SN");
        assert_eq!(initials("Ahmed Samir El-Sayed"), "AS");
        assert_eq!(initials("أحمد سمير"), "أس");
        assert_eq!(initials("acme"), "A");
        assert_eq!(initials("  "), "");
        assert_eq!(initials("sara.n"), "SN");
    }

    #[test]
    fn relation_and_kind_labels() {
        assert_eq!(relation_label("works-at", Lang::En), "works at");
        assert_eq!(relation_label("works-at", Lang::Ar), "يعمل في");
        assert_eq!(relation_label("custom-type", Lang::En), "custom type");
        assert_eq!(kind_label("person", Lang::En), "People");
    }
}
