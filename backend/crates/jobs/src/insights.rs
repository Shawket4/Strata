//! The `entity_insights` job (PLAN §6.7, §9.2): regenerates the AI sections of a person,
//! company or place page (Summary, Insights, Open items, Timeline; documents: Summary) from
//! the notes that mention it, with citations. Debounced ≈5 min after a mentioning note
//! changes (enqueued by linking and filing), swept nightly (`entity_insights_sweep`), forced
//! by `POST /entities/{id}/refresh`; skipped when its input is unchanged.
//!
//! The backend validates the model's output before anything is written:
//!
//! - every bullet keeps only citations of blocks that are in the input; a bullet without one
//!   is **rejected** (uncited);
//! - an insight or open item whose citations are all one-line captures is **rejected**
//!   (speculation: a one-line capture may only add a Timeline entry);
//! - bullets and summary sentences with contact details (e-mail, phone numbers) are dropped:
//!   contact fields are the user's alone;
//! - timeline dates follow the §6.7 rules ([`crate::dates`]), newest first;
//! - `vault-format` renders and validates the sections (`validate_content`) and preserves
//!   every user section byte for byte; frontmatter (contact fields included) is not touched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;
use sha2::{Digest, Sha256};
use strata_ai::outputs::{Citation, CitedItem, EntityInsights};
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService};
use strata_common::{Clock, IdGenerator, JobId, NoteId};
use strata_index::repo::jobs::NewJob;
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai_apply::{
    AiApplied, AiBullet, AiChangeSet, AiSections, BlockIdRequest, Cite, INSIGHTS_HASH_KEY,
};
use vault_format::Document;
use vault_format::frontmatter::KnownKey;
use vault_format::sections::{AiProfile, AiSection};

use crate::dates;
use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::link::LinkParams;
use crate::pipeline::{self, BlockInput, Directory, SourceNote};

/// Job kind of the nightly sweep that queues `entity_insights` for every entity.
pub const SWEEP: &str = "entity_insights_sweep";

/// Mentioning notes given to the model (newest first).
pub const MAX_NOTES: usize = 25;

/// Output token limit.
const MAX_TOKENS: u32 = 3000;

/// The entity in an `entity_insights` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InsightEntity {
    /// ID.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// User-entered descriptive properties (never contact fields).
    pub properties: BTreeMap<String, String>,
    /// Disambiguation hints.
    pub hints: Vec<String>,
}

/// A mentioning note in an `entity_insights` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InsightNote {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// `created` (RFC 3339, user's time zone).
    pub created: String,
    /// A one-line capture (may only feed the Timeline).
    pub one_line: bool,
    /// Blocks.
    pub blocks: Vec<BlockInput>,
}

/// The `entity_insights` prompt input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InsightsInput {
    /// The entity.
    pub entity: InsightEntity,
    /// Mentioning notes, newest first.
    pub notes: Vec<InsightNote>,
}

/// Descriptive properties given to the model per kind (contact fields never are).
fn properties_of(kind: domain::NoteKind) -> &'static [KnownKey] {
    match kind {
        domain::NoteKind::Person => &[KnownKey::Role],
        domain::NoteKind::Company => &[KnownKey::Industry],
        domain::NoteKind::Document => &[KnownKey::DocType, KnownKey::Copy, KnownKey::Expires],
        _ => &[],
    }
}

/// Whether a note body is a one-line capture.
pub fn is_one_line(body: &str) -> bool {
    body.lines().filter(|l| !l.trim().is_empty()).count() <= 1
}

/// Whether `text` carries contact details (an e-mail address or a phone-like number).
pub fn has_contact(text: &str) -> bool {
    let email = text.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.');
        w.split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && b.contains('.'))
    });
    // Dates (`2026-10-01`) are not phone numbers.
    let mut blanked = text.to_owned();
    while let Some(d) = dates::explicit_date(&blanked) {
        let s = d.format("%Y-%m-%d").to_string();
        blanked = blanked.replacen(&s, " ", 1);
    }
    let mut run = 0usize;
    let mut longest = 0usize;
    for c in blanked.chars() {
        if c.is_ascii_digit() || ('٠'..='٩').contains(&c) {
            run += 1;
            longest = longest.max(run);
        } else if !matches!(c, ' ' | '-' | '+' | '(' | ')') {
            run = 0;
        }
    }
    email || longest >= 7
}

