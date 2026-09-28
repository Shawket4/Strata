//! Pulling the server-only records (PLAN §12.4): relation provenance, rejected edges, map
//! clusters and their names, user and device settings and "keep both" pairs arrive in the
//! bootstrap snapshot and in change pages, and their tombstones remove them again; and the
//! device-side rebase of queued ops when a pull brings a new server version (D19): a
//! conflicting edit stays as queued, other ops get the new base, ops waiting for the user are
//! replayed first. Against the fake server; asserted on what the screens show and on the
//! outbox.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use chrono::DateTime;
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::session::Session;
use strata_core::store::outbox::{self, OpStatus};
use strata_core::store::settings;
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::{ConflictResolution, EntityType, Op, OpResult, Record, Version};
use strata_core::view::build;
use sync_model::changes::{
    ClusterAssignmentRecord, ClusterNameRecord, DeviceSettingRecord, RejectedRecord,
    RelationRecord, SettingRecord,
};
use ulid::Ulid;
use vault_format::RelationKey;

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const ACME: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";
const OTHER: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1D";
/// The device the fake account API hands out at sign-in.
const DEVICE: &str = "01K5DSSE00000000000000DEV1";
const OTHER_DEVICE: &str = "01K5DSSE00000000000000DEV2";

fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("ulid")
}

