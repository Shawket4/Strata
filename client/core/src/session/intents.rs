//! Intents (§12.3): each user action becomes one `sync-model` op with client-generated IDs and
//! dates, is applied optimistically and queued. IDs come from the injected generator; creation
//! times are the injected clock's instant (UTC; the server writes them as `created`/`updated`
//! however late the op arrives) and dates are in the user's timezone, so the op carries
//! everything the server needs to apply it identically. Creates first run the offline
//! duplicate check (§9.7).

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use sync_model::ops as sm;
use ulid::Ulid;
use vault_format::RelationKey;

use super::Session;
use crate::error::{CoreError, CoreResult};
use crate::ids::task_block_id;
use crate::store::write::{Intent, LocalEntity, TASK_HOME, note_of_task};
use crate::store::{conflicts, notes, settings};
use crate::sync::apply;
use crate::sync::model::Op;
use crate::view::Topics;
use crate::view::model::{
    CandidateItem, ConflictResolution, CreateOutcome, DuplicateChoice, NotificationAction,
    NotificationActionKind, ResolutionKind,
};

/// A task to create.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewTask {
    /// The note to add it to (`None`: `tasks/Tasks.md`).
    pub note_id: Option<String>,
    /// Description (may contain wikilinks).
    pub description: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🔁 phrase.
    pub recurrence: Option<String>,
    /// Reminders (local wall-clock times).
    pub reminders: Vec<NaiveDateTime>,
    /// Priority.
    pub priority: Option<domain::Priority>,
}

/// Field edits of a task (`None` = unchanged; `Some(None)` = clear).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskEdit {
    /// New description.
    pub text: Option<String>,
    /// 📅
    pub due: Option<Option<NaiveDate>>,
    /// ⏳
    pub scheduled: Option<Option<NaiveDate>>,
    /// 🔁
    pub recurrence: Option<Option<String>>,
    /// Reminders (replace all).
    pub reminders: Option<Vec<NaiveDateTime>>,
    /// Priority.
    pub priority: Option<Option<domain::Priority>>,
}

/// A candidate for the view.
pub fn candidate_item(c: dedupe::DuplicateCandidate) -> CandidateItem {
    CandidateItem {
        id: c.id,
        kind: c.kind.as_str().to_owned(),
        title: c.title,
        snippet: c.snippet,
        match_level: c.level.as_str().to_owned(),
        score: f64::from(c.score),
        path: None,
        reason: String::new(),
    }
}

fn ulid_of(id: &str) -> CoreResult<Ulid> {
    Ulid::from_string(id).map_err(|_| CoreError::invalid("id", "not_a_ulid"))
}

impl Session {
    fn new_ulid(&self) -> Ulid {
        self.env.ids.ulid()
    }

    fn today(&self) -> NaiveDate {
        let ctx = self.ctx();
        ctx.now.with_timezone(&ctx.tz).date_naive()
    }

    fn run(&self, local: LocalEntity, op: Op) -> CoreResult<String> {
        let op_id = self.new_ulid();
        self.execute(&Intent { op_id, local, op })?;
        Ok(op_id.to_string())
    }

    fn duplicates(&self, item: &dedupe::Item) -> CoreResult<Vec<CandidateItem>> {
        self.read(|c, _| crate::search::duplicates::check(c, item))
            .map(|v| v.into_iter().map(candidate_item).collect())
    }

    fn create(
        &self,
        id: &str,
        item: Option<dedupe::Item>,
        force: bool,
        local: LocalEntity,
        op: Op,
    ) -> CoreResult<CreateOutcome> {
        if !force && let Some(item) = item {
            let candidates = self.duplicates(&item)?;
            if !candidates.is_empty() {
                return Ok(CreateOutcome {
                    id: None,
                    candidates,
                });
            }
        }
        self.run(local, op)?;
        Ok(CreateOutcome {
            id: Some(id.to_owned()),
            candidates: Vec::new(),
        })
    }

