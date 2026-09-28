//! The inbox decisions behind the suggestion cards (PLAN §9.7–§9.8, §11 screen 3): "Who is
//! …?" link-or-create, "Which contract?", Undo and "Looks right", a capture flagged as a
//! duplicate (create anyway / discard), and replies in a suggestion's thread. Suggestions are
//! seeded on the fake server with exact payloads; every decision is asserted as the exact
//! outbox op it queues and the inbox it leaves behind.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use common::{Harness, NOW};
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::store::outbox::{self, OpStatus, OutboxOp};
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::suggestions::{
    CustodyPayload, CustodyTarget, DuplicateItem, DuplicatePayload, EntityLinkPayload,
    SuggestionPayload,
};
use strata_core::sync::model::{Op, Record, SuggestionRecord, SuggestionStatus};
use strata_core::view::build;
use strata_core::view::model::{
    CandidateItem, CreateOutcome, DuplicateChoice, InboxFilter, LinkOrCreateChoice,
    LinkOrCreateKind, SuggestionKind,
};
use sync_model::ops as sm;
use ulid::Ulid;

const CAP1: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3A";
const CAP2: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3B";
const PERSON: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3C";
const DOC_A: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3D";
const DOC_B: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3E";
const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3F";

fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("ulid")
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(NOW)
        .expect("now")
        .with_timezone(&Utc)
}

fn sid(n: u128) -> String {
    Ulid::from_parts(1_790_000_000_000, n).to_string()
}

fn suggestion(n: u128, note_id: &str, status: SuggestionStatus, p: &SuggestionPayload) -> Record {
    Record::Suggestion(SuggestionRecord {
        id: ulid(&sid(n)),
        note_id: Some(ulid(note_id)),
        kind: p.kind().into(),
        status,
        payload: p.to_bytes(),
        created: DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts"),
        replies: Vec::new(),
    })
}

fn link(n: u128, mention: &str, kind: &str) -> SuggestionPayload {
    SuggestionPayload::EntityLink(EntityLinkPayload {
        decision_id: Ulid::from_parts(1_790_000_000_001, n),
        mention: mention.into(),
        kind: kind.into(),
        source_note: ulid(CAP1),
        block_id: None,
        proposed: None,
        candidates: vec![ulid(PERSON)],
        is_nickname: false,
        confidence: 0.5,
        reason: "ambiguous".into(),
    })
}

/// A capture, a person and the given suggestions on the server; signed in and pulled.
async fn world(records: Vec<Record>) -> (Harness, std::sync::Arc<strata_core::session::Session>) {
    let h = Harness::new();
    h.server.remote_upsert(
        CAP1,
        "inbox/Capture 1.md",
        &format!("---\nid: {CAP1}\nkind: capture\n---\nلقيت بابا في المكتب مع Mona\n"),
    );
    h.server.remote_upsert(
        PERSON,
        "people/Mona Adel.md",
        &format!("---\nid: {PERSON}\nkind: person\n---\n"),
    );
    for r in records {
        h.server.remote_record(r);
    }
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    (h, s)
}

fn ops(s: &strata_core::session::Session) -> Vec<OutboxOp> {
    s.read(|c, _| outbox::all(c)).expect("outbox")
}

fn accept(id: &str, edits: Option<sm::SuggestionEdits>) -> Op {
    Op::SuggestionAccept(sm::SuggestionAccept {
        id: ulid(id),
        edits,
        created: now(),
    })
}