/// What validation removed (for logs and tests).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rejections {
    /// Bullets without a citation of an input block.
    pub uncited: usize,
    /// Insights or open items citing only one-line captures.
    pub speculative: usize,
    /// Bullets or sentences with contact details.
    pub contact: usize,
}

/// The validated sections of an [`EntityInsights`] output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validated {
    /// Summary (sentences with contact details removed).
    pub summary: Option<String>,
    /// Insights.
    pub insights: Vec<AiBullet>,
    /// Open items.
    pub open_items: Vec<AiBullet>,
    /// Timeline, newest first.
    pub timeline: Vec<AiBullet>,
    /// What was removed.
    pub rejected: Rejections,
    /// Generated block IDs that are cited (to append).
    pub cited_generated: BTreeSet<(NoteId, String)>,
}

/// Validates `out` against the input notes (see the module docs).
pub fn validate(out: &EntityInsights, notes: &[SourceNote]) -> Validated {
    let by_id: BTreeMap<String, &SourceNote> =
        notes.iter().map(|n| (n.id.to_string(), n)).collect();
    let mut v = Validated {
        summary: None,
        insights: Vec::new(),
        open_items: Vec::new(),
        timeline: Vec::new(),
        rejected: Rejections::default(),
        cited_generated: BTreeSet::new(),
    };
    let cites = |citations: &[Citation], v: &mut Validated| -> Vec<(Cite, bool, String, NaiveDate)> {
        let mut out = Vec::new();
        for c in citations {
            let Some(n) = by_id.get(&c.note_id) else { continue };
            let Some(text) = n.block_text(&c.block_id) else { continue };
            if n.generated.contains_key(&c.block_id) {
                v.cited_generated.insert((n.id, c.block_id.clone()));
            }
            let one_line = is_one_line(&n.body());
            let cite = Cite {
                note: n.id,
                block: Some(c.block_id.clone()),
            };
            if !out.iter().any(|(x, _, _, _): &(Cite, bool, String, NaiveDate)| *x == cite) {
                out.push((cite, one_line, text.to_owned(), n.created_date()));
            }
        }
        out
    };
    let bullets = |items: &[CitedItem], v: &mut Validated| -> Vec<AiBullet> {
        let mut out = Vec::new();
        for it in items {
            let cs = cites(&it.citations, v);
            if cs.is_empty() {
                v.rejected.uncited += 1;
                continue;
            }
            if cs.iter().all(|(_, one, _, _)| *one) {
                v.rejected.speculative += 1;
                continue;
            }
            if has_contact(&it.text) {
                v.rejected.contact += 1;
                continue;
            }
            out.push(AiBullet {
                date: None,
                text: it.text.trim().to_owned(),
                cites: cs.into_iter().map(|c| c.0).collect(),
            });
        }
        out
    };
    v.insights = bullets(&out.insights, &mut v);
    v.open_items = bullets(&out.open_items, &mut v);
    for t in &out.timeline {
        let cs = cites(&t.citations, &mut v);
        let Some((_, _, text, created)) = cs.first().cloned() else {
            v.rejected.uncited += 1;
            continue;
        };
        if has_contact(&t.text) {
            v.rejected.contact += 1;
            continue;
        }
        let date = dates::event_date(&text, created, Some(&t.date), None);
        v.timeline.push(AiBullet {
            date: Some(date),
            text: t.text.trim().to_owned(),
            cites: cs.into_iter().map(|c| c.0).collect(),
        });
    }
    // Newest first; the model's order breaks ties.
    v.timeline.sort_by(|a, b| b.date.cmp(&a.date));
    let sentences: Vec<&str> = out
        .summary
        .split_inclusive(['.', '!', '?', '؟'])
        .collect();
    let kept: Vec<&str> = sentences
        .iter()
        .copied()
        .filter(|s| !has_contact(s))
        .collect();
    v.rejected.contact += sentences.len() - kept.len();
    let summary = kept.concat().trim().to_owned();
    v.summary = (!summary.is_empty()).then_some(summary);
    v
}

/// Parameters of an `entity_insights` job.
pub type InsightsParams = LinkParams;

/// The `entity_insights` job.
#[derive(Debug, Clone)]
pub struct InsightsHandler {
    db: AppDb,
    vault: VaultService,
    ai: Arc<AiService>,
}

