//! Correcting the AI in words (PLAN §9.8).
//!
//! - `correct` job: a capture the filing call flagged as a correction, or an Ask message that
//!   looks like one ([`looks_like_correction`]), goes to the `correction` prompt with the
//!   user's last ~50 AI decisions and the candidate entities. A confident, unambiguous fix
//!   (every fix ≥ the relation threshold, targets valid) is applied at once in one
//!   `ai: correct <path>` commit (repoint/retype/reject, the rejected link recorded, hints
//!   stored); otherwise it becomes a `correction` suggestion carrying the model's question.
//! - `suggestion_reply` job: a reply to an AI suggestion ("no, the Petrol Arrows one") goes
//!   to the same prompt with the thread; the AI re-proposes (a new suggestion replacing the
//!   old one, `superseded`), rejects it when the reply says so, or answers in the thread.
//!
//! Disambiguation hints go to the entity's sidecar and the `disambiguation_hints` table and
//! are shown to every later resolution prompt.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use strata_ai::outputs::{Correction, CorrectionAction};
use strata_ai::prompts::{self, ids};
use strata_ai::{AiCaller, AiError, AiService};
use strata_common::{Clock, DecisionId, HintId, IdGenerator, NoteId, ReplyId, SuggestionId};
use strata_index::repo::suggestions as srepo;
use strata_index::types::{DecisionKind, ReplyAuthor, SuggestionStatus};
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai_apply::{
    AiApplied, AiChangeSet, DecideSuggestion, NewDecision, NewHint, NewSuggestion,
    SuggestionDecision,
};
use strata_vault::ops::ai_decide::{self as decide, DecisionFix, DecisionView, FixAction};
use sync_model::suggestions::{
    CorrectionFix, CorrectionHint, CorrectionPayload, CustodyPayload, EntityLinkPayload,
};
use text_normalize::normalize_for_search;
use vault_format::RelationKey;

use crate::handler::{JobClass, JobContext, JobError, JobHandler};
use crate::pipeline::{self, Directory, EntityInput};
use crate::thresholds::AiThresholds;

/// Job kind: a correction in words.
pub const CORRECT: &str = "correct";
/// Job kind: a reply to an AI suggestion (the vault enqueues it).
pub const SUGGESTION_REPLY: &str = decide::SUGGESTION_REPLY_JOB;
/// Decisions given to the model.
pub const RECENT_DECISIONS: i64 = 50;

/// Output token limit.
const MAX_TOKENS: u32 = 1500;

/// Parameters of a `correct` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectParams {
    /// The user's words.
    pub message: String,
    /// When they were written (RFC 3339, user's time zone).
    pub created: String,
    /// The capture holding them (`None` for an Ask message).
    pub source: Option<NoteId>,
}

/// The message in a `correction` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageInput {
    /// Text.
    pub text: String,
    /// Written at.
    pub created: String,
}

/// A decision in a `correction` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionInput {
    /// Decision ID.
    pub id: String,
    /// `relation`, `entity_link`, `custody_event`, `task_suggestion`, `filing`, `concept`.
    pub kind: String,
    /// Source note.
    pub source_note_id: Option<String>,
    /// Its title.
    pub source_title: Option<String>,
    /// Its `created`.
    pub source_created: Option<String>,
    /// Mention text.
    pub mention: Option<String>,
    /// Target.
    pub target_id: Option<String>,
    /// Target's name.
    pub target_name: Option<String>,
    /// Relation key or custody type.
    #[serde(rename = "type")]
    pub rel_type: Option<String>,
    /// When.
    pub created: String,
}

/// The suggestion of a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreadSuggestion {
    /// Suggestion ID.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// What it proposes (the decision's one-line summary).
    pub summary: String,
    /// The decision it came from.
    pub decision_id: Option<String>,
}

/// One reply of a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreadReply {
    /// `user` or `ai`.
    pub author: String,
    /// Text.
    pub text: String,
}

/// A suggestion thread in a `correction` prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreadInput {
    /// The suggestion.
    pub suggestion: ThreadSuggestion,
    /// Replies in order.
    pub replies: Vec<ThreadReply>,
}

