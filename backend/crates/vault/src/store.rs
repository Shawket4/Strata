//! The vault service and its per-user single-writer actors (PLAN §5.2, §7.2).
//!
//! Every write for a user runs on that user's actor: one tokio task draining an mpsc queue,
//! so no two writes to a vault ever race, while different users' actors run independently.
//! Reads go straight to the index and the files (atomic renames make every read see a whole
//! file). An actor loads (and reconciles, §7.3) its vault when it starts.
//!
//! A write is: compute the new file contents → write them atomically → one git commit →
//! re-derive the index rows of every affected note inside the user's scoped transaction,
//! append to the change log and enqueue jobs → commit the transaction.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use chrono::{DateTime, FixedOffset, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use strata_common::{Clock, IdGenerator, JobId, NoteId, UserId};
use strata_index::repo::jobs::{self, NewJob};
use strata_index::repo::sync::{self, NewChange};
use strata_index::types::ChangeOp;
use strata_index::{AppDb, ScopedTx, UserScope};
use tokio::sync::{mpsc, oneshot};
use vault_format::sidecar::NoteSidecar;

use crate::derive::{self, Context, Derived};
use crate::error::{Result, VaultError};
use crate::events::{CommitListener, Committed, ListenerSlot};
use crate::git::{self, FileChange};
use crate::paths;
use crate::state::{NoteMeta, VaultState, name_key};
use crate::{fsio, indexer};

/// Settings key of the user's IANA time zone (written by `PATCH /me`).
pub const SETTING_TIMEZONE: &str = "timezone";

/// Limits on `POST /import` archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportLimits {
    /// Most entries in an archive.
    pub max_entries: usize,
    /// Largest uncompressed entry.
    pub max_entry_bytes: u64,
    /// Largest total uncompressed size.
    pub max_total_bytes: u64,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_entry_bytes: 64 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Vault service configuration.
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Data root (`<root>/users/<id>/vault`).
    pub data_root: PathBuf,
    /// Time zone for users without one.
    pub default_timezone: String,
    /// Import limits.
    pub import: ImportLimits,
    /// Per-kind near-duplicate thresholds (setting overrides of the `domain` defaults).
    pub near_thresholds: BTreeMap<String, f32>,
}

impl VaultConfig {
    /// A configuration with defaults for `data_root`.
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            default_timezone: "UTC".to_owned(),
            import: ImportLimits::default(),
            near_thresholds: BTreeMap::new(),
        }
    }
}

type Job = Box<dyn for<'a> FnOnce(&'a mut Core) -> BoxFuture<'a, ()> + Send>;

pub(crate) struct Inner {
    pub(crate) config: VaultConfig,
    pub(crate) db: AppDb,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) ids: Arc<dyn IdGenerator>,
    actors: Mutex<HashMap<UserId, mpsc::UnboundedSender<Job>>>,
    pub(crate) loaded: Mutex<std::collections::HashSet<UserId>>,
    runtime: Option<tokio::runtime::Handle>,
    listener: ListenerSlot,
    semantic: std::sync::RwLock<Option<Arc<dyn crate::semantic::SemanticDuplicates>>>,
    #[cfg(feature = "fault-injection")]
    crash: Mutex<Option<CrashPoint>>,
}

/// The vault store for all users (cheap to clone).
#[derive(Clone)]
pub struct VaultService {
    pub(crate) inner: Arc<Inner>,
}

impl fmt::Debug for VaultService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultService")
            .field("data_root", &self.inner.config.data_root)
            .finish_non_exhaustive()
    }
}

