//! Golden fixtures: byte-exact round trips, canonical order, unknown keys, every link form,
//! Arabic file names and content, CRLF, code blocks with fake links, tasks, documents,
//! canvas and sidecar files.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate};
use pretty_assertions::assert_eq;
use vault_format::body::BlockKind;
use vault_format::canvas::Canvas;
use vault_format::custody::{self, CustodyState};
use vault_format::filename::{title_from_path, validate_vault_path};
use vault_format::frontmatter::{DocumentStatus, Lang, NoteKind};
use vault_format::sections::{self, AiProfile, AiSection};
use vault_format::sidecar::NoteSidecar;
use vault_format::tasks::{self, DateKind};
use vault_format::{Document, KnownKey, LineEnding, PropertyValue, RelationKey};

const PERSON: &str = include_str!("fixtures/people/أحمد سمير.md");
const MESSY: &str = include_str!("fixtures/notes/Obsidian messy.md");
const MESSY_EXPECTED: &str = include_str!("fixtures/notes/Obsidian messy.expected.md");
const MESSY_CANONICAL: &str = include_str!("fixtures/notes/Obsidian messy.canonical.md");
const CRLF: &str = include_str!("fixtures/notes/ملاحظة مختلطة CRLF.md");
const LINKS: &str = include_str!("fixtures/notes/Every link form.md");
const TASKS: &str = include_str!("fixtures/tasks/Tasks.md");
const CONTRACT: &str = include_str!("fixtures/documents/Watanya contract.md");
const CANVAS: &str = include_str!("fixtures/maps/Pricing.canvas");
const SIDECAR: &str = include_str!("fixtures/meta/01J8ZK3M4X7Q9W2E5R6T8Y0V1H.json");

fn fixture_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fixture_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

#[test]
fn every_markdown_fixture_round_trips_byte_for_byte() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    fixture_files(&root, &mut files);
    files.sort();
    assert_eq!(files.len(), 8);
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let doc = Document::parse(&text);
        assert_eq!(doc.render(), text, "{}", f.display());
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert_eq!(validate_vault_path(&rel), Ok(()), "{rel}");
    }
}

#[test]
fn canonical_person_note_is_stable() {
    let doc = Document::parse(PERSON);
    assert_eq!(doc.render(), PERSON);
    assert_eq!(doc.render_canonical(), PERSON);
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.error(), None);
    assert_eq!(fm.kind(), Some(NoteKind::Person));
    assert_eq!(fm.aliases(), ["أحمد سمير", "Ahmed S.", "A. Samir"]);
    assert_eq!(fm.lang(), Some(Lang::Mixed));
    assert_eq!(fm.text(KnownKey::Role), Some("Operations manager"));
    assert_eq!(fm.text(KnownKey::Phone), Some(""));
    assert_eq!(fm.relation(RelationKey::WorksAt), ["[[Acme Logistics]]"]);
    assert_eq!(
        fm.created().unwrap(),
        Some(DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").unwrap())
    );
    assert_eq!(title_from_path("people/أحمد سمير.md"), "أحمد سمير");
}

#[test]
fn editing_a_canonical_note_keeps_it_canonical() {
    let mut doc = Document::parse(PERSON);
    let fm = doc.frontmatter_mut();
    fm.add_relation_link(RelationKey::Knows, "مريم").unwrap();
    fm.set_updated(&DateTime::parse_from_rfc3339("2026-09-28T08:00:00+03:00").unwrap())
        .unwrap();
    let expected = PERSON
        .replace(
            "updated: 2026-09-27T15:10:00+03:00",
            "updated: 2026-09-28T08:00:00+03:00",
        )
        .replace(
            "knows: [\"[[Shady]]\"]",
            "knows: [\"[[Shady]]\", \"[[مريم]]\"]",
        );
    assert_eq!(doc.render(), expected);
    assert_eq!(doc.render_canonical(), expected);
}

