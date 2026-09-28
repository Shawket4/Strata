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

use chrono_tz::Tz;
use item_render::entity::{EntitySpec, document_relations};
use rusqlite::Connection;
use sync_model::apply as rules;
use ulid::Ulid;
use vault_format::Document;

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

/// What applying an op needs from the local state besides the note itself.
pub trait Links {
    /// Link text for a note ID (shortest unique link, as the server writes it), or `None`
    /// when the note is not known locally.
    fn link_for(&self, id: Ulid) -> Option<String>;

    /// The account's time zone (dates of `tasks/Tasks.md` month headings).
    fn time_zone(&self) -> Tz {
        chrono_tz::UTC
    }

    /// The paths of the live notes other than `except` (a new entity takes the first free
    /// name among them, ` 2`, ` 3`, … as the server does).
    fn taken_paths(&self, except: Ulid) -> Vec<String> {
        let _ = except;
        Vec::new()
    }
}

/// [`Links`] over the local database.
#[derive(Debug)]
pub struct DbLinks {
    paths: std::collections::HashMap<String, String>,
    resolver: LinkResolver,
    tz: Tz,
}

impl DbLinks {
    /// Loads the live paths and the account's time zone.
    pub fn load(conn: &Connection) -> CoreResult<Self> {
        Ok(Self {
            paths: notes::live_paths(conn)?.into_iter().collect(),
            resolver: LinkResolver::load(conn)?,
            tz: crate::store::account::get(conn)?
                .and_then(|a| a.timezone.parse::<Tz>().ok())
                .unwrap_or(chrono_tz::UTC),
        })
    }
}

impl Links for DbLinks {
    fn link_for(&self, id: Ulid) -> Option<String> {
        self.paths
            .get(&id.to_string())
            .map(|p| self.resolver.link_text_for(p))
    }

    fn time_zone(&self) -> Tz {
        self.tz
    }

    fn taken_paths(&self, except: Ulid) -> Vec<String> {
        let except = except.to_string();
        self.paths
            .iter()
            .filter(|(id, _)| **id != except)
            .map(|(_, p)| p.clone())
            .collect()
    }
}

/// Stable reason code of an invalid vault path (never the path itself: errors carry no user
/// content).
fn path_error_code(e: &vault_format::filename::PathError) -> &'static str {
    use vault_format::filename::{FileNameError as F, PathError as P};
    match e {
        P::Absolute => "absolute",
        P::Segment { error, .. } => match error {
            F::Empty => "empty_segment",
            F::ForbiddenChar(_) => "forbidden_char",
            F::ControlChar(_) => "control_char",
            F::LeadingDot => "leading_dot",
            F::EdgeWhitespaceOrDot => "edge_whitespace_or_dot",
            F::Reserved(_) => "reserved_name",
            F::TooLong(_) => "too_long",
        },
    }
}

fn validate_note_path(path: &str) -> CoreResult<()> {
    vault_format::filename::validate_vault_path(path)
        .map_err(|e| CoreError::invalid("path", path_error_code(&e)))?;
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
    links
        .link_for(id)
        .ok_or_else(|| CoreError::not_found("note"))
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

/// A new entity, document or place note (the shared `item-render` skeleton) at the first free
/// name among the local notes, stamped with the op's creation time, as the server writes it.
fn new_entity(
    id: Ulid,
    spec: &EntitySpec,
    created: &chrono::DateTime<chrono::Utc>,
    links: &dyn Links,
) -> CoreResult<Option<NoteState>> {
    let taken = links.taken_paths(id);
    let path = spec.path(taken.iter().map(String::as_str));
    Ok(Some(NoteState {
        content: spec
            .render(id, &path, created)
            .map_err(|e| edit::render_error(&e))?,
        path,
    }))
}

/// What `op` does to the note whose current state is `state`. Pure.
///
/// Creates are rendered with the shared `item-render` / `sync-model` code the server runs
/// (L16), from the op alone — its IDs (a new `tasks/Tasks.md` included) and the device's
/// creation time — so for the same input the device writes the server's bytes. Only names
/// taken on the server but not yet known here (` 2` suffixes) differ until the next pull
/// replaces the base. Ops the device cannot apply locally (`entity.merge`, and the rewriting
/// of inbound links on `note.move`) leave the state as is and arrive by pull.
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
                content: item_render::note::new_note(&p.content, p.id, &p.created)
                    .map_err(|e| edit::render_error(&e))?,
            }))
        }
        Op::Capture(p) => Ok(Some(NoteState {
            path: item_render::paths::capture_path(&p.created, std::iter::empty()),
            content: item_render::capture::capture_content(p.id, &p.created, &p.text)
                .map_err(|e| edit::render_error(&e))?,
        })),
        Op::EntityCreate(p) => {
            if !p.kind.is_entity() && p.kind != domain::NoteKind::Concept {
                return Err(CoreError::invalid("kind", "not_an_entity"));
            }
            new_entity(p.id, &EntitySpec::from_entity_create(p), &p.created, links)
        }
        Op::DocumentCreate(p) => {
            let relations = document_relations(p)
                .into_iter()
                .map(|(rel, id)| Ok((rel, link_or_missing(links, id)?)))
                .collect::<CoreResult<Vec<_>>>()?;
            new_entity(
                p.id,
                &EntitySpec::from_document_create(p, relations),
                &p.created,
                links,
            )
        }
        Op::PlaceCreate(p) => {
            let part_of = p
                .parent_id
                .map(|parent| link_or_missing(links, parent))
                .transpose()?;
            new_entity(
                p.id,
                &EntitySpec::from_place_create(p, part_of),
                &p.created,
                links,
            )
        }
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
            let heading = item_render::task::heading_date(&p.created, links.time_zone());
            // The task home does not exist locally yet (§6.11 `tasks/Tasks.md`): made here
            // with the op's `home_id`, as the server makes it.
            let (path, content, new_id) = match state {
                Some(s) => (s.path, s.content, None),
                None => (TASK_HOME.to_owned(), String::new(), p.home_id),
            };
            let mut doc = Document::parse(&content);
            let body = rules::apply_task_create(doc.body(), p, heading, doc.line_ending())
                .map_err(|e| apply_err(&e))?;
            doc.set_body(body);
            item_render::task::stamp_home(&mut doc, new_id, &p.created)
                .map_err(|e| edit::render_error(&e))?;
            Ok(Some(NoteState {
                path,
                content: doc.render(),
            }))
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
        | Op::EntityMerge(_) => Ok(state),
        Op::DocumentCustody(p) => {
            let s = require(state)?;
            // A user-recorded event without a source note is uncited, as on the server.
            let event = rules::custody_event(
                p,
                |id| links.link_for(id).map(|t| format!("[[{t}]]")),
                Vec::new(),
            )
            .map_err(|e| apply_err(&e))?;
            let mut doc = Document::parse(&s.content);
            rules::record_custody(&mut doc, event).map_err(|e| apply_err(&e))?;
            Ok(Some(NoteState {
                path: s.path,
                content: doc.render(),
            }))
        }
    }
}

/// Default home of tasks created without a note (§6.11).
pub use item_render::paths::DEFAULT_TASK_NOTE as TASK_HOME;

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
    let mut base_content = None;
    let base_version = match &intent.local {
        LocalEntity::Note(id) => {
            let links = DbLinks::load(&tx)?;
            let current = notes::current(&tx, id)?;
            let base_version = base_version_for(current.as_ref(), &intent.op)?;
            if matches!(intent.op, Op::NoteUpdate(_)) {
                base_content = current.as_ref().map(|c| c.content.clone());
            }
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
            base_content: base_content.as_deref(),
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
