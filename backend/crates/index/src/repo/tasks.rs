//! Tasks parsed from checklist lines (§6.11), their reminders, and reminder delivery records.

use chrono::{DateTime, NaiveDate, Utc};
use strata_common::{DeviceId, NoteId, NotificationId};

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{NotifyProvider, NotifyResult, Priority, TaskStatus};

/// A `tasks` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Task {
    /// Block ID of the line (`t-<ulid>`).
    pub id: String,
    /// Note containing the line.
    pub note_id: NoteId,
    /// Task text without signifiers.
    pub text: String,
    /// Status.
    pub status: TaskStatus,
    /// 📅 due date.
    pub due: Option<NaiveDate>,
    /// ⏳ scheduled date.
    pub scheduled: Option<NaiveDate>,
    /// 🛫 start date.
    pub start: Option<NaiveDate>,
    /// 🔁 phrase, verbatim.
    pub recurrence_raw: Option<String>,
    /// Compiled RFC 5545 RRULE.
    pub rrule: Option<String>,
    /// False if the phrase is outside the supported grammar.
    pub recurrence_understood: bool,
    /// Priority.
    pub priority: Option<Priority>,
    /// ✅ done date.
    pub done_at: Option<NaiveDate>,
    /// First line (0-based).
    pub line_start: i32,
    /// Last line (inclusive).
    pub line_end: i32,
}

/// Filter for [`list_tasks`]; `None` fields don't filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskFilter {
    /// Only this status.
    pub status: Option<TaskStatus>,
    /// Only tasks due on or before this date.
    pub due_on_or_before: Option<NaiveDate>,
    /// Only tasks in this note.
    pub note_id: Option<NoteId>,
}

/// A reminder that is due.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DueReminder {
    /// Task.
    pub task_id: String,
    /// Reminder time.
    pub remind_at: DateTime<Utc>,
}

/// A `notification_log` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Notification {
    /// ID.
    pub id: NotificationId,
    /// Task.
    pub task_id: String,
    /// Reminder time.
    pub remind_at: DateTime<Utc>,
    /// Target device.
    pub device_id: DeviceId,
    /// Channel.
    pub provider: NotifyProvider,
    /// Send time.
    pub sent_at: DateTime<Utc>,
    /// Outcome.
    pub result: NotifyResult,
}

const COLS: &str = "id, note_id, text, status, due, scheduled, start, recurrence_raw, rrule, \
    recurrence_understood, priority, done_at, line_start, line_end";

/// Replaces every task of a note (re-derivation after a write). Reminders of removed tasks
/// cascade away.
pub async fn replace_note_tasks(tx: &mut ScopedTx, note: NoteId, tasks: &[Task]) -> Result<()> {
    let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    sqlx::query("DELETE FROM tasks WHERE note_id = $1 AND NOT (id = ANY($2))")
        .bind(note)
        .bind(&ids)
        .execute(tx.conn())
        .await?;
    for t in tasks {
        sqlx::query(
            "INSERT INTO tasks (user_id, id, note_id, text, status, due, scheduled, start, \
               recurrence_raw, rrule, recurrence_understood, priority, done_at, line_start, line_end) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
             ON CONFLICT (user_id, id) DO UPDATE SET note_id = EXCLUDED.note_id, text = EXCLUDED.text, \
               status = EXCLUDED.status, due = EXCLUDED.due, scheduled = EXCLUDED.scheduled, \
               start = EXCLUDED.start, recurrence_raw = EXCLUDED.recurrence_raw, rrule = EXCLUDED.rrule, \
               recurrence_understood = EXCLUDED.recurrence_understood, priority = EXCLUDED.priority, \
               done_at = EXCLUDED.done_at, line_start = EXCLUDED.line_start, line_end = EXCLUDED.line_end",
        )
        .bind(&t.id)
        .bind(note)
        .bind(&t.text)
        .bind(t.status)
        .bind(t.due)
        .bind(t.scheduled)
        .bind(t.start)
        .bind(&t.recurrence_raw)
        .bind(&t.rrule)
        .bind(t.recurrence_understood)
        .bind(t.priority)
        .bind(t.done_at)
        .bind(t.line_start)
        .bind(t.line_end)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}

/// A task by block ID.
pub async fn get_task(tx: &mut ScopedTx, id: &str) -> Result<Option<Task>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLS} FROM tasks WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Tasks matching `filter`, by due date (undated last), then note and line.
pub async fn list_tasks(tx: &mut ScopedTx, filter: TaskFilter) -> Result<Vec<Task>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLS} FROM tasks WHERE ($1::text IS NULL OR status = $1) \
           AND ($2::date IS NULL OR due <= $2) AND ($3::uuid IS NULL OR note_id = $3) \
         ORDER BY due NULLS LAST, note_id, line_start"
    )))
    .bind(filter.status)
    .bind(filter.due_on_or_before)
    .bind(filter.note_id)
    .fetch_all(tx.conn())
    .await?)
}

/// Replaces a task's reminder times.
pub async fn replace_reminders(tx: &mut ScopedTx, task: &str, at: &[DateTime<Utc>]) -> Result<()> {
    sqlx::query("DELETE FROM task_reminders WHERE task_id = $1")
        .bind(task)
        .execute(tx.conn())
        .await?;
    sqlx::query(
        "INSERT INTO task_reminders (user_id, task_id, remind_at) \
         SELECT strata_current_user(), $1, r FROM unnest($2::timestamptz[]) r ON CONFLICT DO NOTHING",
    )
    .bind(task)
    .bind(at)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Reminders of open tasks at or before `until`, oldest first.
pub async fn due_reminders(tx: &mut ScopedTx, until: DateTime<Utc>) -> Result<Vec<DueReminder>> {
    Ok(sqlx::query_as(
        "SELECT r.task_id, r.remind_at FROM task_reminders r \
         JOIN tasks t ON t.user_id = r.user_id AND t.id = r.task_id \
         WHERE t.status = 'open' AND r.remind_at <= $1 ORDER BY r.remind_at, r.task_id",
    )
    .bind(until)
    .fetch_all(tx.conn())
    .await?)
}

/// Records a delivery. Returns false (and stores nothing) if this (task, time, device) was
/// already recorded — the idempotency guarantee of the reminders job.
pub async fn record_notification(tx: &mut ScopedTx, n: &Notification) -> Result<bool> {
    let done = sqlx::query(
        "INSERT INTO notification_log (user_id, id, task_id, remind_at, device_id, provider, sent_at, result) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (user_id, task_id, remind_at, device_id) DO NOTHING",
    )
    .bind(n.id)
    .bind(&n.task_id)
    .bind(n.remind_at)
    .bind(n.device_id)
    .bind(n.provider)
    .bind(n.sent_at)
    .bind(n.result)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Delivery records for a task, by time then device.
pub async fn notifications_for(tx: &mut ScopedTx, task: &str) -> Result<Vec<Notification>> {
    Ok(sqlx::query_as(
        "SELECT id, task_id, remind_at, device_id, provider, sent_at, result FROM notification_log \
         WHERE task_id = $1 ORDER BY remind_at, device_id",
    )
    .bind(task)
    .fetch_all(tx.conn())
    .await?)
}
