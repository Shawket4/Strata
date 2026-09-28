//! Screen intents added for L15 (PLAN §11, §12.3–§12.4): rebasing queued ops after a pull,
//! the `/events` signals (resume, reset, account closed), the waiting-for-approval session,
//! inbox filters and capture intents, reminders and settings, against the fake server.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod common;

use chrono::{DateTime, NaiveDate};
use common::Harness;
use pretty_assertions::assert_eq;
use strata_core::net::{EventSignal, NetError};
use strata_core::store::outbox;
use strata_core::sync::engine::Trigger;
use strata_core::sync::model::suggestions::{
    CorrectionFix, CorrectionPayload, CustodyPayload, CustodyTarget, EntityLinkPayload,
    SuggestionPayload, TaskPayload,
};
use strata_core::sync::model::{
    Op, Record, ReplyAuthor, SuggestionRecord, SuggestionReplyRecord, SuggestionStatus, Version,
};
use strata_core::view::build;
use strata_core::view::model::{InboxFilter, SessionKind, SignInRequest};
use strata_core::view::model::{SuggestionEdits, SuggestionKind};
use ulid::Ulid;

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const CAP1: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3A";
const CAP2: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3B";
const PERSON: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3C";

const BASE: &str = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1B\n---\nFirst.\nSecond.\nThird.\n";

fn update_content(op: &Op) -> String {
    match op {
        Op::NoteUpdate(u) => u.content.clone(),
        other => panic!("not an update: {other:?}"),
    }
}

#[tokio::test]
async fn queued_edits_are_rebased_on_every_pulled_server_version() {
    let h = Harness::new();
    h.server
        .remote_upsert(NOTE, "notes/Pricing experiments.md", BASE);
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");

    // Two edits queued offline, the second made on top of the first (a base the server has
    // never seen).
    let e1 = BASE.replace("First.", "First (mine).");
    let e2 = e1.replace("Second.", "Second (mine).");
    s.update_note(NOTE, &e1).expect("edit 1");
    s.update_note(NOTE, &e2).expect("edit 2");
    // Another device changes the third line.
    let theirs = BASE.replace("Third.", "Third (theirs).");
    h.server
        .remote_upsert(NOTE, "notes/Pricing experiments.md", &theirs);

    // A pull (no push) rewrites both queued edits on top of the new server version.
    s.pull().await.expect("pull");
    let ops = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert_eq!(ops.len(), 2);
    let r1 = theirs.replace("First.", "First (mine).");
    let r2 = r1.replace("Second.", "Second (mine).");
    assert_eq!(update_content(&ops[0].op), r1);
    assert_eq!(ops[0].base_version, Some(Version::of_text(&theirs)));
    assert_eq!(update_content(&ops[1].op), r2);
    assert_eq!(ops[1].base_version, Some(Version::of_text(&r1)));
    // What the user sees is the rebased state.
    let local = s
        .read(|c, _| strata_core::store::notes::current(c, NOTE))
        .expect("note")
        .expect("exists");
    assert_eq!(local.content, r2);

    // The push now fast-forwards: the server ends with both edits and theirs, no conflict.
    s.sync(Trigger::Manual).await.expect("push");
    let server = h.server.notes()[&Ulid::from_string(NOTE).expect("id")]
        .content
        .clone();
    assert_eq!(server, r2);
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox"), []);
}

