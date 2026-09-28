//! L16 cross-check: the server's write path (`strata-vault`) and the device's optimistic apply
//! (`strata_core::store::write::apply_to_note`) render new items through the same shared
//! code (`item-render`, `sync-model`), so for the same op they write the same bytes. Each
//! case runs the op on a real vault (per-test `PostgreSQL` database, fake clock) and on the
//! device, then compares the files byte for byte. Every input comes from the op — IDs (a new
//! `tasks/Tasks.md` included) and the device's creation time, which is not the server's
//! clock — so the files are identical with no adjustment. The user's time zone is Cairo on
//! both sides; every time is written in UTC.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use domain::{CopyKind, CustodyEventType, NoteKind};
use pretty_assertions::assert_eq;
use strata_common::{NoteId, UserId};
use strata_core::store::notes::NoteState;
use strata_core::store::write::{Links, apply_to_note};
use strata_core::sync::model::Op;
use strata_index::UserScope;
use strata_testkit::{TempDataRoot, TestDb, TestUser};
use strata_vault::ops::entities::{NewCustodyEvent, NewEntity};
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::tasks::NewTask;
use strata_vault::{VaultConfig, VaultService};
use sync_model::ops;
use ulid::Ulid;
use vault_format::PathIndex;

/// The server side of one test: a provisioned vault of one user.
struct Server {
    db: TestDb,
    _data: TempDataRoot,
    vault: VaultService,
    user: UserId,
    scope: UserScope,
    /// Note ID → path of every note the test created.
    paths: BTreeMap<Ulid, String>,
}

impl Server {
    async fn new() -> Self {
        let db = TestDb::new().await.expect("db");
        let data = TempDataRoot::new().expect("data root");
        let vault = VaultService::new(
            VaultConfig::new(data.path()),
            db.app_db.clone(),
            Arc::new(db.clock.clone()),
            db.ids.clone(),
        );
        let user = TestUser::new("alice").create(&db).await.expect("user").id;
        assert!(vault.provision(user).await.expect("provision"));
        let scope = db.scope(user);
        // The account's time zone (month headings of `tasks/Tasks.md`).
        let mut tx = db.begin(user).await.expect("tx");
        strata_index::repo::settings::put_setting(
            &mut tx,
            "timezone",
            &rmp_serde::to_vec("Africa/Cairo").expect("encode"),
            DateTime::UNIX_EPOCH,
        )
        .await
        .expect("timezone");
        tx.commit().await.expect("commit");
        Self {
            db,
            _data: data,
            vault,
            user,
            scope,
            paths: BTreeMap::new(),
        }
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.vault.vault_dir(self.user).join(path)).expect("read")
    }

    fn state(&self, id: Ulid) -> NoteState {
        let path = self.paths[&id].clone();
        NoteState {
            content: self.read(&path),
            path,
        }
    }

    async fn finish(self) {
        drop(self.vault);
        self.db.cleanup().await.expect("cleanup");
    }
}

/// The device's view of the server's notes: link texts from the same paths.
struct DeviceLinks<'a>(&'a BTreeMap<Ulid, String>);

impl Links for DeviceLinks<'_> {
    fn link_for(&self, id: Ulid) -> Option<String> {
        let index = PathIndex::new(self.0.values().map(String::as_str));
        self.0.get(&id).map(|p| index.link_text_for(p))
    }

    fn time_zone(&self) -> Tz {
        chrono_tz::Africa::Cairo
    }

    fn taken_paths(&self, except: Ulid) -> Vec<String> {
        self.0
            .iter()
            .filter(|(id, _)| **id != except)
            .map(|(_, p)| p.clone())
            .collect()
    }
}

/// What the device writes for `op` on `state`.
fn device(server: &Server, state: Option<NoteState>, op: &Op) -> NoteState {
    apply_to_note(state, op, &DeviceLinks(&server.paths))
        .expect("device apply")
        .expect("a note")
}

/// The device's creation time of every item: "Monday" 08:15:30 UTC (11:15:30 in Cairo), a
/// day before the server's clock (2026-09-27T12:00:00Z), so a file carrying the server's
/// clock would differ.
fn monday() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-26T08:15:30.250Z")
        .expect("ts")
        .to_utc()
}

/// `created`/`updated` as both sides write them for [`monday`]: UTC, whole seconds.
const STAMP: &str = "created: 2026-09-26T08:15:30Z\nupdated: 2026-09-26T08:15:30Z\n";

fn ulid(n: u128) -> Ulid {
    // 2026-09-27T12:00:00Z, the fake clock's epoch (task block IDs carry their creation time).
    Ulid::from_parts(1_790_510_400_000, n)
}

fn task_id(n: u128) -> String {
    item_render::task::task_block_id(ulid(n))
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
}

