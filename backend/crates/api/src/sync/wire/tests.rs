//! The contract mirrors encode exactly like the `sync-model` types the handlers use.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeZone, Utc};
use dedupe::{DuplicateCandidate, KeepBoth};
use domain::{
    CopyKind, CustodyEventType, DedupeKind, MatchLevel, NoteKind, Priority, RelationOrigin,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sync_model::changes::{
    ClusterAssignmentRecord, ClusterNameRecord, DeviceSettingRecord, NoteRecord, RejectedRecord,
    RelationRecord, SettingRecord, SuggestionRecord, SuggestionReplyRecord, SuggestionStatus,
};
use sync_model::ops::{self as o, Op};
use sync_model::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, ConflictResolution, EntityType, OpOutcome,
    OpResult, Problem, PushRequest, PushResponse, Record, SyncOp, Version,
};
use ulid::Ulid;

use super::*;

/// `model` → bytes → mirror → bytes must be identical, and the bytes decode back to `model`.
fn same_bytes<M, W>(model: &M)
where
    M: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    W: Serialize + DeserializeOwned,
{
    let bytes = rmp_serde::to_vec_named(model).expect("encode model");
    let mirror: W = rmp_serde::from_slice(&bytes).expect("decode as mirror");
    let again = rmp_serde::to_vec_named(&mirror).expect("encode mirror");
    assert_eq!(bytes, again, "{model:?}");
    let back: M = rmp_serde::from_slice(&again).expect("decode as model");
    assert_eq!(&back, model);
}

fn u(n: u128) -> Ulid {
    Ulid(n)
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("date")
}

