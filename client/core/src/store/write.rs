//! The write path (PLAN §12.3) and the rules that keep local state consistent with the outbox.
//!
//! **Model.** For every local entity (a note or a suggestion) the database keeps the server's
//! **base** state and the **current** state shown to the user. The current state is always
//! `fold(apply_op, base, live ops of that entity in outbox order)`:
//!
//! - an intent applies its op to the current state (optimistic), appends the op to the outbox
//!   and re-derives the index — **in one transaction**, so a crash can never leave a local
//!   change without its op (or an op without its change);
//! - a pull replaces the base and [`rebuild_note`] re-applies the live ops on top, so remote
//!   changes and unsynced local edits combine instead of overwriting each other;
//! - a rejected op stops being live and the rebuild rolls its effect back;
//! - after an epoch change the cache is re-bootstrapped and every entity rebuilt, which
//!   re-applies all pending ops to the fresh data.
//!
//! [`apply_to_note`] is a pure function of `(state, op)`. It uses `sync-model`'s shared apply
//! rules (relations, entity patches, task edits) and `vault-format` for everything else, so the
//! device writes the same bytes the server does. Because versions are content hashes
//! (`sync_model::Version`), the device can verify that: a server `applied{new_version}` whose
//! version equals the hash of the locally applied content becomes the new base immediately.

use chrono::{DateTime, FixedOffset};
use rusqlite::Connection;
use sync_model::apply as rules;
use sync_model::ops as sm;
use ulid::Ulid;
use vault_format::tasks::{Reminder, TaskSpec};
use vault_format::{Document, KnownKey, PropertyValue, RelationKey};

use crate::error::{CoreError, CoreResult};
use crate::format::edit;
use crate::store::index::{LinkResolver, Reindex};
use crate::store::notes::{self, NoteBase, NoteState};
use crate::store::outbox;
use crate::sync::model::{Op, Version};
use crate::view::Topics;

/// The local entity an op changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEntity {
    /// A note (by ID).
    Note(String),
    /// A suggestion (by ID).
    Suggestion(String),
    /// Nothing local (e.g. `relink.request`).
    Nothing,
}

impl LocalEntity {
    /// The `outbox.local_entity` spelling.
    pub fn key(&self) -> String {
        match self {
            Self::Note(id) => format!("note:{id}"),
            Self::Suggestion(id) => format!("suggestion:{id}"),
            Self::Nothing => "none:".to_owned(),
        }
    }

    /// Parses the column.
    pub fn parse(s: &str) -> Self {
        if let Some(id) = s.strip_prefix("note:") {
            Self::Note(id.to_owned())
        } else if let Some(id) = s.strip_prefix("suggestion:") {
            Self::Suggestion(id.to_owned())
        } else {
            Self::Nothing
        }
    }
}

/// An intent turned into an op.
#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    /// New op ID.
    pub op_id: Ulid,
    /// Local entity.
    pub local: LocalEntity,
    /// The op.
    pub op: Op,
}

/// Link text for a note ID (shortest unique link, as the server writes it), or `None` when
/// the note is not known locally.
pub trait Links {
    /// The link target for `id`.
    fn link_for(&self, id: Ulid) -> Option<String>;
}

/// [`Links`] over the local database.
#[derive(Debug)]
pub struct DbLinks {
    paths: std::collections::HashMap<String, String>,
    resolver: LinkResolver,
}

impl DbLinks {
    /// Loads the live paths.
    pub fn load(conn: &Connection) -> CoreResult<Self> {
        Ok(Self {
            paths: notes::live_paths(conn)?.into_iter().collect(),
            resolver: LinkResolver::load(conn)?,
        })
    }
}

impl Links for DbLinks {
    fn link_for(&self, id: Ulid) -> Option<String> {
        self.paths
            .get(&id.to_string())
            .map(|p| self.resolver.link_text_for(p))
    }
}

fn validate_note_path(path: &str) -> CoreResult<()> {
    vault_format::filename::validate_vault_path(path)
        .map_err(|e| CoreError::invalid("path", &format!("{e:?}").to_ascii_lowercase()))?;
    if !path.to_ascii_lowercase().ends_with(".md") {
        return Err(CoreError::invalid("path", "not_markdown"));
    }
    Ok(())
}

fn require(state: Option<NoteState>) -> CoreResult<NoteState> {
    state.ok_or_else(|| CoreError::not_found("note"))
}

