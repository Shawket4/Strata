//! Startup reconciliation, `verify` and `reindex` (PLAN §7.3).
//!
//! Only the API writes the vault, so a difference between the files, git and the index means
//! a crash or out-of-band tampering. Reconciliation:
//!
//! 1. removes leftover temporary files of interrupted atomic writes and rolls back a write
//!    that stopped before its git commit ([`crate::journal`]);
//! 2. gives notes without a (unique) ID one, and task lines without a block ID theirs;
//! 3. commits anything uncommitted as `system: recovered changes`;
//! 4. repairs sidecars from frontmatter (frontmatter wins for the existence of an edge; the
//!    sidecar only supplies provenance) and removes sidecars of notes that no longer exist,
//!    committed as `system: repair sidecars`;
//! 5. compares every note's hash and path with the index and re-derives what differs, plus
//!    every note whose links may resolve differently; notes gone from disk are purged;
//! 6. re-inserts the op results of commits the index never committed with (a crash between
//!    a write's git commit and its database commit, [`crate::receipt`]) and records `HEAD` as
//!    `sync_epochs.vault_head`;
//! 7. records an integrity warning for each finding (surfaced by `GET /integrity`).
//!
//! A full reindex instead deletes every derived row and derives the whole vault again, and
//! rebuilds the disambiguation hints from their mirror in the entity sidecars (§9.8).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use strata_common::{HintId, NoteId};
use strata_index::repo::entities as erepo;
use strata_index::repo::notes;
use strata_index::repo::vault::{self as vrepo, IntegrityWarning};
use strata_index::{ScopedTx, UserScope};
use vault_format::Document;
use vault_format::sidecar::NoteSidecar;

use crate::derive::{self, Context, Derived};
use crate::error::Result;
use crate::prepare;
use crate::state::{NoteMeta, VaultState, name_key};
use crate::store::{Core, Synced, blocking, sidecar_id};
use crate::{fsio, git, indexer, paths};

/// Integrity warning kinds.
pub mod kinds {
    /// A leftover temporary file of an interrupted write was removed.
    pub const TEMP_FILE: &str = "temp_file_removed";
    /// Uncommitted changes were committed as `system: recovered changes`.
    pub const UNCOMMITTED: &str = "uncommitted_changes";
    /// A note was given an ID (or a duplicate ID was replaced).
    pub const ID_ASSIGNED: &str = "id_assigned";
    /// A file differs from the index (changed outside the API).
    pub const OUT_OF_BAND: &str = "out_of_band_edit";
    /// A note in the index has no file any more.
    pub const MISSING_FILE: &str = "missing_file";
    /// A sidecar disagreed with its note's frontmatter and was repaired.
    pub const SIDECAR: &str = "sidecar_repaired";
    /// A sidecar without a note was removed.
    pub const ORPHAN_SIDECAR: &str = "orphan_sidecar_removed";
    /// The index was repaired after a failed write.
    pub const INDEX_REPAIRED: &str = "index_repaired";
    /// A write that stopped before its commit was undone.
    pub const WRITE_ROLLED_BACK: &str = "interrupted_write_rolled_back";
    /// A pushed op's result was recovered from its commit.
    pub const OP_RESULT_RECOVERED: &str = "op_result_recovered";
}

/// Most commits walked when looking for op results the index never committed.
const RECOVERY_WALK_LIMIT: usize = 10_000;

/// What reconciliation found and did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Temporary files removed.
    pub temp_files_removed: Vec<String>,
    /// Paths restored because the write changing them never committed.
    pub rolled_back: Vec<String>,
    /// Paths committed as recovered changes.
    pub recovered: Vec<String>,
    /// Notes that were given an ID.
    pub ids_assigned: Vec<String>,
    /// The recovery commit.
    pub recovery_commit: Option<String>,
    /// Sidecars repaired or removed.
    pub sidecars_repaired: Vec<String>,
    /// Notes whose file differed from the index (re-derived).
    pub out_of_band: Vec<String>,
    /// Notes whose file is gone (purged from the index).
    pub missing: Vec<String>,
    /// Notes re-derived in total.
    pub reindexed: usize,
    /// Op IDs whose results were recovered from commit trailers.
    pub ops_recovered: Vec<String>,
}

