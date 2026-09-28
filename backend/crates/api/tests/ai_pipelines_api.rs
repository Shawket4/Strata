//! The AI pipelines' endpoints through the generated client (PLAN §7.5, §9.3, §9.8, §16.3),
//! every response validated against the contract: the decision feed, repoint and reject with
//! their commits and hints, typed suggestion payloads, accept with edits (link-or-create with
//! aliases), entity refresh and relink queueing forced jobs, the auto-file setting, and
//! isolation (another user's IDs are `404`).
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod ai_harness;

use std::collections::BTreeMap;
use std::sync::Arc;

use ai_harness::{H, User, plain, problem};
use domain::NoteKind;
use pretty_assertions::assert_eq;
use serde_json::json;
use strata_ai::prompts::{self, ids};
use strata_ai::request::input_hash;
use strata_client::{operations as ops, types};
use strata_common::NoteId;
use strata_index::UserScope;
use strata_jobs::link::{LinkInput, LinkNote};
use strata_jobs::pipeline::{BlockInput, EntityInput, generated_block_id};
use strata_jobs::{JobHandler, Runner, RunnerConfig};
use strata_testkit::Fixture;
use strata_vault::ops::entities::NewEntity;
use strata_vault::ops::notes::CreateNote;

const CREATED: &str = "2026-09-27T12:00:00+00:00";

fn scope(h: &H, u: &User) -> UserScope {
    h.db.scope(u.id)
}

async fn entity(h: &H, s: &UserScope, kind: NoteKind, name: &str, aliases: &[&str]) -> NoteId {
    h.vault
        .create_entity(
            s,
            NewEntity {
                kind,
                name: name.into(),
                aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
                tags: vec![],
                fields: BTreeMap::new(),
                parent: None,
                id: None,
                force: true,
            },
        )
        .await
        .expect("entity")
        .id
}

async fn note(h: &H, s: &UserScope, path: &str, text: &str) -> NoteId {
    h.vault
        .create_note(
            s,
            CreateNote {
                path: path.into(),
                content: format!("{text}\n"),
                id: None,
                force: true,
            },
        )
        .await
        .expect("note")
        .id
}

