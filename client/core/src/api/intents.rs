//! User intents (§12.3): each one changes the local cache immediately, queues an outbox op and
//! wakes the sync loop. Returns the new item's ID or the op ID.

use chrono::NaiveDateTime;

use super::runtime::{self, core};
use super::{lift, lift_async};
use crate::error::CoreError;
use crate::session::{NewTask, Session, TaskEdit};
use crate::sync::engine::Trigger;
use crate::view::model::CoreFailure;
use crate::view::model::{
    AdminUserItem, AskScope, ConflictResolution, CreateOutcome, CustodyDraft, DocumentDraft,
    DuplicateChoice, ExportSummary, ImportSummary, LinkOrCreateChoice, MentionEdit, NewUserRequest,
    PlaceDraft, SuggestionEdits, TaskDraft, TaskPatch,
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

/// Saves a note's full content made against `base_version` (`NoteView::content_version`
/// when the editor loaded it; `None` saves unconditionally). A stale base is 3-way merged
/// with the changes made since, or refused with `stale_edit`.
pub fn update_note(
    id: String,
    content: String,
    base_version: Option<String>,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.update_note_at(&id, &content, base_version.as_deref())))
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

/// Adds a reminder to a task (local wall-clock time).
pub fn add_reminder(task_id: String, at: NaiveDateTime) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.add_reminder(&task_id, at)))
}

/// Removes a reminder (`ReminderItem::local_at`).
pub fn remove_reminder(task_id: String, at: NaiveDateTime) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.remove_reminder(&task_id, at)))
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

/// Inserts an `@mention` (UTF-16 range `start..end` of the typed `@query`) as a link and adds
/// the entity to `people:`/`companies:`: the new content and caret, saved with `update_note`.
pub fn insert_mention(
    note_id: String,
    content: String,
    start: u32,
    end: u32,
    entity_id: String,
) -> Result<MentionEdit, CoreFailure> {
    lift(|| {
        let _ = &note_id;
        core()?
            .session()?
            .read(|c, _| crate::view::extra::insert_mention(c, &content, start, end, &entity_id))
    })
}

/// Pins a note to the sidebar (this device) or unpins it.
pub fn pin_note(id: String, pinned: bool) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.pin_note(&id, pinned))
}

/// Accepts what the AI proposed for a capture (every suggestion that needs no choice).
pub fn accept_capture(note_id: String) -> Result<Vec<String>, CoreFailure> {
    lift(|| with(|s| s.accept_capture(&note_id)))
}

/// Rejects every pending suggestion of a capture.
pub fn reject_capture(note_id: String) -> Result<Vec<String>, CoreFailure> {
    lift(|| with(|s| s.reject_capture(&note_id)))
}

/// Accepts the listed captures that are ready (bulk bar).
pub fn accept_captures(note_ids: Vec<String>) -> Result<Vec<String>, CoreFailure> {
    lift(|| with(|s| s.accept_captures(&note_ids)))
}

/// "Accept all ready".
pub fn accept_all_ready() -> Result<Vec<String>, CoreFailure> {
    lift(|| with(Session::accept_all_ready))
}

/// Accepts a proposal with the user's edits.
pub fn accept_suggestion_with(id: String, edits: SuggestionEdits) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.accept_suggestion_with(&id, edits)))
}

/// "Who is “بابا”?": link to an existing entity or create one (the mention becomes an alias).
pub fn resolve_link_or_create(
    id: String,
    choice: LinkOrCreateChoice,
) -> Result<CreateOutcome, CoreFailure> {
    lift(|| with(|s| s.resolve_link_or_create(&id, &choice)))
}

/// Picks the document of an ambiguous custody suggestion.
pub fn accept_suggestion_choice(id: String, document_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.accept_suggestion_choice(&id, &document_id)))
}

/// Undo on a suggestion (rejects a pending one; `not_available` for an AI change applied
/// automatically until the server's undo endpoint exists).
pub fn undo_suggestion(id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.undo_suggestion(&id)))
}

/// "Looks right" on an AI change applied automatically.
pub fn acknowledge_suggestion(id: String) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.acknowledge_suggestion(&id))
}

/// A capture flagged as a duplicate: keep it or discard it.
pub fn resolve_capture_duplicate(
    id: String,
    choice: DuplicateChoice,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.resolve_capture_duplicate(&id, choice)))
}

/// Replies in a suggestion's thread.
pub fn reply_to_suggestion(id: String, text: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.reply_to_suggestion(&id, &text)))
}

/// Creates a document (duplicate-checked unless `force`).
pub fn create_document(draft: DocumentDraft, force: bool) -> Result<CreateOutcome, CoreFailure> {
    lift(|| with(|s| s.create_document(&draft, force)))
}

