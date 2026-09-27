//! People, companies, documents and places (§6.7, §6.12), their aliases and mentions, custody
//! events, and disambiguation hints (§9.8).

use chrono::{DateTime, NaiveDate, Utc};
use strata_common::{CustodyEventId, DecisionId, HintId, NoteId};

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{By, CustodyType, DocCopy, DocStatus, EntityKind};

/// An `entities` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Entity {
    /// The entity's note.
    pub note_id: NoteId,
    /// Kind.
    pub kind: EntityKind,
    /// Display name.
    pub display_name: String,
    /// Person role (user-editable).
    pub role: Option<String>,
    /// Company industry.
    pub industry: Option<String>,
}

/// An alias match.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AliasMatch {
    /// Entity.
    pub entity_id: NoteId,
    /// The alias that matched (as written).
    pub alias: String,
    /// Trigram similarity 0..1 (1 for exact).
    pub score: f32,
}

/// A `mentions` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Mention {
    /// Entity.
    pub entity_id: NoteId,
    /// Mentioning note.
    pub note_id: NoteId,
    /// Block ('' for a note-level mention).
    pub block_id: String,
    /// First seen.
    pub first_seen: DateTime<Utc>,
    /// Last seen.
    pub last_seen: DateTime<Utc>,
}

/// A `documents` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Document {
    /// The document's note.
    pub note_id: NoteId,
    /// `doc-type`.
    pub doc_type: String,
    /// `copy`.
    pub copy: DocCopy,
    /// `copy-of`.
    pub copy_of: Option<NoteId>,
    /// Current place.
    pub location_id: Option<NoteId>,
    /// Current holder.
    pub holder_id: Option<NoteId>,
    /// Last holder (derived from custody).
    pub last_holder_id: Option<NoteId>,
    /// Status.
    pub status: DocStatus,
    /// Expiry.
    pub expires: Option<NaiveDate>,
}

/// A `custody_events` row.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct CustodyEvent {
    /// ID.
    pub id: CustodyEventId,
    /// Document.
    pub document_id: NoteId,
    /// Event type.
    #[sqlx(rename = "type")]
    pub event_type: CustodyType,
    /// When it happened (resolved date, §6.7 rules).
    pub at: DateTime<Utc>,
    /// Place involved.
    pub place_id: Option<NoteId>,
    /// Person involved.
    pub person_id: Option<NoteId>,
    /// Third party involved.
    pub counterparty_id: Option<NoteId>,
    /// Provenance.
    pub by: By,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// Citing note.
    pub source_note_id: Option<NoteId>,
    /// Citing block.
    pub source_block_id: Option<String>,
    /// Recorded at.
    pub created: DateTime<Utc>,
}

/// A `disambiguation_hints` row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Hint {
    /// ID.
    pub id: HintId,
    /// Entity the hint is about.
    pub entity_id: NoteId,
    /// Hint text, e.g. "Ahmed at Acme = Ahmed Samir".
    pub hint: String,
    /// The correction decision that produced it.
    pub source_decision_id: Option<DecisionId>,
    /// Created at.
    pub created: DateTime<Utc>,
}

// ---- entities -------------------------------------------------------------------------------

