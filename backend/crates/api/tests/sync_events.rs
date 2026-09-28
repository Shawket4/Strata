//! `GET /events` (PLAN §7.5 Events, D24, §16.3): subscribed through the generated
//! `Subscription`, exact frame sequences for writes, frames conforming to the contract,
//! resume after reconnect, isolation, and `account.disabled` then close.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::assigning_clones,
    clippy::default_trait_access,
    clippy::float_cmp
)]

mod sync_harness;

use std::time::Duration;

use futures_util::StreamExt;
use pretty_assertions::assert_eq;
use strata_client::streaming::{StreamEvent, StreamOptions, Subscription};
use strata_client::{Error, operations as ops, streams, types};
use sync_harness::{H, User, version};
use sync_model::ops::{self as o, Op};
use sync_model::{OpResult, SyncOp, Version};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use ulid::Ulid;

const OP_BASE: u128 = 0x0199_0000_0000_0000_0000_0000_0000_0000;
const ID_BASE: u128 = 0x0199_1111_0000_0000_0000_0000_0000_0000;

fn id(n: u128) -> Ulid {
    Ulid(ID_BASE + n)
}

fn op(n: u128, base: Option<Version>, op: Op) -> SyncOp {
    SyncOp::new(Ulid(OP_BASE + n), base, op)
}

fn create(n: u128, path: &str, body: &str) -> Op {
    Op::NoteCreate(o::NoteCreate {
        created: strata_common::clock::default_test_epoch(),
        id: id(n),
        path: path.into(),
        content: body.into(),
        force: true,
    })
}

async fn push_ok(h: &H, user: &User, ops_: Vec<SyncOp>) {
    let (res, _) = h.push(user, ops_).await;
    for r in res.results {
        assert!(
            matches!(r.result, OpResult::Applied { .. }),
            "{:?}",
            r.result
        );
    }
}

fn options(resume_from: Option<u64>) -> StreamOptions {
    StreamOptions {
        resume_from,
        reconnect: false,
        idle_timeout: Duration::from_secs(20),
        ..StreamOptions::default()
    }
}

/// The next `n` data events (panics on anything else).
async fn take(sub: &mut Subscription<types::Event>, n: usize) -> Vec<(u64, types::Event)> {
    let mut out = Vec::new();
    while out.len() < n {
        let next = tokio::time::timeout(Duration::from_secs(10), sub.next())
            .await
            .unwrap_or_else(|_| panic!("timed out after {} events: {out:#?}", out.len()));
        match next {
            Some(Ok(StreamEvent::Data { seq, payload })) => out.push((seq, payload)),
            other => panic!("expected data, got {other:?}"),
        }
    }
    out
}

fn kind(k: &str) -> types::NoteKind {
    match k {
        "person" => types::NoteKind::Person,
        _ => types::NoteKind::Note,
    }
}