    /// Captures text into the inbox; returns the new note ID. Never refused (§9.7): the
    /// duplicate flag rides on the server's inbox suggestion.
    pub fn capture(&self, text: &str) -> CoreResult<String> {
        if text.trim().is_empty() {
            return Err(CoreError::invalid("text", "empty"));
        }
        let id = self.new_ulid();
        self.run(
            LocalEntity::Note(id.to_string()),
            Op::Capture(sm::Capture {
                id,
                text: text.to_owned(),
                created: self.env.clock.now(),
            }),
        )?;
        Ok(id.to_string())
    }

    /// Creates a note (duplicate-checked unless `force`).
    pub fn create_note(&self, path: &str, content: &str, force: bool) -> CoreResult<CreateOutcome> {
        let id = self.new_ulid();
        let title = crate::format::title_of(path);
        self.create(
            &id.to_string(),
            Some(dedupe::Item::note(Some(&id.to_string()), &title)),
            force,
            LocalEntity::Note(id.to_string()),
            Op::NoteCreate(sm::NoteCreate {
                id,
                path: path.to_owned(),
                content: content.to_owned(),
                created: self.env.clock.now(),
                force,
            }),
        )
    }

    fn note_op(&self, id: &str, op: Op) -> CoreResult<String> {
        self.run(LocalEntity::Note(id.to_owned()), op)
    }

    /// Replaces a note's content.
    pub fn update_note(&self, id: &str, content: &str) -> CoreResult<String> {
        let ulid = ulid_of(id)?;
        self.note_op(
            id,
            Op::NoteUpdate(sm::NoteUpdate {
                id: ulid,
                content: content.to_owned(),
            }),
        )
    }

    /// Moves/renames a note.
    pub fn move_note(&self, id: &str, new_path: &str) -> CoreResult<String> {
        let ulid = ulid_of(id)?;
        self.note_op(
            id,
            Op::NoteMove(sm::NoteMove {
                id: ulid,
                new_path: new_path.to_owned(),
            }),
        )
    }

    /// Deletes a note (soft delete on the server).
    pub fn delete_note(&self, id: &str) -> CoreResult<String> {
        let ulid = ulid_of(id)?;
        self.note_op(id, Op::NoteDelete(sm::NoteRef { id: ulid }))
    }

    /// Creates a person, company or concept (duplicate-checked unless `force`).
    pub fn create_entity(
        &self,
        kind: domain::NoteKind,
        name: &str,
        aliases: &[String],
        force: bool,
    ) -> CoreResult<CreateOutcome> {
        let dkind = match kind {
            domain::NoteKind::Person => domain::DedupeKind::Person,
            domain::NoteKind::Company => domain::DedupeKind::Company,
            domain::NoteKind::Concept => domain::DedupeKind::Concept,
            _ => return Err(CoreError::invalid("kind", "not_an_entity")),
        };
        let id = self.new_ulid();
        let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
        self.create(
            &id.to_string(),
            Some(dedupe::Item::entity(
                dkind,
                Some(&id.to_string()),
                name,
                &refs,
            )),
            force,
            LocalEntity::Note(id.to_string()),
            Op::EntityCreate(sm::EntityCreate {
                id,
                kind,
                name: name.to_owned(),
                aliases: aliases.to_vec(),
                fields: std::collections::BTreeMap::new(),
                created: self.env.clock.now(),
                force,
            }),
        )
    }

    fn relation_op(
        &self,
        src_id: &str,
        dst_id: &str,
        rel_type: &str,
    ) -> CoreResult<sm::RelationRef> {
        if !self.note_exists(dst_id)? {
            return Err(CoreError::not_found("note"));
        }
        Ok(sm::RelationRef {
            src_id: ulid_of(src_id)?,
            dst_id: ulid_of(dst_id)?,
            relation: rel_type
                .parse::<RelationKey>()
                .map_err(|_| CoreError::invalid("rel_type", "unknown_relation"))?,
        })
    }

    /// Adds a relation `src —rel_type→ dst`.
    pub fn add_relation(&self, src_id: &str, dst_id: &str, rel_type: &str) -> CoreResult<String> {
        let r = self.relation_op(src_id, dst_id, rel_type)?;
        self.note_op(src_id, Op::RelationAdd(r))
    }

    /// Removes a relation.
    pub fn remove_relation(
        &self,
        src_id: &str,
        dst_id: &str,
        rel_type: &str,
    ) -> CoreResult<String> {
        let r = self.relation_op(src_id, dst_id, rel_type)?;
        self.note_op(src_id, Op::RelationRemove(r))
    }