fn apply_err(e: &rules::ApplyError) -> CoreError {
    match e {
        rules::ApplyError::TaskNotFound(_) => CoreError::not_found("task"),
        rules::ApplyError::Task(t) => CoreError::TaskChange {
            reason: edit::task_error_code(t).to_owned(),
        },
        rules::ApplyError::MissingNextId(_) => CoreError::TaskChange {
            reason: "missing_next_id".to_owned(),
        },
        rules::ApplyError::UnknownEntity(_) => CoreError::not_found("note"),
        other => CoreError::invalid("op", &other.to_string()),
    }
}

fn fm_err(e: &vault_format::FrontmatterError) -> CoreError {
    CoreError::invalid("frontmatter", &e.to_string())
}

/// Edits a note's frontmatter with a shared rule.
fn edit_fm(
    state: NoteState,
    f: impl FnOnce(&mut vault_format::Frontmatter) -> Result<(), rules::ApplyError>,
) -> CoreResult<Option<NoteState>> {
    let mut doc = Document::parse(&state.content);
    if let Some(e) = doc.frontmatter().and_then(|f| f.error()) {
        return Err(fm_err(e));
    }
    f(doc.frontmatter_mut()).map_err(|e| apply_err(&e))?;
    Ok(Some(NoteState {
        path: state.path,
        content: doc.render(),
    }))
}

fn link_or_missing(links: &dyn Links, id: Ulid) -> CoreResult<String> {
    links.link_for(id).ok_or_else(|| CoreError::not_found("note"))
}

fn with_body(
    state: NoteState,
    f: impl FnOnce(&str) -> CoreResult<String>,
) -> CoreResult<Option<NoteState>> {
    let mut doc = Document::parse(&state.content);
    let body = f(doc.body())?;
    doc.set_body(body);
    Ok(Some(NoteState {
        path: state.path,
        content: doc.render(),
    }))
}

/// The line of a new task (`task.create`), rendered canonically with `vault-format`.
pub fn task_create_line(p: &sm::TaskCreate) -> String {
    TaskSpec {
        description: p.text.trim().to_owned(),
        priority: p.priority,
        recurrence: p.recurrence.clone(),
        start: p.start,
        scheduled: p.scheduled,
        due: p.due,
        reminders: p
            .reminders
            .iter()
            .map(|dt| Reminder {
                date: dt.date(),
                time: Some(dt.time()),
            })
            .collect(),
        block_id: Some(p.id.clone()),
        ..TaskSpec::default()
    }
    .render()
}

/// The vault path of a new entity note.
pub fn entity_path(kind: domain::NoteKind, name: &str) -> String {
    format!(
        "{}/{}.md",
        kind.default_folder(),
        vault_format::filename::sanitize_file_name(name)
    )
}

fn new_entity(
    id: Ulid,
    kind: domain::NoteKind,
    name: &str,
    aliases: &[String],
    fill: impl FnOnce(&mut vault_format::Frontmatter) -> CoreResult<()>,
) -> CoreResult<Option<NoteState>> {
    let content = edit::entity_content(id, kind, aliases)?;
    let mut doc = Document::parse(&content);
    fill(doc.frontmatter_mut())?;
    Ok(Some(NoteState {
        path: entity_path(kind, name),
        content: doc.render(),
    }))
}

fn relation(name: &str) -> CoreResult<RelationKey> {
    name.parse::<RelationKey>()
        .map_err(|_| CoreError::Internal(format!("relation key {name}")))
}

fn document_fields(
    fm: &mut vault_format::Frontmatter,
    p: &sm::DocumentCreate,
    links: &dyn Links,
) -> CoreResult<()> {
    if let Some(t) = &p.doc_type {
        fm.set_text(KnownKey::DocType, t.clone())
            .map_err(|e| fm_err(&e))?;
    }
    if let Some(c) = &p.copy {
        fm.set_text(KnownKey::Copy, c.as_str())
            .map_err(|e| fm_err(&e))?;
    }
    if let Some(e) = p.expires {
        fm.set_text(KnownKey::Expires, e.to_string())
            .map_err(|e| fm_err(&e))?;
    }
    for (rel, ids) in [("companies", &p.companies), ("people", &p.people)] {
        let rk = relation(rel)?;
        for id in ids {
            fm.add_relation_link(rk, &link_or_missing(links, *id)?)
                .map_err(|e| fm_err(&e))?;
        }
    }
    if let Some(orig) = p.copy_of {
        fm.add_relation_link(relation("copy-of")?, &link_or_missing(links, orig)?)
            .map_err(|e| fm_err(&e))?;
    }
    Ok(())
}

