//! The `embed` and `embed_backfill` jobs (PLAN §9.1b, §9.2).
//!
//! `embed` runs for a note after each write that changed its content (the vault store
//! enqueues it, debounced per note). It
//!
//! 1. skips the note when its vector is current: same embedding model and same note version
//!    (idempotent on an unchanged content hash);
//! 2. otherwise chunks the body ([`crate::chunk`]), embeds one chunk per call (so peak memory
//!    stays that of one small inference, and a `claude -p` call can slip in between), stores
//!    the chunks with the model ID and the note vector = L2-normalised mean of the chunk
//!    vectors, and enqueues `summarize`;
//! 3. embeds the note's duplicate-check items (its title/text and its task lines) whose text
//!    changed, for the semantic duplicate level (§9.7), and drops vectors of items that are
//!    gone.
//!
//! A trashed or deleted note loses its vectors. Without an embedding model the job does
//! nothing (the backfill picks the notes up once one is configured).
//!
//! `embed_backfill` makes every live note current with the configured model: the first
//! import of an existing vault, a reindex, or a model change (vectors carry their model ID,
//! so a change shows up as stale rows). It enqueues `embed` jobs in bounded batches and
//! re-enqueues itself until nothing is stale, so it is resumable across restarts; progress
//! is [`crate::status`]'s `embedding_progress`.

use std::sync::Arc;

use strata_ai::Embedder;
use strata_common::{Clock, IdGenerator, JobId};
use strata_index::AppDb;
use strata_index::repo::jobs::NewJob;
use strata_index::repo::notes;
use strata_vault::{VaultError, VaultService};
use vault_format::Document;

use crate::chunk;
use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::repo;
use crate::vectors::{self, normalized_mean};

/// Job kind `embed`.
pub const EMBED: &str = "embed";
/// Job kind `embed_backfill`.
pub const EMBED_BACKFILL: &str = "embed_backfill";
/// Job kind `summarize` (enqueued after a content change).
pub const SUMMARIZE: &str = "summarize";

/// The `embed` job.
#[derive(Debug, Clone)]
pub struct EmbedHandler {
    db: AppDb,
    vault: VaultService,
    embedder: Option<Arc<dyn Embedder>>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    summarize: bool,
}

impl EmbedHandler {
    /// The handler; `summarize` enqueues `summarize` after a re-embed.
    pub fn new(
        db: AppDb,
        vault: VaultService,
        embedder: Option<Arc<dyn Embedder>>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        summarize: bool,
    ) -> Self {
        Self {
            db,
            vault,
            embedder,
            clock,
            ids,
            summarize,
        }
    }

    async fn embed_one(&self, embedder: &dyn Embedder, text: String) -> Result<Vec<f32>, JobError> {
        let mut out = embedder
            .embed(&[text])
            .await
            .map_err(|e| JobError::Retry(format!("embedding: {e}")))?;
        let e = out
            .pop()
            .ok_or_else(|| JobError::Retry("embedding: no vector returned".into()))?;
        if e.vector.len() != embedder.dims() {
            return Err(JobError::Fatal(format!(
                "embedding has {} dimensions, expected {}",
                e.vector.len(),
                embedder.dims()
            )));
        }
        Ok(e.vector)
    }

