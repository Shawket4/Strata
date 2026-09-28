//! `POST /sync/push` (PLAN §7.5 Sync, §12.4, D19).
//!
//! Ops apply **in order**, each through the vault writer actor as its own write (one git
//! commit per op, like the REST endpoint for the same mutation: the device's outbox is a list
//! of user intents and each stays individually revertible). A user's pushes are serialised
//! by [`SyncState::lock`], so the idempotency check and the store of one op never race with
//! another push of the same op.
//!
//! **Idempotency, exactly once.** Every result is encoded once (`MessagePack`, named maps) and
//! stored under the op's `op_id` in `idempotency`; a replayed op (same `op_id`, in the same or
//! a later push) is not applied again and its stored bytes are copied into the response
//! unchanged, so replays are byte-identical. The result of an op that writes is stored **by
//! that write**: each op runs in a [`strata_vault::OpReceipt`] scope, armed with a hook that
//! builds the result from the vault as the write left it, right before the vault call that
//! completes the op; the writer runs it inside the write's own scoped transaction (with the
//! index update and change log) and records it as trailers of the write's git commit.
//! Database-only ops (`relink.request`, `device.settings`) store it in their own transaction.
//! A push first lets the vault recover ([`strata_vault::VaultService::recover`]): a write
//! interrupted before its git commit is rolled back (the replay applies it once), one
//! interrupted after it has its result re-inserted from the commit (the replay answers it).
//! Server failures (`5xx`) abort the push without storing anything for the failing op.
//!
//! **Updates (D19).** `note.update` with the current version is written as is. With a stale
//! base, the base content is looked up in the note's git history (or among contents this push
//! already submitted for the note), and `sync_model::decide_update` decides: already applied,
//! a clean 3-way merge (written, answered `applied{merged: true}`), or a conflict — the server
//! keeps its version, saves the device's content as a conflict copy next to the note (a new
//! note whose ID is the `op_id`, so a retry finds it), records a `conflict` suggestion on the
//! note (synced to every device, announced on `/events`) and answers `conflict{conflict_copy}`.
//! Entity/document/place patches are field-level and re-apply onto the current version when
//! stale (`merged: true`); task edits are line-level and answer `conflict{server_kept}` when
//! the line changed, unless the edit is already in effect.
//!
//! **Isolation.** Every lookup runs in the caller's scope; another user's IDs behave exactly
//! like missing ones (`rejected{404 not_found}`), and nothing is applied.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dedupe::DuplicateCandidate;
use domain::{DedupeKind, MatchLevel, NoteKind};
use serde::{Deserialize, Serialize};
use strata_common::{
    Clock, DeviceId, IdGenerator, JobId, NoteId, OpId, ReplyId, SuggestionId, UserId,
};
use strata_index::repo::{devices, jobs, sync as log};
use strata_index::types::ChangeOp;
use strata_index::{AppDb, UserScope};
use strata_vault::ops::entities::{EntityPatch as VPatch, NewCustodyEvent, NewEntity};
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::tasks::NewTask;
use strata_vault::{
    AfterWrite, Candidate, MatchLevel as VMatch, OpReceipt, ResultHook, VaultError, VaultService,
};
use sync_model::ops::{self as sm_ops, Op};
use sync_model::{
    ConflictResolution, OpResult, Problem as OpProblem, SyncOp, UpdateDecision, Version,
    decide_update,
};
use vault_format::{Document, RelationKey};

use crate::events::{Event, EventBus};
use crate::sync::SyncState;
use crate::vault::problem;
use crate::wire::{Problem, ProblemType};

/// Everything applying an op needs.
pub struct PushContext<'a> {
    /// The vault store.
    pub vault: &'a VaultService,
    /// The `strata_app` handle.
    pub db: &'a AppDb,
    /// The caller's scope.
    pub scope: &'a UserScope,
    /// The pushing device.
    pub device: DeviceId,
    /// Time.
    pub clock: &'a Arc<dyn Clock>,
    /// IDs (jobs).
    pub ids: &'a Arc<dyn IdGenerator>,
    /// Events.
    pub bus: &'a Arc<EventBus>,
    /// Revisions searched for a stale update's base.
    pub merge_history: usize,
}

impl std::fmt::Debug for PushContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PushContext")
            .field("device", &self.device)
            .field("merge_history", &self.merge_history)
            .finish_non_exhaustive()
    }
}

/// `MessagePack` of a `conflict` suggestion's payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictPayload {
    /// The op that conflicted.
    pub op_id: String,
    /// The conflict copy note.
    pub copy_id: String,
    /// Its path.
    pub copy_path: String,
    /// The version the device edited.
    pub base_version: String,
    /// The server's version that was kept.
    pub server_version: String,
    /// Number of conflicting hunks.
    pub hunks: u32,
}