#[tokio::test]
async fn server_and_device_render_new_items_identically() {
    let mut s = Server::new().await;

    // Capture: same path, same bytes (the capture time and ID come from the device; the file
    // is named in UTC: 14:32:05 in Cairo is 11:32:05Z).
    let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:05+03:00")
        .expect("ts")
        .to_utc();
    let capture = ops::Capture {
        id: ulid(0x100),
        text: "كلمت أحمد النهارده\r\nعن الفاتورة".into(),
        created,
    };
    let server = s
        .vault
        .capture_as(
            &s.scope,
            capture.text.clone(),
            NoteId::from_ulid(capture.id),
            created,
        )
        .await
        .expect("capture");
    let local = device(&s, None, &Op::Capture(capture.clone()));
    assert_eq!(local.path, "inbox/2026-09-27-113205.md");
    assert_eq!(
        local.content,
        format!(
            "---\nid: {}\ncreated: 2026-09-27T11:32:05Z\n---\nكلمت أحمد النهارده\r\nعن الفاتورة\n",
            capture.id
        )
    );
    assert_eq!(
        (local.path.as_str(), local.content.as_str()),
        (
            server.note.path.as_str(),
            s.read(&server.note.path).as_str()
        )
    );
    // A second capture in the same second: the server's free name is the shared rule's.
    let again = s
        .vault
        .capture_as(
            &s.scope,
            "ثانية".into(),
            NoteId::from_ulid(ulid(0x101)),
            created,
        )
        .await
        .expect("capture");
    assert_eq!(again.note.path, "inbox/2026-09-27-113205 2.md");
    assert_eq!(
        item_render::paths::capture_path(&created, [server.note.path.as_str()]),
        again.note.path
    );

    // Person with a name that is not a valid file name: `title` keeps the name.
    let person = ops::EntityCreate {
        id: ulid(0x200),
        kind: NoteKind::Person,
        name: " Shady: ops ".into(),
        aliases: vec![" شادي ".into(), "#Shady".into(), "شادي".into()],
        fields: BTreeMap::from([("role".to_owned(), "Operations".to_owned())]),
        created: monday(),
        force: true,
    };
    let view = s
        .vault
        .create_entity(
            &s.scope,
            NewEntity {
                kind: person.kind,
                name: person.name.clone(),
                aliases: person.aliases.clone(),
                tags: Vec::new(),
                fields: person.fields.clone(),
                parent: None,
                id: Some(NoteId::from_ulid(person.id)),
                created: person.created,
                force: true,
            },
        )
        .await
        .expect("person");
    let local = device(&s, None, &Op::EntityCreate(person.clone()));
    assert_eq!(local.path, view.path);
    assert_eq!(
        local.content,
        format!(
            "---\nid: {}\nkind: person\ntitle: \"Shady: ops\"\naliases: [شادي, Shady]\n{STAMP}role: Operations\n---\n## Notes\n",
            person.id
        )
    );
    assert_eq!(local.content, s.read(&view.path));
    s.paths.insert(person.id, view.path);

    // Company, then places (one inside the other).
    let company = ops::EntityCreate {
        id: ulid(0x201),
        kind: NoteKind::Company,
        name: "Watanya".into(),
        aliases: Vec::new(),
        fields: BTreeMap::new(),
        created: monday(),
        force: true,
    };
    let view = s
        .vault
        .create_entity(
            &s.scope,
            NewEntity {
                kind: NoteKind::Company,
                name: "Watanya".into(),
                aliases: Vec::new(),
                tags: Vec::new(),
                fields: BTreeMap::new(),
                parent: None,
                id: Some(NoteId::from_ulid(company.id)),
                created: company.created,
                force: true,
            },
        )
        .await
        .expect("company");
    let local = device(&s, None, &Op::EntityCreate(company.clone()));
    assert_eq!(
        (local.path.as_str(), local.content.as_str()),
        (view.path.as_str(), s.read(&view.path).as_str())
    );
    s.paths.insert(company.id, view.path);

    // The title rule on a taken name: a second "Watanya" is `Watanya 2.md` with
    // `title: Watanya`, on the server and on the device alike.
    let second = ops::EntityCreate {
        id: ulid(0x202),
        ..company.clone()
    };
    let view = s
        .vault
        .create_entity(
            &s.scope,
            NewEntity {
                kind: NoteKind::Company,
                name: "Watanya".into(),
                aliases: Vec::new(),
                tags: Vec::new(),
                fields: BTreeMap::new(),
                parent: None,
                id: Some(NoteId::from_ulid(second.id)),
                created: second.created,
                force: true,
            },
        )
        .await
        .expect("second company");
    let local = device(&s, None, &Op::EntityCreate(second.clone()));
    assert_eq!(local.path, "companies/Watanya 2.md");
    assert_eq!(
        local.content,
        format!(
            "---\nid: {}\nkind: company\ntitle: Watanya\n{STAMP}---\n## Notes\n",
            second.id
        )
    );
    assert_eq!(
        (local.path.as_str(), local.content.as_str()),
        (view.path.as_str(), s.read(&view.path).as_str())
    );
    s.paths.insert(second.id, view.path);

    let mut places = Vec::new();
    for (n, name, parent, address) in [
        (0x300, "Nasr City office", None, Some("12 Abbas El Akkad")),
        (0x301, "Safe — Nasr City office", Some(ulid(0x300)), None),
    ] {
        let op = ops::PlaceCreate {
            id: ulid(n),
            name: name.into(),
            aliases: Vec::new(),
            parent_id: parent,
            address: address.map(str::to_owned),
            created: monday(),
            force: true,
        };
        let view = s
            .vault
            .create_entity(
                &s.scope,
                NewEntity {
                    kind: NoteKind::Place,
                    name: op.name.clone(),
                    aliases: Vec::new(),
                    tags: Vec::new(),
                    fields: item_render::entity::place_fields(&op),
                    parent: op.parent_id.map(NoteId::from_ulid),
                    id: Some(NoteId::from_ulid(op.id)),
                    created: op.created,
                    force: true,
                },
            )
            .await
            .expect("place");
        let local = device(&s, None, &Op::PlaceCreate(op.clone()));
        assert_eq!(
            (local.path.as_str(), local.content.as_str()),
            (view.path.as_str(), s.read(&view.path).as_str())
        );
        s.paths.insert(op.id, view.path);
        places.push(local);
    }
    assert_eq!(
        places[1].content,
        format!(
            "---\nid: {}\nkind: place\n{STAMP}part-of: [\"[[Nasr City office]]\"]\n---\n## Notes\n",
            ulid(0x301)
        )
    );

    // Documents: fields and relations (the copy points at the original).
    let original = ops::DocumentCreate {
        id: ulid(0x400),
        name: "Watanya contract".into(),
        aliases: vec!["عقد وطنية".into()],
        doc_type: Some("contract".into()),
        copy: Some(CopyKind::Original),
        copy_of: None,
        companies: vec![company.id],
        people: vec![person.id, person.id],
        expires: Some(date("2027-03-31")),
        created: monday(),
        force: true,
    };
    let copy = ops::DocumentCreate {
        id: ulid(0x401),
        name: "Watanya contract (copy)".into(),
        aliases: Vec::new(),
        doc_type: None,
        copy: Some(CopyKind::Copy),
        copy_of: Some(original.id),
        companies: Vec::new(),
        people: Vec::new(),
        expires: None,
        created: monday(),
        force: true,
    };
    for op in [&original, &copy] {
        let links = item_render::entity::document_relations(op)
            .into_iter()
            .map(|(rel, id)| (rel, NoteId::from_ulid(id)))
            .collect();
        let view = s
            .vault
            .create_entity_linked(
                &s.scope,
                NewEntity {
                    kind: NoteKind::Document,
                    name: op.name.clone(),
                    aliases: op.aliases.clone(),
                    tags: Vec::new(),
                    fields: item_render::entity::document_fields(op),
                    parent: None,
                    id: Some(NoteId::from_ulid(op.id)),
                    created: op.created,
                    force: true,
                },
                links,
            )
            .await
            .expect("document");
        let local = device(&s, None, &Op::DocumentCreate(op.clone()));
        assert_eq!(
            (local.path.as_str(), local.content.as_str()),
            (view.path.as_str(), s.read(&view.path).as_str())
        );
        s.paths.insert(op.id, view.path);
    }
    assert_eq!(
        s.state(original.id).content,
        format!(
            "---\nid: {id}\nkind: document\naliases: [عقد وطنية]\n\
             {STAMP}doc-type: contract\ncopy: original\n\
             expires: 2027-03-31\npeople: [\"[[Shady - ops]]\"]\ncompanies: [\"[[Watanya]]\"]\n---\n## Notes\n",
            id = original.id,
        )
    );

    // Custody events: the device's apply is the server's, byte for byte.
    for (kind, place, person_id, note) in [
        (CustodyEventType::StoredAt, Some(ulid(0x301)), None, None),
        (
            CustodyEventType::HandedTo,
            None,
            Some(person.id),
            Some("for the audit".to_owned()),
        ),
    ] {
        let op = ops::DocumentCustody {
            document_id: original.id,
            event: kind,
            at: date("2026-09-20"),
            place_id: place,
            person_id,
            counterparty_id: None,
            note: note.clone(),
        };
        let before = s.state(original.id);
        s.vault
            .add_custody_event(
                &s.scope,
                NoteId::from_ulid(original.id),
                NewCustodyEvent {
                    kind,
                    date: op.at,
                    place: place.map(NoteId::from_ulid),
                    person: person_id.map(NoteId::from_ulid),
                    counterparty: None,
                    source: None,
                    note,
                },
            )
            .await
            .expect("custody");
        let local = device(&s, Some(before), &Op::DocumentCustody(op));
        assert_eq!(local, s.state(original.id));
    }

    // Tasks: into a named note, and into `tasks/Tasks.md` (new, with the device's ID for it,
    // then existing).
    let home_id = ulid(0x5FF);
    let task = |n: u128, note_id: Option<Ulid>, text: &str| ops::TaskCreate {
        id: task_id(n),
        note_id,
        text: text.into(),
        due: Some(date("2026-10-01")),
        scheduled: None,
        start: None,
        recurrence: Some("every month on the 1st".into()),
        reminders: vec![
            NaiveDateTime::parse_from_str("2026-10-01 09:00", "%Y-%m-%d %H:%M").expect("dt"),
        ],
        priority: None,
        created: monday(),
        home_id: note_id.is_none().then_some(home_id),
        force: true,
    };
    let server_task = |op: &ops::TaskCreate| NewTask {
        text: op.text.clone(),
        due: op.due,
        scheduled: op.scheduled,
        start: op.start,
        recurrence: op.recurrence.clone(),
        reminders: op.reminders.clone(),
        priority: op.priority,
        note: op.note_id.map(NoteId::from_ulid),
        id: Some(op.id.clone()),
        created: op.created,
        home_id: op.home_id.map(NoteId::from_ulid),
        force: true,
    };
    let in_note = task(0x500, Some(company.id), "  Renew [[Watanya]] contract ");
    let before = s.state(company.id);
    s.vault
        .create_task(&s.scope, server_task(&in_note))
        .await
        .expect("task");
    let local = device(&s, Some(before), &Op::TaskCreate(in_note));
    assert_eq!(local, s.state(company.id));

    let first = task(0x501, None, "Make Watanya's ETA invoice");
    s.vault
        .create_task(&s.scope, server_task(&first))
        .await
        .expect("task");
    let local = device(&s, None, &Op::TaskCreate(first.clone()));
    assert_eq!(local.path, "tasks/Tasks.md");
    assert_eq!(
        local.content,
        format!(
            "---\nid: {home_id}\n{STAMP}---\n## September 2026\n- [ ] Make Watanya's ETA invoice (@2026-10-01 09:00) 🔁 every month on the 1st 📅 2026-10-01 ^{}\n",
            first.id
        )
    );
    assert_eq!(local.content, s.read("tasks/Tasks.md"));
    s.paths.insert(home_id, "tasks/Tasks.md".into());

    // The month heading is the creation date in the user's time zone: 22:30Z on 30 September
    // is 1 October in Cairo (the server's clock moves past it first; the op's time is used).
    s.db.clock.set(
        DateTime::parse_from_rfc3339("2026-10-02T09:00:00Z")
            .expect("ts")
            .to_utc(),
    );
    let second = ops::TaskCreate {
        created: DateTime::parse_from_rfc3339("2026-09-30T22:30:00Z")
            .expect("ts")
            .to_utc(),
        ..task(0x502, None, "Petrol Arrows invoice")
    };
    let before = s.state(home_id);
    s.vault
        .create_task(&s.scope, server_task(&second))
        .await
        .expect("task");
    let local = device(&s, Some(before), &Op::TaskCreate(second.clone()));
    assert_eq!(local, s.state(home_id));
    assert!(
        local.content.ends_with(&format!(
            "\n## October 2026\n- [ ] Petrol Arrows invoice (@2026-10-01 09:00) 🔁 every month on the 1st 📅 2026-10-01 ^{}\n",
            second.id
        )),
        "{}",
        local.content
    );

    // A plain note: `id`, and the device's creation time as `created`/`updated`.
    let note = ops::NoteCreate {
        id: ulid(0x600),
        path: "notes/Pricing experiments.md".into(),
        content: "---\ntags: [pricing]\n---\n# Pricing\n".into(),
        created: monday(),
        force: true,
    };
    let view = s
        .vault
        .create_note(
            &s.scope,
            CreateNote {
                path: note.path.clone(),
                content: note.content.clone(),
                created: note.created,
                id: Some(NoteId::from_ulid(note.id)),
                force: true,
            },
        )
        .await
        .expect("note");
    let local = device(&s, None, &Op::NoteCreate(note.clone()));
    assert_eq!(
        local.content,
        format!(
            "---\nid: {}\ntags: [pricing]\n{STAMP}---\n# Pricing\n",
            note.id
        )
    );
    assert_eq!(
        (local.path.as_str(), local.content.as_str()),
        (view.path.as_str(), s.read(&view.path).as_str())
    );

    s.finish().await;
}
