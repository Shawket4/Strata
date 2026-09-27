//! The sync engine against the fake server (PLAN §12.4, §16.4): bootstrap paging, incremental
//! pulls, epoch change → re-bootstrap, every push result kind (applied, merged, conflict,
//! duplicate, rejected), replay idempotency, offline backoff, and crash safety at every step
//! (the database is dropped and reopened at each step boundary; proptest over crash points).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::collections::BTreeMap;

use chrono::DateTime;
use common::{Harness, seq_id};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use strata_core::net::NetError;
use strata_core::session::Session;
use strata_core::store::{notes, outbox, sync_state};
use strata_core::sync::engine::{CycleOutcome, CycleReport, Step, Trigger, backoff_delay};
use strata_core::sync::model::{
    OpResult, Problem, Record, SuggestionRecord, SuggestionStatus, Version,
};
use strata_core::view::hub::Recorder;
use strata_core::view::model::{
    ConflictResolution, Connectivity, DuplicateChoice, NoteSyncKind, ResolutionKind, SyncPhase,
};
use strata_core::view::{Topics, build};
use ulid::Ulid;

const N1: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1A";
const N2: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const N3: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";

fn synced(pushed: u32, pulled: u32, bootstrapped: bool) -> CycleReport {
    CycleReport {
        pushed,
        pulled,
        bootstrapped,
        outcome: CycleOutcome::Synced,
    }
}

fn local_notes(s: &Session) -> BTreeMap<String, (String, String, Option<String>)> {
    s.read(|c, _| {
        let mut out = BTreeMap::new();
        for (id, _) in notes::live_paths(c)? {
            let n = notes::get(c, &id)?.expect("row");
            out.insert(id, (n.path, n.content, n.base_version));
        }
        Ok(out)
    })
    .expect("notes")
}

fn server_view(h: &Harness) -> BTreeMap<String, (String, String, Option<String>)> {
    h.server
        .notes()
        .into_iter()
        .map(|(id, n)| {
            let v = n.version().as_str().to_owned();
            (id.to_string(), (n.path, n.content, Some(v)))
        })
        .collect()
}

fn seed(h: &Harness) {
    h.server.remote_upsert(
        N1,
        "notes/Churn notes.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1A\n---\nChurn is up.\n",
    );
    h.server.remote_upsert(
        N2,
        "notes/Pricing experiments.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1B\n---\nTry 5% off. [[Churn notes]]\nSecond line.\nThird line.\n",
    );
    h.server.remote_upsert(
        N3,
        "people/Ahmed Samir.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1C\nkind: person\naliases: [أحمد سمير]\n---\n",
    );
}

fn suggestion() -> Record {
    Record::Suggestion(SuggestionRecord {
        id: Ulid::from_string("01J8ZKS0000000000000000001").expect("id"),
        note_id: None,
        kind: "task".into(),
        status: SuggestionStatus::Pending,
        payload: rmp_serde::to_vec_named(&strata_core::sync::model::SuggestionPayload::Task {
            line: "- [ ] Call Ahmed".into(),
        })
        .expect("payload"),
        created: DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts"),
        replies: Vec::new(),
    })
}

