//! The sync engine (PLAN §12.4): push the outbox in order, then pull; bootstrap on first login
//! and after an epoch change; classify failures for backoff and connectivity.
//!
//! The engine never holds the database across a network call: every step is one transaction
//! through [`SyncHost::db`], so the process can die between any two steps without losing or
//! duplicating work:
//!
//! | Crash after… | On restart |
//! |---|---|
//! | marking ops `inflight` | they go back to `pending` and are pushed again with the same `op_id`s; the server returns the stored results (idempotency) |
//! | the server applied a push, before results were recorded | same as above |
//! | recording results | nothing left to do for those ops |
//! | fetching a page, before applying it | the page is fetched again (cursor unchanged) |
//! | applying a page | the cursor already moved; the next page follows |

use std::sync::Arc;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::CoreResult;
use crate::net::{NetError, SyncApi};
use crate::store::index::Reindex;
use crate::store::{outbox, sync_state};
use crate::sync::apply;
use crate::sync::model::SyncCursor;
use crate::view::Topics;
use crate::view::model::{Connectivity, SyncActivity, SyncPhase};

/// What woke the engine (§12.4 triggers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// App start / session open.
    Start,
    /// App resumed.
    Resume,
    /// A local write queued an op.
    AfterWrite,
    /// An `/events` frame arrived.
    EventsFrame,
    /// Foreground timer.
    Timer,
    /// Backoff elapsed.
    Retry,
    /// The user asked ("Sync now").
    Manual,
}

/// A step boundary, for crash simulation in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// Ops were marked `inflight`.
    MarkedInflight,
    /// The server answered the push (results not recorded).
    Pushed,
    /// Push results were recorded.
    Recorded,
    /// A bootstrap page was fetched (not applied).
    BootstrapFetched,
    /// A bootstrap page was applied.
    BootstrapApplied,
    /// A changes page was fetched (not applied).
    ChangesFetched,
    /// A changes page was applied.
    ChangesApplied,
}

/// How a cycle ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleOutcome {
    /// Everything pushed and pulled.
    Synced,
    /// A network call failed; retry after backoff.
    Failed(NetError),
    /// Simulated crash at a step (tests).
    Crashed(Step),
}

/// What a cycle did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleReport {
    /// Ops pushed (results recorded).
    pub pushed: u32,
    /// Records pulled (bootstrap + changes).
    pub pulled: u32,
    /// A bootstrap ran.
    pub bootstrapped: bool,
    /// Outcome.
    pub outcome: CycleOutcome,
}

/// The session side the engine runs against.
pub trait SyncHost: Send + Sync {
    /// Runs `f` in one transaction and then refreshes views for the topics it returns.
    fn db<R>(&self, f: impl FnOnce(&Connection, &str) -> CoreResult<(R, Topics)>) -> CoreResult<R>;
    /// Reports progress (views of the sync pill).
    fn set_activity(&self, activity: SyncActivity);
    /// Reports reachability.
    fn set_connectivity(&self, connectivity: Connectivity);
}

/// Delay before retry number `failures` (1-based): 2 s doubling to at most 5 min.
pub fn backoff_delay(failures: u32) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let exp = failures.saturating_sub(1).min(16);
    Duration::from_secs((2u64 << exp).min(300))
}

/// Push/pull engine.
#[derive(Debug, Clone)]
pub struct SyncEngine {
    api: Arc<dyn SyncApi>,
    /// Ops per push request.
    pub batch_size: usize,
    /// Changes per page.
    pub page_limit: u32,
    /// Crash simulation (tests): stop right after this step.
    pub crash_after: Option<Step>,
}

enum Flow<T> {
    Go(T),
    Stop(CycleOutcome),
}

impl SyncEngine {
    /// An engine over `api`.
    pub fn new(api: Arc<dyn SyncApi>) -> Self {
        Self {
            api,
            batch_size: 100,
            page_limit: 500,
            crash_after: None,
        }
    }

    fn crash<T>(&self, step: Step, value: T) -> Flow<T> {
        if self.crash_after == Some(step) {
            Flow::Stop(CycleOutcome::Crashed(step))
        } else {
            Flow::Go(value)
        }
    }

    fn failed<H: SyncHost>(host: &H, e: NetError) -> CoreResult<CycleOutcome> {
        let offline = matches!(e, NetError::Offline(_));
        host.set_connectivity(if offline {
            Connectivity::Offline
        } else {
            Connectivity::Online
        });
        let key = crate::error::CoreError::from(e.clone()).message_key();
        host.db(|c, _| {
            sync_state::update(c, |s| {
                s.consecutive_failures += 1;
                s.last_error = Some(key);
            })?;
            Ok(((), Topics::SYNC))
        })?;
        Ok(CycleOutcome::Failed(e))
    }

