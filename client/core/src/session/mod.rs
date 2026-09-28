//! The composition root: one [`Session`] per signed-in account wires the account database,
//! the view hub, the sync engine, the token provider and the reminder planner together; the
//! [`Core`] owns the active session, the device registry and the session-state stream, and
//! implements sign-in, sign-up, sign-out and account switching (§12.7).
//!
//! Everything here is callable headless: the frb facade in [`crate::api`] only forwards to it.

mod account_ops;
pub mod ask;
mod core;
pub mod device_name;
mod editing;
mod intents;
mod online;

pub use self::core::{
    Core, CoreEnv, MIN_PASSWORD_LENGTH, NotifyHub, SyncApiFactory, password_strength,
};
pub use ask::AskState;
pub use intents::{NewTask, TaskEdit, candidate_item};

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rusqlite::Connection;
use ulid::Ulid;

use crate::auth::{AccountMode, CoreTokenProvider, TokenStore, account_mode};
use crate::error::{CoreError, CoreResult};
use crate::format::labels::Lang;
use crate::net::{NetError, Tokens};
use crate::notify;
use crate::store::{AccountDb, account, cache, notes, outbox, tokens};
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
    notify_buffer: Vec<NotificationOp>,
    session_changed: bool,
    ask: AskState,
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
        let server_url = env.server_url.clone();
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
                    notify_buffer: Vec::new(),
                    session_changed: false,
                    ask: AskState::default(),
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

    fn ctx_of(&self, g: &Inner) -> ViewCtx {
        let a = account::get(g.db.conn()).ok().flatten();
        ViewCtx {
            now: self.env.clock.now(),
            tz: a
                .as_ref()
                .and_then(|a| a.timezone.parse::<Tz>().ok())
                .unwrap_or(chrono_tz::UTC),
            connectivity: g.connectivity,
            activity: g.activity.clone(),
            notification_mode: notification_mode(self.env.platform),
            lang: a.map_or_else(Lang::default, |a| Lang::from_code(&a.ui_language)),
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
            self.emit_notifications(g, ops);
            let ctx = self.ctx_of(g);
            let Inner { db, hub, .. } = &mut *g;
            hub.notify(db.conn(), &ctx, Topics::SETTINGS)?;
        }
        Ok(())
    }

    fn emit_notifications(&self, g: &mut Inner, ops: Vec<NotificationOp>) {
        if ops.is_empty() {
            return;
        }
        let hub = &self.env.notifications;
        if !hub.is_attached() {
            g.notify_buffer.extend(ops);
            return;
        }
        for op in ops {
            hub.emit(&op);
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

    /// Pulls without pushing (queued ops are rebased onto what arrives).
    pub async fn pull(&self) -> CoreResult<CycleReport> {
        let report = self.engine.pull(self).await?;
        if let CycleOutcome::Failed(e) = &report.outcome {
            self.account_failure(e)?;
        }
        Ok(report)
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

    /// Subscribes to `/events`, resuming after the last seq this device received. A device
    /// that never received one resumes from 0: the server replays what it still buffers or
    /// answers `reset`, so a change made between the last pull and the (lazy) connection is
    /// never missed (subscribing "from now" would drop it).
    pub fn subscribe_events(&self) -> CoreResult<Box<dyn crate::net::EventStream>> {
        let resume = self
            .read(|c, _| Ok(crate::store::sync_state::get(c)?.events_seq))?
            .or(Some(0));
        Ok(self
            .env
            .events_api
            .subscribe(&self.server_url(), self.tokens(), resume)?)
    }

    /// Applies one `/events` signal: the resume point is saved; a change or a reset asks for a
    /// pull (`true`); an account closure moves the account to its restricted state (`false`,
    /// the stream ends).
    pub fn handle_event(&self, signal: &crate::net::EventSignal) -> CoreResult<bool> {
        use crate::net::EventSignal;
        let (seq, pull) = match signal {
            EventSignal::Changed { seq } | EventSignal::Reset { seq } => (*seq, true),
            EventSignal::AccountClosed { seq, .. } => (*seq, false),
        };
        self.write(|c, now| {
            crate::store::sync_state::update(c, |s| s.events_seq = Some(seq))?;
            match signal {
                EventSignal::Reset { .. } => {
                    crate::store::cache::log(c, now, "reset", "")?;
                }
                EventSignal::AccountClosed { reason, .. } => {
                    crate::store::cache::log(c, now, "account", reason)?;
                }
                EventSignal::Changed { .. } => {}
            }
            Ok(((), Topics::SYNC))
        })?;
        if let EventSignal::AccountClosed { reason, .. } = signal {
            let e = match reason.as_str() {
                "deletion_pending" => NetError::AccountDeletionPending,
                "disabled" => NetError::AccountDisabled,
                _ => NetError::Unauthorized,
            };
            self.account_failure(&e)?;
        }
        Ok(pull)
    }

    /// Whether the session state may have changed since last asked (and clears the flag).
    pub fn take_session_changed(&self) -> bool {
        std::mem::take(&mut self.lock().session_changed)
    }

    /// The app-level state of this account.
    pub fn session_state(&self) -> CoreResult<SessionState> {
        let g = self.lock();
        let conn = g.db.conn();
        let ctx = self.ctx_of(&g);
        let labels = ctx.labels();
        let Some(summary) = build::account_summary(conn, &self.env.server_url)? else {
            return Err(CoreError::Internal("session without account row".into()));
        };
        let a = account::get(conn)?.ok_or(CoreError::NotSignedIn)?;
        let has_tokens = tokens::get(conn)?.is_some();
        let unsynced = outbox::unsynced_count(conn)?;
        let devices = build::device_items(conn, &ctx)?;
        let pending_approvals = crate::store::cache::get::<u32>(conn, cache::ADMIN_PENDING)?
            .map(|(n, _)| n)
            .filter(|_| summary.is_admin);
        let export = crate::store::cache::get::<(u64, u32)>(conn, cache::EXPORT)?.map(|(e, _)| e);
        let state = |kind| SessionState {
            account: Some(summary.clone()),
            this_device: devices
                .as_ref()
                .and_then(|d| d.iter().find(|d| d.is_this_device).cloned()),
            device_count: devices
                .as_ref()
                .map(|d| u32::try_from(d.len()).unwrap_or(u32::MAX)),
            pending_approvals,
            export_size_bytes: export.map(|e| e.0),
            export_note_count: export.map(|e| e.1),
            export_label: export.map(|(b, n)| build::export_label(&labels, b, n)),
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
                    deletion_label: at.map(|t| labels.date_long(labels.local(t).date())),
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

    /// Sends ops buffered while no notification stream was attached.
    pub fn flush_notifications(&self) {
        let mut g = self.lock();
        let buffered = std::mem::take(&mut g.notify_buffer);
        for op in buffered {
            self.env.notifications.emit(&op);
        }
    }

    /// Recomputes the reminder plan (app start, resume, after sync).
    pub fn recompute_notifications(&self) -> CoreResult<()> {
        let mut g = self.lock();
        let ctx = self.ctx_of(&g);
        let ops = notify::recompute(g.db.conn(), ctx.now, ctx.tz, ctx.notification_mode)?;
        self.emit_notifications(&mut g, ops);
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
        self.emit_notifications(&mut g, ops);
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
        self.emit_notifications(&mut g, ops);
        Ok(())
    }

    /// Stops every stream (sign-out, switch).
    pub fn close(&self) {
        let mut g = self.lock();
        g.hub.clear();
        g.ask.stop();
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

    /// The server URL of this account: the build's server, the same for every account.
    pub fn server_url(&self) -> String {
        self.env.server_url.clone()
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
