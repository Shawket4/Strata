//! Property tests: round-trip identity for arbitrary and generated documents (random
//! frontmatter, links, embeds, block IDs, mixed scripts, CRLF/LF) and task lines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{NaiveDate, NaiveTime};
use proptest::prelude::*;
use vault_format::frontmatter::{Frontmatter, KnownKey, ValueShape};
use vault_format::tasks::{DateKind, Priority, Reminder, TaskLine, TaskSpec, TaskStatus};
use vault_format::{Document, LineEnding, PropertyValue, analyze};

fn eol() -> impl Strategy<Value = LineEnding> {
    prop_oneof![Just(LineEnding::Lf), Just(LineEnding::CrLf)]
}

/// Scalars that stress YAML quoting: Arabic, punctuation, look-alike numbers/booleans,
/// wikilinks, quotes, escapes.
fn scalar() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ._-]{0,16}",
        "[ء-ي ]{1,12}",
        "[a-zأ-ي0-9 :#,\\[\\]{}\"'|>&*!%@`?\\\\-]{0,16}",
        Just("true".to_owned()),
        Just("null".to_owned()),
        Just("0x1F".to_owned()),
        Just("12.50".to_owned()),
        Just("2026-09-27".to_owned()),
        Just("line\nbreak\ttab".to_owned()),
        "[a-zA-Z أ-ي]{1,10}".prop_map(|s| format!("[[{s}]]")),
        "[a-zA-Z]{1,8}".prop_map(|s| format!("[[{s}#^b1|{s}]]")),
    ]
}

fn known_entry() -> impl Strategy<Value = (String, PropertyValue)> {
    (
        prop::sample::select(KnownKey::ALL.to_vec()),
        scalar(),
        prop::collection::vec(scalar(), 0..4),
    )
        .prop_map(|(k, s, list)| {
            let v = match k.shape() {
                ValueShape::Text | ValueShape::Link => PropertyValue::Text(s),
                ValueShape::List | ValueShape::LinkList => PropertyValue::List(list),
            };
            (k.as_str().to_owned(), v)
        })
}

fn unknown_entry() -> impl Strategy<Value = (String, PropertyValue)> {
    (
        "[a-z][a-z0-9_]{0,8}( [a-z]{1,4})?",
        scalar(),
        prop::collection::vec(scalar(), 0..3),
        0..3u8,
    )
        .prop_filter("not a known key", |(k, ..)| {
            KnownKey::from_name(k).is_none()
        })
        .prop_map(|(k, s, list, pick)| {
            let v = match pick {
                0 => PropertyValue::Text(s),
                1 => PropertyValue::List(list),
                _ => PropertyValue::Null,
            };
            (k, v)
        })
}

fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z]{1,8}",
        "[ء-ي]{1,6}",
        "[a-zA-Z ء-ي]{1,10}".prop_map(|s| format!("[[{}]]", s.trim().replace(' ', "_") + "x")),
        "[a-z]{1,6}".prop_map(|s| format!("[[{s}|alias {s}]]")),
        "[a-z]{1,6}".prop_map(|s| format!("[[{s}#Heading]]")),
        "[a-z]{1,6}".prop_map(|s| format!("![[{s}.png]]")),
        "[a-z]{1,6}".prop_map(|s| format!("#{s}")),
        "[ء-ي]{1,6}".prop_map(|s| format!("#{s}")),
    ]
}

fn paragraph() -> impl Strategy<Value = (String, Option<String>)> {
    (
        prop::collection::vec(word(), 1..8),
        prop::option::of("[a-z0-9]{1,6}"),
    )
        .prop_map(|(words, id)| (words.join(" "), id))
}

fn body(eol: LineEnding) -> impl Strategy<Value = (String, usize, Vec<String>)> {
    prop::collection::vec(paragraph(), 0..6).prop_map(move |paras| {
        let e = eol.as_str();
        let mut ids = Vec::new();
        let mut text = String::new();
        let mut links = 0;
        for (i, (p, id)) in paras.iter().enumerate() {
            links += p.matches("[[").count();
            text.push_str(p);
            if let Some(id) = id {
                let id = format!("{id}-{i}");
                text.push_str(" ^");
                text.push_str(&id);
                ids.push(id);
            }
            text.push_str(e);
            text.push_str(e);
        }
        (text, links, ids)
    })
}

type DocParts = (
    LineEnding,
    Vec<(String, PropertyValue)>,
    (String, usize, Vec<String>),
);

