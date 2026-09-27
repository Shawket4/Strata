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
        let job: Job = Box::new(move |core: &mut Core| {
            Box::pin(async move {
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
                let _ = done_tx.send(result);
            })
        });
        self.sender(scope.user_id())
            .send(job)
            .map_err(|_| VaultError::WriterGone)?;
        // A dropped sender means the job panicked (the actor survives and reloads).
        done_rx.await.map_err(|_| VaultError::WriterGone)?
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

async fn run_actor(mut core: Core, mut rx: mpsc::UnboundedReceiver<Job>) {
    while let Some(job) = rx.recv().await {
        let outcome = AssertUnwindSafe(job(&mut core)).catch_unwind().await;
        if outcome.is_err() {
            tracing::error!(user = %core.user, "vault job panicked; reloading the vault state");
            core.state = None;
            core.repair = true;
        }
    }
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
            return Err(VaultError::Internal("scope does not match the vault".into()));
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

    /// Writes the changes atomically and commits them (one commit). Returns the commit ID,
    /// or `None` when the files already had this content.
    pub(crate) async fn apply(
        &mut self,
        changes: Vec<FileChange>,
        message: String,
    ) -> Result<Option<String>> {
        let dir = self.dir.clone();
        let at = self.now();
        let result = blocking(move || {
            let mut paths = Vec::with_capacity(changes.len());
            for (path, content) in &changes {
                match content {
                    Some(bytes) => fsio::atomic_write(&dir, path, bytes)?,
                    None => fsio::remove(&dir, path)?,
                }
                paths.push(path.clone());
            }
            git::commit_paths(&dir, &paths, &message, at)
        })
        .await;
        if result.is_err() {
            // Files may be half-written: reload (and recover) before the next write.
            self.state = None;
            self.repair = true;
        }
        result
    }

    /// Brings the state and the index up to date with the files at `paths` (after they were
    /// written, reverted, imported or found changed): re-derives every note whose file
    /// changed and every note whose links may now resolve differently, purges notes that are
    /// gone, appends the change log and enqueues jobs. Runs in `tx`.
    pub(crate) async fn sync_paths(
        &mut self,
        tx: &mut ScopedTx,
        paths: &BTreeSet<String>,
    ) -> Result<Synced> {
        let tz = self.tz(tx).await?;
        let mut contents: BTreeMap<String, Option<String>> = BTreeMap::new();
        for p in paths {
            let is_md = p.ends_with(".md");
            if paths::is_content(p) || p.starts_with(paths::TRASH_DIR) {
                let text = if is_md { self.read_text(p).await? } else { None };
                let exists = if is_md {
                    text.is_some()
                } else {
                    self.read(p).await?.is_some()
                };
                contents.insert(p.clone(), if exists { Some(text.unwrap_or_default()) } else { None });
            }
        }
        let state = self.state_mut()?;
        let mut before_versions: HashMap<NoteId, (String, String)> = HashMap::new();
        let mut touched: BTreeSet<NoteId> = BTreeSet::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        for p in paths {
            if let Some(m) = state.remove_note(p) {
                before_versions.insert(m.id, (p.clone(), m.version.clone()));
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
        let mut affected: BTreeSet<NoteId> = touched.clone();
        affected.extend(state.linking_to(&names));
        let synced = self.reindex_ids(tx, &affected, tz).await?;
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
        self.log_changes(tx, &out).await?;
        Ok(out)
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
        let mut batch = Vec::with_capacity(files.len());
        let mut texts = Vec::with_capacity(files.len());
        for (id, path, trashed) in &files {
            let text = self.read_text(path).await?.unwrap_or_default();
            let sidecar = self.sidecar(*id).await?;
            texts.push((*id, path.clone(), *trashed, text, sidecar));
        }
        let state = self.state()?;
        let index = state.path_index();
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
        indexer::write(tx, &batch).await?;
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
