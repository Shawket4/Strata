//! The weekly `digest` job (PLAN §9.2, §16.3): the exact prompt input (the week's new
//! notes, entity open items, AI contradictions), uncited bullets dropped, citations rendered
//! as links (cited blocks made citable), one `ai: digest _ai/digests/<week>.md` commit, and
//! the schedule (weekly on the configured day at the nightly hour).
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod common;
mod pipeline_support;

use std::collections::BTreeMap;

use common::World;
use domain::NoteKind;
use pipeline_support::{assert_input, block, block_with, push, runner};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::ids;
use strata_common::{Clock, JobId};
use strata_index::repo::jobs::NewJob;
use strata_jobs::digest::{
    CitationInput, ContradictionInput, DigestInput, DigestNote, OpenItemInput, PeriodInput,
    SideInput,
};
use strata_jobs::pipeline::generated_block_id;
use strata_vault::ops::ai_apply::{AiChangeSet, EdgeAdd};
use strata_vault::ops::entities::NewEntity;
use strata_vault::ops::relations::AiEdge;
use vault_format::RelationKey;
use vault_format::sidecar::By;

#[tokio::test]
async fn the_weekly_digest_summarises_new_notes_open_items_and_contradictions() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let pricing = w
        .create(&sa, "notes/Pricing.md", "Discounts are capped at 5 percent.\n")
        .await;
    let deal = w
        .create(&sa, "notes/Deal.md", "Flat 10 percent discount for Acme. ^d1\n")
        .await;
    // An AI `contradicts` edge found this week.
    let mut set = AiChangeSet::new("link", deal);
    set.add.push(EdgeAdd {
        src: deal,
        edge: AiEdge {
            rel: RelationKey::Note(domain::RelationType::Contradicts),
            dst: pricing,
            confidence: 0.8,
            reason: "10% flat versus a 5% cap.".into(),
            model: "claude_cli/fake-model".into(),
        },
        by: By::Ai,
    });
    w.vault.ai_apply(&sa, set).await.expect("edge");
    // An entity page with an open item citing the deal.
    let shady = w
        .vault
        .create_entity(
            &sa,
            NewEntity {
                kind: NoteKind::Person,
                name: "Shady".into(),
                aliases: vec![],
                tags: vec![],
                fields: BTreeMap::new(),
                parent: None,
                id: None,
                force: true,
            },
        )
        .await
        .expect("entity");
    let page = w
        .read(a, "people/Shady.md")
        .replace("## Notes\n", "## Open items\n- Owes the signed copy [[Deal#^d1]]\n\n## Notes\n");
    w.vault
        .update_note(&sa, shady.id, page, shady.version.clone())
        .await
        .expect("page");

    // Monday 03:00: the digest of 2026-09-21 … 2026-09-27.
    w.db.clock.set("2026-09-28T03:00:00Z".parse().expect("t"));
    let job = JobId::generate(w.db.ids.as_ref());
    let mut tx = w.db.begin(a).await.expect("tx");
    let now = w.db.clock.now();
    strata_jobs::repo::enqueue(
        &mut tx,
        &NewJob {
            id: job,
            kind: "digest".into(),
            note_id: None,
            payload: vec![],
            run_after: now,
            max_attempts: 5,
            dedupe_key: Some("weekly-Mon".into()),
        },
        now,
    )
    .await
    .expect("enqueue");
    tx.commit().await.expect("commit");
    let pb = generated_block_id("Discounts are capped at 5 percent.");
    let input = DigestInput {
        period: PeriodInput {
            start: "2026-09-21".into(),
            end: "2026-09-27".into(),
            week: "2026-W39".into(),
        },
        new_notes: vec![
            DigestNote {
                id: deal.to_string(),
                title: "Deal".into(),
                created: "2026-09-27T12:00:00+00:00".into(),
                summary: None,
                blocks: vec![block_with("d1", "Flat 10 percent discount for Acme.")],
            },
            DigestNote {
                id: pricing.to_string(),
                title: "Pricing".into(),
                created: "2026-09-27T12:00:00+00:00".into(),
                summary: None,
                blocks: vec![block("Discounts are capped at 5 percent.")],
            },
        ],
        open_items: vec![OpenItemInput {
            entity_name: "Shady".into(),
            text: "Owes the signed copy".into(),
            citation: CitationInput {
                note_id: deal.to_string(),
                block_id: "d1".into(),
            },
        }],
        contradictions: vec![ContradictionInput {
            source: SideInput {
                note_id: deal.to_string(),
                title: "Deal".into(),
            },
            target: SideInput {
                note_id: pricing.to_string(),
                title: "Pricing".into(),
            },
            reason: "10% flat versus a 5% cap.".into(),
        }],
    };
    let c = |n: strata_common::NoteId, b: &str| json!({"note_id": n.to_string(), "block_id": b});
    push(
        &w,
        ids::DIGEST,
        &input,
        json!({
            "title": "Weekly digest 2026-W39", "lang": "en",
            "highlights": [
                {"text": "Acme gets a flat 10% discount", "citations": [c(deal, "d1")]},
                {"text": "Made up", "citations": [c(deal, "nope")]}
            ],
            "open_questions": [
                {"text": "Shady still owes the signed copy", "citations": [c(deal, "d1")]}
            ],
            "contradictions": [
                {"text": "Deal gives 10% while Pricing caps discounts at 5%",
                 "citations": [c(deal, "d1"), c(pricing, &pb)]}
            ]
        }),
    );
    runner(&w, &["digest"]).run_until_idle().await;
    assert_input(&w, ids::DIGEST, &input);
    let path = "_ai/digests/2026-W39.md";
    assert_eq!(w.log(a)[0], format!("ai: digest {path}"));
    assert_eq!(
        w.read(a, path),
        format!(
            "---\nid: {job}\ntitle: Weekly digest 2026-W39\ncreated: 2026-09-28T03:00:00+00:00\nupdated: 2026-09-28T03:00:00+00:00\n---\nWeek 2026-W39: 2026-09-21 – 2026-09-27.\n\n## Highlights\n- Acme gets a flat 10% discount [[Deal#^d1]]\n\n## Open questions\n- Shady still owes the signed copy [[Deal#^d1]]\n\n## Contradictions\n- Deal gives 10% while Pricing caps discounts at 5% [[Deal#^d1]] [[Pricing#^{pb}]]\n"
        )
    );
    // The cited block of Pricing got its ID in the same commit.
    assert!(
        w.read(a, "notes/Pricing.md")
            .ends_with(&format!("Discounts are capped at 5 percent. ^{pb}\n"))
    );
    let mut paths = w.last_commit_paths(a);
    paths.sort();
    assert_eq!(paths, vec![path.to_owned(), "notes/Pricing.md".to_owned()]);
    // The schedule: weekly on the configured day at the nightly hour.
    assert_eq!(
        strata_jobs::periodic(chrono::Weekday::Mon)
            .into_iter()
            .map(|p| (p.kind, p.cadence))
            .collect::<Vec<_>>(),
        vec![
            ("dedupe", strata_jobs::Cadence::Nightly),
            ("entity_insights_sweep", strata_jobs::Cadence::Nightly),
            ("digest", strata_jobs::Cadence::Weekly(chrono::Weekday::Mon)),
        ]
    );
    w.finish().await;
}