/// Creates a place (duplicate-checked unless `force`).
pub fn create_place(draft: PlaceDraft, force: bool) -> Result<CreateOutcome, CoreFailure> {
    lift(|| with(|s| s.create_place(&draft, force)))
}

/// Merges entity `source_id` into `into_id`.
pub fn merge_entities(source_id: String, into_id: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.merge_entities(&source_id, &into_id)))
}

/// Points a relation at another target (D13 "this Ahmed is Ahmed Fathy").
pub fn repoint_relation(
    src_id: String,
    dst_id: String,
    rel_type: String,
    new_dst_id: String,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.repoint_relation(&src_id, &dst_id, &rel_type, &new_dst_id)))
}

/// Rejects a relation (an AI edge is recorded as rejected and never re-proposed; undo with
/// `add_relation`).
pub fn reject_relation(
    src_id: String,
    dst_id: String,
    rel_type: String,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.reject_relation(&src_id, &dst_id, &rel_type)))
}

/// Replaces the user-owned `## Notes` section of an entity, document or place.
pub fn update_user_notes(id: String, text: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.update_user_notes(&id, &text)))
}

/// Sets a property of an entity, document or place.
pub fn set_property(id: String, key: String, value: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.set_property(&id, &key, &value)))
}

/// Sets a property to a list of values (several phone numbers, `aliases`, `tags`); the list
/// replaces the whole value and an empty list removes the key. Relation lists are refused
/// (use the relation intents).
pub fn set_property_values(
    id: String,
    key: String,
    values: Vec<String>,
) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.set_property_values(&id, &key, &values)))
}

/// Removes a property.
pub fn remove_property(id: String, key: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.remove_property(&id, &key)))
}

/// Adds an alias.
pub fn add_alias(id: String, alias: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.add_alias(&id, &alias)))
}

/// Removes an alias.
pub fn remove_alias(id: String, alias: String) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.remove_alias(&id, &alias)))
}

/// Records a custody event of a document ("Record a move"); a draft without a date is
/// recorded today in the account's time zone.
pub fn record_custody(document_id: String, draft: CustodyDraft) -> Result<String, CoreFailure> {
    lift(|| with(|s| s.record_custody(&document_id, &draft)))
}

/// Time used for date-only reminders (`HH:MM`).
pub fn set_default_reminder_time(time: String) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.set_default_reminder_time(&time))
}

/// Quiet hours (`HH:MM`).
pub fn set_quiet_hours(enabled: bool, from: String, until: String) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.set_quiet_hours(enabled, &from, &until))
}

/// The notification's Snooze length (minutes).
pub fn set_snooze_minutes(minutes: u32) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.set_snooze_minutes(minutes))
}

/// Re-reads devices, AI status, integrity warnings and the approval count (online).
pub async fn refresh_settings() -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.refresh_settings().await }).await
}

/// Queues the account's failed AI jobs again and re-reads the AI status; returns how many
/// were queued.
pub async fn retry_failed_jobs() -> Result<u32, CoreFailure> {
    lift_async(async { core()?.session()?.retry_failed_jobs().await }).await
}

/// Renames a device.
pub async fn rename_device(id: String, name: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.rename_device(&id, &name).await }).await
}

/// Signs another device out.
pub async fn revoke_device(id: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.revoke_device(&id).await }).await
}

/// Reminders on/off for a device ("Deliver to").
pub async fn set_device_reminders(id: String, enabled: bool) -> Result<(), CoreFailure> {
    lift_async(async {
        core()?
            .session()?
            .set_device_reminders(&id, enabled)
            .await?;
        runtime::trigger(Trigger::AfterWrite);
        Ok(())
    })
    .await
}

/// Fetches a note's history (online).
pub async fn refresh_history(note_id: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.refresh_history(&note_id).await }).await
}

/// Reverts a note to a revision (online).
pub async fn revert_note(note_id: String, commit: String) -> Result<(), CoreFailure> {
    lift_async(async {
        core()?.session()?.revert_note(&note_id, &commit).await?;
        runtime::trigger(Trigger::Manual);
        Ok(())
    })
    .await
}

/// Downloads the vault export to a file the user chose.
pub async fn export_vault(path: String) -> Result<ExportSummary, CoreFailure> {
    lift_async(async { core()?.session()?.export_vault(&path).await }).await
}

/// Imports a zip archive.
pub async fn import_vault(path: String) -> Result<ImportSummary, CoreFailure> {
    lift_async(async {
        let r = core()?.session()?.import_vault(&path).await?;
        runtime::trigger(Trigger::Manual);
        Ok(r)
    })
    .await
}

/// Admin: approves a sign-up.
pub async fn approve_user(id: String) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.approve_user(&id).await }).await
}