#[tokio::test]
async fn bootstrap_pages_the_snapshot_then_pulls_incrementally() {
    let h = Harness::new();
    seed(&h);
    h.server.remote_record(suggestion());
    let s = h.sign_in_a().await;
    let pill = Recorder::new();
    s.watch(Topics::SYNC, build::sync_pill, pill.clone())
        .expect("watch");

    let report = s.sync(Trigger::Start).await.expect("sync");

    assert_eq!(report, synced(0, 4, true));
    assert_eq!(h.server.bootstrap_calls(), 2, "4 records, pages of 2");
    assert_eq!(local_notes(&s), server_view(&h));
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!(
        (
            state.epoch,
            state.cursor_seq,
            state.bootstrap_complete,
            state.bootstrap_cursor
        ),
        (Some(1), 4, true, None)
    );
    assert_eq!(
        state.last_pull_at.as_deref(),
        Some("2026-09-27T10:00:00+00:00")
    );
    // The first incremental pull asks from the snapshot's seq.
    assert_eq!(h.server.changes_calls(), vec![(4, 1)]);
    // The pill went Idle → Bootstrapping (pages 0, 1, 2) → Pulling → Idle, and Online.
    let phases: Vec<(SyncPhase, u32, Connectivity, bool)> = pill
        .take()
        .into_iter()
        .map(|p| {
            (
                p.activity.phase,
                p.activity.pages_done,
                p.connectivity,
                p.last_sync_at.is_some(),
            )
        })
        .collect();
    assert_eq!(
        phases,
        vec![
            (SyncPhase::Idle, 0, Connectivity::Unknown, false),
            (SyncPhase::Bootstrapping, 0, Connectivity::Unknown, false),
            (SyncPhase::Bootstrapping, 0, Connectivity::Online, false),
            (SyncPhase::Bootstrapping, 1, Connectivity::Online, false),
            (SyncPhase::Bootstrapping, 2, Connectivity::Online, false),
            (SyncPhase::Pulling, 0, Connectivity::Online, false),
            (SyncPhase::Idle, 0, Connectivity::Online, false),
            (SyncPhase::Idle, 0, Connectivity::Online, true),
        ]
    );
    // Derived tables come from the content (vault-format), e.g. the entity and its alias.
    let dir = s
        .read(|c, _| build::directory(c, strata_core::view::model::DirectoryTab::People, "احمد"))
        .expect("directory");
    assert_eq!(
        dir.items
            .iter()
            .map(|i| (i.id.as_str(), i.title.as_str()))
            .collect::<Vec<_>>(),
        vec![(N3, "Ahmed Samir")]
    );
    let inbox = s.read(|c, _| build::inbox(c)).expect("inbox");
    assert_eq!(inbox.suggestions.len(), 1);
    assert_eq!(inbox.suggestions[0].detail.line, "- [ ] Call Ahmed");

    // Incremental: another device edits and deletes.
    h.server.remote_upsert(
        N1,
        "notes/Churn notes.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1A\n---\nChurn is down.\n",
    );
    h.server.remote_delete(N3);
    let report = s.sync(Trigger::EventsFrame).await.expect("sync");
    assert_eq!(report, synced(0, 2, false));
    assert_eq!(local_notes(&s), server_view(&h));
    assert_eq!(h.server.changes_calls(), vec![(4, 1), (4, 1)]);
    assert_eq!(
        s.read(|c, _| sync_state::get(c)).expect("state").cursor_seq,
        6
    );
    // Replaying the same pull is a no-op.
    let report = s.sync(Trigger::Timer).await.expect("sync");
    assert_eq!(report, synced(0, 0, false));
    assert_eq!(h.server.changes_calls().last(), Some(&(6, 1)));
}

#[tokio::test]
async fn epoch_change_rebootstraps_and_keeps_live_local_ops() {
    let h = Harness::new();
    seed(&h);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    // A local create the server flags as a duplicate stays live (awaiting the user) across
    // the re-bootstrap.
    let created = s
        .create_note("notes/Onboarding checklist v2.md", "Steps.\n", false)
        .expect("create");
    let local_id = created.id.expect("created");
    h.server.script(
        &local_id,
        OpResult::Duplicate {
            candidates: Vec::new(),
        },
    );
    // The server rebuilds: N3 is gone from the new snapshot, N1 changed.
    h.server.bump_epoch();
    h.server.remote_delete(N3);
    h.server.remote_upsert(
        N1,
        "notes/Churn notes.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1A\n---\nRebuilt.\n",
    );

    let report = s.sync(Trigger::Resume).await.expect("sync");

    assert_eq!(report, synced(1, 2, true));
    assert_eq!(h.server.bootstrap_calls(), 2 + 1, "one page each time");
    // The old epoch is refused (410), the new snapshot continues from the server's seq.
    assert_eq!(h.server.changes_calls(), vec![(3, 1), (3, 1), (5, 2)]);
    let mut expected = server_view(&h);
    let local = local_notes(&s);
    let (path, content, base) = local.get(&local_id).cloned().expect("still visible");
    assert_eq!(
        (path.as_str(), base),
        ("notes/Onboarding checklist v2.md", None)
    );
    assert_eq!(content, format!("---\nid: {local_id}\n---\nSteps.\n"));
    expected.insert(local_id.clone(), (path, content, None));
    assert_eq!(local, expected);
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!((state.epoch, state.bootstrap_complete), (Some(2), true));
    assert_eq!(
        s.read(|c, _| build::duplicate_prompts(c))
            .expect("prompts")
            .prompts
            .len(),
        1
    );
}

