//! Intents (§12.3): each user action becomes one op with client-generated IDs and dates, applied
//! optimistically and queued. IDs come from the injected generator and dates from the injected
//! clock in the user's timezone, so the op carries everything the server needs to apply it
//! identically.

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveTime};
use vault_format::tasks::{Reminder, TaskLine, TaskSpec};

use super::Session;
use crate::error::{CoreError, CoreResult};
use crate::format::edit;
use crate::ids::task_block_id;
use crate::store::write::{Intent, LocalEntity, TASK_HOME, note_of_task};
use crate::store::{conflicts, notes, settings};
use crate::sync::apply;
use crate::sync::model::OpPayload;
use crate::view::Topics;
use crate::view::model::{ConflictResolution, DuplicateChoice, NotificationAction};

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
    /// Reminders (date, optional time).
    pub reminders: Vec<(NaiveDate, Option<NaiveTime>)>,
    /// Priority.
    pub priority: Option<domain::Priority>,
}

impl Session {
    fn new_id(&self) -> String {
        self.env.ids.ulid().to_string()
    }

    fn local_now(&self) -> DateTime<FixedOffset> {
        let ctx = self.ctx();
        ctx.now.with_timezone(&ctx.tz).fixed_offset()
    }

    fn today(&self) -> NaiveDate {
        self.local_now().date_naive()
    }

    fn run(&self, entity_id: String, local: LocalEntity, payload: OpPayload) -> CoreResult<String> {
        let op_id = self.new_id();
        self.execute(&Intent {
            op_id: op_id.clone(),
            entity_id,
            local,
            payload,
        })?;
        Ok(op_id)
    }

    fn note_intent(&self, id: &str, payload: OpPayload) -> CoreResult<String> {
        self.run(id.to_owned(), LocalEntity::Note(id.to_owned()), payload)
    }

    /// Captures text into the inbox; returns the new note ID. Never refused (§9.7).
    pub fn capture(&self, text: &str) -> CoreResult<String> {
        if text.trim().is_empty() {
            return Err(CoreError::invalid("text", "empty"));
        }
        let id = self.new_id();
        let created = self.local_now();
        let taken: Vec<String> = self.read(|c, _| {
            Ok(crate::store::notes::live_paths(c)?
                .into_iter()
                .map(|(_, p)| p)
                .collect())
        })?;
        let path = edit::capture_path(&created, taken.iter().map(String::as_str));
        self.note_intent(
            &id,
            OpPayload::Capture {
                text: text.to_owned(),
                path,
                created: vault_format::frontmatter::format_timestamp(&created),
            },
        )?;
        Ok(id)
    }

    /// Creates a note; returns its ID.
    pub fn create_note(&self, path: &str, content: &str, force: bool) -> CoreResult<String> {
        let id = self.new_id();
        self.note_intent(
            &id,
            OpPayload::NoteCreate {
                path: path.to_owned(),
                content: content.to_owned(),
                force,
            },
        )?;
        Ok(id)
    }

    /// Replaces a note's content.
    pub fn update_note(&self, id: &str, content: &str) -> CoreResult<String> {
        self.note_intent(
            id,
            OpPayload::NoteUpdate {
                content: content.to_owned(),
            },
        )
    }

    /// Moves/renames a note.
    pub fn move_note(&self, id: &str, new_path: &str) -> CoreResult<String> {
        self.note_intent(
            id,
            OpPayload::NoteMove {
                new_path: new_path.to_owned(),
            },
        )
    }

    /// Deletes a note (soft delete on the server).
    pub fn delete_note(&self, id: &str) -> CoreResult<String> {
        self.note_intent(id, OpPayload::NoteDelete)
    }

    /// Creates a person/company/document/place.
    pub fn create_entity(
        &self,
        kind: domain::NoteKind,
        name: &str,
        aliases: &[String],
        force: bool,
    ) -> CoreResult<String> {
        if !kind.is_entity() {
            return Err(CoreError::invalid("kind", "not_an_entity"));
        }
        let file = vault_format::filename::sanitize_file_name(name);
        let path = format!("{}/{file}.md", kind.default_folder());
        let id = self.new_id();
        self.note_intent(
            &id,
            OpPayload::EntityCreate {
                kind: kind.as_str().to_owned(),
                name: name.to_owned(),
                aliases: aliases.to_vec(),
                path,
                force,
            },
        )?;
        Ok(id)
    }

