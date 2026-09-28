//! Op envelopes, results, change records and cursors: shapes, round trips and validation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)] // test helpers outside #[test] fns; one sample per op kind

use std::collections::BTreeMap;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use dedupe::{DedupeKind, DuplicateCandidate, KeepBoth, MatchLevel};
use domain::{CopyKind, CustodyEventType, NoteKind, Priority, RelationOrigin};
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_model::changes::{NoteRecord, RelationRecord, SettingRecord};
use sync_model::ops::*;
use sync_model::results::PushMismatch;
use sync_model::{
    BootstrapPage, ChangeRecord, ChangesPage, ConflictResolution, CursorError, EntityType, Op,
    OpError, OpKind, OpOutcome, OpResult, Problem, PushRequest, PushResponse, Record, SyncCursor,
    SyncOp, Version,
};
use ulid::Ulid;
use vault_format::RelationKey;

fn u(n: u128) -> Ulid {
    Ulid::from(n)
}

fn rk(s: &str) -> RelationKey {
    s.parse().unwrap()
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

/// The device's creation time of the sample creates.
fn t() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00")
        .unwrap()
        .to_utc()
}

fn v(s: &str) -> Version {
    Version::of_text(s)
}

fn samples() -> Vec<SyncOp> {
    let base = Some(v("base"));
    let ops: Vec<(Op, Option<Version>)> = vec![
        (
            Op::NoteCreate(NoteCreate {
                id: u(1),
                path: "notes/A.md".into(),
                content: "# A\n".into(),
                created: t(),
                force: true,
            }),
            None,
        ),
        (
            Op::NoteUpdate(NoteUpdate {
                id: u(1),
                content: "# A2\n".into(),
            }),
            base.clone(),
        ),
        (
            Op::NoteMove(NoteMove {
                id: u(1),
                new_path: "notes/B.md".into(),
            }),
            base.clone(),
        ),
        (Op::NoteDelete(NoteRef { id: u(1) }), base.clone()),
        (
            Op::Capture(Capture {
                id: u(2),
                text: "remind me to make Watanya's invoice".into(),
                created: t(),
            }),
            None,
        ),
        (
            Op::RelationAdd(RelationRef {
                src_id: u(1),
                dst_id: u(3),
                relation: rk("contradicts"),
            }),
            None,
        ),
        (
            Op::RelationRemove(RelationRef {
                src_id: u(1),
                dst_id: u(3),
                relation: rk("related"),
            }),
            None,
        ),
        (
            Op::RelationRetype(RelationRetype {
                src_id: u(1),
                dst_id: u(3),
                relation: rk("related"),
                new_type: rk("supports"),
            }),
            None,
        ),
        (
            Op::SuggestionAccept(SuggestionAccept {
                id: u(4),
                edits: Some(SuggestionEdits {
                    title: Some("Pricing".into()),
                    aliases: Some(vec!["أحمد".into()]),
                    ..SuggestionEdits::default()
                }),
                created: t(),
            }),
            None,
        ),
        (
            Op::SuggestionReject(SuggestionReject {
                id: u(4),
                reason: None,
            }),
            None,
        ),
        (
            Op::SuggestionReply(SuggestionReply {
                id: u(4),
                reply_id: u(5),
                text: "no, the Petrol Arrows one".into(),
            }),
            None,
        ),
        (
            Op::EntityCreate(EntityCreate {
                id: u(6),
                kind: NoteKind::Person,
                name: "Ahmed Samir".into(),
                aliases: vec!["أحمد سمير".into()],
                fields: BTreeMap::from([("role".to_owned(), "Ops".to_owned())]),
                created: t(),
                force: false,
            }),
            None,
        ),
        (
            Op::EntityPatch(EntityPatch {
                id: u(6),
                add_aliases: vec!["A. Samir".into()],
                ..EntityPatch::default()
            }),
            base.clone(),
        ),
        (
            Op::EntityMerge(EntityMerge {
                id: u(7),
                into_id: u(6),
            }),
            None,
        ),
        (
            Op::DocumentCreate(DocumentCreate {
                id: u(8),
                name: "Watanya contract".into(),
                aliases: vec![],
                doc_type: Some("contract".into()),
                copy: Some(CopyKind::CertifiedCopy),
                copy_of: None,
                companies: vec![u(9)],
                people: vec![],
                expires: Some(d("2027-03-31")),
                created: t(),
                force: false,
            }),
            None,
        ),
        (
            Op::DocumentPatch(EntityPatch {
                id: u(8),
                unset: vec!["expires".into()],
                ..EntityPatch::default()
            }),
            base.clone(),
        ),
        (
            Op::DocumentCustody(DocumentCustody {
                document_id: u(8),
                event: CustodyEventType::HandedTo,
                at: d("2026-09-20"),
                place_id: None,
                person_id: Some(u(10)),
                counterparty_id: None,
            }),
            None,
        ),
        (
            Op::PlaceCreate(PlaceCreate {
                id: u(11),
                name: "Safe".into(),
                aliases: vec![],
                parent_id: Some(u(12)),
                address: None,
                created: t(),
                force: true,
            }),
            None,
        ),
        (
            Op::PlacePatch(EntityPatch {
                id: u(11),
                ..EntityPatch::default()
            }),
            base.clone(),
        ),
        (
            Op::TaskCreate(TaskCreate {
                id: "t-01j9a2".into(),
                note_id: None,
                text: "Make Watanya's ETA invoice".into(),
                due: Some(d("2026-10-01")),
                scheduled: None,
                start: None,
                recurrence: Some("every month on the 1st".into()),
                reminders: vec![d("2026-10-01").and_hms_opt(9, 0, 0).unwrap()],
                priority: Some(Priority::High),
                created: t(),
                home_id: Some(u(14)),
                force: false,
            }),
            None,
        ),
        (
            Op::TaskUpdate(TaskUpdate {
                id: "t-01j9a2".into(),
                due: Some(None),
                priority: Some(Some(Priority::Low)),
                ..TaskUpdate::default()
            }),
            base.clone(),
        ),
        (
            Op::TaskComplete(TaskComplete {
                id: "t-01j9a2".into(),
                done: d("2026-10-01"),
                next_id: Some("t-01j9b0".into()),
            }),
            base.clone(),
        ),
        (
            Op::TaskCancel(TaskCancel {
                id: "t-01j9a2".into(),
                date: d("2026-10-02"),
            }),
            base.clone(),
        ),
        (
            Op::TaskReopen(TaskRef {
                id: "t-01j9a2".into(),
            }),
            base.clone(),
        ),
        (
            Op::TaskDelete(TaskRef {
                id: "t-01j9a2".into(),
            }),
            base.clone(),
        ),
        (Op::RelinkRequest(NoteRef { id: u(1) }), None),
        (
            Op::DeviceSettings(DeviceSettings {
                device_id: u(13),
                reminders_enabled: Some(false),
            }),
            None,
        ),
    ];
    ops.into_iter()
        .enumerate()
        .map(|(i, (op, base))| SyncOp::new(u(1000 + i as u128), base, op))
        .collect()
}

