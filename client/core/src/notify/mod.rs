//! Reminders on the device (PLAN §12.5b, D27 = a): **the core decides, the OS schedules.**
//!
//! After every sync, task edit, app start and resume the session calls [`recompute`]:
//!
//! 1. The **plan**: every future reminder of an open task with a block ID, as an exact UTC
//!    instant resolved in the user's timezone (DST via `vault-format`'s `reminder_instant`), with
//!    a stable ID derived from `(task block ID, remind_at)` ([`stable_id`]); only the nearest
//!    [`WINDOW`] (iOS/macOS allow 64 pending notifications per app).
//! 2. The **diff** against `scheduled_notifications` (what this device already asked for):
//!    `cancel` for items that left the plan, `schedule` for new items, `update` for changed
//!    ones, `schedule` again for items the platform refused (retry).
//! 3. The rows are rewritten to match, and the ops are streamed to Dart, which applies them
//!    with `flutter_local_notifications` and reports each result back ([`record_result`]).
//!
//! Reminders turned off for the device, and sign-out, cancel everything ([`cancel_all`]).
//! On Linux ([`NotificationMode::WhileRunning`]) nothing is scheduled with the OS: the session
//! timer calls [`due_now`], which emits `show_now` for reminders whose time has come.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

use crate::error::CoreResult;
use crate::store::notifications::{self, ScheduledRow};
use crate::store::settings;
use crate::view::model::{NotificationMode, NotificationOp, NotificationResult};

/// Size of the rolling window of scheduled reminders.
pub const WINDOW: usize = 60;

/// A reminder that should be scheduled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// Stable ID.
    pub id: i32,
    /// `<task id>@<remind_at>`.
    pub key: String,
    /// Task block ID.
    pub task_id: String,
    /// Wall-clock time as written (`YYYY-MM-DD HH:MM`).
    pub remind_at: String,
    /// When it fires.
    pub fire_at: DateTime<Utc>,
    /// Notification title (the task).
    pub title: String,
    /// Notification body (time · note).
    pub body: String,
}

/// The stable notification ID of a reminder: the first 31 bits of
/// `sha256("<task id>@<remind_at>")`, so recomputation always yields the same ID and a later
/// server push (D27 → c) can de-duplicate by it.
pub fn stable_id(task_id: &str, remind_at: &str) -> i32 {
    let digest = Sha256::digest(format!("{task_id}@{remind_at}").as_bytes());
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) & 0x7fff_ffff;
    i32::try_from(n).unwrap_or(0)
}