fn op_problem(p: &Problem) -> OpProblem {
    OpProblem {
        problem_type: p.problem_type.clone(),
        title: p.title.clone(),
        status: p.status,
        detail: p.detail.clone(),
    }
}

fn rejected(p: &Problem) -> OpResult {
    OpResult::Rejected {
        problem: op_problem(p),
    }
}

fn rejected_kind(kind: ProblemType, detail: &str) -> OpResult {
    rejected(&Problem::new(kind).with_detail(detail))
}

fn not_found() -> OpResult {
    rejected(&Problem::new(ProblemType::NotFound))
}

fn applied(version: Option<String>, merged: bool) -> OpResult {
    OpResult::Applied {
        new_version: version.and_then(|v| v.parse().ok()),
        merged,
    }
}

fn server_kept(server_version: Option<String>, reason: &str) -> OpResult {
    OpResult::Conflict {
        server_version: server_version.and_then(|v| v.parse().ok()),
        resolution: ConflictResolution::ServerKept {
            reason: reason.to_owned(),
        },
    }
}

fn duplicate(candidates: &[Candidate]) -> OpResult {
    OpResult::Duplicate {
        candidates: candidates
            .iter()
            .map(|c| DuplicateCandidate {
                id: c.item.clone(),
                kind: c.kind.parse().unwrap_or(DedupeKind::Note),
                title: c.title.clone(),
                snippet: c.snippet.clone(),
                level: match c.level {
                    VMatch::Exact => MatchLevel::Exact,
                    VMatch::Near if c.semantic => MatchLevel::Semantic,
                    VMatch::Near => MatchLevel::Near,
                },
                #[allow(clippy::cast_possible_truncation)] // scores are in [0, 1]
                score: c.score as f32,
            })
            .collect(),
    }
}

/// The result for a vault error that has no op-specific meaning; a server failure (`5xx`)
/// is an `Err` (the push fails and nothing is stored for the op).
fn from_vault(e: &VaultError) -> Result<OpResult, Problem> {
    Ok(match e {
        VaultError::Duplicate(c) => duplicate(c),
        VaultError::VersionConflict { current } => {
            server_kept(Some(current.clone()), "the entity changed on the server")
        }
        other => {
            let p = problem(other);
            if p.status >= 500 {
                return Err(p);
            }
            rejected(&p)
        }
    })
}

