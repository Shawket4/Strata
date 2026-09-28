//! Device creation times over the API (owner decision 2026-09-28 "Times, creation stamps and
//! titles"): items created offline on Monday and pushed on Wednesday keep Monday as their
//! `created`/`updated`, written in UTC; the title rule holds for a taken name; a creation time
//! more than `max_future_skew_secs` (5 minutes) ahead of the server's clock is refused with
//! `422 created_in_future`, over sync and over REST, and every response conforms to the
//! contract.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod sync_harness;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use domain::{CopyKind, NoteKind};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use sync_harness::{H, version};
use sync_model::ops::{self as o, Op};
use sync_model::{OpResult, Problem, SyncOp};
use ulid::Ulid;

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).expect("instant").to_utc()
}

fn id(n: u128) -> Ulid {
    Ulid::from_parts(1_790_000_000_000, n)
}

fn op(n: u128, op: Op) -> SyncOp {
    SyncOp::new(Ulid::from_parts(1_790_000_000_000, 0x1000 + n), None, op)
}

/// Monday 08:15:30.4 UTC, when the device made the items (offline).
fn monday() -> DateTime<Utc> {
    at("2026-09-28T08:15:30.400Z")
}

const MONDAY: &str = "created: 2026-09-28T08:15:30Z\nupdated: 2026-09-28T08:15:30Z\n";

fn applied(text: &str) -> OpResult {
    OpResult::Applied {
        new_version: Some(version(text)),
        merged: false,
    }
}

fn in_future(max: u32) -> Problem {
    Problem {
        problem_type: "created_in_future".into(),
        title: "Creation time is in the future".into(),
        status: 422,
        detail: Some(format!(
            "`created` is more than {max} seconds ahead of the server's clock; check the device's clock"
        )),
    }
}

#[tokio::test]
async fn offline_creates_pushed_on_wednesday_keep_monday() {
    let h = H::new().await;
    // Wednesday on the server; the device kept its ops since Monday.
    h.clock.set(at("2026-09-30T09:00:00Z"));
    let alice = h.user("alice").await;
    let uid = alice.id;
    let entity = |n: u128| {
        Op::EntityCreate(o::EntityCreate {
            id: id(n),
            kind: NoteKind::Person,
            name: "Ahmed".into(),
            aliases: Vec::new(),
            fields: BTreeMap::new(),
            created: monday(),
            force: true,
        })
    };
    let ops = vec![
        op(
            1,
            Op::NoteCreate(o::NoteCreate {
                id: id(1),
                path: "notes/Pricing.md".into(),
                content: "# Pricing\n".into(),
                created: monday(),
                force: true,
            }),
        ),
        op(
            2,
            Op::Capture(o::Capture {
                id: id(2),
                text: "كلمت أحمد".into(),
                created: monday(),
            }),
        ),
        op(3, entity(3)),
        // The same name again: `Ahmed 2.md`, and `title` keeps the name.
        op(4, entity(4)),
        op(
            5,
            Op::DocumentCreate(o::DocumentCreate {
                id: id(5),
                name: "Lease".into(),
                aliases: Vec::new(),
                doc_type: Some("contract".into()),
                copy: Some(CopyKind::Original),
                copy_of: None,
                companies: Vec::new(),
                people: vec![id(3)],
                expires: None,
                created: monday(),
                force: true,
            }),
        ),
        op(
            6,
            Op::PlaceCreate(o::PlaceCreate {
                id: id(6),
                name: "Safe".into(),
                aliases: Vec::new(),
                parent_id: None,
                address: None,
                created: monday(),
                force: true,
            }),
        ),
        op(
            7,
            Op::TaskCreate(o::TaskCreate {
                id: format!("t-{}", id(7).to_string().to_ascii_lowercase()),
                note_id: None,
                text: "Send the invoice".into(),
                due: None,
                scheduled: None,
                start: None,
                recurrence: None,
                reminders: Vec::new(),
                priority: None,
                created: monday(),
                home_id: Some(id(8)),
                force: true,
            }),
        ),
    ];
    let (response, _) = h.push(&alice, ops).await;
    let files = [
        (
            "notes/Pricing.md",
            format!("---\nid: {}\n{MONDAY}---\n# Pricing\n", id(1)),
        ),
        (
            "inbox/2026-09-28-081530.md",
            format!(
                "---\nid: {}\ncreated: 2026-09-28T08:15:30Z\n---\nكلمت أحمد\n",
                id(2)
            ),
        ),
        (
            "people/Ahmed.md",
            format!("---\nid: {}\nkind: person\n{MONDAY}---\n## Notes\n", id(3)),
        ),
        (
            "people/Ahmed 2.md",
            format!(
                "---\nid: {}\nkind: person\ntitle: Ahmed\n{MONDAY}---\n## Notes\n",
                id(4)
            ),
        ),
        (
            "documents/Lease.md",
            format!(
                "---\nid: {}\nkind: document\n{MONDAY}doc-type: contract\ncopy: original\npeople: [\"[[Ahmed]]\"]\n---\n## Notes\n",
                id(5)
            ),
        ),
        (
            "places/Safe.md",
            format!("---\nid: {}\nkind: place\n{MONDAY}---\n## Notes\n", id(6)),
        ),
    ];
    for (i, (path, content)) in files.iter().enumerate() {
        assert_eq!(&h.read(uid, path), content, "{path}");
        assert_eq!(response.results[i].result, applied(content), "{path}");
    }
    let task_line = format!(
        "- [ ] Send the invoice ^t-{}",
        id(7).to_string().to_ascii_lowercase()
    );
    assert_eq!(
        h.read(uid, "tasks/Tasks.md"),
        format!(
            "---\nid: {}\n{MONDAY}---\n## September 2026\n{task_line}\n",
            id(8)
        )
    );
    assert_eq!(response.results[6].result, applied(&task_line));
    h.finish().await;
}

