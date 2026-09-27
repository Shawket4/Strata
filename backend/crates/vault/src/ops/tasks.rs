//! Tasks (PLAN §6.11): Obsidian Tasks checklist lines inside notes. Create (default home
//! `tasks/Tasks.md` under a heading per month), edit, complete (recurring tasks follow the
//! Tasks plugin: the done line plus a new next-occurrence line above it), cancel, reopen,
//! and the list views.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use chrono::NaiveDate;
use strata_common::NoteId;
use strata_index::UserScope;
use strata_index::repo::notes;
use strata_index::repo::tasks as trepo;
use strata_index::repo::vault::{self as vrepo, TaskView};
use vault_format::Document;
use vault_format::sidecar::NoteSidecar;
use vault_format::tasks::{
    self as vtasks, DateKind, Priority, Reminder, TaskError, TaskLine, TaskSpec, extract_tasks,
    parse_recurrence,
};

use crate::derive::task_exact_key;
use crate::dup::{self, NewItem};
use crate::error::{Result, VaultError};
use crate::model::TaskItem;
use crate::paths::TASKS_NOTE;
use crate::prepare::{self, new_task_id};
use crate::store::{Author, Core, VaultService};

/// `POST /tasks`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewTask {
    /// Description (may contain wikilinks and tags).
    pub text: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    pub start: Option<NaiveDate>,
    /// 🔁 phrase (Tasks plugin language).
    pub recurrence: Option<String>,
    /// `(@…)` reminders.
    pub reminders: Vec<Reminder>,
    /// Priority.
    pub priority: Option<Priority>,
    /// Home note (default `tasks/Tasks.md`).
    pub note: Option<NoteId>,
    /// Client-generated block ID (`t-<ulid>`).
    pub id: Option<String>,
    /// Create even if it looks like a duplicate.
    pub force: bool,
}

/// `PATCH /tasks/{id}`: `None` leaves a field; `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskPatch {
    /// New description.
    pub text: Option<String>,
    /// 📅
    pub due: Option<Option<NaiveDate>>,
    /// ⏳
    pub scheduled: Option<Option<NaiveDate>>,
    /// 🛫
    pub start: Option<Option<NaiveDate>>,
    /// 🔁
    pub recurrence: Option<Option<String>>,
    /// Replaces the reminders.
    pub reminders: Option<Vec<Reminder>>,
    /// Priority.
    pub priority: Option<Option<Priority>>,
}

/// A task transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Done (recurring: plus the next occurrence).
    Complete,
    /// Cancelled.
    Cancel,
    /// Back to open.
    Reopen,
}

fn task_error(e: &TaskError) -> VaultError {
    match e {
        TaskError::NotOpen(_) => VaultError::TaskState("the task is not open".into()),
        TaskError::RecurrenceNotUnderstood(_) => {
            VaultError::TaskState("the recurrence is not understood; edit it first".into())
        }
        TaskError::NoReferenceDate => {
            VaultError::TaskState("a recurring task needs a due, scheduled or start date".into())
        }
        TaskError::NoNextOccurrence => {
            VaultError::TaskState("the recurrence has no next occurrence".into())
        }
        TaskError::NotATask | TaskError::NotRecurring | TaskError::InvalidBlockId(_) => {
            VaultError::Internal(format!("task edit failed: {e}"))
        }
    }
}

fn validate_text(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(VaultError::invalid("the task text is empty"));
    }
    if text.contains(['\n', '\r']) {
        return Err(VaultError::invalid("the task text is a single line"));
    }
    Ok(())
}

fn validate_recurrence(r: &str) -> Result<()> {
    parse_recurrence(r)
        .map(|_| ())
        .map_err(|_| VaultError::invalid("the recurrence phrase is not understood"))
}

/// The RRULE the index stores for a task line (see [`crate::derive`]).
pub fn rrule_of(task: &TaskLine) -> Option<String> {
    let reference = task
        .date(DateKind::Due)
        .or_else(|| task.date(DateKind::Scheduled))
        .or_else(|| task.date(DateKind::Start));
    match task.recurrence()? {
        Ok(rule) => Some(reference.map_or_else(|| rule.to_rrule(), |d| rule.to_rrule_anchored(d))),
        Err(_) => None,
    }
}