impl VaultService {
    /// A service over `db` (the `strata_app` handle). Actors are spawned on the runtime that is
    /// current here, if any, else on the caller's.
    pub fn new(
        config: VaultConfig,
        db: AppDb,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                db,
                clock,
                ids,
                actors: Mutex::new(HashMap::new()),
                loaded: Mutex::new(std::collections::HashSet::new()),
                runtime: tokio::runtime::Handle::try_current().ok(),
                listener: std::sync::RwLock::new(None),
                semantic: std::sync::RwLock::new(None),
                #[cfg(feature = "fault-injection")]
                crash: Mutex::new(None),
            }),
        }
    }

    /// The configuration.
    pub fn config(&self) -> &VaultConfig {
        &self.inner.config
    }

    /// `<data_root>/users/<user>`.
    pub fn user_dir(&self, user: UserId) -> PathBuf {
        self.inner
            .config
            .data_root
            .join("users")
            .join(user.to_string())
    }

    /// `<data_root>/users/<user>/vault`.
    pub fn vault_dir(&self, user: UserId) -> PathBuf {
        self.user_dir(user).join("vault")
    }

    /// Registers the receiver of commit notices (the event bus); replaces any earlier one.
    pub fn set_listener(&self, listener: Arc<dyn CommitListener>) {
        *self
            .inner
            .listener
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(listener);
    }

    /// Registers the semantic level of the duplicate check on create (the AI subsystem,
    /// PLAN §9.7); replaces any earlier one.
    pub fn set_semantic(&self, source: Arc<dyn crate::semantic::SemanticDuplicates>) {
        *self
            .inner
            .semantic
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(source);
    }

    fn sender(&self, user: UserId) -> mpsc::UnboundedSender<Job> {
        let mut actors = self
            .inner
            .actors
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(tx) = actors.get(&user)
            && !tx.is_closed()
        {
            return tx.clone();
        }
        let (tx, rx) = mpsc::unbounded_channel::<Job>();
        let core = Core {
            user,
            dir: self.vault_dir(user),
            inner: self.inner.clone(),
            state: None,
            repair: false,
            pending: Committed::default(),
            merge_hint: None,
            receipt: None,
        };
        let task = run_actor(core, rx);
        match &self.inner.runtime {
            Some(handle) => drop(handle.spawn(task)),
            None => drop(tokio::spawn(task)),
        }
        actors.insert(user, tx.clone());
        tx
    }

    /// Runs `f` on the user's writer actor, after every write queued before it and before
    /// every write queued after it.
    pub async fn exec<R, F>(&self, scope: &UserScope, f: F) -> Result<R>
    where
        R: Send + 'static,
        F: for<'a> FnOnce(&'a mut Core, UserScope) -> BoxFuture<'a, Result<R>> + Send + 'static,
    {
        self.run(scope, true, f).await
    }

    /// Like [`Self::exec`] but without loading the vault first (for reconciliation itself).
    pub async fn exec_unloaded<R, F>(&self, scope: &UserScope, f: F) -> Result<R>
    where
        R: Send + 'static,
        F: for<'a> FnOnce(&'a mut Core, UserScope) -> BoxFuture<'a, Result<R>> + Send + 'static,
    {
        self.run(scope, false, f).await
    }

    async fn run<R, F>(&self, scope: &UserScope, load: bool, f: F) -> Result<R>
    where
        R: Send + 'static,
        F: for<'a> FnOnce(&'a mut Core, UserScope) -> BoxFuture<'a, Result<R>> + Send + 'static,
    {
        let scope = *scope;
        let (done_tx, done_rx) = oneshot::channel();
        // A pushed op's receipt in scope of the caller applies to the write this job makes.
        let receipt = crate::receipt::current();
        let queued_at = std::time::Instant::now();
        let job: Job = Box::new(move |core: &mut Core| {
            Box::pin(async move {
                crate::prof::add("actor.queue", queued_at.elapsed());
                let _pj = crate::prof::g("actor.job");
                core.receipt = receipt;
                let loaded = if load {
                    core.ensure_loaded(scope).await
                } else if core.dir.join(".git").is_dir() {
                    Ok(())
                } else {
                    Err(VaultError::NotFound)
                };
                let result = match loaded {
                    Ok(()) => f(core, scope).await,
                    Err(e) => Err(e),
                };
                core.receipt = None;
                let _ = done_tx.send(result);
            })
        });
        self.sender(scope.user_id())
            .send(job)
            .map_err(|_| VaultError::WriterGone)?;
        // A dropped sender means the job panicked (the actor survives and reloads).
        done_rx.await.map_err(|_| VaultError::WriterGone)?
    }

    /// Brings the user's vault to a consistent state before a sync push: loads it if the
    /// writer has no state (first use, or after a failed write), which rolls back an
    /// interrupted write and recovers op results committed to git but not to the database
    /// (see [`crate::receipt`]).
    pub async fn recover(&self, scope: &UserScope) -> Result<()> {
        self.exec(scope, |_, _| Box::pin(async { Ok(()) })).await
    }

    /// Makes the next write stop at `point` as if the process died there (one shot; tests).
    #[cfg(feature = "fault-injection")]
    pub fn inject_crash(&self, point: CrashPoint) {
        *self
            .inner
            .crash
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(point);
    }

    /// Stops the user's actor (its state is reloaded — and the vault reconciled — on next
    /// use). Used after out-of-band maintenance and by tests.
    pub fn evict(&self, user: UserId) {
        self.inner
            .actors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&user);
        self.inner
            .loaded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&user);
    }
}

