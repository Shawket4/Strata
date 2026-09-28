//! Tasks (PLAN §6.11, §7.5 Tasks & reminders): Obsidian Tasks checklist lines in notes.
//! List views, create (default home `tasks/Tasks.md`, duplicate check), edit, complete
//! (recurring: the next occurrence is written above the done line), cancel, reopen. A task's
//! version is the hash of its line (`If-Match`).

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, Responder, web};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_index::types::{Priority as IPriority, TaskStatus as IStatus};
use strata_vault::VaultService;
use strata_vault::model::{TaskItem, TaskView};
use strata_vault::ops::tasks::{NewTask, TaskPatch, Transition};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem, ProblemFieldError, ProblemType};

/// Task status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// `[ ]` (or a custom open status).
    Open,
    /// `[x]`.
    Done,
    /// `[-]`.
    Cancelled,
}

/// Priority (Tasks plugin signifiers 🔺⏫🔼🔽⏬; absent = normal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    /// 🔺
    Highest,
    /// ⏫
    High,
    /// 🔼
    Medium,
    /// 🔽
    Low,
    /// ⏬
    Lowest,
}

impl From<TaskPriority> for domain::Priority {
    fn from(p: TaskPriority) -> Self {
        match p {
            TaskPriority::Highest => Self::Highest,
            TaskPriority::High => Self::High,
            TaskPriority::Medium => Self::Medium,
            TaskPriority::Low => Self::Low,
            TaskPriority::Lowest => Self::Lowest,
        }
    }
}

/// A task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Task {
    /// Block ID (`t-<ulid>`).
    pub id: String,
    /// The note holding the line.
    #[schema(value_type = String, format = "ulid")]
    pub note_id: Ulid,
    /// Its path.
    pub note_path: String,
    /// Description (links and tags kept, signifiers removed).
    pub text: String,
    /// The whole checklist line.
    pub line: String,
    /// Version for `If-Match` (hash of the line).
    pub version: String,
    /// Status.
    pub status: TaskStatus,
    /// 📅
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// ⏳
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<NaiveDate>,
    /// 🔁 phrase, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Compiled RFC 5545 rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
    /// False when the phrase is outside the supported grammar (kept verbatim, flagged).
    pub recurrence_understood: bool,
    /// Priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<TaskPriority>,
    /// ✅ date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<NaiveDate>,
    /// Reminder instants (from `(@…)` in the user's time zone).
    pub reminders: Vec<DateTime<Utc>>,
}

impl From<TaskItem> for Task {
    fn from(t: TaskItem) -> Self {
        let r = t.task;
        Self {
            id: r.id,
            note_id: r.note_id.as_ulid(),
            note_path: t.note_path,
            text: r.text,
            line: t.line,
            version: t.version,
            status: match r.status {
                IStatus::Open => TaskStatus::Open,
                IStatus::Done => TaskStatus::Done,
                IStatus::Cancelled => TaskStatus::Cancelled,
            },
            due: r.due,
            scheduled: r.scheduled,
            start: r.start,
            recurrence: r.recurrence_raw,
            rrule: r.rrule,
            recurrence_understood: r.recurrence_understood,
            priority: r.priority.map(|p| match p {
                IPriority::Highest => TaskPriority::Highest,
                IPriority::High => TaskPriority::High,
                IPriority::Medium => TaskPriority::Medium,
                IPriority::Low => TaskPriority::Low,
                IPriority::Lowest => TaskPriority::Lowest,
            }),
            done_at: r.done_at,
            reminders: t.reminders,
        }
    }
}

/// Tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskList {
    /// In the view's order.
    pub items: Vec<Task>,
}

/// List view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskViewKind {
    /// Open, due today (or scheduled today and not due earlier).
    Today,
    /// Open, due after today (soonest first).
    Upcoming,
    /// Open, due before today (oldest first).
    Overdue,
    /// Open recurring tasks.
    Recurring,
    /// Done or cancelled, most recent first.
    Done,
    /// Every task.
    #[default]
    All,
}

/// `GET /tasks` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TasksQuery {
    /// View (default `all`); "today" is the user's time zone's.
    #[serde(default)]
    pub view: Option<TaskViewKind>,
    /// Only tasks linking to this entity.
    #[serde(default)]
    #[param(value_type = Option<String>, format = "ulid")]
    pub entity: Option<Ulid>,
    /// Only tasks in this note.
    #[serde(default)]
    #[param(value_type = Option<String>, format = "ulid")]
    pub note: Option<Ulid>,
}

/// A reminder: a wall-clock time in the user's time zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ReminderAt {
    /// Date.
    pub date: NaiveDate,
    /// Time `HH:MM` (24 h, e.g. `09:00`).
    pub time: String,
}