/// Inserts or updates an entity (its note must exist).
pub async fn upsert_entity(tx: &mut ScopedTx, e: &Entity) -> Result<()> {
    sqlx::query(
        "INSERT INTO entities (user_id, note_id, kind, display_name, role, industry) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, note_id) DO UPDATE SET kind = EXCLUDED.kind, \
           display_name = EXCLUDED.display_name, role = EXCLUDED.role, industry = EXCLUDED.industry",
    )
    .bind(e.note_id)
    .bind(e.kind)
    .bind(&e.display_name)
    .bind(&e.role)
    .bind(&e.industry)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// An entity by note ID.
pub async fn get_entity(tx: &mut ScopedTx, id: NoteId) -> Result<Option<Entity>> {
    Ok(sqlx::query_as(
        "SELECT note_id, kind, display_name, role, industry FROM entities WHERE note_id = $1",
    )
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Entities of a kind, by display name then ID.
pub async fn list_entities(tx: &mut ScopedTx, kind: EntityKind) -> Result<Vec<Entity>> {
    Ok(sqlx::query_as(
        "SELECT note_id, kind, display_name, role, industry FROM entities \
         WHERE kind = $1 ORDER BY display_name, note_id",
    )
    .bind(kind)
    .fetch_all(tx.conn())
    .await?)
}

/// Replaces an entity's aliases with `(alias, alias_normalized)` pairs.
pub async fn replace_entity_aliases(
    tx: &mut ScopedTx,
    entity: NoteId,
    aliases: &[(String, String)],
) -> Result<()> {
    sqlx::query("DELETE FROM entity_aliases WHERE note_id = $1")
        .bind(entity)
        .execute(tx.conn())
        .await?;
    let (raw, normalized): (Vec<&str>, Vec<&str>) = aliases
        .iter()
        .map(|(a, n)| (a.as_str(), n.as_str()))
        .unzip();
    sqlx::query(
        "INSERT INTO entity_aliases (user_id, note_id, alias, alias_normalized) \
         SELECT strata_current_user(), $1, a, n FROM unnest($2::text[], $3::text[]) AS x(a, n) \
         ON CONFLICT DO NOTHING",
    )
    .bind(entity)
    .bind(&raw)
    .bind(&normalized)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Entities with an alias whose normalised form equals `normalized` (score 1).
pub async fn entities_by_alias(tx: &mut ScopedTx, normalized: &str) -> Result<Vec<AliasMatch>> {
    Ok(sqlx::query_as(
        "SELECT note_id AS entity_id, alias, 1::real AS score FROM entity_aliases \
         WHERE alias_normalized = $1 ORDER BY note_id, alias",
    )
    .bind(normalized)
    .fetch_all(tx.conn())
    .await?)
}

/// Fuzzy alias matches with trigram similarity ≥ `threshold`, best first (one row per
/// entity: its best alias).
pub async fn entities_by_alias_similarity(
    tx: &mut ScopedTx,
    normalized: &str,
    threshold: f32,
    limit: i64,
) -> Result<Vec<AliasMatch>> {
    set_trigram_threshold(tx, threshold).await?;
    Ok(sqlx::query_as(
        "SELECT entity_id, alias, score FROM ( \
           SELECT DISTINCT ON (note_id) note_id AS entity_id, alias, \
                  similarity(alias_normalized, $1) AS score \
           FROM entity_aliases WHERE alias_normalized % $1 \
           ORDER BY note_id, score DESC, alias) best \
         ORDER BY score DESC, entity_id LIMIT $2",
    )
    .bind(normalized)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}

/// Sets `pg_trgm.similarity_threshold` for the rest of the transaction (used by `%`).
pub(crate) async fn set_trigram_threshold(tx: &mut ScopedTx, threshold: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(crate::error::IndexError::InvalidArgument(format!(
            "trigram threshold {threshold} outside 0..=1"
        )));
    }
    sqlx::query("SELECT set_config('pg_trgm.similarity_threshold', $1, true)")
        .bind(threshold.to_string())
        .execute(tx.conn())
        .await?;
    Ok(())
}

/// Records a mention; re-recording keeps `first_seen` and advances `last_seen`.
pub async fn record_mention(
    tx: &mut ScopedTx,
    entity: NoteId,
    note: NoteId,
    block_id: &str,
    seen: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO mentions AS m (user_id, entity_id, note_id, block_id, first_seen, last_seen) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $4) \
         ON CONFLICT (user_id, entity_id, note_id, block_id) \
         DO UPDATE SET last_seen = GREATEST(m.last_seen, EXCLUDED.last_seen), \
                       first_seen = LEAST(m.first_seen, EXCLUDED.first_seen)",
    )
    .bind(entity)
    .bind(note)
    .bind(block_id)
    .bind(seen)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Mentions of an entity, most recently seen first.
pub async fn mentions_of(tx: &mut ScopedTx, entity: NoteId) -> Result<Vec<Mention>> {
    Ok(sqlx::query_as(
        "SELECT entity_id, note_id, block_id, first_seen, last_seen FROM mentions \
         WHERE entity_id = $1 ORDER BY last_seen DESC, note_id, block_id",
    )
    .bind(entity)
    .fetch_all(tx.conn())
    .await?)
}

// ---- places & documents ---------------------------------------------------------------------

/// Inserts or updates a place (its entity must exist).
pub async fn upsert_place(tx: &mut ScopedTx, place: NoteId, parent: Option<NoteId>) -> Result<()> {
    sqlx::query(
        "INSERT INTO places (user_id, note_id, parent_id) VALUES (strata_current_user(), $1, $2) \
         ON CONFLICT (user_id, note_id) DO UPDATE SET parent_id = EXCLUDED.parent_id",
    )
    .bind(place)
    .bind(parent)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A place's parent (outer None = unknown place).
pub async fn place_parent(tx: &mut ScopedTx, place: NoteId) -> Result<Option<Option<NoteId>>> {
    Ok(
        sqlx::query_scalar("SELECT parent_id FROM places WHERE note_id = $1")
            .bind(place)
            .fetch_optional(tx.conn())
            .await?,
    )
}

/// `root` and every place nested inside it (any depth), sorted by ID. Cycle-safe.
pub async fn place_subtree(tx: &mut ScopedTx, root: NoteId) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE sub(id) AS ( \
           SELECT note_id FROM places WHERE note_id = $1 \
           UNION SELECT p.note_id FROM places p JOIN sub ON p.parent_id = sub.id) \
         SELECT id FROM sub ORDER BY id",
    )
    .bind(root)
    .fetch_all(tx.conn())
    .await?)
}

const DOC_COLS: &str =
    "note_id, doc_type, copy, copy_of, location_id, holder_id, last_holder_id, status, expires";

/// Inserts or updates a document (its entity must exist).
pub async fn upsert_document(tx: &mut ScopedTx, d: &Document) -> Result<()> {
    sqlx::query(
        "INSERT INTO documents (user_id, note_id, doc_type, copy, copy_of, location_id, holder_id, \
           last_holder_id, status, expires) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (user_id, note_id) DO UPDATE SET doc_type = EXCLUDED.doc_type, \
           copy = EXCLUDED.copy, copy_of = EXCLUDED.copy_of, location_id = EXCLUDED.location_id, \
           holder_id = EXCLUDED.holder_id, last_holder_id = EXCLUDED.last_holder_id, \
           status = EXCLUDED.status, expires = EXCLUDED.expires",
    )
    .bind(d.note_id)
    .bind(&d.doc_type)
    .bind(d.copy)
    .bind(d.copy_of)
    .bind(d.location_id)
    .bind(d.holder_id)
    .bind(d.last_holder_id)
    .bind(d.status)
    .bind(d.expires)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A document by note ID.
pub async fn get_document(tx: &mut ScopedTx, id: NoteId) -> Result<Option<Document>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {DOC_COLS} FROM documents WHERE note_id = $1"
    )))
    .bind(id)
    .fetch_optional(tx.conn())
    .await?)
}

