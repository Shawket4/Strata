//! Creation times, UTC and the title rule in the vault store (owner decision 2026-09-28
//! "Times, creation stamps and titles"): a create writes the device's creation time as
//! `created`/`updated` (never the server's clock), in UTC; a creation time more than the
//! configured skew in the future is refused; `title` is written whenever the file stem differs
//! from the item's name, for user and AI creates alike.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use common::World;
use domain::NoteKind;
use pretty_assertions::assert_eq;
use strata_common::{Clock, NoteId};
use strata_vault::ops::ai_apply::{AiApplied, AiChangeSet, NewEntityNote};
use strata_vault::ops::entities::NewEntity;
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::tasks::NewTask;
use strata_vault::{VaultConfig, VaultError, VaultService};
use ulid::Ulid;

fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).expect("instant").to_utc()
}

/// Monday 08:15:30.4 UTC (11:15:30 in Cairo): when the items below were made offline.
fn monday() -> DateTime<Utc> {
    at("2026-09-28T08:15:30.400Z")
}

/// `created`/`updated` as written for [`monday`]: UTC with `Z`, whole seconds.
const MONDAY: &str = "created: 2026-09-28T08:15:30Z\nupdated: 2026-09-28T08:15:30Z\n";

fn person(name: &str, created: DateTime<Utc>) -> NewEntity {
    NewEntity {
        kind: NoteKind::Person,
        name: name.into(),
        aliases: Vec::new(),
        tags: Vec::new(),
        fields: BTreeMap::new(),
        parent: None,
        id: None,
        created,
        force: true,
    }
}

async fn set_timezone(w: &World, user: strata_common::UserId, zone: &str) {
    let mut tx = w.db.begin(user).await.expect("tx");
    strata_index::repo::settings::put_setting(
        &mut tx,
        "timezone",
        &rmp_serde::to_vec(zone).expect("encode"),
        w.db.clock.now(),
    )
    .await
    .expect("timezone");
    tx.commit().await.expect("commit");
}

/// Items made offline on Monday and written on Wednesday keep Monday, in UTC; the task goes
/// under the month of its creation in the user's time zone, and a new `tasks/Tasks.md` gets
/// the device's ID.
#[tokio::test]
async fn creates_write_the_device_creation_time_in_utc() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    set_timezone(&w, u, "Africa/Cairo").await;
    // Wednesday on the server.
    w.db.clock.set(at("2026-09-30T09:00:00Z"));

    let note = w
        .vault
        .create_note(
            &s,
            CreateNote {
                path: "notes/Pricing.md".into(),
                content: "# Pricing\n".into(),
                created: monday(),
                id: None,
                force: true,
            },
        )
        .await
        .expect("note");
    assert_eq!(
        w.read(u, "notes/Pricing.md"),
        format!("---\nid: {}\n{MONDAY}---\n# Pricing\n", note.id)
    );

    let capture = w
        .vault
        .capture(&s, "كلمت أحمد".into(), monday())
        .await
        .expect("capture");
    assert_eq!(capture.note.path, "inbox/2026-09-28-081530.md");
    assert_eq!(
        w.read(u, &capture.note.path),
        format!(
            "---\nid: {}\ncreated: 2026-09-28T08:15:30Z\n---\nكلمت أحمد\n",
            capture.note.id
        )
    );

    let ahmed = w
        .vault
        .create_entity(&s, person("Ahmed", monday()))
        .await
        .expect("person");
    assert_eq!(
        w.read(u, "people/Ahmed.md"),
        format!("---\nid: {}\nkind: person\n{MONDAY}---\n## Notes\n", ahmed.id)
    );

    // A task made at 22:30Z on 30 September — 1 October, 01:30 in Cairo — goes under
    // October; the new task note gets the device's ID and creation time.
    let home = NoteId::from_ulid(Ulid::from_parts(1_790_000_000_000, 7));
    let late_september = at("2026-09-30T22:30:00Z");
    w.db.clock.set(at("2026-10-01T06:00:00Z"));
    let task = w
        .vault
        .create_task(
            &s,
            NewTask {
                text: "Send the invoice".into(),
                home_id: Some(home),
                ..NewTask::new(late_september)
            },
        )
        .await
        .expect("task");
    assert_eq!(
        w.read(u, "tasks/Tasks.md"),
        format!(
            "---\nid: {home}\ncreated: 2026-09-30T22:30:00Z\nupdated: 2026-09-30T22:30:00Z\n---\n## October 2026\n- [ ] Send the invoice ^{task}\n"
        )
    );
    w.finish().await;
}