fn doc_parts() -> impl Strategy<Value = DocParts> {
    eol().prop_flat_map(|e| {
        (
            Just(e),
            prop::collection::vec(prop_oneof![known_entry(), unknown_entry()], 0..10),
            body(e),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Any text parses and renders back unchanged.
    #[test]
    fn arbitrary_text_round_trips(s in any::<String>()) {
        prop_assert_eq!(Document::parse(&s).render(), s.clone());
        let _ = analyze(&s);
    }

    /// Every span from body analysis lies inside the text on character boundaries, and link
    /// spans slice back to the link. Bodies mix markdown structure, blank lines with
    /// whitespace, tabs and control characters.
    #[test]
    fn analysis_spans_are_sound(parts in prop::collection::vec(prop_oneof![
        Just("- [a]: b".to_owned()), Just("        ".to_owned()), Just("\t\t".to_owned()),
        Just("\u{b}".to_owned()), Just("```".to_owned()), Just("> q".to_owned()),
        Just("# H".to_owned()), Just("| a |\n|---|".to_owned()), Just("text ^id1".to_owned()),
        "[a-z ]{0,6}\\[\\[[a-zء-ي]{1,4}\\]\\][a-z #]{0,6}", "[ \t]{0,5}", any::<String>(),
    ], 0..12), sep in prop::sample::select(vec!["\n", "\r\n", "\r"])) {
        let s = parts.join(sep);
        let a = analyze(&s);
        let spans = a.links.iter().map(|l| l.span.clone())
            .chain(a.tags.iter().map(|t| t.span.clone()))
            .chain(a.headings.iter().map(|h| h.span.clone()))
            .chain(a.blocks.iter().map(|b| b.span.clone()))
            .chain(a.blocks.iter().filter_map(|b| b.id.as_ref().map(|i| i.span.clone())))
            .chain(a.code_spans.iter().cloned());
        for r in spans {
            prop_assert!(r.start <= r.end && r.end <= s.len());
            prop_assert!(s.is_char_boundary(r.start) && s.is_char_boundary(r.end));
        }
        for l in &a.links {
            let text = &s[l.span.clone()];
            prop_assert!(text.starts_with("[[") || text.starts_with("![["));
        }
    }

    /// Any text inside frontmatter delimiters round-trips too (valid YAML or not).
    #[test]
    fn arbitrary_frontmatter_round_trips(
        fm in "[a-z:\\-\\[\\]\"' #\n|>{}0-9ء-ي]{0,80}"
            .prop_filter("no delimiter line inside", |f| !f.lines().any(|l| l.trim_end_matches([' ', '\t']) == "---")),
        body in any::<String>(),
        crlf in any::<bool>(),
    ) {
        let e = if crlf { "\r\n" } else { "\n" };
        let text = format!("---{e}{fm}{e}---{e}{body}");
        let doc = Document::parse(&text);
        prop_assert_eq!(doc.render(), text.clone());
        prop_assert_eq!(doc.body(), body.as_str());
    }

    /// Documents built through the API render canonically, re-parse to the same values, and
    /// are stable under both renderers.
    #[test]
    fn generated_documents_round_trip((e, entries, (body, link_count, ids)) in doc_parts()) {
        let mut doc = Document::parse(&body);
        let mut expected: Vec<(String, PropertyValue)> = Vec::new();
        for (k, v) in entries {
            doc.frontmatter_mut().set(&k, v.clone()).unwrap();
            expected.retain(|(ek, _)| ek != &k);
            expected.push((k, v));
        }
        let text = doc.render();
        if !expected.is_empty() {
            // A new frontmatter uses the body's line ending (LF when the body has none).
            let e = if body.is_empty() { LineEnding::Lf } else { e };
            let opens = text.starts_with(&format!("---{}", e.as_str()));
            prop_assert!(opens);
        }
        let reparsed = Document::parse(&text);
        prop_assert_eq!(reparsed.render(), text.clone());
        prop_assert_eq!(reparsed.render_canonical(), text.clone());
        prop_assert_eq!(reparsed.body(), body.as_str());
        if let Some(fm) = reparsed.frontmatter() {
            prop_assert_eq!(fm.error(), None);
            for (k, v) in &expected {
                prop_assert_eq!(fm.get(k), Some(v), "key {}", k);
            }
            prop_assert_eq!(fm.len(), expected.len());
        } else {
            prop_assert!(expected.is_empty());
        }
        let a = reparsed.analyze_body();
        prop_assert_eq!(a.links.len(), link_count);
        let found: Vec<String> = a.blocks.iter().filter_map(|b| b.id.as_ref().map(|i| i.id.clone())).collect();
        prop_assert_eq!(found, ids);
    }

    /// Canonical rendering is idempotent for messy (hand-written) frontmatter.
    #[test]
    fn canonical_render_is_idempotent(entries in prop::collection::vec(prop_oneof![known_entry(), unknown_entry()], 0..8), messy in any::<bool>()) {
        let mut fm = Frontmatter::new(LineEnding::Lf);
        for (k, v) in &entries {
            fm.set(k, v.clone()).unwrap();
        }
        let mut text = fm.render();
        if messy {
            // Block-style lists and comments, as Obsidian users write them.
            text = text.replace("---\n", "---\n# comment\n");
        }
        let once = Document::parse(&text).render_canonical();
        let twice = Document::parse(&once).render_canonical();
        prop_assert_eq!(&once, &twice);
        prop_assert_eq!(Document::parse(&text).render(), text.clone());
    }
}

fn date() -> impl Strategy<Value = NaiveDate> {
    (2000i32..2100, 1u32..13, 1u32..29)
        .prop_map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap())
}

fn reminder() -> impl Strategy<Value = Reminder> {
    (date(), prop::option::of((0u32..24, 0u32..60))).prop_map(|(date, t)| Reminder {
        date,
        time: t.map(|(h, m)| NaiveTime::from_hms_opt(h, m, 0).unwrap()),
    })
}

fn description() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            "[a-zA-Z0-9']{1,8}",
            "[ء-ي]{1,6}",
            "[a-z]{1,6}".prop_map(|s| format!("[[{s}]]")),
            "[a-z]{1,6}".prop_map(|s| format!("#{s}")),
        ],
        0..6,
    )
    .prop_map(|w| w.join(" "))
}