    fn link_of(&self, dst_id: &str) -> CoreResult<String> {
        self.read(|c, _| {
            let dst = notes::current(c, dst_id)?.ok_or_else(|| CoreError::not_found("note"))?;
            Ok(vault_format::resolve::link_path(&dst.path)
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned())
        })
    }

    /// Adds a relation `src —rel_type→ dst`.
    pub fn add_relation(&self, src_id: &str, dst_id: &str, rel_type: &str) -> CoreResult<String> {
        let dst_link = self.link_of(dst_id)?;
        self.note_intent(
            src_id,
            OpPayload::RelationAdd {
                rel_type: rel_type.to_owned(),
                dst_id: dst_id.to_owned(),
                dst_link,
            },
        )
    }

    /// Removes a relation.
    pub fn remove_relation(&self, src_id: &str, dst_id: &str, rel_type: &str) -> CoreResult<String> {
        let dst_link = self.link_of(dst_id)?;
        self.note_intent(
            src_id,
            OpPayload::RelationRemove {
                rel_type: rel_type.to_owned(),
                dst_id: dst_id.to_owned(),
                dst_link,
            },
        )
    }

    /// Changes a relation's type.
    pub fn retype_relation(
        &self,
        src_id: &str,
        dst_id: &str,
        rel_type: &str,
        new_type: &str,
    ) -> CoreResult<String> {
        let dst_link = self.link_of(dst_id)?;
        self.note_intent(
            src_id,
            OpPayload::RelationRetype {
                rel_type: rel_type.to_owned(),
                new_type: new_type.to_owned(),
                dst_id: dst_id.to_owned(),
                dst_link,
            },
        )
    }

    /// Accepts a suggestion.
    pub fn accept_suggestion(&self, id: &str) -> CoreResult<String> {
        self.run(
            id.to_owned(),
            LocalEntity::Suggestion(id.to_owned()),
            OpPayload::SuggestionAccept,
        )
    }

    /// Rejects a suggestion.
    pub fn reject_suggestion(&self, id: &str) -> CoreResult<String> {
        self.run(
            id.to_owned(),
            LocalEntity::Suggestion(id.to_owned()),
            OpPayload::SuggestionReject,
        )
    }

    /// Asks the server to re-run linking for a note.
    pub fn request_relink(&self, note_id: &str) -> CoreResult<String> {
        if !self.note_exists(note_id)? {
            return Err(CoreError::not_found("note"));
        }
        self.run(note_id.to_owned(), LocalEntity::Nothing, OpPayload::RelinkRequest)
    }

    /// Creates a task; returns its block ID.
    pub fn create_task(&self, t: &NewTask, force: bool) -> CoreResult<String> {
        if t.description.trim().is_empty() {
            return Err(CoreError::invalid("description", "empty"));
        }
        if let Some(r) = &t.recurrence {
            vault_format::tasks::parse_recurrence(r)
                .map_err(|_| CoreError::invalid("recurrence", "not_understood"))?;
        }
        let task_id = task_block_id(self.env.ids.ulid());
        let line = TaskSpec {
            description: t.description.trim().to_owned(),
            priority: t.priority,
            recurrence: t.recurrence.clone(),
            scheduled: t.scheduled,
            due: t.due,
            reminders: t
                .reminders
                .iter()
                .map(|(date, time)| Reminder {
                    date: *date,
                    time: *time,
                })
                .collect(),
            block_id: Some(task_id.clone()),
            ..TaskSpec::default()
        }
        .render();
        let (note_id, create_home) = match &t.note_id {
            Some(n) => {
                if !self.note_exists(n)? {
                    return Err(CoreError::not_found("note"));
                }
                (n.clone(), false)
            }
            None => match self.read(|c, _| notes::id_by_path(c, TASK_HOME))? {
                Some(home) => (home, false),
                None => (self.new_id(), true),
            },
        };
        self.run(
            task_id.clone(),
            LocalEntity::Note(note_id.clone()),
            OpPayload::TaskCreate {
                note_id,
                create_home,
                line,
                force,
            },
        )?;
        Ok(task_id)
    }

    fn task_intent(&self, task_id: &str, payload: OpPayload) -> CoreResult<String> {
        let note = self
            .read(|c, _| note_of_task(c, task_id))?
            .ok_or_else(|| CoreError::not_found("task"))?;
        self.run(task_id.to_owned(), LocalEntity::Note(note), payload)
    }