fn invalid(field: &str, reason: &str) -> CoreError {
    CoreError::InvalidInput {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

#[tokio::test]
async fn who_is_links_the_mention_or_creates_the_entity_with_it_as_an_alias() {
    let (_h, s) = world(vec![
        suggestion(
            1,
            CAP1,
            SuggestionStatus::Pending,
            &link(1, "Mona", "person"),
        ),
        suggestion(
            2,
            CAP1,
            SuggestionStatus::Pending,
            &link(2, "بابا", "company"),
        ),
        suggestion(3, CAP1, SuggestionStatus::Pending, &link(3, "Samir", "")),
        suggestion(
            4,
            CAP1,
            SuggestionStatus::Pending,
            &link(4, "Mona Adel", "person"),
        ),
    ])
    .await;
    let choice = |kind, entity_id: Option<&str>, name: Option<&str>, entity_kind: Option<&str>| {
        LinkOrCreateChoice {
            kind,
            entity_id: entity_id.map(str::to_owned),
            name: name.map(str::to_owned),
            entity_kind: entity_kind.map(str::to_owned),
            force: false,
        }
    };

    // Refusals queue nothing.
    assert_eq!(
        s.resolve_link_or_create(&sid(1), &choice(LinkOrCreateKind::Link, None, None, None)),
        Err(invalid("entity_id", "missing"))
    );
    assert_eq!(
        s.resolve_link_or_create(
            &sid(1),
            &choice(LinkOrCreateKind::Create, None, Some("  "), None)
        ),
        Err(invalid("name", "empty"))
    );
    assert_eq!(
        s.resolve_link_or_create(
            &sid(1),
            &choice(LinkOrCreateKind::Create, None, Some("Mona"), Some("place"))
        ),
        Err(invalid("entity_kind", "unknown"))
    );
    assert_eq!(
        s.resolve_link_or_create(
            &sid(99),
            &choice(LinkOrCreateKind::Link, Some(PERSON), None, None)
        ),
        Err(CoreError::NotFound {
            what: "suggestion".to_owned()
        })
    );
    assert_eq!(
        s.resolve_link_or_create(
            &sid(1),
            &choice(LinkOrCreateKind::Link, Some("not-a-ulid"), None, None)
        ),
        Err(invalid("entity_id", "not_a_ulid"))
    );
    assert_eq!(ops(&s), []);

    // Link: the suggestion is accepted pointing at the entity, the mention becoming an alias.
    assert_eq!(
        s.resolve_link_or_create(
            &sid(1),
            &choice(LinkOrCreateKind::Link, Some(PERSON), None, None)
        ),
        Ok(CreateOutcome {
            id: Some(PERSON.to_owned()),
            candidates: Vec::new(),
        })
    );
    let queued = ops(&s);
    assert_eq!(queued.len(), 1);
    assert_eq!(
        queued[0].op,
        accept(
            &sid(1),
            Some(sm::SuggestionEdits {
                target_id: Some(ulid(PERSON)),
                aliases: Some(vec!["Mona".to_owned()]),
                ..sm::SuggestionEdits::default()
            })
        )
    );
    assert_eq!(
        (queued[0].local_entity.as_str(), queued[0].status),
        (format!("suggestion:{}", sid(1)).as_str(), OpStatus::Pending)
    );

    // Create: the payload's kind (company) is used; the mention becomes the new entity's alias.
    let created = s
        .resolve_link_or_create(
            &sid(2),
            &choice(LinkOrCreateKind::Create, None, Some(" Baba Trading "), None),
        )
        .expect("create");
    let new_id = created.id.clone().expect("created");
    assert_eq!(created.candidates, []);
    let queued = ops(&s);
    assert_eq!(queued.len(), 3);
    assert_eq!(
        queued[1].op,
        Op::EntityCreate(sm::EntityCreate {
            id: ulid(&new_id),
            kind: domain::NoteKind::Company,
            name: "Baba Trading".to_owned(),
            aliases: vec!["بابا".to_owned()],
            fields: BTreeMap::new(),
            created: now(),
            force: false,
        })
    );
    assert_eq!(
        queued[2].op,
        accept(
            &sid(2),
            Some(sm::SuggestionEdits {
                target_id: Some(ulid(&new_id)),
                aliases: Some(vec!["بابا".to_owned()]),
                ..sm::SuggestionEdits::default()
            })
        )
    );

    // No kind in the payload and none chosen: a person. A name equal to the mention adds no
    // alias to the new entity.
    let created = s
        .resolve_link_or_create(
            &sid(3),
            &choice(LinkOrCreateKind::Create, None, Some("Samir"), None),
        )
        .expect("create");
    let samir = created.id.expect("created");
    let queued = ops(&s);
    assert_eq!(
        queued[3].op,
        Op::EntityCreate(sm::EntityCreate {
            id: ulid(&samir),
            kind: domain::NoteKind::Person,
            name: "Samir".to_owned(),
            aliases: Vec::new(),
            fields: BTreeMap::new(),
            created: now(),
            force: false,
        })
    );

    // A create that looks like an existing entity returns the look-alikes and queues nothing
    // until repeated with `force`.
    let before = ops(&s).len();
    let outcome = s
        .resolve_link_or_create(
            &sid(4),
            &choice(LinkOrCreateKind::Create, None, Some("Mona Adel"), None),
        )
        .expect("look-alikes");
    assert_eq!(
        outcome,
        CreateOutcome {
            id: None,
            candidates: vec![CandidateItem {
                id: PERSON.to_owned(),
                kind: "person".to_owned(),
                title: "Mona Adel".to_owned(),
                snippet: None,
                match_level: "exact".to_owned(),
                score: 1.0,
                path: Some("people/Mona Adel.md".to_owned()),
                reason: "Same title".to_owned(),
            }],
        }
    );
    assert_eq!(ops(&s).len(), before);
    let forced = s
        .resolve_link_or_create(
            &sid(4),
            &LinkOrCreateChoice {
                force: true,
                ..choice(LinkOrCreateKind::Create, None, Some("Mona Adel"), None)
            },
        )
        .expect("forced");
    let queued = ops(&s);
    assert_eq!(queued.len(), before + 2);
    assert!(
        matches!(&queued[before].op, Op::EntityCreate(e) if e.force && e.aliases.is_empty()
            && Some(e.id.to_string()) == forced.id),
        "{:?}",
        queued[before].op
    );
}

#[tokio::test]
async fn which_contract_accepts_only_one_of_the_offered_documents() {
    let custody = SuggestionPayload::Custody(CustodyPayload {
        decision_id: Ulid::from_parts(1_790_000_000_001, 9),
        source_note: ulid(CAP1),
        block_id: None,
        event: "handed-to".into(),
        date: NaiveDate::from_ymd_opt(2026, 9, 20).expect("date"),
        document: CustodyTarget {
            mention: "the contract".into(),
            id: None,
            candidates: vec![ulid(DOC_A), ulid(DOC_B)],
        },
        place: None,
        place_part_of: None,
        person: Some(CustodyTarget {
            mention: "Mona".into(),
            id: Some(ulid(PERSON)),
            candidates: Vec::new(),
        }),
        counterparty: None,
        confidence: 0.4,
        reason: "ambiguous_document".into(),
        quote: "Gave Mona the contract.".into(),
    });
    let (h, s) = world(vec![suggestion(
        1,
        CAP1,
        SuggestionStatus::Pending,
        &custody,
    )])
    .await;
    for (id, title) in [(DOC_A, "Lease 2025"), (DOC_B, "Lease 2026")] {
        h.server.remote_upsert(
            id,
            &format!("documents/{title}.md"),
            &format!("---\nid: {id}\nkind: document\n---\n"),
        );
    }
    s.sync(Trigger::Manual).await.expect("pull");
    let item = s
        .read(|c, ctx| build::suggestion_items(c, ctx, true))
        .expect("items")
        .into_iter()
        .next()
        .expect("one");
    assert_eq!(
        (
            item.detail.kind,
            item.needs_you,
            item.can_accept,
            item.detail
                .document_choices
                .iter()
                .map(|d| (d.id.clone(), d.title.clone()))
                .collect::<Vec<_>>()
        ),
        (
            SuggestionKind::Custody,
            true,
            false,
            vec![
                (Some(DOC_A.to_owned()), "Lease 2025".to_owned()),
                (Some(DOC_B.to_owned()), "Lease 2026".to_owned())
            ]
        )
    );

    assert_eq!(
        s.accept_suggestion_choice(&sid(1), PERSON),
        Err(invalid("document_id", "not_a_choice"))
    );
    assert_eq!(ops(&s), []);
    let op_id = s.accept_suggestion_choice(&sid(1), DOC_B).expect("choice");
    let queued = ops(&s);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].op_id, op_id);
    assert_eq!(
        queued[0].op,
        accept(
            &sid(1),
            Some(sm::SuggestionEdits {
                target_id: Some(ulid(DOC_B)),
                ..sm::SuggestionEdits::default()
            })
        )
    );
    // Decided: it leaves the "Needs you" list at once, before the push.
    let inbox = s
        .read(|c, ctx| build::inbox(c, ctx, InboxFilter::NeedsYou))
        .expect("inbox");
    assert_eq!(inbox.needs_you_count, 0);
}

