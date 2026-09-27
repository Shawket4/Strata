//! Derived tables (tags, links, relations, entities, places, documents, custody, tasks,
//! reminders, inbox, full-text) computed from note content with `vault-format` — the same
//! rules the server's indexer uses (L16), so the local cache agrees with the server's index.
//!
//! Writes collect touched notes in a [`Reindex`]; [`Reindex::apply`] re-derives them and, when
//! a path appeared, moved or disappeared, re-resolves every stored link (Obsidian resolution
//! depends on the whole set of paths).

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, params};
use vault_format::{PathIndex, Resolution};

use crate::error::CoreResult;
use crate::format::{self, ParsedNote};
use crate::store::{notes, to_msgpack};
use crate::view::Topics;

/// Resolves link paths to note IDs over the live notes.
#[derive(Debug)]
pub struct LinkResolver {
    index: PathIndex,
    ids: HashMap<String, String>,
}

impl LinkResolver {
    /// Loads every live note path.
    pub fn load(conn: &Connection) -> CoreResult<Self> {
        let paths = notes::live_paths(conn)?;
        let index = PathIndex::new(paths.iter().map(|(_, p)| p.as_str()));
        let ids = paths.into_iter().map(|(id, p)| (p, id)).collect();
        Ok(Self { index, ids })
    }

    /// The note a link in `source` points at (unique resolution only).
    pub fn resolve(&self, link: &str, source: &str) -> Option<String> {
        match self.index.resolve(link, Some(source)) {
            Resolution::Resolved(p) => self.ids.get(&p).cloned(),
            _ => None,
        }
    }
}

/// Notes whose derived rows must be recomputed at the end of a write.
#[derive(Debug, Default)]
pub struct Reindex {
    touched: BTreeSet<String>,
    paths_changed: bool,
    topics: Topics,
}

impl Reindex {
    /// An empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// `id` changed; `path_changed` when it appeared, moved or disappeared.
    pub fn note(&mut self, id: &str, path_changed: bool) {
        self.touched.insert(id.to_owned());
        self.paths_changed |= path_changed;
    }

    /// Marks view topics changed by the write itself (outbox, suggestions, …).
    pub fn topics(&mut self, topics: Topics) {
        self.topics |= topics;
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.touched.is_empty() && !self.paths_changed && self.topics.is_empty()
    }

    /// Re-derives touched notes (and re-resolves all links when paths changed). Returns the
    /// view topics affected.
    pub fn apply(self, conn: &Connection) -> CoreResult<Topics> {
        let mut topics = self.topics;
        if self.touched.is_empty() && !self.paths_changed {
            return Ok(topics);
        }
        let resolver = LinkResolver::load(conn)?;
        for id in &self.touched {
            topics |= reindex_note(conn, id, &resolver)?;
        }
        if self.paths_changed {
            re_resolve_all(conn, &resolver)?;
            topics |= Topics::NOTES | Topics::ENTITIES;
        }
        Ok(topics)
    }
}

const DERIVED_BY_NOTE: &[&str] = &[
    "DELETE FROM tags WHERE note_id = ?1",
    "DELETE FROM links WHERE note_id = ?1",
    "DELETE FROM relations WHERE src_id = ?1",
    "DELETE FROM entities WHERE note_id = ?1",
    "DELETE FROM entity_aliases WHERE note_id = ?1",
    "DELETE FROM places WHERE note_id = ?1",
    "DELETE FROM documents WHERE note_id = ?1",
    "DELETE FROM custody_events WHERE document_id = ?1",
    "DELETE FROM task_reminders WHERE task_id IN (SELECT id FROM tasks WHERE note_id = ?1)",
    "DELETE FROM tasks WHERE note_id = ?1",
    "DELETE FROM inbox WHERE note_id = ?1",
    "DELETE FROM notes_fts WHERE note_id = ?1",
];

/// Removes every derived row of a note.
pub fn unindex(conn: &Connection, id: &str) -> CoreResult<()> {
    for sql in DERIVED_BY_NOTE {
        conn.execute(sql, [id])?;
    }
    Ok(())
}

