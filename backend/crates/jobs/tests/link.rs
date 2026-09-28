//! The `link` job (PLAN §9.2, §9.4, §16.3): debounce after the last edit, the exact prompt
//! input, threshold application with sidecar provenance, `duplicates` as a suggestion,
//! rejected edges never re-added, removal of stale AI edges only, the skip on an unchanged
//! version, invalid output retried then failing, budget pauses, one `ai:` commit per job,
//! and isolation between users.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments
)]

mod common;
mod pipeline_support;

use common::World;
use pipeline_support::{
    CREATED, MODEL, Row, WRITTEN, advance, at, block, block_with, cand, decisions, enqueue,
    jobs_of, push, push_fixture, runner, sidecar, suggestions, version_of,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::BudgetLimits;
use strata_ai::prompts::ids;
use strata_jobs::link::{LinkInput, LinkNote};
use strata_testkit::Fixture;
use sync_model::suggestions::{DuplicateItem, DuplicatesPayload, kinds};
use vault_format::RelationKey;

fn input(
    note: strata_common::NoteId,
    title: &str,
    blocks: Vec<strata_jobs::pipeline::BlockInput>,
) -> LinkInput {
    LinkInput {
        note: LinkNote {
            id: note.to_string(),
            title: title.to_owned(),
            created: CREATED.to_owned(),
            blocks,
        },
        candidates: vec![],
        concepts: vec![],
        entities: vec![],
        rejected: vec![],
    }
}

fn rel(
    target: strata_common::NoteId,
    kind: &str,
    confidence: f64,
    reason: &str,
) -> serde_json::Value {
    json!({"target_id": target.to_string(), "type": kind, "confidence": confidence, "reason": reason})
}

fn linking(relations: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "relations": relations, "concepts": [], "mentions": [], "entity_relations": [],
        "custody": [], "tasks": []
    })
}

#[tokio::test]
async fn edits_are_linked_30_seconds_later_with_relations_at_or_above_the_threshold() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let churn = w
        .create(
            &sa,
            "notes/Churn notes.md",
            "Churn rose after the pricing change.\n",
        )
        .await;
    let caps = w
        .create(
            &sa,
            "notes/Discount caps.md",
            "Discounts are capped at five percent.\n",
        )
        .await;
    let note = w
        .create(
            &sa,
            "notes/Pricing tiers.md",
            "Pricing tiers for next year.\n\nWe offer flat discounts of ten percent. ^d1\n",
        )
        .await;
    let r = runner(&w, &["embed", "link"]);
    // The embed jobs run now and queue linking 30 s after the edit; nothing links yet.
    enqueue(&w, a, "embed", note).await;
    r.run_until_idle().await;
    assert_eq!(
        jobs_of(&w, a, "link").await,
        vec![
            ("queued".to_owned(), 0, at(30)),
            ("queued".to_owned(), 0, at(30)),
            ("queued".to_owned(), 0, at(30)),
        ]
    );
    assert_eq!(w.llm.calls().len(), 0);

    let mut i = input(
        note,
        "Pricing tiers",
        vec![
            block("Pricing tiers for next year."),
            block_with("d1", "We offer flat discounts of ten percent."),
        ],
    );
    i.candidates = vec![cand(caps, "Discount caps"), cand(churn, "Churn notes")];
    push(
        &w,
        ids::LINKING,
        &i,
        linking(vec![
            rel(caps, "contradicts", 0.72, "Flat 10% versus a 5% cap."),
            rel(churn, "related", 0.69, "Both discuss pricing."),
        ]),
    );
    // The other two notes get empty linking results.
    let mut ic = input(
        caps,
        "Discount caps",
        vec![block("Discounts are capped at five percent.")],
    );
    ic.candidates = vec![cand(note, "Pricing tiers")];
    push(&w, ids::LINKING, &ic, linking(vec![]));
    let mut ich = input(
        churn,
        "Churn notes",
        vec![block("Churn rose after the pricing change.")],
    );
    ich.candidates = vec![cand(note, "Pricing tiers")];
    push(&w, ids::LINKING, &ich, linking(vec![]));
    advance(&w, 30);
    r.run_until_idle().await;

    // Only `contradicts` (0.72 ≥ 0.7) is applied; `related` (0.69) is not.
    let text = w.read(a, "notes/Pricing tiers.md");
    assert_eq!(
        text,
        format!(
            "---\nid: {note}\ncreated: {WRITTEN}\nupdated: {WRITTEN}\ncontradicts: [\"[[Discount caps]]\"]\n---\nPricing tiers for next year.\n\nWe offer flat discounts of ten percent. ^d1\n"
        )
    );
    let version = version_of(&w, &sa, note).await;
    assert_eq!(
        sidecar(&w, a, note),
        json!({
            "id": note.to_string(),
            "relations": [{
                "type": "contradicts", "target_id": caps.to_string(), "by": "ai",
                "confidence": 0.72, "reason": "Flat 10% versus a 5% cap.", "model": MODEL,
                "created": "2026-09-27T12:00:30Z"
            }],
            "rejected": [],
            "last_linked_hash": version
        })
    );
    assert_eq!(w.log(a)[0], "ai: link notes/Pricing tiers.md");
    let mut paths = w.last_commit_paths(a);
    paths.sort();
    assert_eq!(
        paths,
        vec![
            format!(".meta/notes/{note}.json"),
            "notes/Pricing tiers.md".to_owned()
        ]
    );
    assert_eq!(
        decisions(&w, a).await,
        vec![Row {
            kind: "relation".into(),
            source: Some(note.to_string()),
            target: caps.to_string(),
            summary: "contradicts → Discount caps: Flat 10% versus a 5% cap.".into(),
            rel: Some("contradicts".into()),
            mention: None,
            suggested: false,
            committed: true,
            reverted: false,
        }]
    );
    // The two empty results changed nothing but the linked version (sidecar only).
    assert_eq!(
        sidecar(&w, a, churn)["last_linked_hash"],
        json!(version_of(&w, &sa, churn).await)
    );
    // Relinking an unchanged note makes no call.
    let calls = w.llm.calls().len();
    enqueue(&w, a, "link", note).await;
    r.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), calls);
    w.finish().await;
}