/// A result hook: `f` builds the result from the vault as the write left it.
fn hook(f: impl FnOnce(&AfterWrite<'_>) -> OpResult + Send + 'static) -> ResultHook {
    Box::new(move |w| crate::wire::encode(&f(w)).map_err(|e| e.to_string()))
}

/// The hook of an op answered `applied` with the live version of `id` after the write.
fn applied_note(id: NoteId, merged: bool) -> ResultHook {
    hook(move |w| applied(w.note_version(id), merged))
}

/// The hook of an op answered `applied` without a version.
fn applied_plain() -> ResultHook {
    hook(|_| applied(None, false))
}

/// Contents this push submitted, by note: (version of the submitted text, text). A later op
/// of the same push whose base is one of them merges against it (the server stamps `id`,
/// `created` and `updated`, so its version differs from the device's).
type Submitted = HashMap<NoteId, Vec<(String, String)>>;

/// Applies `ops` in order (see the module docs) and returns the encoded
/// [`sync_model::PushResponse`].
pub async fn push(
    state: &SyncState,
    ctx: &PushContext<'_>,
    ops: Vec<SyncOp>,
) -> Result<Vec<u8>, Problem> {
    if ops.len() > state.config.max_ops {
        return Err(Problem::new(ProblemType::InvalidBody)
            .with_detail(format!("at most {} ops per push", state.config.max_ops)));
    }
    let lock = state.lock(ctx.scope.user_id());
    let _guard = lock.lock().await;
    // Roll back or recover an interrupted write before looking up any op.
    match ctx.vault.recover(ctx.scope).await {
        Ok(()) | Err(VaultError::NotFound) => {}
        Err(e) => return Err(problem(&e)),
    }
    let mut submitted = Submitted::new();
    let mut results: Vec<(OpId, Vec<u8>)> = Vec::with_capacity(ops.len());
    for op in ops {
        let op_id = OpId::from_ulid(op.op_id);
        let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
        let stored = log::idempotency_get(&mut tx, op_id)
            .await
            .map_err(|e| index(&e))?;
        tx.commit().await.map_err(|e| index(&e))?;
        let bytes = if let Some(rec) = stored {
            rec.result
        } else {
            let rc = OpReceipt::new(op_id, ctx.device);
            let outcome = rc.scope(apply(ctx, &rc, &op, &mut submitted)).await;
            // A write that stored the result committed it: those bytes are the answer.
            if let Some(bytes) = rc.settled() {
                bytes
            } else {
                let result = outcome?;
                let bytes = crate::wire::encode(&result).map_err(|e| Problem::internal(&e))?;
                let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
                let rec = log::idempotency_put(&mut tx, op_id, ctx.device, &bytes, ctx.clock.now())
                    .await
                    .map_err(|e| index(&e))?;
                tx.commit().await.map_err(|e| index(&e))?;
                rec.result
            }
        };
        results.push((op_id, bytes));
    }
    Ok(encode_response(&results))
}

fn index(e: &strata_index::IndexError) -> Problem {
    Problem::internal(e)
}

/// `{results: [{op_id, result}]}` with each stored result copied verbatim.
fn encode_response(results: &[(OpId, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    // Writing into a Vec cannot fail.
    let _ = rmp::encode::write_map_len(&mut out, 1);
    let _ = rmp::encode::write_str(&mut out, "results");
    let _ = rmp::encode::write_array_len(&mut out, u32::try_from(results.len()).unwrap_or(0));
    for (id, bytes) in results {
        let _ = rmp::encode::write_map_len(&mut out, 2);
        let _ = rmp::encode::write_str(&mut out, "op_id");
        let _ = rmp::encode::write_str(&mut out, &id.to_string());
        let _ = rmp::encode::write_str(&mut out, "result");
        out.extend_from_slice(bytes);
    }
    out
}

/// Applies one op (an `Err` only for server failures). The vault call that completes the op
/// runs armed with its result hook (see the module docs).
#[allow(clippy::too_many_lines)] // one arm per op kind
async fn apply(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    op: &SyncOp,
    submitted: &mut Submitted,
) -> Result<OpResult, Problem> {
    if let Err(e) = op.validate() {
        return Ok(rejected_kind(ProblemType::InvalidBody, &e.to_string()));
    }
    let base = op.base_version.as_ref().map(|v| v.as_str().to_owned());
    let (v, s) = (ctx.vault, ctx.scope);
    Ok(match &op.op {
        Op::NoteCreate(p) => note_create(ctx, rc, p, submitted).await?,
        Op::NoteUpdate(p) => {
            note_update(
                ctx,
                rc,
                op.op_id,
                NoteId::from_ulid(p.id),
                base.unwrap_or_default(),
                &p.content,
                submitted,
            )
            .await?
        }
        Op::NoteMove(p) => {
            let id = NoteId::from_ulid(p.id);
            let moved = rc
                .armed(
                    applied_note(id, false),
                    v.move_note(s, id, p.new_path.clone(), None),
                )
                .await;
            match moved {
                Ok(view) => applied(Some(view.version), false),
                Err(VaultError::PathTaken) => server_kept(
                    current_version(ctx, id).await,
                    "a note already exists at the target path",
                ),
                Err(VaultError::NotFound) => deleted_or_missing(ctx, id).await,
                Err(e) => from_vault(&e)?,
            }
        }
        Op::NoteDelete(p) => {
            let id = NoteId::from_ulid(p.id);
            match v.note(s, id).await {
                Ok(n) if n.trashed => applied(None, false),
                Ok(n) if Some(&n.version) != base.as_ref() => server_kept(
                    Some(n.version),
                    "the note changed on the server since this version; it was not deleted",
                ),
                Ok(_) => match rc.armed(applied_plain(), v.delete_note(s, id)).await {
                    Ok(_) => applied(None, false),
                    Err(e) => from_vault(&e)?,
                },
                Err(VaultError::NotFound) => not_found(),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::Capture(p) => {
            let id = NoteId::from_ulid(p.id);
            match rc
                .armed(
                    applied_note(id, false),
                    v.capture_as(s, p.text.clone(), id, p.created),
                )
                .await
            {
                Ok(c) => applied(Some(c.note.version), false),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::RelationAdd(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match rc
                .armed(
                    applied_note(src, false),
                    v.add_relation(s, src, dst, p.relation),
                )
                .await
            {
                Ok(_) => applied(current_version(ctx, src).await, false),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::RelationRemove(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match rc
                .armed(
                    applied_note(src, false),
                    v.remove_relation(s, src, dst, p.relation),
                )
                .await
            {
                Ok(_) => applied(current_version(ctx, src).await, false),
                // Both notes exist but the edge does not: already removed elsewhere.
                Err(VaultError::NotFound) if live(ctx, src).await && live(ctx, dst).await => {
                    applied(current_version(ctx, src).await, false)
                }
                Err(e) => from_vault(&e)?,
            }
        }
        Op::RelationRetype(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match rc
                .armed(
                    applied_note(src, false),
                    v.retype_relation(s, src, dst, p.relation, p.new_type),
                )
                .await
            {
                Ok(()) => applied(current_version(ctx, src).await, false),
                Err(VaultError::NotFound) if live(ctx, src).await && live(ctx, dst).await => {
                    server_kept(
                        current_version(ctx, src).await,
                        "the relation no longer exists",
                    )
                }
                Err(e) => from_vault(&e)?,
            }
        }
        Op::SuggestionAccept(p) => {
            if p.edits
                .as_ref()
                .is_some_and(|e| *e != sm_ops::SuggestionEdits::default())
            {
                rejected_kind(
                    ProblemType::InvalidBody,
                    "edits are not supported for this suggestion kind",
                )
            } else {
                decide(ctx, rc, p.id, true).await?
            }
        }
        Op::SuggestionReject(p) => decide(ctx, rc, p.id, false).await?,
        Op::SuggestionReply(p) => {
            match rc
                .armed(
                    applied_plain(),
                    v.reply_suggestion_as(
                        s,
                        SuggestionId::from_ulid(p.id),
                        ReplyId::from_ulid(p.reply_id),
                        p.text.clone(),
                    ),
                )
                .await
            {
                Ok(_) => applied(None, false),
                Err(VaultError::Invalid(r)) if r.contains("already decided") => {
                    server_kept(None, "the suggestion was already decided")
                }
                Err(e) => from_vault(&e)?,
            }
        }
        Op::EntityCreate(p) => {
            let req = NewEntity {
                kind: p.kind,
                name: p.name.clone(),
                aliases: p.aliases.clone(),
                tags: Vec::new(),
                fields: p.fields.clone(),
                parent: None,
                id: Some(NoteId::from_ulid(p.id)),
                force: p.force,
            };
            create_entity(ctx, rc, req, Vec::new()).await?
        }
        Op::EntityPatch(p) => patch(ctx, rc, p, base, None).await?,
        Op::DocumentPatch(p) => patch(ctx, rc, p, base, Some(NoteKind::Document)).await?,
        Op::PlacePatch(p) => patch(ctx, rc, p, base, Some(NoteKind::Place)).await?,
        Op::EntityMerge(p) => {
            let into = NoteId::from_ulid(p.into_id);
            match rc
                .armed(
                    applied_note(into, false),
                    v.merge_entities(s, NoteId::from_ulid(p.id), into),
                )
                .await
            {
                Ok(view) => applied(Some(view.version), false),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::DocumentCreate(p) => {
            let mut fields = BTreeMap::new();
            if let Some(t) = &p.doc_type {
                fields.insert("doc-type".to_owned(), t.clone());
            }
            if let Some(c) = p.copy {
                fields.insert("copy".to_owned(), c.as_str().to_owned());
            }
            if let Some(e) = p.expires {
                fields.insert("expires".to_owned(), e.format("%Y-%m-%d").to_string());
            }
            let (Some(copy_of), Some(companies), Some(people)) = (
                relation("copy-of"),
                relation("companies"),
                relation("people"),
            ) else {
                return Err(Problem::new(ProblemType::Internal));
            };
            let mut links: Vec<(RelationKey, NoteId)> = Vec::new();
            if let Some(c) = p.copy_of {
                links.push((copy_of, NoteId::from_ulid(c)));
            }
            links.extend(
                p.companies
                    .iter()
                    .map(|c| (companies, NoteId::from_ulid(*c))),
            );
            links.extend(p.people.iter().map(|c| (people, NoteId::from_ulid(*c))));
            let req = NewEntity {
                kind: NoteKind::Document,
                name: p.name.clone(),
                aliases: p.aliases.clone(),
                tags: Vec::new(),
                fields,
                parent: None,
                id: Some(NoteId::from_ulid(p.id)),
                force: p.force,
            };
            create_entity(ctx, rc, req, links).await?
        }
        Op::DocumentCustody(p) => {
            let doc = NoteId::from_ulid(p.document_id);
            let event = NewCustodyEvent {
                kind: p.event,
                date: p.at,
                place: p.place_id.map(NoteId::from_ulid),
                person: p.person_id.map(NoteId::from_ulid),
                counterparty: p.counterparty_id.map(NoteId::from_ulid),
                source: None,
            };
            match rc
                .armed(applied_note(doc, false), v.add_custody_event(s, doc, event))
                .await
            {
                Ok(view) => applied(Some(view.version), false),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::PlaceCreate(p) => {
            let mut fields = BTreeMap::new();
            if let Some(a) = &p.address {
                fields.insert("address".to_owned(), a.clone());
            }
            let req = NewEntity {
                kind: NoteKind::Place,
                name: p.name.clone(),
                aliases: p.aliases.clone(),
                tags: Vec::new(),
                fields,
                parent: p.parent_id.map(NoteId::from_ulid),
                id: Some(NoteId::from_ulid(p.id)),
                force: p.force,
            };
            create_entity(ctx, rc, req, Vec::new()).await?
        }
        Op::TaskCreate(p) => {
            let req = NewTask {
                text: p.text.clone(),
                due: p.due,
                scheduled: p.scheduled,
                start: p.start,
                recurrence: p.recurrence.clone(),
                reminders: p.reminders.clone(),
                priority: p.priority,
                note: p.note_id.map(NoteId::from_ulid),
                id: Some(p.id.clone()),
                force: p.force,
            };
            let block = p.id.clone();
            let task_hook = hook(move |w| applied(w.task_line_version(&block), false));
            match rc.armed(task_hook, v.create_task(s, req)).await {
                Ok(id) => applied(task_version(ctx, &id).await, false),
                Err(e) => from_vault(&e)?,
            }
        }
        Op::TaskUpdate(_)
        | Op::TaskComplete(_)
        | Op::TaskCancel(_)
        | Op::TaskReopen(_)
        | Op::TaskDelete(_) => task_edit(ctx, rc, &op.op, base).await?,
        Op::RelinkRequest(p) => {
            let id = NoteId::from_ulid(p.id);
            if live(ctx, id).await {
                relink(ctx, rc, id).await?;
                applied(None, false)
            } else {
                not_found()
            }
        }
        Op::DeviceSettings(p) => device_settings(ctx, rc, p).await?,
    })
}

fn relation(key: &str) -> Option<RelationKey> {
    key.parse().ok()
}

async fn current_version(ctx: &PushContext<'_>, id: NoteId) -> Option<String> {
    match ctx.vault.note(ctx.scope, id).await {
        Ok(n) if !n.trashed => Some(n.version),
        _ => None,
    }
}

async fn live(ctx: &PushContext<'_>, id: NoteId) -> bool {
    matches!(ctx.vault.note(ctx.scope, id).await, Ok(n) if !n.trashed)
}

/// A missing note is `404`; a trashed one is a `server_kept` conflict.
async fn deleted_or_missing(ctx: &PushContext<'_>, id: NoteId) -> OpResult {
    match ctx.vault.note(ctx.scope, id).await {
        Ok(n) if n.trashed => server_kept(None, "the note was deleted on the server"),
        Ok(n) => server_kept(Some(n.version), "the note changed on the server"),
        Err(_) => not_found(),
    }
}

async fn task_version(ctx: &PushContext<'_>, id: &str) -> Option<String> {
    ctx.vault
        .task_line(ctx.scope, id.to_owned())
        .await
        .ok()
        .map(|(_, v, _)| v)
}

async fn note_create(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    p: &sm_ops::NoteCreate,
    submitted: &mut Submitted,
) -> Result<OpResult, Problem> {
    let id = NoteId::from_ulid(p.id);
    let mut path = p.path.clone();
    // A path taken meanwhile gets a free name next to it: the device's note is never lost.
    for n in 2..=50 {
        let req = CreateNote {
            path: path.clone(),
            content: p.content.clone(),
            id: Some(id),
            force: p.force,
        };
        match rc
            .armed(
                applied_note(id, false),
                ctx.vault.create_note(ctx.scope, req),
            )
            .await
        {
            Ok(view) => {
                submitted.entry(id).or_default().push((
                    Version::of_text(&p.content).as_str().to_owned(),
                    p.content.clone(),
                ));
                return Ok(applied(Some(view.version), false));
            }
            Err(VaultError::PathTaken) => {
                let stem = p.path.strip_suffix(".md").unwrap_or(&p.path);
                path = format!("{stem} {n}.md");
            }
            Err(e) => return from_vault(&e),
        }
    }
    Ok(rejected(&Problem::new(ProblemType::PathTaken)))
}

/// `note.update` (D19; see the module docs).
#[allow(clippy::too_many_lines)] // the D19 decision, one arm per outcome
async fn note_update(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    op_id: ulid::Ulid,
    id: NoteId,
    base: String,
    content: &str,
    submitted: &mut Submitted,
) -> Result<OpResult, Problem> {
    let (v, s) = (ctx.vault, ctx.scope);
    for _attempt in 0..5 {
        let current = match v.note(s, id).await {
            Ok(n) if n.trashed => {
                return Ok(server_kept(None, "the note was deleted on the server"));
            }
            Ok(n) => n,
            Err(VaultError::NotFound) => return Ok(not_found()),
            Err(e) => return from_vault(&e),
        };
        let record = |submitted: &mut Submitted| {
            submitted.entry(id).or_default().push((
                Version::of_text(content).as_str().to_owned(),
                content.to_owned(),
            ));
        };
        let write = |text: String, merged: bool| {
            let version = current.version.clone();
            rc.armed(
                applied_note(id, merged),
                v.update_note(s, id, text, version),
            )
        };
        if current.version == base {
            match write(content.to_owned(), false).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), false));
                }
                Err(VaultError::VersionConflict { .. }) => continue,
                Err(e) => return from_vault(&e),
            }
        }
        let Ok(base_version) = base.parse::<Version>() else {
            return Ok(rejected_kind(
                ProblemType::InvalidBody,
                "invalid base_version",
            ));
        };
        let from_push = submitted
            .get(&id)
            .and_then(|list| list.iter().rev().find(|(v, _)| *v == base))
            .map(|(_, t)| t.clone());
        let base_text = match from_push {
            Some(t) => Some(t),
            None => v
                .content_at_version(s, id, &base, ctx.merge_history)
                .await
                .map_err(|e| problem(&e))?,
        };
        match decide_update(
            &base_version,
            base_text.as_deref(),
            &current.content,
            content,
        ) {
            UpdateDecision::AlreadyApplied => {
                return Ok(applied(Some(current.version), false));
            }
            UpdateDecision::FastForward => match write(content.to_owned(), false).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), false));
                }
                Err(VaultError::VersionConflict { .. }) => {}
                Err(e) => return from_vault(&e),
            },
            UpdateDecision::Merged(text) => match write(text, true).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), true));
                }
                Err(VaultError::VersionConflict { .. }) => {}
                Err(e) => return from_vault(&e),
            },
            UpdateDecision::Conflict(c) => {
                return conflict_copy(
                    ctx,
                    rc,
                    op_id,
                    id,
                    &current.path,
                    &base,
                    &current.version,
                    content,
                    u32::try_from(c.hunks.len()).unwrap_or(u32::MAX),
                )
                .await;
            }
        }
    }
    Ok(server_kept(
        current_version(ctx, id).await,
        "the note kept changing on the server; retry",
    ))
}

