//! The time-zone picker of Settings (`set_timezone`): the IANA zones of `chrono-tz`, each with
//! a name and region in the UI language, its current UTC offset and whether it is the
//! account's zone. The list is built here so the settings screen only renders and filters by
//! what the user types (no zone logic in Dart, L15).
//!
//! Zones are the canonical region zones (`Africa/…`, `America/…`, …, `Pacific/…`) plus `UTC`;
//! legacy spellings and pre-2013 names kept only for compatibility (`Asia/Calcutta`,
//! `Europe/Kiev`, `US/Eastern`, `Etc/GMT+3`, …) are left out. Arabic names cover the region
//! names and a curated set of cities; any other city keeps its Latin name (there is no bundled
//! CLDR data).

use chrono::{DateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;

use crate::format::direction::dir_of;
use crate::format::labels::Lang;
use crate::view::model::TimeZoneItem;

const REGIONS: &[(&str, &str, &str)] = &[
    ("Africa", "Africa", "أفريقيا"),
    ("America", "Americas", "الأمريكتان"),
    ("Antarctica", "Antarctica", "القارة القطبية الجنوبية"),
    ("Arctic", "Arctic", "القطب الشمالي"),
    ("Asia", "Asia", "آسيا"),
    ("Atlantic", "Atlantic", "المحيط الأطلسي"),
    ("Australia", "Australia", "أستراليا"),
    ("Europe", "Europe", "أوروبا"),
    ("Indian", "Indian Ocean", "المحيط الهندي"),
    ("Pacific", "Pacific", "المحيط الهادئ"),
];

/// Old names and spellings kept by tzdb only for compatibility (the `backward` file's
/// obsolete and renamed entries), never offered.
const LEGACY: &[&str] = &[
    "Africa/Asmera",
    "Africa/Timbuktu",
    "America/Argentina/ComodRivadavia",
    "America/Atka",
    "America/Buenos_Aires",
    "America/Catamarca",
    "America/Coral_Harbour",
    "America/Cordoba",
    "America/Ensenada",
    "America/Fort_Wayne",
    "America/Godthab",
    "America/Indianapolis",
    "America/Jujuy",
    "America/Knox_IN",
    "America/Louisville",
    "America/Mendoza",
    "America/Montreal",
    "America/Nipigon",
    "America/Pangnirtung",
    "America/Porto_Acre",
    "America/Rainy_River",
    "America/Rosario",
    "America/Santa_Isabel",
    "America/Shiprock",
    "America/Thunder_Bay",
    "America/Virgin",
    "America/Yellowknife",
    "Antarctica/South_Pole",
    "Asia/Ashkhabad",
    "Asia/Calcutta",
    "Asia/Choibalsan",
    "Asia/Chongqing",
    "Asia/Chungking",
    "Asia/Dacca",
    "Asia/Harbin",
    "Asia/Istanbul",
    "Asia/Kashgar",
    "Asia/Katmandu",
    "Asia/Macao",
    "Asia/Rangoon",
    "Asia/Saigon",
    "Asia/Tel_Aviv",
    "Asia/Thimbu",
    "Asia/Ujung_Pandang",
    "Asia/Ulan_Bator",
    "Atlantic/Faeroe",
    "Atlantic/Jan_Mayen",
    "Australia/ACT",
    "Australia/Canberra",
    "Australia/Currie",
    "Australia/LHI",
    "Australia/NSW",
    "Australia/North",
    "Australia/Queensland",
    "Australia/South",
    "Australia/Tasmania",
    "Australia/Victoria",
    "Australia/West",
    "Australia/Yancowinna",
    "Europe/Belfast",
    "Europe/Kiev",
    "Europe/Nicosia",
    "Europe/Tiraspol",
    "Europe/Uzhgorod",
    "Europe/Zaporozhye",
    "Pacific/Enderbury",
    "Pacific/Johnston",
    "Pacific/Ponape",
    "Pacific/Samoa",
    "Pacific/Truk",
    "Pacific/Yap",
];

/// Arabic city names (IANA city part → Arabic).
const CITIES_AR: &[(&str, &str)] = &[
    ("Abu_Dhabi", "أبوظبي"),
    ("Addis_Ababa", "أديس أبابا"),
    ("Aden", "عدن"),
    ("Algiers", "الجزائر"),
    ("Almaty", "ألماتي"),
    ("Amman", "عمّان"),
    ("Amsterdam", "أمستردام"),
    ("Anchorage", "أنكوريج"),
    ("Ankara", "أنقرة"),
    ("Athens", "أثينا"),
    ("Auckland", "أوكلاند"),
    ("Baghdad", "بغداد"),
    ("Bahrain", "البحرين"),
    ("Baku", "باكو"),
    ("Bangkok", "بانكوك"),
    ("Beirut", "بيروت"),
    ("Berlin", "برلين"),
    ("Brussels", "بروكسل"),
    ("Bucharest", "بوخارست"),
    ("Budapest", "بودابست"),
    ("Buenos_Aires", "بوينس آيرس"),
    ("Cairo", "القاهرة"),
    ("Casablanca", "الدار البيضاء"),
    ("Chicago", "شيكاغو"),
    ("Colombo", "كولومبو"),
    ("Comoro", "جزر القمر"),
    ("Copenhagen", "كوبنهاغن"),
    ("Damascus", "دمشق"),
    ("Dhaka", "دكا"),
    ("Djibouti", "جيبوتي"),
    ("Doha", "الدوحة"),
    ("Dubai", "دبي"),
    ("Dublin", "دبلن"),
    ("Gaza", "غزة"),
    ("Hebron", "الخليل"),
    ("Helsinki", "هلسنكي"),
    ("Hong_Kong", "هونغ كونغ"),
    ("Honolulu", "هونولولو"),
    ("Istanbul", "إسطنبول"),
    ("Jakarta", "جاكرتا"),
    ("Jerusalem", "القدس"),
    ("Johannesburg", "جوهانسبرغ"),
    ("Kabul", "كابول"),
    ("Karachi", "كراتشي"),
    ("Kathmandu", "كاتماندو"),
    ("Khartoum", "الخرطوم"),
    ("Kolkata", "كولكاتا"),
    ("Kuala_Lumpur", "كوالالمبور"),
    ("Kuwait", "الكويت"),
    ("Kyiv", "كييف"),
    ("Lagos", "لاغوس"),
    ("Lisbon", "لشبونة"),
    ("London", "لندن"),
    ("Los_Angeles", "لوس أنجلوس"),
    ("Madrid", "مدريد"),
    ("Maldives", "المالديف"),
    ("Manila", "مانيلا"),
    ("Mauritius", "موريشيوس"),
    ("Mexico_City", "مكسيكو سيتي"),
    ("Mogadishu", "مقديشو"),
    ("Moscow", "موسكو"),
    ("Muscat", "مسقط"),
    ("Nairobi", "نيروبي"),
    ("New_York", "نيويورك"),
    ("Nouakchott", "نواكشوط"),
    ("Oslo", "أوسلو"),
    ("Paris", "باريس"),
    ("Prague", "براغ"),
    ("Qatar", "قطر"),
    ("Reykjavik", "ريكيافيك"),
    ("Riyadh", "الرياض"),
    ("Rome", "روما"),
    ("Sao_Paulo", "ساو باولو"),
    ("Seoul", "سول"),
    ("Shanghai", "شنغهاي"),
    ("Singapore", "سنغافورة"),
    ("Stockholm", "ستوكهولم"),
    ("Sydney", "سيدني"),
    ("Taipei", "تايبيه"),
    ("Tashkent", "طشقند"),
    ("Tbilisi", "تبليسي"),
    ("Tehran", "طهران"),
    ("Tokyo", "طوكيو"),
    ("Toronto", "تورونتو"),
    ("Tripoli", "طرابلس"),
    ("Tunis", "تونس"),
    ("Vancouver", "فانكوفر"),
    ("Vienna", "فيينا"),
    ("Warsaw", "وارسو"),
    ("Yerevan", "يريفان"),
    ("Zurich", "زيورخ"),
];

fn city_ar(city: &str) -> Option<&'static str> {
    CITIES_AR.iter().find(|(k, _)| *k == city).map(|(_, v)| *v)
}