#[test]
fn every_kind_has_a_sample_that_round_trips_and_validates() {
    let ops = samples();
    let kinds: Vec<OpKind> = ops.iter().map(|o| o.op.kind()).collect();
    assert_eq!(kinds, OpKind::ALL);
    for op in &ops {
        assert_eq!(op.validate(), Ok(()), "{}", op.op.kind());
        let bytes = rmp_serde::to_vec_named(op).unwrap();
        let back: SyncOp = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(&back, op);
        // Outbox columns.
        let kind: OpKind = op.op.kind().as_str().parse().unwrap();
        assert_eq!(
            Op::from_parts(kind, &op.op.payload_bytes().unwrap()).unwrap(),
            op.op
        );
        assert_eq!(
            op.op.force(),
            matches!(&op.op, Op::NoteCreate(_) | Op::PlaceCreate(_))
        );
        // Every op that creates a note carries the device's creation time.
        let creates = matches!(
            &op.op,
            Op::NoteCreate(_)
                | Op::Capture(_)
                | Op::EntityCreate(_)
                | Op::DocumentCreate(_)
                | Op::PlaceCreate(_)
                | Op::TaskCreate(_)
                | Op::SuggestionAccept(_)
        );
        assert_eq!(
            op.op.created(),
            creates.then(t).as_ref(),
            "{}",
            op.op.kind()
        );
    }
    let request = PushRequest { ops };
    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn envelope_shape_is_stable() {
    let op = SyncOp::new(
        u(1),
        Some(v("")),
        Op::TaskUpdate(TaskUpdate {
            id: "t-1".into(),
            due: Some(None),
            scheduled: Some(Some(d("2026-10-01"))),
            ..TaskUpdate::default()
        }),
    );
    assert_eq!(
        serde_json::to_value(&op).unwrap(),
        json!({
            "op_id": "00000000000000000000000001",
            "entity_id": "t-1",
            "base_version": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "op": {"kind": "task.update", "payload": {"id": "t-1", "due": null, "scheduled": "2026-10-01"}}
        })
    );
    // Absent, null and set survive MessagePack distinctly.
    let bytes = rmp_serde::to_vec_named(&op).unwrap();
    let back: SyncOp = rmp_serde::from_slice(&bytes).unwrap();
    let Op::TaskUpdate(t) = back.op else {
        panic!("kind changed")
    };
    assert_eq!(
        (t.due, t.scheduled, t.start),
        (Some(None), Some(Some(d("2026-10-01"))), None)
    );

    let rel = SyncOp::new(
        u(2),
        None,
        Op::RelationAdd(RelationRef {
            src_id: u(3),
            dst_id: u(4),
            relation: rk("part-of"),
        }),
    );
    assert_eq!(
        serde_json::to_value(&rel).unwrap()["op"],
        json!({"kind": "relation.add", "payload": {"src_id": "00000000000000000000000003", "dst_id": "00000000000000000000000004", "type": "part-of"}})
    );
}

/// The device's creation time is UTC on the wire, required, and a time sent with another
/// offset is read as the same instant in UTC.
#[test]
fn creation_times_are_required_and_utc() {
    let capture = Op::Capture(Capture {
        id: u(2),
        text: "x".into(),
        created: t(),
    });
    assert_eq!(
        serde_json::to_value(&capture).unwrap()["payload"],
        json!({"id": "00000000000000000000000002", "text": "x", "created": "2026-09-27T11:32:00Z"})
    );
    let payload = json!({"id": "00000000000000000000000002", "text": "x", "created": "2026-09-27T14:32:00+03:00"});
    let bytes = rmp_serde::to_vec_named(&payload).unwrap();
    assert_eq!(Op::from_parts(OpKind::Capture, &bytes), Ok(capture));
    let without =
        json!({"id": "00000000000000000000000001", "path": "notes/A.md", "content": "# A\n"});
    let err = Op::from_parts(
        OpKind::NoteCreate,
        &rmp_serde::to_vec_named(&without).unwrap(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, OpError::InvalidPayload { kind: OpKind::NoteCreate, reason } if reason.contains("created")),
        "{err:?}"
    );
    // `home_id` of a task create is optional.
    let task = json!({
        "id": "t-01j9a2", "note_id": null, "text": "a", "due": null, "scheduled": null,
        "start": null, "recurrence": null, "priority": null, "created": "2026-09-27T11:32:00Z"
    });
    let Op::TaskCreate(back) =
        Op::from_parts(OpKind::TaskCreate, &rmp_serde::to_vec_named(&task).unwrap()).unwrap()
    else {
        panic!("kind changed")
    };
    assert_eq!((back.created, back.home_id), (t(), None));
}

#[test]
fn validation_errors() {
    let mut op = SyncOp::new(
        u(1),
        None,
        Op::NoteUpdate(NoteUpdate {
            id: u(2),
            content: String::new(),
        }),
    );
    assert_eq!(
        op.validate(),
        Err(OpError::MissingBaseVersion(OpKind::NoteUpdate))
    );
    op.base_version = Some(v("x"));
    op.entity_id = "other".into();
    assert_eq!(
        op.validate(),
        Err(OpError::EntityIdMismatch {
            expected: u(2).to_string(),
            got: "other".into()
        })
    );
    let cap = SyncOp::new(u(1), Some(v("x")), Op::RelinkRequest(NoteRef { id: u(2) }));
    assert_eq!(
        cap.validate(),
        Err(OpError::UnexpectedBaseVersion(OpKind::RelinkRequest))
    );
    assert_eq!(
        "note.frobnicate".parse::<OpKind>(),
        Err(OpError::UnknownKind("note.frobnicate".into()))
    );
    let err = Op::from_parts(OpKind::NoteMove, &[0xc0]).unwrap_err();
    assert!(matches!(
        err,
        OpError::InvalidPayload {
            kind: OpKind::NoteMove,
            ..
        }
    ));
    let request = PushRequest {
        ops: vec![samples().remove(0), cap],
    };
    assert_eq!(
        request.validate(),
        Err((1, OpError::UnexpectedBaseVersion(OpKind::RelinkRequest)))
    );
}

#[test]
fn kind_flags() {
    let with_base: Vec<&str> = OpKind::ALL
        .iter()
        .filter(|k| k.requires_base_version())
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        with_base,
        [
            "note.update",
            "note.move",
            "note.delete",
            "entity.patch",
            "document.patch",
            "place.patch",
            "task.update",
            "task.complete",
            "task.cancel",
            "task.reopen",
            "task.delete"
        ]
    );
    let with_force: Vec<&str> = OpKind::ALL
        .iter()
        .filter(|k| k.accepts_force())
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        with_force,
        [
            "note.create",
            "entity.create",
            "document.create",
            "place.create",
            "task.create"
        ]
    );
}