/// The path of a conflict copy of `path` made at `at`.
pub fn conflict_path(path: &str, at: DateTime<Utc>, n: u32) -> String {
    let stem = path.strip_suffix(".md").unwrap_or(path);
    let stamp = at.format("%Y-%m-%d %H%M%S");
    if n <= 1 {
        format!("{stem} (conflict {stamp}).md")
    } else {
        format!("{stem} (conflict {stamp} {n}).md")
    }
}

/// Saves the device's content as a conflict copy, then records the `conflict` suggestion:
/// the suggestion's transaction stores the op result (the copy alone is not the whole op; a
/// replay after a crash in between finds the copy and records the suggestion).
#[allow(clippy::too_many_arguments)] // the parts of one conflict record
async fn conflict_copy(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    op_id: ulid::Ulid,
    id: NoteId,
    path: &str,
    base: &str,
    server_version: &str,
    content: &str,
    hunks: u32,
) -> Result<OpResult, Problem> {
    let copy_id = NoteId::from_ulid(op_id);
    let now = ctx.clock.now();
    let mut made = None;
    for n in 1..=20 {
        let copy_path = conflict_path(path, now, n);
        let req = CreateNote {
            path: copy_path.clone(),
            content: content.to_owned(),
            id: Some(copy_id),
            // The copy resembles the note by design: keep both.
            force: true,
        };
        match ctx.vault.create_note(ctx.scope, req).await {
            Ok(view) => {
                made = Some(view);
                break;
            }
            Err(VaultError::PathTaken) => {}
            // A retry after a crash: the copy exists already.
            Err(VaultError::Invalid(_)) => {
                if let Ok(view) = ctx.vault.note(ctx.scope, copy_id).await {
                    made = Some(view);
                }
                break;
            }
            Err(e) => return from_vault(&e),
        }
    }
    let Some(copy) = made else {
        return Ok(rejected(&Problem::new(ProblemType::PathTaken)));
    };
    let payload = crate::wire::encode(&ConflictPayload {
        op_id: op_id.to_string(),
        copy_id: copy_id.to_string(),
        copy_path: copy.path.clone(),
        base_version: base.to_owned(),
        server_version: server_version.to_owned(),
        hunks,
    })
    .map_err(|e| Problem::internal(&e))?;
    let result = OpResult::Conflict {
        server_version: server_version.parse().ok(),
        resolution: ConflictResolution::ConflictCopy {
            note_id: copy_id.as_ulid(),
            path: copy.path,
            version: copy
                .version
                .parse()
                .map_err(|_| Problem::new(ProblemType::Internal))?,
        },
    };
    let answer = result.clone();
    rc.armed(
        hook(move |_| answer),
        ctx.vault.create_suggestion(
            ctx.scope,
            SuggestionId::from_ulid(op_id),
            Some(id),
            "conflict",
            &payload,
        ),
    )
    .await
    .map_err(|e| problem(&e))?;
    Ok(result)
}