    /// Replaces a task's line (editor).
    pub fn update_task(&self, task_id: &str, line: &str) -> CoreResult<String> {
        self.task_intent(
            task_id,
            OpPayload::TaskUpdate {
                line: line.to_owned(),
            },
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
        let next_task_id = recurring
            .unwrap_or(false)
            .then(|| task_block_id(self.env.ids.ulid()));
        self.task_intent(
            task_id,
            OpPayload::TaskComplete {
                done_date: self.today().to_string(),
                next_task_id,
            },
        )
    }

    /// Cancels a task today.
    pub fn cancel_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_intent(
            task_id,
            OpPayload::TaskCancel {
                date: self.today().to_string(),
            },
        )
    }

    /// Reopens a task.
    pub fn reopen_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_intent(task_id, OpPayload::TaskReopen)
    }

    /// Deletes a task line.
    pub fn delete_task(&self, task_id: &str) -> CoreResult<String> {
        self.task_intent(task_id, OpPayload::TaskDelete)
    }

    /// A notification action (Done / Snooze) forwarded from Dart; applied through the outbox.
    pub fn notification_action(&self, id: i32, action: NotificationAction) -> CoreResult<String> {
        let (task_id, remind_at) = self
            .read(|c, _| crate::notify::lookup(c, id))?
            .ok_or_else(|| CoreError::not_found("notification"))?;
        match action {
            NotificationAction::Done => self.complete_task(&task_id),
            NotificationAction::Snooze { minutes } => {
                let line: String = self.read(|c, _| {
                    Ok(c.query_row("SELECT line FROM tasks WHERE id = ?1", [&task_id], |r| {
                        r.get(0)
                    })?)
                })?;
                let task = TaskLine::parse(&line).ok_or_else(|| CoreError::not_found("task"))?;
                let later = (self.local_now() + Duration::minutes(i64::from(minutes))).naive_local();
                let snoozed = Reminder {
                    date: later.date(),
                    time: Some(later.time().with_second(0).unwrap_or(later.time())),
                };
                let mut reminders = task.reminders();
                let fired = reminders.iter().position(|r| {
                    let t = r.time.map(|t| t.format("%H:%M").to_string());
                    let written = format!("{} {}", r.date, t.unwrap_or_default());
                    written.trim_end() == remind_at || remind_at.starts_with(&r.date.to_string()) && r.time.is_none()
                });
                match fired {
                    Some(i) => reminders[i] = snoozed,
                    None => reminders.push(snoozed),
                }
                let new_line = task.with_reminders(&reminders).as_str().to_owned();
                self.update_task(&task_id, &new_line)
            }
        }
    }

    /// Resolves a conflict (D19).
    pub fn resolve_conflict(&self, op_id: &str, resolution: ConflictResolution) -> CoreResult<()> {
        let keep = match resolution {
            ConflictResolution::KeepServer => None,
            ConflictResolution::Merged { content } => Some(content),
            ConflictResolution::KeepMine => Some(
                self.read(|c, _| conflicts::conflict(c, op_id))?
                    .and_then(|c| c.local_content)
                    .ok_or_else(|| CoreError::not_found("conflict"))?,
            ),
        };
        let new_op = self.new_id();
        self.write(|c, now| {
            let mut re = crate::store::index::Reindex::new();
            apply::resolve_conflict(c, op_id, keep, &new_op, now, &mut re)?;
            Ok(((), re.apply(c)?))
        })
    }

    /// Answers a duplicate prompt.
    pub fn resolve_duplicate(&self, op_id: &str, choice: DuplicateChoice) -> CoreResult<()> {
        let new_op = self.new_id();
        self.write(|c, now| {
            let mut re = crate::store::index::Reindex::new();
            apply::resolve_duplicate(
                c,
                op_id,
                choice == DuplicateChoice::CreateAnyway,
                &new_op,
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

    /// Turns reminders on/off for this device (off cancels everything, §12.5b). Returns the
    /// device ID so the caller can sync the setting to the server.
    pub fn set_reminders_enabled(&self, enabled: bool) -> CoreResult<()> {
        self.write(|c, _| {
            let changed = settings::set(
                c,
                settings::REMINDERS_ENABLED,
                if enabled { "true" } else { "false" },
            )?;
            Ok(((), if changed { Topics::SETTINGS } else { Topics::NONE }))
        })
    }
}

use chrono::Timelike as _;