#[test]
fn result_shapes() {
    let results = PushResponse {
        results: vec![
            OpOutcome {
                op_id: u(1),
                result: OpResult::Applied {
                    new_version: Some(v("a")),
                    merged: true,
                },
            },
            OpOutcome {
                op_id: u(2),
                result: OpResult::Conflict {
                    server_version: Some(v("b")),
                    resolution: ConflictResolution::ConflictCopy {
                        note_id: u(9),
                        path: "notes/A (conflict).md".into(),
                        version: v("c"),
                    },
                },
            },
            OpOutcome {
                op_id: u(3),
                result: OpResult::Duplicate {
                    candidates: vec![DuplicateCandidate {
                        id: "t-01j9a2".into(),
                        kind: DedupeKind::Task,
                        title: "Make Watanya's ETA invoice".into(),
                        snippet: None,
                        level: MatchLevel::Near,
                        score: 0.5,
                    }],
                },
            },
            OpOutcome {
                op_id: u(4),
                result: OpResult::Rejected {
                    problem: Problem {
                        problem_type: "invalid_name".into(),
                        title: "Invalid file name".into(),
                        status: 422,
                        detail: None,
                    },
                },
            },
        ],
    };
    let value = serde_json::to_value(&results).unwrap();
    assert_eq!(
        value["results"][0]["result"],
        json!({"status": "applied", "new_version": v("a").as_str(), "merged": true})
    );
    assert_eq!(
        value["results"][1]["result"]["resolution"],
        json!({"type": "conflict_copy", "note_id": u(9).to_string(), "path": "notes/A (conflict).md", "version": v("c").as_str()})
    );
    assert_eq!(value["results"][2]["result"]["status"], json!("duplicate"));
    assert_eq!(
        value["results"][3]["result"],
        json!({"status": "rejected", "problem": {"type": "invalid_name", "title": "Invalid file name", "status": 422, "detail": null}})
    );
    let bytes = rmp_serde::to_vec_named(&results).unwrap();
    assert_eq!(
        rmp_serde::from_slice::<PushResponse>(&bytes).unwrap(),
        results
    );
    // `merged` defaults to false when absent (older servers).
    let applied: OpResult =
        serde_json::from_value(json!({"status": "applied", "new_version": null})).unwrap();
    assert_eq!(
        applied,
        OpResult::Applied {
            new_version: None,
            merged: false
        }
    );
}