fn table(s: &Session, sql: &str) -> Vec<String> {
    s.read(|c, _| {
        let mut st = c.prepare(sql)?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .expect("query")
}

/// `(rel_type, target title, by, confidence, reason)` of the note's relation chips.
/// `(rel_type, target title, by, confidence, reason)`.
type Chip = (String, String, String, Option<f64>, Option<String>);

fn chips(s: &Session) -> Vec<Chip> {
    s.read(|c, ctx| build::note_screen(c, ctx, NOTE))
        .expect("note")
        .note
        .expect("exists")
        .relations
        .into_iter()
        .map(|r| (r.rel_type, r.target.title, r.by, r.confidence, r.reason))
        .collect()
}

fn keep_both(kind: dedupe::DedupeKind, a: &str, b: &str) -> Record {
    Record::KeepBoth(dedupe::KeepBoth::new(kind, a, b))
}

const SUGGESTION: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V9S";

fn filing_suggestion() -> Record {
    use sync_model::suggestions::{FilingPayload, SuggestionPayload};
    let p = SuggestionPayload::Filing(FilingPayload {
        decision_id: Ulid::from_parts(1_790_000_000_001, 1),
        title: "Q4 pricing".into(),
        tags: vec!["pricing".into()],
        folder: "notes".into(),
    });
    Record::Suggestion(sync_model::changes::SuggestionRecord {
        id: ulid(SUGGESTION),
        note_id: Some(ulid(NOTE)),
        kind: p.kind().into(),
        status: sync_model::changes::SuggestionStatus::Pending,
        payload: p.to_bytes(),
        created: DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts"),
        replies: Vec::new(),
    })
}

#[tokio::test]
async fn server_only_records_arrive_and_their_tombstones_remove_them() {
    let h = Harness::new();
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing.md",
        &format!("---\nid: {NOTE}\nrelated: [\"[[Acme]]\"]\n---\nQ4 prices.\n"),
    );
    h.server.remote_upsert(
        ACME,
        "companies/Acme.md",
        &format!("---\nid: {ACME}\nkind: company\n---\n"),
    );
    h.server.remote_upsert(
        OTHER,
        "companies/Acme Trading.md",
        &format!("---\nid: {OTHER}\nkind: company\n---\n"),
    );
    let relation = RelationRecord {
        src_id: ulid(NOTE),
        dst_id: ulid(ACME),
        relation: RelationKey::Note(domain::RelationType::Related),
        by: domain::RelationOrigin::Ai,
        confidence: Some(0.75),
        reason: Some("Both are about Acme's Q4 prices.".into()),
        created: Some(DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts")),
    };
    let rejected = RejectedRecord {
        src_id: ulid(NOTE),
        dst_id: ulid(OTHER),
        relation: RelationKey::Note(domain::RelationType::Related),
        at: DateTime::parse_from_rfc3339("2026-09-27T12:30:00+03:00").expect("ts"),
    };
    for r in [
        Record::Relation(relation.clone()),
        Record::Rejected(rejected.clone()),
        Record::ClusterAssignment(ClusterAssignmentRecord {
            note_id: ulid(NOTE),
            cluster_id: "c1".into(),
        }),
        Record::ClusterName(ClusterNameRecord {
            cluster_id: "c1".into(),
            name: "Pricing".into(),
        }),
        Record::Setting(SettingRecord {
            key: "digest".into(),
            value: "weekly".into(),
        }),
        Record::DeviceSetting(DeviceSettingRecord {
            device_id: ulid(DEVICE),
            key: settings::SNOOZE_MINUTES.into(),
            value: "25".into(),
        }),
        // Another device's setting is not this device's business.
        Record::DeviceSetting(DeviceSettingRecord {
            device_id: ulid(OTHER_DEVICE),
            key: settings::QUIET_FROM.into(),
            value: "21:00".into(),
        }),
        keep_both(dedupe::DedupeKind::Company, OTHER, ACME),
        filing_suggestion(),
    ] {
        h.server.remote_record(r);
    }
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");

    assert_eq!(
        chips(&s),
        [(
            "related".to_owned(),
            "Acme".to_owned(),
            "ai".to_owned(),
            Some(f64::from(0.75_f32)),
            Some("Both are about Acme's Q4 prices.".to_owned())
        )]
    );
    assert_eq!(
        table(
            &s,
            "SELECT src_id || ' ' || rel_type || ' ' || dst_id || ' ' || at FROM rejected"
        ),
        [format!("{NOTE} related {OTHER} 2026-09-27T12:30:00+03:00")]
    );
    assert_eq!(s.read(build::nav).expect("nav").cluster_count, 1);
    assert_eq!(
        table(
            &s,
            "SELECT c.note_id || ' ' || n.name FROM clusters c JOIN cluster_names n USING (cluster_id)"
        ),
        [format!("{NOTE} Pricing")]
    );
    assert_eq!(
        s.read(|c, _| settings::user_setting(c, "digest")),
        Ok(Some("weekly".to_owned()))
    );
    let reminders = || {
        s.read(build::settings_view)
            .expect("settings")
            .expect("signed in")
            .reminders
    };
    assert_eq!(
        (reminders().snooze_minutes, reminders().quiet_from),
        (25, "22:00".to_owned())
    );
    assert_eq!(
        s.read(|c, ctx| build::suggestion_items(c, ctx, false))
            .expect("items")
            .iter()
            .map(|i| (i.id.as_str(), i.detail.title.as_str()))
            .collect::<Vec<_>>(),
        [(SUGGESTION, "Q4 pricing")]
    );
    assert_eq!(
        table(
            &s,
            "SELECT kind || ' ' || a_id || ' ' || b_id FROM keep_both"
        ),
        [format!("company {ACME} {OTHER}")]
    );

    // A change page carrying every kind of tombstone (and two that do not concern this
    // device or do not parse).
    let relation_key = sync_model::changes::relation_key(
        ulid(NOTE),
        RelationKey::Note(domain::RelationType::Related),
        ulid(ACME),
    );
    let rejected_key = sync_model::changes::relation_key(
        ulid(NOTE),
        RelationKey::Note(domain::RelationType::Related),
        ulid(OTHER),
    );
    for (t, id) in [
        (EntityType::Relation, relation_key.as_str()),
        (EntityType::Rejected, rejected_key.as_str()),
        (EntityType::ClusterAssignment, NOTE),
        (EntityType::ClusterName, "c1"),
        (EntityType::Setting, "digest"),
        (
            EntityType::DeviceSetting,
            &format!("{OTHER_DEVICE}:{}", settings::SNOOZE_MINUTES),
        ),
        (EntityType::KeepBoth, &format!("company:{ACME}:{OTHER}")),
        (EntityType::KeepBoth, "company"),
        (EntityType::Suggestion, SUGGESTION),
    ] {
        h.server.remote_tombstone(t, id);
    }
    s.pull().await.expect("pull");
    // The relation stays (it is in the frontmatter) but loses its provenance: by the user.
    assert_eq!(
        chips(&s),
        [(
            "related".to_owned(),
            "Acme".to_owned(),
            "user".to_owned(),
            None,
            None
        )]
    );
    assert_eq!(
        table(&s, "SELECT src_id FROM rejected"),
        Vec::<String>::new()
    );
    assert_eq!(
        table(&s, "SELECT note_id FROM clusters"),
        Vec::<String>::new()
    );
    assert_eq!(s.read(build::nav).expect("nav").cluster_count, 0);
    assert_eq!(s.read(|c, _| settings::user_setting(c, "digest")), Ok(None));
    assert_eq!(reminders().snooze_minutes, 25, "another device's key");
    assert_eq!(
        table(&s, "SELECT a_id FROM keep_both"),
        Vec::<String>::new()
    );
    assert_eq!(
        s.read(|c, ctx| build::suggestion_items(c, ctx, false))
            .expect("items"),
        []
    );

    // This device's own setting tombstone resets it to the default.
    h.server.remote_tombstone(
        EntityType::DeviceSetting,
        &format!("{DEVICE}:{}", settings::SNOOZE_MINUTES),
    );
    s.pull().await.expect("pull");
    assert_eq!(reminders().snooze_minutes, settings::DEFAULT_SNOOZE_MINUTES);
}

#[tokio::test]
async fn a_malformed_relation_tombstone_fails_the_pull_without_losing_state() {
    let h = Harness::new();
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing.md",
        &format!("---\nid: {NOTE}\n---\nQ4 prices.\n"),
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    h.server
        .remote_tombstone(EntityType::Relation, &format!("{NOTE}:related"));
    assert_eq!(
        s.pull().await.map(|_| ()),
        Err(strata_core::CoreError::Storage(format!(
            "bad relation key {NOTE}:related"
        )))
    );
    assert_eq!(
        table(&s, "SELECT id FROM notes"),
        [NOTE.to_owned()],
        "the failed page rolled back as a whole"
    );
}

const BASE: &str = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1B\n---\nFirst.\nSecond.\nThird.\n";

fn update_of(op: &Op) -> String {
    match op {
        Op::NoteUpdate(u) => u.content.clone(),
        other => panic!("not an update: {other:?}"),
    }
}

async fn pulled_note() -> (Harness, std::sync::Arc<Session>) {
    let h = Harness::new();
    h.server.remote_upsert(NOTE, "notes/Pricing.md", BASE);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    (h, s)
}

#[tokio::test]
async fn a_queued_edit_that_conflicts_with_the_pulled_version_stays_as_queued() {
    let (h, s) = pulled_note().await;
    let mine = BASE.replace("Second.", "Second (mine).");
    s.update_note(NOTE, &mine).expect("edit");
    let before = s.read(|c, _| outbox::all(c)).expect("outbox");
    let theirs = BASE.replace("Second.", "Second (theirs).");
    h.server.remote_upsert(NOTE, "notes/Pricing.md", &theirs);

    s.pull().await.expect("pull");

    let after = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(after, before, "left for the server's conflict answer");
    assert_eq!(update_of(&after[0].op), mine);
    assert_eq!(after[0].base_version, Some(Version::of_text(BASE)));
    let local = s
        .read(|c, _| strata_core::store::notes::current(c, NOTE))
        .expect("note")
        .expect("exists");
    assert_eq!(local.content, mine, "the user keeps seeing their edit");
}

#[tokio::test]
async fn other_queued_ops_get_the_base_of_the_pulled_version() {
    const PERSON: &str = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1C\nkind: person\n---\nMet at Acme.\n";
    let h = Harness::new();
    h.server.remote_upsert(ACME, "people/Mona Adel.md", PERSON);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    s.add_alias(ACME, "Mona").expect("alias");
    let before = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].base_version, Some(Version::of_text(PERSON)));
    let theirs = PERSON.replace("Met at Acme.", "Met at Acme in May.");
    h.server.remote_upsert(ACME, "people/Mona Adel.md", &theirs);

    s.pull().await.expect("pull");

    let after = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].op, before[0].op, "the payload is unchanged");
    assert_eq!(after[0].base_version, Some(Version::of_text(&theirs)));
    let local = s
        .read(|c, _| strata_core::store::notes::current(c, ACME))
        .expect("note")
        .expect("exists");
    assert_eq!(
        local.content,
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1C\nkind: person\naliases: [Mona]\n---\nMet at Acme in May.\n"
    );
    s.sync(Trigger::Manual).await.expect("push");
    assert_eq!(
        h.server.notes()[&ulid(ACME)].content,
        local.content,
        "the push applies on the server's version"
    );
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), []);
}