impl Inner {
    /// The registered semantic duplicate source, if any.
    pub(crate) fn semantic(&self) -> Option<Arc<dyn crate::semantic::SemanticDuplicates>> {
        self.semantic
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn notify(&self, user: UserId, notice: &Committed) {
        if notice.is_empty() {
            return;
        }
        let listener = self
            .listener
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(l) = listener {
            l.committed(user, notice);
        }
    }
}

async fn run_actor(mut core: Core, mut rx: mpsc::UnboundedReceiver<Job>) {
    while let Some(job) = rx.recv().await {
        let outcome = AssertUnwindSafe(job(&mut core)).catch_unwind().await;
        if outcome.is_err() {
            tracing::error!(user = %core.user, "vault job panicked; reloading the vault state");
            core.state = None;
            core.repair = true;
            core.pending = Committed::default();
            core.merge_hint = None;
            core.receipt = None;
        }
    }
}

/// A point inside a write where a crash can be injected (feature `fault-injection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashPoint {
    /// The files are written, nothing is committed.
    AfterFileWrite,
    /// The git commit exists; the database transaction (index, change log, op result) is
    /// not committed.
    AfterGitCommit,
    /// Everything is committed; the caller never hears back.
    AfterDbCommit,
}

/// Who made a change: the commit message prefix (PLAN §7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Author {
    /// `user: <op> <path>`.
    User,
    /// `ai: <job> <path>`.
    Ai(String),
    /// `system: <what>`.
    System,
}

impl Author {
    /// The commit message for `op` on `subject`.
    pub fn message(&self, op: &str, subject: &str) -> String {
        match self {
            Self::User => format!("user: {op} {subject}"),
            Self::Ai(job) => format!("ai: {job} {subject}"),
            Self::System => format!("system: {op} {subject}"),
        }
    }
}

/// What a sync of changed paths did.
#[derive(Debug, Clone, Default)]
pub struct Synced {
    /// Notes whose content changed (new version), live or trashed.
    pub changed: Vec<NoteId>,
    /// Notes that no longer exist.
    pub removed: Vec<NoteId>,
}

/// The writer's state for one vault. Only the actor holds it.
pub struct Core {
    pub(crate) user: UserId,
    pub(crate) dir: PathBuf,
    pub(crate) inner: Arc<Inner>,
    pub(crate) state: Option<VaultState>,
    /// Set after a failed or panicked write: the next load repairs the index from the files.
    pub(crate) repair: bool,
    /// What the write in progress changed; handed to the listener after its commit.
    pub(crate) pending: Committed,
    /// `(loser, survivor)` of the entity merge in progress.
    pub(crate) merge_hint: Option<(NoteId, NoteId)>,
    /// The receipt of the pushed op the current job applies, if any.
    pub(crate) receipt: Option<Arc<crate::receipt::Slot>>,
}

impl fmt::Debug for Core {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Core")
            .field("user", &self.user)
            .field("loaded", &self.state.is_some())
            .finish_non_exhaustive()
    }
}

