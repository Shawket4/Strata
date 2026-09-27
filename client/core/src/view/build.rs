//! Builders: view-models from the local database. Pure reads; every list is sorted here
//! (L15: Dart never sorts or filters).

// SQL rows are read into tuples right where the query is written.
#![allow(clippy::type_complexity)]

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use vault_format::wikilink::{self, WikiLink};

use crate::error::CoreResult;
use crate::format::{self, hints};
use crate::store::{account, conflicts, from_msgpack, outbox, settings, sync_state};
use crate::sync::model::SuggestionPayload;
use crate::view::ViewCtx;
#[allow(clippy::wildcard_imports)] // the builders construct every view-model type
use crate::view::model::*;

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// Parses a stored RFC 3339 timestamp (invalid → the Unix epoch, never a panic).
pub fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).map_or(DateTime::UNIX_EPOCH, |d| d.with_timezone(&Utc))
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn u32_of(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// First line of body text, without markdown noise, ≤ `max` characters.
pub fn snippet(content: &str, max: usize) -> String {
    let doc = vault_format::Document::parse(content);
    let line = doc
        .body()
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("---"))
        .unwrap_or_default();
    let line = line.trim_start_matches(['-', '*', '>', ' ']);
    let mut out: String = line.chars().take(max).collect();
    if line.chars().count() > max {
        out.push('…');
    }
    out
}

/// Local entities (`note:<id>`, `suggestion:<id>`) with live ops, and their op counts.
fn pending_entities(conn: &Connection) -> CoreResult<HashMap<String, u32>> {
    let mut st = conn.prepare(
        "SELECT local_entity, COUNT(*) FROM outbox
         WHERE status IN ('pending', 'inflight', 'conflict', 'duplicate')
         GROUP BY local_entity",
    )?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().map(|(k, n)| (k, u32_of(n))).collect())
}

fn note_pending(pending: &HashMap<String, u32>, id: &str) -> bool {
    pending.contains_key(&format!("note:{id}"))
}