#[tokio::test]
async fn duplicates_become_a_suggestion_and_accepting_merges_the_notes() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let old = w
        .create(
            &sa,
            "notes/ETA invoices.md",
            "Watanya invoices go through ETA.\n",
        )
        .await;
    advance(&w, 60);
    let new = w
        .create(
            &sa,
            "notes/Watanya ETA.md",
            "---\ntags: [watanya]\n---\nWatanya invoices go through the ETA portal.\n",
        )
        .await;
    let mut i = input(
        new,
        "Watanya ETA",
        vec![block("Watanya invoices go through the ETA portal.")],
    );
    i.note.created = "2026-09-27T12:01:00+00:00".into();
    i.candidates = vec![cand(old, "ETA invoices")];
    push(
        &w,
        ids::LINKING,
        &i,
        linking(vec![rel(
            old,
            "duplicates",
            0.97,
            "Same statement about ETA.",
        )]),
    );
    enqueue(&w, a, "link", new).await;
    runner(&w, &["link"]).run_until_idle().await;
    let s = suggestions(&w, a).await;
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0],
        (
            "duplicates".to_owned(),
            "pending".to_owned(),
            json!({
                "a": {"id": new.to_string(), "item": new.to_string(), "snippet": null, "kind": "note",
                      "title": "Watanya ETA", "match_level": "semantic", "score": 0.97},
                "b": {"id": old.to_string(), "item": old.to_string(), "snippet": null, "kind": "note",
                      "title": "ETA invoices", "match_level": "semantic", "score": 0.97},
                "reason": "Same statement about ETA."
            })
        )
    );
    // Never applied automatically: no `duplicates` edge.
    assert!(!w.read(a, "notes/Watanya ETA.md").contains("duplicates"));

    let sid = pipeline_support::suggestion_ids(&w, a).await[0];
    let view = w
        .vault
        .decide_suggestion(&sa, sid, true)
        .await
        .expect("accept");
    assert_eq!(
        view.suggestion.status,
        strata_index::types::SuggestionStatus::Accepted
    );
    // The older note survives with the newer one's text under a dated heading; the newer one
    // is in the trash; one `user: merge` commit.
    assert_eq!(
        w.log(a)[0],
        "user: merge notes/Watanya ETA.md -> notes/ETA invoices.md"
    );
    assert_eq!(
        w.read(a, "notes/ETA invoices.md"),
        format!(
            "---\nid: {old}\naliases: [Watanya ETA]\ntags: [watanya]\ncreated: {WRITTEN}\nupdated: {WRITTEN}\n---\nWatanya invoices go through ETA.\n\n## Merged from Watanya ETA (2026-09-27)\n\nWatanya invoices go through the ETA portal.\n"
        )
    );
    assert!(w.dir(a).join(".trash/notes/Watanya ETA.md").exists());
    assert!(!w.dir(a).join("notes/Watanya ETA.md").exists());
    w.finish().await;
}