#[allow(clippy::too_many_lines)] // one value of every op kind
fn every_op() -> Vec<Op> {
    let rel = "related".parse().expect("relation");
    let at = Utc
        .timestamp_opt(1_790_000_000, 0)
        .single()
        .expect("ts")
        .fixed_offset();
    let reminder = d(2026, 10, 1).and_hms_opt(9, 0, 0).expect("time");
    let patch = o::EntityPatch {
        id: u(9),
        set: BTreeMap::from([("role".to_owned(), "CEO".to_owned())]),
        unset: vec!["phone".into()],
        add_aliases: vec!["Sam".into()],
        remove_aliases: vec![],
    };
    vec![
        Op::NoteCreate(o::NoteCreate {
            created: strata_common::clock::default_test_epoch(),
            id: u(1),
            path: "notes/A.md".into(),
            content: "# A\n".into(),
            force: true,
        }),
        Op::NoteUpdate(o::NoteUpdate {
            id: u(1),
            content: "x".into(),
        }),
        Op::NoteMove(o::NoteMove {
            id: u(1),
            new_path: "notes/B.md".into(),
        }),
        Op::NoteDelete(o::NoteRef { id: u(1) }),
        Op::Capture(o::Capture {
            id: u(2),
            text: "call Sam".into(),
            created: at.to_utc(),
        }),
        Op::RelationAdd(o::RelationRef {
            src_id: u(1),
            dst_id: u(2),
            relation: rel,
        }),
        Op::RelationRemove(o::RelationRef {
            src_id: u(1),
            dst_id: u(2),
            relation: rel,
        }),
        Op::RelationRetype(o::RelationRetype {
            src_id: u(1),
            dst_id: u(2),
            relation: rel,
            new_type: "supports".parse().expect("relation"),
        }),
        Op::SuggestionAccept(o::SuggestionAccept {
            created: strata_common::clock::default_test_epoch(),
            id: u(3),
            edits: Some(o::SuggestionEdits {
                title: Some("T".into()),
                reminders: Some(vec![reminder]),
                due: Some(d(2026, 10, 2)),
                ..o::SuggestionEdits::default()
            }),
        }),
        Op::SuggestionAccept(o::SuggestionAccept {
            created: strata_common::clock::default_test_epoch(),
            id: u(3),
            edits: None,
        }),
        Op::SuggestionReject(o::SuggestionReject {
            id: u(3),
            reason: Some("no".into()),
        }),
        Op::SuggestionReply(o::SuggestionReply {
            id: u(3),
            reply_id: u(4),
            text: "the other one".into(),
        }),
        Op::EntityCreate(o::EntityCreate {
            created: strata_common::clock::default_test_epoch(),
            id: u(5),
            kind: NoteKind::Person,
            name: "Sam".into(),
            aliases: vec!["سام".into()],
            fields: BTreeMap::from([("role".to_owned(), "CEO".to_owned())]),
            force: false,
        }),
        Op::EntityPatch(patch.clone()),
        Op::EntityMerge(o::EntityMerge {
            id: u(5),
            into_id: u(6),
        }),
        Op::DocumentCreate(o::DocumentCreate {
            created: strata_common::clock::default_test_epoch(),
            id: u(7),
            name: "Contract".into(),
            aliases: vec![],
            doc_type: Some("contract".into()),
            copy: Some(CopyKind::CertifiedCopy),
            copy_of: Some(u(8)),
            companies: vec![u(6)],
            people: vec![],
            expires: Some(d(2027, 1, 1)),
            force: false,
        }),
        Op::DocumentPatch(patch.clone()),
        Op::DocumentCustody(o::DocumentCustody {
            document_id: u(7),
            event: CustodyEventType::HandedTo,
            at: d(2026, 9, 20),
            place_id: None,
            person_id: Some(u(5)),
            counterparty_id: None,
        }),
        Op::PlaceCreate(o::PlaceCreate {
            created: strata_common::clock::default_test_epoch(),
            id: u(10),
            name: "Safe".into(),
            aliases: vec![],
            parent_id: Some(u(11)),
            address: None,
            force: true,
        }),
        Op::PlacePatch(patch),
        Op::TaskCreate(o::TaskCreate {
            created: strata_common::clock::default_test_epoch(),
            home_id: None,
            id: "t-01".into(),
            note_id: None,
            text: "Pay".into(),
            due: Some(d(2026, 10, 1)),
            scheduled: None,
            start: None,
            recurrence: Some("every month".into()),
            reminders: vec![reminder],
            priority: Some(Priority::Normal),
            force: false,
        }),
        Op::TaskUpdate(o::TaskUpdate {
            id: "t-01".into(),
            text: Some("Pay rent".into()),
            due: Some(None),
            priority: Some(Some(Priority::High)),
            reminders: Some(vec![]),
            ..o::TaskUpdate::default()
        }),
        Op::TaskUpdate(o::TaskUpdate {
            id: "t-01".into(),
            ..o::TaskUpdate::default()
        }),
        Op::TaskComplete(o::TaskComplete {
            id: "t-01".into(),
            done: d(2026, 10, 1),
            next_id: Some("t-02".into()),
        }),
        Op::TaskCancel(o::TaskCancel {
            id: "t-01".into(),
            date: d(2026, 10, 1),
        }),
        Op::TaskReopen(o::TaskRef { id: "t-01".into() }),
        Op::TaskDelete(o::TaskRef { id: "t-01".into() }),
        Op::RelinkRequest(o::NoteRef { id: u(1) }),
        Op::DeviceSettings(o::DeviceSettings {
            device_id: u(12),
            reminders_enabled: Some(false),
        }),
    ]
}

#[test]
fn every_op_kind_encodes_like_the_mirror() {
    let ops = every_op();
    let kinds: std::collections::BTreeSet<_> = ops.iter().map(Op::kind).collect();
    assert_eq!(kinds.len(), sync_model::OpKind::ALL.len());
    let request = PushRequest {
        ops: ops
            .into_iter()
            .enumerate()
            .map(|(i, op)| {
                let base = op
                    .kind()
                    .requires_base_version()
                    .then(|| Version::of_text(&i.to_string()));
                SyncOp::new(u(100 + i as u128), base, op)
            })
            .collect(),
    };
    same_bytes::<PushRequest, SyncPushRequest>(&request);
}

#[test]
fn every_result_encodes_like_the_mirror() {
    let v = Version::of_text("x");
    let results = vec![
        OpResult::Applied {
            new_version: Some(v.clone()),
            merged: true,
        },
        OpResult::Applied {
            new_version: None,
            merged: false,
        },
        OpResult::Conflict {
            server_version: Some(v.clone()),
            resolution: ConflictResolution::ConflictCopy {
                note_id: u(1),
                path: "notes/A (conflict).md".into(),
                version: v.clone(),
            },
        },
        OpResult::Conflict {
            server_version: None,
            resolution: ConflictResolution::ServerKept {
                reason: "gone".into(),
            },
        },
        OpResult::Duplicate {
            candidates: vec![DuplicateCandidate {
                id: u(2).to_string(),
                kind: DedupeKind::Task,
                title: "Pay".into(),
                snippet: None,
                level: MatchLevel::Near,
                score: 0.75,
            }],
        },
        OpResult::Rejected {
            problem: Problem {
                problem_type: "not_found".into(),
                title: "Not found".into(),
                status: 404,
                detail: None,
            },
        },
    ];
    let response = PushResponse {
        results: results
            .into_iter()
            .enumerate()
            .map(|(i, result)| OpOutcome {
                op_id: u(i as u128),
                result,
            })
            .collect(),
    };
    same_bytes::<PushResponse, SyncPushResponse>(&response);
}

