//! The user's side of the AI pipelines (PLAN §6.7, §6.12, §9.3, §9.7, §9.8): the payloads of
//! AI suggestions, accepting (with optional edits) and rejecting them, the AI decision log,
//! and corrections of applied decisions (repoint, retype, reject) with disambiguation hints.
//!
//! Suggestion kinds and what accepting does (one commit each):
//!
//! | kind | accept | reject |
//! |---|---|---|
//! | `filing` | title, tags and move of the capture (edits: `title`, `tags`, `folder`) | nothing (the capture stays in the inbox) |
//! | `entity_link` | links the note to the entity (edits: `target_id`) or creates it (name: `title` edit, else the mention); adds the mention and `aliases` edits to the entity's aliases | records the mention (and the proposed target) as rejected for the note |
//! | `custody` | records the event on the document (edits: `target_id` resolves the first ambiguous role); unknown documents and places are created | nothing |
//! | `task` | writes the task line to `tasks/Tasks.md` (edits: `text`, `due`, `recurrence`, `reminders`) | nothing |
//! | `correction` | applies the proposed fixes and stores the hints | nothing |
//! | `duplicates` | merges the pair (see [`Core::merge_notes`]; entities: entity merge; tasks: the newer one is cancelled) | keep-both (in `ops::suggestions`) |

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use domain::{NoteKind, RelationType};
use serde::{Deserialize, Serialize};
use strata_common::{DecisionId, HintId, JobId, NoteId, SuggestionId};
use strata_index::UserScope;
use strata_index::repo::jobs::NewJob;
use strata_index::repo::suggestions::{self as srepo, Suggestion};
use strata_index::types::{DecisionKind, SuggestionStatus};
use sync_model::ops::SuggestionEdits;
use vault_format::custody::CustodyEventType;
use vault_format::frontmatter::KnownKey;
use vault_format::sidecar::{By, NoteSidecar};
use vault_format::{Document, RelationKey};

use crate::error::{Result, VaultError};
use crate::ops::ai::DuplicatesPayload;
use crate::ops::ai_apply::{
    AiApplied, AiChangeSet, AiCustody, Cite, DecideSuggestion, EdgeAdd, EdgeRemove, Filing,
    NewEntityNote, NewHint, SuggestionDecision,
};
use crate::ops::relations::AiEdge;
use crate::ops::suggestions::SuggestionView;
use crate::ops::tasks::{NewTask, Transition};
use crate::paths;
use crate::store::{Author, Core, VaultService};

/// Suggestion kind: filing proposal of an inbox capture (§9.3).
pub const KIND_FILING: &str = "filing";
/// Suggestion kind: link a mention to an entity or create it (§6.7, D13 = b).
pub const KIND_ENTITY_LINK: &str = "entity_link";
/// Suggestion kind: a custody event below the threshold or ambiguous (§6.12, D30).
pub const KIND_CUSTODY: &str = "custody";
/// Suggestion kind: a task proposed from a note (§6.11).
pub const KIND_TASK: &str = "task";
/// Suggestion kind: a correction in words that was not applied automatically (§9.8).
pub const KIND_CORRECTION: &str = "correction";
/// Suggestion kind: two stored items that are duplicates (§9.7).
pub const KIND_DUPLICATES: &str = "duplicates";

/// Job kind the vault enqueues when the user replies to an AI suggestion (§9.8 threads).
pub const SUGGESTION_REPLY_JOB: &str = "suggestion_reply";

/// Whether `kind` is decided here (the others by `ops::suggestions`).
pub fn handles(kind: &str, accept: bool) -> bool {
    matches!(
        kind,
        KIND_FILING | KIND_ENTITY_LINK | KIND_CUSTODY | KIND_TASK | KIND_CORRECTION
    ) || (accept && kind == KIND_DUPLICATES)
}

/// Whether a reply to a suggestion of `kind` makes the AI re-propose (§9.8).
pub fn replies_reach_ai(kind: &str) -> bool {
    matches!(
        kind,
        KIND_FILING | KIND_ENTITY_LINK | KIND_CUSTODY | KIND_TASK | KIND_CORRECTION
    )
}

/// `filing` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilingPayload {
    /// The decision.
    pub decision_id: DecisionId,
    /// Proposed title.
    pub title: String,
    /// Proposed tags.
    pub tags: Vec<String>,
    /// Proposed folder.
    pub folder: String,
}

/// `entity_link` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityLinkPayload {
    /// The decision.
    pub decision_id: DecisionId,
    /// The mention as written.
    pub mention: String,
    /// `person`, `company`, `document` or `place`.
    pub kind: String,
    /// The note that mentions it.
    pub source_note: NoteId,
    /// The block stating it.
    pub block_id: Option<String>,
    /// The entity proposed, if one.
    pub proposed: Option<NoteId>,
    /// Plausible entities (ambiguous).
    pub candidates: Vec<NoteId>,
    /// A nickname or kinship term.
    pub is_nickname: bool,
    /// Model confidence.
    pub confidence: f64,
    /// Why it is a suggestion: `ambiguous`, `nickname`, `new`, `low_confidence`.
    pub reason: String,
}

/// One participant of a custody suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodyTarget {
    /// Mention as written.
    pub mention: String,
    /// Resolved entity.
    pub id: Option<NoteId>,
    /// Plausible entities when ambiguous.
    pub candidates: Vec<NoteId>,
}

