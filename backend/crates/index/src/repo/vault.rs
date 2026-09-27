//! Queries of the vault store (`strata-vault`): per-note clean-up of derived rows, full
//! rebuild support, the derived-row snapshot used to prove a rebuild equals the incremental
//! state, directory queries (entity search, documents, places, task views), and integrity
//! warnings (PLAN §7.2–§7.5).

use chrono::{DateTime, NaiveDate, Utc};
use strata_common::NoteId;
use uuid::Uuid;

use crate::error::Result;
use crate::repo::entities::{
    CustodyEvent, Document, Entity, add_custody_event, set_trigram_threshold,
};
use crate::repo::tasks::Task;
use crate::scope::ScopedTx;
use crate::types::{DocStatus, EntityKind};

// ---- per-note clean-up ----------------------------------------------------------------------

/// Deletes a note's outgoing relations.
pub async fn delete_relations_from(tx: &mut ScopedTx, src: NoteId) -> Result<()> {
    sqlx::query("DELETE FROM relations WHERE src_id = $1")
        .bind(src)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Deletes the rejected edges recorded for a source note.
pub async fn delete_rejected_from(tx: &mut ScopedTx, src: NoteId) -> Result<()> {
    sqlx::query("DELETE FROM rejected WHERE src_id = $1")
        .bind(src)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Deletes the mentions a note makes.
pub async fn delete_mentions_in(tx: &mut ScopedTx, note: NoteId) -> Result<()> {
    sqlx::query("DELETE FROM mentions WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Inserts a mention with exact timestamps (replacing an existing one).
pub async fn put_mention(
    tx: &mut ScopedTx,
    entity: NoteId,
    note: NoteId,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO mentions (user_id, entity_id, note_id, block_id, first_seen, last_seen) \
         VALUES (strata_current_user(), $1, $2, '', $3, $4) \
         ON CONFLICT (user_id, entity_id, note_id, block_id) \
         DO UPDATE SET first_seen = EXCLUDED.first_seen, last_seen = EXCLUDED.last_seen",
    )
    .bind(entity)
    .bind(note)
    .bind(first_seen)
    .bind(last_seen)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Deletes an entity row (its aliases, place, document and mentions of it cascade).
pub async fn delete_entity(tx: &mut ScopedTx, note: NoteId) -> Result<bool> {
    Ok(sqlx::query("DELETE FROM entities WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?
        .rows_affected()
        == 1)
}

/// Deletes a place row (children's `parent_id` becomes NULL).
pub async fn delete_place(tx: &mut ScopedTx, note: NoteId) -> Result<()> {
    sqlx::query("DELETE FROM places WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Deletes a document row (its custody events cascade).
pub async fn delete_document(tx: &mut ScopedTx, note: NoteId) -> Result<()> {
    sqlx::query("DELETE FROM documents WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Replaces a document's custody events.
pub async fn replace_custody(
    tx: &mut ScopedTx,
    document: NoteId,
    events: &[CustodyEvent],
) -> Result<()> {
    sqlx::query("DELETE FROM custody_events WHERE document_id = $1")
        .bind(document)
        .execute(tx.conn())
        .await?;
    for e in events {
        add_custody_event(tx, e).await?;
    }
    Ok(())
}

/// Deletes the duplicate keys of a note and of the tasks it contains (call before its task
/// rows are replaced).
pub async fn delete_note_dedupe_keys(tx: &mut ScopedTx, note: NoteId) -> Result<()> {
    sqlx::query(
        "DELETE FROM dedupe_keys WHERE (kind <> 'task' AND item_id = $1) \
           OR (kind = 'task' AND item_id IN (SELECT id FROM tasks WHERE note_id = $2))",
    )
    .bind(note.to_string())
    .bind(note)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Forgets a keep-both pair (order-insensitive).
pub async fn delete_keep_both(tx: &mut ScopedTx, kind: &str, a: &str, b: &str) -> Result<()> {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    sqlx::query("DELETE FROM dedupe_keep_both WHERE kind = $1 AND a_id = $2 AND b_id = $3")
        .bind(kind)
        .bind(lo)
        .bind(hi)
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Every derived table of the scoped user, in the order a rebuild clears them.
pub const DERIVED_TABLES: &[&str] = &[
    "task_reminders",
    "tasks",
    "custody_events",
    "documents",
    "places",
    "mentions",
    "entity_aliases",
    "entities",
    "chunks",
    "blocks",
    "relations",
    "links",
    "tags",
    "aliases",
    "clusters",
    "notes",
    "rejected",
    "dedupe_keys",
    "dedupe_keep_both",
];

/// Deletes every derived row of the scoped user (PLAN §5: the index is a rebuildable cache).
/// App state (jobs, suggestions, settings, change log, integrity warnings) is kept.
pub async fn clear_derived(tx: &mut ScopedTx) -> Result<()> {
    for t in DERIVED_TABLES {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {t}")))
            .execute(tx.conn())
            .await?;
    }
    Ok(())
}

/// Every derived row of the scoped user as text, per table, sorted: two states are equal iff
/// their snapshots are.
pub async fn derived_snapshot(tx: &mut ScopedTx) -> Result<Vec<(String, Vec<String>)>> {
    let mut out = Vec::new();
    for t in DERIVED_TABLES {
        let mut rows: Vec<String> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT x::text FROM {t} x")))
                .fetch_all(tx.conn())
                .await?;
        rows.sort();
        out.push(((*t).to_owned(), rows));
    }
    Ok(out)
}

// ---- directory queries ----------------------------------------------------------------------

/// An entity with its match score.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct EntityHit {
    /// The entity's note.
    pub note_id: NoteId,
    /// Kind.
    pub kind: EntityKind,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: Option<String>,
    /// Industry.
    pub industry: Option<String>,
    /// Best alias similarity (1 for exact, 0 without a query).
    pub score: f32,
}

impl EntityHit {
    /// The entity row.
    pub fn entity(&self) -> Entity {
        Entity {
            note_id: self.note_id,
            kind: self.kind,
            display_name: self.display_name.clone(),
            role: self.role.clone(),
            industry: self.industry.clone(),
        }
    }
}

/// Entities matching the filters. `query` must be normalised (text-normalize); it matches an
/// alias (display name included) by substring or trigram similarity ≥ `threshold`. Best
/// score first, then name.
pub async fn search_entities(
    tx: &mut ScopedTx,
    kind: Option<EntityKind>,
    query: Option<&str>,
    tag: Option<&str>,
    threshold: f32,
    limit: i64,
) -> Result<Vec<EntityHit>> {
    set_trigram_threshold(tx, threshold).await?;
    Ok(sqlx::query_as(
        "SELECT e.note_id, e.kind, e.display_name, e.role, e.industry, \
           COALESCE((SELECT max(CASE WHEN a.alias_normalized = $3 THEN 1::real \
                                    ELSE similarity(a.alias_normalized, $3) END) \
                     FROM entity_aliases a WHERE a.note_id = e.note_id), 0)::real AS score \
         FROM entities e \
         WHERE ($1::text IS NULL OR e.kind = $1) \
           AND ($2::text IS NULL OR EXISTS (SELECT 1 FROM tags t WHERE t.note_id = e.note_id AND t.tag = $2)) \
           AND ($3::text IS NULL OR EXISTS (SELECT 1 FROM entity_aliases a WHERE a.note_id = e.note_id \
                 AND (a.alias_normalized % $3 OR strpos(a.alias_normalized, $3) > 0))) \
         ORDER BY score DESC, e.display_name, e.note_id LIMIT $4",
    )
    .bind(kind)
    .bind(tag)
    .bind(query)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

/// Filter for [`list_documents`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentFilter {
    /// Only documents in this place or any place nested inside it.
    pub place: Option<NoteId>,
    /// Only documents held by this entity now.
    pub holder: Option<NoteId>,
    /// Only this status.
    pub status: Option<DocStatus>,
    /// Only documents expiring strictly before this date.
    pub expiring_before: Option<NaiveDate>,
    /// Normalised name/alias query (substring or trigram).
    pub query: Option<String>,
}

/// Documents matching `filter`, by display name then ID (recursive CTE for nested places).
pub async fn list_documents(
    tx: &mut ScopedTx,
    filter: &DocumentFilter,
    threshold: f32,
) -> Result<Vec<Document>> {
    set_trigram_threshold(tx, threshold).await?;
    Ok(sqlx::query_as(
        "WITH RECURSIVE sub(id) AS ( \
           SELECT note_id FROM places WHERE note_id = $1 \
           UNION SELECT p.note_id FROM places p JOIN sub ON p.parent_id = sub.id) \
         SELECT d.note_id, d.doc_type, d.copy, d.copy_of, d.location_id, d.holder_id, \
           d.last_holder_id, d.status, d.expires \
         FROM documents d JOIN entities e ON e.note_id = d.note_id \
         WHERE ($1::uuid IS NULL OR d.location_id IN (SELECT id FROM sub)) \
           AND ($2::uuid IS NULL OR d.holder_id = $2) \
           AND ($3::text IS NULL OR d.status = $3) \
           AND ($4::date IS NULL OR d.expires < $4) \
           AND ($5::text IS NULL OR EXISTS (SELECT 1 FROM entity_aliases a WHERE a.note_id = d.note_id \
                 AND (a.alias_normalized % $5 OR strpos(a.alias_normalized, $5) > 0))) \
         ORDER BY e.display_name, d.note_id",
    )
    .bind(filter.place)
    .bind(filter.holder)
    .bind(filter.status)
    .bind(filter.expiring_before)
    .bind(filter.query.as_deref())
    .fetch_all(tx.conn())
    .await?)
}

/// Documents whose last holder is `entity`, by ID.
pub async fn documents_last_held_by(tx: &mut ScopedTx, entity: NoteId) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar(
        "SELECT note_id FROM documents WHERE last_holder_id = $1 ORDER BY note_id",
    )
    .bind(entity)
    .fetch_all(tx.conn())
    .await?)
}

/// Documents that list `company` (or person) under `companies:` / `people:`, by ID.
pub async fn documents_concerning(tx: &mut ScopedTx, entity: NoteId) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar(
        "SELECT d.note_id FROM documents d JOIN relations r ON r.src_id = d.note_id \
         WHERE r.dst_id = $1 AND r.type IN ('companies', 'people') ORDER BY d.note_id",
    )
    .bind(entity)
    .fetch_all(tx.conn())
    .await?)
}

/// Copies of a document (`copy-of` pointing at it), by ID.
pub async fn copies_of(tx: &mut ScopedTx, document: NoteId) -> Result<Vec<NoteId>> {
    Ok(
        sqlx::query_scalar("SELECT note_id FROM documents WHERE copy_of = $1 ORDER BY note_id")
            .bind(document)
            .fetch_all(tx.conn())
            .await?,
    )
}

/// Direct children of a place, by ID.
pub async fn place_children(tx: &mut ScopedTx, place: NoteId) -> Result<Vec<NoteId>> {
    Ok(
        sqlx::query_scalar("SELECT note_id FROM places WHERE parent_id = $1 ORDER BY note_id")
            .bind(place)
            .fetch_all(tx.conn())
            .await?,
    )
}

/// A place's ancestors, nearest first (cycle-safe, at most 64 levels).
pub async fn place_ancestors(tx: &mut ScopedTx, place: NoteId) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE up(id, depth, seen) AS ( \
           SELECT parent_id, 1, ARRAY[note_id] FROM places WHERE note_id = $1 AND parent_id IS NOT NULL \
           UNION ALL SELECT p.parent_id, up.depth + 1, up.seen || p.note_id FROM places p \
             JOIN up ON p.note_id = up.id \
             WHERE p.parent_id IS NOT NULL AND NOT (p.parent_id = ANY(up.seen)) AND up.depth < 64) \
         SELECT id FROM up ORDER BY depth",
    )
    .bind(place)
    .fetch_all(tx.conn())
    .await?)
}

/// Task list views (§7.5 Tasks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskView {
    /// Open, due or scheduled on or before `today` and not overdue: due/scheduled today.
    Today,
    /// Open, due after `today`.
    Upcoming,
    /// Open, due before `today`.
    Overdue,
    /// Open with a recurrence.
    Recurring,
    /// Done or cancelled, most recently finished first.
    Done,
    /// Every task.
    All,
}

const TASK_COLS: &str = "id, note_id, text, status, due, scheduled, start, recurrence_raw, rrule, \
    recurrence_understood, priority, done_at, line_start, line_end";

/// Tasks of a view (optionally in one note), in the view's order.
pub async fn list_task_view(
    tx: &mut ScopedTx,
    view: TaskView,
    today: NaiveDate,
    note: Option<NoteId>,
) -> Result<Vec<Task>> {
    let (cond, order) = match view {
        TaskView::Today => (
            "status = 'open' AND (due = $1 OR (scheduled = $1 AND (due IS NULL OR due >= $1)))",
            "due NULLS LAST, note_id, line_start",
        ),
        TaskView::Upcoming => ("status = 'open' AND due > $1", "due, note_id, line_start"),
        TaskView::Overdue => ("status = 'open' AND due < $1", "due, note_id, line_start"),
        TaskView::Recurring => (
            "status = 'open' AND recurrence_raw IS NOT NULL",
            "due NULLS LAST, note_id, line_start",
        ),
        TaskView::Done => (
            "status IN ('done', 'cancelled') AND $1::date IS NOT NULL",
            "done_at DESC NULLS LAST, note_id, line_start",
        ),
        TaskView::All => (
            "$1::date IS NOT NULL",
            "due NULLS LAST, note_id, line_start",
        ),
    };
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TASK_COLS} FROM tasks WHERE {cond} AND ($2::uuid IS NULL OR note_id = $2) ORDER BY {order}"
    )))
    .bind(today)
    .bind(note)
    .fetch_all(tx.conn())
    .await?)
}