    /// Runs one full cycle: push everything, then pull (bootstrapping when needed).
    pub async fn run_cycle<H: SyncHost>(
        &self,
        host: &H,
        _trigger: Trigger,
    ) -> CoreResult<CycleReport> {
        let mut report = CycleReport {
            pushed: 0,
            pulled: 0,
            bootstrapped: false,
            outcome: CycleOutcome::Synced,
        };
        let outcome = match self.push_all(host, &mut report).await? {
            Flow::Stop(o) => o,
            Flow::Go(()) => match self.pull_all(host, &mut report).await? {
                Flow::Stop(o) => o,
                Flow::Go(()) => CycleOutcome::Synced,
            },
        };
        host.set_activity(SyncActivity::default());
        if outcome == CycleOutcome::Synced {
            host.set_connectivity(Connectivity::Online);
            host.db(|c, now| {
                sync_state::update(c, |s| {
                    s.consecutive_failures = 0;
                    s.last_error = None;
                    s.last_pull_at = Some(now.to_owned());
                })?;
                Ok(((), Topics::SYNC))
            })?;
        }
        report.outcome = outcome;
        Ok(report)
    }

    async fn push_all<H: SyncHost>(
        &self,
        host: &H,
        report: &mut CycleReport,
    ) -> CoreResult<Flow<()>> {
        loop {
            let batch = host.db(|c, _| {
                let ops = outbox::next_batch(c, self.batch_size)?;
                for op in &ops {
                    outbox::set_status(c, &op.op_id, outbox::OpStatus::Inflight)?;
                }
                Ok((ops, Topics::SYNC))
            })?;
            if batch.is_empty() {
                return Ok(Flow::Go(()));
            }
            if let Flow::Stop(o) = self.crash(Step::MarkedInflight, ()) {
                return Ok(Flow::Stop(o));
            }
            host.set_activity(SyncActivity {
                phase: SyncPhase::Pushing,
                ops: u32::try_from(batch.len()).unwrap_or(u32::MAX),
                ..SyncActivity::default()
            });
            let request = batch
                .iter()
                .map(outbox::OutboxOp::to_push)
                .collect::<CoreResult<Vec<_>>>()?;
            let results = match self.api.push(request).await {
                Ok(r) => r,
                Err(e) => {
                    let msg = e.to_string();
                    host.db(|c, _| {
                        outbox::requeue_inflight(c, Some(&msg))?;
                        Ok(((), Topics::SYNC))
                    })?;
                    return Ok(Flow::Stop(Self::failed(host, e)?));
                }
            };
            host.set_connectivity(Connectivity::Online);
            if let Flow::Stop(o) = self.crash(Step::Pushed, ()) {
                return Ok(Flow::Stop(o));
            }
            let recorded = host.db(|c, now| {
                let mut re = Reindex::new();
                let mut n = 0u32;
                for outcome in &results {
                    let id = outcome.op_id.to_string();
                    if let Some(op) = batch.iter().find(|o| o.op_id == id) {
                        apply::record_result(c, op, &outcome.result, now, &mut re)?;
                        n += 1;
                    }
                }
                // Ops the server did not answer go back to the queue.
                outbox::requeue_inflight(c, None)?;
                outbox::compact(c)?;
                sync_state::update(c, |s| s.last_push_at = Some(now.to_owned()))?;
                re.topics(Topics::SYNC);
                Ok((n, re.apply(c)?))
            })?;
            report.pushed += recorded;
            if let Flow::Stop(o) = self.crash(Step::Recorded, ()) {
                return Ok(Flow::Stop(o));
            }
        }
    }

    async fn pull_all<H: SyncHost>(
        &self,
        host: &H,
        report: &mut CycleReport,
    ) -> CoreResult<Flow<()>> {
        loop {
            let state = host.db(|c, _| Ok((sync_state::get(c)?, Topics::NONE)))?;
            let (true, Some(epoch)) = (state.bootstrap_complete, state.epoch) else {
                report.bootstrapped = true;
                match self.bootstrap(host, report).await? {
                    Flow::Go(()) => continue,
                    Flow::Stop(o) => return Ok(Flow::Stop(o)),
                }
            };
            host.set_activity(SyncActivity {
                phase: SyncPhase::Pulling,
                ..SyncActivity::default()
            });
            let page = match self
                .api
                .changes(state.cursor_seq, epoch, self.page_limit)
                .await
            {
                Ok(p) => p,
                Err(NetError::EpochChanged) => {
                    // The server rebuilt and cannot continue from our seq: re-bootstrap.
                    host.db(|c, _| {
                        sync_state::update(c, |s| {
                            s.bootstrap_complete = false;
                            s.bootstrap_cursor = None;
                            s.bootstrap_pages = 0;
                        })?;
                        Ok(((), Topics::SYNC))
                    })?;
                    continue;
                }
                Err(e) => return Ok(Flow::Stop(Self::failed(host, e)?)),
            };
            host.set_connectivity(Connectivity::Online);
            if let Flow::Stop(o) = self.crash(Step::ChangesFetched, ()) {
                return Ok(Flow::Stop(o));
            }
            let cursor = SyncCursor {
                epoch,
                seq: state.cursor_seq,
            };
            let next = match cursor.advance(&page) {
                Ok(next) => next,
                Err(sync_model::CursorError::EpochChanged { .. }) => {
                    host.db(|c, _| {
                        sync_state::update(c, |s| {
                            s.bootstrap_complete = false;
                            s.bootstrap_cursor = None;
                            s.bootstrap_pages = 0;
                        })?;
                        Ok(((), Topics::SYNC))
                    })?;
                    continue;
                }
                Err(e) => {
                    return Ok(Flow::Stop(Self::failed(
                        host,
                        NetError::Protocol(e.to_string()),
                    )?));
                }
            };
            let n = u32::try_from(page.changes.len()).unwrap_or(u32::MAX);
            host.db(|c, now| {
                let mut re = Reindex::new();
                for ch in &page.changes {
                    apply::apply_change(c, ch, now, &mut re)?;
                }
                sync_state::update(c, |s| s.cursor_seq = next.seq)?;
                re.topics(Topics::SYNC);
                Ok(((), re.apply(c)?))
            })?;
            report.pulled += n;
            if let Flow::Stop(o) = self.crash(Step::ChangesApplied, ()) {
                return Ok(Flow::Stop(o));
            }
            if !page.has_more {
                return Ok(Flow::Go(()));
            }
        }
    }