/// The `correction` prompt input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorrectionInput {
    /// The message.
    pub message: MessageInput,
    /// The thread, for replies.
    pub thread: Option<ThreadInput>,
    /// Recent decisions, newest first.
    pub decisions: Vec<DecisionInput>,
    /// Candidate entities.
    pub candidates: Vec<EntityInput>,
}

/// A cheap pre-filter for Ask messages (the model decides): cue words of a correction in
/// English or Arabic (normalised).
pub fn looks_like_correction(text: &str) -> bool {
    const CUES: &[&str] = &[
        "is not",
        "isn't",
        "wasn't",
        "wrong",
        "actually",
        "not the",
        "i meant",
        "should be",
        "مش",
        "غلط",
        "قصدي",
        "اقصد",
        "الصح",
        "مو",
        "ليس",
    ];
    let n = format!(" {} ", normalize_for_search(text));
    let lower = format!(" {} ", text.to_lowercase());
    let trimmed = text.trim();
    let question = trimmed.ends_with('?') || trimmed.ends_with('؟');
    // "the Ahmed in yesterday's Acme call is Ahmed Fathy" / "أحمد اللي في المكالمة هو أحمد فتحي"
    let identity = !question
        && ((lower.trim_start().starts_with("the ") && lower.contains(" is "))
            || n.contains(" اللي ") && (n.contains(" هو ") || n.contains(" هي ")));
    text.contains('=')
        || identity
        || CUES.iter().any(|c| {
            let cue = format!(" {} ", normalize_for_search(c));
            n.contains(&cue) || lower.contains(&format!(" {c} "))
        })
}

fn kind_name(k: DecisionKind) -> &'static str {
    match k {
        DecisionKind::EntityMention => "entity_link",
        DecisionKind::Relation => "relation",
        DecisionKind::CustodyEvent => "custody_event",
        DecisionKind::TaskSuggestion => "task_suggestion",
        DecisionKind::Filing => "filing",
        DecisionKind::Concept => "concept",
        DecisionKind::Link => "link",
        DecisionKind::Correction => "correction",
    }
}

/// The prompt form of a decision.
pub fn decision_input(v: &DecisionView) -> DecisionInput {
    let r = &v.row;
    DecisionInput {
        id: r.id.to_string(),
        kind: kind_name(r.kind).to_owned(),
        source_note_id: r.source_note_id.map(|n| n.to_string()),
        source_title: v.source_title.clone(),
        source_created: v.source_created.map(|c| c.to_rfc3339()),
        mention: r.mention.clone(),
        target_id: (!r.target_id.is_empty()).then(|| r.target_id.clone()),
        target_name: v.target_name.clone(),
        rel_type: r.rel_type.clone(),
        created: r.created.to_rfc3339(),
    }
}

/// Everything the correction jobs need.
#[derive(Debug, Clone)]
pub struct CorrectDeps {
    /// Database.
    pub db: AppDb,
    /// Vault.
    pub vault: VaultService,
    /// AI.
    pub ai: Arc<AiService>,
    /// IDs.
    pub ids: Arc<dyn IdGenerator>,
    /// Clock.
    pub clock: Arc<dyn Clock>,
    /// Thresholds.
    pub thresholds: AiThresholds,
}

impl CorrectDeps {
    /// Open decisions (not yet corrected), newest first.
    async fn decisions(&self, scope: &UserScope) -> Result<Vec<DecisionView>, JobError> {
        Ok(self
            .vault
            .ai_decisions(scope, RECENT_DECISIONS)
            .await?
            .into_iter()
            .filter(|d| d.row.reverted_at.is_none() && d.row.kind != DecisionKind::Correction)
            .collect())
    }

