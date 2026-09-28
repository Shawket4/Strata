//! Suggestion payloads: the stored bytes of every kind (field names and values), decoding by
//! kind, the tagged union, and the errors for unknown kinds and bad bytes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)] // one sample per kind

use std::str::FromStr;

use chrono::{DateTime, FixedOffset};
use chrono::{NaiveDate, NaiveDateTime};
use dedupe::MatchLevel;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_model::changes::{ReplyAuthor, SuggestionRecord, SuggestionReplyRecord, SuggestionStatus};
use sync_model::suggestions::{
    ConflictPayload, CorrectionFix, CorrectionHint, CorrectionPayload, CustodyPayload,
    CustodyTarget, DuplicateItem, DuplicatePayload, DuplicatesPayload, EntityLinkPayload,
    FilingPayload, PayloadError, SuggestionPayload, TaskPayload, kinds,
};
use ulid::Ulid;

fn u(n: u128) -> Ulid {
    Ulid::from(n)
}

fn us(n: u128) -> String {
    u(n).to_string()
}

fn item(n: u128, title: &str, level: MatchLevel) -> DuplicateItem {
    DuplicateItem {
        id: u(n),
        item: us(n),
        snippet: None,
        kind: "note".into(),
        title: title.into(),
        match_level: level,
        score: 0.5,
    }
}

fn item_json(n: u128, title: &str, level: &str) -> Value {
    json!({"id": us(n), "item": us(n), "snippet": null, "kind": "note", "title": title,
           "match_level": level, "score": 0.5})
}