/// What `op` does to the note whose current state is `state`. Pure.
///
/// Creates (`note.create`, `capture`, `entity/document/place.create`, `task.create` into a
/// missing task home) are rendered locally with `vault-format`; `sync-model` has no shared
/// renderer for them yet, so the server's bytes may differ until the next pull replaces the
/// base. Ops the device cannot apply locally (`entity.merge`, `document.custody`, and the
/// rewriting of inbound links on `note.move`) leave the state as is and arrive by pull.
#[allow(clippy::too_many_lines)] // one arm per op kind
pub fn apply_to_note(
    state: Option<NoteState>,
    op: &Op,
    links: &dyn Links,
) -> CoreResult<Option<NoteState>> {
    match op {
        Op::NoteCreate(p) => {
            validate_note_path(&p.path)?;
            Ok(Some(NoteState {
                path: p.path.clone(),
                content: edit::with_id(&p.content, p.id)?,
            }))
        }
        Op::Capture(p) => Ok(Some(NoteState {
            path: edit::capture_path(&p.created, std::iter::empty()),
            content: edit::capture_content(p.id, &p.created, &p.text)?,
        })),
        Op::EntityCreate(p) => {
            if !p.kind.is_entity() && p.kind != domain::NoteKind::Concept {
                return Err(CoreError::invalid("kind", "not_an_entity"));
            }
            new_entity(p.id, p.kind, &p.name, &p.aliases, |fm| {
                for (k, v) in &p.fields {
                    fm.set(k, PropertyValue::Text(v.clone()))
                        .map_err(|e| fm_err(&e))?;
                }
                Ok(())
            })
        }
        Op::DocumentCreate(p) => {
            new_entity(p.id, domain::NoteKind::Document, &p.name, &p.aliases, |fm| {
                document_fields(fm, p, links)
            })
        }
        Op::PlaceCreate(p) => new_entity(p.id, domain::NoteKind::Place, &p.name, &p.aliases, |fm| {
            if let Some(parent) = p.parent_id {
                fm.add_relation_link(relation("part-of")?, &link_or_missing(links, parent)?)
                    .map_err(|e| fm_err(&e))?;
            }
            if let Some(a) = &p.address {
                fm.set_text(KnownKey::Address, a.clone())
                    .map_err(|e| fm_err(&e))?;
            }
            Ok(())
        }),
        Op::NoteUpdate(p) => {
            let s = require(state)?;
            Ok(Some(NoteState {
                path: s.path,
                content: p.content.clone(),
            }))
        }
        Op::NoteMove(p) => {
            validate_note_path(&p.new_path)?;
            let s = require(state)?;
            Ok(Some(NoteState {
                path: p.new_path.clone(),
                content: s.content,
            }))
        }
        Op::NoteDelete(_) => {
            require(state)?;
            Ok(None)
        }
        Op::RelationAdd(r) => {
            let target = link_or_missing(links, r.dst_id)?;
            edit_fm(require(state)?, |fm| {
                rules::relation_add(fm, r.relation, &target).map(|_| ())
            })
        }
        Op::RelationRemove(r) => {
            let target = link_or_missing(links, r.dst_id)?;
            edit_fm(require(state)?, |fm| {
                rules::relation_remove(fm, r.relation, &target).map(|_| ())
            })
        }
        Op::RelationRetype(r) => {
            let target = link_or_missing(links, r.dst_id)?;
            edit_fm(require(state)?, |fm| {
                rules::relation_retype(fm, r.relation, r.new_type, &target)
            })
        }
        Op::EntityPatch(p) | Op::DocumentPatch(p) | Op::PlacePatch(p) => {
            edit_fm(require(state)?, |fm| rules::entity_patch(fm, p))
        }
        Op::TaskCreate(p) => {
            let line = task_create_line(p);
            match state {
                Some(s) => with_body(s, |body| Ok(edit::append_task(body, &line))),
                // The task home does not exist locally yet (§6.11 `tasks/Tasks.md`).
                None => Ok(Some(NoteState {
                    path: TASK_HOME.to_owned(),
                    content: edit::append_task("", &line),
                })),
            }
        }
        Op::TaskUpdate(_)
        | Op::TaskComplete(_)
        | Op::TaskCancel(_)
        | Op::TaskReopen(_)
        | Op::TaskDelete(_) => with_body(require(state)?, |body| {
            rules::apply_task_op(body, op).map_err(|e| apply_err(&e))
        }),
        Op::SuggestionAccept(_)
        | Op::SuggestionReject(_)
        | Op::SuggestionReply(_)
        | Op::RelinkRequest(_)
        | Op::DeviceSettings(_)
        | Op::EntityMerge(_)
        | Op::DocumentCustody(_) => Ok(state),
    }
}

