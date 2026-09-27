//! Derived note graph: tags, aliases, body links, typed relations, rejected edges, blocks,
//! and embedding chunks.

use chrono::{DateTime, Utc};
use pgvector::Vector;
use strata_common::{ChunkId, DecisionId, NoteId};

use crate::error::Result;
use crate::scope::ScopedTx;
use crate::types::{By, LinkKind};

/// A body wikilink or embed.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Link {
    /// Source note.
    pub src_id: NoteId,
    /// Position in the source (document order).
    pub ord: i32,
    /// Resolved target (None if unresolved).
    pub dst_id: Option<NoteId>,
    /// Raw target text.
    pub dst_raw: String,
    /// Link or embed.
    pub kind: LinkKind,
    /// `#Heading` anchor.
    pub anchor: Option<String>,
    /// `#^block` reference.
    pub block_id: Option<String>,
}

/// A typed relation (frontmatter key + sidecar provenance).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Relation {
    /// Source note.
    pub src_id: NoteId,
    /// Target note.
    pub dst_id: NoteId,
    /// Relation type (`related`, `part-of`, `works-at`, …).
    #[sqlx(rename = "type")]
    pub rel_type: String,
    /// Provenance.
    pub by: By,
    /// AI confidence.
    pub confidence: Option<f32>,
    /// AI one-line reason.
    pub reason: Option<String>,
    /// Created at.
    pub created: DateTime<Utc>,
    /// The AI decision that created it.
    pub decision_id: Option<DecisionId>,
}

/// A citable block.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Block {
    /// Note.
    pub note_id: NoteId,
    /// Block ID without the caret.
    pub block_id: String,
    /// Heading path, e.g. `Summary/Details`.
    pub heading_path: String,
    /// Block text.
    pub text: String,
    /// Byte offset of the start.
    pub start_offset: i32,
    /// Byte offset of the end.
    pub end_offset: i32,
}

/// A retrieval chunk with an optional embedding.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Chunk {
    /// ID.
    pub id: ChunkId,
    /// Note.
    pub note_id: NoteId,
    /// Block cited by the chunk.
    pub block_id: Option<String>,
    /// Text.
    pub text: String,
    /// Tokens.
    pub token_count: i32,
    /// 384-dim embedding.
    pub embedding: Option<Vector>,
    /// Embedding model ID (set iff `embedding` is).
    pub model: Option<String>,
}

/// A nearest-neighbour hit.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct ChunkHit {
    /// Chunk.
    pub chunk_id: ChunkId,
    /// Note.
    pub note_id: NoteId,
    /// Cosine distance (0 = identical direction).
    pub distance: f64,
}

// ---- tags & aliases -------------------------------------------------------------------------

/// Replaces a note's tags.
pub async fn replace_tags(tx: &mut ScopedTx, note: NoteId, tags: &[String]) -> Result<()> {
    sqlx::query("DELETE FROM tags WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    sqlx::query(
        "INSERT INTO tags (user_id, note_id, tag) SELECT strata_current_user(), $1, t \
         FROM unnest($2::text[]) t ON CONFLICT DO NOTHING",
    )
    .bind(note)
    .bind(tags)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A note's tags, sorted.
pub async fn tags_for(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT tag FROM tags WHERE note_id = $1 ORDER BY tag")
        .bind(note)
        .fetch_all(tx.conn())
        .await?)
}

/// Notes carrying `tag`, by ID.
pub async fn notes_with_tag(tx: &mut ScopedTx, tag: &str) -> Result<Vec<NoteId>> {
    Ok(sqlx::query_scalar("SELECT note_id FROM tags WHERE tag = $1 ORDER BY note_id")
        .bind(tag)
        .fetch_all(tx.conn())
        .await?)
}