/// `custody` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustodyPayload {
    /// The decision.
    pub decision_id: DecisionId,
    /// The note stating it.
    pub source_note: NoteId,
    /// The block stating it.
    pub block_id: Option<String>,
    /// Event type (`stored-at`, …).
    pub event: String,
    /// Resolved date.
    pub date: NaiveDate,
    /// The document.
    pub document: CustodyTarget,
    /// The place.
    pub place: Option<CustodyTarget>,
    /// The enclosing place mention.
    pub place_part_of: Option<String>,
    /// The person.
    pub person: Option<CustodyTarget>,
    /// The third party.
    pub counterparty: Option<CustodyTarget>,
    /// Model confidence.
    pub confidence: f64,
    /// Why it is a suggestion: `low_confidence`, `ambiguous`, `unknown`, `conflict`.
    pub reason: String,
    /// The span stating it.
    pub quote: String,
}

/// `task` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskPayload {
    /// The decision.
    pub decision_id: DecisionId,
    /// The note it came from.
    pub source_note: NoteId,
    /// The block stating it.
    pub block_id: Option<String>,
    /// Title.
    pub title: String,
    /// Due date.
    pub due: Option<NaiveDate>,
    /// Recurrence phrase (Tasks plugin language).
    pub recurrence: Option<String>,
    /// Reminder times (user's time zone).
    pub reminders: Vec<NaiveDateTime>,
    /// Entities the task concerns (linked in the line).
    pub entities: Vec<NoteId>,
    /// Model confidence.
    pub confidence: f64,
}

/// One proposed fix of a `correction` suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixPayload {
    /// The decision to fix.
    pub decision_id: DecisionId,
    /// `repoint`, `retype`, `reject`.
    pub action: String,
    /// New target (`repoint`).
    pub new_target: Option<NoteId>,
    /// New relation type (`retype`).
    pub new_type: Option<String>,
    /// Model confidence.
    pub confidence: f64,
    /// One sentence.
    pub reason: String,
}

/// A hint of a `correction` suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintPayload {
    /// Entity.
    pub entity: NoteId,
    /// Text.
    pub text: String,
}

/// `correction` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionPayload {
    /// The correction decision.
    pub decision_id: DecisionId,
    /// The user's words.
    pub message: String,
    /// Proposed fixes.
    pub fixes: Vec<FixPayload>,
    /// Hints to remember.
    pub hints: Vec<HintPayload>,
    /// The question to ask when ambiguous.
    pub question: Option<String>,
}

/// Custody decision context (`ai_decisions.detail`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodyDetail {
    /// Event date.
    pub date: NaiveDate,
}

/// What a correction does to a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixAction {
    /// Same mention/source, different target.
    Repoint(NoteId),
    /// Different relation type.
    Retype(RelationKey),
    /// The decision is wrong.
    Reject,
}

/// A correction of one AI decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionFix {
    /// The decision.
    pub decision: DecisionId,
    /// What to do.
    pub action: FixAction,
    /// A hint to store with a repoint (default: derived from the mention and source).
    pub hint: Option<String>,
}

/// An `ai_decisions` row with the pipeline columns.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct DecisionRow {
    /// ID.
    pub id: DecisionId,
    /// Kind.
    pub kind: DecisionKind,
    /// Source note.
    pub source_note_id: Option<NoteId>,
    /// Source block.
    pub source_block_id: Option<String>,
    /// Target type.
    pub target_type: String,
    /// Target.
    pub target_id: String,
    /// One line.
    pub summary: String,
    /// Confidence.
    pub confidence: Option<f32>,
    /// Job.
    pub job_id: Option<JobId>,
    /// Suggestion (not applied automatically).
    pub suggestion_id: Option<SuggestionId>,
    /// Commit that applied it.
    pub git_commit: Option<String>,
    /// When.
    pub created: DateTime<Utc>,
    /// Reverted (rejected or repointed) at.
    pub reverted_at: Option<DateTime<Utc>>,
    /// Relation key or custody type.
    pub rel_type: Option<String>,
    /// Mention.
    pub mention: Option<String>,
    /// `MessagePack` context.
    pub detail: Vec<u8>,
}

/// A decision for the activity feed and the correction prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionView {
    /// The row.
    pub row: DecisionRow,
    /// Title of the source note.
    pub source_title: Option<String>,
    /// `created` of the source note.
    pub source_created: Option<DateTime<Utc>>,
    /// Display name of the target (note/entity title) when it is a note.
    pub target_name: Option<String>,
    /// Status of its suggestion, if any.
    pub suggestion_status: Option<SuggestionStatus>,
}

const DECISION_COLS: &str = "id, kind, source_note_id, source_block_id, target_type, target_id, \
    summary, confidence, job_id, suggestion_id, git_commit, created, reverted_at, rel_type, \
    mention, detail";

/// Encodes a payload as a named-map `MessagePack` blob.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    rmp_serde::to_vec_named(value).map_err(|e| VaultError::Internal(format!("payload: {e}")))
}

fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    rmp_serde::from_slice(bytes).map_err(|_| VaultError::invalid("the suggestion payload is unreadable"))
}

fn parse_rel(s: Option<&str>) -> Option<RelationKey> {
    let s = s?;
    RelationKey::all().find(|k| k.as_str() == s)
}

fn entity_rel(kind: NoteKind) -> Option<RelationKey> {
    match kind {
        NoteKind::Person => Some(RelationKey::Mention(domain::MentionType::People)),
        NoteKind::Company => Some(RelationKey::Mention(domain::MentionType::Companies)),
        _ => None,
    }
}

fn user_edge(src: NoteId, rel: RelationKey, dst: NoteId) -> EdgeAdd {
    EdgeAdd {
        src,
        edge: AiEdge {
            rel,
            dst,
            confidence: 1.0,
            reason: String::new(),
            model: String::new(),
        },
        by: By::User,
    }
}

