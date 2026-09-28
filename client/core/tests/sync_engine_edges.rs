//! The sync engine against a server that answers out of line (PLAN §12.4): a changes page
//! from a rebuilt server (new epoch without the `410`), a page whose seqs go backwards, and a
//! rebuild while bootstrap pages are being fetched. The fake server is wrapped so single
//! answers can be replaced; everything else is the fake server's.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{Harness, SERVER};
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use strata_core::net::{NetError, SyncApi, Tokens};
use strata_core::session::{Core, Session};
use strata_core::store::sync_state;
use strata_core::sync::engine::{CycleOutcome, CycleReport, Trigger};
use strata_core::sync::model::{
    BootstrapPage, ChangeRecord, ChangesPage, OpOutcome, Record, SyncOp,
};
use strata_core::testing::FakeServer;
use strata_core::view::build;
use strata_core::view::model::{Connectivity, SignInRequest};
use sync_model::changes::NoteRecord;
use ulid::Ulid;

const N1: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1A";
const N2: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const N3: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";

/// The fake server with replaceable answers.
#[derive(Debug, Clone)]
struct Scripted {
    inner: FakeServer,
    changes: Arc<Mutex<VecDeque<ChangesPage>>>,
    bootstrap: Arc<Mutex<VecDeque<BootstrapPage>>>,
}

impl SyncApi for Scripted {
    fn bootstrap(&self, cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        match self.bootstrap.lock().unwrap().pop_front() {
            Some(p) => Box::pin(async move { Ok(p) }),
            None => self.inner.bootstrap(cursor),
        }
    }

    fn changes(
        &self,
        since: u64,
        epoch: u64,
        limit: u32,
    ) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        match self.changes.lock().unwrap().pop_front() {
            Some(p) => Box::pin(async move { Ok(p) }),
            None => self.inner.changes(since, epoch, limit),
        }
    }

    fn push(&self, ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        self.inner.push(ops)
    }
}

fn note(id: &str, title: &str) -> (String, String) {
    (
        format!("notes/{title}.md"),
        format!("---\nid: {id}\n---\n{title}.\n"),
    )
}

fn record(id: &str, title: &str) -> Record {
    let (path, content) = note(id, title);
    Record::Note(NoteRecord {
        id: Ulid::from_string(id).expect("id"),
        version: sync_model::Version::of_text(&content),
        kind: domain::NoteKind::Note,
        summary: None,
        path,
        content,
    })
}

/// A harness whose core talks to the scripted wrapper; signed in.
async fn scripted() -> (Harness, Scripted, Arc<Session>) {
    let mut h = Harness::new();
    let api = Scripted {
        inner: h.server.clone(),
        changes: Arc::default(),
        bootstrap: Arc::default(),
    };
    let mut env = common::env(
        h.dir.path(),
        &h.server,
        &h.accounts,
        &h.clock,
        &h.ids,
        h.platform,
    );
    let wrapped = api.clone();
    env.sync_api = Arc::new(move |_url: &str, _tokens: Tokens| -> Arc<dyn SyncApi> {
        Arc::new(wrapped.clone())
    });
    h.core = Core::open(env).expect("core");
    h.core
        .sign_in(SignInRequest {
            server_url: SERVER.to_owned(),
            username: "shawket".to_owned(),
            password: "pw-a".to_owned(),
            device_name: "Shawket's laptop".to_owned(),
        })
        .await
        .expect("sign in");
    let s = h.core.session().expect("session");
    (h, api, s)
}

fn local_ids(s: &Session) -> Vec<String> {
    s.read(|c, _| {
        Ok(strata_core::store::notes::live_paths(c)?
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>())
    })
    .expect("notes")
}