fn ent(id: NoteId, kind: &str, name: &str, aliases: &[&str]) -> EntityInput {
    EntityInput {
        id: id.to_string(),
        kind: kind.into(),
        name: name.into(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        hints: vec![],
        part_of: None,
    }
}

fn input(id: NoteId, title: &str, text: &str, entities: Vec<EntityInput>) -> LinkInput {
    LinkInput {
        note: LinkNote {
            id: id.to_string(),
            title: title.into(),
            created: CREATED.into(),
            blocks: vec![BlockInput {
                block_id: generated_block_id(text),
                text: text.into(),
            }],
        },
        candidates: vec![],
        concepts: vec![],
        entities,
        rejected: vec![],
    }
}

fn push(h: &H, i: &LinkInput, mention: &str, existing: Option<NoteId>, nick: bool, text: &str) {
    let p = prompts::latest(ids::LINKING).expect("prompt");
    let user = prompts::render_input(i).expect("render");
    h.llm.push(
        &p.prompt_ref(),
        &input_hash(p.text, &user),
        Fixture::json(json!({
            "relations": [], "concepts": [], "entity_relations": [], "custody": [], "tasks": [],
            "mentions": [{"text": mention, "kind": "person",
                          "existing_id": existing.map(|e| e.to_string()),
                          "candidate_ids": [], "is_nickname": nick, "confidence": 0.9,
                          "evidence_block_id": generated_block_id(text)}]
        })),
    );
}

async fn run_link(h: &H, user: &User, note: NoteId) {
    let deps = strata_jobs::Deps {
        db: h.db.app_db.clone(),
        vault: h.vault.clone(),
        ai: h.ai.clone(),
        embedder: None,
        clock: Arc::new(h.clock.clone()),
        ids: h.db.ids.clone(),
        thresholds: dedupe::Thresholds::new(),
        ai_thresholds: strata_jobs::thresholds::AiThresholds::default(),
        default_tz: chrono_tz::UTC,
    };
    let handlers: Vec<Arc<dyn JobHandler>> = strata_jobs::standard_handlers(&deps)
        .into_iter()
        .filter(|h| h.kind() == "link")
        .collect();
    let mut tx = h.db.begin(user.id).await.expect("tx");
    let now = strata_common::Clock::now(&h.clock);
    strata_jobs::repo::enqueue_for_note(&mut tx, h.db.ids.as_ref(), "link", note, now, now)
        .await
        .expect("enqueue");
    tx.commit().await.expect("commit");
    Runner::new(
        h.db.app_db.clone(),
        h.db.issuer.clone(),
        Arc::new(h.clock.clone()),
        Arc::new(strata_jobs::RecordedEvents::default()),
        RunnerConfig {
            idle_recheck: chrono::Duration::zero(),
            ..RunnerConfig::default()
        },
        handlers,
    )
    .run_until_idle()
    .await;
}

#[tokio::test]
async fn decisions_are_listed_repointed_and_rejected_through_the_api() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let s = scope(&h, &alice);
    let samir = entity(&h, &s, NoteKind::Person, "Ahmed Samir", &["Ahmed"]).await;
    let fathy = entity(&h, &s, NoteKind::Person, "Ahmed Fathy", &["Ahmed"]).await;
    let text = "Ahmed called about the invoice.";
    let call = note(&h, &s, "notes/Call.md", text).await;
    let people = vec![
        ent(fathy, "person", "Ahmed Fathy", &["Ahmed"]),
        ent(samir, "person", "Ahmed Samir", &["Ahmed"]),
    ];
    push(&h, &input(call, "Call", text, people.clone()), "Ahmed", Some(samir), false, text);
    run_link(&h, &alice, call).await;
    assert!(h.read(alice.id, "notes/Call.md").contains("people: [\"[[Ahmed Samir]]\"]"));

    let list = ops::list_ai_decisions(&alice.client, None)
        .await
        .expect("list");
    assert_eq!(list.items.len(), 1);
    let d = list.items[0].clone();
    let commit = d.commit.clone().expect("applied by a commit");
    assert_eq!(
        d,
        types::AiDecision {
            commit: Some(commit.clone()),
            confidence: Some(0.9),
            created: "2026-09-27T12:00:00Z".parse().expect("t"),
            id: d.id,
            kind: "entity_mention".into(),
            mention: Some("Ahmed".into()),
            reverted_at: None,
            source_block_id: Some(generated_block_id(text)),
            source_note_id: Some(call.as_ulid()),
            source_title: Some("Call".into()),
            suggestion_id: None,
            suggestion_status: None,
            summary: "\"Ahmed\" → Ahmed Samir".into(),
            target_id: samir.to_string(),
            target_name: Some("Ahmed Samir".into()),
            target_type: "entity".into(),
            type_: Some("people".into()),
        }
    );
    // Another user sees nothing and cannot touch it.
    assert_eq!(
        ops::list_ai_decisions(&bob.client, Some(10))
            .await
            .expect("bob list")
            .items,
        vec![]
    );
    let foreign = ops::repoint_ai_decision(
        &bob.client,
        d.id,
        &types::RepointRequest {
            hint: None,
            target_id: fathy.as_ulid(),
        },
    )
    .await
    .expect_err("foreign");
    assert_eq!(
        problem(&foreign),
        plain("not_found", "Not found", 404, None)
    );

    // Repoint: Samir out (rejected), Fathy in, a hint on Fathy; one `user: repoint` commit.
    let r = ops::repoint_ai_decision(
        &alice.client,
        d.id,
        &types::RepointRequest {
            hint: None,
            target_id: fathy.as_ulid(),
        },
    )
    .await
    .expect("repoint");
    assert_eq!(
        (r.decision.reverted_at, r.decision.id),
        (Some("2026-09-27T12:00:00Z".parse().expect("t")), d.id)
    );
    assert_eq!(h.log(alice.id)[0], "user: repoint notes/Call.md");
    assert_eq!(r.commit.map(|c| c.len()), Some(40));
    let text_now = h.read(alice.id, "notes/Call.md");
    assert!(text_now.contains("people: [\"[[Ahmed Fathy]]\"]"), "{text_now}");
    let sidecar: serde_json::Value = serde_json::from_str(&h.read(
        alice.id,
        &format!(".meta/notes/{fathy}.json"),
    ))
    .expect("json");
    assert_eq!(
        sidecar["hints"][0]["text"],
        json!("\"Ahmed\" in \"Call\" = Ahmed Fathy")
    );
    // Correcting it twice is refused.
    let again = ops::reject_ai_decision(&alice.client, d.id)
        .await
        .expect_err("twice");
    assert_eq!(
        problem(&again).detail.as_deref(),
        Some("the decision was already corrected")
    );

    // Reject: the next note's AI link is removed and recorded as rejected.
    let text2 = "Ahmed sent the invoice.";
    let second = note(&h, &s, "notes/Invoice.md", text2).await;
    // Fathy now carries the hint the repoint stored.
    let mut hinted = people;
    hinted[0].hints = vec!["\"Ahmed\" in \"Call\" = Ahmed Fathy".into()];
    let mut i2 = input(second, "Invoice", text2, hinted);
    i2.candidates = vec![strata_jobs::pipeline::CandidateInput {
        id: call.to_string(),
        title: "Call".into(),
        kind: "note".into(),
        summary: None,
    }];
    push(&h, &i2, "Ahmed", Some(samir), false, text2);
    run_link(&h, &alice, second).await;
    let d2 = ops::list_ai_decisions(&alice.client, Some(1))
        .await
        .expect("list")
        .items[0]
        .clone();
    let rejected = ops::reject_ai_decision(&alice.client, d2.id)
        .await
        .expect("reject");
    assert!(rejected.decision.reverted_at.is_some());
    assert_eq!(h.log(alice.id)[0], "user: reject notes/Invoice.md");
    assert!(!h.read(alice.id, "notes/Invoice.md").contains("people"));
    h.finish().await;
}