impl Report {
    /// True if nothing was wrong.
    pub fn is_clean(&self) -> bool {
        self.temp_files_removed.is_empty()
            && self.rolled_back.is_empty()
            && self.recovered.is_empty()
            && self.ids_assigned.is_empty()
            && self.sidecars_repaired.is_empty()
            && self.out_of_band.is_empty()
            && self.missing.is_empty()
            && self.ops_recovered.is_empty()
    }
}

struct Scanned {
    temp: Vec<String>,
    rolled_back: Vec<String>,
    dirty: Vec<String>,
    assigned: Vec<String>,
    commit: Option<String>,
    files: Vec<String>,
    texts: BTreeMap<String, String>,
}

/// Blocking part: temp files, IDs, recovery commit, and every note's text.
fn scan_and_recover(
    dir: &std::path::Path,
    ids: &dyn strata_common::IdGenerator,
    at: chrono::DateTime<chrono::Utc>,
    write: bool,
) -> Result<Scanned> {
    let temp = fsio::temp_files(dir, write)?;
    let rolled_back = if write {
        crate::journal::recover(dir)?
    } else {
        Vec::new()
    };
    let dirty = git::dirty_paths(dir)?;
    let files = fsio::scan(dir)?;
    let mut texts = BTreeMap::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_tasks = BTreeSet::new();
    let mut assigned = Vec::new();
    for f in &files {
        let is_trash_note =
            f.ends_with(".md") && paths::untrash_path(f).is_some_and(paths::is_note);
        if !(paths::is_note(f) || is_trash_note) {
            continue;
        }
        let Some(bytes) = fsio::read(dir, f)? else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut doc = Document::parse(&text);
        let id = doc.frontmatter().and_then(|fm| fm.id().ok().flatten());
        let editable = doc.frontmatter().is_none_or(|fm| fm.error().is_none());
        let mut new_text = text.clone();
        if editable {
            let needs_id = id.is_none_or(|i| seen_ids.contains(&i));
            let final_id = if needs_id {
                NoteId::generate(ids)
            } else {
                NoteId::from_ulid(id.unwrap_or_default())
            };
            if needs_id && prepare::stamp(&mut doc, final_id, None, None).is_ok() {
                assigned.push(f.clone());
            }
            let body = prepare::assign_task_ids(doc.body(), ids, &seen_tasks);
            if body != doc.body() {
                doc.set_body(body);
            }
            new_text = doc.render();
            for t in vault_format::tasks::extract_tasks(doc.body()) {
                if let Some(b) = t.task.block_id() {
                    seen_tasks.insert(b.to_owned());
                }
            }
            seen_ids.insert(final_id.as_ulid());
        } else if let Some(i) = id {
            seen_ids.insert(i);
        }
        if new_text != text {
            if write {
                fsio::atomic_write(dir, f, new_text.as_bytes())?;
            }
            if !assigned.contains(f) {
                assigned.push(f.clone());
            }
        }
        // Dry run: the index is compared with the file as it is.
        texts.insert(f.clone(), if write { new_text } else { text });
    }
    let commit = if !write || (dirty.is_empty() && assigned.is_empty()) {
        None
    } else {
        git::commit_all(dir, "system: recovered changes", at)?
    };
    Ok(Scanned {
        temp,
        rolled_back,
        dirty,
        assigned,
        commit,
        files,
        texts,
    })
}

fn build_state(files: &[String], texts: &BTreeMap<String, String>) -> VaultState {
    let mut state = VaultState::default();
    for f in files {
        if let Some(text) = texts.get(f) {
            let doc = Document::parse(text);
            let Some(id) = doc
                .frontmatter()
                .and_then(|fm| fm.id().ok().flatten())
                .map(NoteId::from_ulid)
            else {
                continue;
            };
            if state.contains_id(id) {
                continue;
            }
            let meta = NoteMeta {
                id,
                kind: derive::kind_of(&doc),
                version: fsio::version_of(text.as_bytes()),
                link_names: BTreeSet::new(),
            };
            if paths::is_note(f) {
                state.put_note(f, meta);
            } else {
                state.put_trash(f, meta);
            }
        } else if paths::is_content(f) {
            state.attachments.insert(f.clone());
        }
    }
    state
}

async fn warn(
    core: &Core,
    tx: &mut ScopedTx,
    kind: &str,
    path: Option<&str>,
    detail: &str,
) -> Result<()> {
    vrepo::add_warning(
        tx,
        &IntegrityWarning {
            id: strata_common::NoteId::generate(core.ids()).as_uuid(),
            kind: kind.to_owned(),
            path: path.map(str::to_owned),
            detail: detail.to_owned(),
            created: core.now(),
        },
    )
    .await?;
    tracing::warn!(user = %core.user, kind, "vault integrity warning");
    Ok(())
}