#[tokio::test]
async fn ops_waiting_for_the_user_are_replayed_before_the_pending_ones() {
    let (h, s) = pulled_note().await;
    // The first edit comes back as a conflict (the server kept its version) and waits for
    // the user; a second edit is queued on top of it.
    let first = BASE.replace("First.", "First (mine).");
    let conflict_op = s.update_note(NOTE, &first).expect("edit 1");
    h.server.script(
        NOTE,
        OpResult::Conflict {
            server_version: Some(Version::of_text(BASE)),
            resolution: ConflictResolution::ServerKept {
                reason: "overlapping_edits".into(),
            },
        },
    );
    s.sync(Trigger::AfterWrite).await.expect("push");
    let second = first.replace("Second.", "Second (mine).");
    s.update_note(NOTE, &second).expect("edit 2");
    let before = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(
        before
            .iter()
            .map(|o| (o.op_id.as_str(), o.status))
            .collect::<Vec<_>>()[0],
        (conflict_op.as_str(), OpStatus::Conflict)
    );
    assert_eq!(before[1].status, OpStatus::Pending);

    // The server moves the note: same content, new path.
    h.server
        .remote_upsert(NOTE, "notes/Archive/Pricing.md", BASE);
    s.pull().await.expect("pull");

    let after = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(
        after, before,
        "the pending edit was made against the replayed state"
    );
    let conflict = s
        .read(|c, ctx| build::conflict_screen(c, ctx, &conflict_op))
        .expect("conflict")
        .conflict
        .expect("open");
    assert_eq!(
        (
            conflict.path.as_str(),
            conflict.server.as_deref(),
            conflict.local.as_deref(),
            conflict.server_origin_label.as_str(),
            conflict.conflict_copy_path
        ),
        (
            "notes/Archive/Pricing.md",
            Some(BASE),
            Some(first.as_str()),
            "Server",
            None
        )
    );
}
