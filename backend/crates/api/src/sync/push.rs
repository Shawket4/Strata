//! `POST /sync/push` (PLAN §7.5 Sync, §12.4, D19).
//!
//! Ops apply **in order**, each through the vault writer actor as its own write (one git
//! commit per op, like the REST endpoint for the same mutation: the device's outbox is a list
//! of user intents and each stays individually revertible). A user's pushes are serialised
//! by [`SyncState::lock`], so the idempotency check and the store of one op never race with
//! another push of the same op.
//!
//! **Idempotency.** Every result is encoded once (`MessagePack`, named maps) and stored under
//! the op's `op_id` in `idempotency` before the next op runs; a replayed op (same `op_id`, in
//! the same or a later push) is not applied again and its stored bytes are copied into the
//! response unchanged, so replays are byte-identical. (A crash between the vault commit and
//! the store would apply the op again on replay; creates then answer with the client ID
//! taken — documented in `docs/ARCHITECTURE.md`.)
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
use strata_vault::{Candidate, MatchLevel as VMatch, VaultError, VaultService};
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
                    VMatch::Near => MatchLevel::Near,
                },
                #[allow(clippy::cast_possible_truncation)] // scores are in [0, 1]
                score: c.score as f32,
            })
            .collect(),
    }
}

/// The result for a vault error that has no op-specific meaning.
fn from_vault(e: &VaultError) -> OpResult {
    match e {
        VaultError::Duplicate(c) => duplicate(c),
        VaultError::VersionConflict { current } => {
            server_kept(Some(current.clone()), "the entity changed on the server")
        }
        other => rejected(&problem(other)),
    }
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
        return Err(Problem::new(ProblemType::InvalidBody).with_detail(format!(
            "at most {} ops per push",
            state.config.max_ops
        )));
    }
    let lock = state.lock(ctx.scope.user_id());
    let _guard = lock.lock().await;
    let mut submitted = Submitted::new();
    let mut results: Vec<(OpId, Vec<u8>)> = Vec::with_capacity(ops.len());
    for op in ops {
        let op_id = OpId::from_ulid(op.op_id);
        let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
        let stored = log::idempotency_get(&mut tx, op_id)
            .await
            .map_err(|e| index(&e))?;
        tx.commit().await.map_err(|e| index(&e.into()))?;
        let bytes = if let Some(rec) = stored {
            rec.result
        } else {
            let result = apply(ctx, &op, &mut submitted).await?;
            let bytes = crate::wire::encode(&result).map_err(|e| Problem::internal(&e))?;
            let mut tx = ctx.db.begin(ctx.scope).await.map_err(|e| index(&e))?;
            let rec =
                log::idempotency_put(&mut tx, op_id, ctx.device, &bytes, ctx.clock.now())
                    .await
                    .map_err(|e| index(&e))?;
            tx.commit().await.map_err(|e| index(&e.into()))?;
            rec.result
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

/// Applies one op (never an `Err` for anything the client caused).
#[allow(clippy::too_many_lines)] // one arm per op kind
async fn apply(
    ctx: &PushContext<'_>,
    op: &SyncOp,
    submitted: &mut Submitted,
) -> Result<OpResult, Problem> {
    if let Err(e) = op.validate() {
        return Ok(rejected_kind(ProblemType::InvalidBody, &e.to_string()));
    }
    let base = op.base_version.as_ref().map(|v| v.as_str().to_owned());
    let (v, s) = (ctx.vault, ctx.scope);
    Ok(match &op.op {
        Op::NoteCreate(p) => note_create(ctx, p, submitted).await,
        Op::NoteUpdate(p) => {
            note_update(
                ctx,
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
            match v.move_note(s, id, p.new_path.clone(), None).await {
                Ok(view) => applied(Some(view.version), false),
                Err(VaultError::PathTaken) => server_kept(
                    current_version(ctx, id).await,
                    "a note already exists at the target path",
                ),
                Err(VaultError::NotFound) => deleted_or_missing(ctx, id).await,
                Err(e) => from_vault(&e),
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
                Ok(_) => match v.delete_note(s, id).await {
                    Ok(_) => applied(None, false),
                    Err(e) => from_vault(&e),
                },
                Err(VaultError::NotFound) => not_found(),
                Err(e) => from_vault(&e),
            }
        }
        Op::Capture(p) => {
            match v
                .capture_as(s, p.text.clone(), NoteId::from_ulid(p.id), p.created)
                .await
            {
                Ok(c) => applied(Some(c.note.version), false),
                Err(e) => from_vault(&e),
            }
        }
        Op::RelationAdd(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match v.add_relation(s, src, dst, p.relation).await {
                Ok(_) => applied(current_version(ctx, src).await, false),
                Err(e) => from_vault(&e),
            }
        }
        Op::RelationRemove(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match v.remove_relation(s, src, dst, p.relation).await {
                Ok(_) => applied(current_version(ctx, src).await, false),
                // Both notes exist but the edge does not: already removed elsewhere.
                Err(VaultError::NotFound) if live(ctx, src).await && live(ctx, dst).await => {
                    applied(current_version(ctx, src).await, false)
                }
                Err(e) => from_vault(&e),
            }
        }
        Op::RelationRetype(p) => {
            let (src, dst) = (NoteId::from_ulid(p.src_id), NoteId::from_ulid(p.dst_id));
            match v
                .retype_relation(s, src, dst, p.relation, p.new_type)
                .await
            {
                Ok(()) => applied(current_version(ctx, src).await, false),
                Err(VaultError::NotFound) if live(ctx, src).await && live(ctx, dst).await => {
                    server_kept(
                        current_version(ctx, src).await,
                        "the relation no longer exists",
                    )
                }
                Err(e) => from_vault(&e),
            }
        }
        Op::SuggestionAccept(p) => {
            if p.edits.as_ref().is_some_and(|e| *e != sm_ops::SuggestionEdits::default()) {
                rejected_kind(
                    ProblemType::InvalidBody,
                    "edits are not supported for this suggestion kind",
                )
            } else {
                decide(ctx, p.id, true).await
            }
        }
        Op::SuggestionReject(p) => decide(ctx, p.id, false).await,
        Op::SuggestionReply(p) => {
            match v
                .reply_suggestion_as(
                    s,
                    SuggestionId::from_ulid(p.id),
                    ReplyId::from_ulid(p.reply_id),
                    p.text.clone(),
                )
                .await
            {
                Ok(_) => applied(None, false),
                Err(VaultError::Invalid(r)) if r.contains("already decided") => {
                    server_kept(None, "the suggestion was already decided")
                }
                Err(e) => from_vault(&e),
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
            create_entity(ctx, req, &[]).await
        }
        Op::EntityPatch(p) => patch(ctx, p, base, None).await,
        Op::DocumentPatch(p) => patch(ctx, p, base, Some(NoteKind::Document)).await,
        Op::PlacePatch(p) => patch(ctx, p, base, Some(NoteKind::Place)).await,
        Op::EntityMerge(p) => {
            match v
                .merge_entities(s, NoteId::from_ulid(p.id), NoteId::from_ulid(p.into_id))
                .await
            {
                Ok(view) => applied(Some(view.version), false),
                Err(e) => from_vault(&e),
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
            let (Some(copy_of), Some(companies), Some(people)) =
                (relation("copy-of"), relation("companies"), relation("people"))
            else {
                return Err(Problem::new(ProblemType::Internal));
            };
            let mut links: Vec<(RelationKey, NoteId)> = Vec::new();
            if let Some(c) = p.copy_of {
                links.push((copy_of, NoteId::from_ulid(c)));
            }
            links.extend(p.companies.iter().map(|c| (companies, NoteId::from_ulid(*c))));
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
            create_entity(ctx, req, &links).await
        }
        Op::DocumentCustody(p) => {
            let doc = NoteId::from_ulid(p.document_id);
            match v
                .add_custody_event(
                    s,
                    doc,
                    NewCustodyEvent {
                        kind: p.event,
                        date: p.at,
                        place: p.place_id.map(NoteId::from_ulid),
                        person: p.person_id.map(NoteId::from_ulid),
                        counterparty: p.counterparty_id.map(NoteId::from_ulid),
                        source: None,
                    },
                )
                .await
            {
                Ok(view) => applied(Some(view.version), false),
                Err(e) => from_vault(&e),
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
            create_entity(ctx, req, &[]).await
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
            match v.create_task(s, req).await {
                Ok(id) => applied(task_version(ctx, &id).await, false),
                Err(e) => from_vault(&e),
            }
        }
        Op::TaskUpdate(_)
        | Op::TaskComplete(_)
        | Op::TaskCancel(_)
        | Op::TaskReopen(_)
        | Op::TaskDelete(_) => task_edit(ctx, &op.op, base).await,
        Op::RelinkRequest(p) => {
            let id = NoteId::from_ulid(p.id);
            if live(ctx, id).await {
                relink(ctx, id).await?;
                applied(None, false)
            } else {
                not_found()
            }
        }
        Op::DeviceSettings(p) => device_settings(ctx, p).await?,
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

async fn note_create(ctx: &PushContext<'_>, p: &sm_ops::NoteCreate, submitted: &mut Submitted) -> OpResult {
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
        match ctx.vault.create_note(ctx.scope, req).await {
            Ok(view) => {
                submitted
                    .entry(id)
                    .or_default()
                    .push((Version::of_text(&p.content).as_str().to_owned(), p.content.clone()));
                return applied(Some(view.version), false);
            }
            Err(VaultError::PathTaken) => {
                let stem = p.path.strip_suffix(".md").unwrap_or(&p.path);
                path = format!("{stem} {n}.md");
            }
            Err(e) => return from_vault(&e),
        }
    }
    rejected(&Problem::new(ProblemType::PathTaken))
}

/// `note.update` (D19; see the module docs).
async fn note_update(
    ctx: &PushContext<'_>,
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
            Err(e) => return Ok(from_vault(&e)),
        };
        let record = |submitted: &mut Submitted| {
            submitted.entry(id).or_default().push((
                Version::of_text(content).as_str().to_owned(),
                content.to_owned(),
            ));
        };
        let write = |text: String| {
            let version = current.version.clone();
            async move { v.update_note(s, id, text, version).await }
        };
        if current.version == base {
            match write(content.to_owned()).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), false));
                }
                Err(VaultError::VersionConflict { .. }) => continue,
                Err(e) => return Ok(from_vault(&e)),
            }
        }
        let Ok(base_version) = base.parse::<Version>() else {
            return Ok(rejected_kind(ProblemType::InvalidBody, "invalid base_version"));
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
        match decide_update(&base_version, base_text.as_deref(), &current.content, content) {
            UpdateDecision::AlreadyApplied => {
                return Ok(applied(Some(current.version), false));
            }
            UpdateDecision::FastForward => match write(content.to_owned()).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), false));
                }
                Err(VaultError::VersionConflict { .. }) => {}
                Err(e) => return Ok(from_vault(&e)),
            },
            UpdateDecision::Merged(text) => match write(text).await {
                Ok(view) => {
                    record(submitted);
                    return Ok(applied(Some(view.version), true));
                }
                Err(VaultError::VersionConflict { .. }) => {}
                Err(e) => return Ok(from_vault(&e)),
            },
            UpdateDecision::Conflict(c) => {
                return conflict_copy(
                    ctx,
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

#[allow(clippy::too_many_arguments)] // the parts of one conflict record
async fn conflict_copy(
    ctx: &PushContext<'_>,
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
            Err(e) => return Ok(from_vault(&e)),
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
    ctx.vault
        .create_suggestion(
            ctx.scope,
            SuggestionId::from_ulid(op_id),
            Some(id),
            "conflict",
            &payload,
        )
        .await
        .map_err(|e| problem(&e))?;
    Ok(OpResult::Conflict {
        server_version: server_version.parse().ok(),
        resolution: ConflictResolution::ConflictCopy {
            note_id: copy_id.as_ulid(),
            path: copy.path,
            version: copy
                .version
                .parse()
                .map_err(|_| Problem::new(ProblemType::Internal))?,
        },
    })
}

async fn decide(ctx: &PushContext<'_>, id: ulid::Ulid, accept: bool) -> OpResult {
    match ctx
        .vault
        .decide_suggestion(ctx.scope, SuggestionId::from_ulid(id), accept)
        .await
    {
        Ok(_) => applied(None, false),
        Err(VaultError::Invalid(r)) if r.contains("already decided") => {
            server_kept(None, "the suggestion was already decided")
        }
        Err(e) => from_vault(&e),
    }
}

async fn create_entity(
    ctx: &PushContext<'_>,
    req: NewEntity,
    links: &[(RelationKey, NoteId)],
) -> OpResult {
    let id = req.id;
    match ctx.vault.create_entity(ctx.scope, req).await {
        Ok(mut view) => {
            for (key, dst) in links {
                if let Some(id) = id
                    && let Err(e) = ctx.vault.add_relation(ctx.scope, id, *dst, *key).await
                {
                    // The entity exists; report the link that failed.
                    return from_vault(&e);
                }
            }
            if !links.is_empty()
                && let Some(id) = id
                && let Ok(v) = ctx.vault.note(ctx.scope, id).await
            {
                view = v;
            }
            applied(Some(view.version), false)
        }
        Err(e) => from_vault(&e),
    }
}

/// Entity/document/place patch: field-level, re-applied onto the current version when the
/// base is stale.
async fn patch(
    ctx: &PushContext<'_>,
    p: &sm_ops::EntityPatch,
    base: Option<String>,
    kind: Option<NoteKind>,
) -> OpResult {
    let id = NoteId::from_ulid(p.id);
    for attempt in 0..5 {
        let current = match ctx.vault.note(ctx.scope, id).await {
            Ok(n) if n.trashed => return server_kept(None, "the entity was deleted on the server"),
            Ok(n)
                if kind.is_some_and(|k| k != n.kind)
                    || (!n.kind.is_entity() && n.kind != NoteKind::Concept) =>
            {
                return not_found();
            }
            Ok(n) => n,
            Err(VaultError::NotFound) => return not_found(),
            Err(e) => return from_vault(&e),
        };
        let merged = base.as_deref() != Some(current.version.as_str());
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
        match ctx.vault.patch_entity(ctx.scope, id, req).await {
            Ok(view) => return applied(Some(view.version), merged || attempt > 0),
            Err(VaultError::VersionConflict { .. }) => {}
            Err(e) => return from_vault(&e),
        }
    }
    server_kept(current_version(ctx, id).await, "the entity kept changing on the server; retry")
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
        Op::TaskUpdate(_) => sync_model::apply::apply_task_op(line, op)
            .is_ok_and(|after| after == line),
        _ => false,
    }
}

async fn task_edit(ctx: &PushContext<'_>, op: &Op, base: Option<String>) -> OpResult {
    let id = task_id(op).to_owned();
    match ctx
        .vault
        .apply_task_sync_op(ctx.scope, op.clone(), base)
        .await
    {
        Ok(()) => match op {
            Op::TaskDelete(_) => applied(None, false),
            _ => applied(task_version(ctx, &id).await, false),
        },
        Err(VaultError::VersionConflict { current }) => {
            let line = ctx
                .vault
                .task_line(ctx.scope, id.clone())
                .await
                .ok()
                .map(|(_, _, l)| l);
            if line.as_deref().is_some_and(|l| already_in_effect(op, l)) {
                applied(Some(current), false)
            } else {
                server_kept(Some(current), "the task line changed on the server")
            }
        }
        Err(e) => from_vault(&e),
    }
}

async fn relink(ctx: &PushContext<'_>, id: NoteId) -> Result<(), Problem> {
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
    tx.commit().await.map_err(|e| index(&e.into()))?;
    Ok(())
}

async fn device_settings(
    ctx: &PushContext<'_>,
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
    if let Some(enabled) = p.reminders_enabled {
        devices::set_reminders_enabled(&mut tx, device, enabled)
            .await
            .map_err(|e| index(&e))?;
        let entity = format!("{}:reminders_enabled", p.device_id);
        log::append_change(
            &mut tx,
            &log::NewChange {
                entity_type: "device_setting",
                entity_id: &entity,
                op: ChangeOp::Upsert,
                version: None,
                at: ctx.clock.now(),
            },
        )
        .await
        .map_err(|e| index(&e))?;
    }
    tx.commit().await.map_err(|e| index(&e.into()))?;
    if p.reminders_enabled.is_some() {
        publish(ctx.bus, ctx.scope.user_id(), Event::DeviceSettingsChanged {
            device_id: p.device_id,
        });
    }
    Ok(applied(None, false))
}

fn publish(bus: &EventBus, user: UserId, event: Event) {
    bus.publish(user, [event]);
}