async fn decide(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    id: ulid::Ulid,
    accept: bool,
) -> Result<OpResult, Problem> {
    match rc
        .armed(
            applied_plain(),
            ctx.vault
                .decide_suggestion(ctx.scope, SuggestionId::from_ulid(id), accept),
        )
        .await
    {
        Ok(_) => Ok(applied(None, false)),
        Err(VaultError::Invalid(r)) if r.contains("already decided") => {
            Ok(server_kept(None, "the suggestion was already decided"))
        }
        Err(e) => from_vault(&e),
    }
}

/// Entity, document and place creates; a document's links are written in the same commit.
async fn create_entity(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    req: NewEntity,
    links: Vec<(RelationKey, NoteId)>,
) -> Result<OpResult, Problem> {
    let Some(id) = req.id else {
        return Err(Problem::new(ProblemType::Internal));
    };
    match rc
        .armed(
            applied_note(id, false),
            ctx.vault.create_entity_linked(ctx.scope, req, links),
        )
        .await
    {
        Ok(view) => Ok(applied(Some(view.version), false)),
        Err(e) => from_vault(&e),
    }
}

/// Entity/document/place patch: field-level, re-applied onto the current version when the
/// base is stale.
async fn patch(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    p: &sm_ops::EntityPatch,
    base: Option<String>,
    kind: Option<NoteKind>,
) -> Result<OpResult, Problem> {
    let id = NoteId::from_ulid(p.id);
    for attempt in 0..5 {
        let current = match ctx.vault.note(ctx.scope, id).await {
            Ok(n) if n.trashed => {
                return Ok(server_kept(None, "the entity was deleted on the server"));
            }
            Ok(n)
                if kind.is_some_and(|k| k != n.kind)
                    || (!n.kind.is_entity() && n.kind != NoteKind::Concept) =>
            {
                return Ok(not_found());
            }
            Ok(n) => n,
            Err(VaultError::NotFound) => return Ok(not_found()),
            Err(e) => return from_vault(&e),
        };
        let merged = base.as_deref() != Some(current.version.as_str()) || attempt > 0;
        let aliases = if p.add_aliases.is_empty() && p.remove_aliases.is_empty() {
            None
        } else {
            let doc = Document::parse(&current.content);
            let mut list = doc
                .frontmatter()
                .map(vault_format::Frontmatter::aliases)
                .unwrap_or_default();
            list.retain(|a| !p.remove_aliases.contains(a));
            for a in &p.add_aliases {
                if !list.contains(a) {
                    list.push(a.clone());
                }
            }
            Some(list)
        };
        let mut fields: BTreeMap<String, Option<String>> = p
            .set
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        for k in &p.unset {
            fields.insert(k.clone(), None);
        }
        let req = VPatch {
            name: None,
            aliases,
            tags: None,
            fields,
            parent: None,
            if_match: Some(current.version.clone()),
            force: true,
        };
        match rc
            .armed(
                applied_note(id, merged),
                ctx.vault.patch_entity(ctx.scope, id, req),
            )
            .await
        {
            Ok(view) => return Ok(applied(Some(view.version), merged)),
            Err(VaultError::VersionConflict { .. }) => {}
            Err(e) => return from_vault(&e),
        }
    }
    Ok(server_kept(
        current_version(ctx, id).await,
        "the entity kept changing on the server; retry",
    ))
}