impl InsightsHandler {
    /// The handler.
    pub fn new(db: AppDb, vault: VaultService, ai: Arc<AiService>) -> Self {
        Self { db, vault, ai }
    }
}

/// Live notes of kind `note` that mention `entity` (frontmatter mention keys, entity
/// relations and body links), newest first, at most [`MAX_NOTES`].
pub async fn mentioning_notes(
    db: &AppDb,
    vault: &VaultService,
    scope: &UserScope,
    entity: NoteId,
) -> Result<Vec<SourceNote>, JobError> {
    let mut tx = db.begin(scope).await?;
    let ids: Vec<(NoteId,)> = sqlx::query_as(
        "SELECT n.id FROM notes n WHERE NOT n.trashed AND n.kind = 'note' AND n.id <> $1 AND ( \
           EXISTS (SELECT 1 FROM relations r WHERE r.src_id = n.id AND r.dst_id = $1) \
           OR EXISTS (SELECT 1 FROM links l WHERE l.src_id = n.id AND l.dst_id = $1)) \
         ORDER BY n.created DESC, n.id DESC LIMIT $2",
    )
    .bind(entity)
    .bind(i64::try_from(MAX_NOTES).unwrap_or(25))
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    let mut out = Vec::with_capacity(ids.len());
    for (id,) in ids {
        if let Some(n) = SourceNote::load(vault, scope, id).await? {
            out.push(n);
        }
    }
    Ok(out)
}

#[async_trait::async_trait]
impl JobHandler for InsightsHandler {
    fn kind(&self) -> &'static str {
        pipeline::ENTITY_INSIGHTS
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    #[allow(clippy::too_many_lines)] // one linear pass: input, skip check, call, validate, write
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let Some(id) = ctx.job.note_id else {
            return Err(JobError::Fatal("entity_insights job without an entity".into()));
        };
        let params = InsightsParams::decode(&ctx.job.payload);
        let Some(entity) = SourceNote::load(&self.vault, &ctx.scope, id).await? else {
            return Ok(());
        };
        let profile = match entity.kind {
            domain::NoteKind::Person | domain::NoteKind::Company | domain::NoteKind::Place => {
                AiProfile::Entity
            }
            domain::NoteKind::Document => AiProfile::Document,
            _ => return Ok(()),
        };
        let notes = mentioning_notes(&self.db, &self.vault, &ctx.scope, id).await?;
        if notes.is_empty() {
            return Ok(());
        }
        let dir = Directory::load(&self.db, &ctx.scope).await?;
        let doc = Document::parse(&entity.content);
        let fm = doc.frontmatter();
        let properties: BTreeMap<String, String> = properties_of(entity.kind)
            .iter()
            .filter_map(|k| {
                let v = fm?.text(*k)?.trim().to_owned();
                (!v.is_empty()).then(|| (k.as_str().to_owned(), v))
            })
            .collect();
        let info = dir.entity(id);
        let input = InsightsInput {
            entity: InsightEntity {
                id: id.to_string(),
                kind: entity.kind.as_str().to_owned(),
                name: info.map_or_else(|| entity.title.clone(), |e| e.name.clone()),
                aliases: info.map(|e| e.aliases.clone()).unwrap_or_default(),
                properties,
                hints: dir.hints.get(&id).cloned().unwrap_or_default(),
            },
            notes: notes
                .iter()
                .map(|n| InsightNote {
                    id: n.id.to_string(),
                    title: n.title.clone(),
                    created: n.created_rfc3339(),
                    one_line: is_one_line(&n.body()),
                    blocks: n.blocks.clone(),
                })
                .collect(),
        };
        let prompt = prompts::latest(ids::ENTITY_INSIGHTS)
            .ok_or_else(|| JobError::Fatal("entity_insights prompt missing".into()))?;
        let hash = format!(
            "{}.v{}:{}",
            prompt.id,
            prompt.version,
            hex::encode(Sha256::digest(
                strata_ai::prompts::render_input(&input)
                    .map_err(|e| JobError::Fatal(e.to_string()))?
                    .as_bytes()
            ))
        );
        let unchanged = entity
            .sidecar
            .extra
            .get(INSIGHTS_HASH_KEY)
            .and_then(|v| v.as_str())
            == Some(hash.as_str());
        if unchanged && !params.force {
            return Ok(());
        }
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        let out = match self
            .ai
            .complete::<EntityInsights>(caller, prompt, &input, MAX_TOKENS)
            .await
        {
            Ok(o) => o,
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let v = validate(&out.value, &notes);
        if v.rejected != Rejections::default() {
            tracing::info!(
                entity = %id,
                uncited = v.rejected.uncited,
                speculative = v.rejected.speculative,
                contact = v.rejected.contact,
                "entity insights: rejected bullets"
            );
        }
        let mut set = AiChangeSet::new(pipeline::ENTITY_INSIGHTS, id);
        set.job_id = Some(ctx.job.id);
        set.expect_version = Some(entity.version.clone());
        let bullets = match profile {
            AiProfile::Entity => vec![
                (AiSection::Insights, v.insights),
                (AiSection::OpenItems, v.open_items),
                (AiSection::Timeline, v.timeline),
            ],
            _ => Vec::new(),
        };
        set.sections.push(AiSections {
            note: id,
            profile,
            summary: v.summary,
            bullets,
        });
        for (note, block) in &v.cited_generated {
            if let Some(text) = notes
                .iter()
                .find(|n| n.id == *note)
                .and_then(|n| n.generated.get(block))
            {
                set.block_ids.push(BlockIdRequest {
                    note: *note,
                    block_text: text.clone(),
                    id: block.clone(),
                });
            }
        }
        set.sidecar_extra.push((
            id,
            INSIGHTS_HASH_KEY.to_owned(),
            serde_json::Value::String(hash),
        ));
        match self.vault.ai_apply(&ctx.scope, set).await? {
            AiApplied::Done { .. } | AiApplied::Stale => Ok(()),
        }
    }
}