#[tokio::test]
async fn events_resume_from_the_saved_seq_and_close_the_account() {
    let h = Harness::new();
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");

    h.server.push_event(Ok(EventSignal::Changed { seq: 7 }));
    let mut stream = s.subscribe_events().expect("subscribe");
    let signal = stream.next().await.expect("signal").expect("ok");
    assert!(
        s.handle_event(&signal).expect("handled"),
        "a change asks for a pull"
    );
    drop(stream);

    // Reconnecting resumes after the last seq seen.
    h.server.push_event(Ok(EventSignal::Reset { seq: 12 }));
    let mut stream = s.subscribe_events().expect("resubscribe");
    let signal = stream.next().await.expect("signal").expect("ok");
    assert!(
        s.handle_event(&signal).expect("handled"),
        "a reset re-pulls"
    );
    assert_eq!(h.server.subscriptions(), [Some(0), Some(7)]);
    let log = s.read(build::sync_status).expect("status").log;
    assert_eq!(log.first().map(|l| l.kind.as_str()), Some("reset"));

    // The account is disabled while connected: the stream ends and the session switches
    // to the warning.
    let closed = EventSignal::AccountClosed {
        seq: 13,
        reason: "disabled".into(),
    };
    assert!(!s.handle_event(&closed).expect("handled"));
    assert_eq!(h.core.state().expect("state").kind, SessionKind::Disabled);
    drop(stream);
    let _ = s.subscribe_events();
    assert_eq!(h.server.subscriptions(), [Some(0), Some(7), Some(13)]);
}

#[tokio::test]
async fn a_pending_account_waits_and_check_again_signs_in_once_approved() {
    let h = Harness::new();
    h.accounts
        .login_errors
        .lock()
        .unwrap()
        .insert("shawket".into(), NetError::AccountPending);
    let req = SignInRequest {
        server_url: common::SERVER.to_owned(),
        username: "shawket".into(),
        password: "pw-a".into(),
        device_name: "Shawket's laptop".into(),
    };
    let err = h.core.sign_in(req).await.expect_err("pending");
    assert_eq!(err.message_key(), "error.account_pending");
    let state = h.core.state().expect("state");
    assert_eq!(state.kind, SessionKind::PendingApproval);
    let pending = state.pending.expect("pending");
    assert_eq!(
        (pending.username.as_str(), pending.can_check),
        ("shawket", true)
    );
    assert_eq!(pending.requested_label, "just now");
    assert_eq!(pending.last_checked_label, None);

    // Still pending: the "last checked" time moves.
    h.clock.advance(chrono::Duration::minutes(5));
    let state = h.core.check_approval().await.expect("check");
    assert_eq!(
        state.pending.as_ref().expect("pending").requested_label,
        "5 minutes ago"
    );
    assert_eq!(state.kind, SessionKind::PendingApproval);
    assert_eq!(
        state
            .pending
            .expect("pending")
            .last_checked_label
            .as_deref(),
        Some("Last checked just now")
    );

    // Approved: "Check again" signs in with the password kept in memory.
    h.accounts.login_errors.lock().unwrap().clear();
    let state = h.core.check_approval().await.expect("check");
    assert_eq!(state.kind, SessionKind::Active);
    assert_eq!(state.pending, None);
}

fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("ulid")
}

fn decision(n: u128) -> Ulid {
    Ulid::from_parts(1_790_000_000_001, n)
}

fn suggestion(n: u128, note_id: &str, payload: &SuggestionPayload) -> Record {
    Record::Suggestion(SuggestionRecord {
        id: Ulid::from_parts(1_790_000_000_000, n),
        note_id: Some(Ulid::from_string(note_id).expect("id")),
        kind: payload.kind().into(),
        status: SuggestionStatus::Pending,
        payload: payload.to_bytes(),
        created: DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts"),
        replies: Vec::new(),
    })
}