impl Core {
    fn live_kind(&self, id: NoteId) -> Option<NoteKind> {
        self.state().ok()?.note(id).map(|(_, m)| m.kind)
    }

    fn title_of_id(&self, id: NoteId) -> Option<String> {
        let state = self.state().ok()?;
        let (path, _) = state.note(id)?;
        Some(
            paths::file_name(path)
                .trim_end_matches(".md")
                .to_owned(),
        )
    }

    /// The decision row `id` (scoped).
    pub(crate) async fn decision_row(&self, scope: &UserScope, id: DecisionId) -> Result<DecisionRow> {
        let mut tx = self.begin(scope).await?;
        let row: Option<DecisionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {DECISION_COLS} FROM ai_decisions WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(tx.conn())
        .await?;
        tx.commit().await?;
        row.ok_or(VaultError::NotFound)
    }

    /// Adds the effects of `fix` to `set` (a user or AI correction of an applied decision or
    /// of a pending suggestion).
    #[allow(clippy::too_many_lines)] // one match over decision kinds and actions
    pub(crate) async fn plan_fix(
        &self,
        scope: &UserScope,
        set: &mut AiChangeSet,
        fix: &DecisionFix,
    ) -> Result<()> {
        let d = self.decision_row(scope, fix.decision).await?;
        if d.reverted_at.is_some() {
            return Err(VaultError::invalid("the decision was already corrected"));
        }
        let pending = match d.suggestion_id {
            Some(s) => {
                let mut tx = self.begin(scope).await?;
                let row = srepo::get_suggestion(&mut tx, s).await?;
                tx.commit().await?;
                row.filter(|r| r.status == SuggestionStatus::Pending)
            }
            None => None,
        };
        if d.suggestion_id.is_some() && pending.is_none() {
            return Err(VaultError::invalid("the suggestion was already decided"));
        }
        let hint_for = |set: &mut AiChangeSet, target: NoteId| {
            let Some(mention) = d.mention.clone() else {
                return;
            };
            let source = d
                .source_note_id
                .and_then(|s| self.title_of_id(s))
                .unwrap_or_default();
            let name = self.title_of_id(target).unwrap_or_default();
            let text = fix
                .hint
                .clone()
                .unwrap_or_else(|| format!("\"{mention}\" in \"{source}\" = {name}"));
            set.hints.push(NewHint {
                id: HintId::generate(self.ids()),
                entity: target,
                text,
                source_decision: Some(d.id),
            });
        };
        // A pending suggestion: repoint = accept it pointing elsewhere; reject = reject it.
        if let Some(s) = pending {
            match (&fix.action, s.kind.as_str()) {
                (FixAction::Reject, _) => {
                    self.plan_reject(set, &s)?;
                }
                (FixAction::Repoint(target), KIND_ENTITY_LINK) => {
                    let edits = SuggestionEdits {
                        target_id: Some(target.as_ulid()),
                        ..SuggestionEdits::default()
                    };
                    self.plan_accept_entity_link(set, &s, &edits, false)?;
                    hint_for(set, *target);
                }
                _ => {
                    return Err(VaultError::invalid(
                        "this suggestion can only be accepted, edited or rejected",
                    ));
                }
            }
            set.revert_decisions.push(d.id);
            return Ok(());
        }
        let src = d.source_note_id.ok_or(VaultError::NotFound)?;
        match d.kind {
            DecisionKind::EntityMention | DecisionKind::Relation | DecisionKind::Concept => {
                let rel = parse_rel(d.rel_type.as_deref())
                    .ok_or_else(|| VaultError::invalid("the decision has no relation type"))?;
                let old: NoteId = d
                    .target_id
                    .parse()
                    .map_err(|_| VaultError::invalid("the decision target is not a note"))?;
                set.remove.push(EdgeRemove {
                    src,
                    rel,
                    dst: old,
                    reject: true,
                });
                match &fix.action {
                    FixAction::Reject => {}
                    FixAction::Repoint(new) => {
                        let want = self.live_kind(old);
                        let got = self.live_kind(*new).ok_or(VaultError::NotFound)?;
                        if want.is_some_and(|k| k != got) {
                            return Err(VaultError::invalid(
                                "the new target is of another kind than the old one",
                            ));
                        }
                        set.add.push(user_edge(src, rel, *new));
                        if d.kind == DecisionKind::EntityMention {
                            hint_for(set, *new);
                        }
                    }
                    FixAction::Retype(new_rel) => {
                        if *new_rel == rel {
                            return Err(VaultError::invalid("the new type equals the old type"));
                        }
                        set.add.push(user_edge(src, *new_rel, old));
                    }
                }
            }
            DecisionKind::CustodyEvent => {
                if fix.action != FixAction::Reject {
                    return Err(VaultError::invalid(
                        "a custody event can only be rejected; record the right one instead",
                    ));
                }
                let document: NoteId = d
                    .target_id
                    .parse()
                    .map_err(|_| VaultError::invalid("the decision target is not a note"))?;
                let kind: CustodyEventType = d
                    .rel_type
                    .as_deref()
                    .and_then(|t| t.parse().ok())
                    .ok_or_else(|| VaultError::invalid("the decision has no event type"))?;
                let detail: CustodyDetail = decode(&d.detail)?;
                set.custody_remove.push((document, detail.date, kind, src));
            }
            _ => {
                return Err(VaultError::invalid("this decision cannot be corrected"));
            }
        }
        set.revert_decisions.push(d.id);
        Ok(())
    }