/// Documents located in `place` or any place nested inside it, by ID.
pub async fn documents_in_place(tx: &mut ScopedTx, place: NoteId) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE sub(id) AS ( \
           SELECT note_id FROM places WHERE note_id = $1 \
           UNION SELECT p.note_id FROM places p JOIN sub ON p.parent_id = sub.id) \
         SELECT d.note_id FROM documents d JOIN sub ON d.location_id = sub.id ORDER BY d.note_id",
    )
    .bind(place)
    .fetch_all(tx.conn())
    .await?)
}

/// Documents a person holds now, by ID.
pub async fn documents_held_by(tx: &mut ScopedTx, person: NoteId) -> Result<Vec<NoteId>> {
    Ok(
        sqlx::query_scalar("SELECT note_id FROM documents WHERE holder_id = $1 ORDER BY note_id")
            .bind(person)
            .fetch_all(tx.conn())
            .await?,
    )
}

/// Documents expiring strictly before `date`, soonest first.
pub async fn documents_expiring_before(
    tx: &mut ScopedTx,
    date: NaiveDate,
) -> Result<Vec<Document>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {DOC_COLS} FROM documents WHERE expires < $1 ORDER BY expires, note_id"
    )))
    .bind(date)
    .fetch_all(tx.conn())
    .await?)
}

/// Appends a custody event.
pub async fn add_custody_event(tx: &mut ScopedTx, e: &CustodyEvent) -> Result<()> {
    sqlx::query(
        "INSERT INTO custody_events (user_id, id, document_id, type, at, place_id, person_id, \
           counterparty_id, by, confidence, source_note_id, source_block_id, created) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(e.id)
    .bind(e.document_id)
    .bind(e.event_type)
    .bind(e.at)
    .bind(e.place_id)
    .bind(e.person_id)
    .bind(e.counterparty_id)
    .bind(e.by)
    .bind(e.confidence)
    .bind(e.source_note_id)
    .bind(&e.source_block_id)
    .bind(e.created)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A document's custody history, newest first.
pub async fn custody_history(tx: &mut ScopedTx, document: NoteId) -> Result<Vec<CustodyEvent>> {
    Ok(sqlx::query_as(
        "SELECT id, document_id, type, at, place_id, person_id, counterparty_id, by, confidence, \
           source_note_id, source_block_id, created \
         FROM custody_events WHERE document_id = $1 ORDER BY at DESC, id DESC",
    )
    .bind(document)
    .fetch_all(tx.conn())
    .await?)
}

// ---- disambiguation hints -------------------------------------------------------------------

/// Stores a disambiguation hint.
pub async fn add_hint(tx: &mut ScopedTx, h: &Hint) -> Result<()> {
    sqlx::query(
        "INSERT INTO disambiguation_hints (user_id, id, entity_id, hint, source_decision_id, created) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5)",
    )
    .bind(h.id)
    .bind(h.entity_id)
    .bind(&h.hint)
    .bind(h.source_decision_id)
    .bind(h.created)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// Hints for the given entities, oldest first.
pub async fn hints_for(tx: &mut ScopedTx, entities: &[NoteId]) -> Result<Vec<Hint>> {
    Ok(sqlx::query_as(
        "SELECT id, entity_id, hint, source_decision_id, created FROM disambiguation_hints \
         WHERE entity_id = ANY($1) ORDER BY created, id",
    )
    .bind(entities)
    .fetch_all(tx.conn())
    .await?)
}