fn task_spec() -> impl Strategy<Value = TaskSpec> {
    (
        (
            prop::sample::select(vec!["- ", "* ", "  - ", "1. ", "> - "]),
            prop::sample::select(vec![
                TaskStatus::Todo,
                TaskStatus::Done,
                TaskStatus::Cancelled,
                TaskStatus::Other('/'),
            ]),
            description(),
            prop::option::of(prop::sample::select(Priority::ALL.to_vec())),
            prop::option::of(prop::sample::select(vec![
                "every day",
                "every 2 weeks on Monday, Thursday",
                "every month on the 1st",
                "every year when done",
                "every blue moon",
            ])),
        ),
        prop::collection::vec(prop::option::of(date()), 6),
        prop::collection::vec(reminder(), 0..3),
        prop::option::of("t-[0-9a-z]{4,10}"),
    )
        .prop_map(
            |((prefix, status, description, priority, recurrence), dates, reminders, block_id)| {
                TaskSpec {
                    prefix: prefix.to_owned(),
                    status,
                    description,
                    priority,
                    recurrence: recurrence.map(str::to_owned),
                    created: dates[0],
                    start: dates[1],
                    scheduled: dates[2],
                    due: dates[3],
                    cancelled: dates[4],
                    done: dates[5],
                    reminders,
                    block_id,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Rendered task specs parse back to the same spec.
    #[test]
    fn task_specs_round_trip(spec in task_spec()) {
        let line = spec.render();
        let parsed = TaskLine::parse(&line).unwrap();
        prop_assert_eq!(parsed.as_str(), line.as_str());
        prop_assert_eq!(parsed.to_spec(), spec);
    }

    /// Any checklist line keeps its exact text, and edits keep it a valid, stable task.
    #[test]
    fn arbitrary_task_lines(rest in "[^\r\n]{0,60}", d in date(), status in prop::sample::select(vec![' ', 'x', '-', '/'])) {
        let line = format!("- [{status}] {rest}");
        let t = TaskLine::parse(&line).unwrap();
        prop_assert_eq!(t.as_str(), line.as_str());
        let edited = t.with_date(DateKind::Due, Some(d));
        prop_assert_eq!(edited.date(DateKind::Due), Some(d));
        let again = edited.with_date(DateKind::Due, Some(d));
        prop_assert_eq!(again.as_str(), edited.as_str());
        let reparsed = TaskLine::parse(edited.as_str()).unwrap();
        prop_assert_eq!(reparsed.date(DateKind::Due), Some(d));
        let removed = edited.with_date(DateKind::Due, None);
        prop_assert_eq!(removed.date(DateKind::Due), None);
        let with_id = t.with_block_id("t-abc");
        prop_assert_eq!(with_id.block_id(), Some("t-abc"));
    }

    /// Edits on rendered specs only touch the field being edited.
    #[test]
    fn edits_are_local(spec in task_spec(), d in date()) {
        let t = TaskLine::parse(&spec.render()).unwrap();
        let edited = t.with_date(DateKind::Scheduled, Some(d));
        let mut expected = spec.clone();
        expected.scheduled = Some(d);
        prop_assert_eq!(edited.to_spec(), expected);
    }
}