    /// Changes a relation's type.
    pub fn retype_relation(
        &self,
        src_id: &str,
        dst_id: &str,
        rel_type: &str,
        new_type: &str,
    ) -> CoreResult<String> {
        let r = self.relation_op(src_id, dst_id, rel_type)?;
        let new_type = new_type
            .parse::<RelationKey>()
            .map_err(|_| CoreError::invalid("new_type", "unknown_relation"))?;
        self.note_op(
            src_id,
            Op::RelationRetype(sm::RelationRetype {
                src_id: r.src_id,
                dst_id: r.dst_id,
                relation: r.relation,
                new_type,
            }),
        )
    }

    /// Accepts a suggestion.
    pub fn accept_suggestion(&self, id: &str) -> CoreResult<String> {
        self.run(
            LocalEntity::Suggestion(id.to_owned()),
            Op::SuggestionAccept(sm::SuggestionAccept {
                id: ulid_of(id)?,
                edits: None,
                created: self.env.clock.now(),
            }),
        )
    }

    /// Rejects a suggestion.
    pub fn reject_suggestion(&self, id: &str) -> CoreResult<String> {
        self.run(
            LocalEntity::Suggestion(id.to_owned()),
            Op::SuggestionReject(sm::SuggestionReject {
                id: ulid_of(id)?,
                reason: None,
            }),
        )
    }

    /// Asks the server to re-run linking for a note.
    pub fn request_relink(&self, note_id: &str) -> CoreResult<String> {
        if !self.note_exists(note_id)? {
            return Err(CoreError::not_found("note"));
        }
        self.run(
            LocalEntity::Nothing,
            Op::RelinkRequest(sm::NoteRef {
                id: ulid_of(note_id)?,
            }),
        )
    }

    /// Creates a task (duplicate-checked unless `force`); the new ID is its block ID.
    pub fn create_task(&self, t: &NewTask, force: bool) -> CoreResult<CreateOutcome> {
        if t.description.trim().is_empty() {
            return Err(CoreError::invalid("description", "empty"));
        }
        let rrule = match &t.recurrence {
            Some(r) => Some(
                vault_format::tasks::parse_recurrence(r)
                    .map_err(|_| CoreError::invalid("recurrence", "not_understood"))?
                    .to_rrule(),
            ),
            None => None,
        };
        let task_id = task_block_id(self.new_ulid());
        let (local_note, note_id, home_id) = match &t.note_id {
            Some(n) => {
                if !self.note_exists(n)? {
                    return Err(CoreError::not_found("note"));
                }
                (n.clone(), Some(ulid_of(n)?), None)
            }
            // No task home yet: the device makes `tasks/Tasks.md` with its own ID, which the
            // server gives the note too (both write the same bytes).
            None => self.read(|c, _| notes::id_by_path(c, TASK_HOME))?.map_or_else(
                || {
                    let home = self.new_ulid();
                    (home.to_string(), None, Some(home))
                },
                |home| {
                    let home_id = ulid_of(&home).ok();
                    (home, None, home_id)
                },
            ),
        };
        let links: Vec<String> = vault_format::wikilink::find_all(&t.description)
            .iter()
            .map(|l| vault_format::resolve::link_name(l.target()).to_owned())
            .collect();
        let entities: Vec<&str> = links.iter().map(String::as_str).collect();
        self.create(
            &task_id,
            Some(dedupe::Item::task(
                Some(&task_id),
                &t.description,
                rrule.as_deref(),
                &entities,
            )),
            force,
            LocalEntity::Note(local_note),
            Op::TaskCreate(sm::TaskCreate {
                id: task_id.clone(),
                note_id,
                text: t.description.trim().to_owned(),
                due: t.due,
                scheduled: t.scheduled,
                start: None,
                recurrence: t.recurrence.clone(),
                reminders: t.reminders.clone(),
                priority: t.priority,
                created: self.env.clock.now(),
                home_id,
                force,
            }),
        )
    }