#[tokio::test]
async fn applied_creates_become_the_verified_base() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let id = s
        .create_note("notes/Q4 hiring plan.md", "Hire two.\n", false)
        .expect("create")
        .id
        .expect("id");

    let report = s.sync(Trigger::AfterWrite).await.expect("sync");

    assert_eq!(report, synced(1, 1, false));
    let content = format!("---\nid: {id}\n---\nHire two.\n");
    assert_eq!(
        local_notes(&s).get(&id),
        Some(&(
            "notes/Q4 hiring plan.md".to_owned(),
            content.clone(),
            Some(Version::of_text(&content).as_str().to_owned())
        ))
    );
    assert_eq!(local_notes(&s), server_view(&h));
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), Vec::new());
    assert_eq!(
        h.server.applied_ops(),
        vec![Ulid::from_string(&seq_id(2)).expect("op")]
    );
    let pushed: Vec<(String, Option<Version>)> = h
        .server
        .pushes()
        .concat()
        .into_iter()
        .map(|o| (o.entity_id, o.base_version))
        .collect();
    assert_eq!(pushed, vec![(id, None)]);
}

#[tokio::test]
async fn stale_edits_merge_or_conflict_per_d19() {
    let h = Harness::new();
    seed(&h);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let base = h.server.notes()[&Ulid::from_string(N2).expect("id")]
        .content
        .clone();

    // Non-overlapping: another device edits the third line, we edit the first.
    h.server.remote_upsert(
        N2,
        "notes/Pricing experiments.md",
        &base.replace("Third line.", "Third line, edited remotely."),
    );
    s.update_note(N2, &base.replace("Try 5% off.", "Try 10% off."))
        .expect("edit");
    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report, synced(1, 2, false));
    let merged = base
        .replace("Try 5% off.", "Try 10% off.")
        .replace("Third line.", "Third line, edited remotely.");
    assert_eq!(
        local_notes(&s).get(N2).map(|n| n.1.clone()),
        Some(merged.clone())
    );
    assert_eq!(local_notes(&s), server_view(&h));

    // Overlapping: both change the same line.
    h.server.remote_upsert(
        N2,
        "notes/Pricing experiments.md",
        &merged.replace("Second line.", "Second line (theirs)."),
    );
    let mine = merged.replace("Second line.", "Second line (mine).");
    let op = s.update_note(N2, &mine).expect("edit");
    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report, synced(1, 1, false));

    // The local edit stays visible, the note is marked conflicting, and the preview uses
    // sync-model's merge with the pulled server content.
    assert_eq!(
        local_notes(&s).get(N2).map(|n| n.1.clone()),
        Some(mine.clone())
    );
    let note = s
        .read(|c, ctx| build::note_screen(c, ctx, N2))
        .expect("note")
        .note
        .expect("exists");
    assert_eq!(note.sync.kind, NoteSyncKind::Conflict);
    assert_eq!(note.sync.conflict_op_id.as_deref(), Some(op.as_str()));
    let screen = s
        .read(|c, _| build::conflict_screen(c, &op))
        .expect("conflict");
    let detail = screen.conflict.expect("conflict exists");
    let theirs = merged.replace("Second line.", "Second line (theirs).");
    assert_eq!(detail.base.as_deref(), Some(merged.as_str()));
    assert_eq!(detail.local.as_deref(), Some(mine.as_str()));
    assert_eq!(detail.server.as_deref(), Some(theirs.as_str()));
    assert_eq!(detail.merge_clean, Some(false));
    assert_eq!(detail.hunks.len(), 1);
    assert_eq!(
        (
            detail.hunks[0].ours.as_str(),
            detail.hunks[0].theirs.as_str()
        ),
        ("Second line (mine).\n", "Second line (theirs).\n")
    );
    let status = s.read(build::sync_status).expect("status");
    assert_eq!(status.pill.conflicts, 1);
    assert_eq!(status.conflicts[0].op_id, op);

    // Keep mine: a new update over the server's version, which applies.
    s.resolve_conflict(
        &op,
        ConflictResolution {
            kind: ResolutionKind::KeepMine,
            content: None,
            choices: Vec::new(),
        },
    )
    .expect("resolve");
    let pending = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].base_version, Some(Version::of_text(&theirs)));
    let report = s.sync(Trigger::Manual).await.expect("sync");
    assert_eq!(report, synced(1, 1, false));
    assert_eq!(local_notes(&s).get(N2).map(|n| n.1.clone()), Some(mine));
    assert_eq!(local_notes(&s), server_view(&h));
    assert_eq!(
        s.read(|c, _| build::conflict_screen(c, &op))
            .expect("screen")
            .conflict,
        None
    );
}