/// `POST /tasks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    /// Description (may contain `[[links]]` and `#tags`).
    pub text: String,
    /// 📅
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// ⏳
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<NaiveDate>,
    /// 🔁 phrase in the Tasks plugin's language (`every month on the 1st`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Reminders.
    #[serde(default)]
    pub reminders: Vec<ReminderAt>,
    /// Priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<TaskPriority>,
    /// Home note (default `tasks/Tasks.md`, under a heading per month).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub note_id: Option<Ulid>,
    /// Client-generated block ID (`t-<ulid>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Create even if it looks like a duplicate (records keep-both).
    #[serde(default)]
    pub force: bool,
}

/// A field `PATCH /tasks/{id}` can remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskField {
    /// 📅
    Due,
    /// ⏳
    Scheduled,
    /// 🛫
    Start,
    /// 🔁
    Recurrence,
    /// Priority.
    Priority,
    /// Every reminder.
    Reminders,
}

/// `PATCH /tasks/{id}`: set fields are changed in place; `clear` removes fields.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct PatchTaskRequest {
    /// New description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 📅
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// ⏳
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<NaiveDate>,
    /// 🔁
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// Replaces the reminders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminders: Option<Vec<ReminderAt>>,
    /// Priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<TaskPriority>,
    /// Fields to remove.
    #[serde(default)]
    pub clear: Vec<TaskField>,
}

/// Result of a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskTransitioned {
    /// The task after the transition.
    pub task: Task,
    /// Completing a recurring task: the next occurrence (written above it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Task>,
}

fn reminders(r: &[ReminderAt]) -> Result<Vec<chrono::NaiveDateTime>, Problem> {
    r.iter()
        .enumerate()
        .map(|(i, r)| {
            NaiveTime::parse_from_str(&r.time, "%H:%M")
                .ok()
                .filter(|_| r.time.len() == 5)
                .map(|t| r.date.and_time(t))
                .ok_or_else(|| {
                    let message = "time must be HH:MM";
                    Problem::new(ProblemType::InvalidBody)
                        .with_detail(message)
                        .with_error(ProblemFieldError {
                            code: "invalid_time".to_owned(),
                            pointer: Some(format!("/reminders/{i}/time")),
                            message: message.to_owned(),
                        })
                })
        })
        .collect()
}

/// Tasks of a view.
#[utoipa::path(
    get, path = "/tasks", tag = "tasks", operation_id = "list_tasks",
    params(TasksQuery),
    responses(
        (status = 200, description = "Tasks.", body = TaskList),
        (status = 404, description = "`not_found`: the `entity` or `note` filter names no note in the caller's vault.", body = Problem),
    ),
)]
pub async fn list_tasks(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<TasksQuery>,
) -> Result<MsgPack<TaskList>, Problem> {
    let view = match q.view.unwrap_or_default() {
        TaskViewKind::Today => TaskView::Today,
        TaskViewKind::Upcoming => TaskView::Upcoming,
        TaskViewKind::Overdue => TaskView::Overdue,
        TaskViewKind::Recurring => TaskView::Recurring,
        TaskViewKind::Done => TaskView::Done,
        TaskViewKind::All => TaskView::All,
    };
    let items = vault
        .tasks(
            auth.scope(),
            view,
            q.note.map(NoteId::from_ulid),
            q.entity.map(NoteId::from_ulid),
        )
        .await
        .or_problem()?;
    Ok(MsgPack(TaskList {
        items: items.into_iter().map(Task::from).collect(),
    }))
}

/// Create a task line (duplicate check unless `force`).
#[utoipa::path(
    post, path = "/tasks", tag = "tasks", operation_id = "create_task",
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Created; one `user: task create <path>` commit.", body = Task),
        (status = 404, description = "`not_found`: `note_id` names no note of the caller.", body = Problem),
        (status = 409, description = "`duplicate_candidates`.", body = Problem),
    ),
)]
pub async fn create_task(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CreateTaskRequest>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let id = vault
        .create_task(
            auth.scope(),
            NewTask {
                text: b.text,
                due: b.due,
                scheduled: b.scheduled,
                start: b.start,
                recurrence: b.recurrence,
                reminders: reminders(&b.reminders)?,
                priority: b.priority.map(Into::into),
                note: b.note_id.map(NoteId::from_ulid),
                id: b.id,
                force: b.force,
            },
        )
        .await
        .or_problem()?;
    let t = vault.task(auth.scope(), &id).await.or_problem()?;
    Ok(MsgPack(Task::from(t))
        .customize()
        .with_status(StatusCode::CREATED))
}