#[test]
fn unknown_keys_survive_untouched_and_after_edits() {
    let doc = Document::parse(MESSY);
    assert_eq!(doc.render(), MESSY);
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.error(), None);
    assert_eq!(
        fm.keys().collect::<Vec<_>>(),
        [
            "tags",
            "cssclasses",
            "Title Case Key",
            "related",
            "nested",
            "date created",
            "publish",
            "id",
            "multiline",
            "aliases"
        ]
    );
    assert_eq!(fm.tags(), ["pricing", "pos"]);
    assert_eq!(fm.aliases(), ["pricing tests"]);
    assert_eq!(fm.get("nested"), Some(&PropertyValue::Other));
    assert_eq!(fm.get("publish"), Some(&PropertyValue::Text("true".into())));
    assert_eq!(
        fm.get("Title Case Key"),
        Some(&PropertyValue::Text("keep me".into()))
    );
    assert_eq!(
        fm.get("multiline"),
        Some(&PropertyValue::Text("line one\n\nline three\n".into()))
    );
    assert_eq!(
        fm.relation(RelationKey::Related),
        ["[[Churn notes]]", "[[Discount policy|policy]]"]
    );

    let mut edited = doc.clone();
    edited
        .frontmatter_mut()
        .add_relation_link(RelationKey::Concepts, "Pricing")
        .unwrap();
    assert_eq!(edited.render(), MESSY_EXPECTED);
    assert_eq!(doc.render_canonical(), MESSY_CANONICAL);
    // Re-parsing the outputs is stable.
    assert_eq!(Document::parse(MESSY_EXPECTED).render(), MESSY_EXPECTED);
    assert_eq!(
        Document::parse(MESSY_CANONICAL).render_canonical(),
        MESSY_CANONICAL
    );
}

#[test]
fn crlf_and_arabic_note() {
    let mut doc = Document::parse(CRLF);
    assert_eq!(doc.line_ending(), LineEnding::CrLf);
    assert_eq!(doc.render(), CRLF);
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.title(), Some("ملاحظة مختلطة — mixed note"));
    assert_eq!(fm.tags(), ["عربي", "english"]);
    assert_eq!(fm.get("x-plugin"), Some(&PropertyValue::Other));

    let analysis = doc.analyze_body();
    let links: Vec<_> = analysis.links.iter().map(|l| l.path.as_str()).collect();
    assert_eq!(links, ["أحمد سمير", "Acme Logistics", "شادي"]);
    let tags: Vec<_> = analysis.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tags, ["مشروع/قديم"]);
    let ids: Vec<_> = analysis
        .blocks
        .iter()
        .filter_map(|b| b.id.as_ref().map(|i| i.id.as_str()))
        .collect();
    assert_eq!(ids, ["p-ar1", "p-en1", "t-01j9x1"]);

    doc.frontmatter_mut()
        .set_text(KnownKey::Lang, "mixed")
        .unwrap();
    let out = doc.render();
    assert!(out.contains("lang: mixed\r\n"));
    assert!(!out.replace("\r\n", "").contains('\n'));
    assert_eq!(
        out,
        CRLF.replace(
            "created: 2026-09-27T09:00:00+03:00\r\n",
            "created: 2026-09-27T09:00:00+03:00\r\nlang: mixed\r\n"
        )
    );
}

fn describe_links(text: &str) -> String {
    let a = vault_format::analyze(text);
    let mut s = String::new();
    for l in &a.links {
        writeln!(
            s,
            "{:?} embed={} path={:?} anchor={:?} alias={:?} escaped={}",
            &text[l.span.clone()],
            l.embed,
            l.path,
            l.anchor,
            l.alias,
            l.escaped_pipe
        )
        .unwrap();
    }
    for t in &a.tags {
        writeln!(s, "tag {:?} at {:?}", t.name, t.span).unwrap();
    }
    for h in &a.headings {
        writeln!(s, "heading h{} {:?} path={:?}", h.level, h.text, h.path).unwrap();
    }
    for b in &a.blocks {
        writeln!(
            s,
            "block {:?} {:?} id={:?}",
            b.kind,
            &text[b.span.clone()],
            b.id.as_ref().map(|i| &i.id)
        )
        .unwrap();
    }
    s
}