/// Inserts `line` at the end of the `## <heading>` section of `body` (creating the heading at
/// the end if missing).
fn insert_under_heading(body: &str, heading: &str, line: &str) -> String {
    let secs = vault_format::sections::sections(body);
    if let Some(s) = secs
        .iter()
        .find(|s| s.level == 2 && s.title.trim() == heading)
    {
        let content = &body[s.content_span.clone()];
        let trimmed = content.trim_end().len();
        let at = if trimmed == 0 {
            s.content_span.start
        } else {
            let last = s.content_span.start + trimmed;
            body[last..].find('\n').map_or(body.len(), |i| last + i + 1)
        };
        let mut out = body[..at].to_owned();
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        out.push_str(&body[at..]);
        out
    } else {
        let mut out = body.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() {
            out.push('\n');
        }
        let _ = write!(out, "## {heading}\n\n{line}\n");
        out
    }
}

/// Appends `line` at the end of `body`.
fn append_line(body: &str, line: &str) -> String {
    let mut out = body.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    out.push('\n');
    out
}

/// Finds the line of task `id` in `body`: (line span, parsed line).
fn find_task(body: &str, id: &str) -> Option<(std::ops::Range<usize>, TaskLine)> {
    extract_tasks(body)
        .into_iter()
        .find(|t| t.task.block_id() == Some(id))
        .map(|t| (t.line_span, t.task))
}

impl Core {
    async fn locate_task(&self, scope: &UserScope, id: &str) -> Result<(NoteId, String, String)> {
        let mut tx = self.begin(scope).await?;
        let row = trepo::get_task(&mut tx, id)
            .await?
            .ok_or(VaultError::NotFound)?;
        tx.commit().await?;
        let (path, version) = self.live(row.note_id)?;
        Ok((row.note_id, path, version))
    }

    /// Creates a task line (duplicate check unless `force`). Returns the task ID.
    pub async fn create_task(&mut self, scope: UserScope, req: NewTask) -> Result<String> {
        validate_text(&req.text)?;
        if let Some(r) = &req.recurrence {
            validate_recurrence(r)?;
        }
        let id = match &req.id {
            Some(i) if vault_format::blocks::is_valid_block_id(i) => i.clone(),
            Some(_) => return Err(VaultError::invalid("the task id is not a valid block id")),
            None => new_task_id(self.ids()),
        };
        let mut tx = self.begin(&scope).await?;
        if trepo::get_task(&mut tx, &id).await?.is_some() {
            return Err(VaultError::invalid("a task with this id already exists"));
        }
        let line = TaskSpec {
            description: req.text.trim().to_owned(),
            priority: req.priority,
            recurrence: req.recurrence.clone(),
            start: req.start,
            scheduled: req.scheduled,
            due: req.due,
            reminders: req.reminders.clone(),
            block_id: Some(id.clone()),
            ..TaskSpec::default()
        }
        .render();
        let parsed =
            TaskLine::parse(&line).ok_or(VaultError::invalid("the task line is invalid"))?;
        let description = parsed.description().to_owned();
        let exact = task_exact_key(&description, rrule_of(&parsed).as_deref());
        let candidates = dup::find(
            &mut tx,
            &NewItem {
                kind: "task",
                id: req.id.clone(),
                text: &description,
                exact: Some(exact),
                aliases: &[],
            },
            &self.inner.config.near_thresholds,
        )
        .await?;
        if !candidates.is_empty() && !req.force {
            return Err(VaultError::Duplicate(candidates));
        }
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        let (home_id, path, text) = match req.note {
            Some(n) => {
                let (p, _) = self.live(n)?;
                let t = self.read_text(&p).await?.ok_or(VaultError::NotFound)?;
                (n, p, t)
            }
            None => match self.state()?.notes.get(TASKS_NOTE).map(|m| m.id) {
                Some(n) => {
                    let t = self
                        .read_text(TASKS_NOTE)
                        .await?
                        .ok_or(VaultError::NotFound)?;
                    (n, TASKS_NOTE.to_owned(), t)
                }
                None => (
                    NoteId::generate(self.ids()),
                    TASKS_NOTE.to_owned(),
                    "# Tasks\n".to_owned(),
                ),
            },
        };
        let mut doc = Document::parse(&text);
        let body = if req.note.is_none() {
            insert_under_heading(doc.body(), &now.format("%Y-%m").to_string(), &line)
        } else {
            append_line(doc.body(), &line)
        };
        doc.set_body(body);
        prepare::stamp(&mut doc, home_id, Some(&now), Some(&now))?;
        let mut changes = vec![(path.clone(), Some(doc.render().into_bytes()))];
        if !candidates.is_empty() {
            let mut sc = self
                .sidecar(home_id)
                .await?
                .unwrap_or_else(|| NoteSidecar::new(home_id.as_ulid()));
            let entry = sc
                .extra
                .entry(crate::derive::TASK_KEEP_BOTH_KEY.to_owned())
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
            if let serde_json::Value::Array(items) = entry {
                for c in &candidates {
                    items.push(serde_json::json!({
                        "task": id,
                        "other": c.item,
                        "at": vault_format::frontmatter::format_timestamp(&self.now().fixed_offset()),
                    }));
                }
            }
            changes.push(Core::sidecar_change(&sc)?);
        }
        self.finish(tx, changes, Author::User.message("task create", &path))
            .await?;
        Ok(id)
    }