#[tokio::test]
async fn rejected_edges_are_never_re_added_and_only_stale_ai_edges_are_removed() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let b = w
        .create(&sa, "notes/Budget.md", "Budget for the pricing work.\n")
        .await;
    let c = w
        .create(&sa, "notes/Churn.md", "Churn after pricing.\n")
        .await;
    let d = w
        .create(&sa, "notes/Deals.md", "Deals and pricing.\n")
        .await;
    let e = w
        .create(&sa, "notes/Enterprise.md", "Enterprise pricing.\n")
        .await;
    let note = w.create(&sa, "notes/Pricing.md", "Pricing plan.\n").await;
    // A user edge that no linking run may remove.
    w.vault
        .add_relation(
            &sa,
            note,
            e,
            RelationKey::Note(domain::RelationType::Related),
        )
        .await
        .expect("user edge");
    let r = runner(&w, &["link"]);
    let mut i1 = input(note, "Pricing", vec![block("Pricing plan.")]);
    i1.candidates = vec![
        cand(b, "Budget"),
        cand(c, "Churn"),
        cand(d, "Deals"),
        cand(e, "Enterprise"),
    ];
    push(
        &w,
        ids::LINKING,
        &i1,
        linking(vec![
            rel(b, "related", 0.9, "Budget of the pricing work."),
            rel(c, "related", 0.8, "Churn follows pricing."),
        ]),
    );
    enqueue(&w, a, "link", note).await;
    r.run_until_idle().await;
    assert_eq!(
        w.read(a, "notes/Pricing.md")
            .lines()
            .find(|l| l.starts_with("related")),
        Some("related: [\"[[Enterprise]]\", \"[[Budget]]\", \"[[Churn]]\"]")
    );
    // The user rejects the AI edge to Budget.
    w.vault
        .remove_relation(
            &sa,
            note,
            b,
            RelationKey::Note(domain::RelationType::Related),
        )
        .await
        .expect("reject");
    // The note changes; the next run no longer offers Budget and gets it back anyway.
    let v = version_of(&w, &sa, note).await;
    let text = w
        .read(a, "notes/Pricing.md")
        .replace("Pricing plan.", "Pricing plan for deals.");
    w.vault.update_note(&sa, note, text, v).await.expect("edit");
    let mut i2 = input(note, "Pricing", vec![block("Pricing plan for deals.")]);
    // Keyword ranking: "deals" now matches too.
    i2.candidates = vec![cand(d, "Deals"), cand(c, "Churn"), cand(e, "Enterprise")];
    i2.rejected = vec![strata_jobs::pipeline::RejectedInput {
        kind: "related".into(),
        target_id: Some(b.to_string()),
        mention: None,
    }];
    push(
        &w,
        ids::LINKING,
        &i2,
        linking(vec![
            rel(b, "related", 0.95, "Budget again."),
            rel(d, "follows-up", 0.9, "Deals follow the plan."),
        ]),
    );
    enqueue(&w, a, "link", note).await;
    r.run_until_idle().await;
    let text = w.read(a, "notes/Pricing.md");
    // Budget stays rejected, the stale AI edge to Churn is gone, the user edge to Enterprise
    // stays, Deals is new.
    assert_eq!(
        text.lines()
            .filter(|l| l.starts_with("related") || l.starts_with("follows-up"))
            .collect::<Vec<_>>(),
        vec![
            "related: [\"[[Enterprise]]\"]",
            "follows-up: [\"[[Deals]]\"]"
        ]
    );
    let sc = sidecar(&w, a, note);
    assert_eq!(
        sc["relations"]
            .as_array()
            .expect("relations")
            .iter()
            .map(|r| (r["type"].clone(), r["target_id"].clone()))
            .collect::<Vec<_>>(),
        vec![(json!("follows-up"), json!(d.to_string()))]
    );
    assert_eq!(sc["rejected"][0]["target_id"], json!(b.to_string()));
    assert_eq!(w.log(a)[0], "ai: link notes/Pricing.md");
    w.finish().await;
}