#[tokio::test]
async fn duplicate_result_holds_later_ops_until_the_user_decides() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let id = s
        .create_note("notes/Weekly invoicing proposal.md", "Draft.\n", true)
        .expect("create")
        .id
        .expect("id");
    let candidate = dedupe::DuplicateCandidate {
        id: N2.into(),
        kind: domain::DedupeKind::Note,
        title: "Weekly invoicing".into(),
        snippet: Some("Weekly invoicing for Acme".into()),
        level: domain::MatchLevel::Near,
        score: 0.75,
    };
    h.server.script(
        &id,
        OpResult::Duplicate {
            candidates: vec![candidate],
        },
    );
    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report, synced(1, 0, false));

    // A later edit of the same note is not pushed while the prompt is open.
    s.update_note(&id, &format!("---\nid: {id}\n---\nDraft 2.\n"))
        .expect("edit");
    let report = s.sync(Trigger::AfterWrite).await.expect("sync");
    assert_eq!(report, synced(0, 0, false));
    assert_eq!(h.server.pushes().len(), 1);

    let prompts = s.read(|c, _| build::duplicate_prompts(c)).expect("prompts");
    assert_eq!(prompts.prompts.len(), 1);
    let p = &prompts.prompts[0];
    assert_eq!(
        (p.op_id.as_str(), p.kind.as_str(), p.title.as_str()),
        (seq_id(2).as_str(), "note", "Weekly invoicing proposal")
    );
    assert_eq!(
        (
            p.candidates[0].id.as_str(),
            p.candidates[0].match_level.as_str(),
            p.candidates[0].score
        ),
        (N2, "near", 0.75)
    );

    s.resolve_duplicate(&seq_id(2), DuplicateChoice::CreateAnyway)
        .expect("create anyway");
    let report = s.sync(Trigger::Manual).await.expect("sync");
    assert_eq!(report, synced(2, 2, false));
    let resent: Vec<(String, bool)> = h.server.pushes()[1]
        .iter()
        .map(|o| (o.op_id.to_string(), o.op.force()))
        .collect();
    assert_eq!(
        resent,
        vec![(seq_id(4), true), (seq_id(3), false)],
        "the forced create under a new op ID, then the held-back edit"
    );
    assert_eq!(local_notes(&s), server_view(&h));
    assert_eq!(
        local_notes(&s).get(&id).map(|n| n.1.clone()),
        Some(format!("---\nid: {id}\n---\nDraft 2.\n"))
    );
}