    /// Edits a task line in place (span edits keep every other byte). `if_match` is the
    /// version of the note that holds it.
    pub async fn patch_task(
        &mut self,
        scope: UserScope,
        id: &str,
        patch: TaskPatch,
        if_match: Option<&str>,
    ) -> Result<()> {
        if let Some(t) = &patch.text {
            validate_text(t)?;
        }
        if let Some(Some(r)) = &patch.recurrence {
            validate_recurrence(r)?;
        }
        let (note, note_path, version) = self.locate_task(&scope, id).await?;
        if let Some(m) = if_match
            && m != version
        {
            return Err(VaultError::VersionConflict { current: version });
        }
        let text = self
            .read_text(&note_path)
            .await?
            .ok_or(VaultError::NotFound)?;
        let mut doc = Document::parse(&text);
        let (span, mut task) = find_task(doc.body(), id).ok_or(VaultError::NotFound)?;
        if let Some(t) = &patch.text {
            let mut spec = task.to_spec();
            t.trim().clone_into(&mut spec.description);
            task = TaskLine::parse(&spec.render())
                .ok_or(VaultError::invalid("the task line is invalid"))?;
        }
        for (kind, value) in [
            (DateKind::Due, patch.due),
            (DateKind::Scheduled, patch.scheduled),
            (DateKind::Start, patch.start),
        ] {
            if let Some(v) = value {
                task = task.with_date(kind, v);
            }
        }
        if let Some(r) = &patch.recurrence {
            task = task.with_recurrence(r.as_deref());
        }
        if let Some(r) = &patch.reminders {
            task = task.with_reminders(r);
        }
        if let Some(p) = patch.priority {
            task = task.with_priority(p);
        }
        let body = doc.body();
        let new_body = format!(
            "{}{}{}",
            &body[..span.start],
            task.as_str(),
            &body[span.end..]
        );
        doc.set_body(new_body);
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        prepare::stamp(&mut doc, note, None, Some(&now))?;
        self.finish(
            tx,
            vec![(note_path.clone(), Some(doc.render().into_bytes()))],
            Author::User.message("task update", &note_path),
        )
        .await?;
        Ok(())
    }

    /// Completes, cancels or reopens a task. Completing a recurring task writes exactly two
    /// lines: the next occurrence (new block ID) above the completed line. Returns the new
    /// occurrence's ID, if any.
    pub async fn transition_task(
        &mut self,
        scope: UserScope,
        id: &str,
        transition: Transition,
        if_match: Option<&str>,
    ) -> Result<Option<String>> {
        let (note, path, version) = self.locate_task(&scope, id).await?;
        if let Some(m) = if_match
            && m != version
        {
            return Err(VaultError::VersionConflict { current: version });
        }
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let mut doc = Document::parse(&text);
        let (span, task) = find_task(doc.body(), id).ok_or(VaultError::NotFound)?;
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        let today = now.date_naive();
        let line = task.as_str();
        let mut next = None;
        let replacement = match transition {
            Transition::Complete if task.recurrence_text().is_some() => {
                let new_id = new_task_id(self.ids());
                let [new_line, done] =
                    vtasks::complete_recurring(line, today, &new_id).map_err(|e| task_error(&e))?;
                next = Some(new_id);
                format!("{new_line}{}{done}", doc.line_ending().as_str())
            }
            Transition::Complete => vtasks::complete(line, today).map_err(|e| task_error(&e))?,
            Transition::Cancel => vtasks::cancel(line, today).map_err(|e| task_error(&e))?,
            Transition::Reopen => {
                if task.status().is_open() {
                    return Err(VaultError::TaskState("the task is already open".into()));
                }
                vtasks::reopen(line).map_err(|e| task_error(&e))?
            }
        };
        let body = doc.body();
        let new_body = format!(
            "{}{}{}",
            &body[..span.start],
            replacement,
            &body[span.end..]
        );
        doc.set_body(new_body);
        prepare::stamp(&mut doc, note, None, Some(&now))?;
        let op = match transition {
            Transition::Complete => "task complete",
            Transition::Cancel => "task cancel",
            Transition::Reopen => "task reopen",
        };
        self.finish(
            tx,
            vec![(path.clone(), Some(doc.render().into_bytes()))],
            Author::User.message(op, &path),
        )
        .await?;
        Ok(next)
    }
}