/// Resolves a link target for display: unique case-insensitive title, else exact path.
fn resolve_display(conn: &Connection, target: &str) -> CoreResult<EntityRef> {
    let title = vault_format::resolve::link_name(target).to_owned();
    let ids: Vec<String> = {
        let mut st = conn.prepare_cached(
            "SELECT id FROM notes WHERE deleted = 0 AND (title = ?1 COLLATE NOCASE OR path = ?2)
             ORDER BY id LIMIT 2",
        )?;
        st.query_map(params![title, format!("{target}.md")], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    Ok(EntityRef {
        id: (ids.len() == 1).then(|| ids[0].clone()),
        title,
    })
}

fn entity_ref(conn: &Connection, id: Option<String>, raw: Option<String>) -> CoreResult<Option<EntityRef>> {
    match (id, raw) {
        (Some(id), raw) => {
            let title: Option<String> = conn
                .query_row("SELECT title FROM notes WHERE id = ?1", [&id], |r| r.get(0))
                .optional()?;
            Ok(Some(EntityRef {
                title: title.or(raw).unwrap_or_default(),
                id: Some(id),
            }))
        }
        (None, Some(raw)) => Ok(Some(EntityRef {
            id: None,
            title: vault_format::resolve::link_name(&raw).to_owned(),
        })),
        (None, None) => Ok(None),
    }
}

fn citation(conn: &Connection, link: &WikiLink) -> CoreResult<Citation> {
    let r = resolve_display(conn, link.target())?;
    Ok(Citation {
        note_id: r.id,
        target: link.target().to_owned(),
        anchor: link.anchor.as_ref().map(|a| match a {
            vault_format::Anchor::Heading(h) => h.clone(),
            vault_format::Anchor::Block(b) => b.clone(),
        }),
    })
}

fn note_list_item(
    r: &rusqlite::Row<'_>,
    pending: &HashMap<String, u32>,
) -> rusqlite::Result<NoteListItem> {
    let id: String = r.get(0)?;
    let content: String = r.get(4)?;
    let updated: String = r.get(5)?;
    Ok(NoteListItem {
        pending_sync: note_pending(pending, &id),
        title: r.get(1)?,
        path: r.get(2)?,
        kind: r.get(3)?,
        snippet: snippet(&content, 160),
        tags: Vec::new(),
        updated_at: ts(&updated),
        id,
    })
}

fn fill_tags(conn: &Connection, items: &mut [NoteListItem]) -> CoreResult<()> {
    let mut st = conn.prepare_cached("SELECT tag FROM tags WHERE note_id = ?1 ORDER BY tag")?;
    for item in items {
        item.tags = st
            .query_map([&item.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(())
}

const NOTE_ITEM_COLUMNS: &str = "n.id, n.title, n.path, n.kind, n.content, n.local_updated_at";

fn note_items(
    conn: &Connection,
    sql_tail: &str,
    p: impl rusqlite::Params,
) -> CoreResult<Vec<NoteListItem>> {
    let pending = pending_entities(conn)?;
    let mut st = conn.prepare(&format!(
        "SELECT {NOTE_ITEM_COLUMNS} FROM notes n {sql_tail}"
    ))?;
    let mut items: Vec<NoteListItem> = st
        .query_map(p, |r| note_list_item(r, &pending))?
        .collect::<Result<_, _>>()?;
    fill_tags(conn, &mut items)?;
    Ok(items)
}

// ---------------------------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------------------------

/// The sync pill.
pub fn sync_pill(conn: &Connection, ctx: &ViewCtx) -> CoreResult<SyncPill> {
    let s = sync_state::get(conn)?;
    let count = |sql: &str| -> CoreResult<u32> {
        Ok(u32_of(conn.query_row(sql, [], |r| r.get::<_, i64>(0))?))
    };
    Ok(SyncPill {
        connectivity: ctx.connectivity,
        activity: ctx.activity.clone(),
        pending_ops: outbox::unsynced_count(conn)?,
        conflicts: count("SELECT COUNT(*) FROM conflicts")?,
        duplicates: count("SELECT COUNT(*) FROM duplicates")?,
        last_sync_at: s.last_pull_at.as_deref().map(ts),
    })
}

fn note_title(conn: &Connection, id: &str) -> CoreResult<Option<String>> {
    Ok(conn
        .query_row("SELECT title FROM notes WHERE id = ?1", [id], |r| r.get(0))
        .optional()?)
}

/// The sync status screen.
pub fn sync_status(conn: &Connection, ctx: &ViewCtx) -> CoreResult<SyncStatusView> {
    let s = sync_state::get(conn)?;
    let mut items = Vec::new();
    for op in outbox::live(conn)? {
        let status = match op.status {
            outbox::OpStatus::Inflight => OutboxStatus::Inflight,
            outbox::OpStatus::Conflict => OutboxStatus::Conflict,
            outbox::OpStatus::Duplicate => OutboxStatus::Duplicate,
            _ => OutboxStatus::Pending,
        };
        let title = match crate::store::write::LocalEntity::parse(&op.local_entity) {
            crate::store::write::LocalEntity::Note(id) => note_title(conn, &id)?,
            _ => None,
        };
        items.push(OutboxItem {
            op_id: op.op_id.clone(),
            kind: op.kind().as_str().to_owned(),
            title,
            status,
            attempts: op.attempts,
            created: ts(&op.created),
        });
    }
    let mut conflict_items = Vec::new();
    for c in conflicts::conflicts(conn)? {
        conflict_items.push(ConflictItem {
            title: note_title(conn, &c.entity_id)?.unwrap_or_default(),
            op_id: c.op_id,
            note_id: c.entity_id,
            created: ts(&c.created),
        });
    }
    let rejections = conflicts::rejections(conn)?
        .into_iter()
        .map(|r| RejectionItem {
            message_key: format!("error.{}", r.problem_type),
            op_id: r.op_id,
            kind: r.kind,
            problem_type: r.problem_type,
        })
        .collect();
    Ok(SyncStatusView {
        pill: sync_pill(conn, ctx)?,
        bootstrap_complete: s.bootstrap_complete,
        last_error: s.last_error,
        outbox: items,
        conflicts: conflict_items,
        rejections,
    })
}

/// One conflict.
pub fn conflict_screen(conn: &Connection, op_id: &str) -> CoreResult<ConflictScreen> {
    let Some(c) = conflicts::conflict(conn, op_id)? else {
        return Ok(ConflictScreen {
            op_id: op_id.to_owned(),
            conflict: None,
        });
    };
    let hunks = match &c.merge_outcome {
        Some(sync_model::MergeOutcome::Conflicted(m)) => m
            .hunks
            .iter()
            .map(|h| ConflictHunkView {
                id: h.id,
                location: match &h.location {
                    sync_model::Location::Frontmatter { key } => format!("frontmatter:{key}"),
                    sync_model::Location::Body { ours_line, .. } => format!("body:{ours_line}"),
                    sync_model::Location::LineEndings => "line_endings".to_owned(),
                    sync_model::Location::ByteOrderMark => "byte_order_mark".to_owned(),
                },
                kind: format!("{:?}", h.kind).to_ascii_lowercase(),
                base: h.base.clone(),
                ours: h.ours.clone(),
                theirs: h.theirs.clone(),
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok(ConflictScreen {
        op_id: c.op_id,
        conflict: Some(ConflictDetail {
            title: note_title(conn, &c.entity_id)?.unwrap_or_default(),
            note_id: c.entity_id,
            base: c.base_content,
            local: c.local_content,
            server: c.server_content,
            merged_preview: c.merged_preview,
            merge_clean: c.merge_clean,
            hunks,
        }),
    })
}

/// The `sync-model` choice for a hunk choice of the conflict screen.
pub fn hunk_choice(c: HunkChoiceKind, text: Option<String>) -> sync_model::Choice {
    match c {
        HunkChoiceKind::Ours => sync_model::Choice::Ours,
        HunkChoiceKind::Theirs => sync_model::Choice::Theirs,
        HunkChoiceKind::Base => sync_model::Choice::Base,
        HunkChoiceKind::OursThenTheirs => sync_model::Choice::OursThenTheirs,
        HunkChoiceKind::TheirsThenOurs => sync_model::Choice::TheirsThenOurs,
        HunkChoiceKind::Text => sync_model::Choice::Text(text.unwrap_or_default()),
    }
}

// ---------------------------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------------------------

/// Default time of date-only reminders.
pub fn default_reminder_time(conn: &Connection) -> CoreResult<NaiveTime> {
    Ok(settings::get(conn, settings::DEFAULT_REMINDER_TIME)?
        .and_then(|s| NaiveTime::parse_from_str(&s, "%H:%M").ok())
        .unwrap_or_else(|| NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default()))
}

struct TaskRow {
    id: String,
    note_id: String,
    note_title: String,
    line: String,
    description: String,
    status: String,
    priority: String,
    due: Option<String>,
    scheduled: Option<String>,
    done: Option<String>,
    recurrence_raw: Option<String>,
    recurrence_error: Option<String>,
}

const TASK_COLUMNS: &str = "t.id, t.note_id, n.title, t.line, t.description, t.status, t.priority, \
                            t.due, t.scheduled, t.done_at, t.recurrence_raw, t.recurrence_error";

fn task_rows(conn: &Connection, tail: &str, p: impl rusqlite::Params) -> CoreResult<Vec<TaskRow>> {
    let mut st = conn.prepare(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks t JOIN notes n ON n.id = t.note_id {tail}"
    ))?;
    Ok(st
        .query_map(p, |r| {
            Ok(TaskRow {
                id: r.get(0)?,
                note_id: r.get(1)?,
                note_title: r.get(2)?,
                line: r.get(3)?,
                description: r.get(4)?,
                status: r.get(5)?,
                priority: r.get(6)?,
                due: r.get(7)?,
                scheduled: r.get(8)?,
                done: r.get(9)?,
                recurrence_raw: r.get(10)?,
                recurrence_error: r.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

fn task_item(
    conn: &Connection,
    ctx: &ViewCtx,
    row: TaskRow,
    pending: &HashMap<String, u32>,
    default_time: NaiveTime,
) -> CoreResult<TaskItem> {
    let mut reminders = Vec::new();
    {
        let mut st = conn.prepare_cached(
            "SELECT remind_date, remind_time FROM task_reminders WHERE task_id = ?1
             ORDER BY remind_date, remind_time",
        )?;
        let rows: Vec<(String, String)> = st
            .query_map([&row.id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (d, t) in rows {
            let Some(day) = date(&d) else { continue };
            let time = NaiveTime::parse_from_str(&t, "%H:%M").ok();
            let at = vault_format::tasks::reminder_instant(day, time, default_time, ctx.tz)
                .with_timezone(&Utc);
            reminders.push(ReminderItem {
                local: if t.is_empty() { d } else { format!("{d} {t}") },
                at,
            });
        }
    }
    let mut links = Vec::new();
    for l in wikilink::find_all(&row.line) {
        if !l.embed {
            links.push(resolve_display(conn, l.target())?);
        }
    }
    Ok(TaskItem {
        pending_sync: note_pending(pending, &row.note_id),
        state: match row.status.as_str() {
            "done" => TaskState::Done,
            "cancelled" => TaskState::Cancelled,
            _ => TaskState::Open,
        },
        due: row.due.as_deref().and_then(date),
        scheduled: row.scheduled.as_deref().and_then(date),
        done: row.done.as_deref().and_then(date),
        recurrence_understood: row.recurrence_raw.is_none() || row.recurrence_error.is_none(),
        recurrence: row.recurrence_raw,
        id: row.id,
        note_id: row.note_id,
        note_title: row.note_title,
        description: row.description,
        priority: row.priority,
        reminders,
        links,
    })
}

/// "Today" in the user's timezone.
pub fn today(ctx: &ViewCtx) -> NaiveDate {
    ctx.now.with_timezone(&ctx.tz).date_naive()
}

/// Open tasks grouped into sections. Each section is sorted by date, then priority, then
/// description.
pub fn task_sections(conn: &Connection, ctx: &ViewCtx) -> CoreResult<TaskSections> {
    let pending = pending_entities(conn)?;
    let default_time = default_reminder_time(conn)?;
    let rows = task_rows(
        conn,
        "WHERE t.status = 'open' AND n.deleted = 0
         ORDER BY COALESCE(t.due, t.scheduled, '9999'), t.description, t.id",
        [],
    )?;
    let today = today(ctx);
    let mut s = TaskSections::default();
    for row in rows {
        let item = task_item(conn, ctx, row, &pending, default_time)?;
        if item.recurrence.is_some() {
            s.recurring.push(item.clone());
        }
        match (item.due, item.scheduled) {
            (Some(due), _) if due < today => s.overdue.push(item),
            (Some(due), _) if due == today => s.today.push(item),
            (_, Some(sched)) if sched <= today => s.today.push(item),
            (None, None) => s.no_date.push(item),
            _ => s.upcoming.push(item),
        }
    }
    let prio = |t: &TaskItem| {
        t.priority
            .parse::<domain::Priority>()
            .unwrap_or(domain::Priority::Normal)
    };
    for list in [
        &mut s.overdue,
        &mut s.today,
        &mut s.upcoming,
        &mut s.recurring,
        &mut s.no_date,
    ] {
        list.sort_by(|a, b| {
            (a.due.or(a.scheduled), prio(a), &a.description, &a.id).cmp(&(
                b.due.or(b.scheduled),
                prio(b),
                &b.description,
                &b.id,
            ))
        });
    }
    Ok(s)
}

/// The Tasks screen.
pub fn tasks_view(conn: &Connection, ctx: &ViewCtx) -> CoreResult<TasksView> {
    let pending = pending_entities(conn)?;
    let default_time = default_reminder_time(conn)?;
    let rows = task_rows(
        conn,
        "WHERE t.status IN ('done', 'cancelled') AND n.deleted = 0
         ORDER BY COALESCE(t.done_at, '') DESC, t.description, t.id LIMIT 50",
        [],
    )?;
    let mut done = Vec::new();
    for row in rows {
        done.push(task_item(conn, ctx, row, &pending, default_time)?);
    }
    Ok(TasksView {
        sections: task_sections(conn, ctx)?,
        done,
    })
}

/// Task detail.
pub fn task_screen(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<TaskScreen> {
    let pending = pending_entities(conn)?;
    let default_time = default_reminder_time(conn)?;
    let Some(row) = task_rows(conn, "WHERE t.id = ?1 AND n.deleted = 0", [id])?
        .into_iter()
        .next()
    else {
        return Ok(TaskScreen {
            id: id.to_owned(),
            task: None,
            line: String::new(),
            history: Vec::new(),
        });
    };
    let line = row.line.clone();
    let history_rows = match &row.recurrence_raw {
        Some(rec) => task_rows(
            conn,
            "WHERE t.note_id = ?1 AND t.status = 'done' AND t.description = ?2
               AND t.recurrence_raw = ?3 AND t.id != ?4
             ORDER BY COALESCE(t.done_at, '') DESC, t.id",
            params![row.note_id, row.description, rec, row.id],
        )?,
        None => Vec::new(),
    };
    let task = task_item(conn, ctx, row, &pending, default_time)?;
    let mut history = Vec::new();
    for h in history_rows {
        history.push(task_item(conn, ctx, h, &pending, default_time)?);
    }
    Ok(TaskScreen {
        id: id.to_owned(),
        task: Some(task),
        line,
        history,
    })
}

// ---------------------------------------------------------------------------------------------
// Home, inbox, suggestions
// ---------------------------------------------------------------------------------------------

/// Home.
pub fn home(conn: &Connection, ctx: &ViewCtx) -> CoreResult<HomeView> {
    let recent_notes = note_items(
        conn,
        "WHERE n.deleted = 0 AND n.path NOT LIKE 'inbox/%'
         ORDER BY n.local_updated_at DESC, n.id DESC LIMIT 10",
        [],
    )?;
    let inbox_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM inbox i JOIN notes n ON n.id = i.note_id WHERE n.deleted = 0",
        [],
        |r| r.get(0),
    )?;
    Ok(HomeView {
        recent_notes,
        inbox_count: u32_of(inbox_count),
        tasks: task_sections(conn, ctx)?,
        sync: sync_pill(conn, ctx)?,
    })
}

fn candidate_item(c: dedupe::DuplicateCandidate) -> CandidateItem {
    crate::session::candidate_item(c)
}

fn suggestion_detail(conn: &Connection, p: SuggestionPayload) -> CoreResult<SuggestionDetail> {
    Ok(match p {
        SuggestionPayload::Filing {
            title,
            folder,
            tags,
        } => SuggestionDetail {
            title,
            folder,
            tags,
            ..SuggestionDetail::of(SuggestionKind::Filing)
        },
        SuggestionPayload::EntityLinkOrCreate {
            mention,
            candidates,
        } => SuggestionDetail {
            mention,
            candidates: candidates
                .into_iter()
                .map(|(id, title)| EntityRef { id: Some(id), title })
                .collect(),
            ..SuggestionDetail::of(SuggestionKind::EntityLinkOrCreate)
        },
        SuggestionPayload::Custody {
            document_id,
            line,
            confidence,
        } => SuggestionDetail {
            document: entity_ref(conn, document_id, None)?,
            line,
            confidence: Some(confidence),
            ..SuggestionDetail::of(SuggestionKind::Custody)
        },
        SuggestionPayload::Duplicate { candidates } => SuggestionDetail {
            duplicates: candidates.into_iter().map(candidate_item).collect(),
            ..SuggestionDetail::of(SuggestionKind::Duplicate)
        },
        SuggestionPayload::Relation {
            dst_id,
            rel_type,
            confidence,
            reason,
        } => SuggestionDetail {
            target: entity_ref(conn, Some(dst_id), None)?,
            rel_type,
            confidence: Some(confidence),
            reason,
            ..SuggestionDetail::of(SuggestionKind::Relation)
        },
        SuggestionPayload::Task { line } => SuggestionDetail {
            line,
            ..SuggestionDetail::of(SuggestionKind::Task)
        },
        SuggestionPayload::Other { kind } => SuggestionDetail {
            server_kind: kind,
            ..SuggestionDetail::of(SuggestionKind::Unsupported)
        },
    })
}

fn suggestions(conn: &Connection, only_pending: bool) -> CoreResult<Vec<SuggestionItem>> {
    let pending = pending_entities(conn)?;
    let rows: Vec<(String, Option<String>, Vec<u8>, String, String, String)> = {
        let mut st = conn.prepare(&format!(
            "SELECT id, note_id, payload, status, created, kind FROM suggestions {} ORDER BY created DESC, id",
            if only_pending {
                "WHERE status = 'pending'"
            } else {
                ""
            }
        ))?;
        st.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (id, note_id, payload, status, created, kind) in rows {
        let payload = SuggestionPayload::decode(&kind, &payload);
        out.push(SuggestionItem {
            pending_sync: pending.contains_key(&format!("suggestion:{id}")),
            detail: suggestion_detail(conn, payload)?,
            id,
            note_id,
            status,
            created: ts(&created),
        });
    }
    Ok(out)
}

/// Inbox: captures with their pending suggestions, and other pending suggestions.
pub fn inbox(conn: &Connection) -> CoreResult<InboxView> {
    let pending = pending_entities(conn)?;
    let rows: Vec<(String, String, String, String)> = {
        let mut st = conn.prepare(
            "SELECT n.id, n.title, n.content, i.created FROM inbox i JOIN notes n ON n.id = i.note_id
             WHERE n.deleted = 0 ORDER BY i.created DESC, n.id DESC",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut by_note: BTreeMap<String, Vec<SuggestionItem>> = BTreeMap::new();
    let mut other = Vec::new();
    let inbox_ids: HashSet<&String> = rows.iter().map(|r| &r.0).collect();
    for s in suggestions(conn, true)? {
        match &s.note_id {
            Some(n) if inbox_ids.contains(n) => by_note.entry(n.clone()).or_default().push(s),
            _ => other.push(s),
        }
    }
    let captures = rows
        .into_iter()
        .map(|(id, title, content, created)| {
            let doc = vault_format::Document::parse(&content);
            let text: String = doc.body().trim().chars().take(280).collect();
            InboxItem {
                pending_sync: note_pending(&pending, &id),
                suggestions: by_note.remove(&id).unwrap_or_default(),
                note_id: id,
                title,
                text,
                created: ts(&created),
            }
        })
        .collect();
    Ok(InboxView {
        captures,
        suggestions: other,
    })
}

/// Open duplicate prompts.
pub fn duplicate_prompts(conn: &Connection) -> CoreResult<DuplicatePromptsView> {
    let mut prompts = Vec::new();
    for (op_id, candidates) in conflicts::duplicates(conn)? {
        let Some(op) = outbox::get(conn, &op_id)? else {
            continue;
        };
        let (kind, title) = crate::store::write::describe_create(&op.op);
        prompts.push(DuplicatePrompt {
            op_id,
            kind,
            title,
            candidates: candidates.into_iter().map(candidate_item).collect(),
        });
    }
    Ok(DuplicatePromptsView { prompts })
}

// ---------------------------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------------------------

fn relation_chips(conn: &Connection, sql: &str, id: &str) -> CoreResult<Vec<RelationChip>> {
    let rows: Vec<(String, Option<String>, String, Option<String>, Option<f64>, Option<String>)> = {
        let mut st = conn.prepare(sql)?;
        st.query_map([id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (rel_type, other_id, raw, by, confidence, reason) in rows {
        out.push(RelationChip {
            rel_type,
            target: entity_ref(conn, other_id, Some(raw))?.unwrap_or(EntityRef {
                id: None,
                title: String::new(),
            }),
            by: by.unwrap_or_else(|| "user".to_owned()),
            confidence,
            reason,
        });
    }
    Ok(out)
}

const OUTGOING_RELATIONS: &str = "SELECT r.rel_type, r.dst_id, r.dst_raw, m.by, m.confidence, m.reason
     FROM relations r
     LEFT JOIN relation_meta m ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
     WHERE r.src_id = ?1 ORDER BY r.rel_type, r.dst_raw";

fn backlinks(conn: &Connection, id: &str) -> CoreResult<Vec<BacklinkGroup>> {
    let rows: Vec<(String, String, String)> = {
        let mut st = conn.prepare(
            "SELECT 'link', n.id, n.title FROM links l JOIN notes n ON n.id = l.note_id
               WHERE l.dst_id = ?1 AND n.deleted = 0 AND l.note_id != ?1
             UNION
             SELECT r.rel_type, n.id, n.title FROM relations r JOIN notes n ON n.id = r.src_id
               WHERE r.dst_id = ?1 AND n.deleted = 0 AND r.src_id != ?1
             ORDER BY 1, 3, 2",
        )?;
        st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut groups: Vec<BacklinkGroup> = Vec::new();
    for (kind, note_id, title) in rows {
        let item = BacklinkItem { note_id, title };
        match groups.last_mut() {
            Some(g) if g.kind == kind => g.items.push(item),
            _ => groups.push(BacklinkGroup {
                kind,
                items: vec![item],
            }),
        }
    }
    Ok(groups)
}

fn hint_kind(k: hints::SpanKind) -> HintKind {
    match k {
        hints::SpanKind::Frontmatter => HintKind::Frontmatter,
        hints::SpanKind::Heading => HintKind::Heading,
        hints::SpanKind::WikiLink => HintKind::WikiLink,
        hints::SpanKind::Embed => HintKind::Embed,
        hints::SpanKind::Tag => HintKind::Tag,
        hints::SpanKind::BlockId => HintKind::BlockId,
        hints::SpanKind::TaskLine => HintKind::TaskLine,
        hints::SpanKind::Code => HintKind::Code,
    }
}

fn is_relation_key(key: &str) -> bool {
    key.parse::<vault_format::RelationKey>().is_ok()
}

/// The note screen.
pub fn note_screen(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<NoteScreen> {
    let Some(n) = crate::store::notes::get(conn, id)?.filter(|n| !n.deleted) else {
        return Ok(NoteScreen {
            id: id.to_owned(),
            note: None,
        });
    };
    let parsed = format::parse_note(&n.path, &n.content);
    let pending = pending_entities(conn)?;
    let conflict_op: Option<String> = conn
        .query_row(
            "SELECT op_id FROM outbox WHERE local_entity = ?1 AND status = 'conflict'
             ORDER BY ord LIMIT 1",
            [format!("note:{id}")],
            |r| r.get(0),
        )
        .optional()?;
    let pending_ops = pending.get(&format!("note:{id}")).copied().unwrap_or(0);
    let sync = NoteSyncState {
        kind: match (&conflict_op, pending_ops) {
            (Some(_), _) => NoteSyncKind::Conflict,
            (None, 0) => NoteSyncKind::Synced,
            (None, _) => NoteSyncKind::Pending,
        },
        pending_ops,
        conflict_op_id: conflict_op,
    };
    let default_time = default_reminder_time(conn)?;
    let mut tasks = Vec::new();
    for row in task_rows(conn, "WHERE t.note_id = ?1 ORDER BY t.line_no", [id])? {
        tasks.push(task_item(conn, ctx, row, &pending, default_time)?);
    }
    Ok(NoteScreen {
        id: id.to_owned(),
        note: Some(NoteView {
        id: n.id.clone(),
        path: n.path,
        title: parsed.display_title.clone(),
        kind: n.kind,
        version: n.base_version,
        properties: parsed
            .properties
            .iter()
            .filter(|(k, _)| !is_relation_key(k))
            .map(|(k, v)| PropertyItem {
                key: k.clone(),
                values: format::property_display(v),
            })
            .collect(),
        relations: relation_chips(conn, OUTGOING_RELATIONS, id)?,
        backlinks: backlinks(conn, id)?,
        tags: parsed.tags,
        tasks,
        hints: hints_of(&n.content),
        sync,
        history: availability_online(ctx, "history"),
        content: n.content,
        }),
    })
}

/// Editor highlight spans of `content`.
pub fn hints_of(content: &str) -> Vec<EditorHint> {
    hints::editor_hints(content)
        .into_iter()
        .map(|h| EditorHint {
            kind: hint_kind(h.kind),
            start: h.start,
            end: h.end,
        })
        .collect()
}

/// Admin → Users from the server's list: pending approvals oldest first, then every other
/// account by username.
pub fn admin_users(users: Vec<AdminUserItem>) -> AdminUsersView {
    let (mut pending, mut others): (Vec<_>, Vec<_>) =
        users.into_iter().partition(|u| u.status == "pending");
    pending.sort_by(|a, b| (a.created, &a.id).cmp(&(b.created, &b.id)));
    others.sort_by(|a, b| {
        (a.username.to_lowercase(), &a.id).cmp(&(b.username.to_lowercase(), &b.id))
    });
    AdminUsersView {
        availability: Availability::Available,
        pending,
        users: others,
    }
}

/// Online-only features whose endpoints do not exist yet.
pub fn availability_online(ctx: &ViewCtx, _feature: &str) -> Availability {
    if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::NotYetAvailable
    }
}

/// A folder's subfolders and notes.
pub fn notes_list(conn: &Connection, folder: &str) -> CoreResult<NotesListView> {
    let folder = folder.trim_matches('/');
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    };
    let paths = crate::store::notes::live_paths(conn)?;
    let mut sub: BTreeMap<String, u32> = BTreeMap::new();
    let mut direct: Vec<String> = Vec::new();
    for (id, path) in &paths {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        match rest.split_once('/') {
            Some((dir, _)) => {
                let e = sub.entry(dir.to_owned()).or_default();
                if !rest[dir.len() + 1..].contains('/') {
                    *e += 1;
                }
            }
            None => direct.push(id.clone()),
        }
    }
    let mut notes = Vec::new();
    for id in direct {
        notes.extend(note_items(conn, "WHERE n.id = ?1", [&id])?);
    }
    notes.sort_by(|a, b| (a.title.to_lowercase(), &a.id).cmp(&(b.title.to_lowercase(), &b.id)));
    Ok(NotesListView {
        folder: folder.to_owned(),
        folders: sub
            .into_iter()
            .map(|(name, note_count)| FolderItem {
                path: format!("{prefix}{name}"),
                name,
                note_count,
            })
            .collect(),
        notes,
    })
}

// ---------------------------------------------------------------------------------------------
// Directory and entity pages
// ---------------------------------------------------------------------------------------------

fn aliases_of(conn: &Connection, id: &str, title: &str) -> CoreResult<Vec<String>> {
    let mut st = conn.prepare_cached(
        "SELECT alias FROM entity_aliases WHERE note_id = ?1 AND alias != ?2 ORDER BY alias",
    )?;
    Ok(st
        .query_map(params![id, title], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

fn tab_kind(tab: DirectoryTab) -> &'static str {
    match tab {
        DirectoryTab::People => "person",
        DirectoryTab::Companies => "company",
        DirectoryTab::Documents => "document",
        DirectoryTab::Places => "place",
    }
}

/// The directory tab, filtered by `query` over names and aliases (normalised, both scripts).
pub fn directory(conn: &Connection, tab: DirectoryTab, query: &str) -> CoreResult<DirectoryView> {
    let kind = tab_kind(tab);
    let q = text_normalize::normalize_for_search(query.trim());
    let rows: Vec<(String, String, Option<String>, Option<String>)> = {
        let mut st = conn.prepare(
            "SELECT e.note_id, e.display_name, e.role, e.industry FROM entities e
             JOIN notes n ON n.id = e.note_id
             WHERE e.kind = ?1 AND n.deleted = 0
               AND (?2 = '' OR EXISTS (SELECT 1 FROM entity_aliases a WHERE a.note_id = e.note_id
                                       AND instr(a.alias_normalized, ?2) > 0))
             ORDER BY e.display_name COLLATE NOCASE, e.note_id",
        )?;
        st.query_map(params![kind, q], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut items = Vec::new();
    for (id, title, role, industry) in rows {
        let subtitle = match tab {
            DirectoryTab::People => role,
            DirectoryTab::Companies => industry,
            DirectoryTab::Documents => {
                let brief = document_brief(conn, &id)?;
                match (brief.status, brief.location.or(brief.holder)) {
                    (Some(s), Some(w)) => Some(format!("{s} · {}", w.title)),
                    (Some(s), None) => Some(s),
                    (None, Some(w)) => Some(w.title),
                    (None, None) => None,
                }
            }
            DirectoryTab::Places => place_parent(conn, &id)?.map(|p| p.title),
        };
        items.push(DirectoryItem {
            aliases: aliases_of(conn, &id, &title)?,
            id,
            title,
            subtitle,
        });
    }
    let count = |k: &str| -> CoreResult<u32> {
        Ok(u32_of(conn.query_row(
            "SELECT COUNT(*) FROM entities e JOIN notes n ON n.id = e.note_id
             WHERE e.kind = ?1 AND n.deleted = 0",
            [k],
            |r| r.get(0),
        )?))
    };
    Ok(DirectoryView {
        tab,
        query: query.to_owned(),
        items,
        counts: DirectoryCounts {
            people: count("person")?,
            companies: count("company")?,
            documents: count("document")?,
            places: count("place")?,
        },
    })
}

fn document_brief(conn: &Connection, id: &str) -> CoreResult<DocumentBrief> {
    let row: Option<(String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT n.title, d.status, d.location_id, d.location_raw, d.holder_id, d.holder_raw
             FROM documents d JOIN notes n ON n.id = d.note_id WHERE d.note_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((title, status, loc_id, loc_raw, holder_id, holder_raw)) = row else {
        return Ok(DocumentBrief {
            id: id.to_owned(),
            title: String::new(),
            status: None,
            location: None,
            holder: None,
        });
    };
    Ok(DocumentBrief {
        id: id.to_owned(),
        title,
        status,
        location: entity_ref(conn, loc_id, loc_raw)?,
        holder: entity_ref(conn, holder_id, holder_raw)?,
    })
}

fn place_parent(conn: &Connection, id: &str) -> CoreResult<Option<EntityRef>> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT parent_id, parent_raw FROM places WHERE note_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((pid, raw)) => entity_ref(conn, pid, raw),
        None => Ok(None),
    }
}

/// Enclosing places of `id`, outermost first, including `id` itself when `include_self`.
fn breadcrumb(conn: &Connection, id: &str, include_self: bool) -> CoreResult<Vec<EntityRef>> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    if include_self {
        chain.push(entity_ref(conn, Some(id.to_owned()), None)?.unwrap_or(EntityRef {
            id: Some(id.to_owned()),
            title: String::new(),
        }));
    }
    let mut cur = id.to_owned();
    seen.insert(cur.clone());
    while let Some(parent) = place_parent(conn, &cur)? {
        let next = parent.id.clone();
        chain.push(parent);
        match next {
            Some(n) if seen.insert(n.clone()) => cur = n,
            _ => break,
        }
    }
    chain.reverse();
    Ok(chain)
}

fn custody_items(conn: &Connection, where_sql: &str, p: impl rusqlite::Params) -> CoreResult<Vec<CustodyItem>> {
    type Row = (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Vec<u8>,
    );
    let rows: Vec<Row> = {
        let mut st = conn.prepare(&format!(
            "SELECT c.document_id, c.type, c.at, c.place_id, c.place_raw, c.person_id, c.person_raw,
                    c.counterparty_id, c.counterparty_raw, c.citations
             FROM custody_events c JOIN notes n ON n.id = c.document_id
             WHERE n.deleted = 0 AND {where_sql}
             ORDER BY c.at DESC, c.document_id, c.ord"
        ))?;
        st.query_map(p, |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (doc, kind, at, pid, praw, perid, perraw, cid, craw, cites) in rows {
        let cites: Vec<String> = from_msgpack(&cites)?;
        let mut citations = Vec::new();
        for c in cites {
            if let Some(l) = WikiLink::parse_exact(&c) {
                citations.push(citation(conn, &l)?);
            }
        }
        out.push(CustodyItem {
            date: date(&at).unwrap_or_default(),
            kind,
            document: entity_ref(conn, Some(doc), None)?,
            place: entity_ref(conn, pid, praw)?,
            person: entity_ref(conn, perid, perraw)?,
            counterparty: entity_ref(conn, cid, craw)?,
            citations,
        });
    }
    Ok(out)
}

fn cited_bullets(conn: &Connection, content: &str) -> CoreResult<Vec<CitedBullet>> {
    let mut out = Vec::new();
    for line in content.lines() {
        let Some(item) = line.trim().strip_prefix("- ") else {
            continue;
        };
        let links = wikilink::find_all(item);
        let mut text = String::new();
        let mut last = 0;
        let mut citations = Vec::new();
        for l in &links {
            text.push_str(&item[last..l.span.start]);
            last = l.span.end;
            citations.push(citation(conn, l)?);
        }
        text.push_str(&item[last..]);
        let trimmed = text.trim().trim_end_matches(['—', '–', '-']).trim();
        let day = trimmed.get(..10).and_then(date);
        let text = match day {
            Some(_) => trimmed[10..]
                .trim_start()
                .trim_start_matches(['—', '–', '-'])
                .trim()
                .to_owned(),
            None => trimmed.to_owned(),
        };
        out.push(CitedBullet {
            text,
            date: day,
            citations,
        });
    }
    Ok(out)
}

fn entity_view(conn: &Connection, n: &crate::store::notes::NoteRow) -> CoreResult<EntityView> {
    let parsed = format::parse_note(&n.path, &n.content);
    let body = parsed.body.clone();
    let section = |title: &str| {
        vault_format::sections::sections(&body)
            .into_iter()
            .find(|s| s.level == 2 && s.title.eq_ignore_ascii_case(title))
            .map(|s| body[s.own_content_span].to_owned())
    };
    let pending = pending_entities(conn)?;
    let mentions = note_items(
        conn,
        "WHERE n.deleted = 0 AND n.id != ?1 AND n.id IN (
            SELECT note_id FROM links WHERE dst_id = ?1
            UNION SELECT src_id FROM relations WHERE dst_id = ?1)
         ORDER BY n.local_updated_at DESC, n.id DESC",
        [&n.id],
    )?;
    let mut related = relation_chips(conn, OUTGOING_RELATIONS, &n.id)?;
    related.retain(|c| {
        c.target
            .id
            .as_deref()
            .is_some_and(|t| is_entity(conn, t).unwrap_or(false))
    });
    let incoming = relation_chips(
        conn,
        "SELECT r.rel_type, r.src_id, n.title, m.by, m.confidence, m.reason
         FROM relations r JOIN notes n ON n.id = r.src_id
         JOIN entities e ON e.note_id = r.src_id
         LEFT JOIN relation_meta m ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
         WHERE r.dst_id = ?1 AND n.deleted = 0 ORDER BY r.rel_type, n.title",
        &n.id,
    )?;
    related.extend(incoming);
    let doc_ids: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT d.note_id FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE n.deleted = 0 AND (d.holder_id = ?1 OR d.last_holder_id = ?1
                OR d.note_id IN (SELECT src_id FROM relations WHERE dst_id = ?1
                                 AND rel_type IN ('companies', 'people')))
             ORDER BY n.title, n.id",
        )?;
        st.query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let mut documents = Vec::new();
    for d in doc_ids {
        documents.push(document_brief(conn, &d)?);
    }
    Ok(EntityView {
        id: n.id.clone(),
        kind: n.kind.clone(),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        properties: parsed
            .properties
            .iter()
            .filter(|(k, _)| !is_relation_key(k) && !matches!(k.as_str(), "id" | "kind" | "aliases"))
            .map(|(k, v)| PropertyItem {
                key: k.clone(),
                values: format::property_display(v),
            })
            .collect(),
        summary: section("Summary").map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()),
        insights: cited_bullets(conn, &section("Insights").unwrap_or_default())?,
        open_items: cited_bullets(conn, &section("Open items").unwrap_or_default())?,
        timeline: cited_bullets(conn, &section("Timeline").unwrap_or_default())?,
        mentions,
        related,
        documents,
        pending_sync: note_pending(&pending, &n.id),
    })
}

fn is_entity(conn: &Connection, id: &str) -> CoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM entities WHERE note_id = ?1)",
        [id],
        |r| r.get(0),
    )?)
}

fn document_view(conn: &Connection, n: &crate::store::notes::NoteRow) -> CoreResult<DocumentView> {
    let parsed = format::parse_note(&n.path, &n.content);
    let d = parsed.document.clone().unwrap_or_default();
    let ids: Option<(Option<String>, Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT location_id, holder_id, last_holder_id, copy_of_id FROM documents WHERE note_id = ?1",
            [&n.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (loc_id, holder_id, last_id, copy_of_id) = ids.unwrap_or_default();
    let location = match &loc_id {
        Some(l) => breadcrumb(conn, l, true)?,
        None => d
            .location
            .clone()
            .map(|raw| {
                vec![EntityRef {
                    id: None,
                    title: vault_format::resolve::link_name(&raw).to_owned(),
                }]
            })
            .unwrap_or_default(),
    };
    let mut copies = Vec::new();
    if let Some(orig) = copy_of_id.clone() {
        copies.extend(entity_ref(conn, Some(orig), None)?);
    }
    let copy_ids: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT d.note_id FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE d.copy_of_id = ?1 AND n.deleted = 0 ORDER BY n.title, n.id",
        )?;
        st.query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for c in copy_ids {
        copies.extend(entity_ref(conn, Some(c), None)?);
    }
    let concerns = relation_chips(conn, OUTGOING_RELATIONS, &n.id)?
        .into_iter()
        .filter(|c| c.rel_type == "companies" || c.rel_type == "people")
        .map(|c| c.target)
        .collect();
    Ok(DocumentView {
        id: n.id.clone(),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        doc_type: d.doc_type,
        copy: d.copy,
        status: d.status,
        expires: d.expires.as_deref().and_then(date),
        location,
        holder: entity_ref(conn, holder_id, d.holder)?,
        last_holder: entity_ref(conn, last_id, d.last_holder)?,
        custody: custody_items(conn, "c.document_id = ?1", [&n.id])?,
        copies,
        concerns,
    })
}

const NESTED_PLACES: &str = "WITH RECURSIVE sub(id) AS (
        SELECT ?1 UNION SELECT p.note_id FROM places p JOIN sub ON p.parent_id = sub.id)";

fn place_view(conn: &Connection, n: &crate::store::notes::NoteRow) -> CoreResult<PlaceView> {
    let parsed = format::parse_note(&n.path, &n.content);
    let sub_ids: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT p.note_id FROM places p JOIN notes n ON n.id = p.note_id
             WHERE p.parent_id = ?1 AND n.deleted = 0 ORDER BY n.title, n.id",
        )?;
        st.query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let mut sub_places = Vec::new();
    for s in sub_ids {
        sub_places.extend(entity_ref(conn, Some(s), None)?);
    }
    let doc_ids: Vec<String> = {
        let mut st = conn.prepare(&format!(
            "{NESTED_PLACES}
             SELECT d.note_id FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE n.deleted = 0 AND d.location_id IN (SELECT id FROM sub)
             ORDER BY n.title, n.id"
        ))?;
        st.query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let mut documents = Vec::new();
    for d in doc_ids {
        documents.push(document_brief(conn, &d)?);
    }
    let mut recent = {
        let all: Vec<String> = {
            let mut st = conn.prepare(&format!("{NESTED_PLACES} SELECT id FROM sub"))?;
            st.query_map([&n.id], |r| r.get(0))?
                .collect::<Result<_, _>>()?
        };
        let mut items = Vec::new();
        for place in all {
            items.extend(custody_items(conn, "c.place_id = ?1", [&place])?);
        }
        items
    };
    recent.sort_by(|a, b| b.date.cmp(&a.date));
    recent.truncate(20);
    Ok(PlaceView {
        id: n.id.clone(),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        breadcrumb: breadcrumb(conn, &n.id, false)?,
        sub_places,
        documents,
        recent_movements: recent,
    })
}

/// Entity / document / place page.
pub fn entity_screen(conn: &Connection, id: &str) -> CoreResult<EntityScreen> {
    let mut screen = EntityScreen {
        id: id.to_owned(),
        kind: EntityPageKind::NotFound,
        entity: None,
        document: None,
        place: None,
    };
    let Some(n) = crate::store::notes::get(conn, id)?.filter(|n| !n.deleted) else {
        return Ok(screen);
    };
    match n.kind.as_str() {
        "person" | "company" => {
            screen.kind = EntityPageKind::Entity;
            screen.entity = Some(entity_view(conn, &n)?);
        }
        "document" => {
            screen.kind = EntityPageKind::Document;
            screen.document = Some(document_view(conn, &n)?);
        }
        "place" => {
            screen.kind = EntityPageKind::Place;
            screen.place = Some(place_view(conn, &n)?);
        }
        _ => {}
    }
    Ok(screen)
}

// ---------------------------------------------------------------------------------------------
// Graph, ask, settings, session
// ---------------------------------------------------------------------------------------------

/// Ask: needs the server's `/ask` stream (not in the contract yet) and a connection.
pub fn ask(ctx: &ViewCtx) -> AskView {
    AskView {
        availability: availability_online(ctx, "ask"),
        messages: Vec::new(),
    }
}

/// The account summary.
pub fn account_summary(conn: &Connection) -> CoreResult<Option<AccountSummary>> {
    Ok(account::get(conn)?.map(|a| AccountSummary {
        is_admin: a.role == "admin",
        user_id: a.user_id,
        username: a.username,
        display_name: a.display_name,
        role: a.role,
        server_url: a.server_url,
        timezone: a.timezone,
        ui_language: a.ui_language,
    }))
}

/// Settings.
pub fn settings_view(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Option<SettingsView>> {
    let Some(account) = account_summary(conn)? else {
        return Ok(None);
    };
    let permission = match settings::get(conn, settings::NOTIFICATION_PERMISSION)?.as_deref() {
        Some("granted") => NotificationPermission::Granted,
        Some("denied") => NotificationPermission::Denied,
        _ => NotificationPermission::Unknown,
    };
    let scheduled: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scheduled_notifications WHERE state IN ('requested', 'scheduled')",
        [],
        |r| r.get(0),
    )?;
    let online = |feature: &str| availability_online(ctx, feature);
    let admin = if !account.is_admin {
        Availability::NotAllowed
    } else if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::Available
    };
    let devices = if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::Available
    };
    Ok(Some(SettingsView {
        reminders: RemindersSetting {
            enabled: settings::reminders_enabled(conn)?,
            permission,
            mode: ctx.notification_mode,
            scheduled: u32_of(scheduled),
            default_time: default_reminder_time(conn)?.format("%H:%M").to_string(),
        },
        devices,
        ai: online("ai_settings"),
        export: devices_like(ctx),
        integrity: online("integrity"),
        admin,
        account,
    }))
}

fn devices_like(ctx: &ViewCtx) -> Availability {
    if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::Available
    }
}

/// Whole days from `now` until `at` (0 when past).
pub fn days_until(now: DateTime<Utc>, at: DateTime<Utc>) -> u32 {
    let d = at - now;
    if d <= Duration::zero() {
        0
    } else {
        u32::try_from(d.num_days()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_skip_frontmatter_and_headings() {
        assert_eq!(
            snippet("---\nid: x\n---\n# Title\n\n- first point\nmore", 160),
            "first point"
        );
        assert_eq!(snippet("abcdef", 3), "abc…");
        assert_eq!(snippet("", 3), "");
    }

    #[test]
    fn days_until_rounds_down() {
        let now = ts("2026-09-27T10:00:00Z");
        assert_eq!(days_until(now, ts("2026-10-11T09:00:00Z")), 13);
        assert_eq!(days_until(now, ts("2026-09-26T09:00:00Z")), 0);
    }
}
