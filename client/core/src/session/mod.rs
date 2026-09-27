//! The composition root: one [`Session`] per signed-in account wires the account database,
//! the view hub, the sync engine, the token provider and the reminder planner together; the
//! [`Core`] owns the active session, the device registry and the session-state stream, and
//! implements sign-in, sign-up, sign-out and account switching (§12.7).
//!
//! Everything here is callable headless: the frb facade in [`crate::api`] only forwards to it.

mod core;
mod intents;

pub use self::core::{Core, CoreEnv, SyncApiFactory};
pub use intents::{NewTask, TaskEdit, candidate_item};

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rusqlite::Connection;
use ulid::Ulid;

use crate::auth::{AccountMode, CoreTokenProvider, TokenStore, account_mode};
use crate::error::{CoreError, CoreResult};
use crate::net::{NetError, Tokens};
use crate::notify;
use crate::store::{AccountDb, account, notes, outbox, tokens};
use crate::sync::engine::{CycleOutcome, CycleReport, SyncEngine, SyncHost, Trigger};
use crate::view::build;
use crate::view::model::{
    Connectivity, NotificationMode, NotificationOp, NotificationResult, Platform, SessionKind,
    SessionState, SyncActivity,
};
use crate::view::{Topics, ViewCtx, ViewHub, ViewSink, WatchId};

/// Notification mode of a platform.
pub fn notification_mode(p: Platform) -> NotificationMode {
    match p {
        Platform::Linux => NotificationMode::WhileRunning,
        _ => NotificationMode::OsScheduled,
    }
}

struct Inner {
    db: AccountDb,
    hub: ViewHub,
    connectivity: Connectivity,
    activity: SyncActivity,
    notify_sinks: Vec<Box<dyn ViewSink<NotificationOp>>>,
    notify_buffer: Vec<NotificationOp>,
    session_changed: bool,
}

/// One signed-in account.
pub struct Session {
    env: Arc<CoreEnv>,
    user_id: Ulid,
    inner: Mutex<Inner>,
    engine: SyncEngine,
    tokens: Arc<CoreTokenProvider>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

/// The [`TokenStore`] over the session's database.
#[derive(Debug)]
struct DbTokenStore {
    session: std::sync::Weak<Session>,
}

impl TokenStore for DbTokenStore {
    fn load(&self) -> Option<tokens::StoredTokens> {
        let s = self.session.upgrade()?;
        let g = s.lock();
        tokens::get(g.db.conn()).ok().flatten()
    }

    fn save(&self, t: &tokens::StoredTokens) {
        if let Some(s) = self.session.upgrade() {
            let g = s.lock();
            if let Err(e) = tokens::put(g.db.conn(), t) {
                tracing::warn!("storing refreshed tokens failed: {e}");
            }
        }
    }