fn task_id(op: &Op) -> &str {
    match op {
        Op::TaskUpdate(p) => &p.id,
        Op::TaskComplete(p) => &p.id,
        Op::TaskCancel(p) => &p.id,
        Op::TaskReopen(p) | Op::TaskDelete(p) => &p.id,
        _ => "",
    }
}

/// Whether `op` is already in effect on `line` (another device did the same).
fn already_in_effect(op: &Op, line: &str) -> bool {
    let Some(task) = vault_format::tasks::TaskLine::parse(line) else {
        return false;
    };
    let status = task.status().lifecycle();
    match op {
        Op::TaskComplete(_) => status == domain::TaskStatus::Done,
        Op::TaskCancel(_) => status == domain::TaskStatus::Cancelled,
        Op::TaskReopen(_) => status == domain::TaskStatus::Open,
        Op::TaskUpdate(_) => {
            sync_model::apply::apply_task_op(line, op).is_ok_and(|after| after == line)
        }
        _ => false,
    }
}

async fn task_edit(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    op: &Op,
    base: Option<String>,
) -> Result<OpResult, Problem> {
    let id = task_id(op).to_owned();
    let result_hook = if matches!(op, Op::TaskDelete(_)) {
        applied_plain()
    } else {
        let block = id.clone();
        hook(move |w| applied(w.task_line_version(&block), false))
    };
    match rc
        .armed(
            result_hook,
            ctx.vault.apply_task_sync_op(ctx.scope, op.clone(), base),
        )
        .await
    {
        Ok(()) => Ok(match op {
            Op::TaskDelete(_) => applied(None, false),
            _ => applied(task_version(ctx, &id).await, false),
        }),
        Err(VaultError::VersionConflict { current }) => {
            let line = ctx
                .vault
                .task_line(ctx.scope, id.clone())
                .await
                .ok()
                .map(|(_, _, l)| l);
            Ok(
                if line.as_deref().is_some_and(|l| already_in_effect(op, l)) {
                    applied(Some(current), false)
                } else {
                    server_kept(Some(current), "the task line changed on the server")
                },
            )
        }
        Err(e) => from_vault(&e),
    }
}