/// Admin: rejects a sign-up.
pub async fn reject_user(id: String) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.reject_user(&id).await }).await
}

/// Admin: sets a user's role (`admin` | `member`).
pub async fn set_user_role(id: String, role: String) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.set_user_role(&id, &role).await }).await
}

/// Admin: disables (`enabled = false`) or enables an account.
pub async fn set_user_enabled(id: String, enabled: bool) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.set_user_enabled(&id, enabled).await }).await
}

/// Admin: resets a password; returns the one-time temporary password.
pub async fn reset_password(id: String) -> Result<String, CoreFailure> {
    lift_async(async { core()?.session()?.reset_password(&id).await }).await
}

/// Admin: schedules an account's deletion (D25).
pub async fn schedule_deletion(id: String) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.schedule_deletion(&id).await }).await
}

/// Admin: cancels a scheduled deletion.
pub async fn cancel_deletion(id: String) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.cancel_deletion(&id).await }).await
}

/// Admin: creates an active account.
pub async fn create_user(request: NewUserRequest) -> Result<AdminUserItem, CoreFailure> {
    lift_async(async { core()?.session()?.create_user(request).await }).await
}

/// Ask: asks a question in `scope`; the answer streams into `watch_ask`. Returns the answer's
/// ID when it ended.
pub async fn ask(question: String, scope: AskScope) -> Result<String, CoreFailure> {
    lift_async(async {
        let session = core()?.session()?;
        // A note scope: the conversation is that note's thread.
        if scope.kind == crate::view::model::AskScopeKind::Note
            && let Some(note) = &scope.value
            && session.ask_state_note().as_deref() != Some(note.as_str())
        {
            session.open_note_thread(note)?;
        }
        let value = session.read(|c, _| crate::view::build::ask_scope_value(c, &scope))?;
        session.ask(&question, value, &scope.label).await
    })
    .await
}

/// Ask: makes the conversation about note `id` (its saved thread; "Ask about this note").
pub fn open_note_thread(id: String) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.open_note_thread(&id))
}

/// Ask: stops the streaming answer.
pub fn stop_ask() -> Result<(), CoreFailure> {
    lift(|| {
        core()?.session()?.stop_ask();
        Ok(())
    })
}

/// Ask: starts a new conversation.
pub fn new_conversation() -> Result<(), CoreFailure> {
    lift(|| {
        core()?.session()?.new_conversation();
        Ok(())
    })
}

/// Ask: saves an answer as a note (§9.5); returns the note's ID.
pub async fn save_answer_as_note(message_id: String) -> Result<String, CoreFailure> {
    lift_async(async {
        let id = core()?.session()?.save_answer_as_note(&message_id).await?;
        runtime::trigger(Trigger::Manual);
        Ok(id)
    })
    .await
}

/// Home: re-reads the AI activity feed (the server's AI decisions).
pub async fn refresh_ai_activity() -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.refresh_ai_activity().await }).await
}

/// D13: undoes an AI decision (the server reverts it and never re-proposes it).
pub async fn reject_ai_decision(decision_id: String) -> Result<(), CoreFailure> {
    lift_async(async {
        core()?.session()?.reject_ai_decision(&decision_id).await?;
        runtime::trigger(Trigger::Manual);
        Ok(())
    })
    .await
}

/// D13: points an AI decision at another entity; `hint` is the user's short explanation.
pub async fn repoint_ai_decision(
    decision_id: String,
    target_id: String,
    hint: Option<String>,
) -> Result<(), CoreFailure> {
    lift_async(async {
        core()?
            .session()?
            .repoint_ai_decision(&decision_id, &target_id, hint)
            .await?;
        runtime::trigger(Trigger::Manual);
        Ok(())
    })
    .await
}

/// D13: changes the type of an AI relation.
pub async fn retype_ai_decision(decision_id: String, rel_type: String) -> Result<(), CoreFailure> {
    lift_async(async {
        core()?
            .session()?
            .retype_ai_decision(&decision_id, &rel_type)
            .await?;
        runtime::trigger(Trigger::Manual);
        Ok(())
    })
    .await
}

/// Map: fetches the similarity edges shown by the similarity lens (online only).
pub async fn refresh_similarity() -> Result<(), CoreFailure> {
    lift_async(async { core()?.session()?.refresh_similarity().await }).await
}

/// Map: saves the arranged local map as `maps/<name>.canvas`; returns the map's path.
pub async fn save_layout(
    center_id: String,
    name: String,
    positions: Vec<crate::view::model::NodePosition>,
) -> Result<String, CoreFailure> {
    lift_async(async {
        let path = core()?
            .session()?
            .save_layout(&center_id, &name, &positions)
            .await?;
        runtime::trigger(Trigger::Manual);
        Ok(path)
    })
    .await
}