/// The nightly sweep: queues `entity_insights` for every entity (each skips when unchanged).
#[derive(Debug, Clone)]
pub struct SweepHandler {
    db: AppDb,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
}

impl SweepHandler {
    /// The handler.
    pub fn new(db: AppDb, ids: Arc<dyn IdGenerator>, clock: Arc<dyn Clock>) -> Self {
        Self { db, ids, clock }
    }
}

#[async_trait::async_trait]
impl JobHandler for SweepHandler {
    fn kind(&self) -> &'static str {
        SWEEP
    }

    fn class(&self) -> JobClass {
        JobClass::Light
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let now = self.clock.now();
        let mut tx = self.db.begin(&ctx.scope).await?;
        let ids: Vec<(NoteId,)> = sqlx::query_as(
            "SELECT e.note_id FROM entities e JOIN notes n \
               ON n.user_id = e.user_id AND n.id = e.note_id \
             WHERE NOT n.trashed ORDER BY e.note_id",
        )
        .fetch_all(tx.conn())
        .await?;
        for (id,) in ids {
            crate::repo::enqueue(
                &mut tx,
                &NewJob {
                    id: JobId::generate(self.ids.as_ref()),
                    kind: pipeline::ENTITY_INSIGHTS.to_owned(),
                    note_id: Some(id),
                    payload: Vec::new(),
                    run_after: now,
                    max_attempts: 5,
                    dedupe_key: Some(id.to_string()),
                },
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

/// Enqueues a forced `entity_insights` job for `entity` now (`POST /entities/{id}/refresh`).
pub async fn enqueue_refresh(
    db: &AppDb,
    scope: &UserScope,
    ids: &dyn IdGenerator,
    entity: NoteId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<JobId, JobError> {
    let mut tx = db.begin(scope).await?;
    let job = crate::repo::enqueue(
        &mut tx,
        &NewJob {
            id: JobId::generate(ids),
            kind: pipeline::ENTITY_INSIGHTS.to_owned(),
            note_id: Some(entity),
            payload: pipeline::rmp(&InsightsParams { force: true })?,
            run_after: now,
            max_attempts: 5,
            dedupe_key: Some(entity.to_string()),
        },
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(job.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_details_are_detected() {
        assert!(has_contact("Call him on 0100 123 4567"));
        assert!(has_contact("mail shady@acme.com"));
        assert!(has_contact("رقمه ٠١٠٠١٢٣٤٥٦٧"));
        assert!(!has_contact("Invoice 2026-10 due on the 1st"));
        assert!(!has_contact("Signed 2026-09-20, filed 2026-09-21"));
        assert!(!has_contact("Met at Acme @ 9"));
    }

    #[test]
    fn one_line_notes() {
        assert!(is_one_line("Shady has the Watanya contract\n"));
        assert!(!is_one_line("Line one\n\nLine two\n"));
    }
}
