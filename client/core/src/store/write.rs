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
//! - after an epoch change the whole cache is re-bootstrapped and every entity rebuilt, which
//!   re-applies all pending ops to the fresh data.
//!
//! [`apply_to_note`] is a pure function of `(state, op)`; it is the only place that knows what an
//! op does to a note, and it delegates every markdown edit to `vault-format` via
//! [`crate::format::edit`].

use chrono::{DateTime, NaiveDate};
use rusqlite::Connection;
use ulid::Ulid;

use crate::error::{CoreError, CoreResult};
use crate::format::edit;
use crate::store::index::Reindex;
use crate::store::notes::{self, NoteBase, NoteState};
use crate::store::{outbox, parse_ulid};
use crate::sync::model::OpPayload;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    /// New op ID.
    pub op_id: String,
    /// Server entity (note, task block, suggestion).
    pub entity_id: String,
    /// Local entity.
    pub local: LocalEntity,
    /// Payload.
    pub payload: OpPayload,
}

fn date(s: &str) -> CoreResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| CoreError::invalid("date", "format"))
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

fn with_content(state: NoteState, content: String) -> Option<NoteState> {
    Some(NoteState {
        path: state.path,
        content,
    })
}

/// What `payload` does to a note whose state is `state`. `note_id` is the note's ID and
/// `entity_id` the op's server entity (the task block ID for task ops). Pure.
pub fn apply_to_note(
    note_id: &str,
    entity_id: &str,
    state: Option<NoteState>,
    payload: &OpPayload,
) -> CoreResult<Option<NoteState>> {
    match payload {
        OpPayload::NoteCreate { path, content, .. } => {
            validate_note_path(path)?;
            Ok(Some(NoteState {
                path: path.clone(),
                content: edit::with_id(content, parse_ulid(note_id)?)?,
            }))
        }
        OpPayload::Capture {
            text,
            path,
            created,
        } => {
            let created = DateTime::parse_from_rfc3339(created)
                .map_err(|_| CoreError::invalid("created", "format"))?;
            Ok(Some(NoteState {
                path: path.clone(),
                content: edit::capture_content(parse_ulid(note_id)?, &created, text)?,
            }))
        }
        OpPayload::EntityCreate {
            kind,
            aliases,
            path,
            ..
        } => {
            validate_note_path(path)?;
            let kind = kind
                .parse::<domain::NoteKind>()
                .ok()
                .filter(|k| k.is_entity())
                .ok_or_else(|| CoreError::invalid("kind", "not_an_entity"))?;
            Ok(Some(NoteState {
                path: path.clone(),
                content: edit::entity_content(parse_ulid(note_id)?, kind, aliases)?,
            }))
        }
        OpPayload::NoteUpdate { content } => Ok(with_content(require(state)?, content.clone())),
        OpPayload::NoteMove { new_path } => {
            validate_note_path(new_path)?;
            let s = require(state)?;
            Ok(Some(NoteState {
                path: new_path.clone(),
                content: s.content,
            }))
        }
        OpPayload::NoteDelete => {
            require(state)?;
            Ok(None)
        }
        OpPayload::RelationAdd {
            rel_type, dst_link, ..
        } => {
            let s = require(state)?;
            let c = edit::add_relation(&s.content, rel_type, dst_link)?;
            Ok(with_content(s, c))
        }
        OpPayload::RelationRemove {
            rel_type, dst_link, ..
        } => {
            let s = require(state)?;
            let c = edit::remove_relation(&s.content, rel_type, dst_link)?;
            Ok(with_content(s, c))
        }
        OpPayload::RelationRetype {
            rel_type,
            new_type,
            dst_link,
            ..
        } => {
            let s = require(state)?;
            let c = edit::retype_relation(&s.content, rel_type, new_type, dst_link)?;
            Ok(with_content(s, c))
        }
        OpPayload::TaskCreate { line, .. } => match state {
            Some(s) => {
                let c = edit::append_task(&s.content, line);
                Ok(with_content(s, c))
            }
            // The task home does not exist yet: create it (§6.11 `tasks/Tasks.md`).
            None => Ok(Some(NoteState {
                path: TASK_HOME.to_owned(),
                content: edit::with_id(&edit::append_task("", line), parse_ulid(note_id)?)?,
            })),
        },
        OpPayload::TaskUpdate { line } => {
            let s = require(state)?;
            let c = edit::update_task(&s.content, entity_id, line)?;
            Ok(with_content(s, c))
        }
        OpPayload::TaskComplete {
            done_date,
            next_task_id,
        } => {
            let s = require(state)?;
            let c = edit::complete_task(
                &s.content,
                entity_id,
                date(done_date)?,
                next_task_id.as_deref(),
            )?;
            Ok(with_content(s, c))
        }
        OpPayload::TaskCancel { date: d } => {
            let s = require(state)?;
            let c = edit::cancel_task(&s.content, entity_id, date(d)?)?;
            Ok(with_content(s, c))
        }
        OpPayload::TaskReopen => {
            let s = require(state)?;
            let c = edit::reopen_task(&s.content, entity_id)?;
            Ok(with_content(s, c))
        }
        OpPayload::TaskDelete => {
            let s = require(state)?;
            let c = edit::delete_task(&s.content, entity_id)?;
            Ok(with_content(s, c))
        }
        OpPayload::SuggestionAccept | OpPayload::SuggestionReject | OpPayload::RelinkRequest => {
            Ok(state)
        }
    }
}