impl VaultService {
    /// Today in the user's time zone.
    pub async fn today(&self, scope: &UserScope) -> Result<NaiveDate> {
        self.exec(scope, |core, scope| {
            Box::pin(async move {
                let mut tx = core.begin(&scope).await?;
                let tz = core.tz(&mut tx).await?;
                tx.commit().await?;
                Ok(core.local_now(tz).date_naive())
            })
        })
        .await
    }

    /// Tasks of a view, optionally only in `note` or linking to `entity`.
    pub async fn tasks(
        &self,
        scope: &UserScope,
        view: TaskView,
        note: Option<NoteId>,
        entity: Option<NoteId>,
    ) -> Result<Vec<TaskItem>> {
        let today = self.today(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        if let Some(n) = note
            && notes::get_note(&mut tx, n).await?.is_none_or(|n| n.trashed)
        {
            return Err(VaultError::NotFound);
        }
        let entity_name = match entity {
            Some(e) => Some(
                notes::get_note(&mut tx, e)
                    .await?
                    .filter(|n| !n.trashed)
                    .map(|n| crate::state::name_key(&n.path))
                    .ok_or(VaultError::NotFound)?,
            ),
            None => None,
        };
        let rows = vrepo::list_task_view(&mut tx, view, today, note).await?;
        let mut paths: BTreeMap<NoteId, String> = BTreeMap::new();
        let mut reminders = Vec::new();
        for t in &rows {
            if !paths.contains_key(&t.note_id)
                && let Some(n) = notes::get_note(&mut tx, t.note_id).await?
            {
                paths.insert(t.note_id, n.path);
            }
            reminders.push(vrepo::reminders_of(&mut tx, &t.id).await?);
        }
        tx.commit().await?;
        let mut texts: BTreeMap<NoteId, String> = BTreeMap::new();
        for (id, p) in &paths {
            texts.insert(*id, self.note_text(scope, p).await?.unwrap_or_default());
        }
        let mut out = Vec::new();
        for (t, r) in rows.into_iter().zip(reminders) {
            if let Some(name) = &entity_name {
                let links: BTreeSet<String> = vault_format::wikilink::find_all(&t.text)
                    .iter()
                    .map(|l| crate::state::name_key(l.target()))
                    .collect();
                if !links.contains(name) {
                    continue;
                }
            }
            let line = texts
                .get(&t.note_id)
                .and_then(|text| {
                    let doc = Document::parse(text);
                    find_task(doc.body(), &t.id).map(|(span, _)| doc.body()[span].to_owned())
                })
                .unwrap_or_default();
            out.push(TaskItem {
                note_path: paths.get(&t.note_id).cloned().unwrap_or_default(),
                task: t,
                line,
                reminders: r,
            });
        }
        Ok(out)
    }

    /// One task.
    pub async fn task(&self, scope: &UserScope, id: &str) -> Result<TaskItem> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let t = trepo::get_task(&mut tx, id)
            .await?
            .ok_or(VaultError::NotFound)?;
        let note = notes::get_note(&mut tx, t.note_id)
            .await?
            .ok_or(VaultError::NotFound)?;
        let reminders = vrepo::reminders_of(&mut tx, id).await?;
        tx.commit().await?;
        let text = self.note_text(scope, &note.path).await?.unwrap_or_default();
        let doc = Document::parse(&text);
        let line = find_task(doc.body(), id)
            .map(|(span, _)| doc.body()[span].to_owned())
            .unwrap_or_default();
        Ok(TaskItem {
            task: t,
            note_path: note.path,
            line,
            reminders,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_go_under_their_month_heading() {
        assert_eq!(
            insert_under_heading("# Tasks\n", "2026-09", "- [ ] a"),
            "# Tasks\n\n## 2026-09\n\n- [ ] a\n"
        );
        assert_eq!(
            insert_under_heading(
                "# Tasks\n\n## 2026-09\n\n- [ ] a\n\n## 2026-10\n\n- [ ] c\n",
                "2026-09",
                "- [ ] b"
            ),
            "# Tasks\n\n## 2026-09\n\n- [ ] a\n- [ ] b\n\n## 2026-10\n\n- [ ] c\n"
        );
        assert_eq!(
            insert_under_heading("## 2026-09\n", "2026-09", "- [ ] a"),
            "## 2026-09\n- [ ] a\n"
        );
        assert_eq!(append_line("x", "- [ ] a"), "x\n- [ ] a\n");
    }
}