#[tokio::test]
async fn writes_produce_the_exact_event_sequence() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let uid = alice.id;
    let mut sub = streams::events(&alice.client, options(Some(0))).expect("subscription");

    push_ok(
        &h,
        &alice,
        vec![op(1, None, create(1, "notes/A.md", "a\n"))],
    )
    .await;
    let a1 = h.read(uid, "notes/A.md");
    push_ok(
        &h,
        &alice,
        vec![op(2, None, create(2, "notes/B.md", "b\n"))],
    )
    .await;
    let b = h.read(uid, "notes/B.md");
    push_ok(
        &h,
        &alice,
        vec![op(
            3,
            None,
            Op::RelationAdd(o::RelationRef {
                src_id: id(1),
                dst_id: id(2),
                relation: "related".parse().expect("rel"),
            }),
        )],
    )
    .await;
    let a2 = h.read(uid, "notes/A.md");
    push_ok(
        &h,
        &alice,
        vec![op(
            4,
            None,
            Op::EntityCreate(o::EntityCreate {
                created: strata_common::clock::default_test_epoch(),
                id: id(3),
                kind: domain::NoteKind::Person,
                name: "Sam".into(),
                aliases: vec![],
                fields: Default::default(),
                force: false,
            }),
        )],
    )
    .await;
    let sam = h.read(uid, "people/Sam.md");
    let tid = "t-01j9eventsaaaaaaaaaaaaaaa".to_owned();
    push_ok(
        &h,
        &alice,
        vec![op(
            5,
            None,
            Op::TaskCreate(o::TaskCreate {
                created: strata_common::clock::default_test_epoch(),
                home_id: None,
                id: tid.clone(),
                note_id: Some(id(2)),
                text: "Ask [[Sam]]".into(),
                due: None,
                scheduled: None,
                start: None,
                recurrence: None,
                reminders: vec![],
                priority: None,
                force: false,
            }),
        )],
    )
    .await;
    let b2 = h.read(uid, "notes/B.md");
    let line = format!("- [ ] Ask [[Sam]] ^{tid}");
    push_ok(
        &h,
        &alice,
        vec![op(
            6,
            Some(version(&b2)),
            Op::NoteMove(o::NoteMove {
                id: id(2),
                new_path: "archive/B.md".into(),
            }),
        )],
    )
    .await;
    // A duplicate capture: the capture, then the suggestion.
    for (n, i) in [(7, 4), (8, 5)] {
        push_ok(
            &h,
            &alice,
            vec![op(
                n,
                None,
                Op::Capture(o::Capture {
                    id: id(i),
                    text: "call the notary tomorrow".into(),
                    created: strata_testkit::default_test_epoch().fixed_offset()
                        + chrono::Duration::seconds(i64::try_from(i).expect("small")),
                }),
            )],
        )
        .await;
    }
    let a3 = h.read(uid, "notes/A.md");
    push_ok(
        &h,
        &alice,
        vec![op(
            9,
            Some(version(&a3)),
            Op::NoteDelete(o::NoteRef { id: id(1) }),
        )],
    )
    .await;

    assert_eq!(a3, a2);
    let events = take(&mut sub, 14).await;
    let seqs: Vec<u64> = events.iter().map(|(s, _)| *s).collect();
    assert_eq!(seqs, (1..=14).collect::<Vec<_>>());
    let payloads: Vec<types::Event> = events.into_iter().map(|(_, e)| e).collect();
    let pending = ops::list_suggestions(&alice.client, None)
        .await
        .expect("suggestions");
    let dup = &pending.items[0];
    let cap5 = format!(
        "inbox/{}.md",
        (strata_testkit::default_test_epoch() + chrono::Duration::seconds(5))
            .format("%Y-%m-%d-%H%M%S")
    );
    let cap4 = cap5.replace("120005", "120004");
    let a_trash_path = "notes/A.md".to_owned();
    assert_eq!(
        payloads,
        vec![
            types::Event::NoteCreated {
                id: id(1),
                kind: kind("note"),
                path: "notes/A.md".into(),
                version: version(&a1).to_string(),
            },
            types::Event::NoteCreated {
                id: id(2),
                kind: kind("note"),
                path: "notes/B.md".into(),
                version: version(&b).to_string(),
            },
            types::Event::NoteUpdated {
                id: id(1),
                kind: kind("note"),
                path: "notes/A.md".into(),
                version: version(&a2).to_string(),
            },
            types::Event::RelationAdded {
                dst_id: id(2),
                relation: "related".into(),
                src_id: id(1),
            },
            types::Event::NoteCreated {
                id: id(3),
                kind: kind("person"),
                path: "people/Sam.md".into(),
                version: version(&sam).to_string(),
            },
            types::Event::EntityCreated {
                id: id(3),
                kind: kind("person"),
                version: version(&sam).to_string(),
            },
            types::Event::NoteUpdated {
                id: id(2),
                kind: kind("note"),
                path: "notes/B.md".into(),
                version: version(&b2).to_string(),
            },
            types::Event::TaskChanged {
                id: tid.clone(),
                note_id: id(2),
                version: Some(version(&line).to_string()),
            },
            // `[[B]]` in A still resolves by name after the move: A is unchanged.
            types::Event::NoteMoved {
                id: id(2),
                kind: kind("note"),
                old_path: "notes/B.md".into(),
                path: "archive/B.md".into(),
                version: version(&h.read(uid, "archive/B.md")).to_string(),
            },
            types::Event::NoteCreated {
                id: id(4),
                kind: kind("note"),
                path: cap4.clone(),
                version: version(&h.read(uid, &cap4)).to_string(),
            },
            types::Event::NoteCreated {
                id: id(5),
                kind: kind("note"),
                path: cap5.clone(),
                version: version(&h.read(uid, &cap5)).to_string(),
            },
            types::Event::SuggestionCreated {
                id: dup.id,
                kind: "duplicate".into(),
                note_id: Some(id(5)),
            },
            types::Event::NoteDeleted {
                id: id(1),
                kind: kind("note"),
                path: a_trash_path,
            },
            types::Event::RelationRemoved {
                dst_id: id(2),
                relation: "related".into(),
                src_id: id(1),
            },
        ]
    );
    sub.close().await;
    h.finish().await;
}