    fn task_op(&self, task_id: &str, op: Op) -> CoreResult<String> {
        let note = self
            .read(|c, _| note_of_task(c, task_id))?
            .ok_or_else(|| CoreError::not_found("task"))?;
        self.run(LocalEntity::Note(note), op)
    }

    /// Edits a task's fields.
    pub fn update_task(&self, task_id: &str, e: &TaskEdit) -> CoreResult<String> {
        self.task_op(
            task_id,
            Op::TaskUpdate(sm::TaskUpdate {
                id: task_id.to_owned(),
                text: e.text.clone(),
                due: e.due,
                scheduled: e.scheduled,
                start: None,
                recurrence: e.recurrence.clone(),
                reminders: e.reminders.clone(),
                priority: e.priority,
            }),
        )
    }

    /// Completes a task today; a recurring task gets its next occurrence (new block ID).
    pub fn complete_task(&self, task_id: &str) -> CoreResult<String> {
        let recurring = self.read(|c, _| {
            Ok(rusqlite::OptionalExtension::optional(c.query_row(
                "SELECT recurrence_raw IS NOT NULL FROM tasks WHERE id = ?1",
                [task_id],
                |r| r.get::<_, bool>(0),
            ))?)
        })?;
        let next_id = recurring
            .unwrap_or(false)
            .then(|| task_block_id(self.new_ulid()));
        self.task_op(
            task_id,
            Op::TaskComplete(sm::TaskComplete {
                id: task_id.to_owned(),
                done: self.today(),
                next_id,
            }),
        )
    }