#[tokio::test]
async fn undo_rejects_a_pending_suggestion_and_looks_right_is_idempotent() {
    let (_h, s) = world(vec![
        suggestion(
            1,
            CAP1,
            SuggestionStatus::Pending,
            &link(1, "Mona", "person"),
        ),
        suggestion(
            2,
            CAP1,
            SuggestionStatus::Accepted,
            &link(2, "Samir", "person"),
        ),
    ])
    .await;
    let op_id = s.undo_suggestion(&sid(1)).expect("undo");
    let queued = ops(&s);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].op_id, op_id);
    assert_eq!(
        queued[0].op,
        Op::SuggestionReject(sm::SuggestionReject {
            id: ulid(&sid(1)),
            reason: None,
        })
    );
    // An accepted suggestion is undone through the AI decision (not in the contract yet).
    assert_eq!(
        s.undo_suggestion(&sid(2)),
        Err(CoreError::NotAvailable {
            feature: "undo_ai_change".to_owned()
        })
    );
    assert_eq!(
        s.undo_suggestion(&sid(3)),
        Err(CoreError::NotFound {
            what: "suggestion".to_owned()
        })
    );

    // "Looks right" is local and idempotent; an unknown suggestion is refused.
    assert_eq!(s.acknowledge_suggestion(&sid(2)), Ok(()));
    assert_eq!(s.acknowledge_suggestion(&sid(2)), Ok(()));
    assert_eq!(
        s.read(|c, _| {
            Ok(c.query_row(
                "SELECT count(*) FROM acknowledged_suggestions WHERE id = ?1",
                [sid(2)],
                |r| r.get::<_, i64>(0),
            )?)
        }),
        Ok(1)
    );
    assert_eq!(
        s.acknowledge_suggestion(&sid(3)),
        Err(CoreError::NotFound {
            what: "suggestion".to_owned()
        })
    );
    assert_eq!(ops(&s).len(), 1, "acknowledging queues nothing");
}