/// A creation time more than the skew ahead of the server's clock is refused and nothing is
/// written; exactly at the limit is accepted. The skew is configurable.
#[tokio::test]
async fn creation_times_in_the_future_are_refused() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let now = w.db.clock.now();
    let commits = w.log(u).len();
    let refused = VaultError::CreatedInFuture { max_skew_secs: 300 };

    let late = now + chrono::Duration::seconds(301);
    let err = w
        .vault
        .capture(&s, "early".into(), late)
        .await
        .expect_err("refused");
    assert!(matches!(err, VaultError::CreatedInFuture { max_skew_secs: 300 }), "{err:?}");
    assert!(matches!(
        w.vault.create_entity(&s, person("Mona", late)).await,
        Err(VaultError::CreatedInFuture { max_skew_secs: 300 })
    ));
    assert!(matches!(
        w.vault
            .create_task(&s, NewTask { text: "x".into(), ..NewTask::new(late) })
            .await,
        Err(VaultError::CreatedInFuture { max_skew_secs: 300 })
    ));
    assert!(matches!(
        w.vault
            .create_note(
                &s,
                CreateNote {
                    path: "notes/X.md".into(),
                    content: "x\n".into(),
                    created: late,
                    id: None,
                    force: true,
                },
            )
            .await,
        Err(VaultError::CreatedInFuture { max_skew_secs: 300 })
    ));
    assert_eq!(w.log(u).len(), commits, "nothing written");
    assert_eq!(
        strata_common::DomainError::public_detail(&refused).as_deref(),
        Some("`created` is more than 300 seconds ahead of the server's clock; check the device's clock")
    );

    let edge = now + chrono::Duration::seconds(300);
    let ok = w
        .vault
        .capture(&s, "just in time".into(), edge)
        .await
        .expect("accepted");
    assert_eq!(ok.note.path, "inbox/2026-09-27-120500.md");

    // A stricter server: one minute.
    let mut config = VaultConfig::new(w.data.path());
    config.max_future_skew_secs = 60;
    let strict = VaultService::new(
        config,
        w.db.app_db.clone(),
        Arc::new(w.db.clock.clone()),
        w.db.ids.clone(),
    );
    assert!(matches!(
        strict
            .capture(&s, "two minutes early".into(), now + chrono::Duration::seconds(61))
            .await,
        Err(VaultError::CreatedInFuture { max_skew_secs: 60 })
    ));
    drop(strict);
    w.finish().await;
}

/// The title rule, for a user create and an AI create of a name that is taken: the file is
/// `Ahmed 2.md` (then `Ahmed 3.md`) and `title` keeps the name.
#[tokio::test]
async fn a_taken_name_keeps_its_title_for_user_and_ai_creates() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    w.db.clock.set(at("2026-09-30T09:00:00Z"));
    let first = w
        .vault
        .create_entity(&s, person("Ahmed", monday()))
        .await
        .expect("first");
    assert_eq!(first.path, "people/Ahmed.md");

    // User create.
    let second = w
        .vault
        .create_entity(&s, person(" Ahmed ", monday()))
        .await
        .expect("second");
    assert_eq!(second.path, "people/Ahmed 2.md");
    assert_eq!(
        w.read(u, "people/Ahmed 2.md"),
        format!(
            "---\nid: {}\nkind: person\ntitle: Ahmed\n{MONDAY}---\n## Notes\n",
            second.id
        )
    );

    // AI create (an AI job: the server's clock).
    let id = NoteId::from_ulid(Ulid::from_parts(1_790_510_400_000, 0x42));
    let mut set = AiChangeSet::new("link", first.id);
    set.entities.push(NewEntityNote {
        id,
        kind: NoteKind::Person,
        name: "Ahmed".into(),
        aliases: vec!["أحمد".into()],
    });
    let applied = w.vault.ai_apply(&s, set).await.expect("ai");
    assert!(matches!(applied, AiApplied::Done { .. }), "{applied:?}");
    assert_eq!(
        w.read(u, "people/Ahmed 3.md"),
        format!(
            "---\nid: {id}\nkind: person\ntitle: Ahmed\naliases: [أحمد]\ncreated: 2026-09-30T09:00:00Z\nupdated: 2026-09-30T09:00:00Z\n---\n## Notes\n"
        )
    );

    // A user's decision on AI output (accepting a suggestion that creates an entity) carries
    // the time the user decided on the device.
    let decided = NoteId::from_ulid(Ulid::from_parts(1_790_510_400_000, 0x43));
    let mut set = AiChangeSet::new("suggestion", first.id).by_user("accept entity_link");
    set.created_at = Some(monday());
    set.entities.push(NewEntityNote {
        id: decided,
        kind: NoteKind::Person,
        name: "Ahmed".into(),
        aliases: Vec::new(),
    });
    let applied = w.vault.ai_apply(&s, set).await.expect("decision");
    assert!(matches!(applied, AiApplied::Done { .. }), "{applied:?}");
    assert_eq!(
        w.read(u, "people/Ahmed 4.md"),
        format!("---\nid: {decided}\nkind: person\ntitle: Ahmed\n{MONDAY}---\n## Notes\n")
    );
    w.finish().await;
}