#[tokio::test]
async fn rejected_ops_roll_back_and_leave_a_typed_notice() {
    let h = Harness::new();
    seed(&h);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let before = local_notes(&s);
    let op = s
        .update_note(
            N1,
            "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1A\n---\nLocal edit.\n",
        )
        .expect("edit");
    h.server.script(
        N1,
        OpResult::Rejected {
            problem: Problem {
                problem_type: "not_found".into(),
                title: "Not found".into(),
                status: 404,
                detail: None,
            },
        },
    );

    let report = s.sync(Trigger::AfterWrite).await.expect("sync");

    assert_eq!(report, synced(1, 0, false));
    assert_eq!(local_notes(&s), before, "optimistic change rolled back");
    let status = s.read(build::sync_status).expect("status");
    assert_eq!(status.pill.pending_ops, 0);
    assert_eq!(status.rejections.len(), 1);
    let r = &status.rejections[0];
    assert_eq!(
        (
            r.op_id.as_str(),
            r.kind.as_str(),
            r.problem_type.as_str(),
            r.message_key.as_str()
        ),
        (op.as_str(), "note.update", "not_found", "error.not_found")
    );
    s.dismiss_rejection(&op).expect("dismiss");
    assert_eq!(
        s.read(build::sync_status).expect("status").rejections,
        Vec::new()
    );
}

#[tokio::test]
async fn offline_cycles_keep_the_outbox_and_back_off() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.capture("Mona said Nile Freight rates go up 8% in November")
        .expect("capture");
    h.server.set_offline(true);

    let report = s.sync(Trigger::AfterWrite).await.expect("sync");

    assert_eq!(
        report.outcome,
        CycleOutcome::Failed(NetError::Offline("fake".into()))
    );
    let status = s.read(build::sync_status).expect("status");
    assert_eq!(status.pill.connectivity, Connectivity::Offline);
    assert_eq!(status.pill.pending_ops, 1);
    assert_eq!(status.last_error.as_deref(), Some("error.offline"));
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!(state.consecutive_failures, 1);
    assert_eq!(backoff_delay(state.consecutive_failures).as_secs(), 2);
    let ops = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(ops[0].status, outbox::OpStatus::Pending);
    assert_eq!(ops[0].attempts, 1);

    s.sync(Trigger::Retry).await.expect("sync");
    assert_eq!(
        s.read(|c, _| sync_state::get(c))
            .expect("state")
            .consecutive_failures,
        2
    );

    h.server.set_offline(false);
    let report = s.sync(Trigger::Retry).await.expect("sync");
    assert_eq!(report, synced(1, 1, true));
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!((state.consecutive_failures, state.last_error), (0, None));
    assert_eq!(local_notes(&s), server_view(&h));
}

#[tokio::test]
async fn replays_after_a_lost_response_are_idempotent() {
    let mut h = Harness::new();
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    s.capture("Idea: loyalty tier for customers > 12 months")
        .expect("capture");
    let mut engine = s.engine().clone();
    engine.crash_after = Some(Step::Pushed);
    let report = s
        .sync_with(&engine, Trigger::AfterWrite)
        .await
        .expect("sync");
    assert_eq!(report.outcome, CycleOutcome::Crashed(Step::Pushed));
    drop(s);

    h.restart();
    let s = h.core.session().expect("session survives");
    let ops = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(ops.len(), 1);
    assert_eq!(
        ops[0].status,
        outbox::OpStatus::Pending,
        "inflight requeued at open"
    );
    let report = s.sync(Trigger::Start).await.expect("sync");
    assert_eq!(report, synced(1, 1, false));
    let pushes = h.server.pushes();
    assert_eq!(pushes.len(), 2);
    assert_eq!(
        pushes[0][0].op_id, pushes[1][0].op_id,
        "same op ID replayed"
    );
    assert_eq!(h.server.applied_ops().len(), 1, "applied once");
    assert_eq!(local_notes(&s), server_view(&h));
}

// --- Crash safety: proptest over crash points --------------------------------------------

#[derive(Debug, Clone)]
enum Action {
    Capture(u8),
    Create(u8),
    EditPricing(u8),
    RemoteEdit(u8),
    /// The server rebuilds its index (epoch change → re-bootstrap).
    Rebuild,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0u8..5).prop_map(Action::Capture),
        (0u8..5).prop_map(Action::Create),
        (0u8..5).prop_map(Action::EditPricing),
        (0u8..5).prop_map(Action::RemoteEdit),
        Just(Action::Rebuild),
    ]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        Just(Step::MarkedInflight),
        Just(Step::Pushed),
        Just(Step::Recorded),
        Just(Step::BootstrapFetched),
        Just(Step::BootstrapApplied),
        Just(Step::ChangesFetched),
        Just(Step::ChangesApplied),
    ]
}