#[test]
fn every_link_form_and_code_is_inert() {
    let doc = Document::parse(LINKS);
    assert_eq!(doc.render(), LINKS);
    insta::assert_snapshot!("every_link_form", describe_links(doc.body()));
    let a = doc.analyze_body();
    assert!(a.links.iter().all(|l| !l.path.contains("fake")
        && !l.path.contains("code")
        && !l.path.contains("math")
        && l.path != "escaped"));
    assert_eq!(
        a.block_by_id("quote-id").map(|b| b.kind),
        Some(BlockKind::BlockQuote)
    );
    assert_eq!(a.block_by_id("fakeid"), None);
}

fn describe_tasks(text: &str) -> String {
    let mut s = String::new();
    for t in tasks::extract_tasks(text) {
        let task = &t.task;
        let dates: Vec<String> = DateKind::ALL
            .iter()
            .filter_map(|k| task.date(*k).map(|d| format!("{k:?}={d}")))
            .collect();
        let rec = task.recurrence().map(|r| match r {
            Ok(rule) => rule.to_rrule(),
            Err(e) => format!("NOT UNDERSTOOD: {}", e.phrase),
        });
        writeln!(
            s,
            "line {} {:?} desc={:?} prio={:?} dates={:?} rec={:?} reminders={:?} id={:?}",
            t.line_number,
            task.status(),
            task.description(),
            task.priority(),
            dates,
            rec,
            task.reminders()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            task.block_id()
        )
        .unwrap();
    }
    s
}

#[test]
fn tasks_of_every_kind() {
    insta::assert_snapshot!("tasks_of_every_kind", describe_tasks(TASKS));
    for t in tasks::extract_tasks(TASKS) {
        // Every task line re-parses and renders to itself.
        assert_eq!(t.task.as_str(), &TASKS[t.line_span.clone()]);
    }
}

#[test]
fn completing_the_fixture_tasks() {
    let lines: Vec<&str> = TASKS.lines().collect();
    let done = NaiveDate::from_ymd_opt(2026, 9, 27).unwrap();
    assert_eq!(
        tasks::complete_recurring(lines[3], done, "t-01j9b1").unwrap(),
        [
            "- [ ] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-10-04 (@2026-10-04 10:00) [[Petrol Arrows]] ^t-01j9b1",
            "- [x] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-09-27 ✅ 2026-09-27 (@2026-09-27 10:00) [[Petrol Arrows]] ^t-01j9a3",
        ]
    );
    // `when done` counts from the completion date; reminders keep their offset from the due date.
    let done = NaiveDate::from_ymd_opt(2027, 4, 2).unwrap();
    assert_eq!(
        tasks::complete_recurring(lines[7], done, "t-01j9b2").unwrap(),
        [
            "- [ ] Renew licence 🔁 every year on March 31 when done 📅 2028-03-31 (@2028-03-24 09:00) (@2028-03-30 18:30) ^t-01j9b2",
            "- [x] Renew licence 🔁 every year on March 31 when done 📅 2027-03-31 ✅ 2027-04-02 (@2027-03-24 09:00) (@2027-03-30 18:30) ^t-01j8za",
        ]
    );
    // Unsupported recurrence is reported and the line is left alone.
    let err = tasks::complete_recurring(lines[9], done, "t-x").unwrap_err();
    assert_eq!(
        err.to_string(),
        "recurrence not understood: `every blue moon` (unknown unit `blue`)"
    );
}