#[test]
fn push_response_must_answer_the_request() {
    let request = PushRequest {
        ops: samples().into_iter().take(2).collect(),
    };
    let ok = |id| OpOutcome {
        op_id: id,
        result: OpResult::Applied {
            new_version: None,
            merged: false,
        },
    };
    let good = PushResponse {
        results: request.ops.iter().map(|o| ok(o.op_id)).collect(),
    };
    assert_eq!(good.check_answers(&request), Ok(()));
    let short = PushResponse {
        results: vec![ok(request.ops[0].op_id)],
    };
    assert_eq!(
        short.check_answers(&request),
        Err(PushMismatch::Count { ops: 2, results: 1 })
    );
    let swapped = PushResponse {
        results: vec![ok(request.ops[1].op_id), ok(request.ops[0].op_id)],
    };
    assert_eq!(
        swapped.check_answers(&request),
        Err(PushMismatch::OpId {
            index: 0,
            expected: request.ops[0].op_id,
            got: request.ops[1].op_id
        })
    );
}

fn note_record(content: &str) -> Record {
    Record::Note(NoteRecord {
        id: u(1),
        path: "notes/A.md".into(),
        content: content.into(),
        version: v(content),
        kind: NoteKind::Note,
        summary: None,
    })
}

#[test]
fn change_records_derive_headers_and_validate() {
    let c = ChangeRecord::upsert(5, 2, note_record("# A\n"));
    assert_eq!(
        (c.entity_type, c.entity_id.as_str(), c.version.clone()),
        (
            EntityType::Note,
            "00000000000000000000000001",
            Some(v("# A\n"))
        )
    );
    assert_eq!(c.validate(), Ok(()));
    let rel = Record::Relation(RelationRecord {
        src_id: u(1),
        dst_id: u(2),
        relation: rk("contradicts"),
        by: RelationOrigin::Ai,
        confidence: Some(0.72),
        reason: Some("caps".into()),
        created: None,
    });
    assert_eq!(
        rel.entity_id(),
        "00000000000000000000000001:contradicts:00000000000000000000000002"
    );
    let kb = Record::KeepBoth(KeepBoth::new(DedupeKind::Task, "t-2", "t-1"));
    assert_eq!(kb.entity_id(), "task:t-1:t-2");
    let mut bad = ChangeRecord::upsert(6, 2, note_record("x"));
    bad.entity_id = "other".into();
    assert!(matches!(
        bad.validate(),
        Err(sync_model::changes::RecordError::HeaderMismatch { seq: 6, .. })
    ));
    let mut bad = ChangeRecord::upsert(7, 2, note_record("x"));
    bad.version = Some(v("y"));
    assert_eq!(
        bad.validate(),
        Err(sync_model::changes::RecordError::VersionMismatch { seq: 7 })
    );
    let tomb = ChangeRecord::delete(8, 2, EntityType::Relation, &rel.entity_id());
    assert_eq!(
        serde_json::to_value(&tomb).unwrap(),
        json!({"seq": 8, "epoch": 2, "entity_type": "relation", "entity_id": rel.entity_id(), "version": null, "change": {"op": "delete"}})
    );
    let setting = ChangeRecord::upsert(
        9,
        2,
        Record::Setting(SettingRecord {
            key: "tz".into(),
            value: "Africa/Cairo".into(),
        }),
    );
    assert_eq!(
        serde_json::to_value(&setting).unwrap()["change"],
        json!({"op": "upsert", "record": {"type": "setting", "data": {"key": "tz", "value": "Africa/Cairo"}}})
    );
}

