//! `notes` and keyword search.

use chrono::{DateTime, Utc};
use strata_common::NoteId;

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{Lang, NoteKind};

/// A `notes` row (without the `search` vector).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Note {
    /// ID (frontmatter `id`).
    pub id: NoteId,
    /// Vault-relative path, e.g. `notes/Pricing.md`.
    pub path: String,
    /// Title (file name without extension unless frontmatter overrides for display).
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Dominant language.
    pub lang: Option<Lang>,
    /// Frontmatter `created`.
    pub created: DateTime<Utc>,
    /// Frontmatter `updated`.
    pub updated: DateTime<Utc>,
    /// Version (content hash).
    pub content_hash: String,
    /// Word count.
    pub word_count: i32,
    /// In `.trash/`.
    pub trashed: bool,
}

/// Normalised text for the search vector (normalise with `/crates/text-normalize` first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchText<'a> {
    /// Title (weight A).
    pub title: &'a str,
    /// Space-joined tags (weight B).
    pub tags: &'a str,
    /// Body (weight C).
    pub body: &'a str,
}

/// A keyword search hit.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SearchHit {
    /// Note.
    pub note_id: NoteId,
    /// `ts_rank` score.
    pub rank: f32,
}

const COLS: &str = "id, path, title, kind, lang, created, updated, content_hash, word_count, trashed";

/// Inserts or replaces a note by ID (path uniqueness is per user).
pub async fn upsert_note(tx: &mut ScopedTx, note: &Note) -> Result<()> {
    sqlx::query(
        "INSERT INTO notes (user_id, id, path, title, kind, lang, created, updated, content_hash, word_count, trashed) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (user_id, id) DO UPDATE SET path = EXCLUDED.path, title = EXCLUDED.title, \
           kind = EXCLUDED.kind, lang = EXCLUDED.lang, created = EXCLUDED.created, \
           updated = EXCLUDED.updated, content_hash = EXCLUDED.content_hash, \
           word_count = EXCLUDED.word_count, trashed = EXCLUDED.trashed",
    )
    .bind(note.id)
    .bind(&note.path)
    .bind(&note.title)
    .bind(note.kind)
    .bind(note.lang)
    .bind(note.created)
    .bind(note.updated)
    .bind(&note.content_hash)
    .bind(note.word_count)
    .bind(note.trashed)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A note by ID.
pub async fn get_note(tx: &mut ScopedTx, id: NoteId) -> Result<Option<Note>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM notes WHERE id = $1")))
        .bind(id)
        .fetch_optional(tx.conn())
        .await?)
}

/// A note by vault path.
pub async fn get_note_by_path(tx: &mut ScopedTx, path: &str) -> Result<Option<Note>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLS} FROM notes WHERE path = $1")))
        .bind(path)
        .fetch_optional(tx.conn())
        .await?)
}

/// All notes ordered by path; trashed notes only if `include_trashed`.
pub async fn list_notes(tx: &mut ScopedTx, include_trashed: bool) -> Result<Vec<Note>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLS} FROM notes WHERE ($1 OR NOT trashed) ORDER BY path"
    )))
    .bind(include_trashed)
    .fetch_all(tx.conn())
    .await?)
}

/// Hard-deletes a note's index rows (derived rows cascade). False if absent.
pub async fn delete_note(tx: &mut ScopedTx, id: NoteId) -> Result<bool> {
    let done = sqlx::query("DELETE FROM notes WHERE id = $1")
        .bind(id)
        .execute(tx.conn())
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Rebuilds a note's search vector from normalised text. False if the note is absent.
pub async fn set_note_search(tx: &mut ScopedTx, id: NoteId, text: SearchText<'_>) -> Result<bool> {
    let done = sqlx::query(
        "UPDATE notes SET search = setweight(to_tsvector('simple', $2), 'A') \
           || setweight(to_tsvector('simple', $3), 'B') || setweight(to_tsvector('simple', $4), 'C') \
         WHERE id = $1",
    )
    .bind(id)
    .bind(text.title)
    .bind(text.tags)
    .bind(text.body)
    .execute(tx.conn())
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Keyword search over non-trashed notes (`query` already normalised; web-search syntax),
/// best rank first, ties by ID.
pub async fn search_notes(tx: &mut ScopedTx, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    Ok(sqlx::query_as(
        "SELECT id AS note_id, ts_rank(search, q) AS rank \
         FROM notes, websearch_to_tsquery('simple', $1) q \
         WHERE NOT trashed AND search @@ q ORDER BY rank DESC, id LIMIT $2",
    )
    .bind(query)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}