/// One payload of every kind with the named map it is stored as.
fn samples() -> Vec<(SuggestionPayload, Value)> {
    let date = NaiveDate::from_str("2026-09-20").unwrap();
    let reminder = NaiveDateTime::from_str("2026-10-01T09:00:00").unwrap();
    let target = |mention: &str, id: Option<u128>, candidates: &[u128]| CustodyTarget {
        mention: mention.into(),
        id: id.map(u),
        candidates: candidates.iter().copied().map(u).collect(),
    };
    vec![
        (
            DuplicatePayload {
                candidates: vec![item(1, "Budget", MatchLevel::Exact)],
            }
            .into(),
            json!({"candidates": [item_json(1, "Budget", "exact")]}),
        ),
        (
            DuplicatesPayload {
                a: item(1, "Shadi", MatchLevel::Near),
                b: item(2, "Shady", MatchLevel::Semantic),
                reason: Some("Same person.".into()),
            }
            .into(),
            json!({"a": item_json(1, "Shadi", "near"), "b": item_json(2, "Shady", "semantic"),
                   "reason": "Same person."}),
        ),
        (
            FilingPayload {
                decision_id: u(10),
                title: "Watanya invoice".into(),
                tags: vec!["finance".into()],
                folder: "notes/Clients".into(),
            }
            .into(),
            json!({"decision_id": us(10), "title": "Watanya invoice", "tags": ["finance"],
                   "folder": "notes/Clients"}),
        ),
        (
            EntityLinkPayload {
                decision_id: u(11),
                mention: "بابا".into(),
                kind: "person".into(),
                source_note: u(1),
                block_id: Some("a1b2".into()),
                proposed: None,
                candidates: vec![u(3), u(4)],
                is_nickname: true,
                confidence: 0.4,
                reason: "nickname".into(),
            }
            .into(),
            json!({"decision_id": us(11), "mention": "بابا", "kind": "person",
                   "source_note": us(1), "block_id": "a1b2", "proposed": null,
                   "candidates": [us(3), us(4)], "is_nickname": true, "confidence": 0.4,
                   "reason": "nickname"}),
        ),
        (
            CustodyPayload {
                decision_id: u(12),
                source_note: u(1),
                block_id: None,
                event: "handed-to".into(),
                date,
                document: target("the contract", Some(5), &[]),
                place: None,
                place_part_of: None,
                person: Some(target("Shady", None, &[6, 7])),
                counterparty: None,
                confidence: 0.6,
                reason: "ambiguous".into(),
                quote: "gave the contract to Shady".into(),
            }
            .into(),
            json!({"decision_id": us(12), "source_note": us(1), "block_id": null,
                   "event": "handed-to", "date": "2026-09-20",
                   "document": {"mention": "the contract", "id": us(5), "candidates": []},
                   "place": null, "place_part_of": null,
                   "person": {"mention": "Shady", "id": null, "candidates": [us(6), us(7)]},
                   "counterparty": null, "confidence": 0.6, "reason": "ambiguous",
                   "quote": "gave the contract to Shady"}),
        ),
        (
            TaskPayload {
                decision_id: u(13),
                source_note: u(1),
                block_id: Some("c1d2".into()),
                title: "Make Watanya's ETA invoice".into(),
                due: Some(date),
                recurrence: Some("every month".into()),
                reminders: vec![reminder],
                entities: vec![u(8)],
                confidence: 0.9,
            }
            .into(),
            json!({"decision_id": us(13), "source_note": us(1), "block_id": "c1d2",
                   "title": "Make Watanya's ETA invoice", "due": "2026-09-20",
                   "recurrence": "every month", "reminders": ["2026-10-01T09:00:00"],
                   "entities": [us(8)], "confidence": 0.9}),
        ),
        (
            CorrectionPayload {
                decision_id: u(14),
                message: "the Ahmed in the Acme call is Ahmed Fathy".into(),
                fixes: vec![CorrectionFix {
                    decision_id: u(15),
                    action: "repoint".into(),
                    new_target: Some(u(9)),
                    new_type: None,
                    confidence: 0.7,
                    reason: "The user says so.".into(),
                }],
                hints: vec![CorrectionHint {
                    entity: u(9),
                    text: "Ahmed at Acme = Ahmed Fathy".into(),
                }],
                question: None,
            }
            .into(),
            json!({"decision_id": us(14), "message": "the Ahmed in the Acme call is Ahmed Fathy",
                   "fixes": [{"decision_id": us(15), "action": "repoint", "new_target": us(9),
                              "new_type": null, "confidence": 0.7,
                              "reason": "The user says so."}],
                   "hints": [{"entity": us(9), "text": "Ahmed at Acme = Ahmed Fathy"}],
                   "question": null}),
        ),
        (
            ConflictPayload {
                op_id: u(16),
                copy_id: u(16),
                copy_path: "notes/Plan (conflict 2026-09-28 101500).md".into(),
                base_version: "sha256:aa".into(),
                server_version: "sha256:bb".into(),
                hunks: 2,
            }
            .into(),
            json!({"op_id": us(16), "copy_id": us(16),
                   "copy_path": "notes/Plan (conflict 2026-09-28 101500).md",
                   "base_version": "sha256:aa", "server_version": "sha256:bb", "hunks": 2}),
        ),
    ]
}

#[test]
fn every_kind_is_stored_as_its_struct_and_decodes_back() {
    let samples = samples();
    assert_eq!(
        samples.iter().map(|(p, _)| p.kind()).collect::<Vec<_>>(),
        kinds::ALL
    );
    for (payload, stored) in samples {
        let bytes = payload.to_bytes();
        // Named map, no tag: exactly the kind's fields.
        assert_eq!(
            rmp_serde::from_slice::<Value>(&bytes).unwrap(),
            stored,
            "{}",
            payload.kind()
        );
        assert_eq!(
            SuggestionPayload::decode(payload.kind(), &bytes).unwrap(),
            payload
        );
    }
}

#[test]
fn the_union_is_an_internally_tagged_named_map() {
    for (payload, stored) in samples() {
        let bytes = rmp_serde::to_vec_named(&payload).unwrap();
        let mut tagged = stored.clone();
        tagged
            .as_object_mut()
            .unwrap()
            .insert("type".into(), json!(payload.kind()));
        assert_eq!(rmp_serde::from_slice::<Value>(&bytes).unwrap(), tagged);
        assert_eq!(
            rmp_serde::from_slice::<SuggestionPayload>(&bytes).unwrap(),
            payload
        );
    }
}