/// Stores `result` as the op's result in `tx` (a database-only op); returns what to hand to
/// the receipt once `tx` committed.
async fn store_result(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    tx: &mut strata_index::ScopedTx,
    result: &OpResult,
) -> Result<Vec<u8>, Problem> {
    let bytes = crate::wire::encode(result).map_err(|e| Problem::internal(&e))?;
    let rec = log::idempotency_put(tx, rc.op_id(), rc.device(), &bytes, ctx.clock.now())
        .await
        .map_err(|e| index(&e))?;
    Ok(rec.result)
}

async fn relink(ctx: &PushContext<'_>, rc: &OpReceipt, id: NoteId) -> Result<(), Problem> {
    let now = ctx.clock.now();
    let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
    jobs::enqueue(
        &mut tx,
        &jobs::NewJob {
            id: JobId::generate(ctx.ids.as_ref()),
            kind: "link".to_owned(),
            note_id: Some(id),
            payload: Vec::new(),
            run_after: now,
            max_attempts: 5,
            dedupe_key: Some(id.to_string()),
        },
        now,
    )
    .await
    .map_err(|e| index(&e))?;
    let stored = store_result(ctx, rc, &mut tx, &applied(None, false)).await?;
    tx.commit().await.map_err(|e| index(&e))?;
    rc.mark_settled(stored);
    Ok(())
}