/// Default home of tasks created without a note (§6.11).
pub const TASK_HOME: &str = "tasks/Tasks.md";

/// What a suggestion op does to a suggestion's status. Pure.
pub fn apply_to_suggestion(status: Option<String>, op: &Op) -> Option<String> {
    match op {
        Op::SuggestionAccept(_) => status.map(|_| "accepted".to_owned()),
        Op::SuggestionReject(_) => status.map(|_| "rejected".to_owned()),
        _ => status,
    }
}

/// Recomputes a note's current state from its base and its live ops; records it in `re`.
pub fn rebuild_note(conn: &Connection, id: &str, now: &str, re: &mut Reindex) -> CoreResult<()> {
    let links = DbLinks::load(conn)?;
    let mut state = notes::base(conn, id)?.map(|b| NoteState {
        path: b.path,
        content: b.content,
    });
    for op in outbox::live_for(conn, &LocalEntity::Note(id.to_owned()).key())? {
        // An op that no longer applies (e.g. its task line vanished remotely) is left for the
        // server to decide; the local view simply doesn't show it.
        if let Ok(next) = apply_to_note(state.clone(), &op.op, &links) {
            state = next;
        }
    }
    let changed = notes::write_current(conn, id, state.as_ref(), now)?;
    if state.is_none() {
        notes::prune(conn, id)?;
    }
    re.note(id, changed);
    Ok(())
}

/// Recomputes a suggestion's current status from its base and live ops.
pub fn rebuild_suggestion(conn: &Connection, id: &str, re: &mut Reindex) -> CoreResult<()> {
    let base: Option<String> = rusqlite::OptionalExtension::optional(conn.query_row(
        "SELECT base_status FROM suggestions WHERE id = ?1",
        [id],
        |r| r.get(0),
    ))?;
    let mut status = base;
    for op in outbox::live_for(conn, &LocalEntity::Suggestion(id.to_owned()).key())? {
        status = apply_to_suggestion(status, &op.op);
    }
    if let Some(s) = status {
        conn.execute(
            "UPDATE suggestions SET status = ?2 WHERE id = ?1",
            rusqlite::params![id, s],
        )?;
    }
    re.topics(Topics::INBOX | Topics::SUGGESTIONS);
    Ok(())
}

/// Rebuilds any local entity.
pub fn rebuild(
    conn: &Connection,
    local: &LocalEntity,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    match local {
        LocalEntity::Note(id) => rebuild_note(conn, id, now, re),
        LocalEntity::Suggestion(id) => rebuild_suggestion(conn, id, re),
        LocalEntity::Nothing => Ok(()),
    }
}

/// The block ID a task op addresses.
pub fn task_op_id(op: &Op) -> Option<&str> {
    match op {
        Op::TaskUpdate(p) => Some(&p.id),
        Op::TaskComplete(p) => Some(&p.id),
        Op::TaskCancel(p) => Some(&p.id),
        Op::TaskReopen(p) | Op::TaskDelete(p) => Some(&p.id),
        _ => None,
    }
}

/// The `base_version` of an op made against the note's current content (`sync-model` rules:
/// the note's version for note edits and patches, the task line's version for task edits).
/// Versions are content hashes, so the version of the current local content equals the
/// server's version once every earlier op of the same entity has been applied there.
pub fn base_version_for(current: Option<&NoteState>, op: &Op) -> CoreResult<Option<Version>> {
    if !op.kind().requires_base_version() {
        return Ok(None);
    }
    let Some(s) = current else {
        return Err(CoreError::not_found("note"));
    };
    match task_op_id(op) {
        Some(id) => {
            let doc = Document::parse(&s.content);
            let (span, _) =
                rules::find_task(doc.body(), id).ok_or_else(|| CoreError::not_found("task"))?;
            Ok(Some(rules::task_line_version(&doc.body()[span])))
        }
        None => Ok(Some(Version::of_text(&s.content))),
    }
}

