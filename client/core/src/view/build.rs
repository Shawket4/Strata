//! Builders: view-models from the local database. Pure reads; every list is sorted here
//! (L15: Dart never sorts or filters).

// SQL rows are read into tuples right where the query is written.
#![allow(clippy::type_complexity)]

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use vault_format::wikilink::{self, WikiLink};

use crate::error::CoreResult;
use crate::format::direction::dir_of;
use crate::format::labels::{self, Labels, Lang, NOTES, tr};
use crate::format::{self, hints};
use crate::store::{account, cache, conflicts, from_msgpack, outbox, settings, sync_state};
use crate::sync::model::DecodedPayload;
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
pub(crate) fn resolve_display(conn: &Connection, target: &str) -> CoreResult<EntityRef> {
    let title = vault_format::resolve::link_name(target).to_owned();
    let ids: Vec<(String, String)> = {
        let mut st = conn.prepare_cached(
            "SELECT id, kind FROM notes WHERE deleted = 0 AND (title = ?1 COLLATE NOCASE OR path = ?2)
             ORDER BY id LIMIT 2",
        )?;
        st.query_map(params![title, format!("{target}.md")], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<Result<_, _>>()?
    };
    let one = (ids.len() == 1).then(|| ids[0].clone());
    Ok(EntityRef {
        id: one.as_ref().map(|o| o.0.clone()),
        kind: one.map(|o| o.1),
        title,
    })
}

/// A reference to note `id` (title and kind from the cache), or to an unresolved link `raw`.
pub(crate) fn entity_ref(
    conn: &Connection,
    id: Option<String>,
    raw: Option<String>,
) -> CoreResult<Option<EntityRef>> {
    match (id, raw) {
        (Some(id), raw) => {
            let row: Option<(String, String)> = conn
                .query_row("SELECT title, kind FROM notes WHERE id = ?1", [&id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            Ok(Some(EntityRef {
                title: row
                    .as_ref()
                    .map(|r| r.0.clone())
                    .or(raw.map(|r| vault_format::resolve::link_name(&r).to_owned()))
                    .unwrap_or_default(),
                kind: row.map(|r| r.1),
                id: Some(id),
            }))
        }
        (None, Some(raw)) => Ok(Some(EntityRef {
            id: None,
            title: vault_format::resolve::link_name(&raw).to_owned(),
            kind: None,
        })),
        (None, None) => Ok(None),
    }
}

/// A reference to an existing note by ID.
pub(crate) fn note_ref(conn: &Connection, id: &str) -> CoreResult<EntityRef> {
    Ok(
        entity_ref(conn, Some(id.to_owned()), None)?.unwrap_or(EntityRef {
            id: Some(id.to_owned()),
            title: String::new(),
            kind: None,
        }),
    )
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
    labels: &Labels,
) -> rusqlite::Result<NoteListItem> {
    let id: String = r.get(0)?;
    let title: String = r.get(1)?;
    let content: String = r.get(4)?;
    let updated: String = r.get(5)?;
    let links: i64 = r.get(6)?;
    let snippet = snippet(&content, 160);
    let updated_at = ts(&updated);
    Ok(NoteListItem {
        pending_sync: note_pending(pending, &id),
        title_dir: dir_of(&title),
        snippet_dir: dir_of(&snippet),
        title,
        path: r.get(2)?,
        kind: r.get(3)?,
        snippet,
        tags: Vec::new(),
        updated_label: labels.list_label(updated_at),
        updated_at,
        link_count: u32_of(links),
        highlights: Vec::new(),
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

const NOTE_ITEM_COLUMNS: &str = "n.id, n.title, n.path, n.kind, n.content, n.local_updated_at, \
     (SELECT COUNT(*) FROM links l WHERE l.note_id = n.id) \
     + (SELECT COUNT(*) FROM relations r WHERE r.src_id = n.id)";

pub(crate) fn note_items(
    conn: &Connection,
    ctx: &ViewCtx,
    sql_tail: &str,
    p: impl rusqlite::Params,
) -> CoreResult<Vec<NoteListItem>> {
    let pending = pending_entities(conn)?;
    let labels = ctx.labels();
    let mut st = conn.prepare(&format!(
        "SELECT {NOTE_ITEM_COLUMNS} FROM notes n {sql_tail}"
    ))?;
    let mut items: Vec<NoteListItem> = st
        .query_map(p, |r| note_list_item(r, &pending, &labels))?
        .collect::<Result<_, _>>()?;
    fill_tags(conn, &mut items)?;
    Ok(items)
}

/// UTF-16 spans of `needles` (normalised, case-insensitive) in `text`.
pub(crate) fn highlight_spans(text: &str, needles: &[String]) -> Vec<HighlightSpan> {
    let mut out: Vec<HighlightSpan> = Vec::new();
    let lower: Vec<(usize, char)> = text.char_indices().collect();
    let folded: String = text.chars().flat_map(char::to_lowercase).collect();
    if folded.chars().count() != lower.len() {
        // Case folding changed the length (rare scripts): no highlights rather than wrong ones.
        return out;
    }
    let chars: Vec<char> = folded.chars().collect();
    let map = hints::Utf16Map::new(text);
    for needle in needles {
        let n: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
        if n.is_empty() || n.len() > chars.len() {
            continue;
        }
        let mut i = 0;
        while i + n.len() <= chars.len() {
            if chars[i..i + n.len()] == n[..] {
                let start = lower[i].0;
                let end = lower.get(i + n.len()).map_or(text.len(), |c| c.0);
                out.push(HighlightSpan {
                    start: map.at(start),
                    end: map.at(end),
                });
                i += n.len();
            } else {
                i += 1;
            }
        }
    }
    out.sort_by_key(|s| (s.start, s.end));
    out.dedup();
    out
}

// ---------------------------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------------------------

/// The sync pill: one display state chosen by priority (conflict > duplicates > syncing >
/// paused > offline > error > synced) with its label and progress.
pub fn sync_pill(conn: &Connection, ctx: &ViewCtx) -> CoreResult<SyncPill> {
    let s = sync_state::get(conn)?;
    let count = |sql: &str| -> CoreResult<u32> {
        Ok(u32_of(conn.query_row(sql, [], |r| r.get::<_, i64>(0))?))
    };
    let labels = ctx.labels();
    let lang = ctx.lang;
    let pending_ops = outbox::unsynced_count(conn)?;
    let conflicts = count("SELECT COUNT(*) FROM conflicts")?;
    let duplicates = count("SELECT COUNT(*) FROM duplicates")?;
    let a = &ctx.activity;
    let (done, total) = match a.phase {
        SyncPhase::Pushing => (a.ops_done, a.ops_total),
        SyncPhase::Bootstrapping => (a.pages_done, a.pages_total.unwrap_or(0)),
        SyncPhase::Pulling => (a.pulled, 0),
        _ => (0, 0),
    };
    let syncing = !matches!(a.phase, SyncPhase::Idle | SyncPhase::Backoff);
    let display = if conflicts > 0 {
        SyncPillKind::Conflict
    } else if duplicates > 0 {
        SyncPillKind::Duplicates
    } else if syncing {
        SyncPillKind::Syncing
    } else if s.paused {
        SyncPillKind::Paused
    } else if ctx.connectivity == Connectivity::Offline {
        SyncPillKind::Offline
    } else if s.last_error.is_some() {
        SyncPillKind::Error
    } else {
        SyncPillKind::Synced
    };
    let queued = |base: &str, base_ar: &str| -> String {
        if pending_ops == 0 {
            tr(lang, base, base_ar)
        } else {
            match lang {
                Lang::En => format!("{base} · {pending_ops} queued"),
                Lang::Ar => format!("{base_ar} · {pending_ops} في الانتظار"),
            }
        }
    };
    let label = match display {
        SyncPillKind::Conflict => labels::CONFLICTS.of(i64::from(conflicts), lang),
        SyncPillKind::Duplicates => match lang {
            Lang::En => format!("{duplicates} to review"),
            Lang::Ar => format!("{duplicates} للمراجعة"),
        },
        SyncPillKind::Syncing if total > 0 => match lang {
            Lang::En => format!("Syncing {done}/{total}"),
            Lang::Ar => format!("مزامنة {done}/{total}"),
        },
        SyncPillKind::Syncing => tr(lang, "Syncing…", "جارٍ المزامنة…"),
        SyncPillKind::Paused => queued("Paused", "متوقفة"),
        SyncPillKind::Offline => queued("Offline", "غير متصل"),
        SyncPillKind::Error => queued("Sync failed", "فشلت المزامنة"),
        SyncPillKind::Synced => queued("Synced", "تمت المزامنة"),
    };
    let last_sync_at = s.last_pull_at.as_deref().map(ts);
    Ok(SyncPill {
        connectivity: ctx.connectivity,
        activity: ctx.activity.clone(),
        pending_ops,
        conflicts,
        duplicates,
        last_sync_label: last_sync_at.map(|t| labels.moment_label(t)),
        last_sync_at,
        display,
        progress_done: if display == SyncPillKind::Syncing {
            done
        } else {
            0
        },
        progress_total: if display == SyncPillKind::Syncing {
            total
        } else {
            0
        },
        label,
    })
}

fn note_title(conn: &Connection, id: &str) -> CoreResult<Option<String>> {
    Ok(conn
        .query_row("SELECT title FROM notes WHERE id = ?1", [id], |r| r.get(0))
        .optional()?)
}

fn task_description(conn: &Connection, id: &str) -> CoreResult<String> {
    Ok(conn
        .query_row("SELECT description FROM tasks WHERE id = ?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .optional()?
        .unwrap_or_else(|| id.to_owned()))
}

/// What an outbox op does, in one line.
fn op_detail(conn: &Connection, op: &outbox::OutboxOp, lang: Lang) -> CoreResult<String> {
    use crate::sync::model::Op;
    let title_of = |id: &ulid::Ulid| -> CoreResult<String> {
        Ok(note_title(conn, &id.to_string())?.unwrap_or_default())
    };
    Ok(match &op.op {
        Op::NoteUpdate(u) => match &op.base_content {
            Some(base) => format::diff::summary(base, &u.content, lang),
            None => tr(lang, "Edited", "تعديل"),
        },
        Op::NoteCreate(p) => format::title_of(&p.path),
        Op::Capture(c) => c
            .text
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(120)
            .collect(),
        Op::NoteMove(m) => format!("→ {}", m.new_path),
        Op::NoteDelete(_) => tr(lang, "Deleted", "حذف"),
        Op::RelationAdd(r) => format!(
            "{} → {}",
            labels::relation_label(r.relation.as_str(), lang),
            title_of(&r.dst_id)?
        ),
        Op::RelationRemove(r) => format!(
            "− {} → {}",
            labels::relation_label(r.relation.as_str(), lang),
            title_of(&r.dst_id)?
        ),
        Op::RelationRetype(r) => format!(
            "{} → {} ({})",
            labels::relation_label(r.relation.as_str(), lang),
            labels::relation_label(r.new_type.as_str(), lang),
            title_of(&r.dst_id)?
        ),
        Op::EntityCreate(p) => p.name.clone(),
        Op::DocumentCreate(p) => p.name.clone(),
        Op::PlaceCreate(p) => p.name.clone(),
        Op::TaskCreate(p) => p.text.clone(),
        Op::TaskUpdate(p) => match &p.text {
            Some(t) => t.clone(),
            None => task_description(conn, &p.id)?,
        },
        Op::TaskComplete(p) => format!("✓ {}", task_description(conn, &p.id)?),
        Op::TaskCancel(p) => format!("✕ {}", task_description(conn, &p.id)?),
        Op::TaskReopen(p) | Op::TaskDelete(p) => task_description(conn, &p.id)?,
        Op::SuggestionReply(r) => r.text.clone(),
        Op::DocumentCustody(c) => c.event.as_str().to_owned(),
        Op::EntityMerge(m) => format!("→ {}", title_of(&m.into_id)?),
        _ => String::new(),
    })
}

/// The sync status screen.
pub fn sync_status(conn: &Connection, ctx: &ViewCtx) -> CoreResult<SyncStatusView> {
    let s = sync_state::get(conn)?;
    let labels = ctx.labels();
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
        let detail = op_detail(conn, &op, ctx.lang)?;
        let created = ts(&op.created);
        items.push(OutboxItem {
            op_id: op.op_id.clone(),
            kind: op.kind().as_str().to_owned(),
            title,
            status,
            attempts: op.attempts,
            created,
            detail_dir: dir_of(&detail),
            detail,
            created_label: labels.moment_label(created),
        });
    }
    let mut conflict_items = Vec::new();
    for c in conflicts::conflicts(conn)? {
        let created = ts(&c.created);
        conflict_items.push(ConflictItem {
            title: note_title(conn, &c.entity_id)?.unwrap_or_default(),
            op_id: c.op_id,
            note_id: c.entity_id,
            created,
            created_label: labels.moment_label(created),
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
    let (retry_interval_secs, next_retry_label, retry_label) =
        match (ctx.activity.phase, ctx.activity.retry_at) {
            (SyncPhase::Backoff, Some(at)) => {
                let secs = u32::try_from(
                    crate::sync::engine::backoff_delay(s.consecutive_failures).as_secs(),
                )
                .unwrap_or(u32::MAX);
                let next = labels.hms(at);
                let label = match ctx.lang {
                    Lang::En => format!("Retrying automatically every {secs} s · next at {next}"),
                    Lang::Ar => format!("إعادة المحاولة تلقائيًا كل {secs} ث · التالية {next}"),
                };
                (Some(secs), Some(next), Some(label))
            }
            _ => (None, None, None),
        };
    let log = cache::log_entries(conn)?
        .into_iter()
        .map(|(at, kind, detail)| {
            let at = ts(&at);
            SyncLogItem {
                at_label: labels.hms(at),
                at,
                kind,
                detail,
            }
        })
        .collect();
    Ok(SyncStatusView {
        pill: sync_pill(conn, ctx)?,
        bootstrap_complete: s.bootstrap_complete,
        last_error: s.last_error,
        outbox: items,
        conflicts: conflict_items,
        rejections,
        paused: s.paused,
        retry_interval_secs,
        next_retry_label,
        retry_label,
        log,
    })
}

fn hunk_location_label(loc: &sync_model::Location, lang: Lang) -> String {
    match loc {
        sync_model::Location::Frontmatter { key } => match lang {
            Lang::En => format!("Property “{key}”"),
            Lang::Ar => format!("الخاصية «{key}»"),
        },
        sync_model::Location::Body { ours_line, .. } => match lang {
            Lang::En => format!("Line {ours_line}"),
            Lang::Ar => format!("السطر {ours_line}"),
        },
        sync_model::Location::LineEndings => tr(lang, "Line endings", "نهايات الأسطر"),
        sync_model::Location::ByteOrderMark => tr(lang, "Byte order mark", "علامة ترتيب البايت"),
    }
}

/// Choices valid for a hunk: every hunk can take a side or the base; only body lines can be
/// combined or replaced by typed text (`sync-model` `Choice`), and a task placed on both
/// sides cannot be kept twice.
pub fn allowed_choices(
    location: &sync_model::Location,
    kind: sync_model::ConflictKind,
) -> Vec<HunkChoiceKind> {
    let mut v = vec![
        HunkChoiceKind::Ours,
        HunkChoiceKind::Theirs,
        HunkChoiceKind::Base,
    ];
    if matches!(location, sync_model::Location::Body { .. }) {
        if kind != sync_model::ConflictKind::TaskPlacement {
            v.extend([
                HunkChoiceKind::OursThenTheirs,
                HunkChoiceKind::TheirsThenOurs,
            ]);
        }
        v.push(HunkChoiceKind::Text);
    }
    v
}

/// One conflict.
pub fn conflict_screen(
    conn: &Connection,
    ctx: &ViewCtx,
    op_id: &str,
) -> CoreResult<ConflictScreen> {
    let Some(c) = conflicts::conflict(conn, op_id)? else {
        return Ok(ConflictScreen {
            op_id: op_id.to_owned(),
            conflict: None,
        });
    };
    let lang = ctx.lang;
    let labels = ctx.labels();
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
                location_label: hunk_location_label(&h.location, lang),
                allowed_choices: allowed_choices(&h.location, h.kind),
            })
            .collect(),
        _ => Vec::new(),
    };
    let base = c.base_content.clone().unwrap_or_default();
    let local = c.local_content.clone().unwrap_or_default();
    let server = c.server_content.clone();
    let (base_lines, local_lines, server_lines) = match &server {
        Some(sv) => (
            format::diff::annotate_base(&base, &local, sv),
            format::diff::annotate_side(&base, &local, sv),
            format::diff::annotate_side(&base, sv, &local),
        ),
        None => (
            format::diff::annotate_base(&base, &local, &base),
            format::diff::annotate_side(&base, &local, &base),
            Vec::new(),
        ),
    };
    let created = ts(&c.created);
    let local_origin_label = match lang {
        Lang::En => format!(
            "This device · {} · edited offline",
            labels.moment_with_day(created)
        ),
        Lang::Ar => format!(
            "هذا الجهاز · {} · تعديل دون اتصال",
            labels.moment_with_day(created)
        ),
    };
    let server_origin_label = if server.is_some() {
        tr(lang, "Server", "الخادم")
    } else {
        tr(lang, "Server · not pulled yet", "الخادم · لم يُسحب بعد")
    };
    let conflict_copy_path = match &c.resolution {
        Some(sync_model::ConflictResolution::ConflictCopy { path, .. }) => Some(path.clone()),
        _ => None,
    };
    let path = conn
        .query_row(
            "SELECT path FROM notes WHERE id = ?1",
            [&c.entity_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_default();
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
            path,
            local_origin_label,
            server_origin_label,
            base_lines,
            local_lines,
            server_lines,
            conflict_copy_path,
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
        HunkChoiceKind::Text => {
            // The user's own text replaces whole lines: it ends with a line terminator.
            let mut t = text.unwrap_or_default();
            if !t.is_empty() && !t.ends_with('\n') {
                t.push('\n');
            }
            sync_model::Choice::Text(t)
        }
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
    note_path: String,
    line_no: i64,
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

const TASK_COLUMNS: &str = "t.id, t.note_id, n.title, n.path, t.line_no, t.line, t.description, \
                            t.status, t.priority, t.due, t.scheduled, t.done_at, t.recurrence_raw, \
                            t.recurrence_error";

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
                note_path: r.get(3)?,
                line_no: r.get(4)?,
                line: r.get(5)?,
                description: r.get(6)?,
                status: r.get(7)?,
                priority: r.get(8)?,
                due: r.get(9)?,
                scheduled: r.get(10)?,
                done: r.get(11)?,
                recurrence_raw: r.get(12)?,
                recurrence_error: r.get(13)?,
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
    let labels = ctx.labels();
    let due = row.due.as_deref().and_then(date);
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
            let local_at = day.and_time(time.unwrap_or(default_time));
            reminders.push(ReminderItem {
                local: if t.is_empty() { d } else { format!("{d} {t}") },
                at,
                time_label: labels.local_time_label(local_at),
                offset_label: labels.reminder_offset(due, day),
                local_at,
            });
        }
    }
    let mut links = Vec::new();
    for l in wikilink::find_all(&row.line) {
        if !l.embed {
            links.push(resolve_display(conn, l.target())?);
        }
    }
    let state = match row.status.as_str() {
        "done" => TaskState::Done,
        "cancelled" => TaskState::Cancelled,
        _ => TaskState::Open,
    };
    let done = row.done.as_deref().and_then(date);
    let open = state == TaskState::Open;
    let is_overdue = open && due.is_some_and(|d| d < labels.today());
    let origin_label = {
        let mut from_doc = false;
        for l in &links {
            if l.kind.as_deref() == Some("document")
                && let Some(id) = &l.id
            {
                let expires: Option<Option<String>> = conn
                    .query_row(
                        "SELECT expires FROM documents WHERE note_id = ?1",
                        [id],
                        |r| r.get(0),
                    )
                    .optional()?;
                from_doc |= expires.flatten().is_some();
            }
        }
        from_doc.then(|| tr(ctx.lang, "From document expiry", "من تاريخ انتهاء المستند"))
    };
    Ok(TaskItem {
        pending_sync: note_pending(pending, &row.note_id),
        state,
        due,
        scheduled: row.scheduled.as_deref().and_then(date),
        done,
        recurrence_understood: row.recurrence_raw.is_none() || row.recurrence_error.is_none(),
        due_label: due.map(|d| labels.relative_day(d)),
        lateness_label: if is_overdue {
            due.and_then(|d| labels.lateness(d))
        } else {
            None
        },
        completion_label: match (state, done, due) {
            (TaskState::Done, Some(done), Some(due)) => Some(labels.completion(done, due)),
            _ => None,
        },
        next_in_label: match (
            open,
            &row.recurrence_raw,
            due.or(row.scheduled.as_deref().and_then(date)),
        ) {
            (true, Some(_), Some(d)) if d >= labels.today() => Some(labels.next_in(d)),
            _ => None,
        },
        origin_label,
        is_overdue,
        description_dir: dir_of(&row.description),
        line_number: u32_of(row.line_no) + 1,
        note_path: row.note_path,
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
/// description; upcoming tasks are also grouped by day for the next week, then "later".
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
    let labels = ctx.labels();
    let week_end = today + Duration::days(7);
    for t in &s.upcoming {
        let day = t.due.or(t.scheduled);
        let key = day.filter(|d| *d <= week_end);
        match s.upcoming_groups.last_mut() {
            Some(g) if g.date == key => g.tasks.push(t.clone()),
            _ => s.upcoming_groups.push(TaskGroup {
                label: key.map_or_else(|| labels.later(), |d| labels.group_header(d)),
                date: key,
                tasks: vec![t.clone()],
            }),
        }
    }
    s.today_count = u32_of(i64::try_from(s.overdue.len() + s.today.len()).unwrap_or(0));
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
    let labels = ctx.labels();
    let week_start = labels.week_start().format("%Y-%m-%d").to_string();
    let count = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> CoreResult<u32> {
        Ok(u32_of(conn.query_row(sql, p, |r| r.get::<_, i64>(0))?))
    };
    let open_count = count(
        "SELECT COUNT(*) FROM tasks t JOIN notes n ON n.id = t.note_id
         WHERE t.status = 'open' AND n.deleted = 0",
        &[],
    )?;
    let done_this_week = count(
        "SELECT COUNT(*) FROM tasks t JOIN notes n ON n.id = t.note_id
         WHERE t.status = 'done' AND n.deleted = 0 AND t.done_at >= ?1",
        &[&week_start],
    )?;
    let notes_with_tasks = count(
        "SELECT COUNT(DISTINCT t.note_id) FROM tasks t JOIN notes n ON n.id = t.note_id
         WHERE t.status = 'open' AND n.deleted = 0",
        &[],
    )?;
    Ok(TasksView {
        sections: task_sections(conn, ctx)?,
        done,
        open_count,
        done_this_week,
        done_this_week_label: match ctx.lang {
            Lang::En => format!("{done_this_week} done this week"),
            Lang::Ar => format!("{done_this_week} منجزة هذا الأسبوع"),
        },
        notes_with_tasks,
    })
}

/// Devices reminders go to (`Pixel 9, MacBook Pro`), from the cached device list.
fn delivery_label(conn: &Connection) -> CoreResult<Option<String>> {
    Ok(
        cache::get::<Vec<crate::net::DeviceInfo>>(conn, cache::DEVICES)?.map(|(d, _)| {
            d.iter()
                .filter(|d| d.reminders_enabled)
                .map(|d| d.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }),
    )
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
            location_label: String::new(),
            delivery_label: None,
            next_occurrence_label: None,
            recurrence_form: None,
            recurrence_preview: Vec::new(),
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
    let labels = ctx.labels();
    let rule = task
        .recurrence
        .as_deref()
        .and_then(|r| vault_format::tasks::parse_recurrence(r).ok());
    let anchor = task
        .due
        .or(task.scheduled)
        .unwrap_or_else(|| labels.today());
    let location_label = match ctx.lang {
        Lang::En => format!("{} · line {}", task.note_path, task.line_number),
        Lang::Ar => format!("{} · السطر {}", task.note_path, task.line_number),
    };
    Ok(TaskScreen {
        id: id.to_owned(),
        location_label,
        delivery_label: if task.reminders.is_empty() {
            None
        } else {
            delivery_label(conn)?
        },
        next_occurrence_label: match (&rule, task.state) {
            (Some(_), TaskState::Open) => Some(labels.next_occurrence(anchor)),
            _ => None,
        },
        recurrence_form: rule.as_ref().map(format::recurrence::form_of),
        recurrence_preview: rule
            .as_ref()
            .map(|r| format::recurrence::preview(r, anchor, 3, &labels))
            .unwrap_or_default(),
        task: Some(task),
        line,
        history,
    })
}

// ---------------------------------------------------------------------------------------------
// Home, inbox, suggestions
// ---------------------------------------------------------------------------------------------

/// One-line summary of what a suggestion proposes (Home inbox preview).
fn suggestion_summary(d: &SuggestionDetail, lang: Lang) -> String {
    match d.kind {
        SuggestionKind::Filing => format!("→ {}", d.title),
        SuggestionKind::EntityLink => match lang {
            Lang::En => format!("Who is “{}”?", d.mention),
            Lang::Ar => format!("من هو «{}»؟", d.mention),
        },
        SuggestionKind::Custody | SuggestionKind::Task => d.line.clone(),
        SuggestionKind::Duplicate => match (lang, d.duplicates.first()) {
            (Lang::En, Some(c)) => format!("Already exists · {}", c.title),
            (Lang::Ar, Some(c)) => format!("موجود بالفعل · {}", c.title),
            _ => tr(lang, "Already exists", "موجود بالفعل"),
        },
        SuggestionKind::Correction => d.question.clone().unwrap_or_else(|| d.title.clone()),
        SuggestionKind::Conflict => match lang {
            Lang::En => format!("Conflict copy · {}", d.folder),
            Lang::Ar => format!("نسخة تعارض · {}", d.folder),
        },
        SuggestionKind::Duplicates => format!(
            "{} ≈ {}",
            d.title,
            d.other
                .as_ref()
                .map(|o| o.title.clone())
                .unwrap_or_default()
        ),
        SuggestionKind::Unsupported => String::new(),
    }
}

/// Whether a suggestion is a conflict (Inbox "Conflicts" tab).
fn is_conflict(s: &SuggestionItem) -> bool {
    s.detail.kind == SuggestionKind::Conflict
}

fn open_items(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Vec<OpenItem>> {
    let rows: Vec<(String, String, String)> = {
        let mut st = conn.prepare(
            "SELECT n.id, n.path, n.content FROM notes n JOIN entities e ON e.note_id = n.id
             WHERE n.deleted = 0 AND e.kind IN ('person', 'company')
             ORDER BY n.local_updated_at DESC, n.id",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (id, path, content) in rows {
        let parsed = format::parse_note(&path, &content);
        let section = vault_format::sections::sections(&parsed.body)
            .into_iter()
            .find(|s| s.level == 2 && s.title.eq_ignore_ascii_case("Open items"))
            .map(|s| parsed.body[s.own_content_span].to_owned());
        let Some(section) = section else { continue };
        let person = note_ref(conn, &id)?;
        for (i, b) in cited_bullets(conn, ctx, &section)?.into_iter().enumerate() {
            out.push(OpenItem {
                id: format!("{id}:{i}"),
                text_dir: b.dir,
                text: b.text,
                person: person.clone(),
                citation: b.citations.into_iter().next(),
                done: false,
            });
            if out.len() >= 10 {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// Pinned notes, in pin order.
pub fn pinned_notes(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Vec<NoteListItem>> {
    let mut out = Vec::new();
    for id in cache::pinned(conn)? {
        out.extend(note_items(
            conn,
            ctx,
            "WHERE n.id = ?1 AND n.deleted = 0",
            [&id],
        )?);
    }
    Ok(out)
}

/// Home.
pub fn home(conn: &Connection, ctx: &ViewCtx) -> CoreResult<HomeView> {
    let labels = ctx.labels();
    let lang = ctx.lang;
    let recent_notes = note_items(
        conn,
        ctx,
        "WHERE n.deleted = 0 AND n.path NOT LIKE 'inbox/%'
         ORDER BY n.local_updated_at DESC, n.id DESC LIMIT 10",
        [],
    )?;
    let inbox_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM inbox i JOIN notes n ON n.id = i.note_id WHERE n.deleted = 0",
        [],
        |r| r.get(0),
    )?;
    let display_name = account::get(conn)?
        .map(|a| a.display_name)
        .unwrap_or_default();
    let inbox_view = inbox(conn, ctx, InboxFilter::All)?;
    let inbox_preview = inbox_view
        .captures
        .iter()
        .take(3)
        .map(|c| {
            let text: String = c
                .text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(120)
                .collect();
            InboxPreviewItem {
                note_id: c.note_id.clone(),
                text_dir: dir_of(&text),
                text,
                summary: c
                    .suggestions
                    .first()
                    .map(|s| suggestion_summary(&s.detail, lang))
                    .unwrap_or_default(),
                needs_you: c.needs_you,
            }
        })
        .collect();
    // Contradictions are AI relations (decisions), not suggestions: the ones in the activity
    // feed that are still in place.
    let (ai_activity, ai_activity_items, ai_activity_headline) = ai_activity(conn, ctx)?;
    let contradictions_count = u32_of(
        i64::try_from(
            ai_activity_items
                .iter()
                .filter(|a| a.kind == "contradiction" && !a.reverted)
                .count(),
        )
        .unwrap_or(0),
    );
    let needs_you_count = inbox_view.needs_you_count;
    let mut parts = Vec::new();
    if needs_you_count > 0 {
        parts.push(match lang {
            Lang::En => format!("{needs_you_count} needs you"),
            Lang::Ar => format!("{needs_you_count} بانتظارك"),
        });
    }
    if contradictions_count > 0 {
        parts.push(match (lang, contradictions_count) {
            (Lang::En, 1) => "1 contradiction to review".to_owned(),
            (Lang::En, n) => format!("{n} contradictions to review"),
            (Lang::Ar, n) => format!("{n} تناقض للمراجعة"),
        });
    }
    let open_item_list = open_items(conn, ctx)?;
    Ok(HomeView {
        recent_notes,
        inbox_count: u32_of(inbox_count),
        tasks: task_sections(conn, ctx)?,
        sync: sync_pill(conn, ctx)?,
        today_label: labels.today_long(),
        greeting: labels.greeting(&display_name),
        display_name,
        inbox_preview,
        needs_you_count,
        contradictions_count,
        inbox_summary: parts.join(" · "),
        ai_activity,
        ai_activity_items,
        ai_activity_headline,
        open_items: Availability::Available,
        open_item_list,
        pinned: pinned_notes(conn, ctx)?,
    })
}

/// The AI activity feed from the cached decisions (newest first, ≤ 20) and today's headline.
fn ai_activity(
    conn: &Connection,
    ctx: &ViewCtx,
) -> CoreResult<(Availability, Vec<AiActivityItem>, String)> {
    let Some((decisions, _)) =
        cache::get::<Vec<crate::net::AiDecisionInfo>>(conn, cache::AI_DECISIONS)?
    else {
        let a = if ctx.connectivity == Connectivity::Offline {
            Availability::Offline
        } else {
            // Not fetched yet (`refresh_ai_activity`).
            Availability::Available
        };
        return Ok((a, Vec::new(), String::new()));
    };
    let labels = ctx.labels();
    let today = labels.today();
    let (mut added, mut contradictions) = (0u32, 0u32);
    let mut items = Vec::new();
    for d in decisions.iter().take(20) {
        let contradiction = d.rel_type.as_deref() == Some("contradicts");
        let kind = match d.kind.as_str() {
            "relation" if contradiction => "contradiction",
            "relation" => "relation_added",
            "custody_event" => "custody_applied",
            other => other,
        };
        if labels.local(d.created).date() == today && d.reverted_at.is_none() {
            match kind {
                "contradiction" => contradictions += 1,
                "relation_added" => added += 1,
                _ => {}
            }
        }
        let source = match &d.source_note_id {
            Some(id) => entity_ref(conn, Some(id.clone()), d.source_title.clone())?,
            None => None,
        };
        let target = if ulid::Ulid::from_string(&d.target_id).is_ok() {
            entity_ref(conn, Some(d.target_id.clone()), d.target_name.clone())?
        } else {
            d.target_name.clone().map(|t| EntityRef {
                id: None,
                title: t,
                kind: None,
            })
        };
        items.push(AiActivityItem {
            at_label: labels.moment_label(d.created),
            kind: kind.to_owned(),
            summary: d.summary.clone(),
            source,
            target,
            rel_type: d.rel_type.clone(),
            confidence: d.confidence,
            undo_suggestion_id: d.suggestion_id.clone(),
            decision_id: d.id.clone(),
            reverted: d.reverted_at.is_some(),
        });
    }
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(match ctx.lang {
            Lang::En => format!("{} added", labels::RELATIONS.of(i64::from(added), Lang::En)),
            Lang::Ar => format!("أُضيفت {}", labels::RELATIONS.of(i64::from(added), Lang::Ar)),
        });
    }
    if contradictions > 0 {
        parts.push(match (ctx.lang, contradictions) {
            (Lang::En, 1) => "1 contradiction found".to_owned(),
            (Lang::En, n) => format!("{n} contradictions found"),
            (Lang::Ar, n) => format!("وُجد {n} تناقض"),
        });
    }
    let sep = if ctx.lang == Lang::Ar { "، " } else { ", " };
    Ok((Availability::Available, items, parts.join(sep)))
}

/// Navigation counts and pinned notes.
pub fn nav(conn: &Connection, ctx: &ViewCtx) -> CoreResult<NavView> {
    let count = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> CoreResult<u32> {
        Ok(u32_of(conn.query_row(sql, p, |r| r.get::<_, i64>(0))?))
    };
    let today = today(ctx).format("%Y-%m-%d").to_string();
    Ok(NavView {
        inbox_count: count(
            "SELECT COUNT(*) FROM inbox i JOIN notes n ON n.id = i.note_id WHERE n.deleted = 0",
            &[],
        )?,
        tasks_due_count: count(
            "SELECT COUNT(*) FROM tasks t JOIN notes n ON n.id = t.note_id
             WHERE t.status = 'open' AND n.deleted = 0
               AND (t.due <= ?1 OR (t.due IS NULL AND t.scheduled <= ?1))",
            &[&today],
        )?,
        notes_count: count("SELECT COUNT(*) FROM notes WHERE deleted = 0", &[])?,
        directory_count: count(
            "SELECT COUNT(*) FROM entities e JOIN notes n ON n.id = e.note_id
             WHERE n.deleted = 0 AND e.kind IN ('person', 'company', 'document', 'place')",
            &[],
        )?,
        cluster_count: count("SELECT COUNT(*) FROM cluster_names", &[])?,
        pinned: pinned_notes(conn, ctx)?,
        sync: sync_pill(conn, ctx)?,
    })
}

/// The "Recent" block with a filter.
pub fn recent(
    conn: &Connection,
    ctx: &ViewCtx,
    filter: RecentFilter,
) -> CoreResult<RecentNotesView> {
    let tail = match filter {
        RecentFilter::Edited => {
            "WHERE n.deleted = 0 AND n.path NOT LIKE 'inbox/%'
             ORDER BY n.local_updated_at DESC, n.id DESC LIMIT 20"
        }
        RecentFilter::Created => {
            "WHERE n.deleted = 0 AND n.path NOT LIKE 'inbox/%'
             ORDER BY COALESCE(n.created, '') DESC, n.id DESC LIMIT 20"
        }
        RecentFilter::FiledByAi => {
            "WHERE n.deleted = 0 AND n.path NOT LIKE 'inbox/%' AND n.id IN (
                SELECT note_id FROM suggestions WHERE kind = 'filing' AND status = 'accepted')
             ORDER BY n.local_updated_at DESC, n.id DESC LIMIT 20"
        }
    };
    Ok(RecentNotesView {
        filter,
        notes: note_items(conn, ctx, tail, [])?,
    })
}

fn candidate_item(c: dedupe::DuplicateCandidate) -> CandidateItem {
    crate::session::candidate_item(c)
}

/// Why a candidate matched.
pub fn match_reason(level: &str, lang: Lang) -> String {
    match level {
        "exact" => tr(lang, "Same title", "نفس العنوان"),
        "near" => tr(lang, "Very similar text", "نص مشابه جدًا"),
        _ => tr(lang, "Similar meaning", "معنى مشابه"),
    }
}

/// Fills a candidate's path and reason from the cache.
pub(crate) fn complete_candidate(
    conn: &Connection,
    mut c: CandidateItem,
    lang: Lang,
) -> CoreResult<CandidateItem> {
    c.path = conn
        .query_row("SELECT path FROM notes WHERE id = ?1", [&c.id], |r| {
            r.get(0)
        })
        .optional()?;
    c.reason = match_reason(&c.match_level, lang);
    Ok(c)
}

fn duplicate_item(
    conn: &Connection,
    c: sync_model::suggestions::DuplicateItem,
    lang: Lang,
) -> CoreResult<CandidateItem> {
    complete_candidate(
        conn,
        CandidateItem {
            id: c.id.to_string(),
            kind: c.kind,
            title: c.title,
            snippet: c.snippet,
            match_level: c.match_level.as_str().to_owned(),
            score: c.score,
            path: None,
            reason: String::new(),
        },
        lang,
    )
}

/// A participant of a custody suggestion: the resolved entity, else the mention as written.
fn custody_target(
    conn: &Connection,
    t: &sync_model::suggestions::CustodyTarget,
) -> CoreResult<EntityRef> {
    let resolved = match t.id {
        Some(id) => entity_ref(conn, Some(id.to_string()), Some(t.mention.clone()))?,
        None => None,
    };
    Ok(resolved.unwrap_or_else(|| EntityRef {
        id: t.id.map(|i| i.to_string()),
        title: t.mention.clone(),
        kind: None,
    }))
}

fn ulid_ref(conn: &Connection, id: ulid::Ulid) -> CoreResult<EntityRef> {
    Ok(
        entity_ref(conn, Some(id.to_string()), None)?.unwrap_or_else(|| EntityRef {
            id: Some(id.to_string()),
            title: String::new(),
            kind: None,
        }),
    )
}

/// The custody line a suggestion proposes (`2026-09-20 — stored-at [[Desk drawer]]`) and the
/// location / holder / last holder it results in (`vault-format`'s custody rules, L16).
fn custody_preview(
    p: &sync_model::suggestions::CustodyPayload,
    place: Option<&EntityRef>,
    person: Option<&EntityRef>,
    counterparty: Option<&EntityRef>,
) -> (
    String,
    Option<EntityRef>,
    Option<EntityRef>,
    Option<EntityRef>,
) {
    use vault_format::custody::{CustodyEvent, CustodyState};
    let wikilink = |r: Option<&EntityRef>| r.map(|r| format!("[[{}]]", r.title));
    let Ok(kind) = p.event.parse::<domain::CustodyEventType>() else {
        return (
            format!("{} — {}", p.date.format("%Y-%m-%d"), p.event),
            None,
            None,
            None,
        );
    };
    let event = CustodyEvent {
        date: p.date,
        kind,
        place: wikilink(place),
        person: wikilink(person),
        counterparty: wikilink(counterparty),
        citations: Vec::new(),
    };
    let rendered = event.to_line();
    let line = rendered.strip_prefix("- ").unwrap_or(&rendered).to_owned();
    let state = CustodyState::derive(std::slice::from_ref(&event));
    let pick = |v: Option<&String>| -> Option<EntityRef> {
        let v = v?;
        [place, person, counterparty]
            .into_iter()
            .flatten()
            .find(|r| &format!("[[{}]]", r.title) == v)
            .cloned()
    };
    match state {
        Some(st) => (
            line,
            pick(st.location.as_ref()),
            pick(st.holder.as_ref()),
            pick(st.last_holder.as_ref()),
        ),
        None => (line, None, None, None),
    }
}

/// The task line a suggestion proposes (the shared preview: recurrence, then due).
fn task_line(p: &sync_model::suggestions::TaskPayload) -> String {
    item_render::task::suggestion_preview_line(p)
}

#[allow(clippy::too_many_lines)] // one arm per suggestion kind
fn suggestion_detail(
    conn: &Connection,
    ctx: &ViewCtx,
    p: DecodedPayload,
) -> CoreResult<SuggestionDetail> {
    use sync_model::SuggestionPayload as P;
    let lang = ctx.lang;
    let labels = ctx.labels();
    let date_label = |d: NaiveDate| labels.date_in_list(d);
    let p = match p {
        DecodedPayload::Known(p) => p,
        DecodedPayload::Other { kind } => {
            return Ok(SuggestionDetail {
                server_kind: kind,
                ..SuggestionDetail::of(SuggestionKind::Unsupported)
            });
        }
    };
    let detail = match p {
        P::Filing(f) => SuggestionDetail {
            title: f.title,
            folder: f.folder,
            tags: f.tags,
            decision_id: Some(f.decision_id.to_string()),
            ..SuggestionDetail::of(SuggestionKind::Filing)
        },
        P::EntityLink(e) => {
            let target = match e.proposed {
                Some(id) => Some(ulid_ref(conn, id)?),
                None => None,
            };
            let mut candidates = Vec::new();
            for id in e.proposed.into_iter().chain(e.candidates) {
                let r = ulid_ref(conn, id)?;
                if !candidates.contains(&r) {
                    candidates.push(r);
                }
            }
            SuggestionDetail {
                mention: e.mention,
                entity_kind: e.kind,
                target,
                candidates,
                is_nickname: e.is_nickname,
                confidence: Some(e.confidence),
                reason: e.reason,
                decision_id: Some(e.decision_id.to_string()),
                ..SuggestionDetail::of(SuggestionKind::EntityLink)
            }
        }
        P::Custody(c) => {
            let document = custody_target(conn, &c.document)?;
            let mut choices = Vec::new();
            for id in &c.document.candidates {
                choices.push(ulid_ref(conn, *id)?);
            }
            let place = c
                .place
                .as_ref()
                .map(|t| custody_target(conn, t))
                .transpose()?;
            let person = c
                .person
                .as_ref()
                .map(|t| custody_target(conn, t))
                .transpose()?;
            let counterparty = c
                .counterparty
                .as_ref()
                .map(|t| custody_target(conn, t))
                .transpose()?;
            let (line, location, holder, last_holder) =
                custody_preview(&c, place.as_ref(), person.as_ref(), counterparty.as_ref());
            SuggestionDetail {
                document: Some(document),
                line,
                date: Some(c.date),
                date_label: Some(date_label(c.date)),
                location,
                holder,
                last_holder,
                document_choices: choices,
                quote: c.quote,
                confidence: Some(c.confidence),
                reason: c.reason,
                decision_id: Some(c.decision_id.to_string()),
                ..SuggestionDetail::of(SuggestionKind::Custody)
            }
        }
        P::Task(t) => {
            let mut entities = Vec::new();
            for id in &t.entities {
                entities.push(ulid_ref(conn, *id)?);
            }
            SuggestionDetail {
                line: task_line(&t),
                title: t.title,
                date: t.due,
                date_label: t.due.map(date_label),
                recurrence: t.recurrence,
                entities,
                confidence: Some(t.confidence),
                decision_id: Some(t.decision_id.to_string()),
                ..SuggestionDetail::of(SuggestionKind::Task)
            }
        }
        P::Correction(c) => {
            let first = c.fixes.first();
            let target = match first.and_then(|f| f.new_target) {
                Some(id) => Some(ulid_ref(conn, id)?),
                None => None,
            };
            SuggestionDetail {
                title: c.message,
                question: c.question,
                rel_type: first.and_then(|f| f.new_type.clone()).unwrap_or_default(),
                target,
                reason: first.map(|f| f.reason.clone()).unwrap_or_default(),
                confidence: first.map(|f| f.confidence),
                decision_id: Some(c.decision_id.to_string()),
                ..SuggestionDetail::of(SuggestionKind::Correction)
            }
        }
        P::Duplicate(d) => {
            let mut v = Vec::new();
            for c in d.candidates {
                v.push(duplicate_item(conn, c, lang)?);
            }
            SuggestionDetail {
                duplicates: v,
                ..SuggestionDetail::of(SuggestionKind::Duplicate)
            }
        }
        P::Duplicates(d) => {
            let a = duplicate_item(conn, d.a, lang)?;
            let b = duplicate_item(conn, d.b, lang)?;
            SuggestionDetail {
                title: a.title.clone(),
                other: Some(EntityRef {
                    id: Some(b.id.clone()),
                    title: b.title.clone(),
                    kind: Some(b.kind.clone()),
                }),
                reason: d.reason.unwrap_or_default(),
                duplicates: vec![a, b],
                ..SuggestionDetail::of(SuggestionKind::Duplicates)
            }
        }
        P::Conflict(c) => SuggestionDetail {
            folder: c.copy_path.clone(),
            other: Some(EntityRef {
                id: Some(c.copy_id.to_string()),
                title: format::title_of(&c.copy_path),
                kind: Some("note".to_owned()),
            }),
            ..SuggestionDetail::of(SuggestionKind::Conflict)
        },
    };
    Ok(detail)
}

/// Whether only the user can decide a suggestion.
pub fn needs_you(d: &SuggestionDetail) -> bool {
    match d.kind {
        SuggestionKind::EntityLink
        | SuggestionKind::Duplicate
        | SuggestionKind::Duplicates
        | SuggestionKind::Conflict
        | SuggestionKind::Unsupported => true,
        SuggestionKind::Custody => !d.document_choices.is_empty(),
        SuggestionKind::Correction => d.question.is_some(),
        SuggestionKind::Filing | SuggestionKind::Task => false,
    }
}

#[derive(serde::Deserialize)]
struct ReplyRow {
    id: String,
    text: String,
    at: String,
    #[serde(default)]
    author: crate::sync::model::ReplyAuthor,
}

/// Suggestions (pending only, or all) with their details, labels and threads, newest first.
/// Pending lists include AI changes applied automatically that the user has not confirmed.
#[allow(clippy::too_many_lines)] // the row read with its thread
pub fn suggestion_items(
    conn: &Connection,
    ctx: &ViewCtx,
    only_pending: bool,
) -> CoreResult<Vec<SuggestionItem>> {
    let pending = pending_entities(conn)?;
    let labels = ctx.labels();
    let rows: Vec<(
        String,
        Option<String>,
        Vec<u8>,
        String,
        String,
        String,
        Option<Vec<u8>>,
        bool,
    )> = {
        let mut st = conn.prepare(
            "SELECT s.id, s.note_id, s.payload, s.status, s.created, s.kind, s.replies,
                    EXISTS (SELECT 1 FROM acknowledged_suggestions a WHERE a.id = s.id)
             FROM suggestions s ORDER BY s.created DESC, s.id",
        )?;
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (id, note_id, payload, status, created, kind, replies, acknowledged) in rows {
        let detail = suggestion_detail(conn, ctx, DecodedPayload::decode(&kind, &payload))?;
        // Suggestions are what the AI did *not* apply (applied changes are AI decisions, in
        // the activity feed); an accepted one is never "applied automatically".
        let auto_applied = false;
        let shown = status == "pending" || (auto_applied && status == "accepted" && !acknowledged);
        if only_pending && !shown {
            continue;
        }
        let mut thread: Vec<ThreadMessage> = Vec::new();
        let replies: Vec<ReplyRow> = replies
            .as_deref()
            .and_then(|b| rmp_serde::from_slice(b).ok())
            .unwrap_or_default();
        for r in replies {
            thread.push(ThreadMessage {
                text_dir: dir_of(&r.text),
                created_label: labels.moment_label(ts(&r.at)),
                id: r.id,
                author: match r.author {
                    crate::sync::model::ReplyAuthor::User => "user",
                    crate::sync::model::ReplyAuthor::Ai => "ai",
                }
                .to_owned(),
                text: r.text,
                pending_sync: false,
            });
        }
        for op in outbox::live_for(conn, &format!("suggestion:{id}"))? {
            if let crate::sync::model::Op::SuggestionReply(r) = &op.op
                && !thread.iter().any(|t| t.id == r.reply_id.to_string())
            {
                thread.push(ThreadMessage {
                    id: r.reply_id.to_string(),
                    author: "user".to_owned(),
                    text_dir: dir_of(&r.text),
                    text: r.text.clone(),
                    created_label: labels.moment_label(ts(&op.created)),
                    pending_sync: true,
                });
            }
        }
        let source_text: Option<String> = match &note_id {
            Some(n) => conn
                .query_row(
                    "SELECT n.content FROM notes n JOIN inbox i ON i.note_id = n.id WHERE n.id = ?1",
                    [n],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .map(|c| {
                    vault_format::Document::parse(&c)
                        .body()
                        .trim()
                        .chars()
                        .take(280)
                        .collect()
                }),
            None => None,
        };
        let nyou = needs_you(&detail);
        let created = ts(&created);
        out.push(SuggestionItem {
            pending_sync: pending.contains_key(&format!("suggestion:{id}")),
            can_accept: status == "pending" && !nyou,
            needs_you: nyou && status == "pending",
            auto_applied,
            detail,
            created_label: labels.moment_label(created),
            source_dir: source_text.as_deref().map_or(TextDir::Neutral, dir_of),
            source_text,
            thread,
            id,
            note_id,
            status,
            created,
        });
    }
    Ok(out)
}

/// Inbox: captures with their pending suggestions, and other pending suggestions, filtered.
pub fn inbox(conn: &Connection, ctx: &ViewCtx, filter: InboxFilter) -> CoreResult<InboxView> {
    let pending = pending_entities(conn)?;
    let labels = ctx.labels();
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
    for s in suggestion_items(conn, ctx, true)? {
        match &s.note_id {
            Some(n) if inbox_ids.contains(n) => by_note.entry(n.clone()).or_default().push(s),
            _ => other.push(s),
        }
    }
    let captures: Vec<InboxItem> = rows
        .into_iter()
        .map(|(id, title, content, created)| {
            let doc = vault_format::Document::parse(&content);
            let text: String = doc.body().trim().chars().take(280).collect();
            let source = format::parse_note("inbox/x.md", &content)
                .properties
                .iter()
                .find(|(k, _)| k == "source")
                .map(|(_, v)| format::property_display(v).join(", "))
                .map(|v| vault_format::resolve::link_name(v.trim_matches(['[', ']'])).to_owned());
            let suggestions = by_note.remove(&id).unwrap_or_default();
            let created = ts(&created);
            InboxItem {
                pending_sync: note_pending(&pending, &id),
                needs_you: suggestions.iter().any(|s| s.needs_you),
                ready: !suggestions.is_empty() && suggestions.iter().all(|s| s.can_accept),
                is_duplicate: suggestions
                    .iter()
                    .any(|s| s.detail.kind == SuggestionKind::Duplicate),
                suggestions,
                note_id: id,
                title,
                text_dir: dir_of(&text),
                text,
                created_label: labels.moment_label(created),
                created,
                source_label: source,
            }
        })
        .collect();
    let conflicted = |c: &InboxItem| c.suggestions.iter().any(is_conflict);
    let ready_count =
        u32_of(i64::try_from(captures.iter().filter(|c| c.ready).count()).unwrap_or(0));
    let needs_you_count = u32_of(
        i64::try_from(
            captures.iter().filter(|c| c.needs_you).count()
                + other.iter().filter(|s| s.needs_you).count(),
        )
        .unwrap_or(0),
    );
    let conflicts_count = u32_of(
        i64::try_from(
            captures.iter().filter(|c| conflicted(c)).count()
                + other.iter().filter(|s| is_conflict(s)).count(),
        )
        .unwrap_or(0),
    );
    let all_count = u32_of(i64::try_from(captures.len() + other.len()).unwrap_or(0));
    let (captures, suggestions) = match filter {
        InboxFilter::All => (captures, other),
        InboxFilter::NeedsYou => (
            captures.into_iter().filter(|c| c.needs_you).collect(),
            other.into_iter().filter(|s| s.needs_you).collect(),
        ),
        InboxFilter::Conflicts => (
            captures.into_iter().filter(conflicted).collect(),
            other.into_iter().filter(is_conflict).collect(),
        ),
    };
    Ok(InboxView {
        captures,
        suggestions,
        filter,
        ready_count,
        needs_you_count,
        conflicts_count,
        all_count,
    })
}

/// Open duplicate prompts.
pub fn duplicate_prompts(conn: &Connection, ctx: &ViewCtx) -> CoreResult<DuplicatePromptsView> {
    let mut prompts = Vec::new();
    for (op_id, candidates) in conflicts::duplicates(conn)? {
        let Some(op) = outbox::get(conn, &op_id)? else {
            continue;
        };
        let (kind, title) = crate::store::write::describe_create(&op.op);
        let mut items = Vec::new();
        for c in candidates {
            items.push(complete_candidate(conn, candidate_item(c), ctx.lang)?);
        }
        prompts.push(DuplicatePrompt {
            op_id,
            kind,
            title,
            candidates: items,
        });
    }
    Ok(DuplicatePromptsView { prompts })
}

// ---------------------------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------------------------

fn relation_chips(
    conn: &Connection,
    ctx: &ViewCtx,
    sql: &str,
    id: &str,
) -> CoreResult<Vec<RelationChip>> {
    let labels = ctx.labels();
    let rows: Vec<(
        String,
        Option<String>,
        String,
        Option<String>,
        Option<f64>,
        Option<String>,
        Option<String>,
    )> = {
        let mut st = conn.prepare(sql)?;
        st.query_map([id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    let decisions = cache::get::<Vec<crate::net::AiDecisionInfo>>(conn, cache::AI_DECISIONS)?
        .map(|(d, _)| d)
        .unwrap_or_default();
    for (rel_type, other_id, raw, by, confidence, reason, created) in rows {
        let target = entity_ref(conn, other_id, Some(raw))?.unwrap_or(EntityRef {
            id: None,
            title: String::new(),
            kind: None,
        });
        let mut citations = Vec::new();
        if let Some(r) = &reason {
            for l in wikilink::find_all(r) {
                citations.push(citation(conn, &l)?);
            }
        }
        out.push(RelationChip {
            rel_label: labels::relation_label(&rel_type, ctx.lang),
            by: by.unwrap_or_else(|| "user".to_owned()),
            confidence,
            reason,
            created_label: created.map(|c| labels.moment_label(ts(&c))),
            decision_id: decisions
                .iter()
                .find(|d| {
                    d.reverted_at.is_none()
                        && d.kind == "relation"
                        && d.rel_type.as_deref() == Some(rel_type.as_str())
                        && target.id.as_deref() == Some(d.target_id.as_str())
                })
                .map(|d| d.id.clone()),
            rel_type,
            target,
            citations,
        });
    }
    Ok(out)
}

const OUTGOING_RELATIONS: &str = "SELECT r.rel_type, r.dst_id, r.dst_raw, m.by, m.confidence, m.reason,
            m.created
     FROM relations r
     LEFT JOIN relation_meta m ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
     WHERE r.src_id = ?1 ORDER BY r.rel_type, r.dst_raw";

/// The line of `content` holding the first link to `target_id` (≤ 200 characters).
fn linking_sentence(
    content: &str,
    path: &str,
    target_id: &str,
    resolver: &crate::store::index::LinkResolver,
) -> Option<String> {
    let doc = vault_format::Document::parse(content);
    for line in doc.body().lines() {
        for l in wikilink::find_all(line) {
            if resolver.resolve(l.target(), path).as_deref() == Some(target_id) {
                let text = line.trim().trim_start_matches(['-', '*', '>', ' ']);
                let mut s: String = text.chars().take(200).collect();
                if text.chars().count() > 200 {
                    s.push('…');
                }
                return Some(s);
            }
        }
    }
    None
}

fn backlinks(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<Vec<BacklinkGroup>> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
    )> = {
        let mut st = conn.prepare(
            "SELECT 'link', n.id, n.title, n.path, n.content, NULL, NULL
               FROM links l JOIN notes n ON n.id = l.note_id
               WHERE l.dst_id = ?1 AND n.deleted = 0 AND l.note_id != ?1
             UNION
             SELECT r.rel_type, n.id, n.title, n.path, n.content, m.by, m.confidence
               FROM relations r JOIN notes n ON n.id = r.src_id
               LEFT JOIN relation_meta m
                 ON m.src_id = r.src_id AND m.dst_id = r.dst_id AND m.rel_type = r.rel_type
               WHERE r.dst_id = ?1 AND n.deleted = 0 AND r.src_id != ?1
             ORDER BY 1, 3, 2",
        )?;
        st.query_map([id], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let resolver = crate::store::index::LinkResolver::load(conn)?;
    let mut groups: Vec<BacklinkGroup> = Vec::new();
    for (kind, note_id, title, path, content, by, confidence) in rows {
        let snippet = if kind == "link" {
            linking_sentence(&content, &path, id, &resolver)
        } else {
            None
        };
        let item = BacklinkItem {
            note_id,
            title_dir: dir_of(&title),
            title,
            snippet_dir: snippet.as_deref().map_or(TextDir::Neutral, dir_of),
            snippet,
            by: if kind == "link" {
                None
            } else {
                Some(by.unwrap_or_else(|| "user".to_owned()))
            },
            confidence,
        };
        match groups.last_mut() {
            Some(g) if g.kind == kind => {
                if !g.items.iter().any(|i| i.note_id == item.note_id) {
                    g.items.push(item);
                }
            }
            _ => groups.push(BacklinkGroup {
                label: labels::relation_label(&kind, ctx.lang),
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
        hints::SpanKind::Bold => HintKind::Bold,
        hints::SpanKind::Italic => HintKind::Italic,
        hints::SpanKind::Strike => HintKind::Strike,
        hints::SpanKind::Mark => HintKind::Mark,
        hints::SpanKind::RtlLine => HintKind::RtlLine,
        hints::SpanKind::LtrLine => HintKind::LtrLine,
    }
}

fn is_relation_key(key: &str) -> bool {
    key.parse::<vault_format::RelationKey>().is_ok()
}

fn history_entries(
    conn: &Connection,
    ctx: &ViewCtx,
    id: &str,
) -> CoreResult<Option<Vec<HistoryEntry>>> {
    let labels = ctx.labels();
    let Some((revisions, _)) =
        cache::get::<Vec<crate::net::RevisionInfo>>(conn, &cache::history_key(id))?
    else {
        return Ok(None);
    };
    let n = revisions.len();
    Ok(Some(
        revisions
            .into_iter()
            .enumerate()
            .map(|(i, r)| HistoryEntry {
                version_label: format!("v{}", n - i),
                can_revert: i > 0 && r.change != "deleted",
                at_label: labels.moment_with_day(r.at),
                at: r.at,
                commit: r.commit,
                message: r.message,
                author: r.author,
            })
            .collect(),
    ))
}

/// The note screen.
#[allow(clippy::too_many_lines)] // every field of the note view
pub fn note_screen(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<NoteScreen> {
    let Some(n) = crate::store::notes::get(conn, id)?.filter(|n| !n.deleted) else {
        return Ok(NoteScreen {
            id: id.to_owned(),
            note: None,
        });
    };
    let labels = ctx.labels();
    let lang = ctx.lang;
    let parsed = format::parse_note(&n.path, &n.content);
    let pending = pending_entities(conn)?;
    let op_of = |status: &str| -> CoreResult<Option<String>> {
        Ok(conn
            .query_row(
                "SELECT op_id FROM outbox WHERE local_entity = ?1 AND status = ?2
                 ORDER BY ord LIMIT 1",
                params![format!("note:{id}"), status],
                |r| r.get(0),
            )
            .optional()?)
    };
    let conflict_op = op_of("conflict")?;
    let duplicate_op = op_of("duplicate")?;
    let pending_ops = pending.get(&format!("note:{id}")).copied().unwrap_or(0);
    let history = history_entries(conn, ctx, id)?;
    let version_label = history
        .as_ref()
        .and_then(|h| h.first())
        .map(|h| h.version_label.clone());
    let kind = match (&conflict_op, &duplicate_op, pending_ops) {
        (Some(_), _, _) => NoteSyncKind::Conflict,
        (None, Some(_), _) => NoteSyncKind::Duplicate,
        (None, None, 0) => NoteSyncKind::Synced,
        (None, None, _) => NoteSyncKind::Pending,
    };
    let status_label = match kind {
        NoteSyncKind::Synced => match (&version_label, lang) {
            (Some(v), Lang::En) => format!("Saved · {v}"),
            (Some(v), Lang::Ar) => format!("محفوظ · {v}"),
            (None, _) => tr(lang, "Saved", "محفوظ"),
        },
        NoteSyncKind::Pending => match lang {
            Lang::En => format!(
                "Saved on this device · {} to sync",
                labels::CHANGES.of(i64::from(pending_ops), lang)
            ),
            Lang::Ar => format!(
                "محفوظ على هذا الجهاز · {} للمزامنة",
                labels::CHANGES.of(i64::from(pending_ops), lang)
            ),
        },
        NoteSyncKind::Conflict => tr(lang, "Conflict", "تعارض"),
        NoteSyncKind::Duplicate => tr(lang, "Already exists", "موجود بالفعل"),
    };
    let sync = NoteSyncState {
        kind,
        pending_ops,
        conflict_op_id: conflict_op,
        duplicate_op_id: duplicate_op,
        label: status_label,
    };
    let default_time = default_reminder_time(conn)?;
    let mut tasks = Vec::new();
    for row in task_rows(conn, "WHERE t.note_id = ?1 ORDER BY t.line_no", [id])? {
        tasks.push(task_item(conn, ctx, row, &pending, default_time)?);
    }
    let backlinks = backlinks(conn, ctx, id)?;
    let backlink_count = u32_of(
        i64::try_from(
            backlinks
                .iter()
                .flat_map(|g| g.items.iter().map(|i| i.note_id.as_str()))
                .collect::<HashSet<_>>()
                .len(),
        )
        .unwrap_or(0),
    );
    let created_label = parsed
        .created
        .as_deref()
        .and_then(|c| DateTime::parse_from_rfc3339(c).ok())
        .map(|c| {
            let d = labels.local(c.with_timezone(&Utc)).date();
            match lang {
                Lang::En => format!("Created {}", labels.date_in_list(d)),
                Lang::Ar => format!("أُنشئت {}", labels.date_in_list(d)),
            }
        });
    let edited_at = parsed
        .updated
        .as_deref()
        .and_then(|c| DateTime::parse_from_rfc3339(c).ok())
        .map_or_else(|| ts(&n.local_updated_at), |c| c.with_timezone(&Utc));
    let edited_label = Some(match lang {
        Lang::En => format!(
            "Edited {}",
            labels.moment_with_day(edited_at).to_lowercase_first()
        ),
        Lang::Ar => format!("عُدّلت {}", labels.moment_with_day(edited_at)),
    });
    let account_name = account::get(conn)?
        .map(|a| a.display_name)
        .unwrap_or_default();
    let edited_by = history
        .as_ref()
        .and_then(|h| h.first())
        .map(|h| match h.author.as_str() {
            "user" => account_name.clone(),
            "ai" => "AI".to_owned(),
            _ => tr(lang, "System", "النظام"),
        });
    let word_count = u32_of(i64::try_from(parsed.body.split_whitespace().count()).unwrap_or(0));
    let pinned = cache::pinned(conn)?.iter().any(|p| p == id);
    Ok(NoteScreen {
        id: id.to_owned(),
        note: Some(NoteView {
            id: n.id.clone(),
            path: n.path.clone(),
            title_dir: dir_of(&parsed.display_title),
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
            relations: relation_chips(conn, ctx, OUTGOING_RELATIONS, id)?,
            backlinks,
            tags: parsed.tags,
            tasks,
            hints: hints_resolved(conn, &n.path, &n.content)?,
            sync,
            history: if history.is_some() && ctx.connectivity != Connectivity::Offline {
                Availability::Available
            } else {
                availability_history(ctx)
            },
            content_version: crate::sync::model::Version::of_text(&n.content)
                .as_str()
                .to_owned(),
            version_label,
            created_label,
            edited_label,
            edited_by,
            word_count,
            backlink_count,
            history_entries: history.unwrap_or_default(),
            pinned,
            content: n.content,
        }),
    })
}

trait LowerFirst {
    fn to_lowercase_first(&self) -> String;
}

impl LowerFirst for String {
    fn to_lowercase_first(&self) -> String {
        let mut c = self.chars();
        c.next()
            .map(|f| f.to_lowercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    }
}

/// History availability before it was fetched: offline, or fetchable.
fn availability_history(ctx: &ViewCtx) -> Availability {
    if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::Available
    }
}

fn hint_of(h: hints::Span) -> EditorHint {
    EditorHint {
        kind: hint_kind(h.kind),
        start: h.start,
        end: h.end,
        target_id: None,
        target_anchor: h.anchor,
        task_id: h.task_id,
        level: h.level,
    }
}

/// Editor highlight spans of `content` (links unresolved: no database).
pub fn hints_of(content: &str) -> Vec<EditorHint> {
    hints::editor_hints(content)
        .into_iter()
        .map(hint_of)
        .collect()
}

/// Editor highlight spans with wikilinks resolved to note IDs (as written in the note at
/// `path`).
pub fn hints_resolved(conn: &Connection, path: &str, content: &str) -> CoreResult<Vec<EditorHint>> {
    let resolver = crate::store::index::LinkResolver::load(conn)?;
    Ok(hints::editor_hints(content)
        .into_iter()
        .map(|h| {
            let target = h
                .link_path
                .as_deref()
                .filter(|p| !p.is_empty())
                .and_then(|p| resolver.resolve(p, path));
            let mut e = hint_of(h);
            e.target_id = target;
            e
        })
        .collect())
}

/// An account row of Admin → Users.
pub fn admin_user_item(ctx: &ViewCtx, me: &str, u: crate::net::AdminUserInfo) -> AdminUserItem {
    let labels = ctx.labels();
    let created_label = if u.status == "pending" {
        match ctx.lang {
            Lang::En => format!("Requested {}", labels.ago(u.created)),
            Lang::Ar => format!("طُلب {}", labels.ago(u.created)),
        }
    } else {
        match ctx.lang {
            Lang::En => format!(
                "Joined {}",
                labels.date_long(labels.local(u.created).date())
            ),
            Lang::Ar => format!("انضم {}", labels.date_long(labels.local(u.created).date())),
        }
    };
    AdminUserItem {
        initials: labels::initials(&u.display_name),
        is_self: u.id == me,
        created_label,
        deletion_label: u.deletion_at.map(|d| match ctx.lang {
            Lang::En => format!("Deleted on {}", labels.date_long(labels.local(d).date())),
            Lang::Ar => format!("يُحذف في {}", labels.date_long(labels.local(d).date())),
        }),
        password_change_required: u.password_change_required,
        id: u.id,
        username: u.username,
        display_name: u.display_name,
        role: u.role,
        status: u.status,
        created: u.created,
        deletion_at: u.deletion_at,
        export_downloaded_at: u.export_downloaded_at,
    }
}

/// Admin → Users from the server's list, filtered by `query` (username or display name,
/// normalised): pending approvals oldest first, then every other account by username.
pub fn admin_users(
    ctx: &ViewCtx,
    me: &str,
    users: Vec<crate::net::AdminUserInfo>,
    query: &str,
) -> AdminUsersView {
    let q = text_normalize::normalize_for_search(query.trim());
    let users: Vec<AdminUserItem> = users
        .into_iter()
        .filter(|u| {
            q.is_empty()
                || text_normalize::normalize_for_search(&u.username).contains(&q)
                || text_normalize::normalize_for_search(&u.display_name).contains(&q)
        })
        .map(|u| admin_user_item(ctx, me, u))
        .collect();
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
        query: query.to_owned(),
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

/// A folder's subfolders and notes, with the breadcrumb.
pub fn notes_list(conn: &Connection, ctx: &ViewCtx, folder: &str) -> CoreResult<NotesListView> {
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
    for id in &direct {
        notes.extend(note_items(conn, ctx, "WHERE n.id = ?1", [id])?);
    }
    notes.sort_by(|a, b| (a.title.to_lowercase(), &a.id).cmp(&(b.title.to_lowercase(), &b.id)));
    let direct_count = |p: &str| -> u32 {
        let pre = if p.is_empty() {
            String::new()
        } else {
            format!("{p}/")
        };
        u32_of(
            i64::try_from(
                paths
                    .iter()
                    .filter(|(_, path)| {
                        path.strip_prefix(&pre)
                            .is_some_and(|rest| !rest.contains('/'))
                    })
                    .count(),
            )
            .unwrap_or(0),
        )
    };
    let mut breadcrumb = vec![FolderItem {
        path: String::new(),
        name: tr(ctx.lang, "Notes", "الملاحظات"),
        note_count: direct_count(""),
    }];
    let mut acc = String::new();
    for seg in folder.split('/').filter(|s| !s.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        breadcrumb.push(FolderItem {
            path: acc.clone(),
            name: seg.to_owned(),
            note_count: direct_count(&acc),
        });
    }
    Ok(NotesListView {
        folder: folder.to_owned(),
        note_count: u32_of(i64::try_from(direct.len()).unwrap_or(0)),
        breadcrumb,
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

/// Days before expiry that count as "expiring soon".
pub const EXPIRING_DAYS: i64 = 60;

fn tags_of(conn: &Connection, id: &str) -> CoreResult<Vec<String>> {
    let mut st = conn.prepare_cached("SELECT tag FROM tags WHERE note_id = ?1 ORDER BY tag")?;
    Ok(st
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Notes mentioning `id` (links or relations) and the last change among them and it.
fn activity_of(conn: &Connection, id: &str) -> CoreResult<(u32, Option<DateTime<Utc>>)> {
    let (count, last): (i64, Option<String>) = conn.query_row(
        "WITH m AS (SELECT note_id AS nid FROM links WHERE dst_id = ?1
                    UNION SELECT src_id FROM relations WHERE dst_id = ?1)
         SELECT (SELECT COUNT(*) FROM m JOIN notes n ON n.id = m.nid
                 WHERE n.deleted = 0 AND n.id != ?1),
                (SELECT MAX(n.local_updated_at) FROM notes n
                 WHERE n.deleted = 0 AND (n.id = ?1 OR n.id IN (SELECT nid FROM m)))",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((u32_of(count), last.as_deref().map(ts)))
}

fn section_body(parsed: &format::ParsedNote, title: &str, own: bool) -> Option<String> {
    vault_format::sections::sections(&parsed.body)
        .into_iter()
        .find(|s| s.level == 2 && s.title.eq_ignore_ascii_case(title))
        .map(|s| {
            let span = if own {
                s.own_content_span
            } else {
                s.content_span
            };
            parsed.body[span].to_owned()
        })
}

fn has_open_items(conn: &Connection, id: &str) -> CoreResult<bool> {
    let row: Option<(String, String)> = conn
        .query_row("SELECT path, content FROM notes WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    Ok(row.is_some_and(|(path, content)| {
        section_body(&format::parse_note(&path, &content), "Open items", true)
            .is_some_and(|b| b.lines().any(|l| l.trim().starts_with("- ")))
    }))
}

fn company_of(conn: &Connection, id: &str) -> CoreResult<Option<EntityRef>> {
    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT dst_id, dst_raw FROM relations WHERE src_id = ?1
               AND rel_type IN ('works-at', 'companies')
             ORDER BY CASE rel_type WHEN 'works-at' THEN 0 ELSE 1 END, dst_raw LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((dst, raw)) => entity_ref(conn, dst, Some(raw)),
        None => Ok(None),
    }
}

fn holder_label(
    labels: &Labels,
    holder: Option<&EntityRef>,
    last: Option<&EntityRef>,
    last_date: Option<NaiveDate>,
) -> Option<String> {
    match (holder, last) {
        (Some(h), _) => Some(match labels.lang {
            Lang::En => format!("With {}", h.title),
            Lang::Ar => format!("مع {}", h.title),
        }),
        (None, Some(l)) => Some(match (labels.lang, last_date) {
            (Lang::En, Some(d)) => format!("Last with {} · {}", l.title, labels.day_month(d)),
            (Lang::En, None) => format!("Last with {}", l.title),
            (Lang::Ar, Some(d)) => format!("آخر مرة مع {} · {}", l.title, labels.day_month(d)),
            (Lang::Ar, None) => format!("آخر مرة مع {}", l.title),
        }),
        (None, None) => None,
    }
}

/// Date of the newest custody event of a document.
fn last_moved(conn: &Connection, id: &str) -> CoreResult<Option<NaiveDate>> {
    let at: Option<String> = conn.query_row(
        "SELECT MAX(at) FROM custody_events WHERE document_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    Ok(at.as_deref().and_then(date))
}

fn expires_label(labels: &Labels, d: NaiveDate) -> String {
    let when = labels.date_long(d);
    if d < labels.today() {
        match labels.lang {
            Lang::En => format!("Expired {when}"),
            Lang::Ar => format!("انتهى {when}"),
        }
    } else {
        match labels.lang {
            Lang::En => format!("Expires {when}"),
            Lang::Ar => format!("ينتهي {when}"),
        }
    }
}

fn expiring(labels: &Labels, d: Option<NaiveDate>) -> bool {
    d.is_some_and(|d| labels.days_from_today(d) <= EXPIRING_DAYS)
}

#[allow(clippy::needless_pass_by_value)] // the row's owned columns move into the item
fn directory_item(
    conn: &Connection,
    ctx: &ViewCtx,
    tab: DirectoryTab,
    id: String,
    title: String,
    role: Option<String>,
    industry: Option<String>,
) -> CoreResult<DirectoryItem> {
    let labels = ctx.labels();
    let (mention_count, last_active) = activity_of(conn, &id)?;
    let mut item = DirectoryItem {
        aliases: aliases_of(conn, &id, &title)?,
        kind: tab_kind(tab).to_owned(),
        title_dir: dir_of(&title),
        initials: labels::initials(&title),
        mention_count,
        last_active_label: last_active.map(|t| labels.date_in_list(labels.local(t).date())),
        last_active,
        role: role.clone(),
        company: None,
        industry: industry.clone(),
        tags: tags_of(conn, &id)?,
        status: None,
        doc_type: None,
        location: Vec::new(),
        holder: None,
        last_holder: None,
        holder_label: None,
        copy: None,
        expires: None,
        expires_label: None,
        expiring_soon: false,
        breadcrumb: Vec::new(),
        document_count: 0,
        has_open_items: false,
        subtitle: None,
        id,
        title,
    };
    match tab {
        DirectoryTab::People => {
            item.company = company_of(conn, &item.id)?;
            item.has_open_items = has_open_items(conn, &item.id)?;
            item.subtitle = match (&role, &item.company) {
                (Some(r), Some(c)) => Some(format!("{r} · {}", c.title)),
                (Some(r), None) => Some(r.clone()),
                (None, Some(c)) => Some(c.title.clone()),
                (None, None) => None,
            };
        }
        DirectoryTab::Companies => {
            item.has_open_items = has_open_items(conn, &item.id)?;
            item.subtitle = industry;
        }
        DirectoryTab::Documents => {
            let b = document_brief(conn, &item.id)?;
            let extra: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT copy, expires FROM documents WHERE note_id = ?1",
                    [&item.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (copy, expires) = extra.unwrap_or_default();
            let expires = expires.as_deref().and_then(date);
            item.subtitle = match (&b.status, b.location.as_ref().or(b.holder.as_ref())) {
                (Some(s), Some(w)) => Some(format!("{s} · {}", w.title)),
                (Some(s), None) => Some(s.clone()),
                (None, Some(w)) => Some(w.title.clone()),
                (None, None) => None,
            };
            item.holder_label = holder_label(
                &labels,
                b.holder.as_ref(),
                b.last_holder.as_ref(),
                last_moved(conn, &item.id)?,
            );
            item.status = b.status;
            item.doc_type = b.doc_type;
            item.location = b.location_path;
            item.holder = b.holder;
            item.last_holder = b.last_holder;
            item.copy = copy;
            item.expires_label = expires.map(|d| expires_label(&labels, d));
            item.expiring_soon = expiring(&labels, expires);
            item.expires = expires;
        }
        DirectoryTab::Places => {
            item.breadcrumb = breadcrumb(conn, &item.id, false)?;
            item.subtitle = item.breadcrumb.last().map(|p| p.title.clone());
            item.document_count = u32_of(conn.query_row(
                &format!(
                    "{NESTED_PLACES} SELECT COUNT(*) FROM documents d JOIN notes n ON n.id = d.note_id
                     WHERE n.deleted = 0 AND d.location_id IN (SELECT id FROM sub)"
                ),
                [&item.id],
                |r| r.get(0),
            )?);
        }
    }
    Ok(item)
}

fn place_contains(conn: &Connection, place: &str, target: Option<&str>) -> CoreResult<bool> {
    let Some(target) = target else {
        return Ok(false);
    };
    Ok(conn.query_row(
        &format!("{NESTED_PLACES} SELECT EXISTS (SELECT 1 FROM sub WHERE id = ?2)"),
        params![place, target],
        |r| r.get(0),
    )?)
}

fn matches_filter(conn: &Connection, i: &DirectoryItem, f: &DirectoryFilter) -> CoreResult<bool> {
    let eq = |a: &Option<String>, b: &Option<String>| match b {
        None => true,
        Some(b) => a.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(b)),
    };
    Ok(f.tags.iter().all(|t| i.tags.contains(t))
        && eq(&i.role, &f.role)
        && f.company_id
            .as_ref()
            .is_none_or(|c| i.company.as_ref().and_then(|x| x.id.as_ref()) == Some(c))
        && eq(&i.industry, &f.industry)
        && eq(&i.doc_type, &f.doc_type)
        && eq(&i.status, &f.status)
        && match &f.place_id {
            None => true,
            Some(p) => place_contains(conn, p, i.location.last().and_then(|l| l.id.as_deref()))?,
        }
        && f.holder_id
            .as_ref()
            .is_none_or(|h| i.holder.as_ref().and_then(|x| x.id.as_ref()) == Some(h))
        && (!f.expiring || i.expiring_soon)
        && (!f.has_open_items || i.has_open_items))
}

fn facet_options(
    items: &[DirectoryItem],
    tab: DirectoryTab,
    f: &DirectoryFilter,
    lang: Lang,
) -> Vec<FilterOption> {
    let mut counts: BTreeMap<(String, String), (String, u32)> = BTreeMap::new();
    let mut add = |facet: &str, value: &str, label: &str| {
        let e = counts
            .entry((facet.to_owned(), value.to_owned()))
            .or_insert_with(|| (label.to_owned(), 0));
        e.1 += 1;
    };
    for i in items {
        for t in &i.tags {
            add("tag", t, &format!("#{t}"));
        }
        match tab {
            DirectoryTab::People => {
                if let Some(r) = &i.role {
                    add("role", r, r);
                }
                if let Some(c) = &i.company
                    && let Some(id) = &c.id
                {
                    add("company", id, &c.title);
                }
                if i.has_open_items {
                    add("has_open_items", "", &tr(lang, "Open items", "بنود مفتوحة"));
                }
            }
            DirectoryTab::Companies => {
                if let Some(v) = &i.industry {
                    add("industry", v, v);
                }
                if i.has_open_items {
                    add("has_open_items", "", &tr(lang, "Open items", "بنود مفتوحة"));
                }
            }
            DirectoryTab::Documents => {
                if let Some(v) = &i.doc_type {
                    add("doc_type", v, v);
                }
                if let Some(v) = &i.status {
                    add("status", v, v);
                }
                if let Some(p) = i.location.first()
                    && let Some(id) = &p.id
                {
                    add("place", id, &p.title);
                }
                if let Some(h) = &i.holder
                    && let Some(id) = &h.id
                {
                    add("holder", id, &h.title);
                }
                if i.expiring_soon {
                    add("expiring", "", &tr(lang, "Expiring", "قارب على الانتهاء"));
                }
            }
            DirectoryTab::Places => {}
        }
    }
    counts
        .into_iter()
        .map(|((facet, value), (label, count))| {
            let selected = match facet.as_str() {
                "tag" => f.tags.contains(&value),
                "role" => f.role.as_deref() == Some(value.as_str()),
                "company" => f.company_id.as_deref() == Some(value.as_str()),
                "industry" => f.industry.as_deref() == Some(value.as_str()),
                "doc_type" => f.doc_type.as_deref() == Some(value.as_str()),
                "status" => f.status.as_deref() == Some(value.as_str()),
                "place" => f.place_id.as_deref() == Some(value.as_str()),
                "holder" => f.holder_id.as_deref() == Some(value.as_str()),
                "expiring" => f.expiring,
                "has_open_items" => f.has_open_items,
                _ => false,
            };
            FilterOption {
                facet,
                value,
                label,
                count,
                selected,
            }
        })
        .collect()
}

/// The directory tab: rows matching `query` (names and aliases, normalised, both scripts) and
/// `filter`, in `sort` order, with filter chips, sections and the tab's entity suggestions.
#[allow(clippy::too_many_lines)] // query, filter, sort and sections
pub fn directory_filtered(
    conn: &Connection,
    ctx: &ViewCtx,
    tab: DirectoryTab,
    query: &str,
    filter: &DirectoryFilter,
    sort: DirectorySort,
) -> CoreResult<DirectoryView> {
    let kind = tab_kind(tab);
    let lang = ctx.lang;
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
    let mut all = Vec::new();
    for (id, title, role, industry) in rows {
        all.push(directory_item(conn, ctx, tab, id, title, role, industry)?);
    }
    let filter_options = facet_options(&all, tab, filter, lang);
    let expiring_count =
        u32_of(i64::try_from(all.iter().filter(|i| i.expiring_soon).count()).unwrap_or(0));
    let mut items = Vec::new();
    for i in all {
        if matches_filter(conn, &i, filter)? {
            items.push(i);
        }
    }
    match sort {
        DirectorySort::Name => {}
        DirectorySort::LastActive => items.sort_by(|a, b| {
            b.last_active
                .cmp(&a.last_active)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        }),
        DirectorySort::RecentlyMoved => {
            let mut keyed = Vec::new();
            for i in items {
                keyed.push((last_moved(conn, &i.id)?, i));
            }
            keyed.sort_by(|(a, x), (b, y)| {
                b.cmp(a)
                    .then_with(|| x.title.to_lowercase().cmp(&y.title.to_lowercase()))
                    .then_with(|| x.id.cmp(&y.id))
            });
            items = keyed.into_iter().map(|(_, i)| i).collect();
        }
    }
    let plain =
        sort == DirectorySort::Name && q.is_empty() && *filter == DirectoryFilter::default();
    let all_label = match (tab, lang) {
        (DirectoryTab::People, Lang::En) => "All people · A–Z",
        (DirectoryTab::Companies, Lang::En) => "All companies · A–Z",
        (DirectoryTab::Documents, Lang::En) => "All documents · A–Z",
        (DirectoryTab::Places, Lang::En) => "All places · A–Z",
        (DirectoryTab::People, Lang::Ar) => "كل الأشخاص · أ–ي",
        (DirectoryTab::Companies, Lang::Ar) => "كل الشركات · أ–ي",
        (DirectoryTab::Documents, Lang::Ar) => "كل المستندات · أ–ي",
        (DirectoryTab::Places, Lang::Ar) => "كل الأماكن · أ–ي",
    };
    let mut sections = Vec::new();
    if plain && matches!(tab, DirectoryTab::People | DirectoryTab::Companies) && items.len() > 3 {
        let mut recent: Vec<DirectoryItem> = items
            .iter()
            .filter(|i| i.last_active.is_some())
            .cloned()
            .collect();
        recent.sort_by(|a, b| {
            b.last_active
                .cmp(&a.last_active)
                .then_with(|| a.id.cmp(&b.id))
        });
        recent.truncate(3);
        if !recent.is_empty() {
            sections.push(DirectorySection {
                label: tr(lang, "Recently active", "نشطون مؤخرًا"),
                items: recent,
            });
        }
    }
    sections.push(DirectorySection {
        label: if sort == DirectorySort::Name {
            all_label.to_owned()
        } else {
            tr(lang, "Results", "النتائج")
        },
        items: items.clone(),
    });
    let suggestions = suggestion_items(conn, ctx, true)?
        .into_iter()
        .filter(|s| match (tab, s.detail.kind) {
            (_, SuggestionKind::Duplicates) => {
                s.detail.duplicates.first().is_some_and(|c| c.kind == kind)
            }
            (DirectoryTab::People | DirectoryTab::Companies, SuggestionKind::EntityLink)
            | (DirectoryTab::Documents, SuggestionKind::Custody) => true,
            _ => false,
        })
        .collect();
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
        filter: filter.clone(),
        sort,
        filter_options,
        sections,
        suggestions,
        expiring_count,
    })
}

/// The directory tab without filters (A–Z).
pub fn directory(
    conn: &Connection,
    ctx: &ViewCtx,
    tab: DirectoryTab,
    query: &str,
) -> CoreResult<DirectoryView> {
    directory_filtered(
        conn,
        ctx,
        tab,
        query,
        &DirectoryFilter::default(),
        DirectorySort::Name,
    )
}

fn document_brief(conn: &Connection, id: &str) -> CoreResult<DocumentBrief> {
    type Row = (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let row: Option<Row> = conn
        .query_row(
            "SELECT n.title, d.status, d.location_id, d.location_raw, d.holder_id, d.holder_raw,
                    d.doc_type, d.last_holder_id, d.last_holder_raw, d.expires
             FROM documents d JOIN notes n ON n.id = d.note_id WHERE d.note_id = ?1",
            [id],
            |r| {
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
            },
        )
        .optional()?;
    let Some((
        title,
        status,
        loc_id,
        loc_raw,
        holder_id,
        holder_raw,
        doc_type,
        last_id,
        last_raw,
        expires,
    )) = row
    else {
        return Ok(DocumentBrief {
            id: id.to_owned(),
            title: String::new(),
            status: None,
            location: None,
            holder: None,
            doc_type: None,
            last_holder: None,
            location_path: Vec::new(),
            expiring_soon: false,
            title_dir: TextDir::Neutral,
        });
    };
    let location_path = match &loc_id {
        Some(l) => breadcrumb(conn, l, true)?,
        None => entity_ref(conn, None, loc_raw.clone())?
            .into_iter()
            .collect(),
    };
    let _ = expires;
    Ok(DocumentBrief {
        id: id.to_owned(),
        title_dir: dir_of(&title),
        title,
        status,
        location: entity_ref(conn, loc_id, loc_raw)?,
        holder: entity_ref(conn, holder_id, holder_raw)?,
        doc_type,
        last_holder: entity_ref(conn, last_id, last_raw)?,
        location_path,
        // Needs the account's "today": see `brief_in_ctx`.
        expiring_soon: false,
    })
}

/// A document brief with `expiring_soon` computed for the account's today.
fn brief_in_ctx(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<DocumentBrief> {
    let mut b = document_brief(conn, id)?;
    let expires: Option<Option<String>> = conn
        .query_row(
            "SELECT expires FROM documents WHERE note_id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    b.expiring_soon = expiring(&ctx.labels(), expires.flatten().as_deref().and_then(date));
    Ok(b)
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
pub(crate) fn breadcrumb(
    conn: &Connection,
    id: &str,
    include_self: bool,
) -> CoreResult<Vec<EntityRef>> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    if include_self {
        chain.push(note_ref(conn, id)?);
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

fn custody_sentence(
    kind: &str,
    place: Option<&EntityRef>,
    person: Option<&EntityRef>,
    counterparty: Option<&EntityRef>,
    lang: Lang,
) -> (String, Option<EntityRef>, Option<EntityRef>, String) {
    let t = |r: Option<&EntityRef>| r.map(|r| r.title.clone()).unwrap_or_default();
    let (actor, dest) = match kind {
        "stored-at" | "moved-to" | "found" => (None, place.cloned()),
        "handed-to" => (None, person.cloned()),
        "returned-by" => (person.cloned(), place.cloned()),
        "sent-to" => (None, counterparty.cloned()),
        "received-from" => (counterparty.cloned(), place.cloned()),
        _ => (None, None),
    };
    let sentence = match (kind, lang) {
        ("stored-at", Lang::En) => format!("Stored at {}", t(place)),
        ("stored-at", Lang::Ar) => format!("حُفظ في {}", t(place)),
        ("moved-to", Lang::En) => format!("Moved to {}", t(place)),
        ("moved-to", Lang::Ar) => format!("نُقل إلى {}", t(place)),
        ("handed-to", Lang::En) => format!("Handed to {}", t(person)),
        ("handed-to", Lang::Ar) => format!("سُلّم إلى {}", t(person)),
        ("returned-by", Lang::En) if place.is_some() => {
            format!("{} returned it to {}", t(person), t(place))
        }
        ("returned-by", Lang::En) => format!("{} returned it", t(person)),
        ("returned-by", Lang::Ar) if place.is_some() => {
            format!("أعاده {} إلى {}", t(person), t(place))
        }
        ("returned-by", Lang::Ar) => format!("أعاده {}", t(person)),
        ("sent-to", Lang::En) => format!("Sent to {}", t(counterparty)),
        ("sent-to", Lang::Ar) => format!("أُرسل إلى {}", t(counterparty)),
        ("received-from", Lang::En) => format!("Received from {}", t(counterparty)),
        ("received-from", Lang::Ar) => format!("استُلم من {}", t(counterparty)),
        ("lost", Lang::En) => "Lost".to_owned(),
        ("lost", Lang::Ar) => "فُقد".to_owned(),
        ("found", Lang::En) if place.is_some() => format!("Found at {}", t(place)),
        ("found", Lang::En) => "Found".to_owned(),
        ("found", Lang::Ar) if place.is_some() => format!("وُجد في {}", t(place)),
        ("found", Lang::Ar) => "وُجد".to_owned(),
        ("destroyed", Lang::En) => "Destroyed".to_owned(),
        ("destroyed", Lang::Ar) => "أُتلف".to_owned(),
        (other, _) => other.to_owned(),
    };
    (
        format!("custody.{}", kind.replace('-', "_")),
        actor,
        dest,
        sentence,
    )
}

fn custody_items(
    conn: &Connection,
    ctx: &ViewCtx,
    here: Option<&str>,
    where_sql: &str,
    p: impl rusqlite::Params,
) -> CoreResult<Vec<CustodyItem>> {
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
    let labels = ctx.labels();
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
        let place = entity_ref(conn, pid.clone(), praw)?;
        let person = entity_ref(conn, perid, perraw)?;
        let counterparty = entity_ref(conn, cid, craw)?;
        let (sentence_key, actor, destination, sentence) = custody_sentence(
            &kind,
            place.as_ref(),
            person.as_ref(),
            counterparty.as_ref(),
            ctx.lang,
        );
        let day = date(&at).unwrap_or_default();
        out.push(CustodyItem {
            date: day,
            date_label: labels.day_month(day),
            by: if citations.is_empty() { "user" } else { "ai" }.to_owned(),
            confidence: None,
            decision_id: None,
            sentence_key,
            actor,
            destination,
            sentence,
            here: here.is_some_and(|h| pid.as_deref() == Some(h)),
            kind,
            document: entity_ref(conn, Some(doc), None)?,
            place,
            person,
            counterparty,
            citations,
        });
    }
    Ok(out)
}

fn cited_bullets(conn: &Connection, ctx: &ViewCtx, content: &str) -> CoreResult<Vec<CitedBullet>> {
    let labels = ctx.labels();
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
            dir: dir_of(&text),
            date_label: day.map(|d| labels.day_month(d)),
            text,
            date: day,
            citations,
        });
    }
    Ok(out)
}

fn user_notes(parsed: &format::ParsedNote) -> String {
    section_body(parsed, "Notes", false)
        .map(|s| s.trim_end().to_owned())
        .unwrap_or_default()
}

#[allow(clippy::too_many_lines)] // every section of the page
fn entity_view(
    conn: &Connection,
    ctx: &ViewCtx,
    n: &crate::store::notes::NoteRow,
) -> CoreResult<EntityView> {
    let labels = ctx.labels();
    let parsed = format::parse_note(&n.path, &n.content);
    let section = |title: &str| section_body(&parsed, title, true);
    let pending = pending_entities(conn)?;
    let mentions = note_items(
        conn,
        ctx,
        "WHERE n.deleted = 0 AND n.id != ?1 AND n.id IN (
            SELECT note_id FROM links WHERE dst_id = ?1
            UNION SELECT src_id FROM relations WHERE dst_id = ?1)
         ORDER BY n.local_updated_at DESC, n.id DESC",
        [&n.id],
    )?;
    let mut needles = vec![parsed.display_title.clone()];
    needles.extend(parsed.aliases.iter().cloned());
    let mentions: Vec<NoteListItem> = mentions
        .into_iter()
        .map(|mut m| {
            m.highlights = highlight_spans(&m.snippet, &needles);
            m
        })
        .collect();
    let mut related = relation_chips(conn, ctx, OUTGOING_RELATIONS, &n.id)?;
    related.retain(|c| {
        c.target
            .id
            .as_deref()
            .is_some_and(|t| is_entity(conn, t).unwrap_or(false))
    });
    let incoming = relation_chips(
        conn,
        ctx,
        "SELECT r.rel_type, r.src_id, n.title, m.by, m.confidence, m.reason, m.created
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
        documents.push(brief_in_ctx(conn, ctx, &d)?);
    }
    let summary = section("Summary")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let summary_citations = match &summary {
        Some(s) => wikilink::find_all(s)
            .iter()
            .map(|l| citation(conn, l))
            .collect::<CoreResult<Vec<_>>>()?,
        None => Vec::new(),
    };
    let open_items = cited_bullets(conn, ctx, &section("Open items").unwrap_or_default())?;
    let (mention_count, last_active) = activity_of(conn, &n.id)?;
    Ok(EntityView {
        id: n.id.clone(),
        kind: n.kind.clone(),
        title_dir: dir_of(&parsed.display_title),
        initials: labels::initials(&parsed.display_title),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        properties: parsed
            .properties
            .iter()
            .filter(|(k, _)| {
                !is_relation_key(k) && !matches!(k.as_str(), "id" | "kind" | "aliases")
            })
            .map(|(k, v)| PropertyItem {
                key: k.clone(),
                values: format::property_display(v),
            })
            .collect(),
        summary_dir: summary.as_deref().map_or(TextDir::Neutral, dir_of),
        summary,
        summary_citations,
        insights: cited_bullets(conn, ctx, &section("Insights").unwrap_or_default())?,
        open_count: u32_of(i64::try_from(open_items.len()).unwrap_or(0)),
        done_count: 0,
        open_items,
        timeline: cited_bullets(conn, ctx, &section("Timeline").unwrap_or_default())?,
        mentions,
        related,
        documents,
        pending_sync: note_pending(&pending, &n.id),
        path: n.path.clone(),
        tags: parsed.tags.clone(),
        user_notes: user_notes(&parsed),
        ai_updated_label: None,
        mention_count,
        last_active_label: last_active.map(|t| {
            let d = labels.local(t).date();
            match ctx.lang {
                Lang::En => format!(
                    "last active {}",
                    labels.relative_day(d).to_lowercase_first()
                ),
                Lang::Ar => format!("آخر نشاط {}", labels.relative_day(d)),
            }
        }),
    })
}

fn is_entity(conn: &Connection, id: &str) -> CoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM entities WHERE note_id = ?1)",
        [id],
        |r| r.get(0),
    )?)
}

fn renewal_task(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<Option<TaskItem>> {
    let pending = pending_entities(conn)?;
    let default_time = default_reminder_time(conn)?;
    let resolver = crate::store::index::LinkResolver::load(conn)?;
    for row in task_rows(
        conn,
        "WHERE t.status = 'open' AND n.deleted = 0
         ORDER BY COALESCE(t.due, t.scheduled, '9999'), t.id",
        [],
    )? {
        let links = wikilink::find_all(&row.line);
        if links
            .iter()
            .any(|l| resolver.resolve(l.target(), &row.note_path).as_deref() == Some(id))
        {
            return Ok(Some(task_item(conn, ctx, row, &pending, default_time)?));
        }
    }
    Ok(None)
}

fn document_view(
    conn: &Connection,
    ctx: &ViewCtx,
    n: &crate::store::notes::NoteRow,
) -> CoreResult<DocumentView> {
    let labels = ctx.labels();
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
                    kind: None,
                }]
            })
            .unwrap_or_default(),
    };
    let mut copy_ids = Vec::new();
    if let Some(orig) = copy_of_id.clone() {
        copy_ids.push(orig);
    }
    {
        let mut st = conn.prepare(
            "SELECT d.note_id FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE d.copy_of_id = ?1 AND n.deleted = 0 ORDER BY n.title, n.id",
        )?;
        let more: Vec<String> = st
            .query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        copy_ids.extend(more);
    }
    let mut copies = Vec::new();
    let mut copy_briefs = Vec::new();
    for c in copy_ids {
        copies.extend(entity_ref(conn, Some(c.clone()), None)?);
        copy_briefs.push(brief_in_ctx(conn, ctx, &c)?);
    }
    let concerns = relation_chips(conn, ctx, OUTGOING_RELATIONS, &n.id)?
        .into_iter()
        .filter(|c| c.rel_type == "companies" || c.rel_type == "people")
        .map(|c| c.target)
        .collect();
    let expires = d.expires.as_deref().and_then(date);
    let holder = entity_ref(conn, holder_id, d.holder)?;
    let last_holder = entity_ref(conn, last_id, d.last_holder)?;
    let pending = pending_entities(conn)?;
    Ok(DocumentView {
        id: n.id.clone(),
        title_dir: dir_of(&parsed.display_title),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        doc_type: d.doc_type,
        copy: d.copy,
        status: d.status,
        expires_label: expires.map(|e| expires_label(&labels, e)),
        expiring_soon: expiring(&labels, expires),
        expires,
        location,
        holder_label: holder_label(
            &labels,
            holder.as_ref(),
            last_holder.as_ref(),
            last_moved(conn, &n.id)?,
        ),
        holder,
        last_holder,
        custody: custody_items(conn, ctx, None, "c.document_id = ?1", [&n.id])?,
        copies,
        concerns,
        path: n.path.clone(),
        pending_sync: note_pending(&pending, &n.id),
        renewal_task: renewal_task(conn, ctx, &n.id)?,
        mentions: note_items(
            conn,
            ctx,
            "WHERE n.deleted = 0 AND n.id != ?1 AND n.id IN (
                SELECT note_id FROM links WHERE dst_id = ?1
                UNION SELECT src_id FROM relations WHERE dst_id = ?1)
             ORDER BY n.local_updated_at DESC, n.id DESC",
            [&n.id],
        )?,
        copy_briefs,
        user_notes: user_notes(&parsed),
    })
}

const NESTED_PLACES: &str = "WITH RECURSIVE sub(id) AS (
        SELECT ?1 UNION SELECT p.note_id FROM places p JOIN sub ON p.parent_id = sub.id)";

fn place_tree(
    conn: &Connection,
    id: &str,
    depth: u32,
    seen: &mut HashSet<String>,
    out: &mut Vec<PlaceNode>,
) -> CoreResult<()> {
    let children: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT p.note_id FROM places p JOIN notes n ON n.id = p.note_id
             WHERE p.parent_id = ?1 AND n.deleted = 0 ORDER BY n.title, n.id",
        )?;
        st.query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for c in children {
        if !seen.insert(c.clone()) {
            continue;
        }
        let document_count = u32_of(conn.query_row(
            "SELECT COUNT(*) FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE n.deleted = 0 AND d.location_id = ?1",
            [&c],
            |r| r.get(0),
        )?);
        out.push(PlaceNode {
            place: note_ref(conn, &c)?,
            depth,
            document_count,
            parent_id: id.to_owned(),
        });
        place_tree(conn, &c, depth + 1, seen, out)?;
    }
    Ok(())
}

fn place_view(
    conn: &Connection,
    ctx: &ViewCtx,
    n: &crate::store::notes::NoteRow,
) -> CoreResult<PlaceView> {
    let parsed = format::parse_note(&n.path, &n.content);
    let mut tree = Vec::new();
    let mut seen = HashSet::from([n.id.clone()]);
    place_tree(conn, &n.id, 1, &mut seen, &mut tree)?;
    let sub_places = tree
        .iter()
        .filter(|p| p.depth == 1)
        .map(|p| p.place.clone())
        .collect();
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
        documents.push(brief_in_ctx(conn, ctx, &d)?);
    }
    // Held by a person now, last stored here or in a nested place.
    let out_ids: Vec<String> = {
        let mut st = conn.prepare(&format!(
            "{NESTED_PLACES}
             SELECT d.note_id FROM documents d JOIN notes n ON n.id = d.note_id
             WHERE n.deleted = 0 AND d.holder_id IS NOT NULL
               AND (d.location_id IN (SELECT id FROM sub) OR (
                    SELECT c.place_id FROM custody_events c
                    WHERE c.document_id = d.note_id AND c.place_id IS NOT NULL
                    ORDER BY c.at DESC, c.ord LIMIT 1) IN (SELECT id FROM sub))
             ORDER BY n.title, n.id"
        ))?;
        st.query_map([&n.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    let mut out_with_people = Vec::new();
    for d in out_ids {
        out_with_people.push(brief_in_ctx(conn, ctx, &d)?);
    }
    let mut recent = Vec::new();
    for place in std::iter::once(n.id.clone()).chain(tree.iter().filter_map(|p| p.place.id.clone()))
    {
        recent.extend(custody_items(
            conn,
            ctx,
            Some(&n.id),
            "c.place_id = ?1",
            [&place],
        )?);
    }
    recent.sort_by(|a, b| b.date.cmp(&a.date));
    recent.truncate(20);
    Ok(PlaceView {
        id: n.id.clone(),
        title_dir: dir_of(&parsed.display_title),
        title: parsed.display_title.clone(),
        aliases: parsed.aliases.clone(),
        breadcrumb: breadcrumb(conn, &n.id, false)?,
        sub_places,
        documents,
        recent_movements: recent,
        tree,
        out_with_people,
        user_notes: user_notes(&parsed),
        path: n.path.clone(),
    })
}

/// Entity / document / place page.
pub fn entity_screen(conn: &Connection, ctx: &ViewCtx, id: &str) -> CoreResult<EntityScreen> {
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
            screen.entity = Some(entity_view(conn, ctx, &n)?);
        }
        "document" => {
            screen.kind = EntityPageKind::Document;
            screen.document = Some(document_view(conn, ctx, &n)?);
        }
        "place" => {
            screen.kind = EntityPageKind::Place;
            screen.place = Some(place_view(conn, ctx, &n)?);
        }
        _ => {}
    }
    Ok(screen)
}

// ---------------------------------------------------------------------------------------------
// Graph, ask, settings, session
// ---------------------------------------------------------------------------------------------

/// Scopes Ask offers: all notes, then people and companies (A–Z), then top-level folders.
pub fn ask_scopes(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Vec<AskScope>> {
    let mut out = vec![AskScope {
        kind: AskScopeKind::All,
        value: None,
        label: tr(ctx.lang, "All notes", "كل الملاحظات"),
    }];
    let rows: Vec<(String, String)> = {
        let mut st = conn.prepare(
            "SELECT e.note_id, e.display_name FROM entities e JOIN notes n ON n.id = e.note_id
             WHERE n.deleted = 0 AND e.kind IN ('person', 'company')
             ORDER BY e.display_name COLLATE NOCASE, e.note_id",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    out.extend(rows.into_iter().map(|(id, name)| AskScope {
        kind: AskScopeKind::Entity,
        value: Some(id),
        label: name,
    }));
    let mut folders: Vec<String> = crate::store::notes::live_paths(conn)?
        .into_iter()
        .filter_map(|(_, p)| p.split_once('/').map(|(f, _)| f.to_owned()))
        .collect();
    folders.sort();
    folders.dedup();
    out.extend(folders.into_iter().map(|f| AskScope {
        kind: AskScopeKind::Folder,
        label: f.clone(),
        value: Some(f),
    }));
    Ok(out)
}

/// The Ask scope value the server takes (`None` = everything): an entity's path or a folder.
pub fn ask_scope_value(conn: &Connection, scope: &AskScope) -> CoreResult<Option<String>> {
    Ok(match (scope.kind, &scope.value) {
        (AskScopeKind::All, _) | (_, None) => None,
        (AskScopeKind::Entity, Some(id)) => conn
            .query_row("SELECT path FROM notes WHERE id = ?1", [id], |r| r.get(0))
            .optional()?,
        (AskScopeKind::Folder, Some(f)) => Some(f.clone()),
    })
}

/// Splits an answer into text runs and citation markers: `[[target]]` links whose target a
/// citation names become markers of that citation.
pub fn ask_spans(text: &str, citations: &[crate::session::ask::AskCitation]) -> Vec<AskSpan> {
    let mut out = Vec::new();
    let mut last = 0;
    for l in wikilink::find_all(text) {
        let target = l.target();
        let hit = citations.iter().find(|c| c.target == target);
        if let Some(c) = hit {
            if l.span.start > last {
                out.push(AskSpan {
                    text: text[last..l.span.start].to_owned(),
                    citation: None,
                });
            }
            out.push(AskSpan {
                text: String::new(),
                citation: Some(c.index),
            });
            last = l.span.end;
        }
    }
    if last < text.len() {
        out.push(AskSpan {
            text: text[last..].to_owned(),
            citation: None,
        });
    }
    out
}

fn ai_status_view(ctx: &ViewCtx, s: &crate::net::AiStatusInfo) -> AiStatusView {
    let labels = ctx.labels();
    let percent = if s.tokens_limit == 0 {
        0
    } else {
        u32::try_from((s.tokens_used.saturating_mul(100) / s.tokens_limit).min(100)).unwrap_or(100)
    };
    AiStatusView {
        enabled: s.enabled,
        provider: s.provider.clone(),
        paused_label: s.paused_until.map(|u| match ctx.lang {
            Lang::En => format!("Paused until {}", labels.moment_label(u)),
            Lang::Ar => format!("متوقف حتى {}", labels.moment_label(u)),
        }),
        queue_depth: u32::try_from(s.queue_depth).unwrap_or(u32::MAX),
        budget_used_percent: percent,
        budget_label: match ctx.lang {
            Lang::En => format!("{percent}% used"),
            Lang::Ar => format!("استُخدم {percent}%"),
        },
        embedding_percent: s.embedded.map(|(done, total)| {
            if total == 0 {
                100
            } else {
                u32::try_from((done * 100 / total).min(100)).unwrap_or(100)
            }
        }),
    }
}

/// Ask: the conversation (online only).
pub fn ask(
    conn: &Connection,
    ctx: &ViewCtx,
    entries: &[crate::session::ask::AskEntry],
) -> CoreResult<AskView> {
    let labels = ctx.labels();
    let messages = entries
        .iter()
        .map(|e| {
            let mut sources: Vec<AskSource> = Vec::new();
            let mut cites = e.citations.clone();
            cites.sort_by_key(|c| c.index);
            for c in &cites {
                match sources.iter_mut().find(|s| s.note_id == c.note_id) {
                    Some(s) => {
                        s.indexes.push(c.index);
                        if let Some(b) = &c.block_id
                            && !s.anchors.contains(b)
                        {
                            s.anchors.push(b.clone());
                        }
                    }
                    None => sources.push(AskSource {
                        note_id: c.note_id.clone(),
                        title: c.title.clone(),
                        path: c.path.clone(),
                        anchors: c.block_id.iter().cloned().collect(),
                        indexes: vec![c.index],
                    }),
                }
            }
            AskMessage {
                role: e.role.clone(),
                text: e.text.clone(),
                citations: cites
                    .iter()
                    .map(|c| Citation {
                        note_id: Some(c.note_id.clone()),
                        target: c.target.clone(),
                        anchor: c.block_id.clone(),
                    })
                    .collect(),
                id: e.id.clone(),
                streaming: e.streaming,
                spans: ask_spans(&e.text, &cites),
                source_count: u32::try_from(sources.len()).unwrap_or(u32::MAX),
                sources,
                scope_label: match ctx.lang {
                    Lang::En => format!("Scope: {}", e.scope_label),
                    Lang::Ar => format!("النطاق: {}", e.scope_label),
                },
                created_label: labels.moment_label(e.created),
                dir: dir_of(&e.text),
                error_key: e.error_key.clone(),
                saved_note_id: e.saved_note_id.clone(),
            }
        })
        .collect::<Vec<_>>();
    let ai_status = cache::get::<crate::net::AiStatusInfo>(conn, cache::AI_STATUS)?
        .map(|(s, _)| ai_status_view(ctx, &s));
    let availability = if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else if ai_status.as_ref().is_some_and(|s| !s.enabled) {
        Availability::NotAllowed
    } else {
        Availability::Available
    };
    Ok(AskView {
        availability,
        streaming: entries.iter().any(|e| e.streaming),
        messages,
        scopes: ask_scopes(conn, ctx)?,
        ai_status,
    })
}

/// The account summary.
pub fn account_summary(conn: &Connection) -> CoreResult<Option<AccountSummary>> {
    Ok(account::get(conn)?.map(|a| AccountSummary {
        is_admin: a.role == "admin",
        initials: labels::initials(&a.display_name),
        user_id: a.user_id,
        username: a.username,
        display_name: a.display_name,
        role: a.role,
        server_url: a.server_url,
        timezone: a.timezone,
        ui_language: a.ui_language,
    }))
}

/// The account's devices from the cache (`None` until fetched), most recently seen first.
pub fn device_items(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Option<Vec<DeviceItem>>> {
    let labels = ctx.labels();
    let this = crate::sync::apply::device_id(conn)?;
    Ok(
        cache::get::<Vec<crate::net::DeviceInfo>>(conn, cache::DEVICES)?.map(|(mut list, _)| {
            list.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.id.cmp(&b.id)));
            list.into_iter()
                .map(|d| DeviceItem {
                    is_this_device: d.current || this.as_deref() == Some(d.id.as_str()),
                    last_seen_label: if (ctx.now - d.last_seen).num_minutes() < 5 {
                        tr(ctx.lang, "Active now", "نشط الآن")
                    } else {
                        labels.list_label(d.last_seen)
                    },
                    signed_in_label: labels.date_long(labels.local(d.created).date()),
                    id: d.id,
                    name: d.name,
                    platform: d.platform,
                    last_seen: d.last_seen,
                    signed_in: d.created,
                    reminders_enabled: d.reminders_enabled,
                })
                .collect()
        }),
    )
}

/// "18.4 MB · 412 notes".
pub fn export_label(labels: &Labels, bytes: u64, notes: u32) -> String {
    format!(
        "{} · {}",
        labels.bytes(bytes),
        NOTES.of(i64::from(notes), labels.lang)
    )
}

/// Settings.
pub fn settings_view(conn: &Connection, ctx: &ViewCtx) -> CoreResult<Option<SettingsView>> {
    let Some(account) = account_summary(conn)? else {
        return Ok(None);
    };
    let labels = ctx.labels();
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
    let online = if ctx.connectivity == Connectivity::Offline {
        Availability::Offline
    } else {
        Availability::Available
    };
    let admin = if account.is_admin {
        online
    } else {
        Availability::NotAllowed
    };
    let (quiet_enabled, quiet_from, quiet_until) = settings::quiet_hours(conn)?;
    let devices = cache::get::<Vec<crate::net::DeviceInfo>>(conn, cache::DEVICES)?;
    let refreshed_label = devices.as_ref().map(|(_, at)| match ctx.lang {
        Lang::En => format!("Updated {}", labels.moment_label(ts(at))),
        Lang::Ar => format!("حُدّث {}", labels.moment_label(ts(at))),
    });
    let ai_status = cache::get::<crate::net::AiStatusInfo>(conn, cache::AI_STATUS)?
        .map(|(s, _)| ai_status_view(ctx, &s));
    let integrity_warnings: Vec<IntegrityItem> =
        cache::get::<Vec<crate::net::IntegrityInfo>>(conn, cache::INTEGRITY)?
            .map(|(mut w, _)| {
                w.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
                w.into_iter()
                    .map(|w| IntegrityItem {
                        message_key: format!("integrity.{}", w.kind),
                        created_label: labels.moment_label(w.created),
                        id: w.id,
                        kind: w.kind,
                        path: w.path,
                    })
                    .collect()
            })
            .unwrap_or_default();
    Ok(Some(SettingsView {
        reminders: RemindersSetting {
            enabled: settings::reminders_enabled(conn)?,
            permission,
            mode: ctx.notification_mode,
            scheduled: u32_of(scheduled),
            default_time: default_reminder_time(conn)?.format("%H:%M").to_string(),
            snooze_minutes: settings::snooze_minutes(conn)?,
            quiet_enabled,
            quiet_from,
            quiet_until,
        },
        devices: online,
        ai: if ai_status.is_some() {
            Availability::Available
        } else {
            online
        },
        export: online,
        integrity: if integrity_warnings.is_empty() && devices.is_none() {
            online
        } else {
            Availability::Available
        },
        admin,
        device_list: device_items(conn, ctx)?.unwrap_or_default(),
        ai_status,
        integrity_warnings,
        refreshed_label,
        account,
    }))
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
mod hunk_tests {
    use super::*;

    #[test]
    fn own_text_gets_a_line_terminator() {
        let text = |t: &str| match hunk_choice(HunkChoiceKind::Text, Some(t.to_owned())) {
            sync_model::Choice::Text(t) => t,
            _ => String::new(),
        };
        assert_eq!(text("Mine"), "Mine\n");
        assert_eq!(text("Mine\n"), "Mine\n");
        assert_eq!(text(""), "");
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