async fn device_settings(
    ctx: &PushContext<'_>,
    rc: &OpReceipt,
    p: &sm_ops::DeviceSettings,
) -> Result<OpResult, Problem> {
    let device = DeviceId::from_ulid(p.device_id);
    let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
    if devices::get_device(&mut tx, device)
        .await
        .map_err(|e| index(&e))?
        .is_none()
    {
        return Ok(not_found());
    }
    let changed = match p.reminders_enabled {
        Some(enabled) => set_device_reminders(&mut tx, device, enabled, ctx.clock.now())
            .await
            .map_err(|e| index(&e))?,
        None => false,
    };
    let result = applied(None, false);
    let stored = store_result(ctx, rc, &mut tx, &result).await?;
    tx.commit().await.map_err(|e| index(&e))?;
    rc.mark_settled(stored);
    if changed {
        publish(
            ctx.bus,
            ctx.scope.user_id(),
            Event::DeviceSettingsChanged {
                device_id: p.device_id,
            },
        );
    }
    Ok(result)
}

/// Sets a device's `reminders_enabled` and, when it changed, appends its `device_setting`
/// change-log row (`<device>:reminders_enabled`) in `tx`, so every device pulls it. Shared by
/// `device.settings` and `PATCH /devices/{id}`. Returns whether the value changed.
pub async fn set_device_reminders(
    tx: &mut strata_index::ScopedTx,
    device: DeviceId,
    enabled: bool,
    now: DateTime<Utc>,
) -> Result<bool, strata_index::IndexError> {
    let before = devices::get_device(tx, device)
        .await?
        .map(|d| d.reminders_enabled);
    if before.is_none() || before == Some(enabled) {
        return Ok(false);
    }
    devices::set_reminders_enabled(tx, device, enabled).await?;
    let entity = format!("{}:reminders_enabled", device.as_ulid());
    log::append_change(
        tx,
        &log::NewChange {
            entity_type: "device_setting",
            entity_id: &entity,
            op: ChangeOp::Upsert,
            version: None,
            at: now,
        },
    )
    .await?;
    Ok(true)
}

fn publish(bus: &EventBus, user: UserId, event: Event) {
    bus.publish(user, [event]);
}