/// Replaces a note's aliases.
pub async fn replace_aliases(tx: &mut ScopedTx, note: NoteId, aliases: &[String]) -> Result<()> {
    sqlx::query("DELETE FROM aliases WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    sqlx::query(
        "INSERT INTO aliases (user_id, note_id, alias) SELECT strata_current_user(), $1, a \
         FROM unnest($2::text[]) a ON CONFLICT DO NOTHING",
    )
    .bind(note)
    .bind(aliases)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// A note's aliases, sorted.
pub async fn aliases_for(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT alias FROM aliases WHERE note_id = $1 ORDER BY alias")
        .bind(note)
        .fetch_all(tx.conn())
        .await?)
}

// ---- links ----------------------------------------------------------------------------------

/// Replaces a note's outgoing body links (each `Link::src_id` must equal `src`).
pub async fn replace_links(tx: &mut ScopedTx, src: NoteId, links: &[Link]) -> Result<()> {
    sqlx::query("DELETE FROM links WHERE src_id = $1")
        .bind(src)
        .execute(tx.conn())
        .await?;
    for l in links {
        sqlx::query(
            "INSERT INTO links (user_id, src_id, ord, dst_id, dst_raw, kind, anchor, block_id) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(src)
        .bind(l.ord)
        .bind(l.dst_id)
        .bind(&l.dst_raw)
        .bind(l.kind)
        .bind(&l.anchor)
        .bind(&l.block_id)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}

const LINK_COLS: &str = "src_id, ord, dst_id, dst_raw, kind, anchor, block_id";

/// Outgoing links in document order.
pub async fn links_from(tx: &mut ScopedTx, src: NoteId) -> Result<Vec<Link>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LINK_COLS} FROM links WHERE src_id = $1 ORDER BY ord"
    )))
    .bind(src)
    .fetch_all(tx.conn())
    .await?)
}

/// Incoming body links (backlinks), by source then position.
pub async fn backlinks(tx: &mut ScopedTx, dst: NoteId) -> Result<Vec<Link>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LINK_COLS} FROM links WHERE dst_id = $1 ORDER BY src_id, ord"
    )))
    .bind(dst)
    .fetch_all(tx.conn())
    .await?)
}

// ---- relations & rejected -------------------------------------------------------------------

/// Inserts or updates a relation (keyed by src, dst, type).
pub async fn upsert_relation(tx: &mut ScopedTx, r: &Relation) -> Result<()> {
    sqlx::query(
        "INSERT INTO relations (user_id, src_id, dst_id, type, by, confidence, reason, created, decision_id) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (user_id, src_id, dst_id, type) DO UPDATE SET by = EXCLUDED.by, \
           confidence = EXCLUDED.confidence, reason = EXCLUDED.reason, decision_id = EXCLUDED.decision_id",
    )
    .bind(r.src_id)
    .bind(r.dst_id)
    .bind(&r.rel_type)
    .bind(r.by)
    .bind(r.confidence)
    .bind(&r.reason)
    .bind(r.created)
    .bind(r.decision_id)
    .execute(tx.conn())
    .await?;
    Ok(())
}

const REL_COLS: &str = "src_id, dst_id, type, by, confidence, reason, created, decision_id";

/// Removes a relation and returns it (None if absent).
pub async fn remove_relation(
    tx: &mut ScopedTx,
    src: NoteId,
    dst: NoteId,
    rel_type: &str,
) -> Result<Option<Relation>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "DELETE FROM relations WHERE src_id = $1 AND dst_id = $2 AND type = $3 RETURNING {REL_COLS}"
    )))
    .bind(src)
    .bind(dst)
    .bind(rel_type)
    .fetch_optional(tx.conn())
    .await?)
}

/// Outgoing relations, by type then target.
pub async fn relations_from(tx: &mut ScopedTx, src: NoteId) -> Result<Vec<Relation>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {REL_COLS} FROM relations WHERE src_id = $1 ORDER BY type, dst_id"
    )))
    .bind(src)
    .fetch_all(tx.conn())
    .await?)
}