/// Runs an intent: optimistic local change + outbox op + index, in one transaction. Returns
/// the view topics that changed. On error nothing is written.
pub fn execute(conn: &mut Connection, intent: &Intent, now: &str) -> CoreResult<Topics> {
    let tx = conn.transaction()?;
    let mut re = Reindex::new();
    let base_version = match &intent.local {
        LocalEntity::Note(id) => {
            let links = DbLinks::load(&tx)?;
            let current = notes::current(&tx, id)?;
            let base_version = base_version_for(current.as_ref(), &intent.op)?;
            let next = apply_to_note(current, &intent.op, &links)?;
            let changed = notes::write_current(&tx, id, next.as_ref(), now)?;
            if next.is_none() {
                notes::prune(&tx, id)?;
            }
            re.note(id, changed);
            base_version
        }
        LocalEntity::Suggestion(id) => {
            let status: Option<String> = rusqlite::OptionalExtension::optional(tx.query_row(
                "SELECT status FROM suggestions WHERE id = ?1",
                [id],
                |r| r.get(0),
            ))?;
            let Some(next) = apply_to_suggestion(status, &intent.op) else {
                return Err(CoreError::not_found("suggestion"));
            };
            tx.execute(
                "UPDATE suggestions SET status = ?2 WHERE id = ?1",
                rusqlite::params![id, next],
            )?;
            re.topics(Topics::INBOX | Topics::SUGGESTIONS);
            None
        }
        LocalEntity::Nothing => None,
    };
    outbox::append(
        &tx,
        &outbox::NewOp {
            op_id: &intent.op_id.to_string(),
            local_entity: &intent.local.key(),
            base_version: base_version.as_ref(),
            op: &intent.op,
            created: now,
        },
    )?;
    re.topics(Topics::SYNC);
    let topics = re.apply(&tx)?;
    tx.commit()?;
    Ok(topics)
}

/// The note that holds task `task_id`, if indexed.
pub fn note_of_task(conn: &Connection, task_id: &str) -> CoreResult<Option<String>> {
    Ok(rusqlite::OptionalExtension::optional(conn.query_row(
        "SELECT note_id FROM tasks WHERE id = ?1",
        [task_id],
        |r| r.get(0),
    ))?)
}

/// The server base after the server applied `op` with `new_version`: the op folded over the
/// previous base, **if** its content hashes to `new_version` (the server wrote the same
/// bytes). `None` when unknown — the next pull brings the server's state.
pub fn base_after_applied(
    previous: Option<NoteBase>,
    op: &Op,
    new_version: Option<&Version>,
    links: &dyn Links,
) -> Option<Option<NoteBase>> {
    let state = previous.map(|b| NoteState {
        path: b.path,
        content: b.content,
    });
    let next = apply_to_note(state, op, links).ok()?;
    match (next, new_version) {
        (None, _) => Some(None),
        (Some(s), Some(v)) if v.matches(&s.content) => Some(Some(NoteBase {
            path: s.path,
            content: s.content,
            version: v.as_str().to_owned(),
        })),
        _ => None,
    }
}

/// `(kind, title)` of the item a create op creates (duplicate prompts).
pub fn describe_create(op: &Op) -> (String, String) {
    match op {
        Op::NoteCreate(p) => ("note".to_owned(), crate::format::title_of(&p.path)),
        Op::Capture(p) => (
            "capture".to_owned(),
            p.text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        ),
        Op::EntityCreate(p) => (p.kind.as_str().to_owned(), p.name.clone()),
        Op::DocumentCreate(p) => ("document".to_owned(), p.name.clone()),
        Op::PlaceCreate(p) => ("place".to_owned(), p.name.clone()),
        Op::TaskCreate(p) => ("task".to_owned(), p.text.clone()),
        other => (other.kind().as_str().to_owned(), String::new()),
    }
}

/// The local creation time of a capture, in whole seconds (the inbox file name has second
/// precision, §6.9).
pub fn capture_created(now: DateTime<chrono::Utc>, tz: chrono_tz::Tz) -> DateTime<FixedOffset> {
    let local = now.with_timezone(&tz).fixed_offset();
    local - chrono::Duration::nanoseconds(i64::from(local.timestamp_subsec_nanos()))
}
