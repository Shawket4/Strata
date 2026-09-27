//! The `notes` table: the current local view of every note plus the server base it came from.
//!
//! A note's **current** state is its server base with every live outbox op re-applied
//! ([`crate::store::write::rebuild_note`]); `content`/`path` hold the current state and
//! `base_*` the server's. A note deleted locally keeps its row (`deleted = 1`) until the server
//! confirms, so a rejected delete can be rolled back.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;
use crate::format;
use crate::store::to_msgpack;

/// One row of `notes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRow {
    /// Note ID (ULID string).
    pub id: String,
    /// Current path.
    pub path: String,
    /// Title (file name).
    pub title: String,
    /// Kind.
    pub kind: String,
    /// Current markdown.
    pub content: String,
    /// Deleted locally, awaiting the server.
    pub deleted: bool,
    /// The server has this note.
    pub base_exists: bool,
    /// Server path.
    pub base_path: Option<String>,
    /// Server content.
    pub base_content: Option<String>,
    /// Server version.
    pub base_version: Option<String>,
    /// When the local row last changed (RFC 3339 UTC).
    pub local_updated_at: String,
}

/// A note's path and content (a state the write path folds ops over).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteState {
    /// Vault path.
    pub path: String,
    /// Markdown.
    pub content: String,
}

/// The server's state of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteBase {
    /// Vault path.
    pub path: String,
    /// Markdown.
    pub content: String,
    /// Version.
    pub version: String,
}

const COLUMNS: &str = "id, path, title, kind, content, deleted, base_exists, base_path, \
                       base_content, base_version, local_updated_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<NoteRow> {
    Ok(NoteRow {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        kind: r.get(3)?,
        content: r.get(4)?,
        deleted: r.get(5)?,
        base_exists: r.get(6)?,
        base_path: r.get(7)?,
        base_content: r.get(8)?,
        base_version: r.get(9)?,
        local_updated_at: r.get(10)?,
    })
}

/// A note row by ID (deleted rows included).
pub fn get(conn: &Connection, id: &str) -> CoreResult<Option<NoteRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM notes WHERE id = ?1"),
            [id],
            row,
        )
        .optional()?)
}

/// The live (not locally deleted) note at `path`, if any.
pub fn id_by_path(conn: &Connection, path: &str) -> CoreResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM notes WHERE path = ?1 AND deleted = 0 ORDER BY id LIMIT 1",
            [path],
            |r| r.get(0),
        )
        .optional()?)
}

/// `(id, path)` of every live note, ordered by path.
pub fn live_paths(conn: &Connection) -> CoreResult<Vec<(String, String)>> {
    let mut st = conn.prepare("SELECT id, path FROM notes WHERE deleted = 0 ORDER BY path, id")?;
    let rows = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The current state of a live note.
pub fn current(conn: &Connection, id: &str) -> CoreResult<Option<NoteState>> {
    Ok(get(conn, id)?.filter(|n| !n.deleted).map(|n| NoteState {
        path: n.path,
        content: n.content,
    }))
}

/// The server base of a note.
pub fn base(conn: &Connection, id: &str) -> CoreResult<Option<NoteBase>> {
    Ok(get(conn, id)?.and_then(|n| {
        if !n.base_exists {
            return None;
        }
        Some(NoteBase {
            path: n.base_path?,
            content: n.base_content?,
            version: n.base_version?,
        })
    }))
}

/// Writes the current state (`None` = deleted locally). Returns whether the path changed or
/// the note appeared/disappeared (links elsewhere must then be re-resolved).
pub fn write_current(
    conn: &Connection,
    id: &str,
    state: Option<&NoteState>,
    now: &str,
) -> CoreResult<bool> {
    let before = get(conn, id)?;
    let before_path = before
        .as_ref()
        .filter(|n| !n.deleted)
        .map(|n| n.path.clone());
    match state {
        Some(s) => {
            let parsed = format::parse_note(&s.path, &s.content);
            let props: Vec<(String, Vec<String>)> = parsed
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), format::property_display(v)))
                .collect();
            conn.execute(
                "INSERT INTO notes (id, path, title, kind, content, frontmatter, created, updated, \
                                    deleted, local_updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)
                 ON CONFLICT (id) DO UPDATE SET path = excluded.path, title = excluded.title,
                    kind = excluded.kind, content = excluded.content,
                    frontmatter = excluded.frontmatter, created = excluded.created,
                    updated = excluded.updated, deleted = 0,
                    local_updated_at = excluded.local_updated_at",
                params![
                    id,
                    s.path,
                    parsed.title,
                    parsed.kind.as_str(),
                    s.content,
                    to_msgpack(&props)?,
                    parsed.created,
                    parsed.updated,
                    now
                ],
            )?;
        }
        None => match &before {
            Some(n) if n.base_exists => {
                conn.execute(
                    "UPDATE notes SET deleted = 1, local_updated_at = ?2 WHERE id = ?1",
                    params![id, now],
                )?;
            }
            Some(_) => {
                conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
            }
            None => {}
        },
    }
    Ok(before_path != state.map(|s| s.path.clone()))
}

/// Sets (or clears) the server base of a note, creating a placeholder row when needed.
pub fn set_base(conn: &Connection, id: &str, base: Option<&NoteBase>, now: &str) -> CoreResult<()> {
    match base {
        Some(b) => {
            conn.execute(
                "INSERT INTO notes (id, path, title, kind, content, frontmatter, deleted,
                                    base_exists, base_path, base_content, base_version,
                                    local_updated_at)
                 VALUES (?1, ?2, '', 'note', '', x'90', 1, 1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (id) DO UPDATE SET base_exists = 1, base_path = excluded.base_path,
                    base_content = excluded.base_content, base_version = excluded.base_version",
                params![id, b.path, b.content, b.version, now],
            )?;
        }
        None => {
            conn.execute(
                "UPDATE notes SET base_exists = 0, base_path = NULL, base_content = NULL,
                                  base_version = NULL
                 WHERE id = ?1",
                [id],
            )?;
        }
    }
    Ok(())
}

/// Removes a row that has neither a current state nor a server base.
pub fn prune(conn: &Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM notes WHERE id = ?1 AND deleted = 1 AND base_exists = 0",
        [id],
    )?;
    Ok(())
}