/// The user content of a retry after a reply that was not JSON (`AiService` feedback).
fn not_json_retry(input: &LinkInput) -> String {
    format!(
        "{}\n\n---\nYour previous reply was not a JSON value.\nReply again with exactly one JSON object that matches the JSON schema supplied with this request.",
        strata_ai::prompts::render_input(input).expect("render")
    )
}

fn push_raw(w: &World, user: &str, f: Fixture) {
    let p = strata_ai::prompts::latest(ids::LINKING).expect("prompt");
    w.llm.push(
        &p.prompt_ref(),
        &strata_ai::request::input_hash(p.text, user),
        f,
    );
}

#[tokio::test]
async fn invalid_output_is_retried_then_the_job_fails_and_retries_later() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let note = w.create(&sa, "notes/Solo.md", "Solo note.\n").await;
    let i = input(note, "Solo", vec![block("Solo note.")]);
    // Every reply is not JSON: the service asks twice more with the feedback appended, then
    // the job fails for now and is queued again with backoff; nothing is written.
    push_fixture(&w, ids::LINKING, &i, Fixture::error("not_json", None));
    push_raw(&w, &not_json_retry(&i), Fixture::error("not_json", None));
    let r = runner(&w, &["link"]);
    enqueue(&w, a, "link", note).await;
    let commits = w.log(a).len();
    r.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 3);
    assert_eq!(
        w.llm
            .calls()
            .iter()
            .map(|c| c.user.clone())
            .collect::<Vec<_>>(),
        vec![
            strata_ai::prompts::render_input(&i).expect("render"),
            not_json_retry(&i),
            not_json_retry(&i)
        ]
    );
    assert_eq!(
        jobs_of(&w, a, "link").await,
        vec![("queued".to_owned(), 1, at(30))]
    );
    let mut tx = w.db.begin(a).await.expect("tx");
    let err: Option<String> = sqlx::query_scalar("SELECT last_error FROM jobs WHERE kind = 'link'")
        .fetch_one(tx.conn())
        .await
        .expect("error");
    tx.commit().await.expect("commit");
    assert_eq!(
        err.as_deref(),
        Some(
            "output of prompt linking v2 failed schema validation after 3 attempts: [\"/: not JSON\"]"
        )
    );
    assert_eq!(w.log(a).len(), commits);
    // On the retry the third attempt answers validly (the fake replays the queued not-JSON
    // reply once more first): applied then (sidecar only).
    push_raw(&w, &not_json_retry(&i), Fixture::json(linking(vec![])));
    advance(&w, 30);
    r.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 6);
    assert_eq!(
        jobs_of(&w, a, "link").await,
        vec![("done".to_owned(), 2, at(30))]
    );
    assert_eq!(w.log(a)[0], "ai: link notes/Solo.md");
    assert_eq!(w.log(a).len(), commits + 1);
    w.finish().await;
}

#[tokio::test]
async fn a_reached_budget_pauses_linking_until_the_next_day() {
    let w = World::with_limits(BudgetLimits {
        per_user_daily_tokens: 100,
        ..BudgetLimits::default()
    })
    .await;
    let (a, sa) = w.user("alice").await;
    let one = w.create(&sa, "notes/One.md", "First.\n").await;
    let two = w.create(&sa, "notes/Two.md", "Second.\n").await;
    push(
        &w,
        ids::LINKING,
        &input(one, "One", vec![block("First.")]),
        linking(vec![]),
    );
    push(
        &w,
        ids::LINKING,
        &input(two, "Two", vec![block("Second.")]),
        linking(vec![]),
    );
    let r = runner(&w, &["link"]);
    enqueue(&w, a, "link", one).await;
    r.run_until_idle().await;
    enqueue(&w, a, "link", two).await;
    r.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 1);
    let jobs = jobs_of(&w, a, "link").await;
    assert_eq!(
        jobs,
        vec![
            ("done".to_owned(), 1, at(0)),
            (
                "queued".to_owned(),
                0,
                "2026-09-28T00:00:00Z".parse().expect("t")
            ),
        ]
    );
    w.db.clock.set("2026-09-28T00:00:00Z".parse().expect("t"));
    r.run_until_idle().await;
    assert_eq!(w.llm.calls().len(), 2);
    assert_eq!(w.log(a)[0], "ai: link notes/Two.md");
    w.finish().await;
}