#[tokio::test]
async fn creation_times_in_the_future_are_refused_over_sync_and_rest() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let now = strata_common::Clock::now(&h.clock);
    let commits = h.log(alice.id).len();
    let late = now + chrono::Duration::seconds(301);
    let capture = |n: u128, created: DateTime<Utc>| {
        op(
            n,
            Op::Capture(o::Capture {
                id: id(n),
                text: format!("capture {n}"),
                created,
            }),
        )
    };
    let (response, _) = h
        .push(
            &alice,
            vec![
                capture(1, late),
                op(
                    2,
                    Op::NoteCreate(o::NoteCreate {
                        id: id(2),
                        path: "notes/Later.md".into(),
                        content: "x\n".into(),
                        created: late,
                        force: true,
                    }),
                ),
            ],
        )
        .await;
    assert_eq!(
        response
            .results
            .iter()
            .map(|r| r.result.clone())
            .collect::<Vec<_>>(),
        vec![
            OpResult::Rejected {
                problem: in_future(300)
            },
            OpResult::Rejected {
                problem: in_future(300)
            },
        ]
    );
    assert_eq!(h.log(alice.id).len(), commits, "nothing written");

    // Exactly at the limit is accepted.
    let edge = now + chrono::Duration::seconds(300);
    let (response, _) = h.push(&alice, vec![capture(3, edge)]).await;
    assert!(
        matches!(response.results[0].result, OpResult::Applied { .. }),
        "{:?}",
        response.results[0].result
    );
    assert!(h.exists(alice.id, "inbox/2026-09-27-120500.md"));

    // REST: the same `422` problem, as the contract documents it.
    let err = ops::capture(
        &alice.client,
        &types::CaptureRequest {
            created: late,
            text: "too early".into(),
        },
    )
    .await
    .expect_err("refused");
    let problem = err.api().expect("a problem").problem().clone();
    assert_eq!(
        (
            problem.type_.as_str(),
            problem.status,
            problem.title.as_str(),
            problem.detail.as_deref()
        ),
        (
            "created_in_future",
            422,
            "Creation time is in the future",
            in_future(300).detail.as_deref()
        )
    );
    let err = ops::create_task(
        &alice.client,
        &types::CreateTaskRequest {
            created: late,
            due: None,
            force: None,
            home_id: None,
            id: None,
            note_id: None,
            priority: None,
            recurrence: None,
            reminders: vec![],
            scheduled: None,
            start: None,
            text: "later".into(),
        },
    )
    .await
    .expect_err("refused");
    assert_eq!(
        err.api().expect("a problem").problem().type_,
        "created_in_future"
    );
    assert_eq!(
        h.log(alice.id).len(),
        commits + 1,
        "only the capture at the limit"
    );
    h.finish().await;
}