/// Computes the plan (empty when reminders are off for this device).
pub fn plan(conn: &Connection, now: DateTime<Utc>, tz: Tz) -> CoreResult<Vec<Planned>> {
    if !settings::reminders_enabled(conn)? {
        return Ok(Vec::new());
    }
    let default_time = crate::view::build::default_reminder_time(conn)?;
    let rows: Vec<(String, String, String, String, String)> = {
        let mut st = conn.prepare(
            "SELECT t.id, t.description, n.title, r.remind_date, r.remind_time
             FROM task_reminders r
             JOIN tasks t ON t.id = r.task_id
             JOIN notes n ON n.id = t.note_id
             WHERE t.status = 'open' AND n.deleted = 0 AND t.id NOT LIKE 'line:%'",
        )?;
        st.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (task_id, description, note_title, d, t) in rows {
        let Ok(day) = NaiveDate::parse_from_str(&d, "%Y-%m-%d") else {
            continue;
        };
        let time = NaiveTime::parse_from_str(&t, "%H:%M").ok();
        let fire_at =
            vault_format::tasks::reminder_instant(day, time, default_time, tz).with_timezone(&Utc);
        if fire_at <= now {
            continue;
        }
        let wall = time.unwrap_or(default_time).format("%H:%M").to_string();
        let remind_at = format!("{d} {wall}");
        out.push(Planned {
            id: stable_id(&task_id, &remind_at),
            key: format!("{task_id}@{remind_at}"),
            body: format!("{wall} · {note_title}"),
            title: description,
            task_id,
            remind_at,
            fire_at,
        });
    }
    out.sort_by(|a, b| (a.fire_at, a.id).cmp(&(b.fire_at, b.id)));
    out.dedup_by_key(|p| p.id);
    out.truncate(WINDOW);
    Ok(out)
}

fn row_of(p: &Planned) -> ScheduledRow {
    ScheduledRow {
        id: p.id,
        key: p.key.clone(),
        task_id: p.task_id.clone(),
        remind_at: p.remind_at.clone(),
        fire_at: p.fire_at.to_rfc3339(),
        title: p.title.clone(),
        body: p.body.clone(),
        state: "requested".to_owned(),
        last_result: None,
    }
}

/// Recomputes the plan, updates `scheduled_notifications` and returns the ops for Dart
/// (cancels first by ID, then schedules/updates by fire time).
pub fn recompute(
    conn: &Connection,
    now: DateTime<Utc>,
    tz: Tz,
    mode: NotificationMode,
) -> CoreResult<Vec<NotificationOp>> {
    let planned = plan(conn, now, tz)?;
    let existing = notifications::all(conn)?;
    let now_s = now.to_rfc3339();
    let mut cancels = Vec::new();
    let mut schedules = Vec::new();
    for row in &existing {
        if planned.iter().any(|p| p.id == row.id) {
            continue;
        }
        notifications::delete(conn, row.id)?;
        let fired = crate::view::build::ts(&row.fire_at) <= now || row.state == "shown";
        if mode == NotificationMode::OsScheduled && !fired {
            // A reminder that already fired is left alone: cancelling would remove it from the
            // notification centre.
            cancels.push(NotificationOp::Cancel { id: row.id });
        }
    }
    for p in &planned {
        let old = existing.iter().find(|r| r.id == p.id);
        let op_schedule = || NotificationOp::Schedule {
            id: p.id,
            at: p.fire_at,
            title: p.title.clone(),
            body: p.body.clone(),
            task_id: p.task_id.clone(),
        };
        let op = match old {
            None => Some(op_schedule()),
            Some(r) if r.state == "failed" => Some(op_schedule()),
            Some(r)
                if r.fire_at != p.fire_at.to_rfc3339() || r.title != p.title || r.body != p.body =>
            {
                Some(NotificationOp::Update {
                    id: p.id,
                    at: p.fire_at,
                    title: p.title.clone(),
                    body: p.body.clone(),
                    task_id: p.task_id.clone(),
                })
            }
            Some(_) => None,
        };
        if let Some(op) = op {
            let mut row = row_of(p);
            if let Some(r) = old
                && r.state == "shown"
            {
                "shown".clone_into(&mut row.state);
            }
            notifications::put(conn, &row, &now_s)?;
            if mode == NotificationMode::OsScheduled {
                schedules.push(op);
            }
        }
    }
    cancels.sort_by_key(|op| match op {
        NotificationOp::Cancel { id } => *id,
        _ => 0,
    });
    cancels.extend(schedules);
    Ok(cancels)
}

/// Cancels everything this device scheduled (reminders off, sign-out).
pub fn cancel_all(conn: &Connection, now: DateTime<Utc>, mode: NotificationMode) -> CoreResult<Vec<NotificationOp>> {
    let mut ops = Vec::new();
    for row in notifications::all(conn)? {
        notifications::delete(conn, row.id)?;
        let fired = crate::view::build::ts(&row.fire_at) <= now || row.state == "shown";
        if mode == NotificationMode::OsScheduled && !fired {
            ops.push(NotificationOp::Cancel { id: row.id });
        }
    }
    ops.sort_by_key(|op| match op {
        NotificationOp::Cancel { id } => *id,
        _ => 0,
    });
    Ok(ops)
}

/// Records the platform's answer to an op. A denied permission is remembered for Settings
/// ("Reminders are off on this device"). Returns whether anything changed.
pub fn record_result(
    conn: &Connection,
    id: i32,
    result: NotificationResult,
    now: DateTime<Utc>,
) -> CoreResult<bool> {
    let (state, text, permission) = match result {
        NotificationResult::Ok => ("scheduled", "ok", "granted"),
        NotificationResult::PermissionDenied => ("failed", "permission_denied", "denied"),
        NotificationResult::PlatformLimit => ("failed", "platform_limit", "granted"),
    };
    let row = notifications::set_result(conn, id, state, text, &now.to_rfc3339())?;
    let perm = settings::set(conn, settings::NOTIFICATION_PERMISSION, permission)?;
    Ok(row || perm)
}

/// Linux mode: reminders due at `now` that were not shown yet → `show_now` (marked shown).
pub fn due_now(conn: &Connection, now: DateTime<Utc>) -> CoreResult<Vec<NotificationOp>> {
    let mut ops = Vec::new();
    for row in notifications::all(conn)? {
        if row.state == "shown" || crate::view::build::ts(&row.fire_at) > now {
            continue;
        }
        conn.execute(
            "UPDATE scheduled_notifications SET state = 'shown', updated_at = ?2 WHERE id = ?1",
            params![row.id, now.to_rfc3339()],
        )?;
        ops.push(NotificationOp::ShowNow {
            id: row.id,
            title: row.title,
            body: row.body,
            task_id: row.task_id,
        });
    }
    Ok(ops)
}

/// The next time [`due_now`] has something to show (Linux timer).
pub fn next_fire_at(conn: &Connection) -> CoreResult<Option<DateTime<Utc>>> {
    Ok(notifications::all(conn)?
        .into_iter()
        .filter(|r| r.state != "shown")
        .map(|r| crate::view::build::ts(&r.fire_at))
        .min())
}

/// The reminder a notification belongs to: `(task id, remind_at)`.
pub fn lookup(conn: &Connection, id: i32) -> CoreResult<Option<(String, String)>> {
    Ok(notifications::all(conn)?
        .into_iter()
        .find(|r| r.id == id)
        .map(|r| (r.task_id, r.remind_at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_are_deterministic_31_bit() {
        let a = stable_id("t-01j9a3", "2026-09-27 10:00");
        assert_eq!(a, stable_id("t-01j9a3", "2026-09-27 10:00"));
        assert_ne!(a, stable_id("t-01j9a3", "2026-10-04 10:00"));
        assert!(a >= 0);
        // Pinned: changing the derivation would orphan every scheduled notification.
        let digest = Sha256::digest(b"t-01j9a3@2026-09-27 10:00");
        let expected = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) & 0x7fff_ffff;
        assert_eq!(i64::from(a), i64::from(expected));
    }
}