/// Runs blocking filesystem/git work on the blocking pool.
pub(crate) async fn blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| VaultError::Internal(format!("blocking task failed: {e}")))?
}

impl Core {
    /// The vault directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The loaded state.
    pub(crate) fn state(&self) -> Result<&VaultState> {
        self.state
            .as_ref()
            .ok_or_else(|| VaultError::Internal("vault state not loaded".into()))
    }

    pub(crate) fn state_mut(&mut self) -> Result<&mut VaultState> {
        self.state
            .as_mut()
            .ok_or_else(|| VaultError::Internal("vault state not loaded".into()))
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.inner.clock.now()
    }

    pub(crate) fn ids(&self) -> &dyn IdGenerator {
        self.inner.ids.as_ref()
    }

    /// Opens a transaction scoped to this vault's user.
    pub(crate) async fn begin(&self, scope: &UserScope) -> Result<ScopedTx> {
        if scope.user_id() != self.user {
            return Err(VaultError::Internal(
                "scope does not match the vault".into(),
            ));
        }
        Ok(self.inner.db.begin(scope).await?)
    }

    /// Loads the vault (reconciling it) if this actor has not yet.
    async fn ensure_loaded(&mut self, scope: UserScope) -> Result<()> {
        if self.state.is_some() {
            return Ok(());
        }
        if !self.dir.join(".git").is_dir() {
            return Err(VaultError::NotFound);
        }
        let (dir, at) = (self.dir.clone(), self.now());
        blocking(move || crate::archive::complete_init(&dir, at)).await?;
        let repair = self.repair;
        crate::reconcile::reconcile(self, scope, repair).await?;
        self.repair = false;
        Ok(())
    }

    /// The user's time zone (setting, else the configured default, else UTC).
    pub(crate) async fn tz(&self, tx: &mut ScopedTx) -> Result<Tz> {
        let setting = strata_index::repo::settings::get_setting(tx, SETTING_TIMEZONE).await?;
        let name: Option<String> = setting.and_then(|b| rmp_serde::from_slice(&b).ok());
        Ok(name
            .and_then(|n| n.parse::<Tz>().ok())
            .or_else(|| self.inner.config.default_timezone.parse::<Tz>().ok())
            .unwrap_or(Tz::UTC))
    }

    /// Now in the user's time zone as a fixed offset (for frontmatter timestamps).
    pub(crate) fn local_now(&self, tz: Tz) -> DateTime<FixedOffset> {
        let now = self.now();
        let local = tz.from_utc_datetime(&now.naive_utc());
        local.with_timezone(&local.offset().fix())
    }