#[tokio::test]
async fn a_changes_page_from_a_new_epoch_starts_a_new_bootstrap() {
    let (h, api, s) = scripted().await;
    let (p, c) = note(N1, "Churn");
    h.server.remote_upsert(N1, &p, &c);
    s.sync(Trigger::Start).await.expect("bootstrap");
    assert_eq!(h.server.bootstrap_calls(), 1);

    // The server was rebuilt, but answers the old cursor with a page of the new epoch
    // instead of `410`.
    api.changes.lock().unwrap().push_back(ChangesPage {
        epoch: 2,
        changes: vec![ChangeRecord::upsert(1, 2, record(N2, "Pricing"))],
        next_seq: 1,
        has_more: false,
    });
    let report = s.sync(Trigger::Manual).await.expect("cycle");

    assert_eq!(
        report,
        CycleReport {
            pushed: 0,
            pulled: 1,
            bootstrapped: true,
            outcome: CycleOutcome::Synced,
        }
    );
    assert_eq!(h.server.bootstrap_calls(), 2, "the snapshot was fetched again");
    // Nothing of the rejected page was applied; the snapshot is what the client has.
    assert_eq!(local_ids(&s), [N1.to_owned()]);
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!((state.bootstrap_complete, state.epoch), (true, Some(1)));
}

#[tokio::test]
async fn a_page_whose_seqs_go_backwards_fails_the_cycle_and_keeps_the_cursor() {
    let (h, api, s) = scripted().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    let before = s.read(|c, _| sync_state::get(c)).expect("state");

    api.changes.lock().unwrap().push_back(ChangesPage {
        epoch: 1,
        changes: vec![
            ChangeRecord::upsert(5, 1, record(N1, "Churn")),
            ChangeRecord::upsert(3, 1, record(N2, "Pricing")),
        ],
        next_seq: 5,
        has_more: false,
    });
    let report = s.sync(Trigger::Manual).await.expect("cycle");

    assert_eq!(
        report.outcome,
        CycleOutcome::Failed(NetError::Protocol("change seq 3 is not after 5".into()))
    );
    assert_eq!(local_ids(&s), Vec::<String>::new());
    let after = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!(after.cursor_seq, before.cursor_seq);
    assert_eq!(after.consecutive_failures, 1);
    let status = s.read(build::sync_status).expect("status");
    assert_eq!(status.last_error.as_deref(), Some("error.internal"));
    assert_eq!(status.pill.connectivity, Connectivity::Online);
    assert_eq!(
        status.log.first().map(|l| (l.kind.as_str(), l.detail.as_str())),
        Some(("failed", "error.internal"))
    );

    // The next, well-formed page is taken normally.
    let (p, c) = note(N3, "Roadmap");
    h.server.remote_upsert(N3, &p, &c);
    let report = s.sync(Trigger::Retry).await.expect("cycle");
    assert_eq!(report.outcome, CycleOutcome::Synced);
    assert_eq!(local_ids(&s), [N3.to_owned()]);
}

#[tokio::test]
async fn a_rebuild_between_bootstrap_pages_restarts_the_snapshot() {
    let (h, api, s) = scripted().await;
    h.server.set_page_size(1);
    for (id, title) in [(N1, "Churn"), (N2, "Pricing")] {
        let (p, c) = note(id, title);
        h.server.remote_upsert(id, &p, &c);
    }
    // Page 1 comes from the old snapshot (epoch 1, cursor "1"); page 2 from a rebuilt server
    // (epoch 7) holding only N3.
    api.bootstrap.lock().unwrap().extend([
        BootstrapPage {
            epoch: 1,
            seq: 2,
            records: vec![record(N1, "Churn")],
            next_cursor: Some("1".into()),
        },
        BootstrapPage {
            epoch: 7,
            seq: 1,
            records: vec![record(N3, "Roadmap")],
            next_cursor: None,
        },
    ]);

    let report = s.sync(Trigger::Start).await.expect("bootstrap");

    assert_eq!(report.outcome, CycleOutcome::Synced);
    assert!(report.bootstrapped);
    // The restarted snapshot is the fake server's (both of its notes, one page each); the
    // stale page's N1 stays only because the new snapshot has it too, the rebuilt page's N3
    // was never applied.
    assert_eq!(local_ids(&s), [N1.to_owned(), N2.to_owned()]);
    assert_eq!(h.server.bootstrap_calls(), 2);
    let state = s.read(|c, _| sync_state::get(c)).expect("state");
    assert_eq!(
        (state.bootstrap_complete, state.epoch, state.cursor_seq),
        (true, Some(1), 2)
    );
}