    fn plan_reject(&self, set: &mut AiChangeSet, s: &Suggestion) -> Result<()> {
        if s.kind == KIND_ENTITY_LINK {
            let p: EntityLinkPayload = decode(&s.payload)?;
            set.rejected_mentions
                .push((p.source_note, p.mention.clone(), p.kind.clone()));
            if let (Some(target), Some(rel)) = (
                p.proposed,
                p.kind.parse::<NoteKind>().ok().and_then(entity_rel),
            ) {
                // Records the rejected link even though it was never written.
                set.remove.push(EdgeRemove {
                    src: p.source_note,
                    rel,
                    dst: target,
                    reject: true,
                });
            }
        }
        set.decide.push(DecideSuggestion {
            id: s.id,
            note: s.note_id,
            kind: s.kind.clone(),
            decision: SuggestionDecision::Rejected,
        });
        Ok(())
    }

    fn plan_accept_entity_link(
        &self,
        set: &mut AiChangeSet,
        s: &Suggestion,
        edits: &SuggestionEdits,
        add_alias: bool,
    ) -> Result<()> {
        let p: EntityLinkPayload = decode(&s.payload)?;
        let kind: NoteKind = p
            .kind
            .parse()
            .map_err(|_| VaultError::invalid("unknown entity kind"))?;
        let target = match edits.target_id.map(NoteId::from_ulid) {
            Some(t) => {
                if self.live_kind(t) != Some(kind) {
                    return Err(VaultError::invalid(
                        "target_id must be an existing entity of the mention's kind",
                    ));
                }
                t
            }
            None => match (p.proposed, p.candidates.as_slice()) {
                (Some(t), _) if self.live_kind(t) == Some(kind) => t,
                (_, [only]) if self.live_kind(*only) == Some(kind) => *only,
                (_, []) | (None, _) if p.candidates.is_empty() => {
                    let id = NoteId::generate(self.ids());
                    let name = edits
                        .title
                        .clone()
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or_else(|| p.mention.clone());
                    set.entities.push(NewEntityNote {
                        id,
                        kind,
                        name,
                        aliases: Vec::new(),
                    });
                    id
                }
                _ => {
                    return Err(VaultError::invalid(
                        "several entities match: choose one with target_id",
                    ));
                }
            },
        };
        if let Some(rel) = entity_rel(kind)
            && self.live_kind(p.source_note).is_some()
        {
            set.add.push(user_edge(p.source_note, rel, target));
        }
        let mut aliases: Vec<String> = Vec::new();
        if add_alias {
            aliases.push(p.mention.clone());
        }
        aliases.extend(edits.aliases.clone().unwrap_or_default());
        if !aliases.is_empty() {
            if let Some(e) = set.entities.iter_mut().find(|e| e.id == target) {
                e.aliases.extend(aliases);
            } else {
                set.aliases.push((target, aliases));
            }
        }
        set.decide.push(DecideSuggestion {
            id: s.id,
            note: s.note_id,
            kind: s.kind.clone(),
            decision: SuggestionDecision::Accepted,
        });
        Ok(())
    }

    fn resolve_custody_target(
        &self,
        t: &CustodyTarget,
        kind: NoteKind,
        edit: &mut Option<NoteId>,
        set: &mut AiChangeSet,
        create: bool,
    ) -> Result<NoteId> {
        if let Some(id) = t.id.filter(|i| self.live_kind(*i) == Some(kind)) {
            return Ok(id);
        }
        let live: Vec<NoteId> = t
            .candidates
            .iter()
            .copied()
            .filter(|i| self.live_kind(*i) == Some(kind))
            .collect();
        if live.len() == 1 {
            return Ok(live[0]);
        }
        if live.len() > 1 || (!create && t.candidates.is_empty()) {
            return match edit.take() {
                Some(e) if self.live_kind(e) == Some(kind) => Ok(e),
                _ => Err(VaultError::invalid(
                    "an entity of the event is ambiguous: choose it with target_id",
                )),
            };
        }
        if let Some(e) = edit.take().filter(|e| self.live_kind(*e) == Some(kind)) {
            return Ok(e);
        }
        let id = NoteId::generate(self.ids());
        set.entities.push(NewEntityNote {
            id,
            kind,
            name: t.mention.clone(),
            aliases: Vec::new(),
        });
        Ok(id)
    }

    fn plan_accept_custody(
        &self,
        set: &mut AiChangeSet,
        s: &Suggestion,
        edits: &SuggestionEdits,
    ) -> Result<()> {
        let p: CustodyPayload = decode(&s.payload)?;
        let kind: CustodyEventType = p
            .event
            .parse()
            .map_err(|_| VaultError::invalid("unknown custody event type"))?;
        let mut edit = edits.target_id.map(NoteId::from_ulid);
        let document = self.resolve_custody_target(&p.document, NoteKind::Document, &mut edit, set, true)?;
        let place = match &p.place {
            Some(t) => Some(self.resolve_custody_target(t, NoteKind::Place, &mut edit, set, true)?),
            None => None,
        };
        let person = match &p.person {
            Some(t) => Some(self.resolve_custody_target(t, NoteKind::Person, &mut edit, set, true)?),
            None => None,
        };
        let counterparty = match &p.counterparty {
            Some(t) => {
                let as_person = self.resolve_custody_target(t, NoteKind::Person, &mut edit, set, false);
                match as_person {
                    Ok(i) => Some(i),
                    Err(_) => Some(self.resolve_custody_target(
                        t,
                        NoteKind::Company,
                        &mut edit,
                        set,
                        true,
                    )?),
                }
            }
            None => None,
        };
        let cite = self.live_kind(p.source_note).map(|_| Cite {
            note: p.source_note,
            block: p.block_id.clone(),
        });
        set.custody.push(AiCustody {
            document,
            kind,
            date: p.date,
            place,
            person,
            counterparty,
            cite,
        });
        set.decide.push(DecideSuggestion {
            id: s.id,
            note: s.note_id,
            kind: s.kind.clone(),
            decision: SuggestionDecision::Accepted,
        });
        Ok(())
    }