    fn session_ended(&self) {
        if let Some(s) = self.session.upgrade() {
            let mut g = s.lock();
            let _ = tokens::clear(g.db.conn());
            g.session_changed = true;
        }
    }
}

impl Session {
    /// Opens the session of `user_id` (its database must exist or is created). Ops interrupted
    /// mid-push by a crash go back to the queue.
    pub fn open(env: &Arc<CoreEnv>, user_id: Ulid) -> CoreResult<Arc<Self>> {
        let db = AccountDb::open(&env.paths, user_id)?;
        crate::sync::engine::recover(db.conn())?;
        let server_url = account::get(db.conn())?
            .map(|a| a.server_url)
            .unwrap_or_default();
        Ok(Arc::new_cyclic(|weak: &std::sync::Weak<Session>| {
            let store: Arc<dyn TokenStore> = Arc::new(DbTokenStore {
                session: weak.clone(),
            });
            let tokens = Arc::new(CoreTokenProvider::new(
                store,
                env.account_api.clone(),
                server_url.clone(),
                env.clock.clone(),
            ));
            let provider: Tokens = tokens.clone();
            let api = (env.sync_api)(&server_url, provider);
            Session {
                engine: SyncEngine::new(api),
                env: env.clone(),
                user_id,
                inner: Mutex::new(Inner {
                    db,
                    hub: ViewHub::new(),
                    connectivity: Connectivity::Unknown,
                    activity: SyncActivity::default(),
                    notify_sinks: Vec::new(),
                    notify_buffer: Vec::new(),
                    session_changed: false,
                }),
                tokens,
            }
        }))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The account.
    pub fn user_id(&self) -> Ulid {
        self.user_id
    }

    /// The token provider (authenticated calls).
    pub fn tokens(&self) -> Tokens {
        self.tokens.clone()
    }

    /// The sync engine (tests set crash points on a clone).
    pub fn engine(&self) -> &SyncEngine {
        &self.engine
    }

    /// The environment.
    pub fn env(&self) -> &Arc<CoreEnv> {
        &self.env
    }

    fn tz(conn: &Connection) -> Tz {
        account::get(conn)
            .ok()
            .flatten()
            .and_then(|a| a.timezone.parse::<Tz>().ok())
            .unwrap_or(chrono_tz::UTC)
    }

    fn ctx_of(&self, g: &Inner) -> ViewCtx {
        ViewCtx {
            now: self.env.clock.now(),
            tz: Self::tz(g.db.conn()),
            connectivity: g.connectivity,
            activity: g.activity.clone(),
            notification_mode: notification_mode(self.env.platform),
        }
    }

    /// The current view context.
    pub fn ctx(&self) -> ViewCtx {
        let g = self.lock();
        self.ctx_of(&g)
    }

    /// Runs a read against the database.
    pub fn read<R>(&self, f: impl FnOnce(&Connection, &ViewCtx) -> CoreResult<R>) -> CoreResult<R> {
        let g = self.lock();
        let ctx = self.ctx_of(&g);
        f(g.db.conn(), &ctx)
    }

    /// Starts a view-model stream.
    pub fn watch<V, B, S>(&self, topics: Topics, build: B, sink: S) -> CoreResult<WatchId>
    where
        V: PartialEq + Clone + Send + 'static,
        B: Fn(&Connection, &ViewCtx) -> CoreResult<V> + Send + 'static,
        S: ViewSink<V> + 'static,
    {
        let mut g = self.lock();
        let ctx = self.ctx_of(&g);
        let Inner { db, hub, .. } = &mut *g;
        hub.watch(db.conn(), &ctx, topics, build, sink)
    }

    /// Stops a view-model stream.
    pub fn unwatch(&self, id: WatchId) {
        self.lock().hub.unwatch(id);
    }

    /// Refreshes views after `topics` changed (and the reminder plan when tasks or settings
    /// changed).
    fn after_change(&self, g: &mut Inner, topics: Topics) -> CoreResult<()> {
        let ctx = self.ctx_of(g);
        let Inner { db, hub, .. } = &mut *g;
        hub.notify(db.conn(), &ctx, topics)?;
        if topics.intersects(Topics::TASKS | Topics::SETTINGS | Topics::ACCOUNT) {
            let ops = notify::recompute(db.conn(), ctx.now, ctx.tz, ctx.notification_mode)?;
            Self::emit_notifications(g, ops);
            let ctx = self.ctx_of(g);
            let Inner { db, hub, .. } = &mut *g;
            hub.notify(db.conn(), &ctx, Topics::SETTINGS)?;
        }
        Ok(())
    }

    fn emit_notifications(g: &mut Inner, ops: Vec<NotificationOp>) {
        if ops.is_empty() {
            return;
        }
        if g.notify_sinks.is_empty() {
            g.notify_buffer.extend(ops);
            return;
        }
        for op in ops {
            g.notify_sinks.retain(|s| s.emit(op.clone()));
        }
    }

    /// Runs a write transaction and refreshes views.
    pub fn write<R>(
        &self,
        f: impl FnOnce(&Connection, &str) -> CoreResult<(R, Topics)>,
    ) -> CoreResult<R> {
        let mut g = self.lock();
        let now = self.env.clock.now().to_rfc3339();
        let (r, topics) = {
            let tx = g.db.conn_mut().transaction()?;
            let out = f(&tx, &now)?;
            tx.commit()?;
            out
        };
        self.after_change(&mut g, topics)?;
        Ok(r)
    }

    /// Runs an intent (optimistic change + outbox op, §12.3).
    pub fn execute(&self, intent: &crate::store::write::Intent) -> CoreResult<()> {
        let mut g = self.lock();
        Self::ensure_writable(&g)?;
        let now = self.env.clock.now().to_rfc3339();
        let topics = crate::store::write::execute(g.db.conn_mut(), intent, &now)?;
        self.after_change(&mut g, topics)
    }

    fn ensure_writable(g: &Inner) -> CoreResult<()> {
        match account::get(g.db.conn())?.map(|a| account_mode(&a.status, false)) {
            Some(AccountMode::DeletionPending) => Err(CoreError::AccountDeletionPending),
            Some(AccountMode::Disabled) => Err(CoreError::AccountDisabled),
            _ => Ok(()),
        }
    }

    /// Runs one sync cycle and maps account-level failures to account states.
    pub async fn sync(&self, trigger: Trigger) -> CoreResult<CycleReport> {
        self.sync_with(&self.engine, trigger).await
    }

    /// Runs one sync cycle with a specific engine (tests: crash points).
    pub async fn sync_with(
        &self,
        engine: &SyncEngine,
        trigger: Trigger,
    ) -> CoreResult<CycleReport> {
        let report = engine.run_cycle(self, trigger).await?;
        if let CycleOutcome::Failed(e) = &report.outcome {
            self.account_failure(e)?;
        }
        Ok(report)
    }

    /// Applies an account-level failure: disabled / deletion pending / session ended.
    pub fn account_failure(&self, e: &NetError) -> CoreResult<()> {
        let status = match e {
            NetError::AccountDisabled => "disabled",
            NetError::AccountDeletionPending => "deletion_pending",
            NetError::Unauthorized => {
                let mut g = self.lock();
                g.session_changed = true;
                return Ok(());
            }
            _ => return Ok(()),
        };
        self.write(|c, _| {
            account::update(c, |a| status.clone_into(&mut a.status))?;
            Ok(((), Topics::ACCOUNT))
        })?;
        self.lock().session_changed = true;
        Ok(())
    }

    /// Whether the session state may have changed since last asked (and clears the flag).
    pub fn take_session_changed(&self) -> bool {
        std::mem::take(&mut self.lock().session_changed)
    }

    /// The app-level state of this account.
    pub fn session_state(&self) -> CoreResult<SessionState> {
        let g = self.lock();
        let conn = g.db.conn();
        let Some(summary) = build::account_summary(conn)? else {
            return Err(CoreError::Internal("session without account row".into()));
        };
        let a = account::get(conn)?.ok_or(CoreError::NotSignedIn)?;
        let has_tokens = tokens::get(conn)?.is_some();
        let unsynced = outbox::unsynced_count(conn)?;
        let state = |kind| SessionState {
            account: Some(summary.clone()),
            ..SessionState::of(kind)
        };
        Ok(match account_mode(&a.status, a.password_change_required) {
            AccountMode::Disabled => SessionState {
                unsynced_ops: unsynced,
                ..state(SessionKind::Disabled)
            },
            AccountMode::DeletionPending => {
                let at = a.deletion_at.as_deref().map(build::ts);
                SessionState {
                    days_remaining: at.map(|t| build::days_until(self.env.clock.now(), t)),
                    deletion_at: at,
                    unsynced_ops: unsynced,
                    ..state(SessionKind::DeletionPending)
                }
            }
            _ if !has_tokens => SessionState::of(SessionKind::SignedOut),
            AccountMode::PasswordChangeRequired => state(SessionKind::PasswordChangeRequired),
            AccountMode::Active => state(SessionKind::Active),
        })
    }

    /// Attaches the notification-ops stream (Dart's adapter); buffered ops are flushed.
    pub fn attach_notifications(&self, sink: Box<dyn ViewSink<NotificationOp>>) {
        let mut g = self.lock();
        let buffered = std::mem::take(&mut g.notify_buffer);
        let mut open = true;
        for op in buffered {
            open = open && sink.emit(op);
        }
        if open {
            g.notify_sinks.push(sink);
        }
    }

    /// Recomputes the reminder plan (app start, resume, after sync).
    pub fn recompute_notifications(&self) -> CoreResult<()> {
        let mut g = self.lock();
        let ctx = self.ctx_of(&g);
        let ops = notify::recompute(g.db.conn(), ctx.now, ctx.tz, ctx.notification_mode)?;
        Self::emit_notifications(&mut g, ops);
        let Inner { db, hub, .. } = &mut *g;
        hub.notify(db.conn(), &ctx, Topics::SETTINGS)
    }

    /// Linux timer: shows reminders whose time has come. Returns the next fire time.
    pub fn tick_notifications(&self) -> CoreResult<Option<DateTime<Utc>>> {
        let mut g = self.lock();
        if notification_mode(self.env.platform) != NotificationMode::WhileRunning {
            return Ok(None);
        }
        let now = self.env.clock.now();
        let ops = notify::due_now(g.db.conn(), now)?;
        Self::emit_notifications(&mut g, ops);
        notify::next_fire_at(g.db.conn())
    }

    /// Records the platform's result of a notification op.
    pub fn notification_result(&self, id: i32, result: NotificationResult) -> CoreResult<()> {
        let now = self.env.clock.now();
        self.write(|c, _| {
            let changed = notify::record_result(c, id, result, now)?;
            Ok((
                (),
                if changed {
                    Topics::SETTINGS
                } else {
                    Topics::NONE
                },
            ))
        })
    }

    /// Cancels every scheduled reminder (sign-out).
    pub fn cancel_all_notifications(&self) -> CoreResult<()> {
        let mut g = self.lock();
        let now = self.env.clock.now();
        let ops = notify::cancel_all(g.db.conn(), now, notification_mode(self.env.platform))?;
        Self::emit_notifications(&mut g, ops);
        Ok(())
    }

    /// Stops every stream (sign-out, switch).
    pub fn close(&self) {
        let mut g = self.lock();
        g.hub.clear();
        g.notify_sinks.clear();
    }

    /// Number of live view streams.
    pub fn watcher_count(&self) -> usize {
        self.lock().hub.len()
    }

    /// Unsynced ops.
    pub fn unsynced(&self) -> CoreResult<u32> {
        outbox::unsynced_count(self.lock().db.conn())
    }

    /// Closes the database and deletes the account's file (sign-out, wipe).
    pub(crate) fn destroy(self: Arc<Self>) -> CoreResult<()> {
        self.close();
        let path = self.lock().db.path().to_path_buf();
        drop(self);
        crate::store::delete_db_files(&path)
    }

    /// The server URL of this account.
    pub fn server_url(&self) -> CoreResult<String> {
        Ok(account::get(self.lock().db.conn())?
            .map(|a| a.server_url)
            .unwrap_or_default())
    }

    /// Whether a live note exists.
    pub fn note_exists(&self, id: &str) -> CoreResult<bool> {
        Ok(notes::current(self.lock().db.conn(), id)?.is_some())
    }
}

impl SyncHost for Session {
    fn db<R>(&self, f: impl FnOnce(&Connection, &str) -> CoreResult<(R, Topics)>) -> CoreResult<R> {
        self.write(f)
    }

    fn set_activity(&self, activity: SyncActivity) {
        let mut g = self.lock();
        if g.activity == activity {
            return;
        }
        g.activity = activity;
        let ctx = self.ctx_of(&g);
        let Inner { db, hub, .. } = &mut *g;
        if let Err(e) = hub.notify(db.conn(), &ctx, Topics::SYNC) {
            tracing::warn!("view refresh failed: {e}");
        }
    }

    fn set_connectivity(&self, connectivity: Connectivity) {
        let mut g = self.lock();
        if g.connectivity == connectivity {
            return;
        }
        g.connectivity = connectivity;
        let ctx = self.ctx_of(&g);
        let Inner { db, hub, .. } = &mut *g;
        if let Err(e) = hub.notify(db.conn(), &ctx, Topics::SYNC | Topics::SETTINGS) {
            tracing::warn!("view refresh failed: {e}");
        }
    }
}
