//! Builders of the smaller views and one-shot reads added for the screens' actions:
//! editor completions, tags, blocks, task homes, the custody place picker, merge previews,
//! citation previews and the task-text preview.

use rusqlite::{Connection, OptionalExtension, params};
use vault_format::Document;

use crate::error::CoreResult;
use crate::format::completions::{Token, token_at};
use crate::format::direction::dir_of;
use crate::format::hints::Utf16Map;
use crate::format::labels::{Lang, tr};
use crate::format::{self, task_text};
use crate::store::index::LinkResolver;
use crate::view::ViewCtx;
use crate::view::build::{breadcrumb, entity_ref, note_ref, resolve_display};
use crate::view::model::{
    BlockItem, CitationPreview, CompletionItem, CompletionKind, Completions, EntityRef,
    MergePreview, PlaceOption, TagItem, TaskChip, TaskChipKind, TaskDraft, TaskDraftPreview,
    TaskHomeItem, TaskHomesView,
};

/// Most completion items.
pub const COMPLETION_LIMIT: usize = 20;

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Tags with note counts, filtered by prefix (case-insensitive), most used first.
pub fn tags(conn: &Connection, prefix: &str) -> CoreResult<Vec<TagItem>> {
    let p = prefix.trim_start_matches('#').to_lowercase();
    let mut st = conn.prepare(
        "SELECT t.tag, COUNT(*) FROM tags t JOIN notes n ON n.id = t.note_id
         WHERE n.deleted = 0 GROUP BY t.tag ORDER BY COUNT(*) DESC, t.tag",
    )?;
    let rows: Vec<(String, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(t, _)| t.to_lowercase().starts_with(&p))
        .map(|(tag, n)| TagItem {
            tag,
            count: u32::try_from(n).unwrap_or(u32::MAX),
        })
        .collect())
}

/// The blocks of a note (block reference picker), in order.
pub fn blocks(conn: &Connection, note_id: &str) -> CoreResult<Vec<BlockItem>> {
    let Some(n) = crate::store::notes::current(conn, note_id)? else {
        return Ok(Vec::new());
    };
    let doc = Document::parse(&n.content);
    let off = doc.body_offset();
    let first_line = n.content[..off].matches('\n').count();
    let body = doc.body();
    let a = doc.analyze_body();
    Ok(a.blocks
        .iter()
        .filter(|b| {
            !matches!(
                b.kind,
                vault_format::body::BlockKind::Heading | vault_format::body::BlockKind::CodeBlock
            )
        })
        .map(|b| {
            let raw = &body[b.span.clone()];
            let text: String = raw
                .trim()
                .trim_start_matches(['-', '*', '>', ' '])
                .split(" ^")
                .next()
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect();
            BlockItem {
                block_id: b.id.as_ref().map(|i| i.id.clone()),
                text_dir: dir_of(&text),
                text,
                line: u32_of(first_line + body[..b.span.start].matches('\n').count() + 1),
            }
        })
        .collect())
}