fn every_record() -> Vec<Record> {
    let at = Utc
        .timestamp_opt(1_790_000_000, 0)
        .single()
        .expect("ts")
        .fixed_offset();
    let rel = "works-at".parse().expect("relation");
    vec![
        Record::Note(NoteRecord {
            id: u(1),
            path: "people/Sam.md".into(),
            content: "---\nid: x\n---\n".into(),
            version: Version::of_text("---\nid: x\n---\n"),
            kind: NoteKind::Person,
            summary: None,
        }),
        Record::Note(NoteRecord {
            id: u(1),
            path: "a.md".into(),
            content: String::new(),
            version: Version::of_text(""),
            kind: NoteKind::Note,
            summary: Some("s".into()),
        }),
        Record::Relation(RelationRecord {
            src_id: u(1),
            dst_id: u(2),
            relation: rel,
            by: RelationOrigin::Ai,
            confidence: Some(0.5),
            reason: Some("because".into()),
            created: Some(at),
        }),
        Record::Rejected(RejectedRecord {
            src_id: u(1),
            dst_id: u(2),
            relation: rel,
            at,
        }),
        Record::Suggestion(SuggestionRecord {
            id: u(3),
            note_id: Some(u(1)),
            kind: "duplicate".into(),
            status: SuggestionStatus::Pending,
            payload: vec![0x80],
            created: at,
            replies: vec![
                SuggestionReplyRecord {
                    id: u(4),
                    text: "no".into(),
                    at,
                    author: sync_model::changes::ReplyAuthor::User,
                },
                SuggestionReplyRecord {
                    id: u(5),
                    text: "Re-proposed.".into(),
                    at,
                    author: sync_model::changes::ReplyAuthor::Ai,
                },
            ],
        }),
        Record::ClusterAssignment(ClusterAssignmentRecord {
            note_id: u(1),
            cluster_id: "7".into(),
        }),
        Record::ClusterName(ClusterNameRecord {
            cluster_id: "7".into(),
            name: "Pricing".into(),
        }),
        Record::Setting(SettingRecord {
            key: "timezone".into(),
            value: "Africa/Cairo".into(),
        }),
        Record::DeviceSetting(DeviceSettingRecord {
            device_id: u(5),
            key: "reminders_enabled".into(),
            value: "false".into(),
        }),
        Record::KeepBoth(KeepBoth::new(DedupeKind::Task, "t-1", "t-2")),
    ]
}

#[test]
fn every_record_and_page_encodes_like_the_mirror() {
    let records = every_record();
    let types: std::collections::BTreeSet<_> = records.iter().map(Record::entity_type).collect();
    assert_eq!(types.len(), EntityType::ALL.len());
    same_bytes::<BootstrapPage, SyncBootstrapPage>(&BootstrapPage {
        epoch: 1,
        seq: 300,
        records: records.clone(),
        next_cursor: Some("abc".into()),
    });
    same_bytes::<BootstrapPage, SyncBootstrapPage>(&BootstrapPage {
        epoch: 1,
        seq: 0,
        records: vec![],
        next_cursor: None,
    });
    let mut changes: Vec<ChangeRecord> = records
        .into_iter()
        .enumerate()
        .map(|(i, r)| ChangeRecord::upsert(i as u64 + 1, 2, r))
        .collect();
    changes.push(ChangeRecord::delete(
        99,
        2,
        EntityType::Relation,
        "a:related:b",
    ));
    assert!(matches!(changes[changes.len() - 1].change, Change::Delete));
    same_bytes::<ChangesPage, SyncChangesPage>(&ChangesPage {
        epoch: 2,
        changes,
        next_seq: 99,
        has_more: true,
    });
}