/// Incoming relations (derived reverse edges), by type then source.
pub async fn relations_to(tx: &mut ScopedTx, dst: NoteId) -> Result<Vec<Relation>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {REL_COLS} FROM relations WHERE dst_id = $1 ORDER BY type, src_id"
    )))
    .bind(dst)
    .fetch_all(tx.conn())
    .await?)
}

/// Records a rejected edge (idempotent).
pub async fn add_rejected(
    tx: &mut ScopedTx,
    src: NoteId,
    dst: NoteId,
    rel_type: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO rejected (user_id, src_id, dst_id, type, at) \
         VALUES (strata_current_user(), $1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(src)
    .bind(dst)
    .bind(rel_type)
    .bind(at)
    .execute(tx.conn())
    .await?;
    Ok(())
}

/// True if the AI must not propose this edge: the exact triple was rejected, or — for
/// `related` — any type between the pair was rejected (§6.5).
pub async fn is_rejected(tx: &mut ScopedTx, src: NoteId, dst: NoteId, rel_type: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM rejected WHERE src_id = $1 AND dst_id = $2 \
           AND (type = $3 OR $3 = 'related'))",
    )
    .bind(src)
    .bind(dst)
    .bind(rel_type)
    .fetch_one(tx.conn())
    .await?)
}

// ---- blocks & chunks ------------------------------------------------------------------------

/// Replaces a note's blocks.
pub async fn replace_blocks(tx: &mut ScopedTx, note: NoteId, blocks: &[Block]) -> Result<()> {
    sqlx::query("DELETE FROM blocks WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    for b in blocks {
        sqlx::query(
            "INSERT INTO blocks (user_id, note_id, block_id, heading_path, text, start_offset, end_offset) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6)",
        )
        .bind(note)
        .bind(&b.block_id)
        .bind(&b.heading_path)
        .bind(&b.text)
        .bind(b.start_offset)
        .bind(b.end_offset)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}

/// A note's blocks in document order.
pub async fn blocks_for(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<Block>> {
    Ok(sqlx::query_as(
        "SELECT note_id, block_id, heading_path, text, start_offset, end_offset \
         FROM blocks WHERE note_id = $1 ORDER BY start_offset, block_id",
    )
    .bind(note)
    .fetch_all(tx.conn())
    .await?)
}

/// Replaces a note's chunks.
pub async fn replace_chunks(tx: &mut ScopedTx, note: NoteId, chunks: &[Chunk]) -> Result<()> {
    sqlx::query("DELETE FROM chunks WHERE note_id = $1")
        .bind(note)
        .execute(tx.conn())
        .await?;
    for c in chunks {
        sqlx::query(
            "INSERT INTO chunks (user_id, id, note_id, block_id, text, token_count, embedding, model) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(c.id)
        .bind(note)
        .bind(&c.block_id)
        .bind(&c.text)
        .bind(c.token_count)
        .bind(&c.embedding)
        .bind(&c.model)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}

/// A note's chunks, by ID.
pub async fn chunks_for(tx: &mut ScopedTx, note: NoteId) -> Result<Vec<Chunk>> {
    Ok(sqlx::query_as(
        "SELECT id, note_id, block_id, text, token_count, embedding, model \
         FROM chunks WHERE note_id = $1 ORDER BY id",
    )
    .bind(note)
    .fetch_all(tx.conn())
    .await?)
}

/// Nearest chunks by cosine distance among vectors from `model`, closest first.
pub async fn nearest_chunks(
    tx: &mut ScopedTx,
    embedding: &Vector,
    model: &str,
    limit: i64,
) -> Result<Vec<ChunkHit>> {
    Ok(sqlx::query_as(
        "SELECT id AS chunk_id, note_id, (embedding <=> $1)::float8 AS distance FROM chunks \
         WHERE embedding IS NOT NULL AND model = $2 ORDER BY embedding <=> $1, id LIMIT $3",
    )
    .bind(embedding)
    .bind(model)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?)
}