/// Reminder instants of a task, ascending.
pub async fn reminders_of(tx: &mut ScopedTx, task: &str) -> Result<Vec<DateTime<Utc>>> {
    Ok(sqlx::query_scalar(
        "SELECT remind_at FROM task_reminders WHERE task_id = $1 ORDER BY remind_at",
    )
    .bind(task)
    .fetch_all(tx.conn())
    .await?)
}

// ---- integrity warnings ---------------------------------------------------------------------

/// An `integrity_warnings` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct IntegrityWarning {
    /// ID (a ULID stored as uuid).
    pub id: Uuid,
    /// Kind (`out_of_band_edit`, `uncommitted_changes`, `sidecar_repaired`, …).
    pub kind: String,
    /// Path concerned.
    pub path: Option<String>,
    /// Content-free description.
    pub detail: String,
    /// When it was raised.
    pub created: DateTime<Utc>,
}

/// Records a warning.
pub async fn add_warning(tx: &mut ScopedTx, w: &IntegrityWarning) -> Result<()> {
    sqlx::query(
        "INSERT INTO integrity_warnings (user_id, id, kind, path, detail, created) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5)",
    )
    .bind(w.id)
    .bind(&w.kind)
    .bind(&w.path)
    .bind(&w.detail)
    .bind(w.created)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Warnings, newest first.
pub async fn list_warnings(tx: &mut ScopedTx, limit: i64) -> Result<Vec<IntegrityWarning>> {
    Ok(sqlx::query_as(
        "SELECT id, kind, path, detail, created FROM integrity_warnings \
         ORDER BY created DESC, id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}