#[tokio::test]
async fn inbox_filters_and_capture_intents() {
    let h = Harness::new();
    h.server.remote_upsert(
        CAP1,
        "inbox/Capture 1.md",
        &format!("---\nid: {CAP1}\nkind: capture\n---\nCall Ahmed tomorrow\n"),
    );
    h.server.remote_upsert(
        CAP2,
        "inbox/Capture 2.md",
        &format!("---\nid: {CAP2}\nkind: capture\n---\nMet Mona at the office\n"),
    );
    h.server.remote_upsert(
        PERSON,
        "people/Mona Adel.md",
        &format!("---\nid: {PERSON}\nkind: person\n---\n"),
    );
    h.server.remote_record(suggestion(
        1,
        CAP1,
        &SuggestionPayload::Task(TaskPayload {
            decision_id: decision(1),
            source_note: ulid(CAP1),
            block_id: None,
            title: "Call Ahmed".into(),
            due: NaiveDate::from_ymd_opt(2026, 9, 28),
            recurrence: None,
            reminders: Vec::new(),
            entities: Vec::new(),
            confidence: 0.9,
        }),
    ));
    h.server.remote_record(suggestion(
        2,
        CAP2,
        &SuggestionPayload::EntityLink(EntityLinkPayload {
            decision_id: decision(2),
            mention: "Mona".into(),
            kind: "person".into(),
            source_note: ulid(CAP2),
            block_id: None,
            proposed: None,
            candidates: vec![ulid(PERSON)],
            is_nickname: false,
            confidence: 0.55,
            reason: "ambiguous".into(),
        }),
    ));
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");

    let inbox = |f: InboxFilter| s.read(|c, ctx| build::inbox(c, ctx, f)).expect("inbox");
    let all = inbox(InboxFilter::All);
    assert_eq!(
        (
            all.all_count,
            all.ready_count,
            all.needs_you_count,
            all.conflicts_count
        ),
        (2, 1, 1, 0)
    );
    let ready: Vec<(&str, bool, bool)> = all
        .captures
        .iter()
        .map(|c| (c.note_id.as_str(), c.ready, c.needs_you))
        .collect();
    assert!(ready.contains(&(CAP1, true, false)));
    assert!(ready.contains(&(CAP2, false, true)));
    let needs_you = inbox(InboxFilter::NeedsYou);
    assert_eq!(
        needs_you
            .captures
            .iter()
            .map(|c| c.note_id.as_str())
            .collect::<Vec<_>>(),
        [CAP2]
    );
    assert_eq!(inbox(InboxFilter::Conflicts).captures, []);

    // A capture needing a choice cannot be accepted as is.
    assert_eq!(
        s.accept_capture(CAP2)
            .expect_err("needs choice")
            .message_key(),
        "error.invalid_input"
    );
    // "Accept all ready" accepts the ready capture only: one op.
    let ops = s.accept_all_ready().expect("accept all");
    assert_eq!(ops.len(), 1);
    let after = inbox(InboxFilter::All);
    assert_eq!(after.ready_count, 0);
    assert_eq!(after.needs_you_count, 1);
    // Rejecting the other capture's proposals leaves nothing to decide.
    let rejected = s.reject_capture(CAP2).expect("reject");
    assert_eq!(rejected.len(), 1);
    assert_eq!(inbox(InboxFilter::NeedsYou).needs_you_count, 0);
    assert_eq!(s.read(|c, _| outbox::all(c)).expect("outbox").len(), 2);
}