    /// Accepts or rejects an AI suggestion (see the module docs).
    #[allow(clippy::too_many_lines)] // one match over the suggestion kinds
    pub(crate) async fn decide_ai(
        &mut self,
        scope: UserScope,
        s: Suggestion,
        accept: bool,
        edits: Option<SuggestionEdits>,
    ) -> Result<SuggestionView> {
        let edits = edits.unwrap_or_default();
        let id = s.id;
        let subject = s
            .note_id
            .filter(|n| self.live_kind(*n).is_some());
        let op = if accept { "accept" } else { "reject" };
        let mut set = AiChangeSet::new("suggestion", subject.unwrap_or(NoteId::from_ulid(ulid::Ulid::nil())))
            .by_user(&format!("{op} {}", s.kind));
        if !accept {
            self.plan_reject(&mut set, &s)?;
            return self.finish_decision(scope, set, subject, id).await;
        }
        match s.kind.as_str() {
            KIND_FILING => {
                let p: FilingPayload = decode(&s.payload)?;
                let note = subject.ok_or(VaultError::NotFound)?;
                let (_, version) = self.live(note)?;
                let linked = self
                    .sidecar(note)
                    .await?
                    .and_then(|sc| sc.last_linked_hash)
                    .is_some_and(|h| h == version);
                set.filing = Some(Filing {
                    title: edits.title.clone().unwrap_or(p.title),
                    tags: edits.tags.clone().unwrap_or(p.tags),
                    folder: edits.folder.clone().unwrap_or(p.folder),
                });
                set.mark_linked = linked;
                set.decide.push(DecideSuggestion {
                    id,
                    note: s.note_id,
                    kind: s.kind.clone(),
                    decision: SuggestionDecision::Accepted,
                });
            }
            KIND_ENTITY_LINK => {
                self.plan_accept_entity_link(&mut set, &s, &edits, true)?;
            }
            KIND_CUSTODY => {
                self.plan_accept_custody(&mut set, &s, &edits)?;
            }
            KIND_CORRECTION => {
                let p: CorrectionPayload = decode(&s.payload)?;
                for f in &p.fixes {
                    let action = match f.action.as_str() {
                        "repoint" => FixAction::Repoint(
                            edits
                                .target_id
                                .map(NoteId::from_ulid)
                                .or(f.new_target)
                                .ok_or_else(|| VaultError::invalid("the fix has no new target"))?,
                        ),
                        "retype" => FixAction::Retype(
                            parse_rel(f.new_type.as_deref())
                                .ok_or_else(|| VaultError::invalid("the fix has no valid type"))?,
                        ),
                        _ => FixAction::Reject,
                    };
                    let fix = DecisionFix {
                        decision: f.decision_id,
                        action,
                        hint: None,
                    };
                    self.plan_fix(&scope, &mut set, &fix).await?;
                }
                for h in &p.hints {
                    if self.live_kind(h.entity).is_some() {
                        set.hints.push(NewHint {
                            id: HintId::generate(self.ids()),
                            entity: h.entity,
                            text: h.text.clone(),
                            source_decision: Some(p.decision_id),
                        });
                    }
                }
                set.decide.push(DecideSuggestion {
                    id,
                    note: s.note_id,
                    kind: s.kind.clone(),
                    decision: SuggestionDecision::Accepted,
                });
            }
            KIND_TASK => {
                let p: TaskPayload = decode(&s.payload)?;
                let mut text = edits.text.clone().unwrap_or_else(|| p.title.clone());
                if edits.text.is_none() {
                    let state = self.state()?;
                    let index = state.path_index();
                    for e in &p.entities {
                        if let Some((path, _)) = state.note(*e) {
                            let link = format!("[[{}]]", index.link_text_for(path));
                            if !text.contains(&link) {
                                text.push(' ');
                                text.push_str(&link);
                            }
                        }
                    }
                }
                let req = NewTask {
                    text,
                    due: edits.due.or(p.due),
                    recurrence: edits.recurrence.clone().or(p.recurrence),
                    reminders: edits.reminders.clone().unwrap_or(p.reminders),
                    force: true,
                    ..NewTask::default()
                };
                let receipt = self.receipt.take();
                let created = self.create_task(scope, req).await;
                self.receipt = receipt;
                created?;
                set.decide.push(DecideSuggestion {
                    id,
                    note: s.note_id,
                    kind: s.kind.clone(),
                    decision: SuggestionDecision::Accepted,
                });
                return self.finish_decision(scope, set, None, id).await;
            }
            KIND_DUPLICATES => {
                let p: DuplicatesPayload = decode(&s.payload)?;
                let receipt = self.receipt.take();
                let merged = self.merge_duplicates(scope, &p).await;
                self.receipt = receipt;
                merged?;
                set.decide.push(DecideSuggestion {
                    id,
                    note: s.note_id,
                    kind: s.kind.clone(),
                    decision: SuggestionDecision::Accepted,
                });
                return self.finish_decision(scope, set, None, id).await;
            }
            _ => return Err(VaultError::invalid("unknown suggestion kind")),
        }
        self.finish_decision(scope, set, subject, id).await
    }

