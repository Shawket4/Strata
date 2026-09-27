//! The nightly `dedupe` job (PLAN §9.2, §9.7): a semantic sweep across all kinds.
//!
//! 1. **Blocking by vector similarity.** For every stored duplicate-check item with a vector
//!    (the `embed` job keeps them), its nearest items of compatible kinds at or above the
//!    kind's semantic candidate threshold form a block.
//! 2. **Decisions.** Within each block, `dedupe::sweep` finds exact and near pairs, and
//!    `dedupe::check` with the cosines as [`dedupe::SemanticEvidence`] finds semantic ones:
//!    at or above the confirmed threshold directly, borderline ones after one
//!    `duplicate_confirm` LLM call each (at most [`DedupeHandler::max_confirmations`] per run;
//!    the verdict is remembered per pair and item text in `dedupe_verdicts`, so a pair is
//!    asked about once, not every night).
//!    Keep-both pairs are never reported (the shared `dedupe` crate suppresses them).
//! 3. **Suggestions.** Every new pair becomes one `duplicates` suggestion (never an automatic
//!    merge). Pairs already suggested (whatever their status) are skipped, so a run interrupted
//!    by a pause resumes where it left off and a rejected pair is never proposed again
//!    (rejecting also records keep-both).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;
use strata_ai::outputs::{DuplicateConfirmation, DuplicateVerdict};
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService, Embedder};
use strata_common::{IdGenerator, NoteId, SuggestionId};
use strata_index::AppDb;
use strata_vault::VaultService;
use strata_vault::ops::ai::DuplicatesPayload;
use strata_vault::ops::notes::DuplicatePayloadItem;

use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::vectors::{self, StoredItem};

/// Job kind `dedupe`.
pub const DEDUPE: &str = "dedupe";

/// Suggestion kind of a sweep result.
pub const DUPLICATES: &str = "duplicates";

/// Neighbours compared per item.
const NEIGHBOURS: i64 = 10;

#[derive(Debug, Serialize)]
struct ConfirmItem<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    text: &'a str,
    due: Option<String>,
    recurrence: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct ConfirmInput<'a> {
    kind: &'a str,
    new_item: ConfirmItem<'a>,
    existing: ConfirmItem<'a>,
    similarity: f32,
}

/// One pair found by a run.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundPair {
    /// First item (smaller ID).
    pub a: StoredItem,
    /// Its note.
    pub a_note: Option<NoteId>,
    /// Second item.
    pub b: StoredItem,
    /// Its note.
    pub b_note: Option<NoteId>,
    /// Level.
    pub level: dedupe::MatchLevel,
    /// Score (1 for exact, trigram/phonetic for near, cosine for semantic).
    pub score: f32,
    /// The LLM's reason for a confirmed borderline pair.
    pub reason: Option<String>,
}

/// The `dedupe` job.
#[derive(Debug, Clone)]
pub struct DedupeHandler {
    db: AppDb,
    vault: VaultService,
    ai: Arc<AiService>,
    embedder: Option<Arc<dyn Embedder>>,
    thresholds: dedupe::Thresholds,
    ids: Arc<dyn IdGenerator>,
    max_confirmations: usize,
}

impl DedupeHandler {
    /// The handler; at most `max_confirmations` LLM confirmations per run.
    pub fn new(
        db: AppDb,
        vault: VaultService,
        ai: Arc<AiService>,
        embedder: Option<Arc<dyn Embedder>>,
        thresholds: dedupe::Thresholds,
        ids: Arc<dyn IdGenerator>,
        max_confirmations: usize,
    ) -> Self {
        Self {
            db,
            vault,
            ai,
            embedder,
            thresholds,
            ids,
            max_confirmations,
        }
    }

    /// LLM confirmations allowed per run.
    pub fn max_confirmations(&self) -> usize {
        self.max_confirmations
    }