    async fn item_vectors(
        &self,
        ctx: &JobContext,
        embedder: &dyn Embedder,
        note: strata_common::NoteId,
    ) -> Result<(), JobError> {
        let model = embedder.model_id().to_owned();
        let mut tx = self.db.begin(&ctx.scope).await?;
        let items = vectors::items_of_note(&mut tx, note).await?;
        let stored = vectors::item_hashes(&mut tx, note).await?;
        tx.commit().await?;
        let mut todo = Vec::new();
        for it in &items {
            let text = vectors::item_text(&it.item).to_owned();
            let hash = vectors::text_hash(&model, &text);
            let current = stored
                .iter()
                .any(|(k, i, h)| *k == it.kind && *i == it.item_id && *h == hash);
            if !current {
                todo.push((it.kind.clone(), it.item_id.clone(), text, hash));
            }
        }
        let mut computed = Vec::with_capacity(todo.len());
        for (kind, item_id, text, hash) in todo {
            let v = self.embed_one(embedder, text).await?;
            computed.push((kind, item_id, hash, v));
        }
        let now = self.clock.now();
        let mut tx = self.db.begin(&ctx.scope).await?;
        for (kind, item_id, hash, v) in &computed {
            vectors::upsert_item_vector(&mut tx, kind, item_id, note, &model, hash, v, now).await?;
        }
        let keep: Vec<(String, String)> = items
            .iter()
            .map(|i| (i.kind.clone(), i.item_id.clone()))
            .collect();
        vectors::prune_item_vectors(&mut tx, note, &keep).await?;
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl JobHandler for EmbedHandler {
    fn kind(&self) -> &'static str {
        EMBED
    }

    fn class(&self) -> JobClass {
        JobClass::Embed
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(note) = ctx.job.note_id else {
            return Err(JobError::Fatal("embed job without a note".into()));
        };
        let Some(embedder) = self.embedder.clone() else {
            return Ok(());
        };
        let model = embedder.model_id().to_owned();
        let mut tx = self.db.begin(&ctx.scope).await?;
        let row = notes::get_note(&mut tx, note).await?;
        let current = vectors::note_vector(&mut tx, note).await?;
        tx.commit().await?;
        let Some(row) = row.filter(|r| !r.trashed) else {
            let mut tx = self.db.begin(&ctx.scope).await?;
            vectors::delete_note_embeddings(&mut tx, note).await?;
            tx.commit().await?;
            return Ok(());
        };
        let up_to_date = current
            .as_ref()
            .is_some_and(|v| v.model == model && v.content_hash == row.content_hash);
        if !up_to_date {
            let view = match self.vault.note(&ctx.scope, note).await {
                Ok(v) => v,
                Err(VaultError::NotFound) => return Ok(()),
                Err(e) => return Err(e.into()),
            };
            if view.trashed {
                return Ok(());
            }
            let doc = Document::parse(&view.content);
            let chunks = chunk::chunk_body(doc.body());
            let mut embeddings = Vec::with_capacity(chunks.len());
            for c in &chunks {
                embeddings.push(
                    self.embed_one(embedder.as_ref(), c.embed_input(&view.title))
                        .await?,
                );
            }
            let note_vector = normalized_mean(&embeddings)
                .ok_or_else(|| JobError::Fatal("the chunk vectors average to zero".into()))?;
            let now = self.clock.now();
            let mut tx = self.db.begin(&ctx.scope).await?;
            vectors::store_note_embeddings(
                &mut tx,
                self.ids.as_ref(),
                note,
                &view.version,
                &model,
                &chunks,
                &embeddings,
                &note_vector,
                now,
            )
            .await?;
            if self.summarize {
                repo::enqueue_for_note(&mut tx, self.ids.as_ref(), SUMMARIZE, note, now, now)
                    .await?;
            }
            tx.commit().await?;
        }
        self.item_vectors(&ctx, embedder.as_ref(), note).await
    }
}

/// The `embed_backfill` job.
#[derive(Debug, Clone)]
pub struct BackfillHandler {
    db: AppDb,
    embedder: Option<Arc<dyn Embedder>>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    batch: i64,
}

impl BackfillHandler {
    /// Enqueues at most `batch` embed jobs at a time.
    pub fn new(
        db: AppDb,
        embedder: Option<Arc<dyn Embedder>>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        batch: i64,
    ) -> Self {
        Self {
            db,
            embedder,
            clock,
            ids,
            batch: batch.max(1),
        }
    }
}

#[async_trait::async_trait]
impl JobHandler for BackfillHandler {
    fn kind(&self) -> &'static str {
        EMBED_BACKFILL
    }

    fn class(&self) -> JobClass {
        JobClass::Light
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(embedder) = &self.embedder else {
            return Ok(());
        };
        let now = self.clock.now();
        let mut tx = self.db.begin(&ctx.scope).await?;
        let pending = repo::pending_of_kind(&mut tx, EMBED).await?;
        let room = self.batch - i64::try_from(pending).unwrap_or(i64::MAX);
        let stale = if room > 0 {
            vectors::stale_notes(&mut tx, embedder.model_id(), room).await?
        } else {
            Vec::new()
        };
        for note in &stale {
            // A backfill embed runs after interactive work queued now.
            repo::enqueue_for_note(&mut tx, self.ids.as_ref(), EMBED, *note, now, now).await?;
        }
        // More to do (or a full queue): look again once this batch had time to run.
        let more = room <= 0 || i64::try_from(stale.len()).unwrap_or(0) == room;
        if more {
            repo::enqueue(
                &mut tx,
                &NewJob {
                    id: JobId::generate(self.ids.as_ref()),
                    kind: EMBED_BACKFILL.to_owned(),
                    note_id: None,
                    payload: Vec::new(),
                    run_after: now + chrono::Duration::seconds(30),
                    max_attempts: 5,
                    dedupe_key: Some("backfill".to_owned()),
                },
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
