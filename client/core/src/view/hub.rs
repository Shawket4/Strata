//! The watcher hub: every open view-model stream is a watcher that rebuilds its view when a
//! write touches one of its [`Topics`] and emits it only when it differs from the last
//! emission. Rebuilds run synchronously after the write that caused them, so the order of
//! emissions is deterministic (tests assert full sequences).

use std::fmt;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rusqlite::Connection;

use crate::error::CoreResult;
use crate::view::Topics;
use crate::view::model::{Connectivity, NotificationMode, SyncPhase};

/// Receives a stream's values. `emit` returns `false` once the receiver is gone (the watcher
/// is then dropped).
pub trait ViewSink<T>: Send {
    /// Sends one value.
    fn emit(&self, value: T) -> bool;
}

/// Everything a builder needs besides the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewCtx {
    /// Now.
    pub now: DateTime<Utc>,
    /// The user's timezone.
    pub tz: Tz,
    /// Reachability.
    pub connectivity: Connectivity,
    /// Sync phase.
    pub phase: SyncPhase,
    /// How reminders are delivered on this platform.
    pub notification_mode: NotificationMode,
}

type Refresh = Box<dyn FnMut(&Connection, &ViewCtx) -> CoreResult<bool> + Send>;

struct Watcher {
    id: WatchId,
    topics: Topics,
    refresh: Refresh,
}

/// Identifies a watcher (to stop it).
pub type WatchId = u64;

/// The set of live view-model streams of one session.
#[derive(Default)]
pub struct ViewHub {
    watchers: Vec<Watcher>,
    next: WatchId,
}

impl fmt::Debug for ViewHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewHub")
            .field("watchers", &self.watchers.len())
            .finish()
    }
}

impl ViewHub {
    /// An empty hub.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a stream: emits the current view immediately, then again whenever a write
    /// touching `topics` changes it.
    pub fn watch<V, B, S>(
        &mut self,
        conn: &Connection,
        ctx: &ViewCtx,
        topics: Topics,
        build: B,
        sink: S,
    ) -> CoreResult<WatchId>
    where
        V: PartialEq + Clone + Send + 'static,
        B: Fn(&Connection, &ViewCtx) -> CoreResult<V> + Send + 'static,
        S: ViewSink<V> + 'static,
    {
        let mut last: Option<V> = None;
        let mut refresh: Refresh = Box::new(move |conn, ctx| {
            let v = build(conn, ctx)?;
            if last.as_ref() == Some(&v) {
                return Ok(true);
            }
            last = Some(v.clone());
            Ok(sink.emit(v))
        });
        if !refresh(conn, ctx)? {
            return Ok(0);
        }
        self.next += 1;
        let id = self.next;
        self.watchers.push(Watcher {
            id,
            topics,
            refresh,
        });
        Ok(id)
    }

    /// Rebuilds every watcher interested in `changed`; drops watchers whose sink closed.
    /// A builder error keeps the watcher (the next change retries) and is returned after all
    /// watchers ran.
    pub fn notify(&mut self, conn: &Connection, ctx: &ViewCtx, changed: Topics) -> CoreResult<()> {
        if changed.is_empty() {
            return Ok(());
        }
        let mut first_err = None;
        self.watchers.retain_mut(|w| {
            if !w.topics.intersects(changed) {
                return true;
            }
            match (w.refresh)(conn, ctx) {
                Ok(open) => open,
                Err(e) => {
                    first_err.get_or_insert(e);
                    true
                }
            }
        });
        first_err.map_or(Ok(()), Err)
    }

    /// Stops a watcher.
    pub fn unwatch(&mut self, id: WatchId) {
        self.watchers.retain(|w| w.id != id);
    }

    /// Number of live watchers.
    pub fn len(&self) -> usize {
        self.watchers.len()
    }

    /// Whether no watcher is live.
    pub fn is_empty(&self) -> bool {
        self.watchers.is_empty()
    }

    /// Stops every watcher (sign-out, account switch).
    pub fn clear(&mut self) {
        self.watchers.clear();
    }
}

/// A sink collecting values in memory (tests and headless use).
#[derive(Debug, Clone)]
pub struct Recorder<T> {
    values: std::sync::Arc<std::sync::Mutex<Vec<T>>>,
    open: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl<T> Default for Recorder<T> {
    fn default() -> Self {
        Self {
            values: std::sync::Arc::default(),
            open: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
}

impl<T: Clone> Recorder<T> {
    /// A new, open recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything received so far, then clears it.
    pub fn take(&self) -> Vec<T> {
        std::mem::take(
            &mut *self
                .values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// Everything received so far.
    pub fn all(&self) -> Vec<T> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Simulates the receiver going away.
    pub fn close(&self) {
        self.open.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl<T: Send> ViewSink<T> for Recorder<T> {
    fn emit(&self, value: T) -> bool {
        if !self.open.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(value);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ViewCtx {
        ViewCtx {
            now: DateTime::UNIX_EPOCH,
            tz: chrono_tz::UTC,
            connectivity: Connectivity::Unknown,
            phase: SyncPhase::Idle,
            notification_mode: NotificationMode::OsScheduled,
        }
    }

    #[test]
    fn emits_initially_then_only_on_change_and_drops_closed_sinks() {
        let conn = Connection::open_in_memory().expect("db");
        conn.execute_batch("CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (1);")
            .expect("schema");
        let mut hub = ViewHub::new();
        let rec = Recorder::new();
        hub.watch(
            &conn,
            &ctx(),
            Topics::NOTES,
            |c, _| Ok(c.query_row("SELECT v FROM t", [], |r| r.get::<_, i64>(0))?),
            rec.clone(),
        )
        .expect("watch");
        hub.notify(&conn, &ctx(), Topics::NOTES).expect("same");
        conn.execute("UPDATE t SET v = 2", []).expect("update");
        hub.notify(&conn, &ctx(), Topics::TASKS).expect("other topic");
        hub.notify(&conn, &ctx(), Topics::NOTES).expect("changed");
        assert_eq!(rec.take(), vec![1, 2]);
        rec.close();
        conn.execute("UPDATE t SET v = 3", []).expect("update");
        hub.notify(&conn, &ctx(), Topics::NOTES).expect("closed");
        assert_eq!(hub.len(), 0);
        assert_eq!(rec.take(), Vec::<i64>::new());
    }
}
