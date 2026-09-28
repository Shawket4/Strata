//! Pure op application rules shared by the server (on push) and the client core (optimistic
//! local apply, §12.3), so both produce the same bytes.
//!
//! Only rules that need no I/O live here. The caller resolves IDs to wikilink targets
//! (it owns the path index) and supplies dates and citations.

use chrono::{NaiveDate, NaiveDateTime};
use ulid::Ulid;
use vault_format::blocks::is_valid_block_id;
use vault_format::custody::{self, CustodyEvent, CustodyState};
use vault_format::frontmatter::ValueShape;
use vault_format::sections::sections;
use vault_format::tasks::{self, DateKind, Priority, Reminder, TaskError, TaskLine, TaskSpec};
use vault_format::{Document, Frontmatter, FrontmatterError, KnownKey, LineEnding, RelationKey};

use crate::Version;
use crate::ops::{DocumentCustody, EntityPatch, Op, TaskCreate, TaskUpdate};

/// The note a `task.create` without `note_id` goes to (PLAN §6.11).
pub use vault_format::tasks::DEFAULT_TASK_NOTE;

/// Why an op cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    /// The frontmatter refused the edit.
    #[error(transparent)]
    Frontmatter(#[from] FrontmatterError),
    /// The relation to retype does not exist.
    #[error("no `{relation}` relation to `{target}`")]
    RelationMissing {
        /// Type.
        relation: RelationKey,
        /// Target.
        target: String,
    },
    /// `id`/`kind` never change.
    #[error("`{0}` cannot be changed")]
    ImmutableKey(String),
    /// The key is changed through another op (aliases, relations, custody fields).
    #[error("`{0}` is not changed with a patch")]
    NotPatchable(String),
    /// A referenced ID has no wikilink target.
    #[error("unknown entity {0}")]
    UnknownEntity(Ulid),
    /// The `## Custody` section has lines that are not events; rewriting it would lose them.
    #[error("the custody section has {0} unparseable line(s)")]
    MalformedCustody(usize),
    /// The custody event is invalid (e.g. missing its primary argument).
    #[error("invalid custody event: {0}")]
    InvalidCustodyEvent(String),
    /// No task line with this block ID.
    #[error("no task `{0}`")]
    TaskNotFound(String),
    /// The task edit failed.
    #[error(transparent)]
    Task(#[from] TaskError),
    /// Completing a recurring task needs the next occurrence's block ID.
    #[error("recurring task `{0}` needs next_id")]
    MissingNextId(String),
    /// The op is not handled by this function.
    #[error("`{0}` is not a task op")]
    NotATaskOp(String),
    /// `task.create` with a block ID the note already has.
    #[error("task `{0}` already exists")]
    TaskExists(String),
    /// `task.create` whose fields would not read back from the written line (e.g. a
    /// description containing a line break or a Tasks signifier).
    #[error("task `{0}` cannot be written as a task line")]
    InvalidTask(String),
}

// ---------------------------------------------------------------------------------------
// Relations

/// `relation.add`: adds `[[target]]` to the relation list on the source note. Returns
/// whether it was added (a link to the same target already there is kept once).
pub fn relation_add(
    fm: &mut Frontmatter,
    relation: RelationKey,
    target: &str,
) -> Result<bool, ApplyError> {
    Ok(fm.add_relation_link(relation, target)?)
}

/// Removes every link to `target` from the list, and the key itself once it is empty
/// ("empty keys may be omitted", §6.4). Returns how many links were removed.
fn remove_link(
    fm: &mut Frontmatter,
    relation: RelationKey,
    target: &str,
) -> Result<usize, ApplyError> {
    let removed = fm.remove_relation_link(relation, target)?;
    if removed > 0 && fm.relation(relation).is_empty() {
        fm.remove(relation.as_str())?;
    }
    Ok(removed)
}

/// `relation.remove`: removes every link to `target` from the list (and the key once it is
/// empty). Returns whether any was removed (removing a missing relation is a no-op, so
/// replays are harmless).
pub fn relation_remove(
    fm: &mut Frontmatter,
    relation: RelationKey,
    target: &str,
) -> Result<bool, ApplyError> {
    Ok(remove_link(fm, relation, target)? > 0)
}

/// `relation.retype`: moves the link to `target` from `from` to `to`.
pub fn relation_retype(
    fm: &mut Frontmatter,
    from: RelationKey,
    to: RelationKey,
    target: &str,
) -> Result<(), ApplyError> {
    if from == to {
        return Ok(());
    }
    if remove_link(fm, from, target)? == 0 {
        return Err(ApplyError::RelationMissing {
            relation: from,
            target: target.to_owned(),
        });
    }
    fm.add_relation_link(to, target)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Entity / document / place patches

fn check_patchable(key: &str) -> Result<(), ApplyError> {
    match KnownKey::from_name(key) {
        Some(KnownKey::Id | KnownKey::Kind) => Err(ApplyError::ImmutableKey(key.to_owned())),
        Some(
            KnownKey::Location
            | KnownKey::Holder
            | KnownKey::LastHolder
            | KnownKey::Status
            | KnownKey::Aliases,
        ) => Err(ApplyError::NotPatchable(key.to_owned())),
        Some(k) if matches!(k.shape(), ValueShape::List | ValueShape::LinkList) => {
            Err(ApplyError::NotPatchable(key.to_owned()))
        }
        _ => Ok(()),
    }
}

/// Whether `key` may be set to a list by `set_lists`: `aliases`, `tags` and any scalar user
/// field (several phone numbers); never `id`/`kind`, relation lists or the custody fields.
fn check_list_patchable(key: &str) -> Result<(), ApplyError> {
    match KnownKey::from_name(key) {
        Some(KnownKey::Aliases | KnownKey::Tags) => Ok(()),
        Some(KnownKey::Relation(_)) => Err(ApplyError::NotPatchable(key.to_owned())),
        _ => check_patchable(key),
    }
}

/// A list value as `set_lists` writes it: items trimmed, empty items and repeats dropped
/// (first occurrence kept); tags also lose a leading `#`.
pub fn clean_list_value(key: &str, values: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let mut t = v.trim();
        if key == KnownKey::Tags.as_str() {
            t = t.trim_start_matches('#').trim();
        }
        if !t.is_empty() && !out.iter().any(|o| o == t) {
            out.push(t.to_owned());
        }
    }
    out
}

/// `entity.patch` / `document.patch` / `place.patch`: sets and removes scalar user fields,
/// sets list values (`set_lists`: each list replaces the key's value; an empty list removes
/// it) and adds/removes aliases (kept in order, each once). `id` and `kind` are immutable;
/// relation lists change through relation ops and `location`/`holder`/`last-holder`/`status`
/// only through custody events (§6.12). Nothing is changed when any key is refused.
pub fn entity_patch(fm: &mut Frontmatter, patch: &EntityPatch) -> Result<(), ApplyError> {
    for key in patch.set.keys().chain(&patch.unset) {
        check_patchable(key)?;
    }
    for key in patch.set_lists.keys() {
        check_list_patchable(key)?;
    }
    let mut next = fm.clone();
    for (key, value) in &patch.set {
        next.set(key, vault_format::PropertyValue::Text(value.clone()))?;
    }
    for key in &patch.unset {
        next.remove(key)?;
    }
    for (key, values) in &patch.set_lists {
        let values = clean_list_value(key, values);
        match KnownKey::from_name(key) {
            _ if values.is_empty() => {
                next.remove(key)?;
            }
            Some(k @ (KnownKey::Aliases | KnownKey::Tags)) => next.set_list(k, values)?,
            _ => next.set(key, vault_format::PropertyValue::List(values))?,
        }
    }
    if !patch.add_aliases.is_empty() || !patch.remove_aliases.is_empty() {
        let mut aliases = next.aliases();
        aliases.retain(|a| !patch.remove_aliases.contains(a));
        for a in &patch.add_aliases {
            if !aliases.contains(a) {
                aliases.push(a.clone());
            }
        }
        if aliases.is_empty() {
            next.remove_key(KnownKey::Aliases)?;
        } else {
            next.set_list(KnownKey::Aliases, aliases)?;
        }
    }
    *fm = next;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Custody

/// Builds the custody event of a `document.custody` op. `link` resolves an entity ID to its
/// wikilink (`[[Safe — Nasr City office]]`); `citations` are the notes/blocks stating it.
pub fn custody_event(
    op: &DocumentCustody,
    link: impl Fn(Ulid) -> Option<String>,
    citations: Vec<String>,
) -> Result<CustodyEvent, ApplyError> {
    let resolve = |id: Option<Ulid>| -> Result<Option<String>, ApplyError> {
        id.map(|id| link(id).ok_or(ApplyError::UnknownEntity(id)))
            .transpose()
    };
    let event = CustodyEvent {
        date: op.at,
        kind: op.event,
        place: resolve(op.place_id)?,
        person: resolve(op.person_id)?,
        counterparty: resolve(op.counterparty_id)?,
        citations,
        note: op.note.as_deref().and_then(custody::clean_note),
    };
    // Round-trip through the line format so an event that the section parser would reject
    // (or read back differently, e.g. a note of links only) is refused now rather than written.
    let back = CustodyEvent::parse(&event.to_line())
        .map_err(|e| ApplyError::InvalidCustodyEvent(e.to_string()))?;
    if back == event {
        Ok(back)
    } else {
        Err(ApplyError::InvalidCustodyEvent(
            "the event does not read back unchanged".to_owned(),
        ))
    }
}

const CUSTODY: &str = "Custody";

/// Records a custody event in a document note: the `## Custody` section is rewritten with
/// the event added (newest first; a new event on the same date counts as the newest), and
/// `location`, `holder`, `last-holder` and `status` are recomputed from all events. A missing
/// section is created before `## Notes` (or at the end). Returns the new state.
pub fn record_custody(doc: &mut Document, event: CustodyEvent) -> Result<CustodyState, ApplyError> {
    let body = doc.body().to_owned();
    let section = sections(&body)
        .into_iter()
        .find(|s| s.level == 2 && s.title == CUSTODY);
    let new_body = if let Some(s) = section {
        let content = &body[s.own_content_span.clone()];
        let (mut events, bad) = custody::parse_section(content);
        if !bad.is_empty() {
            return Err(ApplyError::MalformedCustody(bad.len()));
        }
        events.insert(0, event);
        let trailing = &content[content.trim_end_matches(['\n', '\r']).len()..];
        let trailing = if trailing.is_empty() { "\n" } else { trailing };
        format!(
            "{}{}{}{}",
            &body[..s.own_content_span.start],
            custody::render_section(&events),
            trailing,
            &body[s.own_content_span.end..]
        )
    } else {
        let block = format!("## {CUSTODY}\n{}\n", custody::render_section(&[event]));
        let notes = sections(&body)
            .into_iter()
            .find(|s| s.level == 2 && s.title == "Notes");
        match notes {
            Some(n) => {
                let at = body[..n.heading_span.start]
                    .rfind('\n')
                    .map_or(0, |i| i + 1);
                format!("{}{block}\n{}", &body[..at], &body[at..])
            }
            None if body.is_empty() => block,
            None if body.ends_with("\n\n") => format!("{body}{block}"),
            None if body.ends_with('\n') => format!("{body}\n{block}"),
            None => format!("{body}\n\n{block}"),
        }
    };
    let section = sections(&new_body)
        .into_iter()
        .find(|s| s.level == 2 && s.title == CUSTODY)
        .map(|s| new_body[s.own_content_span].to_owned())
        .unwrap_or_default();
    let (events, _) = custody::parse_section(&section);
    let state = CustodyState::derive(&events).ok_or(ApplyError::MalformedCustody(0))?;
    state.write_to(doc.frontmatter_mut())?;
    doc.set_body(new_body);
    Ok(state)
}

// ---------------------------------------------------------------------------------------
// Tasks

/// The version of a task line (`base_version` of task ops): the hash of the line without its
/// terminator.
pub fn task_line_version(line: &str) -> Version {
    Version::of_text(line.trim_end_matches(['\n', '\r']))
}

/// Finds the task line with `block_id`; returns its span (without terminator).
pub fn find_task(body: &str, block_id: &str) -> Option<(std::ops::Range<usize>, TaskLine)> {
    tasks::extract_tasks(body)
        .into_iter()
        .find(|t| t.task.block_id() == Some(block_id))
        .map(|t| (t.line_span, t.task))
}

fn reminders(at: &[NaiveDateTime]) -> Vec<Reminder> {
    at.iter()
        .map(|dt| Reminder {
            date: dt.date(),
            time: Some(dt.time()),
        })
        .collect()
}

fn update_line(task: &TaskLine, u: &TaskUpdate) -> TaskLine {
    let mut t = task.clone();
    if let Some(text) = &u.text {
        let mut spec = t.to_spec();
        spec.description.clone_from(text);
        t = TaskLine::parse(&spec.render()).unwrap_or(t);
    }
    for (kind, value) in [
        (DateKind::Due, u.due),
        (DateKind::Scheduled, u.scheduled),
        (DateKind::Start, u.start),
    ] {
        if let Some(v) = value {
            t = t.with_date(kind, v);
        }
    }
    if let Some(r) = &u.recurrence {
        t = t.with_recurrence(r.as_deref());
    }
    if let Some(r) = &u.reminders {
        t = t.with_reminders(&reminders(r));
    }
    if let Some(p) = u.priority {
        t = t.with_priority(p);
    }
    t
}

/// The canonical line of a `task.create` (`TaskSpec::render`: the description trimmed,
/// reminders, Tasks fields in plugin order, `^<id>`). Fails when the ID is not a valid block
/// ID, when the text has a line break, or when the line would not read back as exactly the
/// op's fields.
pub fn task_create_line(op: &TaskCreate) -> Result<String, ApplyError> {
    if !op.id.starts_with("t-") || !is_valid_block_id(&op.id) {
        return Err(TaskError::InvalidBlockId(op.id.clone()).into());
    }
    if op.text.contains(['\n', '\r']) {
        return Err(ApplyError::InvalidTask(op.id.clone()));
    }
    let spec = TaskSpec {
        description: op.text.trim().to_owned(),
        priority: op.priority.filter(|p| *p != Priority::Normal),
        recurrence: op.recurrence.clone(),
        start: op.start,
        scheduled: op.scheduled,
        due: op.due,
        reminders: reminders(&op.reminders),
        block_id: Some(op.id.clone()),
        ..TaskSpec::default()
    };
    let line = spec.render();
    match TaskLine::parse(&line) {
        Some(t) if !line.contains(['\n', '\r']) && t.to_spec() == spec => Ok(line),
        _ => Err(ApplyError::InvalidTask(op.id.clone())),
    }
}

/// `task.create`: writes the new task line into its home note's body and returns the new
/// body. Without `note_id` the home is [`DEFAULT_TASK_NOTE`] (pass `""` when it does not
/// exist yet): the line goes under the `## <Month> <YYYY>` heading of `created` (the
/// creation date in the user's time zone), which is added in chronological position when
/// missing (`tasks::insert_under_month`). With `note_id` the line is appended at the end of
/// that note's body. New lines use `eol` (the home note's line ending); every other byte is
/// kept. A block ID the body already has is refused.
pub fn apply_task_create(
    body: &str,
    op: &TaskCreate,
    created: NaiveDate,
    eol: LineEnding,
) -> Result<String, ApplyError> {
    let line = task_create_line(op)?;
    if find_task(body, &op.id).is_some() {
        return Err(ApplyError::TaskExists(op.id.clone()));
    }
    if op.note_id.is_none() {
        return Ok(tasks::insert_under_month(body, created, &line, eol)?);
    }
    let eol = eol.as_str();
    let sep = if body.is_empty() || body.ends_with('\n') {
        ""
    } else {
        eol
    };
    Ok(format!("{body}{sep}{line}{eol}"))
}

/// Applies a task op (`task.update/complete/cancel/reopen/delete`) to a note body and
/// returns the new body (`task.create` is [`apply_task_create`]). Completing a recurring task writes the next occurrence (block ID
/// `next_id`) directly above the completed line, as the Tasks plugin does.
pub fn apply_task_op(body: &str, op: &Op) -> Result<String, ApplyError> {
    let id = match op {
        Op::TaskUpdate(p) => &p.id,
        Op::TaskComplete(p) => &p.id,
        Op::TaskCancel(p) => &p.id,
        Op::TaskReopen(p) | Op::TaskDelete(p) => &p.id,
        other => return Err(ApplyError::NotATaskOp(other.kind().to_string())),
    };
    let (span, task) = find_task(body, id).ok_or_else(|| ApplyError::TaskNotFound(id.clone()))?;
    let line = &body[span.clone()];
    let rest = &body[span.end..];
    let eol = if rest.starts_with("\r\n") {
        "\r\n"
    } else if rest.starts_with('\n') {
        "\n"
    } else {
        ""
    };
    let replacement = match op {
        Op::TaskUpdate(u) => update_line(&task, u).as_str().to_owned(),
        Op::TaskComplete(c) if task.recurrence_text().is_some() => {
            let next_id = c
                .next_id
                .as_deref()
                .ok_or_else(|| ApplyError::MissingNextId(id.clone()))?;
            let [next, done] = tasks::complete_recurring(line, c.done, next_id)?;
            format!("{next}{}{done}", if eol.is_empty() { "\n" } else { eol })
        }
        Op::TaskComplete(c) => tasks::complete(line, c.done)?,
        Op::TaskCancel(c) => tasks::cancel(line, c.date)?,
        Op::TaskReopen(_) => tasks::reopen(line)?,
        _ => {
            // Delete: the line and its terminator.
            return Ok(format!("{}{}", &body[..span.start], &rest[eol.len()..]));
        }
    };
    Ok(format!("{}{replacement}{rest}", &body[..span.start]))
}