#[tokio::test]
async fn reminders_pins_and_notification_settings() {
    let h = Harness::new();
    h.server.remote_upsert(
        NOTE,
        "notes/Pricing experiments.md",
        "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1B\n---\n- [ ] Send offer 📅 2026-09-30 ^t1\n",
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let task_id = "t1";
    let at = NaiveDate::from_ymd_opt(2026, 9, 30)
        .expect("date")
        .and_hms_opt(9, 0, 0)
        .expect("time");
    s.add_reminder(task_id, at).expect("add");
    let content = || {
        s.read(|c, _| strata_core::store::notes::current(c, NOTE))
            .expect("note")
            .expect("exists")
            .content
    };
    assert!(content().contains("(@2026-09-30 09:00)"), "{}", content());
    s.remove_reminder(task_id, at).expect("remove");
    assert!(!content().contains("(@"));

    s.pin_note(NOTE, true).expect("pin");
    let nav = s.read(build::nav).expect("nav");
    assert_eq!(
        nav.pinned.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        [NOTE]
    );
    s.pin_note(NOTE, false).expect("unpin");
    assert_eq!(s.read(build::nav).expect("nav").pinned, []);

    s.set_snooze_minutes(30).expect("snooze");
    s.set_quiet_hours(true, "23:00", "06:30").expect("quiet");
    let settings = s
        .read(build::settings_view)
        .expect("settings")
        .expect("signed in");
    assert_eq!(settings.reminders.snooze_minutes, 30);
    assert_eq!(
        (
            settings.reminders.quiet_enabled,
            settings.reminders.quiet_from.as_str(),
            settings.reminders.quiet_until.as_str()
        ),
        (true, "23:00", "06:30")
    );
    assert_eq!(
        s.set_snooze_minutes(0).expect_err("zero").message_key(),
        "error.invalid_input"
    );
    assert_eq!(
        s.set_quiet_hours(true, "25:00", "06:00")
            .expect_err("bad time")
            .message_key(),
        "error.invalid_input"
    );
}

#[tokio::test]
async fn shared_suggestion_payloads_map_to_the_inbox_view() {
    const DOC: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3D";
    const DRAWER: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3E";
    const SRC: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V3F";
    let h = Harness::new();
    h.server.remote_upsert(
        DOC,
        "documents/Car license.md",
        &format!("---\nid: {DOC}\nkind: document\n---\n"),
    );
    h.server.remote_upsert(
        DRAWER,
        "places/Desk drawer.md",
        &format!("---\nid: {DRAWER}\nkind: place\n---\n"),
    );
    h.server.remote_upsert(
        PERSON,
        "people/Mona Adel.md",
        &format!("---\nid: {PERSON}\nkind: person\n---\n"),
    );
    h.server.remote_upsert(
        SRC,
        "notes/Errands.md",
        &format!("---\nid: {SRC}\n---\nPut the car license in the drawer.\n"),
    );
    let custody = SuggestionPayload::Custody(CustodyPayload {
        decision_id: decision(3),
        source_note: ulid(SRC),
        block_id: None,
        event: "stored-at".into(),
        date: NaiveDate::from_ymd_opt(2026, 9, 20).expect("date"),
        document: CustodyTarget {
            mention: "car license".into(),
            id: Some(ulid(DOC)),
            candidates: Vec::new(),
        },
        place: Some(CustodyTarget {
            mention: "the drawer".into(),
            id: Some(ulid(DRAWER)),
            candidates: Vec::new(),
        }),
        place_part_of: None,
        person: None,
        counterparty: None,
        confidence: 0.7,
        reason: "low_confidence".into(),
        quote: "Put the car license in the drawer.".into(),
    });
    let correction = SuggestionPayload::Correction(CorrectionPayload {
        decision_id: decision(4),
        message: "That Mona is Mona Adel".into(),
        fixes: vec![CorrectionFix {
            decision_id: decision(9),
            action: "repoint".into(),
            new_target: Some(ulid(PERSON)),
            new_type: None,
            confidence: 0.6,
            reason: "Same first name, same company.".into(),
        }],
        hints: Vec::new(),
        question: Some("Which Mona do you mean?".into()),
    });
    let mut with_thread = suggestion(5, SRC, &correction);
    if let Record::Suggestion(r) = &mut with_thread {
        r.replies = vec![
            SuggestionReplyRecord {
                id: Ulid::from_parts(1_790_000_000_002, 1),
                text: "The one at Acme".into(),
                at: DateTime::parse_from_rfc3339("2026-09-27T12:05:00+03:00").expect("ts"),
                author: ReplyAuthor::User,
            },
            SuggestionReplyRecord {
                id: Ulid::from_parts(1_790_000_000_002, 2),
                text: "Mona Adel, then.".into(),
                at: DateTime::parse_from_rfc3339("2026-09-27T12:06:00+03:00").expect("ts"),
                author: ReplyAuthor::Ai,
            },
        ];
    }
    h.server.remote_record(suggestion(4, SRC, &custody));
    h.server.remote_record(with_thread);
    // A kind this client does not know is kept, as "unsupported".
    h.server.remote_record(Record::Suggestion(SuggestionRecord {
        id: Ulid::from_parts(1_790_000_000_000, 6),
        note_id: Some(ulid(SRC)),
        kind: "hunch".into(),
        status: SuggestionStatus::Pending,
        payload: vec![0x80],
        created: DateTime::parse_from_rfc3339("2026-09-27T12:00:00+03:00").expect("ts"),
        replies: Vec::new(),
    }));
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let inbox = s
        .read(|c, ctx| build::inbox(c, ctx, InboxFilter::All))
        .expect("inbox");
    let by_kind = |k: SuggestionKind| {
        inbox
            .suggestions
            .iter()
            .find(|x| x.detail.kind == k)
            .unwrap_or_else(|| panic!("{k:?}"))
            .clone()
    };

    let c = by_kind(SuggestionKind::Custody);
    assert_eq!(c.detail.line, "2026-09-20 — stored-at [[Desk drawer]]");
    assert_eq!(
        c.detail.document.as_ref().map(|d| d.title.as_str()),
        Some("Car license")
    );
    assert_eq!(
        c.detail.location.as_ref().and_then(|l| l.id.as_deref()),
        Some(DRAWER)
    );
    assert_eq!(
        (c.detail.holder.clone(), c.detail.last_holder.clone()),
        (None, None)
    );
    assert_eq!(c.detail.date_label.as_deref(), Some("20 Sep"));
    assert_eq!(c.detail.quote, "Put the car license in the drawer.");
    assert_eq!(c.detail.decision_id, Some(decision(3).to_string()));
    assert_eq!(
        (c.detail.confidence, c.can_accept, c.auto_applied),
        (Some(0.7), true, false)
    );

    let k = by_kind(SuggestionKind::Correction);
    assert_eq!(k.detail.title, "That Mona is Mona Adel");
    assert_eq!(
        k.detail.question.as_deref(),
        Some("Which Mona do you mean?")
    );
    assert_eq!(
        k.detail.target.as_ref().and_then(|t| t.id.as_deref()),
        Some(PERSON)
    );
    assert!(k.needs_you);
    assert_eq!(
        k.thread
            .iter()
            .map(|m| (m.author.as_str(), m.text.as_str()))
            .collect::<Vec<_>>(),
        [("user", "The one at Acme"), ("ai", "Mona Adel, then.")]
    );

    let u = by_kind(SuggestionKind::Unsupported);
    assert_eq!(u.detail.server_kind, "hunch");

    // Accept with edits: the op carries every edit (`suggestion.accept`).
    let op = s
        .accept_suggestion_with(
            &c.id,
            SuggestionEdits {
                target_id: Some(DOC.into()),
                aliases: Some(vec!["الرخصة".into()]),
                ..SuggestionEdits::default()
            },
        )
        .expect("accept");
    let queued = s.read(|c, _| outbox::all(c)).expect("outbox");
    let accept = queued.iter().find(|o| o.op_id == op).expect("op");
    match &accept.op {
        Op::SuggestionAccept(a) => {
            let e = a.edits.as_ref().expect("edits");
            assert_eq!(e.target_id, Some(ulid(DOC)));
            assert_eq!(e.aliases.as_deref(), Some(&["الرخصة".to_owned()][..]));
        }
        other => panic!("{other:?}"),
    }

    // Forced relink (`relink.request`).
    let relink = s.request_relink(SRC).expect("relink");
    let queued = s.read(|c, _| outbox::all(c)).expect("outbox");
    assert!(matches!(
        queued.iter().find(|o| o.op_id == relink).map(|o| &o.op),
        Some(Op::RelinkRequest(r)) if r.id == ulid(SRC)
    ));
}