#[tokio::test]
async fn frames_conform_to_the_contract_and_resume_after_a_reconnect() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    // The generated subscription resumes from the start; a raw socket connects fresh (at the
    // head) before the write.
    let mut sub = streams::events(&alice.client, options(Some(0))).expect("subscription");
    let url = format!("ws://{}/api/v1/events", h.addr());
    let mut request = url.into_client_request().expect("request");
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", alice.token).parse().expect("header"),
    );
    let (mut raw, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("raw socket");
    push_ok(
        &h,
        &alice,
        vec![op(1, None, create(1, "notes/A.md", "a\n"))],
    )
    .await;
    let first = take(&mut sub, 1).await;
    assert_eq!(first[0].0, 1);
    for expected_seq in [1_u64] {
        let msg = raw.next().await.expect("frame").expect("message");
        let Message::Binary(bytes) = msg else {
            panic!("expected a binary frame, got {msg:?}");
        };
        h.conformance
            .contract
            .validate_frame("events", &bytes)
            .expect("frame conforms");
        let frame: strata_api::wire::ws::Frame<strata_api::events::Event> =
            strata_api::wire::ws::Frame::decode(&bytes, &strata_api::wire::DecodeLimits::default())
                .expect("frame");
        assert_eq!(frame.seq(), expected_seq);
    }
    // Disconnect; writes happen meanwhile; reconnect resuming after the last seq seen.
    let last = sub.last_seq().expect("seq");
    sub.close().await;
    let a = h.read(alice.id, "notes/A.md");
    push_ok(
        &h,
        &alice,
        vec![
            op(
                2,
                Some(version(&a)),
                Op::NoteUpdate(o::NoteUpdate {
                    id: id(1),
                    content: a.replace("a\n", "a2\n"),
                }),
            ),
            op(3, None, create(2, "notes/B.md", "b\n")),
        ],
    )
    .await;
    let mut resumed = streams::events(&alice.client, options(Some(last))).expect("subscription");
    let events = take(&mut resumed, 2).await;
    assert_eq!(
        events,
        vec![
            (
                2,
                types::Event::NoteUpdated {
                    id: id(1),
                    kind: types::NoteKind::Note,
                    path: "notes/A.md".into(),
                    version: version(&h.read(alice.id, "notes/A.md")).to_string(),
                }
            ),
            (
                3,
                types::Event::NoteCreated {
                    id: id(2),
                    kind: types::NoteKind::Note,
                    path: "notes/B.md".into(),
                    version: version(&h.read(alice.id, "notes/B.md")).to_string(),
                }
            ),
        ]
    );
    resumed.close().await;
    // A seq the server never issued (e.g. from before a restart) gets `reset` to the head.
    let mut future = streams::events(&alice.client, options(Some(99))).expect("subscription");
    assert_eq!(
        future.next().await.expect("frame").expect("reset"),
        StreamEvent::Reset { seq: 3 }
    );
    future.close().await;
    // The raw socket received the later events too, each conforming.
    for expected_seq in [2_u64, 3] {
        let Message::Binary(bytes) = raw.next().await.expect("frame").expect("message") else {
            panic!("binary frame expected");
        };
        h.conformance
            .contract
            .validate_frame("events", &bytes)
            .expect("frame conforms");
        let frame: strata_api::wire::ws::Frame<strata_api::events::Event> =
            strata_api::wire::ws::Frame::decode(&bytes, &strata_api::wire::DecodeLimits::default())
                .expect("frame");
        assert_eq!(frame.seq(), expected_seq);
    }
    h.finish().await;
}