    /// Writes `set` (on `subject`, or database rows only) and returns the decided suggestion.
    async fn finish_decision(
        &mut self,
        scope: UserScope,
        mut set: AiChangeSet,
        subject: Option<NoteId>,
        id: SuggestionId,
    ) -> Result<SuggestionView> {
        let target_subject = subject.or_else(|| {
            // Another live note the change touches (an entity, a document) as the subject.
            set.add
                .first()
                .map(|a| a.src)
                .or_else(|| set.custody.first().map(|c| c.document))
                .or_else(|| set.aliases.first().map(|a| a.0))
                .filter(|n| self.live_kind(*n).is_some())
        });
        match target_subject {
            Some(n) => {
                set.subject = n;
                match self.ai_apply(scope, set).await? {
                    AiApplied::Done { .. } => {}
                    AiApplied::Stale => {
                        return Err(VaultError::invalid("the note changed; try again"));
                    }
                }
            }
            None => {
                self.rows_only(scope, set).await?;
            }
        }
        let mut tx = self.begin(&scope).await?;
        let s = srepo::get_suggestion(&mut tx, id)
            .await?
            .ok_or(VaultError::NotFound)?;
        let replies = srepo::replies(&mut tx, id).await?;
        tx.commit().await?;
        Ok(SuggestionView {
            suggestion: s,
            replies,
        })
    }

    /// Database rows of `set` without any file change (and with the pushed op's receipt).
    async fn rows_only(&mut self, scope: UserScope, set: AiChangeSet) -> Result<()> {
        let files_touched = !set.add.is_empty()
            || !set.remove.is_empty()
            || !set.entities.is_empty()
            || !set.custody.is_empty()
            || set.filing.is_some();
        if files_touched {
            return Err(VaultError::NotFound);
        }
        // Any live note works as the subject of a rows-only write; without one, nothing
        // references the vault and the rows are written directly.
        let anchor = self.state()?.by_id.keys().next().copied();
        match anchor {
            Some(n) => {
                let mut set = set;
                set.subject = n;
                set.expect_version = None;
                self.ai_apply(scope, set).await.map(|_| ())
            }
            None => Err(VaultError::NotFound),
        }
    }

    /// Merges a `duplicates` pair (older survives).
    async fn merge_duplicates(&mut self, scope: UserScope, p: &DuplicatesPayload) -> Result<()> {
        let kind = p.a.kind.as_str();
        if kind == "task" {
            // Keep the older task line; cancel the newer one.
            let mut ids = [p.a.item.clone(), p.b.item.clone()];
            ids.sort();
            return self
                .transition_task(scope, &ids[1], Transition::Cancel, None)
                .await
                .map(|_| ());
        }
        let a: NoteId = p.a.item.parse().map_err(|_| VaultError::NotFound)?;
        let b: NoteId = p.b.item.parse().map_err(|_| VaultError::NotFound)?;
        let (survivor, loser) = self.older_first(a, b).await?;
        match self.live_kind(survivor) {
            Some(NoteKind::Person | NoteKind::Company | NoteKind::Document | NoteKind::Place) => {
                self.merge_entities(scope, loser, survivor).await.map(|_| ())
            }
            Some(_) => self.merge_notes(scope, loser, survivor).await,
            None => Err(VaultError::NotFound),
        }
    }

    async fn older_first(&self, a: NoteId, b: NoteId) -> Result<(NoteId, NoteId)> {
        let created = |text: Option<String>| {
            text.and_then(|t| {
                Document::parse(&t)
                    .frontmatter()
                    .and_then(|f| f.created().ok().flatten())
            })
        };
        let (pa, _) = self.live(a)?;
        let (pb, _) = self.live(b)?;
        let ca = created(self.read_text(&pa).await?);
        let cb = created(self.read_text(&pb).await?);
        Ok(if (ca, a) <= (cb, b) { (a, b) } else { (b, a) })
    }