/// Reconciles the vault with git and the index (see the module docs) and loads the writer
/// state. With `repair`, findings are reported as `index_repaired` (after a failed write)
/// rather than as out-of-band edits.
pub async fn reconcile(core: &mut Core, scope: UserScope, repair: bool) -> Result<Report> {
    run(core, scope, repair, false).await
}

/// `stratad verify`: what [`reconcile`] would find, without changing the vault, git, the
/// index or the writer state (nothing is written, no warning is recorded).
pub async fn check(core: &mut Core, scope: UserScope) -> Result<Report> {
    run(core, scope, false, true).await
}

#[allow(clippy::too_many_lines)] // one linear pass; splitting would scatter the rules
async fn run(core: &mut Core, scope: UserScope, repair: bool, dry: bool) -> Result<Report> {
    let dir = core.dir.clone();
    let inner = core.inner.clone();
    let at = core.now();
    let scanned = blocking(move || scan_and_recover(&dir, inner.ids.as_ref(), at, !dry)).await?;
    let mut state = build_state(&scanned.files, &scanned.texts);
    let mut report = Report {
        temp_files_removed: scanned.temp.clone(),
        rolled_back: scanned.rolled_back.clone(),
        recovered: scanned.dirty.clone(),
        ids_assigned: scanned.assigned.clone(),
        recovery_commit: scanned.commit.clone(),
        ..Report::default()
    };

    // Sidecars.
    let index = state.path_index();
    let mut sidecar_changes = Vec::new();
    let mut sidecars: HashMap<NoteId, NoteSidecar> = HashMap::new();
    for f in scanned
        .files
        .iter()
        .filter(|f| f.starts_with(".meta/notes/"))
    {
        let Some(id) = sidecar_id(f) else { continue };
        let Some(text) = core.read_text(f).await? else {
            continue;
        };
        let Ok(mut sc) = NoteSidecar::from_json(&text) else {
            continue;
        };
        if !state.contains_id(id) {
            sidecar_changes.push((f.clone(), None));
            report.sidecars_repaired.push(f.clone());
            continue;
        }
        let mut changed = false;
        if sc.id != id.as_ulid() {
            sc.id = id.as_ulid();
            changed = true;
        }
        if let Some((path, _)) = state.note(id)
            && let Some(note_text) = scanned.texts.get(path)
        {
            let edges =
                prepare::frontmatter_edges(&Document::parse(note_text), path, &index, &state);
            changed |= prepare::prune_sidecar(&mut sc, &edges);
        }
        if changed {
            sidecar_changes.push(Core::sidecar_change(&sc)?);
            report.sidecars_repaired.push(f.clone());
        }
        sidecars.insert(id, sc);
    }
    if !dry && !sidecar_changes.is_empty() {
        let dir = core.dir.clone();
        let changes = sidecar_changes.clone();
        blocking(move || {
            let mut paths = Vec::new();
            for (p, c) in &changes {
                match c {
                    Some(b) => fsio::atomic_write(&dir, p, b)?,
                    None => fsio::remove(&dir, p)?,
                }
                paths.push(p.clone());
            }
            git::commit_paths(&dir, &paths, "system: repair sidecars", at)
        })
        .await?;
    }

    // Derive everything (sets link names), then compare with the index.
    let mut tx = core.begin(&scope).await?;
    // Op results committed to git but not to the database: a write crashed after its commit,
    // so what differs from the index is its unfinished update (reported as a repair).
    if !dry {
        report.ops_recovered = recover_op_results(core, &mut tx).await?;
    }
    let repair = repair || !report.ops_recovered.is_empty();
    let tz = core.tz(&mut tx).await?;
    let indexed: Vec<notes::Note> = notes::list_notes(&mut tx, true).await?;
    let db: HashMap<NoteId, (String, String)> = indexed
        .iter()
        .map(|n| (n.id, (n.path.clone(), n.content_hash.clone())))
        .collect();
    let derived = derive_all(&mut state, &scanned.texts, &sidecars, tz);
    let first_build = db.is_empty();
    let mut changed: BTreeSet<NoteId> = BTreeSet::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for (path, meta) in state.notes.iter().chain(state.trash.iter()) {
        let same = db
            .get(&meta.id)
            .is_some_and(|(p, h)| p == path && *h == meta.version);
        if !same {
            changed.insert(meta.id);
            names.insert(name_key(path));
            if let Some((old, _)) = db.get(&meta.id) {
                names.insert(name_key(old));
            }
            if (dry || !first_build) && !report.ids_assigned.contains(path) {
                report.out_of_band.push(path.clone());
            }
        }
    }
    for id in sidecars.keys() {
        if report
            .sidecars_repaired
            .iter()
            .any(|p| sidecar_id(p) == Some(*id))
        {
            changed.insert(*id);
        }
    }
    let mut removed = Vec::new();
    for n in &indexed {
        if !state.contains_id(n.id) {
            removed.push(n.id);
            names.insert(name_key(&n.path));
            if !n.trashed {
                report.missing.push(n.path.clone());
            }
        }
    }
    if dry {
        tx.commit().await?;
        return Ok(report);
    }
    let mut affected = changed.clone();
    affected.extend(state.linking_to(&names));
    if repair {
        affected.extend(state.by_id.keys().copied());
        affected.extend(state.trash_by_id.keys().copied());
    }
    let batch: Vec<Derived> = affected
        .iter()
        .filter_map(|id| derived.get(id).cloned())
        .collect();
    report.reindexed = batch.len();
    let mut watched = affected.clone();
    watched.extend(removed.iter().copied());
    let before = crate::diff::Snapshot::take(&mut tx, &watched, !first_build).await?;
    indexer::write(&mut tx, &batch).await?;
    for id in &removed {
        indexer::purge(&mut tx, *id, None).await?;
    }
    let after = crate::diff::Snapshot::take(&mut tx, &watched, !first_build).await?;

    // Warnings.
    let (oob_kind, oob_detail) = if repair {
        (
            kinds::INDEX_REPAIRED,
            "the index was repaired from the file after a failed write",
        )
    } else {
        (
            kinds::OUT_OF_BAND,
            "the file was changed outside the API; the index was updated",
        )
    };
    for p in &report.temp_files_removed {
        warn(
            core,
            &mut tx,
            kinds::TEMP_FILE,
            Some(p),
            "a temporary file of an interrupted write was removed",
        )
        .await?;
    }
    for p in &report.rolled_back {
        warn(
            core,
            &mut tx,
            kinds::WRITE_ROLLED_BACK,
            Some(p),
            "a write that stopped before its commit was undone",
        )
        .await?;
    }
    if !report.recovered.is_empty() {
        let detail = format!(
            "{} uncommitted file(s) were committed as system: recovered changes",
            report.recovered.len()
        );
        warn(core, &mut tx, kinds::UNCOMMITTED, None, &detail).await?;
    }
    for p in &report.ids_assigned {
        warn(
            core,
            &mut tx,
            kinds::ID_ASSIGNED,
            Some(p),
            "the note was given an id or task block ids",
        )
        .await?;
    }
    for p in &report.sidecars_repaired {
        let orphan = sidecar_id(p).is_some_and(|id| !state.contains_id(id));
        if orphan {
            warn(
                core,
                &mut tx,
                kinds::ORPHAN_SIDECAR,
                Some(p),
                "a sidecar without a note was removed",
            )
            .await?;
        } else {
            warn(
                core,
                &mut tx,
                kinds::SIDECAR,
                Some(p),
                "sidecar provenance was repaired from the frontmatter",
            )
            .await?;
        }
    }
    for p in &report.out_of_band {
        warn(core, &mut tx, oob_kind, Some(p), oob_detail).await?;
    }
    for p in &report.missing {
        warn(
            core,
            &mut tx,
            kinds::MISSING_FILE,
            Some(p),
            "the note's file is missing; it was removed from the index",
        )
        .await?;
    }
    let old_paths: HashMap<NoteId, String> = indexed
        .iter()
        .filter(|n| !n.trashed)
        .map(|n| (n.id, n.path.clone()))
        .collect();
    core.state = Some(state);
    let mut notice = crate::events::Committed::default();
    if !first_build {
        notice = core.log_derived_diff(&mut tx, &before, &after).await?;
        notice.notes = reconciled_note_events(core, &changed, &removed, &old_paths, &indexed)?;
        let synced = Synced {
            changed: changed.into_iter().collect(),
            removed,
        };
        core.log_changes(&mut tx, &synced).await?;
    }
    for op in &report.ops_recovered {
        let detail = format!("the result of op {op} was recovered from its commit");
        warn(core, &mut tx, kinds::OP_RESULT_RECOVERED, None, &detail).await?;
    }
    let warnings = vrepo::list_warnings(&mut tx, i64::MAX).await?.len();
    tx.commit().await?;
    notice.integrity_warnings = u32::try_from(
        report.temp_files_removed.len()
            + report.rolled_back.len()
            + report.ops_recovered.len()
            + usize::from(!report.recovered.is_empty())
            + report.ids_assigned.len()
            + report.sidecars_repaired.len()
            + report.out_of_band.len()
            + report.missing.len(),
    )
    .unwrap_or(u32::MAX)
    .min(u32::try_from(warnings).unwrap_or(u32::MAX));
    "reconcile".clone_into(&mut notice.op);
    notice.user = Some(core.user);
    core.inner.notify(core.user, &notice);
    Ok(report)
}

