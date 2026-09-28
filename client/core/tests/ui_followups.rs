//! UI follow-ups on one device (fake server): citation previews carry the block's line,
//! offset and anchor; Home's AI activity items say whether they can be repointed and list the
//! targets; the time-zone picker; exact editor markers through the view builder.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

use chrono::{DateTime, Utc};
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::net::AiDecisionInfo;
use strata_core::store::cache;
use strata_core::view::model::{HintKind, MarkerRange, RepointChoice, TextDir};
use strata_core::view::{build, extra};

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).expect("instant").to_utc()
}

fn utf16(s: &str) -> u32 {
    u32::try_from(s.encode_utf16().count()).expect("fits")
}

#[tokio::test]
async fn citations_carry_line_offset_and_anchor() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let body = "# خطة الربع\n\nمقدمة 😀\n\n- first point ^b1\n\n## Risks\nnone\n";
    let id = s
        .create_note("notes/Plan.md", body, false)
        .expect("note")
        .id
        .expect("id");
    let content = s
        .read(|c, _| {
            Ok(strata_core::store::notes::current(c, &id)?
                .expect("note")
                .content)
        })
        .expect("read");
    let preview = |anchor: Option<&str>| {
        s.read(|c, ctx| extra::citation_preview(c, ctx, &id, anchor))
            .expect("preview")
    };
    // A block anchor: the offset is where the block's line starts (UTF-16 units, Arabic and
    // an emoji before it), and the line counts the frontmatter.
    let block_at = content.find("- first point").expect("block");
    let p = preview(Some("^b1"));
    assert_eq!(
        (
            p.anchor.as_deref(),
            p.line,
            p.offset,
            p.block_text.as_deref(),
            p.heading.as_deref()
        ),
        (
            Some("b1"),
            Some(u32::try_from(content[..block_at].matches('\n').count()).expect("line")),
            Some(utf16(&content[..block_at])),
            Some("first point"),
            Some("خطة الربع")
        )
    );
    // The same without `^`; a heading anchor.
    assert_eq!(preview(Some("b1")).offset, p.offset);
    let risks_at = content.find("## Risks").expect("heading");
    let r = preview(Some("Risks"));
    assert_eq!(
        (r.anchor.as_deref(), r.offset),
        (Some("Risks"), Some(utf16(&content[..risks_at])))
    );
    // A missing anchor or none at all: no position.
    let missing = preview(Some("^gone"));
    assert_eq!(
        (
            missing.anchor,
            missing.line,
            missing.offset,
            missing.block_text
        ),
        (None, None, None, None)
    );
    let top = preview(None);
    assert_eq!((top.anchor, top.line, top.offset), (None, None, None));
}

fn decision(id: &str, kind: &str, source: &str, target: &str) -> AiDecisionInfo {
    AiDecisionInfo {
        id: id.to_owned(),
        kind: kind.to_owned(),
        rel_type: Some("people".to_owned()),
        summary: "\"Ahmed\" · Ahmed Samir".to_owned(),
        source_note_id: Some(source.to_owned()),
        source_title: Some("Call".to_owned()),
        target_id: target.to_owned(),
        target_name: Some("Ahmed Samir".to_owned()),
        confidence: Some(0.9),
        created: at("2026-09-27T09:00:00Z"),
        reverted_at: None,
        suggestion_id: None,
    }
}

#[tokio::test]
async fn home_ai_activity_can_be_repointed_to_a_note_of_the_same_kind() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let person = |name: &str| {
        s.create_entity(domain::NoteKind::Person, name, &[], true)
            .expect("person")
            .id
            .expect("id")
    };
    let samir = person("Ahmed Samir");
    let fathy = person("Ahmed Fathy");
    let mona = person("منى");
    let call = s
        .create_note("notes/Call.md", "Called Ahmed\n", false)
        .expect("note")
        .id
        .expect("id");
    let acme = s
        .create_entity(domain::NoteKind::Company, "Acme", &[], false)
        .expect("company")
        .id
        .expect("id");
    let mut custody = decision("D3", "custody_event", &call, &acme);
    custody.rel_type = None;
    let mut undone = decision("D2", "entity_mention", &call, &samir);
    undone.reverted_at = Some(at("2026-09-27T09:30:00Z"));
    s.read(|c, _| {
        cache::put(
            c,
            cache::AI_DECISIONS,
            &vec![
                decision("D1", "entity_mention", &call, &samir),
                undone,
                custody,
            ],
            "2026-09-27T10:00:00Z",
        )
    })
    .expect("cache");
    let home = s.read(build::home).expect("home");
    assert_eq!(
        home.ai_activity_items
            .iter()
            .map(|i| (i.decision_id.as_str(), i.can_repoint))
            .collect::<Vec<_>>(),
        vec![("D1", true), ("D2", false), ("D3", false)]
    );
    // People only, never the current target or the source; matched in either script.
    let choices = |q: &str| {
        s.read(|c, _| extra::repoint_choices(c, "D1", q))
            .expect("choices")
    };
    let choice = |id: &str, title: &str, dir| RepointChoice {
        id: id.to_owned(),
        title: title.to_owned(),
        title_dir: dir,
        kind: "person".to_owned(),
        folder: "people".to_owned(),
    };
    assert_eq!(
        choices(""),
        vec![
            choice(&fathy, "Ahmed Fathy", TextDir::Ltr),
            choice(&mona, "منى", TextDir::Rtl),
        ]
    );
    assert_eq!(
        choices("fath"),
        vec![choice(&fathy, "Ahmed Fathy", TextDir::Ltr)]
    );
    assert_eq!(choices("مني"), vec![choice(&mona, "منى", TextDir::Rtl)]);
    // An unknown decision has no choices.
    assert_eq!(
        s.read(|c, _| extra::repoint_choices(c, "D9", ""))
            .expect("none"),
        Vec::new()
    );
}

#[tokio::test]
async fn time_zones_follow_the_account_zone_and_language() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let ctx = s.ctx();
    let zones = strata_core::format::timezones::timezones(ctx.now, ctx.tz, ctx.lang, "");
    let current: Vec<&str> = zones
        .iter()
        .filter(|z| z.is_current)
        .map(|z| z.id.as_str())
        .collect();
    assert_eq!(current, vec!["Africa/Cairo"]);
    // Every zone offered is accepted by `set_timezone`'s check.
    assert!(zones.iter().all(|z| z.id.parse::<chrono_tz::Tz>().is_ok()));
}

#[tokio::test]
async fn editor_hints_carry_exact_markers() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    let content = "***both*** ###\n";
    let hints = s
        .read(|c, _| build::hints_resolved(c, "notes/x.md", content))
        .expect("hints");
    let styled: Vec<(HintKind, Vec<MarkerRange>)> = hints
        .into_iter()
        .filter(|h| !h.markers.is_empty())
        .map(|h| (h.kind, h.markers))
        .collect();
    let r = |start, end| MarkerRange { start, end };
    assert_eq!(
        styled,
        vec![
            (HintKind::Italic, vec![r(0, 1), r(9, 10)]),
            (HintKind::Bold, vec![r(1, 3), r(7, 9)]),
        ]
    );
}