/// Recomputes the derived rows of one note from its current content.
pub fn reindex_note(conn: &Connection, id: &str, resolver: &LinkResolver) -> CoreResult<Topics> {
    let had_tasks: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM tasks WHERE note_id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    unindex(conn, id)?;
    let mut topics = Topics::NOTES;
    if had_tasks {
        topics |= Topics::TASKS;
    }
    let Some(note) = notes::get(conn, id)?.filter(|n| !n.deleted) else {
        return Ok(topics | Topics::INBOX | Topics::ENTITIES);
    };
    let p = format::parse_note(&note.path, &note.content);
    topics |= index_parsed(conn, id, &note.path, &note.local_updated_at, &p, resolver)?;
    Ok(topics)
}

#[allow(clippy::too_many_lines)] // one insert block per derived table
fn index_parsed(
    conn: &Connection,
    id: &str,
    path: &str,
    local_updated_at: &str,
    p: &ParsedNote,
    r: &LinkResolver,
) -> CoreResult<Topics> {
    let mut topics = Topics::NOTES;
    for t in &p.tags {
        conn.execute(
            "INSERT OR IGNORE INTO tags (note_id, tag) VALUES (?1, ?2)",
            params![id, t],
        )?;
    }
    for (i, l) in p.links.iter().enumerate() {
        conn.execute(
            "INSERT INTO links (note_id, ord, dst_id, dst_raw, kind, anchor)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                i64::try_from(i).unwrap_or(i64::MAX),
                r.resolve(&l.path, path),
                l.path,
                if l.embed { "embed" } else { "link" },
                l.anchor
            ],
        )?;
    }
    for (rel, target) in &p.relations {
        conn.execute(
            "INSERT OR IGNORE INTO relations (src_id, rel_type, dst_raw, dst_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, rel, target, r.resolve(target, path)],
        )?;
    }
    if p.kind.is_entity() {
        topics |= Topics::ENTITIES;
        conn.execute(
            "INSERT INTO entities (note_id, kind, display_name, role, industry)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, p.kind.as_str(), p.title, p.role, p.industry],
        )?;
        let mut names = vec![p.title.clone()];
        names.extend(p.aliases.iter().cloned());
        for alias in names {
            conn.execute(
                "INSERT OR IGNORE INTO entity_aliases (note_id, alias, alias_normalized)
                 VALUES (?1, ?2, ?3)",
                params![id, alias, text_normalize::normalize_for_search(&alias)],
            )?;
        }
    }
    if p.kind == domain::NoteKind::Place {
        conn.execute(
            "INSERT INTO places (note_id, parent_id, parent_raw) VALUES (?1, ?2, ?3)",
            params![
                id,
                p.place_parent.as_deref().and_then(|x| r.resolve(x, path)),
                p.place_parent
            ],
        )?;
    }
    if let Some(d) = &p.document {
        let res = |x: &Option<String>| x.as_deref().and_then(|x| r.resolve(x, path));
        conn.execute(
            "INSERT INTO documents (note_id, doc_type, copy, copy_of_id, location_id, location_raw,
                                    holder_id, holder_raw, last_holder_id, last_holder_raw,
                                    status, expires)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                d.doc_type,
                d.copy,
                res(&d.copy_of),
                res(&d.location),
                d.location,
                res(&d.holder),
                d.holder,
                res(&d.last_holder),
                d.last_holder,
                d.status,
                d.expires
            ],
        )?;
        for (i, e) in p.custody.iter().enumerate() {
            let target = |l: &Option<String>| {
                l.as_deref()
                    .and_then(vault_format::WikiLink::parse_exact)
                    .map(|w| w.target().to_owned())
            };
            let (place, person, party) = (
                target(&e.place),
                target(&e.person),
                target(&e.counterparty),
            );
            conn.execute(
                "INSERT INTO custody_events (document_id, ord, type, at, place_id, place_raw,
                                             person_id, person_raw, counterparty_id,
                                             counterparty_raw, citations)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    i64::try_from(i).unwrap_or(i64::MAX),
                    e.kind.as_str(),
                    e.date.to_string(),
                    place.as_deref().and_then(|x| r.resolve(x, path)),
                    place,
                    person.as_deref().and_then(|x| r.resolve(x, path)),
                    person,
                    party.as_deref().and_then(|x| r.resolve(x, path)),
                    party,
                    to_msgpack(&e.citations)?
                ],
            )?;
        }
    }
    if !p.tasks.is_empty() {
        topics |= Topics::TASKS;
    }
    for t in &p.tasks {
        let task_id = t
            .block_id
            .clone()
            .unwrap_or_else(|| format!("line:{id}:{}", t.line_no));
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO tasks (id, note_id, line_no, line, description, status, priority,
                                          due, scheduled, start, done_at, cancelled_at,
                                          recurrence_raw, rrule, recurrence_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                task_id,
                id,
                i64::try_from(t.line_no).unwrap_or(i64::MAX),
                t.line,
                t.description,
                t.status.as_str(),
                t.priority.as_str(),
                t.due.map(|d| d.to_string()),
                t.scheduled.map(|d| d.to_string()),
                t.start.map(|d| d.to_string()),
                t.done.map(|d| d.to_string()),
                t.cancelled.map(|d| d.to_string()),
                t.recurrence_raw,
                t.rrule,
                t.recurrence_error
            ],
        )?;
        if inserted == 0 {
            continue;
        }
        for rem in &t.reminders {
            conn.execute(
                "INSERT OR IGNORE INTO task_reminders (task_id, remind_date, remind_time)
                 VALUES (?1, ?2, ?3)",
                params![
                    task_id,
                    rem.date.to_string(),
                    rem.time
                        .map(|t| t.format("%H:%M").to_string())
                        .unwrap_or_default()
                ],
            )?;
        }
    }
    if path.starts_with("inbox/") {
        topics |= Topics::INBOX;
        conn.execute(
            "INSERT INTO inbox (note_id, created) VALUES (?1, ?2)",
            params![id, p.created.as_deref().unwrap_or(local_updated_at)],
        )?;
    }
    let norm = text_normalize::normalize_for_search;
    conn.execute(
        "INSERT INTO notes_fts (note_id, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
        params![
            id,
            norm(&format!("{} {}", p.title, p.aliases.join(" "))),
            norm(&p.body),
            norm(&p.tags.join(" "))
        ],
    )?;
    Ok(topics)
}

