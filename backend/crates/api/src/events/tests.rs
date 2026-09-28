use std::sync::Arc;

use chrono::Duration;
use futures_util::StreamExt;
use strata_common::{FakeClock, IdGenerator, NoteId, SequentialIdGenerator, SuggestionId};
use strata_vault::events::{
    CustodyEvent, NoteChange, NoteEvent, RelationEvent, SuggestionEvent, TaskEvent,
};

use super::*;

fn ids() -> SequentialIdGenerator {
    SequentialIdGenerator::default()
}

fn note(id: NoteId, kind: domain::NoteKind, change: NoteChange, v: Option<&str>) -> NoteEvent {
    NoteEvent {
        id,
        kind,
        change,
        path: format!("p/{id}.md"),
        old_path: (change == NoteChange::Moved).then(|| "old.md".to_owned()),
        version: v.map(str::to_owned),
    }
}

#[test]
fn commit_notices_map_to_events_in_a_fixed_order() {
    let idgen = ids();
    let (a, b, c) = (
        NoteId::from_ulid(idgen.next_ulid()),
        NoteId::from_ulid(idgen.next_ulid()),
        NoteId::from_ulid(idgen.next_ulid()),
    );
    let s = SuggestionId::from_ulid(idgen.next_ulid());
    let notice = Committed {
        user: None,
        op: "merge".into(),
        notes: vec![
            note(a, domain::NoteKind::Person, NoteChange::Deleted, None),
            note(
                b,
                domain::NoteKind::Person,
                NoteChange::Updated,
                Some("v-b"),
            ),
            note(c, domain::NoteKind::Note, NoteChange::Moved, Some("v-c")),
        ],
        merged: Some((a, b)),
        relations: vec![RelationEvent {
            src: c,
            rel: "related".into(),
            dst: b,
            added: true,
        }],
        tasks: vec![TaskEvent {
            id: "t-1".into(),
            note_id: c,
            version: None,
        }],
        custody: vec![CustodyEvent {
            document_id: b,
            version: Some("v-b".into()),
        }],
        suggestions: vec![SuggestionEvent {
            id: s,
            note_id: Some(c),
            kind: "conflict".into(),
            status: "pending".into(),
            created: true,
        }],
        integrity_warnings: 2,
    };
    assert_eq!(
        events_of(&notice),
        vec![
            Event::NoteDeleted {
                id: a.as_ulid(),
                path: format!("p/{a}.md"),
                kind: NoteKind::Person,
            },
            Event::NoteUpdated {
                id: b.as_ulid(),
                path: format!("p/{b}.md"),
                kind: NoteKind::Person,
                version: "v-b".into(),
            },
            Event::NoteMoved {
                id: c.as_ulid(),
                old_path: "old.md".into(),
                path: format!("p/{c}.md"),
                kind: NoteKind::Note,
                version: "v-c".into(),
            },
            Event::EntityMerged {
                id: a.as_ulid(),
                into_id: b.as_ulid(),
                kind: NoteKind::Person,
                version: Some("v-b".into()),
            },
            Event::RelationAdded {
                src_id: c.as_ulid(),
                dst_id: b.as_ulid(),
                relation: "related".into(),
            },
            Event::TaskChanged {
                id: "t-1".into(),
                note_id: c.as_ulid(),
                version: None,
            },
            Event::CustodyChanged {
                document_id: b.as_ulid(),
                version: Some("v-b".into()),
            },
            Event::SuggestionCreated {
                id: s.as_ulid(),
                note_id: Some(c.as_ulid()),
                kind: "conflict".into(),
            },
            Event::IntegrityWarning { count: 2 },
        ]
    );
}

fn tick(n: u32) -> Event {
    Event::IntegrityWarning { count: n }
}

#[test]
fn users_have_separate_seqs_and_replay_buffers() {
    let g = ids();
    let (u1, u2) = (UserId::from(g.next_ulid()), UserId::from(g.next_ulid()));
    let bus = EventBus::new(BusConfig {
        replay_capacity: 2,
        broadcast_capacity: 8,
        first_seq_after: 100,
    });
    assert_eq!(bus.publish(u1, [tick(1), tick(2)]), 102);
    assert_eq!(bus.publish(u2, [tick(9)]), 101);
    assert_eq!(bus.publish(u1, [tick(3)]), 103);
    let sub = bus.subscribe(u1, Some(101));
    assert_eq!(
        sub.replay,
        vec![
            Frame::Data {
                seq: 102,
                payload: tick(2)
            },
            Frame::Data {
                seq: 103,
                payload: tick(3)
            }
        ]
    );
    // 101 was evicted (capacity 2): resuming after 100 needs a reset.
    assert_eq!(
        bus.subscribe(u1, Some(100)).replay,
        vec![Frame::Reset { seq: 103 }]
    );
    assert_eq!(bus.subscribe(u2, None).replay, vec![]);
    assert_eq!(bus.head(u2), 101);
}

#[tokio::test]
async fn a_disabled_account_gets_account_disabled_then_a_terminal_error() {
    let g = ids();
    let (user, session) = (UserId::from(g.next_ulid()), SessionId::from(g.next_ulid()));
    let clock = FakeClock::at_default_epoch();
    let revocations = Arc::new(RevocationSet::new(
        Duration::minutes(15),
        Arc::new(clock.clone()),
    ));
    let bus = Arc::new(EventBus::new(BusConfig::default()));
    let mut frames = connection(bus.clone(), revocations.clone(), user, session, None);
    bus.publish(user, [tick(1)]);
    assert_eq!(
        frames.next().await,
        Some(Frame::Data {
            seq: 1,
            payload: tick(1)
        })
    );
    revocations.update_user(user, |f| f.disabled = true);
    assert_eq!(
        frames.next().await,
        Some(Frame::Data {
            seq: 2,
            payload: Event::AccountDisabled {
                reason: AccountClosure::Disabled
            }
        })
    );
    assert_eq!(
        frames.next().await,
        Some(Frame::Error {
            seq: 2,
            problem: Problem::new(ProblemType::AccountDisabled)
        })
    );
    assert_eq!(frames.next().await, None);
}

#[tokio::test]
async fn a_revoked_session_ends_with_unauthorized_and_other_users_are_unaffected() {
    let g = ids();
    let (user, other) = (UserId::from(g.next_ulid()), UserId::from(g.next_ulid()));
    let (session, other_session) = (
        SessionId::from(g.next_ulid()),
        SessionId::from(g.next_ulid()),
    );
    let clock = FakeClock::at_default_epoch();
    let revocations = Arc::new(RevocationSet::new(
        Duration::minutes(15),
        Arc::new(clock.clone()),
    ));
    let bus = Arc::new(EventBus::new(BusConfig::default()));
    let mut mine = connection(bus.clone(), revocations.clone(), user, session, None);
    let mut theirs = connection(bus.clone(), revocations.clone(), other, other_session, None);
    revocations.revoke_sessions([session]);
    assert_eq!(
        mine.next().await,
        Some(Frame::Error {
            seq: 0,
            problem: Problem::new(ProblemType::Unauthorized)
        })
    );
    assert_eq!(mine.next().await, None);
    bus.publish(user, [tick(5)]);
    bus.publish(other, [tick(6)]);
    assert_eq!(
        theirs.next().await,
        Some(Frame::Data {
            seq: 1,
            payload: tick(6)
        })
    );
}