/// Default home of tasks created without a note (§6.11).
pub const TASK_HOME: &str = "tasks/Tasks.md";

/// What a suggestion op does to a suggestion's status. Pure.
pub fn apply_to_suggestion(status: Option<String>, payload: &OpPayload) -> Option<String> {
    match payload {
        OpPayload::SuggestionAccept => status.map(|_| "accepted".to_owned()),
        OpPayload::SuggestionReject => status.map(|_| "rejected".to_owned()),
        _ => status,
    }
}

/// Recomputes a note's current state from its base and its live ops; records it in `re`.
pub fn rebuild_note(conn: &Connection, id: &str, now: &str, re: &mut Reindex) -> CoreResult<()> {
    let mut state = notes::base(conn, id)?.map(|b| NoteState {
        path: b.path,
        content: b.content,
    });
    for op in outbox::live_for(conn, &LocalEntity::Note(id.to_owned()).key())? {
        // An op that no longer applies (e.g. its task line vanished remotely) is left for the
        // server to decide; the local view simply doesn't show it.
        if let Ok(next) = apply_to_note(id, &op.entity_id, state.clone(), &op.payload) {
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
        status = apply_to_suggestion(status, &op.payload);
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
pub fn rebuild(conn: &Connection, local: &LocalEntity, now: &str, re: &mut Reindex) -> CoreResult<()> {
    match local {
        LocalEntity::Note(id) => rebuild_note(conn, id, now, re),
        LocalEntity::Suggestion(id) => rebuild_suggestion(conn, id, re),
        LocalEntity::Nothing => Ok(()),
    }
}

/// Runs an intent: optimistic local change + outbox op + index, in one transaction. Returns
/// the view topics that changed. On error nothing is written.
pub fn execute(conn: &mut Connection, intent: &Intent, now: &str) -> CoreResult<Topics> {
    let tx = conn.transaction()?;
    let mut re = Reindex::new();
    let base_version = match &intent.local {
        LocalEntity::Note(id) => {
            let current = notes::current(&tx, id)?;
            let next = apply_to_note(id, &intent.entity_id, current, &intent.payload)?;
            let base_version = notes::base(&tx, id)?.map(|b| b.version);
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
            let Some(next) = apply_to_suggestion(status, &intent.payload) else {
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
            op_id: &intent.op_id,
            entity_id: &intent.entity_id,
            local_entity: &intent.local.key(),
            base_version: base_version.as_deref(),
            payload: &intent.payload,
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

/// The server base after an op was applied by the server: the op folded over the previous
/// base. `None` when the op's effect on the base cannot be computed (the next pull brings the
/// server's state anyway).
pub fn base_after_applied(
    note_id: &str,
    entity_id: &str,
    previous: Option<NoteBase>,
    payload: &OpPayload,
    new_version: &str,
) -> Option<Option<NoteBase>> {
    let state = previous.map(|b| NoteState {
        path: b.path,
        content: b.content,
    });
    let next = apply_to_note(note_id, entity_id, state, payload).ok()?;
    Some(next.map(|s| NoteBase {
        path: s.path,
        content: s.content,
        version: new_version.to_owned(),
    }))
}

/// `(kind, title)` of the item a create op creates (duplicate prompts).
pub fn describe_create(conn: &Connection, op: &outbox::OutboxOp) -> CoreResult<(String, String)> {
    let _ = conn;
    Ok(match &op.payload {
        OpPayload::NoteCreate { path, .. } => ("note".to_owned(), crate::format::title_of(path)),
        OpPayload::Capture { text, .. } => {
            ("capture".to_owned(), text.lines().next().unwrap_or_default().chars().take(80).collect())
        }
        OpPayload::EntityCreate { kind, name, .. } => (kind.clone(), name.clone()),
        OpPayload::TaskCreate { line, .. } => (
            "task".to_owned(),
            vault_format::tasks::TaskLine::parse(line)
                .map(|t| t.description().to_owned())
                .unwrap_or_default(),
        ),
        other => (other.kind().as_str().to_owned(), String::new()),
    })
}

/// A fresh op ID string.
pub fn op_id(id: Ulid) -> String {
    id.to_string()
}