#[tokio::test]
async fn a_duplicate_capture_is_kept_or_discarded() {
    let dup = SuggestionPayload::Duplicate(DuplicatePayload {
        candidates: vec![DuplicateItem {
            id: ulid(NOTE),
            item: NOTE.into(),
            snippet: Some("Pricing for Q4".into()),
            kind: "note".into(),
            title: "Pricing experiments".into(),
            match_level: domain::MatchLevel::Near,
            score: 0.82,
        }],
    });
    let (h, s) = world(vec![
        suggestion(1, CAP1, SuggestionStatus::Pending, &dup),
        suggestion(2, CAP2, SuggestionStatus::Pending, &dup),
        suggestion(
            3,
            CAP1,
            SuggestionStatus::Pending,
            &link(3, "Mona", "person"),
        ),
    ])
    .await;
    h.server.remote_upsert(
        CAP2,
        "inbox/Capture 2.md",
        &format!("---\nid: {CAP2}\nkind: capture\n---\nPricing for Q4 again\n"),
    );
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing experiments.md",
        &format!("---\nid: {NOTE}\n---\nPricing for Q4\n"),
    );
    s.sync(Trigger::Manual).await.expect("pull");

    let items = s
        .read(|c, ctx| build::suggestion_items(c, ctx, true))
        .expect("items");
    let d = items.iter().find(|i| i.id == sid(1)).expect("duplicate");
    assert_eq!(
        (d.detail.kind, d.needs_you, d.can_accept),
        (SuggestionKind::Duplicate, true, false)
    );
    assert_eq!(
        d.detail.duplicates,
        vec![CandidateItem {
            id: NOTE.to_owned(),
            kind: "note".to_owned(),
            title: "Pricing experiments".to_owned(),
            snippet: Some("Pricing for Q4".to_owned()),
            match_level: "near".to_owned(),
            score: 0.82,
            path: Some("notes/Pricing experiments.md".to_owned()),
            reason: "Very similar text".to_owned(),
        }]
    );

    assert_eq!(
        s.resolve_capture_duplicate(&sid(3), DuplicateChoice::Discard),
        Err(invalid("suggestion", "not_a_duplicate"))
    );
    // Create anyway: accepted (the server records "keep both"); the capture stays.
    let kept = s
        .resolve_capture_duplicate(&sid(1), DuplicateChoice::CreateAnyway)
        .expect("keep");
    // Discard: rejected, and the capture deleted.
    let discarded = s
        .resolve_capture_duplicate(&sid(2), DuplicateChoice::Discard)
        .expect("discard");
    let queued = ops(&s);
    assert_eq!(
        queued
            .iter()
            .map(|o| (o.op_id == kept || o.op_id == discarded, o.op.clone()))
            .collect::<Vec<_>>(),
        vec![
            (true, accept(&sid(1), None)),
            (
                true,
                Op::SuggestionReject(sm::SuggestionReject {
                    id: ulid(&sid(2)),
                    reason: None,
                })
            ),
            (false, Op::NoteDelete(sm::NoteRef { id: ulid(CAP2) })),
        ]
    );
    let note_exists = |id: &str| {
        s.read(|c, _| strata_core::store::notes::current(c, id))
            .expect("read")
            .is_some()
    };
    assert_eq!((note_exists(CAP1), note_exists(CAP2)), (true, false));
}