/// Edit a task line in place (`If-Match`: the task's version).
#[utoipa::path(
    patch, path = "/tasks/{id}", tag = "tasks", operation_id = "patch_task",
    params(
        ("id" = String, Path, description = "Task block ID (`t-<ulid>`)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the task's current version."),
    ),
    request_body = PatchTaskRequest,
    responses(
        (status = 200, description = "Updated.", body = Task),
        (status = 409, description = "`version_conflict`.", body = Problem),
    ),
)]
pub async fn patch_task(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<String>,
    body: MsgPack<PatchTaskRequest>,
) -> Result<MsgPack<Task>, Problem> {
    let if_match = crate::vault::if_match(req.headers())?;
    let b = body.into_inner();
    let cleared = |f: TaskField| b.clear.contains(&f);
    let date = |v: Option<NaiveDate>, f: TaskField| {
        if cleared(f) { Some(None) } else { v.map(Some) }
    };
    let patch = TaskPatch {
        text: b.text.clone(),
        due: date(b.due, TaskField::Due),
        scheduled: date(b.scheduled, TaskField::Scheduled),
        start: date(b.start, TaskField::Start),
        recurrence: if cleared(TaskField::Recurrence) {
            Some(None)
        } else {
            b.recurrence.clone().map(Some)
        },
        reminders: if cleared(TaskField::Reminders) {
            Some(Vec::new())
        } else {
            b.reminders.as_deref().map(reminders).transpose()?
        },
        priority: if cleared(TaskField::Priority) {
            Some(None)
        } else {
            b.priority.map(|p| Some(p.into()))
        },
    };
    let id = task_id(id)?;
    vault
        .patch_task(auth.scope(), id.clone(), patch, if_match)
        .await
        .or_problem()?;
    Ok(MsgPack(
        vault.task(auth.scope(), &id).await.or_problem()?.into(),
    ))
}

/// A task ID is the line's block ID; anything else names no task. Checked before any lookup,
/// so bytes the database cannot hold (NUL) never reach a query.
fn task_id(id: web::Path<String>) -> Result<String, Problem> {
    let id = id.into_inner();
    if vault_format::blocks::is_valid_block_id(&id) {
        Ok(id)
    } else {
        Err(Problem::new(ProblemType::NotFound))
    }
}

async fn transition(
    auth: &Authenticated,
    vault: &VaultService,
    req: &HttpRequest,
    id: web::Path<String>,
    t: Transition,
) -> Result<MsgPack<TaskTransitioned>, Problem> {
    let id = task_id(id)?;
    let if_match = crate::vault::if_match(req.headers())?;
    let next = vault
        .transition_task(auth.scope(), id.clone(), t, if_match)
        .await
        .or_problem()?;
    let task = vault.task(auth.scope(), &id).await.or_problem()?.into();
    let next = match next {
        Some(n) => Some(vault.task(auth.scope(), &n).await.or_problem()?.into()),
        None => None,
    };
    Ok(MsgPack(TaskTransitioned { task, next }))
}

/// Complete a task (a recurring task gets its next occurrence above the done line).
#[utoipa::path(
    post, path = "/tasks/{id}/complete", tag = "tasks", operation_id = "complete_task",
    params(
        ("id" = String, Path, description = "Task block ID."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the task's current version."),
    ),
    responses(
        (status = 200, description = "Completed.", body = TaskTransitioned),
        (status = 409, description = "`task_state_conflict` or `version_conflict`.", body = Problem),
    ),
)]
pub async fn complete_task(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<String>,
) -> Result<MsgPack<TaskTransitioned>, Problem> {
    transition(&auth, &vault, &req, id, Transition::Complete).await
}

/// Cancel a task.
#[utoipa::path(
    post, path = "/tasks/{id}/cancel", tag = "tasks", operation_id = "cancel_task",
    params(
        ("id" = String, Path, description = "Task block ID."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the task's current version."),
    ),
    responses(
        (status = 200, description = "Cancelled.", body = TaskTransitioned),
        (status = 409, description = "`task_state_conflict` or `version_conflict`.", body = Problem),
    ),
)]
pub async fn cancel_task(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<String>,
) -> Result<MsgPack<TaskTransitioned>, Problem> {
    transition(&auth, &vault, &req, id, Transition::Cancel).await
}

/// Reopen a done or cancelled task.
#[utoipa::path(
    post, path = "/tasks/{id}/reopen", tag = "tasks", operation_id = "reopen_task",
    params(
        ("id" = String, Path, description = "Task block ID."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the task's current version."),
    ),
    responses(
        (status = 200, description = "Reopened.", body = TaskTransitioned),
        (status = 409, description = "`task_state_conflict` or `version_conflict`.", body = Problem),
    ),
)]
pub async fn reopen_task(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<String>,
) -> Result<MsgPack<TaskTransitioned>, Problem> {
    transition(&auth, &vault, &req, id, Transition::Reopen).await
}

/// Mounts the task routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/tasks")
            .route(web::get().to(list_tasks))
            .route(web::post().to(create_task)),
    )
    .route("/tasks/{id}", web::patch().to(patch_task))
    .route("/tasks/{id}/complete", web::post().to(complete_task))
    .route("/tasks/{id}/cancel", web::post().to(cancel_task))
    .route("/tasks/{id}/reopen", web::post().to(reopen_task));
}