/// Re-inserts the op results recorded in the commits after `sync_epochs.vault_head` (their
/// database transaction never committed) and records `HEAD` there. Returns the recovered op
/// IDs, oldest first.
async fn recover_op_results(core: &Core, tx: &mut ScopedTx) -> Result<Vec<String>> {
    use strata_index::repo::sync as log;
    let recorded = log::vault_head(tx).await?;
    let dir = core.dir.clone();
    let stop = recorded.clone();
    let (head, commits) = blocking(move || {
        let head = git::head(&dir)?.map(|c| c.id);
        if head.is_none() || head == stop {
            return Ok((head, Vec::new()));
        }
        Ok((
            head,
            git::commits_since(&dir, stop.as_deref(), RECOVERY_WALK_LIMIT)?,
        ))
    })
    .await?;
    let Some(head) = head else {
        return Ok(Vec::new());
    };
    let mut recovered = Vec::new();
    for (_, message) in &commits {
        let Some(r) = crate::receipt::parse_trailers(message) else {
            continue;
        };
        if log::idempotency_get(tx, r.op_id).await?.is_none() {
            log::idempotency_put(tx, r.op_id, r.device, &r.result, core.now()).await?;
            recovered.push(r.op_id.to_string());
        }
    }
    if recorded.as_deref() != Some(head.as_str()) {
        log::set_vault_head(tx, &head, core.now()).await?;
    }
    Ok(recovered)
}