#[tokio::test]
async fn streams_are_isolated_and_upgrades_need_a_token() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let mut bob_sub = streams::events(&bob.client, options(Some(0))).expect("subscription");
    push_ok(
        &h,
        &alice,
        vec![op(1, None, create(1, "notes/Same.md", "alice\n"))],
    )
    .await;
    push_ok(
        &h,
        &bob,
        vec![op(1, None, create(2, "notes/Same.md", "bob\n"))],
    )
    .await;
    let events = take(&mut bob_sub, 1).await;
    assert_eq!(
        events,
        vec![(
            1,
            types::Event::NoteCreated {
                id: id(2),
                kind: types::NoteKind::Note,
                path: "notes/Same.md".into(),
                version: version(&h.read(bob.id, "notes/Same.md")).to_string(),
            }
        )]
    );
    bob_sub.close().await;
    // No token: 401 on upgrade.
    let anon = strata_client::Client::new(&h.base_url()).expect("client");
    let mut sub = streams::events(&anon, options(None)).expect("subscription");
    match sub.next().await {
        Some(Err(Error::WebSocket(e))) => {
            assert!(
                matches!(&*e, tokio_tungstenite::tungstenite::Error::Http(r) if r.status() == 401),
                "{e:?}"
            );
        }
        other => panic!("expected 401, got {other:?}"),
    }
    h.finish().await;
}

#[tokio::test]
async fn disabling_the_account_sends_account_disabled_then_closes() {
    let h = H::new().await;
    let admin = h.admin("root").await;
    let alice = h.user("alice").await;
    let mut sub = streams::events(&alice.client, options(Some(0))).expect("subscription");
    push_ok(
        &h,
        &alice,
        vec![op(1, None, create(1, "notes/A.md", "a\n"))],
    )
    .await;
    assert_eq!(take(&mut sub, 1).await[0].0, 1);
    ops::admin_update_user(
        &admin.client,
        alice.id.as_ulid(),
        &types::UpdateUser {
            status: Some(types::SettableStatus::Disabled),
            ..Default::default()
        },
    )
    .await
    .expect("disable");
    assert_eq!(
        take(&mut sub, 1).await,
        vec![(
            2,
            types::Event::AccountDisabled {
                reason: types::AccountClosure::Disabled
            }
        )]
    );
    match sub.next().await {
        Some(Err(err)) => {
            let problem = err.api().expect("problem").problem().clone();
            assert_eq!(
                (problem.type_.as_str(), problem.status),
                ("account_disabled", 403)
            );
        }
        other => panic!("expected the terminal error, got {other:?}"),
    }
    assert!(sub.next().await.is_none());
    // The stream cannot be reopened.
    let mut again = streams::events(&alice.client, options(None)).expect("subscription");
    assert!(matches!(again.next().await, Some(Err(_))));
    h.finish().await;
}