fn note_candidates(
    conn: &Connection,
    query: &str,
) -> CoreResult<Vec<(String, String, String, String)>> {
    let q = text_normalize::normalize_for_search(query.trim());
    let mut st = conn.prepare(
        "SELECT id, title, path, kind FROM notes WHERE deleted = 0 ORDER BY local_updated_at DESC, id",
    )?;
    let rows: Vec<(String, String, String, String)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let mut scored: Vec<(u8, (String, String, String, String))> = rows
        .into_iter()
        .filter_map(|r| {
            let t = text_normalize::normalize_for_search(&r.1);
            let rank = if q.is_empty() {
                2
            } else if t.starts_with(&q) {
                0
            } else if t.contains(&q) {
                1
            } else {
                return None;
            };
            Some((rank, r))
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(scored
        .into_iter()
        .take(COMPLETION_LIMIT)
        .map(|(_, r)| r)
        .collect())
}

/// Editor completions for the token before `cursor` (UTF-16) in `content` of note `note_id`.
#[allow(clippy::too_many_lines)] // one arm per completion kind
pub fn completions(
    conn: &Connection,
    ctx: &ViewCtx,
    note_id: &str,
    content: &str,
    cursor: u32,
) -> CoreResult<Completions> {
    let map = Utf16Map::new(content);
    let byte = map.byte_of(cursor);
    let path = crate::store::notes::current(conn, note_id)?
        .map(|n| n.path)
        .unwrap_or_default();
    let resolver = LinkResolver::load(conn)?;
    let none = || Completions {
        kind: CompletionKind::None,
        replace_start: cursor,
        replace_end: cursor,
        query: String::new(),
        items: Vec::new(),
    };
    let lang = ctx.lang;
    Ok(match token_at(content, byte) {
        Token::None => none(),
        Token::WikiLink {
            query,
            start,
            end,
            closed,
        } => {
            let items = note_candidates(conn, &query)?
                .into_iter()
                .filter(|r| r.0 != note_id)
                .map(|(id, title, p, kind)| {
                    let link = resolver.link_text_for(&p);
                    CompletionItem {
                        label_dir: dir_of(&title),
                        label: title,
                        detail: p,
                        insert_text: if closed { link } else { format!("{link}]]") },
                        target_id: Some(id),
                        entity_kind: Some(kind),
                    }
                })
                .collect();
            Completions {
                kind: CompletionKind::WikiLink,
                replace_start: map.at(start),
                replace_end: map.at(end),
                query,
                items,
            }
        }
        Token::BlockRef {
            note,
            query,
            start,
            end,
            closed,
        } => {
            let target = resolver.resolve(&note, &path);
            let mut items = Vec::new();
            if let Some(t) = &target {
                for b in blocks(conn, t)? {
                    if !query.is_empty()
                        && !b.block_id.as_deref().is_some_and(|i| i.starts_with(&query))
                        && !b.text.to_lowercase().contains(&query.to_lowercase())
                    {
                        continue;
                    }
                    let Some(id) = b.block_id.clone() else {
                        continue;
                    };
                    items.push(CompletionItem {
                        label: b.text.clone(),
                        label_dir: b.text_dir,
                        detail: format!("^{id}"),
                        insert_text: if closed { id } else { format!("{id}]]") },
                        target_id: Some(t.clone()),
                        entity_kind: None,
                    });
                    if items.len() >= COMPLETION_LIMIT {
                        break;
                    }
                }
            }
            Completions {
                kind: CompletionKind::BlockRef,
                replace_start: map.at(start),
                replace_end: map.at(end),
                query,
                items,
            }
        }
        Token::Mention { query, start, end } => {
            let q = text_normalize::normalize_for_search(query.trim());
            let mut st = conn.prepare(
                "SELECT e.note_id, e.display_name, e.kind, n.path FROM entities e
                 JOIN notes n ON n.id = e.note_id
                 WHERE n.deleted = 0 AND e.kind IN ('person', 'company')
                   AND (?1 = '' OR EXISTS (SELECT 1 FROM entity_aliases a
                        WHERE a.note_id = e.note_id AND instr(a.alias_normalized, ?1) > 0))
                 ORDER BY e.display_name COLLATE NOCASE, e.note_id LIMIT ?2",
            )?;
            let rows: Vec<(String, String, String, String)> = st
                .query_map(
                    params![q, i64::try_from(COMPLETION_LIMIT).unwrap_or(20)],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )?
                .collect::<Result<_, _>>()?;
            Completions {
                kind: CompletionKind::Mention,
                replace_start: map.at(start),
                replace_end: map.at(end),
                query,
                items: rows
                    .into_iter()
                    .map(|(id, name, kind, p)| CompletionItem {
                        label_dir: dir_of(&name),
                        insert_text: format!("[[{}]]", resolver.link_text_for(&p)),
                        detail: crate::format::labels::kind_label(&kind, lang),
                        label: name,
                        target_id: Some(id),
                        entity_kind: Some(kind),
                    })
                    .collect(),
            }
        }
        Token::Tag { query, start, end } => Completions {
            kind: CompletionKind::Tag,
            replace_start: map.at(start),
            replace_end: map.at(end),
            items: tags(conn, &query)?
                .into_iter()
                .take(COMPLETION_LIMIT)
                .map(|t| CompletionItem {
                    label: format!("#{}", t.tag),
                    label_dir: dir_of(&t.tag),
                    detail: crate::format::labels::NOTES.of(i64::from(t.count), lang),
                    insert_text: t.tag,
                    target_id: None,
                    entity_kind: None,
                })
                .collect(),
            query,
        },
    })
}

/// Candidate homes of a new task: `tasks/Tasks.md` first (created on first use), then
/// notes with open tasks, by title.
pub fn task_homes(conn: &Connection, ctx: &ViewCtx) -> CoreResult<TaskHomesView> {
    let home: Option<(String, String)> = conn
        .query_row(
            "SELECT id, title FROM notes WHERE deleted = 0 AND path = ?1",
            [crate::store::write::TASK_HOME],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let open_of = |id: &str| -> CoreResult<u32> {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE note_id = ?1 AND status = 'open'",
            [id],
            |r| r.get(0),
        )?)
    };
    let mut homes = vec![TaskHomeItem {
        open_tasks: home
            .as_ref()
            .map(|h| open_of(&h.0))
            .transpose()?
            .unwrap_or(0),
        note_id: home.as_ref().map(|h| h.0.clone()),
        title: home.map_or_else(|| tr(ctx.lang, "Tasks", "المهام"), |h| h.1),
        path: crate::store::write::TASK_HOME.to_owned(),
        is_default: true,
    }];
    let mut st = conn.prepare(
        "SELECT n.id, n.title, n.path, COUNT(*) FROM tasks t JOIN notes n ON n.id = t.note_id
         WHERE n.deleted = 0 AND t.status = 'open' AND n.path != ?1
         GROUP BY n.id ORDER BY n.title COLLATE NOCASE, n.id",
    )?;
    let rows: Vec<(String, String, String, u32)> = st
        .query_map([crate::store::write::TASK_HOME], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    homes.extend(rows.into_iter().map(|(id, title, path, n)| TaskHomeItem {
        note_id: Some(id),
        title,
        path,
        is_default: false,
        open_tasks: n,
    }));
    Ok(TaskHomesView { homes })
}

/// Depth-first walk of the place tree for the picker.
fn walk_places(
    conn: &Connection,
    rows: &[(String, Option<String>)],
    parent: Option<&str>,
    depth: u32,
    current: Option<&str>,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<PlaceOption>,
) -> CoreResult<()> {
    for (id, _) in rows.iter().filter(|r| r.1.as_deref() == parent) {
        if !seen.insert(id.clone()) {
            continue;
        }
        let r = note_ref(conn, id)?;
        out.push(PlaceOption {
            id: id.clone(),
            title: r.title,
            breadcrumb: breadcrumb(conn, id, false)?,
            depth,
            is_current: current == Some(id.as_str()),
        });
        walk_places(conn, rows, Some(id), depth + 1, current, seen, out)?;
    }
    Ok(())
}

/// Places for the custody picker, depth-first with breadcrumbs; `document_id` marks where
/// that document is now.
pub fn place_options(conn: &Connection, document_id: Option<&str>) -> CoreResult<Vec<PlaceOption>> {
    let current: Option<String> = match document_id {
        Some(d) => conn
            .query_row(
                "SELECT location_id FROM documents WHERE note_id = ?1",
                [d],
                |r| r.get(0),
            )
            .optional()?
            .flatten(),
        None => None,
    };
    let rows: Vec<(String, Option<String>)> = {
        let mut st = conn.prepare(
            "SELECT p.note_id, p.parent_id FROM places p JOIN notes n ON n.id = p.note_id
             WHERE n.deleted = 0 ORDER BY n.title COLLATE NOCASE, n.id",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let ids: std::collections::HashSet<&String> = rows.iter().map(|r| &r.0).collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Roots: no parent, or a parent that is not a known place.
    let roots: Vec<(String, Option<String>)> = rows
        .iter()
        .filter(|r| r.1.as_ref().is_none_or(|p| !ids.contains(p)))
        .map(|r| (r.0.clone(), None))
        .collect();
    for (root, _) in &roots {
        if !seen.insert(root.clone()) {
            continue;
        }
        let r = note_ref(conn, root)?;
        out.push(PlaceOption {
            id: root.clone(),
            title: r.title,
            breadcrumb: Vec::new(),
            depth: 0,
            is_current: current.as_deref() == Some(root.as_str()),
        });
        walk_places(
            conn,
            &rows,
            Some(root),
            1,
            current.as_deref(),
            &mut seen,
            &mut out,
        )?;
    }
    Ok(out)
}

/// What merging `source` into `into` moves.
pub fn merge_preview(conn: &Connection, source: &str, into: &str) -> CoreResult<MergePreview> {
    let s = note_ref(conn, source)?;
    let i = note_ref(conn, into)?;
    let mut aliases: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT alias FROM entity_aliases WHERE note_id = ?1 AND alias NOT IN (
                SELECT alias FROM entity_aliases WHERE note_id = ?2) ORDER BY alias",
        )?;
        st.query_map(params![source, into], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    if !s.title.is_empty() && !aliases.contains(&s.title) {
        aliases.insert(0, s.title.clone());
    }
    let mention_count: u32 = conn.query_row(
        "SELECT COUNT(DISTINCT n.id) FROM notes n WHERE n.deleted = 0 AND n.id != ?1 AND n.id IN (
            SELECT note_id FROM links WHERE dst_id = ?1
            UNION SELECT src_id FROM relations WHERE dst_id = ?1)",
        [source],
        |r| r.get(0),
    )?;
    let relation_count: u32 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM relations WHERE src_id = ?1)
              + (SELECT COUNT(*) FROM relations WHERE dst_id = ?1)",
        [source],
        |r| r.get(0),
    )?;
    Ok(MergePreview {
        source: s,
        into: i,
        aliases,
        mention_count,
        relation_count,
    })
}

/// The block a citation points to.
pub fn citation_preview(
    conn: &Connection,
    ctx: &ViewCtx,
    note_id: &str,
    anchor: Option<&str>,
) -> CoreResult<CitationPreview> {
    let Some(n) = crate::store::notes::current(conn, note_id)? else {
        return Ok(CitationPreview {
            note_id: None,
            title: String::new(),
            path: String::new(),
            block_text: None,
            block_dir: crate::view::model::TextDir::Neutral,
            heading: None,
            date_label: None,
            tags: Vec::new(),
            anchor: None,
            line: None,
            offset: None,
        });
    };
    let parsed = format::parse_note(&n.path, &n.content);
    let doc = Document::parse(&n.content);
    let body = doc.body();
    let off = doc.body_offset();
    let a = doc.analyze_body();
    let anchor = anchor.map(|a| a.trim_start_matches('^'));
    // Where the block starts in the whole content: its line and UTF-16 offset.
    let at = |body_start: usize| {
        let start = off + body_start;
        let line = n.content[..start].matches('\n').count();
        (
            Some(u32::try_from(line).unwrap_or(u32::MAX)),
            Some(Utf16Map::new(&n.content).at(start)),
        )
    };
    let (block_text, heading, found, (line, offset)) = match anchor {
        Some(id) => match a
            .blocks
            .iter()
            .find(|b| b.id.as_ref().is_some_and(|i| i.id == id))
        {
            Some(b) => {
                let raw = &body[b.span.clone()];
                let text = raw
                    .trim()
                    .trim_start_matches(['-', '*', '>', ' '])
                    .replace(&format!("^{id}"), "")
                    .trim()
                    .to_owned();
                (
                    Some(text),
                    b.heading_path.last().cloned(),
                    Some(id.to_owned()),
                    at(b.span.start),
                )
            }
            None => match a.headings.iter().find(|h| h.text == id) {
                Some(h) => (
                    Some(h.text.clone()),
                    Some(h.text.clone()),
                    Some(h.text.clone()),
                    at(h.span.start),
                ),
                None => (None, None, None, (None, None)),
            },
        },
        None => (
            Some(crate::view::build::snippet(&n.content, 280)),
            None,
            None,
            (None, None),
        ),
    };
    let labels = ctx.labels();
    let date_label = parsed
        .created
        .as_deref()
        .and_then(|c| chrono::DateTime::parse_from_rfc3339(c).ok())
        .map(|c| labels.date_long(labels.local(c.with_timezone(&chrono::Utc)).date()));
    Ok(CitationPreview {
        note_id: Some(note_id.to_owned()),
        title: parsed.display_title.clone(),
        path: n.path,
        block_dir: block_text
            .as_deref()
            .map_or(crate::view::model::TextDir::Neutral, dir_of),
        block_text,
        heading,
        date_label,
        tags: parsed.tags,
        anchor: found,
        line,
        offset,
    })
}

/// Most repoint choices listed.
pub const REPOINT_LIMIT: usize = 50;

/// New targets for the AI decision `decision_id` of the cached activity feed (Home →
/// Repoint): live notes of the current target's kind whose title matches `query`
/// (normalised: case, Arabic letter variants, diacritics), never the current target or the
/// source, by title. Empty when the decision is unknown or cannot be repointed.
pub fn repoint_choices(
    conn: &Connection,
    decision_id: &str,
    query: &str,
) -> CoreResult<Vec<crate::view::model::RepointChoice>> {
    let Some((decisions, _)) = crate::store::cache::get::<Vec<crate::net::AiDecisionInfo>>(
        conn,
        crate::store::cache::AI_DECISIONS,
    )?
    else {
        return Ok(Vec::new());
    };
    let Some(d) = decisions.iter().find(|d| d.id == decision_id) else {
        return Ok(Vec::new());
    };
    let kind: Option<String> = conn
        .query_row(
            "SELECT kind FROM notes WHERE id = ?1 AND deleted = 0",
            [&d.target_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(kind) = kind else {
        return Ok(Vec::new());
    };
    let source = d.source_note_id.clone().unwrap_or_default();
    let q = text_normalize::normalize_for_search(query.trim());
    let mut st = conn.prepare(
        "SELECT id, title, path FROM notes
         WHERE deleted = 0 AND kind = ?1 AND id != ?2 AND id != ?3",
    )?;
    let rows: Vec<(String, String, String)> = st
        .query_map(params![kind, d.target_id, source], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out: Vec<crate::view::model::RepointChoice> = rows
        .into_iter()
        .filter(|(_, title, _)| {
            q.is_empty() || text_normalize::normalize_for_search(title).contains(&q)
        })
        .map(|(id, title, path)| crate::view::model::RepointChoice {
            title_dir: dir_of(&title),
            folder: path
                .rsplit_once('/')
                .map(|(f, _)| f.to_owned())
                .unwrap_or_default(),
            kind: kind.clone(),
            id,
            title,
        })
        .collect();
    out.sort_by(|a, b| (a.title.to_lowercase(), &a.id).cmp(&(b.title.to_lowercase(), &b.id)));
    out.truncate(REPOINT_LIMIT);
    Ok(out)
}

/// The new-task sheet's "Understood as": the text parsed, `@mentions` resolved to people and
/// companies (their links written into the description).
#[allow(clippy::too_many_lines)] // one chip per understood piece
pub fn task_draft_preview(
    conn: &Connection,
    ctx: &ViewCtx,
    text: &str,
) -> CoreResult<TaskDraftPreview> {
    let labels = ctx.labels();
    let parsed = task_text::parse(text, labels.today());
    let resolver = LinkResolver::load(conn)?;
    let mut description = parsed.description.clone();
    let mut links: Vec<EntityRef> = Vec::new();
    for name in &parsed.mentions {
        let q = text_normalize::normalize_for_search(name);
        let hit: Option<(String, String)> = conn
            .query_row(
                "SELECT e.note_id, n.path FROM entities e JOIN notes n ON n.id = e.note_id
                 JOIN entity_aliases a ON a.note_id = e.note_id
                 WHERE n.deleted = 0 AND a.alias_normalized = ?1
                 ORDER BY e.note_id LIMIT 1",
                [&q],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, path)) = hit {
            let link = format!("[[{}]]", resolver.link_text_for(&path));
            description = description.replacen(&format!("@{}", name.replace(' ', "_")), &link, 1);
            description = description.replacen(&format!("@{name}"), &link, 1);
            links.extend(entity_ref(conn, Some(id), None)?);
        }
    }
    for l in vault_format::wikilink::find_all(&description) {
        let r = resolve_display(conn, l.target())?;
        if !links.iter().any(|x| x.id.is_some() && x.id == r.id) {
            links.push(r);
        }
    }
    let lang = ctx.lang;
    let mut chips = Vec::new();
    for (piece, _) in &parsed.pieces {
        use task_text::Piece;
        match piece {
            Piece::Due => {
                if let Some(d) = parsed.due {
                    chips.push(TaskChip {
                        kind: TaskChipKind::Due,
                        label: labels.relative_day(d),
                    });
                }
            }
            Piece::Recurrence => {
                if let Some(r) = parsed
                    .recurrence
                    .as_deref()
                    .and_then(|r| vault_format::tasks::parse_recurrence(r).ok())
                {
                    chips.push(TaskChip {
                        kind: TaskChipKind::Recurrence,
                        label: format::recurrence::summary(&r, &labels),
                    });
                }
            }
            Piece::Time | Piece::Mention => {}
            Piece::Priority => chips.push(TaskChip {
                kind: TaskChipKind::Priority,
                label: match (parsed.priority.as_deref(), lang) {
                    (Some("highest"), Lang::En) => "Urgent".to_owned(),
                    (Some("high"), Lang::En) => "High priority".to_owned(),
                    (Some("low"), Lang::En) => "Low priority".to_owned(),
                    (Some("highest"), Lang::Ar) => "عاجل".to_owned(),
                    (Some("high"), Lang::Ar) => "أولوية عالية".to_owned(),
                    (Some("low"), Lang::Ar) => "أولوية منخفضة".to_owned(),
                    _ => String::new(),
                },
            }),
        }
    }
    for r in &parsed.reminders {
        chips.push(TaskChip {
            kind: TaskChipKind::Reminder,
            label: labels.local_time_label(*r),
        });
    }
    for l in &links {
        chips.push(TaskChip {
            kind: TaskChipKind::Link,
            label: l.title.clone(),
        });
    }
    Ok(TaskDraftPreview {
        description_dir: dir_of(&description),
        due_label: parsed.due.map(|d| labels.relative_day(d)),
        draft: TaskDraft {
            note_id: None,
            description: description.clone(),
            due: parsed.due,
            scheduled: None,
            recurrence: parsed.recurrence.clone(),
            reminders: parsed.reminders.clone(),
            priority: parsed.priority.clone(),
        },
        description,
        due: parsed.due,
        recurrence: parsed.recurrence,
        reminders: parsed.reminders,
        priority: parsed.priority,
        links,
        chips,
    })
}

/// Inserts an `@mention` as a link (path-disambiguated when titles collide, §6.3) and adds the
/// entity to `people:` / `companies:` of the note, as one content change for `update_note`.
/// `start..end` (UTF-16) is the typed `@query`.
pub fn insert_mention(
    conn: &Connection,
    content: &str,
    start: u32,
    end: u32,
    entity_id: &str,
) -> CoreResult<crate::view::model::MentionEdit> {
    use crate::error::CoreError;
    let (path, kind): (String, String) = conn
        .query_row(
            "SELECT n.path, e.kind FROM notes n JOIN entities e ON e.note_id = n.id
             WHERE n.id = ?1 AND n.deleted = 0",
            [entity_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| CoreError::not_found("entity"))?;
    let key: vault_format::RelationKey = match kind.as_str() {
        "person" => "people",
        "company" => "companies",
        _ => return Err(CoreError::invalid("entity_id", "not_a_person_or_company")),
    }
    .parse()
    .map_err(|_| CoreError::Internal("relation key".into()))?;
    let resolver = LinkResolver::load(conn)?;
    let link_text = resolver.link_text_for(&path);
    let link = format!("[[{link_text}]]");
    let map = Utf16Map::new(content);
    let (a, b) = (map.byte_of(start), map.byte_of(end.max(start)));
    let replaced = format!("{}{}{}", &content[..a], link, &content[b..]);
    let cursor = map.at(a) + u32::try_from(link.encode_utf16().count()).unwrap_or(0);
    let mut doc = Document::parse(&replaced);
    if let Some(e) = doc.frontmatter().and_then(|f| f.error()) {
        return Err(CoreError::invalid("frontmatter", &e.to_string()));
    }
    let before = doc.body_offset();
    doc.frontmatter_mut()
        .add_relation_link(key, &link_text)
        .map_err(|e| CoreError::invalid("frontmatter", &e.to_string()))?;
    let rendered = doc.render();
    let grown = Document::parse(&rendered).body_offset();
    let delta = Utf16Map::new(&rendered).at(grown) - Utf16Map::new(&replaced).at(before);
    Ok(crate::view::model::MentionEdit {
        content: rendered,
        cursor: if a >= before { cursor + delta } else { cursor },
    })
}