#[tokio::test]
async fn users_are_isolated_in_linking() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let (b, sb) = w.user("bob").await;
    // Same titles in both vaults: each user's candidates are their own notes only.
    let a_other = w
        .create(&sa, "notes/Pricing.md", "Alice pricing notes.\n")
        .await;
    let b_other = w
        .create(&sb, "notes/Pricing.md", "Bob pricing notes.\n")
        .await;
    let a_note = w.create(&sa, "notes/Plan.md", "Pricing plan.\n").await;
    let b_note = w.create(&sb, "notes/Plan.md", "Pricing plan.\n").await;
    let mut ia = input(a_note, "Plan", vec![block("Pricing plan.")]);
    ia.candidates = vec![cand(a_other, "Pricing")];
    push(
        &w,
        ids::LINKING,
        &ia,
        linking(vec![rel(a_other, "related", 0.9, "Pricing.")]),
    );
    let mut ib = input(b_note, "Plan", vec![block("Pricing plan.")]);
    ib.candidates = vec![cand(b_other, "Pricing")];
    // Bob's model reply names Alice's note: it is not a candidate and is ignored.
    push(
        &w,
        ids::LINKING,
        &ib,
        linking(vec![rel(a_other, "related", 0.9, "Leak.")]),
    );
    enqueue(&w, a, "link", a_note).await;
    enqueue(&w, b, "link", b_note).await;
    runner(&w, &["link"]).run_until_idle().await;
    assert!(
        w.read(a, "notes/Plan.md")
            .contains("related: [\"[[Pricing]]\"]")
    );
    assert!(!w.read(b, "notes/Plan.md").contains("related"));
    assert_eq!(decisions(&w, b).await, vec![]);
    assert_eq!(decisions(&w, a).await.len(), 1);
    assert_eq!(w.vault.ai_decisions(&sb, 50).await.expect("list"), vec![]);
    w.finish().await;
}

#[tokio::test]
async fn accepting_an_entity_duplicates_pair_merges_the_entities() {
    let w = World::new().await;
    let (a, sa) = w.user("alice").await;
    let entity = |name: &'static str, aliases: Vec<String>| {
        let w = &w;
        let sa = &sa;
        async move {
            w.vault
                .create_entity(
                    sa,
                    strata_vault::ops::entities::NewEntity {
                        created: strata_common::clock::default_test_epoch(),
                        kind: domain::NoteKind::Person,
                        name: name.into(),
                        aliases,
                        tags: vec![],
                        fields: std::collections::BTreeMap::new(),
                        parent: None,
                        id: None,
                        force: true,
                    },
                )
                .await
                .expect("entity")
                .id
        }
    };
    let shady = entity("Shady", vec![]).await;
    advance(&w, 60);
    let other = entity("Shadi", vec!["شادي".into()]).await;
    let item = |id: strata_common::NoteId, title: &str| DuplicateItem {
        id: id.as_ulid(),
        item: id.to_string(),
        snippet: None,
        kind: "person".into(),
        title: title.into(),
        match_level: dedupe::MatchLevel::Semantic,
        score: 0.97,
    };
    let payload = DuplicatesPayload {
        a: item(other, "Shadi"),
        b: item(shady, "Shady"),
        reason: None,
    };
    let sid = strata_common::SuggestionId::generate(w.db.ids.as_ref());
    w.vault
        .create_suggestion(
            &sa,
            sid,
            Some(other),
            kinds::DUPLICATES,
            &rmp_serde::to_vec_named(&payload).expect("payload"),
        )
        .await
        .expect("suggestion");
    w.vault
        .decide_suggestion(&sa, sid, true)
        .await
        .expect("accept");
    // The entity created first survives with the other's name and aliases; one merge commit.
    assert_eq!(
        w.log(a)[0],
        "user: merge people/Shadi.md -> people/Shady.md"
    );
    assert!(
        w.read(a, "people/Shady.md")
            .contains("aliases: [Shadi, شادي]")
    );
    assert!(!w.dir(a).join("people/Shadi.md").exists());
    assert_eq!(suggestions(&w, a).await[0].1, "accepted");
    w.finish().await;
}