#[test]
fn missing_optional_fields_decode_as_absent() {
    // A minimal `entity_link` and `task` (optional fields and lists left out).
    let link = rmp_serde::to_vec_named(&json!({
        "decision_id": us(1), "mention": "Ahmed", "kind": "person", "source_note": us(2),
        "is_nickname": false, "confidence": 0.5, "reason": "new"
    }))
    .unwrap();
    assert_eq!(
        SuggestionPayload::decode("entity_link", &link).unwrap(),
        SuggestionPayload::EntityLink(EntityLinkPayload {
            decision_id: u(1),
            mention: "Ahmed".into(),
            kind: "person".into(),
            source_note: u(2),
            block_id: None,
            proposed: None,
            candidates: vec![],
            is_nickname: false,
            confidence: 0.5,
            reason: "new".into(),
        })
    );
    let task = rmp_serde::to_vec_named(&json!({
        "decision_id": us(1), "source_note": us(2), "title": "Call Shady", "confidence": 0.8
    }))
    .unwrap();
    assert_eq!(
        SuggestionPayload::decode("task", &task).unwrap(),
        SuggestionPayload::Task(TaskPayload {
            decision_id: u(1),
            source_note: u(2),
            block_id: None,
            title: "Call Shady".into(),
            due: None,
            recurrence: None,
            reminders: vec![],
            entities: vec![],
            confidence: 0.8,
        })
    );
}

#[test]
fn unknown_kinds_and_bad_bytes_are_errors() {
    assert_eq!(
        SuggestionPayload::decode("relation", &[0x80]),
        Err(PayloadError::UnknownKind("relation".into()))
    );
    let Err(PayloadError::Invalid { kind, .. }) = SuggestionPayload::decode("filing", &[0xc0])
    else {
        panic!("nil is not a filing payload");
    };
    assert_eq!(kind, "filing");
    // A filing payload is not a task payload.
    let filing = samples()[2].0.to_bytes();
    assert!(matches!(
        SuggestionPayload::decode("task", &filing),
        Err(PayloadError::Invalid { kind, .. }) if kind == "task"
    ));
}

#[test]
fn records_decode_their_payload_and_carry_reply_authors() {
    let at: DateTime<FixedOffset> =
        DateTime::parse_from_rfc3339("2026-09-28T10:00:00+03:00").unwrap();
    let (payload, _) = samples().remove(2);
    let record = SuggestionRecord {
        id: u(20),
        note_id: Some(u(1)),
        kind: payload.kind().into(),
        status: SuggestionStatus::Pending,
        payload: payload.to_bytes(),
        created: at,
        replies: vec![
            SuggestionReplyRecord {
                id: u(21),
                text: "no, put it under Clients".into(),
                at,
                author: ReplyAuthor::User,
            },
            SuggestionReplyRecord {
                id: u(22),
                text: "Re-proposed: notes/Clients.".into(),
                at,
                author: ReplyAuthor::Ai,
            },
        ],
    };
    assert_eq!(record.decode_payload().unwrap(), payload);
    let bytes = rmp_serde::to_vec_named(&record.replies).unwrap();
    let value: Value = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        json!([
            {"id": us(21), "text": "no, put it under Clients", "at": "2026-09-28T10:00:00+03:00",
             "author": "user"},
            {"id": us(22), "text": "Re-proposed: notes/Clients.",
             "at": "2026-09-28T10:00:00+03:00", "author": "ai"}
        ])
    );
    // A reply from an older server (no author) is the user's.
    let old = rmp_serde::to_vec_named(
        &json!({"id": us(21), "text": "no", "at": "2026-09-28T10:00:00+03:00"}),
    )
    .unwrap();
    assert_eq!(
        rmp_serde::from_slice::<SuggestionReplyRecord>(&old).unwrap(),
        SuggestionReplyRecord {
            id: u(21),
            text: "no".into(),
            at,
            author: ReplyAuthor::User,
        }
    );
}