    /// Reads a vault file.
    pub(crate) async fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let dir = self.dir.clone();
        let rel = rel.to_owned();
        blocking(move || Ok(fsio::read(&dir, &rel)?)).await
    }

    /// Reads a vault file as UTF-8 text (lossless for notes Strata wrote).
    pub(crate) async fn read_text(&self, rel: &str) -> Result<Option<String>> {
        Ok(self
            .read(rel)
            .await?
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// The sidecar of `id`, if it exists and parses.
    pub(crate) async fn sidecar(&self, id: NoteId) -> Result<Option<NoteSidecar>> {
        let path = NoteSidecar::path_for(id.as_ulid());
        Ok(self
            .read_text(&path)
            .await?
            .and_then(|t| NoteSidecar::from_json(&t).ok()))
    }

    /// The file change that stores `sidecar` (or removes it when it holds nothing).
    pub(crate) fn sidecar_change(sidecar: &NoteSidecar) -> Result<FileChange> {
        let path = NoteSidecar::path_for(sidecar.id);
        let empty = sidecar.summary.is_none()
            && sidecar.relations.is_empty()
            && sidecar.rejected.is_empty()
            && sidecar.keep_both.is_empty()
            && sidecar.content_hash.is_none()
            && sidecar.last_linked_hash.is_none()
            && sidecar.extra.is_empty();
        if empty {
            return Ok((path, None));
        }
        let json = sidecar
            .to_json()
            .map_err(|e| VaultError::Internal(format!("sidecar encoding: {e}")))?;
        Ok((path, Some(json.into_bytes())))
    }

    /// Journals the write, then writes the changes atomically (not committed yet).
    pub(crate) async fn write_files(&mut self, changes: &[FileChange]) -> Result<()> {
        let dir = self.dir.clone();
        let changes = changes.to_vec();
        blocking(move || {
            let _pw = crate::prof::g("write.files_total");
            let paths: Vec<String> = changes.iter().map(|(p, _)| p.clone()).collect();
            let pt = crate::prof::g("write.journal_begin");
            crate::journal::begin(&dir, &paths)?;
            drop(pt);
            for (path, content) in &changes {
                match content {
                    Some(bytes) => fsio::atomic_write(&dir, path, bytes)?,
                    None => fsio::remove(&dir, path)?,
                }
            }
            Ok(())
        })
        .await
    }

    /// Commits the written `paths` (one commit) and closes the journal. Returns the commit
    /// ID, or `None` when the files already had this content.
    pub(crate) async fn commit_files(
        &mut self,
        paths: Vec<String>,
        message: String,
    ) -> Result<Option<String>> {
        let dir = self.dir.clone();
        let at = self.now();
        blocking(move || {
            let pt = crate::prof::g("git.commit_paths");
            let commit = git::commit_paths(&dir, &paths, &message, at)?;
            drop(pt);
            let pt = crate::prof::g("git.journal_end");
            crate::journal::end(&dir)?;
            drop(pt);
            Ok(commit)
        })
        .await
    }

    /// After a failed write: reload (roll back or recover) before the next one.
    pub(crate) fn failed(&mut self) {
        self.state = None;
        self.repair = true;
        self.drop_notice();
    }

    /// Stops here if a crash was injected at `point` (feature `fault-injection`).
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)] // no-op without the feature
    pub(crate) fn crash_point(&self, point: CrashPoint) -> Result<()> {
        #[cfg(feature = "fault-injection")]
        {
            let mut armed = self
                .inner
                .crash
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if *armed == Some(point) {
                *armed = None;
                return Err(VaultError::Internal(format!("injected crash {point:?}")));
            }
        }
        let _ = point;
        Ok(())
    }

    /// Brings the state and the index up to date with the files at `paths` (after they were
    /// written, reverted, imported or found changed): re-derives every note whose file
    /// changed and every note whose links may now resolve differently, purges notes that are
    /// gone, appends the change log and enqueues jobs. Runs in `tx`.
    #[allow(clippy::too_many_lines)] // one linear pass over the changed paths
    pub(crate) async fn sync_paths(
        &mut self,
        tx: &mut ScopedTx,
        paths: &BTreeSet<String>,
    ) -> Result<Synced> {
        let _ps = crate::prof::g("sync_paths");
        let pt = crate::prof::g("sync.tz");
        let tz = self.tz(tx).await?;
        drop(pt);
        let pt = crate::prof::g("sync.read_contents");
        let mut contents: BTreeMap<String, Option<String>> = BTreeMap::new();
        for p in paths {
            let is_md = p.ends_with(".md");
            if paths::is_content(p) || p.starts_with(paths::TRASH_DIR) {
                let text = if is_md {
                    self.read_text(p).await?
                } else {
                    None
                };
                let exists = if is_md {
                    text.is_some()
                } else {
                    self.read(p).await?.is_some()
                };
                contents.insert(
                    p.clone(),
                    if exists {
                        Some(text.unwrap_or_default())
                    } else {
                        None
                    },
                );
            }
        }
        drop(pt);
        let pt = crate::prof::g("sync.state_update");
        let state = self.state_mut()?;
        let mut before_versions: HashMap<NoteId, (String, String)> = HashMap::new();
        // Live path and kind of every touched note before the change (for the notice).
        let mut before_live: HashMap<NoteId, (String, domain::NoteKind)> = HashMap::new();
        let mut touched: BTreeSet<NoteId> = BTreeSet::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        for p in paths {
            if let Some(id) = sidecar_id(p)
                && let Some((lp, m)) = state.note(id)
            {
                before_live.entry(id).or_insert((lp.to_owned(), m.kind));
            }
        }
        for p in paths {
            if let Some(m) = state.remove_note(p) {
                before_versions.insert(m.id, (p.clone(), m.version.clone()));
                before_live.insert(m.id, (p.clone(), m.kind));
                touched.insert(m.id);
                names.insert(name_key(p));
            }
            if let Some(m) = state.remove_trash(p) {
                before_versions.insert(m.id, (p.clone(), m.version.clone()));
                touched.insert(m.id);
            }
            if state.attachments.remove(p) {
                names.insert(name_key(p));
            }
            if let Some(id) = sidecar_id(p)
                && state.contains_id(id)
            {
                touched.insert(id);
            }
        }
        for (p, content) in &contents {
            let Some(text) = content else { continue };
            let live = paths::is_note(p);
            let trashed = p.ends_with(".md") && paths::untrash_path(p).is_some_and(paths::is_note);
            if live || trashed {
                let doc = vault_format::Document::parse(text);
                let Some(id) = doc
                    .frontmatter()
                    .and_then(|f| f.id().ok().flatten())
                    .map(NoteId::from_ulid)
                else {
                    continue;
                };
                if state.contains_id(id) {
                    // Duplicate ID: reconciliation re-assigns it; skip here.
                    continue;
                }
                let meta = NoteMeta {
                    id,
                    kind: derive::kind_of(&doc),
                    version: fsio::version_of(text.as_bytes()),
                    link_names: BTreeSet::new(),
                };
                touched.insert(id);
                if live {
                    names.insert(name_key(p));
                    state.put_note(p, meta);
                } else {
                    state.put_trash(p, meta);
                }
            } else if paths::is_content(p) {
                state.attachments.insert(p.clone());
                names.insert(name_key(p));
            }
        }
        drop(pt);
        let pt = crate::prof::g("sync.linking_to");
        let mut affected: BTreeSet<NoteId> = touched.clone();
        affected.extend(state.linking_to(&names));
        drop(pt);
        crate::prof::add("sync.affected_count(us=count)", std::time::Duration::from_micros(affected.len() as u64));
        let watch_sidecars = paths.iter().any(|p| sidecar_id(p).is_some());
        let pt = crate::prof::g("sync.snapshot_before");
        let before = crate::diff::Snapshot::take(tx, &affected, watch_sidecars).await?;
        drop(pt);
        let pt = crate::prof::g("sync.reindex_ids");
        let synced = self.reindex_ids(tx, &affected, tz).await?;
        drop(pt);
        let state = self.state()?;
        let mut out = Synced::default();
        for id in &touched {
            let now = state
                .note(*id)
                .map(|(p, m)| (p.to_owned(), m.version.clone()))
                .or_else(|| {
                    state
                        .trash_by_id
                        .get(id)
                        .and_then(|p| state.trash.get(p).map(|m| (p.clone(), m.version.clone())))
                });
            match now {
                Some(v) if before_versions.get(id) != Some(&v) => out.changed.push(*id),
                Some(_) => {}
                None => out.removed.push(*id),
            }
        }
        for id in &out.removed {
            indexer::purge(tx, *id, synced.get(id)).await?;
        }
        let notes = self.note_events(&touched, &before_live, &before_versions)?;
        let pt = crate::prof::g("sync.log_changes");
        self.log_changes(tx, &out).await?;
        drop(pt);
        let pt = crate::prof::g("sync.snapshot_after");
        let after = crate::diff::Snapshot::take(tx, &affected, watch_sidecars).await?;
        drop(pt);
        let pt = crate::prof::g("sync.derived_diff");
        let mut notice = self.log_derived_diff(tx, &before, &after).await?;
        drop(pt);
        notice.notes = notes;
        self.pending.absorb(notice);
        Ok(out)
    }

    /// The note part of a commit notice for the `touched` notes.
    fn note_events(
        &self,
        touched: &BTreeSet<NoteId>,
        before_live: &HashMap<NoteId, (String, domain::NoteKind)>,
        before_versions: &HashMap<NoteId, (String, String)>,
    ) -> Result<Vec<crate::events::NoteEvent>> {
        use crate::events::{NoteChange, NoteEvent};
        let state = self.state()?;
        let mut out = Vec::new();
        for id in touched {
            let now = state.note(*id);
            let before = before_live.get(id);
            let event = match (before, now) {
                (Some((old, _)), Some((path, m))) => {
                    let version_changed = before_versions
                        .get(id)
                        .is_some_and(|(_, v)| *v != m.version);
                    let change = if old != path {
                        NoteChange::Moved
                    } else if version_changed {
                        NoteChange::Updated
                    } else {
                        continue;
                    };
                    NoteEvent {
                        id: *id,
                        kind: m.kind,
                        change,
                        path: path.to_owned(),
                        old_path: (old != path).then(|| old.clone()),
                        version: Some(m.version.clone()),
                    }
                }
                (None, Some((path, m))) => NoteEvent {
                    id: *id,
                    kind: m.kind,
                    change: NoteChange::Created,
                    path: path.to_owned(),
                    old_path: None,
                    version: Some(m.version.clone()),
                },
                (Some((old, kind)), None) => NoteEvent {
                    id: *id,
                    kind: *kind,
                    change: NoteChange::Deleted,
                    path: old.clone(),
                    old_path: None,
                    version: None,
                },
                (None, None) => continue,
            };
            out.push(event);
        }
        Ok(out)
    }

    /// Appends change-log rows for derived records that are not in note text (relations with
    /// provenance, rejections, keep-both pairs) and returns the rest of the commit notice
    /// (relations, tasks, custody).
    pub(crate) async fn log_derived_diff(
        &self,
        tx: &mut ScopedTx,
        before: &crate::diff::Snapshot,
        after: &crate::diff::Snapshot,
    ) -> Result<Committed> {
        let now = self.now();
        let diff = crate::diff::diff(before, after);
        for (entity_type, entity_id, op) in &diff.log {
            sync::append_change(
                tx,
                &NewChange {
                    entity_type,
                    entity_id,
                    op: *op,
                    version: None,
                    at: now,
                },
            )
            .await?;
        }
        let mut notice = Committed {
            relations: diff.relations,
            ..Committed::default()
        };
        let state = self.state()?;
        for (task, note) in diff.tasks {
            let version = match &note {
                Some(n) => match state.note(*n) {
                    Some((p, _)) => self.read_text(p).await?.and_then(|t| {
                        let doc = vault_format::Document::parse(&t);
                        sync_model::apply::find_task(doc.body(), &task).map(|(span, _)| {
                            sync_model::apply::task_line_version(&doc.body()[span])
                                .as_str()
                                .to_owned()
                        })
                    }),
                    None => None,
                },
                None => None,
            };
            let note_id = note
                .or_else(|| before.task_note(&task))
                .unwrap_or_else(|| NoteId::from_ulid(ulid::Ulid::nil()));
            notice.tasks.push(crate::events::TaskEvent {
                id: task,
                note_id,
                version,
            });
        }
        for doc in diff.custody {
            notice.custody.push(crate::events::CustodyEvent {
                document_id: doc,
                version: state.note(doc).map(|(_, m)| m.version.clone()),
            });
        }
        Ok(notice)
    }

    /// Hands the pending notice to the listener (after the commit of `message`).
    pub(crate) fn flush_notice(&mut self, message: &str) {
        let mut notice = std::mem::take(&mut self.pending);
        notice.merged = self.merge_hint.take();
        notice.op = crate::events::op_label(message);
        notice.user = Some(self.user);
        self.inner.notify(self.user, &notice);
    }

    /// Starts a fresh notice for the write about to run (keeps a merge hint set for it).
    pub(crate) fn drop_notice_keep_merge(&mut self) {
        self.pending = Committed::default();
    }

    /// Drops the pending notice (the write failed).
    pub(crate) fn drop_notice(&mut self) {
        self.pending = Committed::default();
        self.merge_hint = None;
    }

    /// Re-derives the given notes (live or trashed) from their files and writes the rows.
    /// Returns the derived rows by ID (for removed notes: nothing).
    pub(crate) async fn reindex_ids(
        &mut self,
        tx: &mut ScopedTx,
        ids: &BTreeSet<NoteId>,
        tz: Tz,
    ) -> Result<HashMap<NoteId, Derived>> {
        let mut files: Vec<(NoteId, String, bool)> = Vec::new();
        {
            let state = self.state()?;
            for id in ids {
                if let Some((p, _)) = state.note(*id) {
                    files.push((*id, p.to_owned(), false));
                } else if let Some(p) = state.trash_by_id.get(id) {
                    files.push((*id, p.clone(), true));
                }
            }
        }
        let pt = crate::prof::g("reidx.read");
        let mut batch = Vec::with_capacity(files.len());
        let mut texts = Vec::with_capacity(files.len());
        for (id, path, trashed) in &files {
            let text = self.read_text(path).await?.unwrap_or_default();
            let sidecar = self.sidecar(*id).await?;
            texts.push((*id, path.clone(), *trashed, text, sidecar));
        }
        drop(pt);
        let pt = crate::prof::g("reidx.path_index");
        let state = self.state()?;
        let index = state.path_index();
        drop(pt);
        let pt = crate::prof::g("reidx.derive");
        let ctx = Context {
            index: &index,
            state,
            tz,
        };
        let mut link_names = Vec::new();
        let mut out = HashMap::new();
        for (id, path, trashed, text, sidecar) in &texts {
            if let Some(d) = derive::derive(path, text, sidecar.as_ref(), &ctx, *trashed) {
                link_names.push((*id, d.link_names.clone()));
                out.insert(*id, d.clone());
                batch.push(d);
            }
        }
        drop(pt);
        let pt = crate::prof::g("reidx.indexer_write");
        indexer::write(tx, &batch).await?;
        drop(pt);
        let state = self.state_mut()?;
        for (id, names) in link_names {
            if let Some(path) = state.by_id.get(&id).cloned()
                && let Some(m) = state.notes.get_mut(&path)
            {
                m.link_names = names;
            }
        }
        Ok(out)
    }

    /// Appends change-log rows and enqueues the AI jobs for changed notes (Phase 4 runs them;
    /// the rows are written now so nothing is lost).
    pub(crate) async fn log_changes(&self, tx: &mut ScopedTx, synced: &Synced) -> Result<()> {
        let now = self.now();
        let state = self.state()?;
        for id in &synced.changed {
            let live = state.note(*id);
            let (op, version) = match live {
                Some((_, m)) => (ChangeOp::Upsert, Some(m.version.clone())),
                None => (ChangeOp::Delete, None),
            };
            let id_text = id.to_string();
            sync::append_change(
                tx,
                &NewChange {
                    entity_type: "note",
                    entity_id: &id_text,
                    op,
                    version: version.as_deref(),
                    at: now,
                },
            )
            .await?;
            if let Some((path, _)) = live {
                let mut kinds = vec!["embed"];
                if derive::is_inbox(path) {
                    kinds.push("file_inbox");
                }
                for kind in kinds {
                    jobs::enqueue(
                        tx,
                        &NewJob {
                            id: JobId::generate(self.ids()),
                            kind: kind.to_owned(),
                            note_id: Some(*id),
                            payload: Vec::new(),
                            run_after: now,
                            max_attempts: 5,
                            dedupe_key: Some(id_text.clone()),
                        },
                        now,
                    )
                    .await?;
                }
            }
        }
        for id in &synced.removed {
            let id_text = id.to_string();
            sync::append_change(
                tx,
                &NewChange {
                    entity_type: "note",
                    entity_id: &id_text,
                    op: ChangeOp::Delete,
                    version: None,
                    at: now,
                },
            )
            .await?;
        }
        Ok(())
    }
}

/// The note ID of a sidecar path `.meta/notes/<id>.json`.
pub(crate) fn sidecar_id(path: &str) -> Option<NoteId> {
    let name = path.strip_prefix(".meta/notes/")?.strip_suffix(".json")?;
    name.parse().ok()
}