    async fn confirm(
        &self,
        ctx: &JobContext,
        new: &dedupe::Item,
        existing: &dedupe::Item,
        cosine: f32,
    ) -> Result<Option<(DuplicateVerdict, String)>, JobError> {
        let prompt = prompts::latest(ids::DUPLICATE_CONFIRM)
            .ok_or_else(|| JobError::Fatal("duplicate_confirm prompt missing".into()))?;
        let input = ConfirmInput {
            kind: new.kind.as_str(),
            new_item: ConfirmItem {
                id: None,
                text: vectors::item_text(new),
                due: None,
                recurrence: new.rrule.as_deref(),
            },
            existing: ConfirmItem {
                id: existing.id.as_deref(),
                text: vectors::item_text(existing),
                due: None,
                recurrence: existing.rrule.as_deref(),
            },
            similarity: (cosine * 1000.0).round() / 1000.0,
        };
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        match self
            .ai
            .complete::<DuplicateConfirmation>(caller, prompt, &input, 300)
            .await
        {
            Ok(out) => Ok(Some((out.value.verdict, out.value.reason))),
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Finds the pairs of the scoped user not yet suggested (see the module docs).
    #[allow(clippy::too_many_lines)] // one pass: block, decide, confirm, collect
    pub async fn find_pairs(&self, ctx: &JobContext) -> Result<Vec<FoundPair>, JobError> {
        let Some(embedder) = &self.embedder else {
            return Ok(Vec::new());
        };
        let model = embedder.model_id().to_owned();
        let mut tx = self.db.begin(&ctx.scope).await?;
        let items = vectors::all_items(&mut tx).await?;
        let keep_rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT kind, a_id, b_id FROM dedupe_keep_both ORDER BY kind, a_id, b_id")
                .fetch_all(tx.conn())
                .await?;
        let suggested = suggested_pairs(&mut tx).await?;
        tx.commit().await?;
        let mut keep = dedupe::KeepBothSet::new();
        for (kind, a, b) in keep_rows {
            if let Ok(k) = kind.parse::<dedupe::DedupeKind>() {
                keep.insert(dedupe::KeepBoth::new(k, &a, &b));
            }
        }
        let mut found: BTreeMap<(String, String), FoundPair> = BTreeMap::new();
        let mut confirmations = 0usize;
        for it in &items {
            let kinds: Vec<dedupe::DedupeKind> = dedupe::compatible_kinds(it.item.kind)
                .iter()
                .copied()
                .filter(|k| self.thresholds.semantic_between(it.item.kind, *k).is_some())
                .collect();
            let Some(floor) = kinds
                .iter()
                .filter_map(|k| self.thresholds.semantic_between(it.item.kind, *k))
                .map(|t| t.candidate)
                .reduce(f32::min)
            else {
                continue;
            };
            let mut tx = self.db.begin(&ctx.scope).await?;
            let Some((vector, own_note)) =
                vectors::item_vector(&mut tx, &it.kind, &it.item_id, &model).await?
            else {
                tx.commit().await?;
                continue;
            };
            let names: Vec<String> = kinds.iter().map(|k| k.as_str().to_owned()).collect();
            let neighbours: Vec<vectors::ItemNeighbour> = vectors::nearest_items(
                &mut tx,
                &vector,
                &model,
                &names,
                f64::from(floor),
                NEIGHBOURS + 1,
            )
            .await?
            .into_iter()
            .filter(|n| !(n.item.kind == it.kind && n.item.item_id == it.item_id))
            .collect();
            tx.commit().await?;
            if neighbours.is_empty() {
                continue;
            }
            let note_of = |id: &str| -> Option<NoteId> {
                neighbours
                    .iter()
                    .find(|n| n.item.item_id == id)
                    .and_then(|n| n.note_id)
            };
            let existing: Vec<dedupe::Existing> = neighbours
                .iter()
                .map(|n| dedupe::Existing {
                    item: n.item.item.clone(),
                    semantic: Some(dedupe::SemanticEvidence {
                        cosine: n.similarity,
                        llm_confirmed: None,
                    }),
                })
                .collect();
            let mut outcome = dedupe::check(&it.item, &existing, &self.thresholds, &keep);
            // The exact/near levels within the block (symmetric pairs, keep-both skipped).
            let mut block: Vec<dedupe::Item> = vec![it.item.clone()];
            block.extend(neighbours.iter().map(|n| n.item.item.clone()));
            let block_pairs = dedupe::sweep(&block, &self.thresholds, &keep);
            // Borderline semantic scores: confirm with the LLM (bounded per run).
            for id in std::mem::take(&mut outcome.needs_confirmation) {
                let Some(n) = neighbours.iter().find(|n| n.item.item.id.as_deref() == Some(id.as_str())) else {
                    continue;
                };
                let key = ordered(&it.item_id, &n.item.item_id);
                if suggested.contains(&key) || found.contains_key(&key) {
                    continue;
                }
                let hashes = pair_hashes(&model, (&it.item_id, &it.item), (&n.item.item_id, &n.item.item));
                let verdict = match self.stored_verdict(ctx, &key, &hashes).await? {
                    Some(v) => Some(v),
                    None if confirmations < self.max_confirmations => {
                        confirmations += 1;
                        let v = self.confirm(ctx, &it.item, &n.item.item, n.similarity).await?;
                        if let Some((verdict, reason)) = &v {
                            self.store_verdict(ctx, &key, &hashes, *verdict, reason).await?;
                        }
                        v
                    }
                    None => None,
                };
                if let Some((DuplicateVerdict::Duplicate, reason)) = verdict {
                    insert_pair(
                        &mut found,
                        &suggested,
                        (it, own_note),
                        (&n.item, n.note_id),
                        dedupe::MatchLevel::Semantic,
                        n.similarity,
                        Some(reason),
                    );
                }
            }
            for c in outcome.candidates {
                let Some(n) = neighbours.iter().find(|n| n.item.item.id.as_deref() == Some(c.id.as_str())) else {
                    continue;
                };
                insert_pair(
                    &mut found,
                    &suggested,
                    (it, own_note),
                    (&n.item, n.note_id),
                    c.level,
                    c.score,
                    None,
                );
            }
            for p in block_pairs {
                let find = |id: &str| -> Option<(&StoredItem, Option<NoteId>)> {
                    if it.item_id == id {
                        Some((it, own_note))
                    } else {
                        neighbours
                            .iter()
                            .find(|n| n.item.item_id == id)
                            .map(|n| (&n.item, note_of(id)))
                    }
                };
                if let (Some(a), Some(b)) = (find(&p.a_id), find(&p.b_id)) {
                    insert_pair(&mut found, &suggested, a, b, p.level, p.score, None);
                }
            }
        }
        Ok(found.into_values().collect())
    }
}

/// Text hashes of the pair's items, in the order of `ordered(a, b)`.
fn pair_hashes(
    model: &str,
    a: (&str, &dedupe::Item),
    b: (&str, &dedupe::Item),
) -> (String, String) {
    let (x, y) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    (
        vectors::text_hash(model, vectors::item_text(x.1)),
        vectors::text_hash(model, vectors::item_text(y.1)),
    )
}

impl DedupeHandler {
    /// The remembered verdict on the pair, if both items still have the judged text.
    async fn stored_verdict(
        &self,
        ctx: &JobContext,
        key: &(String, String),
        hashes: &(String, String),
    ) -> Result<Option<(DuplicateVerdict, String)>, JobError> {
        let mut tx = self.db.begin(&ctx.scope).await?;
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT verdict, reason FROM dedupe_verdicts \
             WHERE a_id = $1 AND b_id = $2 AND a_hash = $3 AND b_hash = $4",
        )
        .bind(&key.0)
        .bind(&key.1)
        .bind(&hashes.0)
        .bind(&hashes.1)
        .fetch_optional(tx.conn())
        .await?;
        tx.commit().await?;
        Ok(row.map(|(v, reason)| {
            let verdict = match v.as_str() {
                "duplicate" => DuplicateVerdict::Duplicate,
                "distinct" => DuplicateVerdict::Distinct,
                _ => DuplicateVerdict::Uncertain,
            };
            (verdict, reason)
        }))
    }

    /// Remembers an LLM verdict on the pair for the judged texts.
    async fn store_verdict(
        &self,
        ctx: &JobContext,
        key: &(String, String),
        hashes: &(String, String),
        verdict: DuplicateVerdict,
        reason: &str,
    ) -> Result<(), JobError> {
        let v = match verdict {
            DuplicateVerdict::Duplicate => "duplicate",
            DuplicateVerdict::Distinct => "distinct",
            DuplicateVerdict::Uncertain => "uncertain",
        };
        let mut tx = self.db.begin(&ctx.scope).await?;
        sqlx::query(
            "INSERT INTO dedupe_verdicts (user_id, a_id, b_id, a_hash, b_hash, verdict, reason, at) \
             VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (user_id, a_id, b_id) DO UPDATE SET a_hash = EXCLUDED.a_hash, \
               b_hash = EXCLUDED.b_hash, verdict = EXCLUDED.verdict, reason = EXCLUDED.reason, \
               at = EXCLUDED.at",
        )
        .bind(&key.0)
        .bind(&key.1)
        .bind(&hashes.0)
        .bind(&hashes.1)
        .bind(v)
        .bind(reason)
        .bind(ctx.now)
        .execute(tx.conn())
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn ordered(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

/// Keeps the strongest match of each unordered pair not suggested before.
#[allow(clippy::too_many_arguments)]
fn insert_pair(
    found: &mut BTreeMap<(String, String), FoundPair>,
    suggested: &BTreeSet<(String, String)>,
    x: (&StoredItem, Option<NoteId>),
    y: (&StoredItem, Option<NoteId>),
    level: dedupe::MatchLevel,
    score: f32,
    reason: Option<String>,
) {
    let key = ordered(&x.0.item_id, &y.0.item_id);
    if key.0 == key.1 || suggested.contains(&key) {
        return;
    }
    let ((a, a_note), (b, b_note)) = if x.0.item_id <= y.0.item_id {
        (x, y)
    } else {
        (y, x)
    };
    let pair = FoundPair {
        a: a.clone(),
        a_note,
        b: b.clone(),
        b_note,
        level,
        score,
        reason,
    };
    match found.get(&key) {
        Some(prev)
            if prev.level < pair.level
                || (prev.level == pair.level && prev.score >= pair.score) => {}
        _ => {
            found.insert(key, pair);
        }
    }
}

/// Pairs already proposed as `duplicates` suggestions (any status).
async fn suggested_pairs(
    tx: &mut strata_index::ScopedTx,
) -> Result<BTreeSet<(String, String)>, JobError> {
    let rows: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT payload FROM suggestions WHERE kind = $1")
            .bind(DUPLICATES)
            .fetch_all(tx.conn())
            .await?;
    Ok(rows
        .iter()
        .filter_map(|p| rmp_serde::from_slice::<DuplicatesPayload>(p).ok())
        .map(|p| ordered(&p.a.item, &p.b.item))
        .collect())
}

/// The ULID an item is shown under: the note's own ID, or the task's.
fn display_id(item: &StoredItem, note: Option<NoteId>) -> Option<String> {
    if let Ok(n) = item.item_id.parse::<NoteId>() {
        return Some(n.to_string());
    }
    strata_vault::prepare::task_ulid(&item.item_id)
        .map(|u| u.to_string())
        .or_else(|| note.map(|n| n.to_string()))
}

fn level_name(level: dedupe::MatchLevel) -> &'static str {
    match level {
        dedupe::MatchLevel::Exact => "exact",
        dedupe::MatchLevel::Near => "near",
        dedupe::MatchLevel::Semantic => "semantic",
    }
}

fn payload_item(item: &StoredItem, note: Option<NoteId>, level: &str, score: f32) -> Option<DuplicatePayloadItem> {
    Some(DuplicatePayloadItem {
        id: display_id(item, note)?,
        item: item.item_id.clone(),
        snippet: item.item.snippet.clone(),
        kind: item.kind.clone(),
        title: item.item.title.clone(),
        match_level: level.to_owned(),
        score: f64::from(score),
    })
}

#[async_trait::async_trait]
impl JobHandler for DedupeHandler {
    fn kind(&self) -> &'static str {
        DEDUPE
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let pairs = self.find_pairs(&ctx).await?;
        for p in pairs {
            let level = level_name(p.level);
            let (Some(a), Some(b)) = (
                payload_item(&p.a, p.a_note, level, p.score),
                payload_item(&p.b, p.b_note, level, p.score),
            ) else {
                continue;
            };
            let note = p.a_note.or_else(|| a.id.parse().ok());
            let payload = rmp_serde::to_vec_named(&DuplicatesPayload {
                a,
                b,
                reason: p.reason,
            })
            .map_err(|e| JobError::Fatal(format!("payload: {e}")))?;
            self.vault
                .create_suggestion(
                    &ctx.scope,
                    SuggestionId::generate(self.ids.as_ref()),
                    note,
                    DUPLICATES,
                    &payload,
                )
                .await?;
        }
        Ok(())
    }
}
