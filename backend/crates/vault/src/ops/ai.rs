//! Writes made by AI jobs (PLAN §9.2, §9.5, §9.7). Each is one `ai: <job> …` commit and never
//! touches the user's prose (principle 3): the summary goes into the sidecar, citations only
//! append block IDs (`^id`, the one automated body edit, §6.3).
//!
//! - [`VaultService::ai_set_summary`]: the `summary` of a note's sidecar, recorded with the
//!   note version it summarises (`content_hash`), so an unchanged note is never summarised
//!   twice.
//! - [`VaultService::ai_cite_blocks`]: makes retrieved blocks citable for an Ask answer by
//!   appending the IDs the answer used, in one commit across notes.
//! - `duplicates` suggestions from the nightly sweep ([`DuplicatesPayload`]): rejecting one
//!   records keep-both for the pair so it is never proposed again.

use std::collections::BTreeMap;

use strata_common::NoteId;
use strata_index::UserScope;
use vault_format::Document;
use vault_format::body::{self, BlockKind};
use vault_format::sidecar::NoteSidecar;

use crate::error::{Candidate, Result, VaultError};
use crate::ops::notes::DuplicatePayloadItem;
use crate::store::{Author, Core, VaultService};

/// What [`VaultService::ai_set_summary`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryWrite {
    /// The sidecar now holds the summary (one `ai:` commit).
    Written,
    /// The sidecar already held this summary for this version (no commit).
    Unchanged,
    /// The note changed since it was read (no commit; a new job follows the change).
    Stale,
}

/// A block an Ask answer cites, to be made citable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiteRequest {
    /// Note.
    pub note_id: NoteId,
    /// Note version when the block was retrieved.
    pub version: String,
    /// Body byte offset of the block in that version.
    pub block_start: usize,
    /// The block's text in that version (locates it again when the note changed since).
    pub block_text: String,
    /// The ID to append when the block has none (`[a-z0-9-]+`).
    pub block_id: String,
}

/// The outcome of one [`CiteRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiteOutcome {
    /// The requested ID was appended.
    Appended,
    /// The block already carries this ID (use it instead).
    Existing(String),
    /// The block is gone, is a heading, or the ID is taken: cite the note without a block.
    Missing,
}

/// Payload of a `duplicates` suggestion (`MessagePack`): two stored items the nightly sweep
/// found to be duplicates (PLAN §9.2 `dedupe`, §9.7). Never merged automatically; rejecting
/// the suggestion records keep-both for the pair.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DuplicatesPayload {
    /// The first item (its note is the suggestion's note).
    pub a: DuplicatePayloadItem,
    /// The second item.
    pub b: DuplicatePayloadItem,
    /// The LLM's one-sentence reason when a borderline score was confirmed.
    pub reason: Option<String>,
}