    /// Merges note `loser` into `survivor` (PLAN §9.7 duplicates), one commit:
    ///
    /// - the survivor keeps its path, ID, frontmatter and body;
    /// - its `aliases` gain the loser's title and aliases, its `tags` and relation keys the
    ///   loser's (links to either note of the pair are dropped);
    /// - unless the two bodies are equal (after trimming), the loser's body is appended under
    ///   `## Merged from <loser title> (<YYYY-MM-DD>)`;
    /// - every link and relation to the loser (bodies, frontmatter, sidecars) points at the
    ///   survivor;
    /// - the loser goes to the trash (restorable; the commit is revertible).
    #[allow(clippy::too_many_lines)] // one linear pass, like the entity merge
    pub async fn merge_notes(
        &mut self,
        scope: UserScope,
        loser: NoteId,
        survivor: NoteId,
    ) -> Result<()> {
        if loser == survivor {
            return Err(VaultError::invalid("a note cannot be merged into itself"));
        }
        let (lpath, _) = self.live(loser)?;
        let (spath, _) = self.live(survivor)?;
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let today = self.local_now(tz).format("%Y-%m-%d").to_string();
        let state = self.state()?;
        let index = state.path_index();
        let ltext = self.read_text(&lpath).await?.ok_or(VaultError::NotFound)?;
        let stext = self.read_text(&spath).await?.ok_or(VaultError::NotFound)?;
        let ldoc = Document::parse(&ltext);
        let mut sdoc = Document::parse(&stext);
        let ltitle = crate::derive::title_of(&lpath, &ldoc);
        let stitle = crate::derive::title_of(&spath, &sdoc);
        let points_home = |item: &str, from: &str| {
            vault_format::WikiLink::parse_exact(item.trim()).is_some_and(|l| {
                matches!(index.resolve(&l.path, Some(from)),
                    vault_format::Resolution::Resolved(p) if p == spath || p == lpath)
            })
        };
        {
            let lfm = ldoc.frontmatter();
            let fm = sdoc.frontmatter_mut();
            if fm.error().is_some() {
                return Err(VaultError::invalid("the survivor's frontmatter cannot be edited"));
            }
            let err = |_| VaultError::invalid("the survivor's frontmatter cannot be edited");
            let mut aliases = fm.aliases();
            for a in std::iter::once(ltitle.clone())
                .chain(lfm.map(vault_format::Frontmatter::aliases).unwrap_or_default())
            {
                if a != stitle && !aliases.contains(&a) {
                    aliases.push(a);
                }
            }
            fm.set_list(KnownKey::Aliases, aliases).map_err(err)?;
            let mut tags = fm.tags();
            for t in lfm.map(vault_format::Frontmatter::tags).unwrap_or_default() {
                if !tags.contains(&t) {
                    tags.push(t);
                }
            }
            if !tags.is_empty() {
                fm.set_list(KnownKey::Tags, tags).map_err(err)?;
            }
            if let Some(lfm) = lfm {
                for rk in RelationKey::all() {
                    let mut list = fm.relation(rk);
                    let before = list.len();
                    for item in lfm.relation(rk) {
                        if !points_home(&item, &lpath) && !list.contains(&item) {
                            list.push(item);
                        }
                    }
                    if list.len() != before {
                        fm.set_relation(rk, list).map_err(err)?;
                    }
                }
            }
        }
        if ldoc.body().trim() != sdoc.body().trim() && !ldoc.body().trim().is_empty() {
            let mut body = sdoc.body().to_owned();
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&format!(
                "## Merged from {ltitle} ({today})\n\n{}\n",
                ldoc.body().trim_end()
            ));
            sdoc.set_body(body);
        }
        let mut changes: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        changes.insert(spath.clone(), Some(sdoc.render().into_bytes()));
        let survivor_link = index.link_text_for(&spath);
        let names: BTreeSet<String> = [crate::state::name_key(&lpath)].into();
        let sources: Vec<String> = state
            .linking_to(&names)
            .into_iter()
            .filter_map(|i| state.note(i).map(|(p, _)| p.to_owned()))
            .filter(|p| *p != lpath)
            .collect();
        for p in sources {
            let text = match changes.get(&p) {
                Some(Some(b)) => String::from_utf8_lossy(b).into_owned(),
                _ => self.read_text(&p).await?.unwrap_or_default(),
            };
            let mut d = Document::parse(&text);
            if crate::ops::entities::retarget_links(&mut d, &p, &lpath, &spath, &survivor_link, &index)
            {
                changes.insert(p, Some(d.render().into_bytes()));
            }
        }
        let dir = self.dir.clone();
        let sidecar_files = crate::store::blocking(move || {
            let mut out = Vec::new();
            for f in crate::fsio::scan(&dir)? {
                if f.starts_with(".meta/notes/")
                    && let Some(b) = crate::fsio::read(&dir, &f)?
                {
                    out.push((f, String::from_utf8_lossy(&b).into_owned()));
                }
            }
            Ok(out)
        })
        .await?;
        for (f, text) in sidecar_files {
            let Ok(mut sc) = NoteSidecar::from_json(&text) else {
                continue;
            };
            if sc.id == loser.as_ulid() {
                continue;
            }
            let mut touched = false;
            for r in &mut sc.relations {
                if r.target_id == loser.as_ulid() {
                    r.target_id = survivor.as_ulid();
                    touched = true;
                }
            }
            for r in &mut sc.rejected {
                if r.target_id == loser.as_ulid() {
                    r.target_id = survivor.as_ulid();
                    touched = true;
                }
            }
            if touched {
                let (_, content) = Core::sidecar_change(&sc)?;
                changes.insert(f, content);
            }
        }
        let trash = paths::trash_path(&lpath);
        let trash = {
            let st = self.state()?;
            let folder = paths::parent(&trash);
            let stem = paths::file_name(&trash).trim_end_matches(".md");
            let taken: Vec<&str> = st
                .trash
                .keys()
                .filter(|p| paths::parent(p) == folder)
                .map(|p| paths::file_name(p).trim_end_matches(".md"))
                .collect();
            paths::join(
                folder,
                &format!("{}.md", vault_format::filename::unique_name(stem, taken)),
            )
        };
        changes.insert(lpath.clone(), None);
        changes.insert(trash, Some(ltext.into_bytes()));
        self.finish(
            tx,
            changes.into_iter().collect(),
            Author::User.message("merge", &format!("{lpath} -> {spath}")),
        )
        .await?;
        Ok(())
    }
}

impl VaultService {
    /// Accepts (with optional `edits`) or rejects a pending suggestion. Kinds without edit
    /// support refuse non-empty edits.
    pub async fn decide_suggestion_with(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        accept: bool,
        edits: Option<SuggestionEdits>,
    ) -> Result<SuggestionView> {
        let edits = edits.filter(|e| *e != SuggestionEdits::default());
        if edits.is_none() {
            return self.decide_suggestion(scope, id, accept).await;
        }
        self.exec(scope, move |core, scope| {
            Box::pin(async move {
                let mut tx = core.begin(&scope).await?;
                let s = srepo::get_suggestion(&mut tx, id)
                    .await?
                    .ok_or(VaultError::NotFound)?;
                tx.commit().await?;
                if s.status != SuggestionStatus::Pending {
                    return Err(VaultError::invalid("the suggestion was already decided"));
                }
                if !accept || !handles(&s.kind, true) || s.kind == KIND_DUPLICATES {
                    return Err(VaultError::invalid(
                        "edits are not supported for this suggestion kind",
                    ));
                }
                core.decide_ai(scope, s, true, edits).await
            })
        })
        .await
    }