#[test]
fn document_custody_matches_frontmatter() {
    let doc = Document::parse(CONTRACT);
    assert_eq!(doc.render(), CONTRACT);
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.kind(), Some(NoteKind::Document));
    assert_eq!(fm.expires().unwrap(), NaiveDate::from_ymd_opt(2027, 3, 31));
    let body = doc.body();
    let custody_section = sections::sections(body)
        .into_iter()
        .find(|s| s.title == "Custody")
        .unwrap();
    let (events, bad) = custody::parse_section(&body[custody_section.own_content_span.clone()]);
    assert_eq!(bad, vec![]);
    assert_eq!(events.len(), 3);
    let state = CustodyState::derive(&events).unwrap();
    assert_eq!(state.location.as_deref(), fm.text(KnownKey::Location));
    assert_eq!(
        state.holder.as_deref().unwrap_or(""),
        fm.text(KnownKey::Holder).unwrap()
    );
    assert_eq!(state.last_holder.as_deref(), fm.text(KnownKey::LastHolder));
    assert_eq!(Some(state.status.clone()), fm.status());
    assert_eq!(state.status, DocumentStatus::Stored);

    let mut rewritten = doc.clone();
    let new_body = sections::replace_ai_sections(
        body,
        AiProfile::Document,
        &[(
            AiSection::Custody,
            "- 2026-09-27 — handed-to [[Accountant]] — [[Capture 2026-09-27#^d9e0]]\n\
             - 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[Capture 2026-09-20#^c1d2]]\n\
             - 2026-09-10 — handed-to [[Shady]] — [[Capture 2026-09-10#^b7c8]]\n\
             - 2026-01-15 — stored-at [[Safe — Nasr City office]] — [[Capture 2026-01-15]]",
        )],
    )
    .unwrap();
    rewritten.set_body(new_body);
    let (events, _) = custody::parse_section(
        &rewritten.body()[sections::sections(rewritten.body())[1]
            .own_content_span
            .clone()],
    );
    let state = CustodyState::derive(&events).unwrap();
    state.write_to(rewritten.frontmatter_mut()).unwrap();
    let expected = CONTRACT
        .replace(
            "location: \"[[Safe — Nasr City office]]\"",
            "location: \"\"",
        )
        .replace("holder: \"\"", "holder: \"[[Accountant]]\"")
        .replace(
            "last-holder: \"[[Shady]]\"",
            "last-holder: \"[[Accountant]]\"",
        )
        .replace("status: stored", "status: checked-out")
        .replace(
            "## Custody\n",
            "## Custody\n- 2026-09-27 — handed-to [[Accountant]] — [[Capture 2026-09-27#^d9e0]]\n",
        );
    assert_eq!(rewritten.render(), expected);
}

#[test]
fn canvas_fixture_round_trips() {
    let c = Canvas::from_json(CANVAS).unwrap();
    assert_eq!(c.validate(), vec![]);
    assert_eq!(
        c.files(),
        ["notes/Pricing experiments.md", "people/أحمد سمير.md"]
    );
    assert_eq!(c.to_json(), CANVAS);
}

#[test]
fn sidecar_fixture_round_trips() {
    let s = NoteSidecar::from_json(SIDECAR).unwrap();
    assert_eq!(s.to_json().unwrap(), SIDECAR);
    let fm_id = Document::parse(PERSON).frontmatter().unwrap().id().unwrap();
    assert_eq!(Some(s.id), fm_id);
    assert_eq!(
        NoteSidecar::path_for(s.id),
        ".meta/notes/01J8ZK3M4X7Q9W2E5R6T8Y0V1H.json"
    );
}

#[test]
fn entity_sections_regenerate_without_touching_user_text() {
    let doc = Document::parse(PERSON);
    let new_body = sections::replace_ai_sections(
        doc.body(),
        AiProfile::Entity,
        &[
            (AiSection::Summary, "Ahmed runs operations at [[Acme Logistics]] in Cairo."),
            (
                AiSection::Timeline,
                "- 2026-09-20 — asked for the quote [[Call 2026-09-20#^e1]]\n- 2026-09-12 — call about invoicing [[Call 2026-09-12]]",
            ),
        ],
    )
    .unwrap();
    let expected = doc
        .body()
        .replace(
            "Ahmed runs operations at [[Acme Logistics]].",
            "Ahmed runs operations at [[Acme Logistics]] in Cairo.",
        )
        .replace(
            "## Timeline\n",
            "## Timeline\n- 2026-09-20 — asked for the quote [[Call 2026-09-20#^e1]]\n",
        );
    assert_eq!(new_body, expected);
    let notes_at = new_body.find("## Notes").unwrap();
    let old_notes_at = doc.body().find("## Notes").unwrap();
    assert_eq!(&new_body[notes_at..], &doc.body()[old_notes_at..]);
}