#[tokio::test]
async fn suggestion_payloads_are_typed_and_accepting_with_edits_creates_the_entity() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let s = scope(&h, &alice);
    let text = "بابا called about the apartment.";
    let n = note(&h, &s, "notes/Apartment.md", text).await;
    push(&h, &input(n, "Apartment", text, vec![]), "بابا", None, true, text);
    run_link(&h, &alice, n).await;
    let list = ops::list_suggestions(&alice.client, None)
        .await
        .expect("suggestions");
    assert_eq!(list.items.len(), 1);
    let sug = &list.items[0];
    let types::SuggestionPayload::EntityLink {
        block_id,
        candidates,
        confidence,
        entity_kind,
        is_nickname,
        mention,
        proposed,
        reason,
        source_note,
        ..
    } = &sug.payload
    else {
        panic!("an entity_link payload: {:?}", sug.payload);
    };
    assert_eq!(
        (
            block_id.clone(),
            candidates.clone(),
            *confidence,
            entity_kind.as_str(),
            *is_nickname,
            mention.as_str(),
            *proposed,
            reason.as_str(),
            *source_note
        ),
        (
            Some(generated_block_id(text)),
            vec![],
            0.9,
            "person",
            true,
            "بابا",
            None,
            "nickname",
            n.as_ulid()
        )
    );
    let accepted = ops::accept_suggestion_with_edits(
        &alice.client,
        sug.id,
        &types::AcceptWithEditsRequest {
            edits: types::SuggestionEditsDto {
                aliases: Some(vec!["Baba".into()]),
                title: Some("Ibrahim Nasr".into()),
                ..types::SuggestionEditsDto::default()
            },
        },
    )
    .await
    .expect("accept");
    assert_eq!(accepted.status, types::SuggestionStatus::Accepted);
    assert_eq!(h.log(alice.id)[0], "user: accept entity_link notes/Apartment.md");
    let person = h.read(alice.id, "people/Ibrahim Nasr.md");
    assert!(person.contains("kind: person"), "{person}");
    assert!(person.contains("aliases: [بابا, Baba]"), "{person}");
    assert!(
        h.read(alice.id, "notes/Apartment.md")
            .contains("people: [\"[[Ibrahim Nasr]]\"]")
    );
    // Edits on an already decided suggestion are refused.
    let err = ops::accept_suggestion_with_edits(
        &alice.client,
        sug.id,
        &types::AcceptWithEditsRequest {
            edits: types::SuggestionEditsDto::default(),
        },
    )
    .await
    .expect_err("decided");
    assert_eq!(problem(&err).status, 422);
    h.finish().await;
}

#[tokio::test]
async fn refresh_relink_and_the_auto_file_setting() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let s = scope(&h, &alice);
    let shady = entity(&h, &s, NoteKind::Person, "Shady", &[]).await;
    let n = note(&h, &s, "notes/Plain.md", "Plain.").await;
    let queued = ops::refresh_entity(&alice.client, shady.as_ulid())
        .await
        .expect("refresh");
    let relinked = ops::relink_note(&alice.client, n.as_ulid())
        .await
        .expect("relink");
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    let jobs: Vec<(strata_common::JobId, String, Option<NoteId>, Vec<u8>)> = sqlx::query_as(
        "SELECT id, kind, note_id, payload FROM jobs WHERE kind IN ('entity_insights', 'link') \
         ORDER BY kind",
    )
    .fetch_all(tx.conn())
    .await
    .expect("jobs");
    tx.commit().await.expect("commit");
    let force = rmp_serde::to_vec_named(&strata_jobs::link::LinkParams { force: true }).expect("p");
    assert_eq!(
        jobs,
        vec![
            (
                strata_common::JobId::from_ulid(queued.job_id),
                "entity_insights".into(),
                Some(shady),
                force.clone()
            ),
            (
                strata_common::JobId::from_ulid(relinked.job_id),
                "link".into(),
                Some(n),
                force
            ),
        ]
    );
    // A plain note is not an entity; foreign IDs are unknown.
    assert_eq!(
        problem(
            &ops::refresh_entity(&alice.client, n.as_ulid())
                .await
                .expect_err("not an entity")
        )
        .status,
        404
    );
    assert_eq!(
        problem(
            &ops::refresh_entity(&bob.client, shady.as_ulid())
                .await
                .expect_err("foreign")
        )
        .status,
        404
    );
    assert_eq!(
        problem(
            &ops::relink_note(&bob.client, n.as_ulid())
                .await
                .expect_err("foreign")
        )
        .status,
        404
    );
    // Auto-file: off by default, per user.
    assert_eq!(
        ops::get_ai_settings(&alice.client).await.expect("get"),
        types::AiSettings { auto_file: false }
    );
    assert_eq!(
        ops::put_ai_settings(&alice.client, &types::AiSettings { auto_file: true })
            .await
            .expect("put"),
        types::AiSettings { auto_file: true }
    );
    assert_eq!(
        ops::get_ai_settings(&bob.client).await.expect("bob"),
        types::AiSettings { auto_file: false }
    );
    h.finish().await;
}