    async fn bootstrap<H: SyncHost>(
        &self,
        host: &H,
        report: &mut CycleReport,
    ) -> CoreResult<Flow<()>> {
        loop {
            let state = host.db(|c, _| {
                let s = sync_state::get(c)?;
                if s.bootstrap_cursor.is_none() && s.bootstrap_pages == 0 {
                    apply::begin_bootstrap(c)?;
                }
                Ok((s, Topics::NONE))
            })?;
            host.set_activity(SyncActivity {
                phase: SyncPhase::Bootstrapping,
                pages_done: state.bootstrap_pages,
                ..SyncActivity::default()
            });
            let page = match self.api.bootstrap(state.bootstrap_cursor.clone()).await {
                Ok(p) => p,
                Err(e) => return Ok(Flow::Stop(Self::failed(host, e)?)),
            };
            host.set_connectivity(Connectivity::Online);
            if let Flow::Stop(o) = self.crash(Step::BootstrapFetched, ()) {
                return Ok(Flow::Stop(o));
            }
            if state.bootstrap_pages > 0 && state.epoch != Some(page.epoch) {
                // The server rebuilt while we were paging: the saved cursor belongs to the old
                // snapshot. Start over.
                host.db(|c, _| {
                    sync_state::update(c, |s| {
                        s.bootstrap_cursor = None;
                        s.bootstrap_pages = 0;
                    })?;
                    Ok(((), Topics::NONE))
                })?;
                continue;
            }
            let n = u32::try_from(page.records.len()).unwrap_or(u32::MAX);
            let done = host.db(|c, now| {
                let mut re = Reindex::new();
                for r in &page.records {
                    apply::apply_record(c, r, true, now, &mut re)?;
                }
                let last = page.next_cursor.is_none();
                if last {
                    apply::finish_bootstrap(c, now, &mut re)?;
                }
                sync_state::update(c, |s| {
                    // Pull changes after the *oldest* page's position: records that changed
                    // while later pages were fetched (or while a crash interrupted paging) are
                    // replayed by `/sync/changes` instead of being skipped. Replays are
                    // idempotent (versions are content hashes).
                    let seq = SyncCursor::after_bootstrap(&page).seq;
                    s.cursor_seq = if s.bootstrap_pages == 0 {
                        seq
                    } else {
                        s.cursor_seq.min(seq)
                    };
                    s.epoch = Some(page.epoch);
                    s.bootstrap_pages += 1;
                    s.bootstrap_cursor.clone_from(&page.next_cursor);
                    if last {
                        s.bootstrap_complete = true;
                        s.bootstrap_pages = 0;
                    }
                })?;
                re.topics(Topics::SYNC);
                Ok((last, re.apply(c)?))
            })?;
            report.pulled += n;
            host.set_activity(SyncActivity {
                phase: SyncPhase::Bootstrapping,
                pages_done: state.bootstrap_pages + 1,
                ..SyncActivity::default()
            });
            if let Flow::Stop(o) = self.crash(Step::BootstrapApplied, ()) {
                return Ok(Flow::Stop(o));
            }
            if done {
                return Ok(Flow::Go(()));
            }
        }
    }
}

/// Puts ops interrupted mid-push back in the queue (session open after a crash).
pub fn recover(conn: &Connection) -> CoreResult<usize> {
    outbox::requeue_inflight(conn, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_to_five_minutes() {
        let secs: Vec<u64> = (0..=10).map(|n| backoff_delay(n).as_secs()).collect();
        assert_eq!(secs, vec![0, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300]);
        assert_eq!(backoff_delay(u32::MAX).as_secs(), 300);
    }
}