    /// Cancels a task today.
    pub fn cancel_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_op(
            task_id,
            Op::TaskCancel(sm::TaskCancel {
                id: task_id.to_owned(),
                date: self.today(),
            }),
        )
    }

    /// Reopens a task.
    pub fn reopen_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_op(
            task_id,
            Op::TaskReopen(sm::TaskRef {
                id: task_id.to_owned(),
            }),
        )
    }

    /// Deletes a task line.
    pub fn delete_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_op(
            task_id,
            Op::TaskDelete(sm::TaskRef {
                id: task_id.to_owned(),
            }),
        )
    }

    /// A notification action (Done / Snooze) forwarded from Dart; applied through the outbox
    /// like any other mutation (§12.5b).
    pub fn notification_action(&self, id: i32, action: NotificationAction) -> CoreResult<String> {
        let (task_id, remind_at) = self
            .read(|c, _| crate::notify::lookup(c, id))?
            .ok_or_else(|| CoreError::not_found("notification"))?;
        let minutes = self.read(|c, _| settings::snooze_minutes(c))?;
        match action.kind {
            NotificationActionKind::Done => self.complete_task(&task_id),
            NotificationActionKind::Snooze => {
                let (rows, default_time) = self.read(|c, _| {
                    let mut st = c.prepare(
                        "SELECT remind_date, remind_time FROM task_reminders WHERE task_id = ?1
                         ORDER BY remind_date, remind_time",
                    )?;
                    let rows: Vec<(String, String)> = st
                        .query_map([&task_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                        .collect::<Result<_, _>>()?;
                    Ok((rows, crate::view::build::default_reminder_time(c)?))
                })?;
                let ctx = self.ctx();
                let later = (ctx.now.with_timezone(&ctx.tz)
                    + Duration::minutes(i64::from(minutes)))
                .naive_local();
                let later = later
                    .with_second(0)
                    .unwrap_or(later)
                    .with_nanosecond(0)
                    .unwrap_or(later);
                let mut reminders: Vec<NaiveDateTime> = rows
                    .iter()
                    .filter_map(|(d, t)| {
                        let day = NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
                        let time = NaiveTime::parse_from_str(t, "%H:%M").unwrap_or(default_time);
                        Some(day.and_time(time))
                    })
                    .collect();
                let fired = reminders
                    .iter()
                    .position(|r| r.format("%Y-%m-%d %H:%M").to_string() == remind_at);
                match fired {
                    Some(i) => reminders[i] = later,
                    None => reminders.push(later),
                }
                self.update_task(
                    &task_id,
                    &TaskEdit {
                        reminders: Some(reminders),
                        ..TaskEdit::default()
                    },
                )
            }
        }
    }

    /// Resolves a conflict (D19).
    pub fn resolve_conflict(&self, op_id: &str, resolution: ConflictResolution) -> CoreResult<()> {
        let conflict = self
            .read(|c, _| conflicts::conflict(c, op_id))?
            .ok_or_else(|| CoreError::not_found("conflict"))?;
        let keep = match resolution.kind {
            ResolutionKind::KeepServer => None,
            ResolutionKind::SaveBothAsCopies => {
                // The server's version stays; the local edit survives as its own note: the
                // conflict copy the server already wrote, or a copy created now.
                let server_copied = matches!(
                    conflict.resolution,
                    Some(sync_model::ConflictResolution::ConflictCopy { .. })
                );
                if !server_copied && let Some(local) = conflict.local_content.clone() {
                    let path = self
                        .read(|c, _| notes::current(c, &conflict.entity_id))?
                        .map_or_else(|| "notes/Conflict.md".to_owned(), |n| n.path);
                    // Named in UTC, like the server's copies (vault content every device and
                    // Obsidian share).
                    let copy_path =
                        item_render::paths::conflict_copy_path(&path, &self.env.clock.now(), 1);
                    let body = vault_format::Document::parse(&local).body().to_owned();
                    self.create_note(&copy_path, &body, true)?;
                }
                None
            }
            ResolutionKind::Merged => Some(
                resolution
                    .content
                    .ok_or_else(|| CoreError::invalid("content", "missing"))?,
            ),
            ResolutionKind::KeepMine => Some(
                conflict
                    .local_content
                    .ok_or_else(|| CoreError::not_found("conflict"))?,
            ),
            ResolutionKind::Hunks => {
                let Some(sync_model::MergeOutcome::Conflicted(c)) = conflict.merge_outcome else {
                    return Err(CoreError::invalid("resolution", "no_hunks"));
                };
                let choices: Vec<(u32, sync_model::Choice)> = resolution
                    .choices
                    .into_iter()
                    .map(|h| (h.hunk, crate::view::build::hunk_choice(h.choice, h.text)))
                    .collect();
                Some(
                    c.resolve(&choices)
                        .map_err(|e| CoreError::invalid("resolution", &e.to_string()))?,
                )
            }
        };
        let new_op = self.new_ulid();
        self.write(|c, now| {
            let mut re = crate::store::index::Reindex::new();
            apply::resolve_conflict(c, op_id, keep, new_op, now, &mut re)?;
            Ok(((), re.apply(c)?))
        })
    }

    /// Answers a duplicate prompt.
    pub fn resolve_duplicate(&self, op_id: &str, choice: DuplicateChoice) -> CoreResult<()> {
        let new_op = self.new_ulid();
        self.write(|c, now| {
            let mut re = crate::store::index::Reindex::new();
            apply::resolve_duplicate(
                c,
                op_id,
                choice == DuplicateChoice::CreateAnyway,
                new_op,
                now,
                &mut re,
            )?;
            Ok(((), re.apply(c)?))
        })
    }

    /// Dismisses a rolled-back op's notice.
    pub fn dismiss_rejection(&self, op_id: &str) -> CoreResult<()> {
        self.write(|c, _| {
            conflicts::delete_rejection(c, op_id)?;
            Ok(((), Topics::SYNC))
        })
    }

    /// Turns reminders on/off for this device: stored locally (off cancels everything,
    /// §12.5b) and synced as a device setting through the outbox.
    pub fn set_reminders_enabled(&self, enabled: bool) -> CoreResult<()> {
        let device_id = self
            .read(|c, _| apply::device_id(c))?
            .map(|d| ulid_of(&d))
            .transpose()?;
        self.write(|c, _| {
            let changed = settings::set(
                c,
                settings::REMINDERS_ENABLED,
                if enabled { "true" } else { "false" },
            )?;
            Ok((
                (),
                if changed {
                    Topics::SETTINGS
                } else {
                    Topics::NONE
                },
            ))
        })?;
        if let Some(device_id) = device_id {
            self.run(
                LocalEntity::Nothing,
                Op::DeviceSettings(sm::DeviceSettings {
                    device_id,
                    reminders_enabled: Some(enabled),
                }),
            )?;
        }
        Ok(())
    }
}