/// Note events for what reconciliation re-derived or purged.
fn reconciled_note_events(
    core: &Core,
    changed: &BTreeSet<NoteId>,
    removed: &[NoteId],
    old_paths: &HashMap<NoteId, String>,
    indexed: &[notes::Note],
) -> Result<Vec<crate::events::NoteEvent>> {
    use crate::events::{NoteChange, NoteEvent};
    let state = core.state()?;
    let mut out = Vec::new();
    let mut ids: BTreeSet<NoteId> = changed.clone();
    ids.extend(removed.iter().copied());
    for id in ids {
        let before = old_paths.get(&id);
        let event = match (before, state.note(id)) {
            (Some(old), Some((path, m))) => NoteEvent {
                id,
                kind: m.kind,
                change: if old == path {
                    NoteChange::Updated
                } else {
                    NoteChange::Moved
                },
                path: path.to_owned(),
                old_path: (old != path).then(|| old.clone()),
                version: Some(m.version.clone()),
            },
            (None, Some((path, m))) => NoteEvent {
                id,
                kind: m.kind,
                change: NoteChange::Created,
                path: path.to_owned(),
                old_path: None,
                version: Some(m.version.clone()),
            },
            (Some(old), None) => NoteEvent {
                id,
                kind: indexed
                    .iter()
                    .find(|n| n.id == id)
                    .and_then(|n| n.kind.as_str().parse().ok())
                    .unwrap_or(domain::NoteKind::Note),
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

fn derive_all(
    state: &mut VaultState,
    texts: &BTreeMap<String, String>,
    sidecars: &HashMap<NoteId, NoteSidecar>,
    tz: chrono_tz::Tz,
) -> HashMap<NoteId, Derived> {
    let index = state.path_index();
    let mut out = HashMap::new();
    let mut names = Vec::new();
    {
        let ctx = Context {
            index: &index,
            state: &*state,
            tz,
        };
        for (path, meta) in state.notes.iter().chain(state.trash.iter()) {
            let Some(text) = texts.get(path) else {
                continue;
            };
            let trashed = !paths::is_note(path);
            if let Some(d) = derive::derive(path, text, sidecars.get(&meta.id), &ctx, trashed) {
                names.push((path.clone(), d.link_names.clone()));
                out.insert(meta.id, d);
            }
        }
    }
    for (path, n) in names {
        if let Some(m) = state.notes.get_mut(&path) {
            m.link_names = n;
        }
    }
    out
}

/// Deletes every derived row of the user and derives the whole vault again (`stratad
/// reindex`), reloading the cluster rows from `.meta/clusters.json` (change-log rows only for
/// what differs from before) and the disambiguation hints from the entity sidecars (see
/// [`rebuild_hints`]). The vault is reconciled first.
pub async fn reindex(core: &mut Core, scope: UserScope) -> Result<usize> {
    let dir = core.dir.clone();
    let inner = core.inner.clone();
    let at = core.now();
    let scanned = blocking(move || scan_and_recover(&dir, inner.ids.as_ref(), at, true)).await?;
    let mut state = build_state(&scanned.files, &scanned.texts);
    let mut sidecars = HashMap::new();
    for f in scanned
        .files
        .iter()
        .filter(|f| f.starts_with(".meta/notes/"))
    {
        if let Some(id) = sidecar_id(f)
            && let Some(text) = core.read_text(f).await?
            && let Ok(sc) = NoteSidecar::from_json(&text)
        {
            sidecars.insert(id, sc);
        }
    }
    // The cluster rows come back from `.meta/clusters.json` (missing or unreadable: none).
    let cluster_file = core
        .read_text(vault_format::clusters::CLUSTERS_PATH)
        .await?
        .and_then(|t| vault_format::clusters::Clusters::from_json(&t).ok());
    let mut tx = core.begin(&scope).await?;
    let tz = core.tz(&mut tx).await?;
    let derived = derive_all(&mut state, &scanned.texts, &sidecars, tz);
    let clusters_before = crate::clusters::snapshot(&mut tx).await?;
    // Deleting the entity rows cascades to their hints; they come back from the sidecars.
    let hints_before = erepo::all_hints(&mut tx).await?;
    vrepo::clear_derived(&mut tx).await?;
    let mut batch: Vec<(NoteId, Derived)> = derived.into_iter().collect();
    batch.sort_by_key(|(id, _)| *id);
    let batch: Vec<Derived> = batch.into_iter().map(|(_, d)| d).collect();
    indexer::write(&mut tx, &batch).await?;
    rebuild_hints(&mut tx, &sidecars, &hints_before).await?;
    let clusters_now = cluster_file
        .as_ref()
        .map(crate::clusters::ClusterRows::of_file)
        .unwrap_or_default();
    crate::clusters::replace(&mut tx, &clusters_before, &clusters_now, at).await?;
    tx.commit().await?;
    core.state = Some(state);
    Ok(batch.len())
}

/// Rebuilds `disambiguation_hints` (§9.8) from the `hints` mirrored in the sidecars of the
/// entities the index now holds: the vault is the source of truth, the table the cache the
/// resolution prompts read. A hint keeps its ID, text and time; the correction decision it
/// came from (app state, not in the sidecar) is carried over from the row it had before, by
/// ID. Unreadable entries and repeated IDs are skipped.
pub(crate) async fn rebuild_hints(
    tx: &mut ScopedTx,
    sidecars: &HashMap<NoteId, NoteSidecar>,
    before: &[erepo::Hint],
) -> Result<usize> {
    let decisions: HashMap<HintId, _> = before
        .iter()
        .map(|h| (h.id, h.source_decision_id))
        .collect();
    let mut seen = BTreeSet::new();
    let mut written = 0;
    for entity in erepo::entity_ids(tx).await? {
        let Some(list) = sidecars
            .get(&entity)
            .and_then(|sc| sc.extra.get(crate::ops::ai_apply::HINTS_KEY))
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for value in list {
            let Ok(h) = serde_json::from_value::<crate::ops::ai_apply::SidecarHint>(value.clone())
            else {
                continue;
            };
            let Ok(id) = h.id.parse::<HintId>() else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            erepo::add_hint(
                tx,
                &erepo::Hint {
                    id,
                    entity_id: entity,
                    hint: h.text,
                    source_decision_id: decisions.get(&id).copied().flatten(),
                    created: h.at,
                },
            )
            .await?;
            written += 1;
        }
    }
    Ok(written)
}