#[test]
fn cursor_advances_and_rejects_bad_pages() {
    let bootstrap = BootstrapPage {
        epoch: 3,
        seq: 10,
        records: vec![note_record("a")],
        next_cursor: None,
    };
    let cur = SyncCursor::after_bootstrap(&bootstrap);
    assert_eq!(cur, SyncCursor { epoch: 3, seq: 10 });
    let page = |epoch, seqs: &[u64], next_seq| ChangesPage {
        epoch,
        changes: seqs
            .iter()
            .map(|&s| ChangeRecord::upsert(s, epoch, note_record("a")))
            .collect(),
        next_seq,
        has_more: false,
    };
    assert_eq!(
        cur.advance(&page(3, &[11, 12], 12)),
        Ok(SyncCursor { epoch: 3, seq: 12 })
    );
    assert_eq!(cur.advance(&page(3, &[], 10)), Ok(cur));
    assert_eq!(
        cur.advance(&page(4, &[1], 1)),
        Err(CursorError::EpochChanged {
            expected: 3,
            got: 4
        })
    );
    assert_eq!(
        cur.advance(&page(4, &[], 0)),
        Err(CursorError::EpochChanged {
            expected: 3,
            got: 4
        })
    );
    assert_eq!(
        cur.advance(&page(3, &[12, 11], 12)),
        Err(CursorError::OutOfOrder { seq: 11, after: 12 })
    );
    assert_eq!(
        cur.advance(&page(3, &[10], 10)),
        Err(CursorError::OutOfOrder { seq: 10, after: 10 })
    );
    assert_eq!(
        cur.advance(&page(3, &[11], 5)),
        Err(CursorError::NextSeqBehind {
            next_seq: 5,
            last: 11
        })
    );
    let mut bad = page(3, &[11], 11);
    bad.changes[0].entity_id = "x".into();
    assert!(matches!(cur.advance(&bad), Err(CursorError::Record(_))));
}