/// Re-resolves every stored link-like column against the current paths.
pub fn re_resolve_all(conn: &Connection, r: &LinkResolver) -> CoreResult<()> {
    let sources: HashMap<String, String> = notes::live_paths(conn)?.into_iter().collect();
    let path_of = |id: &str| sources.get(id).map(String::as_str).unwrap_or_default();
    // (table, source column, raw column, id column, key columns)
    let specs: &[(&str, &str, &str, &str)] = &[
        ("links", "note_id", "dst_raw", "dst_id"),
        ("relations", "src_id", "dst_raw", "dst_id"),
        ("places", "note_id", "parent_raw", "parent_id"),
        ("documents", "note_id", "location_raw", "location_id"),
        ("documents", "note_id", "holder_raw", "holder_id"),
        ("documents", "note_id", "last_holder_raw", "last_holder_id"),
        ("custody_events", "document_id", "place_raw", "place_id"),
        ("custody_events", "document_id", "person_raw", "person_id"),
        (
            "custody_events",
            "document_id",
            "counterparty_raw",
            "counterparty_id",
        ),
    ];
    for (table, src, raw, dst) in specs {
        let rows: Vec<(i64, String, Option<String>, Option<String>)> = {
            let mut st =
                conn.prepare(&format!("SELECT rowid, {src}, {raw}, {dst} FROM {table}"))?;
            st.query_map([], |q| Ok((q.get(0)?, q.get(1)?, q.get(2)?, q.get(3)?)))?
                .collect::<Result<_, _>>()?
        };
        for (rowid, source, raw_value, old) in rows {
            let new = raw_value
                .as_deref()
                .and_then(|v| r.resolve(v, path_of(&source)));
            if new != old {
                conn.execute(
                    &format!("UPDATE {table} SET {dst} = ?1 WHERE rowid = ?2"),
                    params![new, rowid],
                )?;
            }
        }
    }
    Ok(())
}