    async fn call(
        &self,
        ctx: &JobContext,
        input: &CorrectionInput,
    ) -> Result<Option<Correction>, JobError> {
        let prompt = prompts::latest(ids::CORRECTION)
            .ok_or_else(|| JobError::Fatal("correction prompt missing".into()))?;
        let caller = AiCaller {
            scope: ctx.scope,
            username: ctx.username.clone(),
        };
        match self
            .ai
            .complete::<Correction>(caller, prompt, input, MAX_TOKENS)
            .await
        {
            Ok(o) => Ok(Some(o.value)),
            Err(AiError::Disabled | AiError::ProviderNotConfigured(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Candidate entities: those named in `text` and the entity targets of `decisions`.
    fn candidates(
        dir: &Directory,
        text: &str,
        decisions: &[DecisionView],
        extra: &[NoteId],
    ) -> Vec<EntityInput> {
        let mut ids: BTreeSet<NoteId> = decisions
            .iter()
            .filter_map(|d| d.row.target_id.parse::<NoteId>().ok())
            .filter(|i| dir.entity(*i).is_some())
            .collect();
        ids.extend(extra.iter().copied());
        dir.entity_inputs(&dir.select(text, &ids))
    }
}

/// Validates the model's fixes against the decisions and directory.
fn valid_fixes(
    out: &Correction,
    decisions: &[DecisionView],
    dir: &Directory,
) -> Option<Vec<(DecisionFix, f64, String)>> {
    let mut fixes = Vec::new();
    for f in &out.fixes {
        let d = decisions
            .iter()
            .find(|d| d.row.id.to_string() == f.decision_id)?;
        let action = match f.action {
            CorrectionAction::Repoint => {
                let t: NoteId = f.new_target_id.as_deref()?.parse().ok()?;
                let old_kind = d
                    .row
                    .target_id
                    .parse::<NoteId>()
                    .ok()
                    .and_then(|o| dir.entity(o))
                    .map(|e| e.kind);
                let new_kind = dir.entity(t).map(|e| e.kind);
                if old_kind.is_some() && old_kind != new_kind {
                    return None;
                }
                FixAction::Repoint(t)
            }
            CorrectionAction::Retype => {
                let s = f.new_type.as_deref()?;
                FixAction::Retype(RelationKey::all().find(|k| k.as_str() == s)?)
            }
            CorrectionAction::Reject => FixAction::Reject,
        };
        fixes.push((
            DecisionFix {
                decision: d.row.id,
                action,
                hint: None,
            },
            f.confidence,
            f.reason.clone(),
        ));
    }
    Some(fixes)
}

fn fix_payload(f: &DecisionFix, confidence: f64, reason: &str) -> CorrectionFix {
    let (action, new_target, new_type) = match &f.action {
        FixAction::Repoint(t) => ("repoint", Some(*t), None),
        FixAction::Retype(k) => ("retype", None, Some(k.as_str().to_owned())),
        FixAction::Reject => ("reject", None, None),
    };
    CorrectionFix {
        decision_id: f.decision.as_ulid(),
        action: action.to_owned(),
        new_target: new_target.map(|n| n.as_ulid()),
        new_type,
        confidence,
        reason: reason.to_owned(),
    }
}

/// The `correct` job.
#[derive(Debug, Clone)]
pub struct CorrectHandler {
    deps: CorrectDeps,
}

impl CorrectHandler {
    /// The handler.
    pub fn new(deps: CorrectDeps) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl JobHandler for CorrectHandler {
    fn kind(&self) -> &'static str {
        CORRECT
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    #[allow(clippy::too_many_lines)] // one linear pass: context, call, validate, apply or suggest
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let d = &self.deps;
        let params: CorrectParams = rmp_serde::from_slice(&ctx.job.payload)
            .map_err(|_| JobError::Fatal("correct job without parameters".into()))?;
        let decisions = d.decisions(&ctx.scope).await?;
        if decisions.is_empty() {
            return Ok(());
        }
        let dir = Directory::load(&d.db, &ctx.scope).await?;
        let input = CorrectionInput {
            message: MessageInput {
                text: params.message.clone(),
                created: params.created.clone(),
            },
            thread: None,
            decisions: decisions.iter().map(decision_input).collect(),
            candidates: CorrectDeps::candidates(&dir, &params.message, &decisions, &[]),
        };
        let Some(out) = d.call(&ctx, &input).await? else {
            return Ok(());
        };
        if !out.is_correction || out.fixes.is_empty() {
            return Ok(());
        }
        let fixes = valid_fixes(&out, &decisions, &dir);
        let hints: Vec<(NoteId, String)> = out
            .hints
            .iter()
            .filter_map(|h| {
                let e: NoteId = h.entity_id.parse().ok()?;
                dir.entity(e).map(|_| (e, h.text.clone()))
            })
            .collect();
        // The commit is about the capture holding the words, else the corrected note.
        let first_fixed = out.fixes.first().and_then(|f| {
            decisions
                .iter()
                .find(|x| x.row.id.to_string() == f.decision_id)
                .and_then(|x| x.row.source_note_id)
        });
        let Some(subject) = params.source.or(first_fixed) else {
            return Ok(());
        };
        let decision_id = DecisionId::generate(d.ids.as_ref());
        let confident = fixes.as_ref().is_some_and(|f| {
            !out.ambiguous && f.iter().all(|(_, c, _)| *c >= d.thresholds.relation)
        });
        let mut set = AiChangeSet::new(CORRECT, subject);
        set.job_id = Some(ctx.job.id);
        let summary = format!(
            "correction: {}",
            params.message.chars().take(120).collect::<String>()
        );
        let mut decision = NewDecision {
            id: decision_id,
            kind: DecisionKind::Correction,
            source_note: params.source,
            source_block: None,
            target_type: "decision".into(),
            target_id: out
                .fixes
                .first()
                .map(|f| f.decision_id.clone())
                .unwrap_or_default(),
            summary,
            confidence: out
                .fixes
                .iter()
                .map(|f| f.confidence)
                .fold(None, |m: Option<f64>, c| Some(m.map_or(c, |m| m.min(c))))
                .map(|c| {
                    #[allow(clippy::cast_possible_truncation)]
                    let c = c as f32;
                    c
                }),
            rel_type: None,
            mention: None,
            detail: Vec::new(),
            suggestion: None,
        };
        if confident && let Some(fixes) = fixes {
            for (entity, text) in hints {
                set.hints.push(NewHint {
                    id: HintId::generate(d.ids.as_ref()),
                    entity,
                    text,
                    source_decision: Some(decision_id),
                });
            }
            set.decisions.push(decision);
            let fixes: Vec<DecisionFix> = fixes.into_iter().map(|f| f.0).collect();
            match d.vault.ai_correct(&ctx.scope, set, fixes).await {
                Ok(AiApplied::Done { .. } | AiApplied::Stale) => return Ok(()),
                // The decision was corrected meanwhile, or the fix does not apply: nothing
                // to do automatically.
                Err(strata_vault::VaultError::Invalid(_) | strata_vault::VaultError::NotFound) => {
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        }
        let payload = CorrectionPayload {
            decision_id: decision_id.as_ulid(),
            message: params.message.clone(),
            fixes: fixes
                .map(|f| f.iter().map(|(fx, c, r)| fix_payload(fx, *c, r)).collect())
                .unwrap_or_default(),
            hints: hints
                .into_iter()
                .map(|(entity, text)| CorrectionHint {
                    entity: entity.as_ulid(),
                    text,
                })
                .collect(),
            question: out.clarification_question.clone(),
        };
        let sid = SuggestionId::generate(d.ids.as_ref());
        set.suggestions.push(NewSuggestion {
            id: sid,
            note: params.source,
            kind: decide::KIND_CORRECTION.to_owned(),
            payload: pipeline::rmp(&payload)?,
        });
        decision.suggestion = Some(sid);
        set.decisions.push(decision);
        match d.vault.ai_apply(&ctx.scope, set).await? {
            AiApplied::Done { .. } | AiApplied::Stale => Ok(()),
        }
    }
}

/// The `suggestion_reply` job.
#[derive(Debug, Clone)]
pub struct ReplyHandler {
    deps: CorrectDeps,
}

impl ReplyHandler {
    /// The handler.
    pub fn new(deps: CorrectDeps) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl JobHandler for ReplyHandler {
    fn kind(&self) -> &'static str {
        SUGGESTION_REPLY
    }

    fn class(&self) -> JobClass {
        JobClass::Llm
    }

    #[allow(clippy::too_many_lines)] // one linear pass: thread, call, re-propose or answer
    async fn run(&self, ctx: JobContext) -> Result<(), JobError> {
        let d = &self.deps;
        let sid: SuggestionId = rmp_serde::from_slice(&ctx.job.payload)
            .map_err(|_| JobError::Fatal("suggestion_reply job without a suggestion".into()))?;
        let mut tx = d.db.begin(&ctx.scope).await?;
        let s = srepo::get_suggestion(&mut tx, sid).await?;
        let replies = srepo::replies(&mut tx, sid).await?;
        tx.commit().await?;
        let Some(s) = s.filter(|s| s.status == SuggestionStatus::Pending) else {
            return Ok(());
        };
        let Some(last) = replies.last().filter(|r| r.author == ReplyAuthor::User) else {
            return Ok(());
        };
        let own = d.vault.decision_of_suggestion(&ctx.scope, sid).await?;
        let mut decisions = d.decisions(&ctx.scope).await?;
        if let Some(own) = &own {
            decisions.retain(|x| x.row.id != own.id);
            decisions.insert(0, d.vault.ai_decision(&ctx.scope, own.id).await?);
        }
        let dir = Directory::load(&d.db, &ctx.scope).await?;
        let mut extra: Vec<NoteId> = Vec::new();
        let mut mention = None;
        match s.kind.as_str() {
            decide::KIND_ENTITY_LINK => {
                if let Ok(p) = rmp_serde::from_slice::<EntityLinkPayload>(&s.payload) {
                    extra.extend(p.candidates.iter().copied().map(NoteId::from_ulid));
                    extra.extend(p.proposed.map(NoteId::from_ulid));
                    mention = Some(p.mention);
                }
            }
            decide::KIND_CUSTODY => {
                if let Ok(p) = rmp_serde::from_slice::<CustodyPayload>(&s.payload) {
                    for t in [
                        Some(&p.document),
                        p.place.as_ref(),
                        p.person.as_ref(),
                        p.counterparty.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        extra.extend(t.candidates.iter().copied().map(NoteId::from_ulid));
                        extra.extend(t.id.map(NoteId::from_ulid));
                    }
                }
            }
            _ => {}
        }
        let text_for_match = format!("{} {}", mention.clone().unwrap_or_default(), last.body);
        let input = CorrectionInput {
            message: MessageInput {
                text: last.body.clone(),
                created: last.created.to_rfc3339(),
            },
            thread: Some(ThreadInput {
                suggestion: ThreadSuggestion {
                    id: sid.to_string(),
                    kind: s.kind.clone(),
                    summary: own.as_ref().map(|o| o.summary.clone()).unwrap_or_default(),
                    decision_id: own.as_ref().map(|o| o.id.to_string()),
                },
                replies: replies
                    .iter()
                    .map(|r| ThreadReply {
                        author: match r.author {
                            ReplyAuthor::User => "user",
                            ReplyAuthor::Ai => "ai",
                        }
                        .to_owned(),
                        text: r.body.clone(),
                    })
                    .collect(),
            }),
            decisions: decisions.iter().map(decision_input).collect(),
            candidates: CorrectDeps::candidates(&dir, &text_for_match, &[], &extra),
        };
        let Some(out) = d.call(&ctx, &input).await? else {
            return Ok(());
        };
        let Some(subject) = s.note_id else {
            return Ok(());
        };
        let mut set = AiChangeSet::new(SUGGESTION_REPLY, subject);
        set.job_id = Some(ctx.job.id);
        let own_id = own.as_ref().map(|o| o.id.to_string());
        let fix = out
            .fixes
            .iter()
            .find(|f| Some(&f.decision_id) == own_id.as_ref())
            .or_else(|| out.fixes.first());
        let supersede = |set: &mut AiChangeSet, decision: SuggestionDecision| {
            set.decide.push(DecideSuggestion {
                id: sid,
                note: s.note_id,
                kind: s.kind.clone(),
                decision,
            });
        };
        let reply_id = ReplyId::generate(d.ids.as_ref());
        let target = fix
            .filter(|f| f.action == CorrectionAction::Repoint && !out.ambiguous)
            .and_then(|f| f.new_target_id.as_deref())
            .and_then(|t| t.parse::<NoteId>().ok())
            .and_then(|t| dir.entity(t).map(|e| (t, e.name.clone(), e.kind)));
        match (s.kind.as_str(), fix.map(|f| f.action), target) {
            (_, Some(CorrectionAction::Reject), _) if !out.ambiguous => {
                if s.kind == decide::KIND_ENTITY_LINK
                    && let Ok(p) = rmp_serde::from_slice::<EntityLinkPayload>(&s.payload)
                {
                    set.rejected_mentions
                        .push((NoteId::from_ulid(p.source_note), p.mention.clone(), p.kind.clone()));
                }
                supersede(&mut set, SuggestionDecision::Rejected);
                set.ai_replies.push((
                    sid,
                    reply_id,
                    "Understood — I withdrew this suggestion.".to_owned(),
                ));
            }
            (decide::KIND_ENTITY_LINK, _, Some((t, name, kind))) => {
                let mut p: EntityLinkPayload = rmp_serde::from_slice(&s.payload)
                    .map_err(|_| JobError::Fatal("unreadable suggestion payload".into()))?;
                if p.kind == kind.as_str() {
                    let new_decision = DecisionId::generate(d.ids.as_ref());
                    p.decision_id = new_decision.as_ulid();
                    p.proposed = Some(t.as_ulid());
                    p.candidates = vec![t.as_ulid()];
                    "reply".clone_into(&mut p.reason);
                    let new_sid = SuggestionId::generate(d.ids.as_ref());
                    set.suggestions.push(NewSuggestion {
                        id: new_sid,
                        note: s.note_id,
                        kind: s.kind.clone(),
                        payload: pipeline::rmp(&p)?,
                    });
                    set.decisions.push(NewDecision {
                        id: new_decision,
                        kind: DecisionKind::EntityMention,
                        source_note: Some(NoteId::from_ulid(p.source_note)),
                        source_block: p.block_id.clone(),
                        target_type: "entity".into(),
                        target_id: t.to_string(),
                        summary: format!("\"{}\" → {name}? (reply)", p.mention),
                        confidence: None,
                        rel_type: own.as_ref().and_then(|o| o.rel_type.clone()),
                        mention: Some(p.mention.clone()),
                        detail: Vec::new(),
                        suggestion: Some(new_sid),
                    });
                    if let Some(own) = &own {
                        set.revert_decisions.push(own.id);
                    }
                    supersede(&mut set, SuggestionDecision::Superseded);
                    set.ai_replies.push((
                        sid,
                        reply_id,
                        format!("New suggestion: \"{}\" → {name}.", p.mention),
                    ));
                } else {
                    set.ai_replies.push((
                        sid,
                        reply_id,
                        format!("{name} is not a {}; I kept the suggestion.", p.kind),
                    ));
                }
            }
            (decide::KIND_CUSTODY, _, Some((t, name, kind))) => {
                let mut p: CustodyPayload = rmp_serde::from_slice(&s.payload)
                    .map_err(|_| JobError::Fatal("unreadable suggestion payload".into()))?;
                let roles = [
                    (domain::NoteKind::Document, Some(&mut p.document)),
                    (domain::NoteKind::Place, p.place.as_mut()),
                    (domain::NoteKind::Person, p.person.as_mut()),
                    (domain::NoteKind::Company, p.counterparty.as_mut()),
                ];
                let mut done = false;
                for (k, role) in roles {
                    if let Some(r) = role
                        && k == kind
                        && (r.id.is_none() || r.candidates.contains(&t.as_ulid()))
                    {
                        r.id = Some(t.as_ulid());
                        r.candidates = Vec::new();
                        done = true;
                        break;
                    }
                }
                if done {
                    let new_decision = DecisionId::generate(d.ids.as_ref());
                    p.decision_id = new_decision.as_ulid();
                    "reply".clone_into(&mut p.reason);
                    let new_sid = SuggestionId::generate(d.ids.as_ref());
                    set.suggestions.push(NewSuggestion {
                        id: new_sid,
                        note: s.note_id,
                        kind: s.kind.clone(),
                        payload: pipeline::rmp(&p)?,
                    });
                    if let Some(own) = &own {
                        let mut nd = NewDecision {
                            id: new_decision,
                            kind: DecisionKind::CustodyEvent,
                            source_note: own.source_note_id,
                            source_block: own.source_block_id.clone(),
                            target_type: own.target_type.clone(),
                            target_id: p.document.id.map_or_else(String::new, |i| i.to_string()),
                            summary: format!("{} (with {name}, reply)", own.summary),
                            confidence: own.confidence,
                            rel_type: own.rel_type.clone(),
                            mention: own.mention.clone(),
                            detail: own.detail.clone(),
                            suggestion: Some(new_sid),
                        };
                        nd.summary = nd.summary.trim().to_string();
                        set.decisions.push(nd);
                        set.revert_decisions.push(own.id);
                    }
                    supersede(&mut set, SuggestionDecision::Superseded);
                    set.ai_replies
                        .push((sid, reply_id, format!("New suggestion with {name}.")));
                } else {
                    set.ai_replies.push((
                        sid,
                        reply_id,
                        format!("{name} does not fit this event; edit it when accepting."),
                    ));
                }
            }
            _ => {
                let text = out.clarification_question.clone().unwrap_or_else(|| {
                    "I could not change the proposal from this reply; edit it when accepting."
                        .to_owned()
                });
                set.ai_replies.push((sid, reply_id, text));
            }
        }
        for h in &out.hints {
            if let Ok(e) = h.entity_id.parse::<NoteId>()
                && dir.entity(e).is_some()
            {
                set.hints.push(NewHint {
                    id: HintId::generate(d.ids.as_ref()),
                    entity: e,
                    text: h.text.clone(),
                    source_decision: own.as_ref().map(|o| o.id),
                });
            }
        }
        match d.vault.ai_apply(&ctx.scope, set).await {
            Ok(_) | Err(strata_vault::VaultError::Invalid(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Enqueues a `correct` job for an Ask message that looks like a correction.
pub async fn enqueue_for_message(
    db: &AppDb,
    scope: &UserScope,
    ids: &dyn IdGenerator,
    message: &str,
    created: chrono::DateTime<chrono::FixedOffset>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, JobError> {
    if !looks_like_correction(message) {
        return Ok(false);
    }
    let mut tx = db.begin(scope).await?;
    crate::repo::enqueue(
        &mut tx,
        &strata_index::repo::jobs::NewJob {
            id: strata_common::JobId::generate(ids),
            kind: CORRECT.to_owned(),
            note_id: None,
            payload: pipeline::rmp(&CorrectParams {
                message: message.trim().to_owned(),
                created: created.to_rfc3339(),
                source: None,
            })?,
            run_after: now,
            max_attempts: 5,
            dedupe_key: None,
        },
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_cues_pre_filter_ask_messages() {
        assert!(looks_like_correction(
            "the Ahmed in yesterday's Acme call is actually Ahmed Fathy"
        ));
        assert!(looks_like_correction(
            "أحمد اللي في مكالمة أكمي مش أحمد سمير"
        ));
        assert!(looks_like_correction("Ahmed = Ahmed Fathy"));
        assert!(looks_like_correction(
            "the Ahmed in yesterday's Acme call is Ahmed Fathy"
        ));
        assert!(looks_like_correction(
            "أحمد اللي في مكالمة أكمي هو أحمد فتحي"
        ));
        assert!(!looks_like_correction("the invoice is due tomorrow?"));
        assert!(!looks_like_correction("where is the Watanya contract?"));
        assert!(!looks_like_correction("فين عقد وطنية؟"));
    }
}