    /// Corrects applied AI decisions (or pending AI suggestions) in one `user: correct`
    /// commit: repoint, retype or reject, with hints for repointed entity mentions (§9.8).
    pub async fn correct_decisions(
        &self,
        scope: &UserScope,
        fixes: Vec<DecisionFix>,
        extra_hints: Vec<(NoteId, String)>,
    ) -> Result<AiApplied> {
        self.exec(scope, move |core, scope| {
            Box::pin(async move {
                let first = fixes.first().ok_or_else(|| VaultError::invalid("nothing to correct"))?;
                let d = core.decision_row(&scope, first.decision).await?;
                let subject = d
                    .source_note_id
                    .filter(|n| core.live_kind(*n).is_some())
                    .ok_or(VaultError::NotFound)?;
                let op = match first.action {
                    FixAction::Repoint(_) => "repoint",
                    FixAction::Retype(_) => "retype",
                    FixAction::Reject => "reject",
                };
                let mut set = AiChangeSet::new("correct", subject).by_user(op);
                for f in &fixes {
                    core.plan_fix(&scope, &mut set, f).await?;
                }
                for (entity, text) in extra_hints {
                    if core.live_kind(entity).is_some() {
                        set.hints.push(NewHint {
                            id: HintId::generate(core.ids()),
                            entity,
                            text,
                            source_decision: Some(first.decision),
                        });
                    }
                }
                core.ai_apply(scope, set).await
            })
        })
        .await
    }

    /// Applies an AI correction (auto-applied, §9.8) in one `ai: correct` commit.
    pub async fn ai_correct(
        &self,
        scope: &UserScope,
        mut set: AiChangeSet,
        fixes: Vec<DecisionFix>,
    ) -> Result<AiApplied> {
        self.exec(scope, move |core, scope| {
            Box::pin(async move {
                for f in &fixes {
                    core.plan_fix(&scope, &mut set, f).await?;
                }
                core.ai_apply(scope, set).await
            })
        })
        .await
    }

    /// The most recent AI decisions, newest first (activity feed, correction context).
    pub async fn ai_decisions(&self, scope: &UserScope, limit: i64) -> Result<Vec<DecisionView>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let rows: Vec<DecisionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {DECISION_COLS} FROM ai_decisions ORDER BY created DESC, id DESC LIMIT $1"
        )))
        .bind(limit.clamp(1, 500))
        .fetch_all(tx.conn())
        .await?;
        self.decision_views(&mut tx, rows).await
    }

    /// One decision.
    pub async fn ai_decision(&self, scope: &UserScope, id: DecisionId) -> Result<DecisionView> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let row: Option<DecisionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {DECISION_COLS} FROM ai_decisions WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(tx.conn())
        .await?;
        let row = row.ok_or(VaultError::NotFound)?;
        self.decision_views(&mut tx, vec![row])
            .await?
            .pop()
            .ok_or(VaultError::NotFound)
    }

    /// The decision a suggestion came from.
    pub async fn decision_of_suggestion(
        &self,
        scope: &UserScope,
        suggestion: SuggestionId,
    ) -> Result<Option<DecisionRow>> {
        let mut tx = self.inner.db.begin(scope).await?;
        let row: Option<DecisionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {DECISION_COLS} FROM ai_decisions WHERE suggestion_id = $1 \
             ORDER BY created DESC, id DESC LIMIT 1"
        )))
        .bind(suggestion)
        .fetch_optional(tx.conn())
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    async fn decision_views(
        &self,
        tx: &mut strata_index::ScopedTx,
        rows: Vec<DecisionRow>,
    ) -> Result<Vec<DecisionView>> {
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let source = match row.source_note_id {
                Some(n) => strata_index::repo::notes::get_note(tx, n).await?,
                None => None,
            };
            let target = match row.target_id.parse::<NoteId>() {
                Ok(n) => strata_index::repo::notes::get_note(tx, n).await?,
                Err(_) => None,
            };
            let status = match row.suggestion_id {
                Some(s) => srepo::get_suggestion(tx, s).await?.map(|s| s.status),
                None => None,
            };
            out.push(DecisionView {
                source_title: source.as_ref().map(|n| n.title.clone()),
                source_created: source.as_ref().map(|n| n.created),
                target_name: target.map(|n| n.title),
                suggestion_status: status,
                row,
            });
        }
        Ok(out)
    }

    /// Enqueues the `suggestion_reply` job for `suggestion` (debounced per suggestion).
    pub(crate) async fn enqueue_reply_job(
        tx: &mut strata_index::ScopedTx,
        ids: &dyn strata_common::IdGenerator,
        s: &Suggestion,
        now: DateTime<Utc>,
    ) -> Result<()> {
        if !replies_reach_ai(&s.kind) {
            return Ok(());
        }
        strata_index::repo::jobs::enqueue(
            tx,
            &NewJob {
                id: JobId::generate(ids),
                kind: SUGGESTION_REPLY_JOB.to_owned(),
                note_id: s.note_id,
                payload: encode(&s.id)?,
                run_after: now,
                max_attempts: 5,
                dedupe_key: Some(s.id.to_string()),
            },
            now,
        )
        .await?;
        Ok(())
    }
}

/// The relation key of a note-relation type.
pub fn note_rel(t: RelationType) -> RelationKey {
    RelationKey::Note(t)
}