#[tokio::test]
async fn replies_join_the_thread_as_pending_until_synced() {
    let (_h, s) = world(vec![suggestion(
        1,
        CAP1,
        SuggestionStatus::Pending,
        &link(1, "Mona", "person"),
    )])
    .await;
    assert_eq!(
        s.reply_to_suggestion(&sid(1), "  \n "),
        Err(invalid("text", "empty"))
    );
    assert_eq!(
        s.reply_to_suggestion(&sid(2), "Who?"),
        Err(CoreError::NotFound {
            what: "suggestion".to_owned()
        })
    );
    let op_id = s
        .reply_to_suggestion(&sid(1), "  The one at Acme \n")
        .expect("reply");
    let queued = ops(&s);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].op_id, op_id);
    let Op::SuggestionReply(reply) = &queued[0].op else {
        panic!("not a reply: {:?}", queued[0].op);
    };
    assert_eq!(
        (reply.id.to_string(), reply.text.as_str()),
        (sid(1), "The one at Acme")
    );
    let item = s
        .read(|c, ctx| build::suggestion_items(c, ctx, true))
        .expect("items")
        .into_iter()
        .next()
        .expect("one");
    assert_eq!(
        item.thread
            .iter()
            .map(|t| (
                t.id.clone(),
                t.author.as_str(),
                t.text.as_str(),
                t.pending_sync
            ))
            .collect::<Vec<_>>(),
        [(reply.reply_id.to_string(), "user", "The one at Acme", true)]
    );
    assert!(item.pending_sync);
}

#[tokio::test]
async fn an_accept_the_server_already_decided_leaves_the_queue_with_the_rest_of_its_batch() {
    use strata_core::store::conflicts;
    use strata_core::sync::model::{ConflictResolution, OpResult};
    let (h, s) = world(vec![suggestion(
        1,
        CAP1,
        SuggestionStatus::Pending,
        &link(1, "Mona", "person"),
    )])
    .await;
    // Accepted here while another device (or the server) already decided it; a note delete
    // rides in the same push.
    let accept_op = s.accept_suggestion(&sid(1)).expect("accept");
    s.delete_note(PERSON).expect("delete");
    h.server.script(
        &sid(1),
        OpResult::Conflict {
            server_version: None,
            resolution: ConflictResolution::ServerKept {
                reason: "the suggestion was already decided".into(),
            },
        },
    );
    s.sync(Trigger::Manual).await.expect("push");

    assert_eq!(ops(&s), [], "nothing is left sending");
    assert!(
        !h.server.notes().contains_key(&ulid(PERSON)),
        "the delete in the same batch reached the server"
    );
    let rejections = s.read(|c, _| conflicts::rejections(c)).expect("rejections");
    assert_eq!(
        rejections
            .iter()
            .map(|r| (
                r.op_id.as_str(),
                r.kind.as_str(),
                r.entity_id.as_str(),
                r.problem_type.as_str(),
                r.status
            ))
            .collect::<Vec<_>>(),
        [(
            accept_op.as_str(),
            "suggestion.accept",
            sid(1).as_str(),
            "version_conflict",
            409
        )]
    );
    // The next cycle has nothing to send.
    let pushes = h.server.pushes().len();
    s.sync(Trigger::Manual).await.expect("again");
    assert_eq!(h.server.pushes().len(), pushes);
}