impl Core {
    /// See [`VaultService::ai_set_summary`].
    pub async fn ai_set_summary(
        &mut self,
        scope: UserScope,
        id: NoteId,
        version: &str,
        summary: &str,
        job: &str,
    ) -> Result<SummaryWrite> {
        let (path, current) = self.live(id)?;
        if current != version {
            return Ok(SummaryWrite::Stale);
        }
        let mut sc = self
            .sidecar(id)
            .await?
            .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
        if sc.summary.as_deref() == Some(summary) && sc.content_hash.as_deref() == Some(version) {
            return Ok(SummaryWrite::Unchanged);
        }
        sc.summary = Some(summary.to_owned());
        sc.content_hash = Some(version.to_owned());
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            vec![Core::sidecar_change(&sc)?],
            Author::Ai(job.to_owned()).message("summarize", &path),
        )
        .await?;
        Ok(SummaryWrite::Written)
    }

    /// See [`VaultService::ai_cite_blocks`].
    pub async fn ai_cite_blocks(
        &mut self,
        scope: UserScope,
        requests: &[CiteRequest],
        job: &str,
    ) -> Result<Vec<CiteOutcome>> {
        let mut outcomes = vec![CiteOutcome::Missing; requests.len()];
        let mut by_note: BTreeMap<NoteId, Vec<usize>> = BTreeMap::new();
        for (i, r) in requests.iter().enumerate() {
            by_note.entry(r.note_id).or_default().push(i);
        }
        let mut changes = Vec::new();
        let mut paths = Vec::new();
        for (note, idx) in by_note {
            let Ok((path, version)) = self.live(note) else {
                continue;
            };
            let Some(text) = self.read_text(&path).await? else {
                continue;
            };
            let mut doc = Document::parse(&text);
            let original = doc.body().to_owned();
            let analysis = body::analyze(&original);
            // (block index, request index, id to append)
            let mut appends: Vec<(usize, usize)> = Vec::new();
            let mut claimed: Vec<String> = Vec::new();
            for i in idx {
                let r = &requests[i];
                let found = analysis.blocks.iter().position(|b| {
                    let same_text = original.get(b.span.clone()) == Some(r.block_text.as_str());
                    same_text && (version != r.version || b.span.start == r.block_start)
                });
                let Some(bi) = found else { continue };
                let block = &analysis.blocks[bi];
                if let Some(existing) = &block.id {
                    outcomes[i] = CiteOutcome::Existing(existing.id.clone());
                } else if block.kind == BlockKind::Heading
                    || !vault_format::blocks::is_valid_block_id(&r.block_id)
                {
                    outcomes[i] = CiteOutcome::Missing;
                } else if let Some(&(_, other)) = appends.iter().find(|(b, _)| *b == bi) {
                    // Two requests for one block: both use the first ID.
                    outcomes[i] = CiteOutcome::Existing(requests[other].block_id.clone());
                } else if analysis.block_by_id(&r.block_id).is_some()
                    || claimed.contains(&r.block_id)
                {
                    outcomes[i] = CiteOutcome::Missing;
                } else {
                    claimed.push(r.block_id.clone());
                    appends.push((bi, i));
                }
            }
            if appends.is_empty() {
                continue;
            }
            // Append from the end so earlier spans stay valid.
            appends.sort_by(|x, y| analysis.blocks[y.0].span.start.cmp(&analysis.blocks[x.0].span.start));
            let mut body_text = original.clone();
            for &(bi, i) in &appends {
                let appended = vault_format::blocks::append_to(
                    &body_text,
                    &analysis.blocks[bi],
                    &requests[i].block_id,
                )
                .map_err(|_| VaultError::invalid("a block id cannot be appended there"))?;
                body_text = appended.body;
                outcomes[i] = CiteOutcome::Appended;
            }
            doc.set_body(body_text);
            changes.push((path.clone(), Some(doc.render().into_bytes())));
            paths.push(path);
        }
        if changes.is_empty() {
            return Ok(outcomes);
        }
        let subject = if paths.len() == 1 {
            paths[0].clone()
        } else {
            format!("{} notes", paths.len())
        };
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            changes,
            Author::Ai(job.to_owned()).message("cite", &subject),
        )
        .await?;
        Ok(outcomes)
    }

    /// Records keep-both for the pair of a rejected `duplicates` suggestion of `note`.
    pub(crate) async fn reject_duplicates(
        &mut self,
        scope: UserScope,
        note: NoteId,
        payload: &DuplicatesPayload,
    ) -> Result<()> {
        let Ok((path, _)) = self.live(note) else {
            return Ok(());
        };
        let (Some(a), Some(b)) = (payload.a.candidate(), payload.b.candidate()) else {
            return Ok(());
        };
        let mut tx = self.begin(&scope).await?;
        let item: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT item FROM dedupe_keys WHERE kind = $1 AND item_id = $2 AND item IS NOT NULL \
             ORDER BY key_no LIMIT 1",
        )
        .bind(&a.kind)
        .bind(&a.item)
        .fetch_optional(tx.conn())
        .await?;
        tx.commit().await?;
        let Some(item) = item.and_then(|b| rmp_serde::from_slice::<dedupe::Item>(&b).ok()) else {
            return Ok(());
        };
        let mut sc = self
            .sidecar(note)
            .await?
            .unwrap_or_else(|| NoteSidecar::new(note.as_ulid()));
        let others: Vec<Candidate> = vec![b];
        self.keep_both(&mut sc, &item, &others);
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            vec![Core::sidecar_change(&sc)?],
            Author::User.message("keep-both", &path),
        )
        .await?;
        Ok(())
    }
}

impl VaultService {
    /// The sidecar of note `id` as stored (`None` when absent or unreadable).
    pub async fn note_sidecar(&self, scope: &UserScope, id: NoteId) -> Result<Option<NoteSidecar>> {
        self.ready(scope).await?;
        let dir = self.vault_dir(scope.user_id());
        let rel = NoteSidecar::path_for(id.as_ulid());
        let bytes = crate::store::blocking(move || Ok(crate::fsio::read(&dir, &rel)?)).await?;
        Ok(bytes.and_then(|b| NoteSidecar::from_json(&String::from_utf8_lossy(&b)).ok()))
    }

    /// Stores `summary` in the sidecar of note `id` if the note is still at `version`
    /// (`ai: summarize <path>`, sidecar only).
    pub async fn ai_set_summary(
        &self,
        scope: &UserScope,
        id: NoteId,
        version: String,
        summary: String,
        job: String,
    ) -> Result<SummaryWrite> {
        self.exec(scope, move |core, s| {
            Box::pin(async move { core.ai_set_summary(s, id, &version, &summary, &job).await })
        })
        .await
    }

    /// Appends the requested block IDs where blocks have none, in one `ai: <job> <path>`
    /// commit (no commit when nothing needed an ID). Outcomes are in request order.
    pub async fn ai_cite_blocks(
        &self,
        scope: &UserScope,
        requests: Vec<CiteRequest>,
        job: String,
    ) -> Result<Vec<CiteOutcome>> {
        self.exec(scope, move |core, s| {
            Box::pin(async move { core.ai_cite_blocks(s, &requests, &job).await })
        })
        .await
    }
}