async fn apply(h: &Harness, s: &Session, a: &Action) {
    match a {
        Action::Capture(n) => {
            s.capture(&format!("capture {n}")).expect("capture");
        }
        Action::Create(n) => {
            s.create_note(&format!("notes/Note {n}.md"), &format!("Body {n}.\n"), true)
                .expect("create");
        }
        Action::EditPricing(n) => {
            // Only this device edits N2, so its final content does not depend on timing.
            let content = s
                .read(|c, _| Ok(notes::current(c, N2)?.expect("note").content))
                .expect("content");
            s.update_note(N2, &format!("{content}edit {n}\n"))
                .expect("edit");
        }
        Action::Rebuild => h.server.bump_epoch(),
        Action::RemoteEdit(n) => {
            h.server.remote_upsert(
                N1,
                "notes/Churn notes.md",
                &format!("---\nid: {N1}\n---\nRemote {n}.\n"),
            );
        }
    }
}

/// Runs `actions` with a sync after each, crashing (drop + reopen of every database) at
/// `crash` during the first sync that reaches it; returns the converged local and server
/// state.
async fn run(
    actions: &[Action],
    crash: Option<Step>,
) -> (
    BTreeMap<String, (String, String, Option<String>)>,
    BTreeMap<String, (String, String, Option<String>)>,
) {
    let mut h = Harness::new();
    seed(&h);
    let mut s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let mut crashed = false;
    for a in actions {
        apply(&h, &s, a).await;
        let mut engine = s.engine().clone();
        if !crashed {
            engine.crash_after = crash;
        }
        let report = s
            .sync_with(&engine, Trigger::AfterWrite)
            .await
            .expect("sync");
        if let CycleOutcome::Crashed(_) = report.outcome {
            crashed = true;
            drop(s);
            h.restart();
            s = h.core.session().expect("session");
        }
    }
    // Converge.
    for _ in 0..3 {
        s.sync(Trigger::Manual).await.expect("final sync");
    }
    let outbox_left = s.read(|c, _| outbox::unsynced_count(c)).expect("count");
    assert_eq!(outbox_left, 0, "every op pushed");
    (local_notes(&s), server_view(&h))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn crashes_at_any_step_never_lose_or_duplicate_ops(
        actions in proptest::collection::vec(action(), 1..6),
        crash in step(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("rt");
        let (local, server) = rt.block_on(run(&actions, Some(crash)));
        // The device converges to the server's state …
        prop_assert_eq!(&local, &server);
        // … which is exactly the state of the same history without a crash.
        let (clean_local, clean_server) = rt.block_on(run(&actions, None));
        prop_assert_eq!(server, clean_server);
        prop_assert_eq!(local, clean_local);
    }
}

#[tokio::test]
async fn a_crash_mid_bootstrap_resumes_from_the_saved_cursor() {
    for (step, calls_before_crash, calls_after) in [
        (Step::BootstrapFetched, 1, 2 + 1),
        (Step::BootstrapApplied, 1, 1 + 1),
    ] {
        let mut h = Harness::new();
        seed(&h);
        h.server.remote_record(suggestion());
        let s = h.sign_in_a().await;
        let mut engine = s.engine().clone();
        engine.crash_after = Some(step);
        let report = s.sync_with(&engine, Trigger::Start).await.expect("sync");
        assert_eq!(report.outcome, CycleOutcome::Crashed(step));
        assert_eq!(h.server.bootstrap_calls(), calls_before_crash);
        drop(s);

        h.restart();
        let s = h.core.session().expect("session");
        let report = s.sync(Trigger::Start).await.expect("sync");
        assert_eq!(report.outcome, CycleOutcome::Synced);
        // A page fetched but not applied is fetched again; an applied page is not.
        assert_eq!(h.server.bootstrap_calls(), calls_after, "{step:?}");
        assert_eq!(local_notes(&s), server_view(&h));
        let state = s.read(|c, _| sync_state::get(c)).expect("state");
        assert_eq!((state.bootstrap_complete, state.cursor_seq), (true, 4));
    }
}