/// "UTC", "UTC+3", "UTC+5:30", "UTC-3:30".
pub fn offset_label(minutes: i32) -> String {
    if minutes == 0 {
        return "UTC".to_owned();
    }
    let sign = if minutes < 0 { '-' } else { '+' };
    let (h, m) = (minutes.abs() / 60, minutes.abs() % 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

/// The offset of `tz` from UTC at `now`, in minutes.
pub fn offset_minutes(tz: Tz, now: DateTime<Utc>) -> i32 {
    tz.offset_from_utc_datetime(&now.naive_utc())
        .fix()
        .local_minus_utc()
        / 60
}

fn item(tz: Tz, now: DateTime<Utc>, current: Tz, lang: Lang) -> Option<TimeZoneItem> {
    let id = tz.name();
    let minutes = offset_minutes(tz, now);
    let (name, region) = if id == "UTC" {
        match lang {
            Lang::En => ("Coordinated Universal Time".to_owned(), String::new()),
            Lang::Ar => ("التوقيت العالمي المنسق".to_owned(), String::new()),
        }
    } else {
        let (area, rest) = id.split_once('/')?;
        if LEGACY.contains(&id) {
            return None;
        }
        let (_, en, ar) = REGIONS.iter().find(|(k, _, _)| *k == area)?;
        let city = rest.rsplit('/').next().unwrap_or(rest);
        let latin = city.replace('_', " ");
        match lang {
            Lang::En => (latin, (*en).to_owned()),
            Lang::Ar => (city_ar(city).map_or(latin, str::to_owned), (*ar).to_owned()),
        }
    };
    Some(TimeZoneItem {
        id: id.to_owned(),
        name_dir: dir_of(&name),
        name,
        region,
        offset_minutes: minutes,
        offset_label: offset_label(minutes),
        is_current: tz == current,
    })
}

/// The zones matching `query` (IANA ID, the name and region in either language, or the
/// offset label; case, Arabic letter variants and diacritics ignored), sorted by current
/// offset, then name. An empty query lists every zone.
pub fn timezones(now: DateTime<Utc>, current: Tz, lang: Lang, query: &str) -> Vec<TimeZoneItem> {
    let q = text_normalize::normalize_for_search(query.trim());
    let mut out: Vec<TimeZoneItem> = chrono_tz::TZ_VARIANTS
        .iter()
        .filter_map(|tz| {
            let shown = item(*tz, now, current, lang)?;
            if q.is_empty() {
                return Some(shown);
            }
            let other = item(
                *tz,
                now,
                current,
                match lang {
                    Lang::En => Lang::Ar,
                    Lang::Ar => Lang::En,
                },
            )?;
            let haystack = [
                shown.id.replace('_', " "),
                shown.name.clone(),
                shown.region.clone(),
                other.name,
                other.region,
                shown.offset_label.clone(),
            ];
            haystack
                .iter()
                .any(|h| text_normalize::normalize_for_search(h).contains(&q))
                .then_some(shown)
        })
        .collect();
    out.sort_by(|a, b| (a.offset_minutes, &a.name, &a.id).cmp(&(b.offset_minutes, &b.name, &b.id)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        // Cairo is on summer time (UTC+3) until the last Thursday of October.
        "2026-09-28T09:00:00Z".parse().expect("time")
    }

    #[test]
    fn offset_labels() {
        assert_eq!(offset_label(0), "UTC");
        assert_eq!(offset_label(180), "UTC+3");
        assert_eq!(offset_label(330), "UTC+5:30");
        assert_eq!(offset_label(-210), "UTC-3:30");
        assert_eq!(offset_label(-600), "UTC-10");
    }

    #[test]
    fn cairo_in_both_languages() {
        let cairo = chrono_tz::Africa::Cairo;
        assert_eq!(
            timezones(now(), cairo, Lang::En, "cairo"),
            vec![TimeZoneItem {
                id: "Africa/Cairo".into(),
                name: "Cairo".into(),
                region: "Africa".into(),
                offset_minutes: 180,
                offset_label: "UTC+3".into(),
                is_current: true,
                name_dir: crate::view::model::TextDir::Ltr,
            }]
        );
        // Arabic names are searchable in either script, with letter variants folded.
        let ar = timezones(now(), chrono_tz::UTC, Lang::Ar, "القاهره");
        assert_eq!(
            ar,
            vec![TimeZoneItem {
                id: "Africa/Cairo".into(),
                name: "القاهرة".into(),
                region: "أفريقيا".into(),
                offset_minutes: 180,
                offset_label: "UTC+3".into(),
                is_current: false,
                name_dir: crate::view::model::TextDir::Rtl,
            }]
        );
        assert_eq!(timezones(now(), chrono_tz::UTC, Lang::Ar, "Cairo"), ar);
        // After the October switch Cairo is UTC+2.
        let winter: DateTime<Utc> = "2026-11-02T09:00:00Z".parse().expect("time");
        assert_eq!(
            timezones(winter, cairo, Lang::En, "Africa/Cairo")[0].offset_label,
            "UTC+2"
        );
    }

    #[test]
    fn the_list_is_canonical_and_sorted_by_offset() {
        let all = timezones(now(), chrono_tz::UTC, Lang::En, "");
        let ids: Vec<&str> = all.iter().map(|z| z.id.as_str()).collect();
        for legacy in [
            "Asia/Calcutta",
            "Europe/Kiev",
            "US/Eastern",
            "Etc/GMT+3",
            "EST5EDT",
        ] {
            assert!(!ids.contains(&legacy), "{legacy}");
        }
        for kept in [
            "Asia/Kolkata",
            "Europe/Kyiv",
            "UTC",
            "Europe/Oslo",
            "Asia/Kuwait",
        ] {
            assert!(ids.contains(&kept), "{kept}");
        }
        assert!(
            all.windows(2)
                .all(|w| w[0].offset_minutes <= w[1].offset_minutes)
        );
        assert_eq!(all.iter().filter(|z| z.is_current).count(), 1);
        let utc = all.iter().find(|z| z.id == "UTC").expect("utc");
        assert_eq!(
            (
                utc.name.as_str(),
                utc.region.as_str(),
                utc.offset_label.as_str()
            ),
            ("Coordinated Universal Time", "", "UTC")
        );
        // Offsets are searchable.
        let plus_530: Vec<String> = timezones(now(), chrono_tz::UTC, Lang::En, "UTC+5:30")
            .into_iter()
            .map(|z| z.id)
            .collect();
        assert_eq!(plus_530, ["Asia/Colombo", "Asia/Kolkata"]);
    }
}
