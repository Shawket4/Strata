//! User intents (§12.3): each one changes the local cache immediately, queues an outbox op and
//! wakes the sync loop. Returns the new item's ID or the op ID.

use super::lift;
use super::runtime::{self, core};
use crate::error::CoreError;
use crate::session::{NewTask, Session, TaskEdit};
use crate::sync::engine::Trigger;
use crate::view::model::CoreFailure;
use crate::view::model::{
    ConflictResolution, CreateOutcome, DuplicateChoice, TaskDraft, TaskPatch,
};

fn with<T>(f: impl FnOnce(&Session) -> Result<T, CoreError>) -> Result<T, CoreError> {
    let session = core()?.session()?;
    let r = f(&session)?;
    runtime::trigger(Trigger::AfterWrite);
    Ok(r)
}

fn priority(p: Option<&str>) -> Result<Option<domain::Priority>, CoreError> {
    p.map(|s| {
        s.parse::<domain::Priority>()
            .map_err(|_| CoreError::invalid("priority", "unknown"))
    })
    .transpose()
}

/// The core's task draft from the editor's.
pub(crate) fn new_task(d: TaskDraft) -> Result<NewTask, CoreError> {
    Ok(NewTask {
        priority: priority(d.priority.as_deref())?.filter(|p| *p != domain::Priority::Normal),
        note_id: d.note_id,
        description: d.description,
        due: d.due,
        scheduled: d.scheduled,
        recurrence: d.recurrence,
        reminders: d.reminders,
    })
}

/// The core's task edit from the editor's patch.
pub(crate) fn task_edit(p: TaskPatch) -> Result<TaskEdit, CoreError> {
    Ok(TaskEdit {
        text: p.text,
        due: if p.clear_due {
            Some(None)
        } else {
            p.due.map(Some)
        },
        scheduled: None,
        recurrence: if p.clear_recurrence {
            Some(None)
        } else {
            p.recurrence.map(Some)
        },
        reminders: p.reminders,
        priority: match p.priority.as_deref() {
            None => None,
            Some("normal") => Some(None),
            Some(s) => Some(priority(Some(s))?),
        },
    })
}

/// Captures text into the inbox (never refused).
pub fn capture(text: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.capture(&text)))
}

/// Creates a note (duplicate-checked unless `force`).
pub fn create_note(
    path: String,
    content: String,
    force: bool,
) -> Result<CreateOutcome, CoreFailure> {
    lift(|| with(|s| s.create_note(&path, &content, force)))
}

/// Saves a note's content.
pub fn update_note(id: String, content: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.update_note(&id, &content)))
}

/// Moves/renames a note.
pub fn move_note(id: String, new_path: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.move_note(&id, &new_path)))
}

/// Deletes a note.
pub fn delete_note(id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.delete_note(&id)))
}

/// Creates a person, company or concept (`kind` = `person` | `company` | `concept`).
pub fn create_entity(
    kind: String,
    name: String,
    aliases: Vec<String>,
    force: bool,
) -> Result<CreateOutcome, CoreFailure> {
    lift(|| {
        let kind = kind
            .parse::<domain::NoteKind>()
            .map_err(|_| CoreError::invalid("kind", "unknown"))?;
        with(|s| s.create_entity(kind, &name, &aliases, force))
    })
}

/// Adds a relation.
pub fn add_relation(
    src_id: String,
    dst_id: String,
    rel_type: String,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.add_relation(&src_id, &dst_id, &rel_type)))
}

/// Removes a relation.
pub fn remove_relation(
    src_id: String,
    dst_id: String,
    rel_type: String,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.remove_relation(&src_id, &dst_id, &rel_type)))
}

/// Changes a relation's type.
pub fn retype_relation(
    src_id: String,
    dst_id: String,
    rel_type: String,
    new_type: String,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.retype_relation(&src_id, &dst_id, &rel_type, &new_type)))
}

/// Accepts a suggestion.
pub fn accept_suggestion(id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.accept_suggestion(&id)))
}

/// Rejects a suggestion.
pub fn reject_suggestion(id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.reject_suggestion(&id)))
}

/// Queues a relink request.
pub fn request_relink(note_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.request_relink(&note_id)))
}

/// Creates a task (duplicate-checked unless `force`).
pub fn create_task(draft: TaskDraft, force: bool) -> Result<CreateOutcome, CoreFailure> {
    lift(|| {
        let t = new_task(draft)?;
        with(|s| s.create_task(&t, force))
    })
}

/// Edits a task's fields.
pub fn update_task(task_id: String, patch: TaskPatch) -> Result<String, CoreFailure> {
    lift(|| {
        let e = task_edit(patch)?;
        with(|s| s.update_task(&task_id, &e))
    })
}

/// Completes a task (a recurring one gets its next occurrence).
pub fn complete_task(task_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.complete_task(&task_id)))
}

/// Cancels a task.
pub fn cancel_task(task_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.cancel_task(&task_id)))
}

/// Reopens a task.
pub fn reopen_task(task_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.reopen_task(&task_id)))
}

/// Deletes a task.
pub fn delete_task(task_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.delete_task(&task_id)))
}

/// Resolves a sync conflict (D19).
pub fn resolve_conflict(op_id: String, resolution: ConflictResolution) -> Result<(), CoreFailure> {
    lift(|| with(|s| s.resolve_conflict(&op_id, resolution)))
}

/// Answers an "Already exists" prompt from a push.
pub fn resolve_duplicate(op_id: String, choice: DuplicateChoice) -> Result<(), CoreFailure> {
    lift(|| with(|s| s.resolve_duplicate(&op_id, choice)))
}

/// Dismisses a rolled-back op's notice.
pub fn dismiss_rejection(op_id: String) -> Result<(), CoreFailure> {
    lift(|| with(|s| s.dismiss_rejection(&op_id)))
}

/// Reminders on/off for this device.
pub fn set_reminders_enabled(enabled: bool) -> Result<(), CoreFailure> {
    lift(|| with(|s| s.set_reminders_enabled(enabled)))
}
