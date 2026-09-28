//! One AI job, one `ai:` commit (PLAN §7.2, §9.2–§9.4): the vault half of the AI pipelines.
//!
//! A job (linking, inbox filing, entity insights, a correction) decides *what* to write and
//! hands the vault an [`AiChangeSet`]; [`Core::ai_apply`] writes all of it — across every note
//! it touches — as **one** commit, and records the job's AI decisions, suggestions,
//! disambiguation hints and follow-up jobs in the **same** database transaction as the index
//! update. Nothing here edits the user's prose (principle 3):
//!
//! - relation keys in frontmatter (with provenance in the sidecar, `by: ai`), and removal of
//!   the job's own stale `by: ai` edges only (user edges are never removed);
//! - block IDs appended to cited blocks (the one automated body edit, §6.3);
//! - concept notes (`concepts/<Name>.md`, `kind: concept`, AI-owned `## Summary`);
//! - the AI-owned sections of entity pages (Summary, Insights, Open items, Timeline), through
//!   `vault-format`'s section replacement, which validates citations and never touches user
//!   sections;
//! - custody lines of documents (cited) and the frontmatter derived from them (§6.12);
//! - title, tags and folder of an inbox capture when auto-file is on (§9.3), with every inbound
//!   link rewritten in the same commit.
//!
//! [`Core::ai_apply`] is also used for user decisions that act on AI proposals (accepting a
//! suggestion, repointing an AI link), with [`Author::User`].

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, Utc};
use domain::NoteKind;
use serde::{Deserialize, Serialize};
use strata_common::{DecisionId, HintId, JobId, NoteId, ReplyId, SuggestionId};
use strata_index::repo::entities as erepo;
use strata_index::repo::jobs::{self, NewJob};
use strata_index::repo::suggestions as srepo;
use strata_index::repo::sync::{self as synclog, NewChange};
use strata_index::types::{ChangeOp, DecisionKind};
use strata_index::{ScopedTx, UserScope};
use vault_format::body::{self, BlockKind};
use vault_format::canvas::Canvas;
use vault_format::custody::CustodyEventType;
use vault_format::filename::{sanitize_file_name, unique_name};
use vault_format::frontmatter::KnownKey;
use vault_format::sections::{self, AiProfile, AiSection};
use vault_format::sidecar::{By, NoteSidecar, SidecarRelation};
use vault_format::{Document, PathIndex, RelationKey, rewrite::MoveSet};

use crate::error::{Result, VaultError};
use crate::git::FileChange;
use crate::ops::relations::{AiEdge, add_link, remove_links};
use crate::paths;
use crate::prepare;
use crate::state::name_key;
use crate::store::{Author, Core, VaultService};

/// Sidecar extension key: disambiguation hints about this entity (§9.8 memory), mirrored in
/// the `disambiguation_hints` table.
pub const HINTS_KEY: &str = "hints";
/// Sidecar extension key: mention texts the user refused to link from this note (§6.7).
pub const REJECTED_MENTIONS_KEY: &str = "rejected_mentions";
/// Sidecar extension key: the input hash of the last entity-insights run (skip if unchanged).
pub const INSIGHTS_HASH_KEY: &str = "insights_hash";
/// Sidecar extension key: the version of an inbox capture that was filed (proposal made).
pub const FILED_KEY: &str = "filed";

/// A citation of a note (and block).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cite {
    /// Cited note.
    pub note: NoteId,
    /// Cited block (`None`: the note as a whole).
    pub block: Option<String>,
}

/// Makes a block citable: appends `id` to the block of `note` whose text (without an ID) is
/// `block_text`, unless it already has an ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIdRequest {
    /// Note.
    pub note: NoteId,
    /// The block's text as given to the model.
    pub block_text: String,
    /// The ID to append (`[a-z0-9-]+`).
    pub id: String,
}

/// An AI edge to add on `src`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeAdd {
    /// Source note.
    pub src: NoteId,
    /// The edge (type, target, provenance).
    pub edge: AiEdge,
    /// Who adds it: `Ai` (provenance in the sidecar) or `User` (an accepted suggestion or a
    /// correction: no AI provenance, so it is never removed as stale).
    pub by: By,
}

/// An edge to remove from `src` — only when the sidecar says it was added `by: ai` (stale AI
/// edges); with `reject`, the removal is recorded as a rejection so the AI never re-adds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRemove {
    /// Source note.
    pub src: NoteId,
    /// Relation.
    pub rel: RelationKey,
    /// Target.
    pub dst: NoteId,
    /// Record as rejected (a user correction), and remove even a user edge.
    pub reject: bool,
}

/// A concept note to create (`concepts/<name>.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewConcept {
    /// Its ID (edges in the same change set may point at it).
    pub id: NoteId,
    /// Name (file name, sanitised).
    pub name: String,
    /// The AI-owned `## Summary`.
    pub summary: Option<String>,
}

/// An entity note to create (accepting a link-or-create suggestion; D13 = b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntityNote {
    /// Its ID.
    pub id: NoteId,
    /// Kind (person, company, document, place).
    pub kind: NoteKind,
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
}

/// A custody event to record on a document (§6.12), cited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCustody {
    /// Document.
    pub document: NoteId,
    /// Event type.
    pub kind: CustodyEventType,
    /// Resolved date.
    pub date: NaiveDate,
    /// Place.
    pub place: Option<NoteId>,
    /// Person.
    pub person: Option<NoteId>,
    /// Third party.
    pub counterparty: Option<NoteId>,
    /// The note (and block) stating it; `None` only for user-recorded events.
    pub cite: Option<Cite>,
}

/// One AI bullet: optional leading date (Timeline), text, citations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBullet {
    /// Date (Timeline entries).
    pub date: Option<NaiveDate>,
    /// One line.
    pub text: String,
    /// Citations (at least one for AI content).
    pub cites: Vec<Cite>,
}

/// New content for the AI-owned sections of one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSections {
    /// The note (entity, document or concept).
    pub note: NoteId,
    /// Its section profile.
    pub profile: AiProfile,
    /// `## Summary` text.
    pub summary: Option<String>,
    /// Bullet sections (Insights, Open items, Timeline).
    pub bullets: Vec<(AiSection, Vec<AiBullet>)>,
}

/// A note in an AI-owned folder (`_ai/`) written whole by a job (the weekly digest, §9.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiNoteWrite {
    /// Its ID (the existing note's ID when the path exists).
    pub id: NoteId,
    /// Path under `_ai/`.
    pub path: String,
    /// Display title (frontmatter `title`).
    pub title: String,
    /// The body.
    pub body: String,
}

/// Title, tags and folder for an inbox capture (§9.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filing {
    /// Title (file name; sanitised).
    pub title: String,
    /// Tags to add.
    pub tags: Vec<String>,
    /// Destination folder.
    pub folder: String,
}

/// An AI decision to record (§9.8: every decision has an ID and a source).
#[derive(Debug, Clone, PartialEq)]
pub struct NewDecision {
    /// ID.
    pub id: DecisionId,
    /// Kind.
    pub kind: DecisionKind,
    /// Source note.
    pub source_note: Option<NoteId>,
    /// Source block.
    pub source_block: Option<String>,
    /// `note`, `entity`, `document`, `concept`, `task`, `folder`, …
    pub target_type: String,
    /// Target (note ID, folder, task title…).
    pub target_id: String,
    /// One line for the activity feed.
    pub summary: String,
    /// Confidence.
    pub confidence: Option<f32>,
    /// Relation key or custody type.
    pub rel_type: Option<String>,
    /// Mention text.
    pub mention: Option<String>,
    /// `MessagePack` context.
    pub detail: Vec<u8>,
    /// The suggestion it became (not applied automatically).
    pub suggestion: Option<SuggestionId>,
}

/// A suggestion to create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSuggestion {
    /// ID.
    pub id: SuggestionId,
    /// Note it concerns.
    pub note: Option<NoteId>,
    /// Kind (`filing`, `entity_link`, `custody`, `task`, `correction`, `duplicates`).
    pub kind: String,
    /// `MessagePack` payload.
    pub payload: Vec<u8>,
}

/// A disambiguation hint to store (table + the entity's sidecar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewHint {
    /// ID.
    pub id: HintId,
    /// Entity.
    pub entity: NoteId,
    /// Text, e.g. "Ahmed at Acme = Ahmed Samir".
    pub text: String,
    /// The correction decision it came from.
    pub source_decision: Option<DecisionId>,
}

/// A hint as mirrored in the entity's sidecar (`hints`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarHint {
    /// Hint ID.
    pub id: String,
    /// Text.
    pub text: String,
    /// When.
    pub at: DateTime<Utc>,
}

/// A rejected mention as recorded in the source note's sidecar (`rejected_mentions`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedMention {
    /// Mention text.
    pub text: String,
    /// Entity kind it was proposed as.
    pub kind: String,
    /// When.
    pub at: DateTime<Utc>,
}

/// A status change of an existing suggestion made by the same write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecideSuggestion {
    /// Suggestion.
    pub id: SuggestionId,
    /// Its note.
    pub note: Option<NoteId>,
    /// Its kind.
    pub kind: String,
    /// New status.
    pub decision: SuggestionDecision,
}

/// Status changes of existing suggestions made by the same write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionDecision {
    /// Accepted.
    Accepted,
    /// Rejected.
    Rejected,
    /// Replaced by a re-proposal.
    Superseded,
}

/// Everything one AI job (or one user decision on AI output) writes.
#[derive(Debug, Clone)]
pub struct AiChangeSet {
    /// Commit label: `link`, `file_inbox`, `entity_insights`, `correct`, … (`ai: <job> <path>`).
    pub job: String,
    /// Commit author (AI jobs: `Author::Ai(job)`).
    pub author: Author,
    /// Operation for user commits (`accept`, `repoint`, …).
    pub op: String,
    /// The job row (recorded on decisions).
    pub job_id: Option<JobId>,
    /// The note the commit is about.
    pub subject: NoteId,
    /// Skip everything when the subject is no longer at this version.
    pub expect_version: Option<String>,
    /// Store the subject's version after this write as `last_linked_hash`.
    pub mark_linked: bool,
    /// Store the subject's version after this write as `filed` (inbox filing done).
    pub mark_filed: bool,
    /// Block IDs to append.
    pub block_ids: Vec<BlockIdRequest>,
    /// Edges to add.
    pub add: Vec<EdgeAdd>,
    /// Edges to remove.
    pub remove: Vec<EdgeRemove>,
    /// Concept notes to create.
    pub concepts: Vec<NewConcept>,
    /// Entity notes to create.
    pub entities: Vec<NewEntityNote>,
    /// Notes in `_ai/` to create or replace.
    pub ai_notes: Vec<AiNoteWrite>,
    /// Aliases to add to entities.
    pub aliases: Vec<(NoteId, Vec<String>)>,
    /// Custody events to record.
    pub custody: Vec<AiCustody>,
    /// Custody lines to remove (a rejected AI event): document, date, type, cited note.
    pub custody_remove: Vec<(NoteId, NaiveDate, CustodyEventType, NoteId)>,
    /// AI sections to replace.
    pub sections: Vec<AiSections>,
    /// Sidecar extension values to set.
    pub sidecar_extra: Vec<(NoteId, String, serde_json::Value)>,
    /// Mentions to record as rejected on a note.
    pub rejected_mentions: Vec<(NoteId, String, String)>,
    /// Title/tags/folder for the subject.
    pub filing: Option<Filing>,
    /// Decisions to record.
    pub decisions: Vec<NewDecision>,
    /// Decisions to mark reverted (repointed or rejected).
    pub revert_decisions: Vec<DecisionId>,
    /// Suggestions to create.
    pub suggestions: Vec<NewSuggestion>,
    /// Existing suggestions decided by this write.
    pub decide: Vec<DecideSuggestion>,
    /// Hints to store.
    pub hints: Vec<NewHint>,
    /// AI replies to add to suggestion threads (§9.8).
    pub ai_replies: Vec<(SuggestionId, ReplyId, String)>,
    /// Jobs to enqueue.
    pub jobs: Vec<NewJob>,
}

impl AiChangeSet {
    /// An empty change set of AI job `job` about `subject`.
    pub fn new(job: &str, subject: NoteId) -> Self {
        Self {
            job: job.to_owned(),
            author: Author::Ai(job.to_owned()),
            op: job.to_owned(),
            job_id: None,
            subject,
            expect_version: None,
            mark_linked: false,
            mark_filed: false,
            block_ids: Vec::new(),
            add: Vec::new(),
            remove: Vec::new(),
            concepts: Vec::new(),
            entities: Vec::new(),
            ai_notes: Vec::new(),
            aliases: Vec::new(),
            custody: Vec::new(),
            custody_remove: Vec::new(),
            sections: Vec::new(),
            sidecar_extra: Vec::new(),
            rejected_mentions: Vec::new(),
            filing: None,
            decisions: Vec::new(),
            revert_decisions: Vec::new(),
            suggestions: Vec::new(),
            decide: Vec::new(),
            hints: Vec::new(),
            ai_replies: Vec::new(),
            jobs: Vec::new(),
        }
    }

    /// The same, as a user decision (`user: <op> <path>`).
    #[must_use]
    pub fn by_user(mut self, op: &str) -> Self {
        self.author = Author::User;
        op.clone_into(&mut self.op);
        self
    }
}

/// What [`VaultService::ai_apply`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiApplied {
    /// Written (commit `None` when only database rows changed).
    Done {
        /// The commit.
        commit: Option<String>,
        /// The subject's path after the write.
        path: String,
        /// The subject's version after the write.
        version: String,
    },
    /// The subject changed (or is gone) since the job read it: nothing was written.
    Stale,
}

/// Files and sidecars as they will be after the write.
#[derive(Debug, Default)]
pub(crate) struct Overlay {
    texts: BTreeMap<String, String>,
    original: BTreeMap<String, Option<String>>,
    sidecars: BTreeMap<NoteId, NoteSidecar>,
    sidecar_original: BTreeMap<NoteId, Option<NoteSidecar>>,
    created: BTreeMap<NoteId, String>,
    moved: BTreeMap<NoteId, String>,
    removed: BTreeSet<String>,
}

fn frontmatter_err(_: vault_format::FrontmatterError) -> VaultError {
    VaultError::invalid("the frontmatter cannot be edited")
}

impl Core {
    /// The note's path in the overlay (created, moved or live).
    pub(crate) fn ov_path(&self, ov: &Overlay, id: NoteId) -> Option<String> {
        if let Some(p) = ov.moved.get(&id).or_else(|| ov.created.get(&id)) {
            return Some(p.clone());
        }
        self.state()
            .ok()?
            .note(id)
            .map(|(p, _)| p.to_owned())
            .filter(|p| !ov.removed.contains(p))
    }

    /// A path index over the overlay's files.
    pub(crate) fn ov_index(&self, ov: &Overlay) -> Result<PathIndex> {
        let state = self.state()?;
        let mut paths: BTreeSet<String> = state
            .notes
            .keys()
            .chain(state.attachments.iter())
            .cloned()
            .collect();
        for r in &ov.removed {
            paths.remove(r);
        }
        paths.extend(ov.created.values().cloned());
        paths.extend(ov.moved.values().cloned());
        Ok(PathIndex::new(paths))
    }

    /// The text of `path` in the overlay.
    pub(crate) async fn ov_text(&self, ov: &mut Overlay, path: &str) -> Result<String> {
        if let Some(t) = ov.texts.get(path) {
            return Ok(t.clone());
        }
        let text = self.read_text(path).await?;
        ov.original.entry(path.to_owned()).or_insert(text.clone());
        text.ok_or(VaultError::NotFound)
    }

    /// Sets the text of `path` in the overlay.
    pub(crate) fn ov_put(ov: &mut Overlay, path: &str, text: String) {
        ov.texts.insert(path.to_owned(), text);
    }

    /// The sidecar of `id` in the overlay (created empty when missing).
    pub(crate) async fn ov_sidecar<'o>(
        &self,
        ov: &'o mut Overlay,
        id: NoteId,
    ) -> Result<&'o mut NoteSidecar> {
        if !ov.sidecars.contains_key(&id) {
            let sc = self.sidecar(id).await?;
            ov.sidecar_original.insert(id, sc.clone());
            ov.sidecars
                .insert(id, sc.unwrap_or_else(|| NoteSidecar::new(id.as_ulid())));
        }
        ov.sidecars
            .get_mut(&id)
            .ok_or_else(|| VaultError::Internal("sidecar overlay".into()))
    }

    /// The file changes of the overlay.
    pub(crate) fn ov_changes(ov: &Overlay) -> Result<Vec<FileChange>> {
        let mut out: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        for r in &ov.removed {
            out.insert(r.clone(), None);
        }
        for (p, t) in &ov.texts {
            if ov.original.get(p).and_then(Option::as_deref) != Some(t.as_str()) {
                out.insert(p.clone(), Some(t.clone().into_bytes()));
            }
        }
        for (id, sc) in &ov.sidecars {
            let before = ov.sidecar_original.get(id).cloned().flatten();
            let unchanged = before.as_ref() == Some(sc)
                || (before.is_none() && Core::sidecar_change(sc)?.1.is_none());
            if !unchanged {
                let (path, content) = Core::sidecar_change(sc)?;
                out.insert(path, content);
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Moves `id` to `new_path` in the overlay, rewriting every link to it (in live notes,
    /// in overlay files and in canvases) — the overlay form of [`Core::plan_move`].
    pub(crate) async fn ov_move(&self, ov: &mut Overlay, id: NoteId, new_path: &str) -> Result<()> {
        let old = self.ov_path(ov, id).ok_or(VaultError::NotFound)?;
        if old == new_path {
            return Ok(());
        }
        let before = self.ov_index(ov)?;
        let after = PathIndex::new(before.paths().iter().map(|p| {
            if *p == old {
                new_path.to_owned()
            } else {
                p.clone()
            }
        }));
        let moves = MoveSet::new(&before, &after, [(old.clone(), new_path.to_owned())]);
        let names: BTreeSet<String> = [name_key(&old), name_key(new_path)].into();
        let state = self.state()?;
        let mut sources: BTreeSet<String> = state
            .linking_to(&names)
            .into_iter()
            .filter_map(|i| self.ov_path(ov, i))
            .collect();
        sources.extend(ov.texts.keys().filter(|p| p.ends_with(".md")).cloned());
        sources.remove(&old);
        let canvases: Vec<String> = state
            .attachments
            .iter()
            .filter(|p| p.ends_with(".canvas"))
            .cloned()
            .collect();
        let text = self.ov_text(ov, &old).await?;
        let mut doc = Document::parse(&text);
        doc.rewrite_links(&moves, &old).map_err(frontmatter_err)?;
        ov.texts.remove(&old);
        ov.removed.insert(old.clone());
        ov.original.entry(new_path.to_owned()).or_insert(None);
        Self::ov_put(ov, new_path, doc.render());
        ov.moved.insert(id, new_path.to_owned());
        for p in sources {
            let Ok(t) = self.ov_text(ov, &p).await else {
                continue;
            };
            let mut d = Document::parse(&t);
            if d.rewrite_links(&moves, &p).unwrap_or(0) > 0 {
                Self::ov_put(ov, &p, d.render());
            }
        }
        for c in canvases {
            let Ok(t) = self.ov_text(ov, &c).await else {
                continue;
            };
            if let Ok(mut canvas) = Canvas::from_json(&t)
                && canvas.rename_file(&old, new_path) > 0
            {
                Self::ov_put(ov, &c, canvas.to_json());
            }
        }
        Ok(())
    }

    /// `[[Link#^block]]` for a citation.
    fn cite_link(&self, ov: &Overlay, index: &PathIndex, cite: &Cite) -> Result<String> {
        let path = self.ov_path(ov, cite.note).ok_or(VaultError::NotFound)?;
        let text = index.link_text_for(&path);
        Ok(match &cite.block {
            Some(b) => format!("[[{text}#^{b}]]"),
            None => format!("[[{text}]]"),
        })
    }

    /// A free path `<folder>/<name>.md` in the overlay.
    fn ov_free_path(
        &self,
        ov: &Overlay,
        folder: &str,
        name: &str,
        own: Option<&str>,
    ) -> Result<String> {
        let stem = sanitize_file_name(name);
        let state = self.state()?;
        let mut taken: BTreeSet<String> = state
            .notes
            .keys()
            .chain(ov.created.values())
            .chain(ov.moved.values())
            .filter(|p| Some(p.as_str()) != own && !ov.removed.contains(*p))
            .filter(|p| paths::parent(p) == folder)
            .filter_map(|p| paths::file_name(p).strip_suffix(".md"))
            .map(str::to_owned)
            .collect();
        taken.extend(
            state
                .trash
                .keys()
                .filter_map(|p| paths::untrash_path(p))
                .filter(|p| paths::parent(p) == folder)
                .filter_map(|p| paths::file_name(p).strip_suffix(".md"))
                .map(str::to_owned),
        );
        let name = unique_name(&stem, taken.iter().map(String::as_str));
        Ok(paths::join(folder, &format!("{name}.md")))
    }

    /// Appends the requested block IDs (grouped per note).
    async fn ov_block_ids(&self, ov: &mut Overlay, reqs: &[BlockIdRequest]) -> Result<()> {
        let mut by_note: BTreeMap<NoteId, Vec<&BlockIdRequest>> = BTreeMap::new();
        for r in reqs {
            by_note.entry(r.note).or_default().push(r);
        }
        for (note, list) in by_note {
            let Some(path) = self.ov_path(ov, note) else {
                continue;
            };
            let text = self.ov_text(ov, &path).await?;
            let mut doc = Document::parse(&text);
            let mut body_text = doc.body().to_owned();
            for r in list {
                if !vault_format::blocks::is_valid_block_id(&r.id) {
                    continue;
                }
                let analysis = body::analyze(&body_text);
                if analysis.block_by_id(&r.id).is_some() {
                    continue;
                }
                let found = analysis.blocks.iter().find(|b| {
                    b.id.is_none()
                        && b.kind != BlockKind::Heading
                        && body_text
                            .get(b.span.clone())
                            .is_some_and(|t| t.trim_end() == r.block_text.trim_end())
                });
                let Some(block) = found else { continue };
                if let Ok(a) = vault_format::blocks::append_to(&body_text, block, &r.id) {
                    body_text = a.body;
                }
            }
            if body_text != doc.body() {
                doc.set_body(body_text);
                Self::ov_put(ov, &path, doc.render());
            }
        }
        Ok(())
    }

    /// Adds an edge in the overlay. Returns whether the frontmatter changed.
    async fn ov_add_edge(&self, ov: &mut Overlay, add: &EdgeAdd) -> Result<bool> {
        let e = &add.edge;
        if e.dst == add.src {
            return Ok(false);
        }
        let (Some(src_path), Some(dst_path)) = (self.ov_path(ov, add.src), self.ov_path(ov, e.dst))
        else {
            return Ok(false);
        };
        let now = self.now().fixed_offset();
        {
            let sc = self.ov_sidecar(ov, add.src).await?;
            if add.by == By::Ai && sc.is_blocked(e.rel, e.dst.as_ulid()) {
                return Ok(false);
            }
        }
        let index = self.ov_index(ov)?;
        let text = self.ov_text(ov, &src_path).await?;
        let mut doc = Document::parse(&text);
        let added = add_link(&mut doc, e.rel, &dst_path, &index, &src_path)?;
        if added {
            Self::ov_put(ov, &src_path, doc.render());
        }
        let sc = self.ov_sidecar(ov, add.src).await?;
        if add.by == By::Ai {
            // An existing user edge keeps its (absent) AI provenance.
            let had_user_edge = !added
                && !sc
                    .relations
                    .iter()
                    .any(|r| r.kind == e.rel && r.target_id == e.dst.as_ulid());
            if had_user_edge {
                return Ok(false);
            }
            sc.relations
                .retain(|r| !(r.kind == e.rel && r.target_id == e.dst.as_ulid()));
            sc.relations.push(SidecarRelation {
                kind: e.rel,
                target_id: e.dst.as_ulid(),
                by: By::Ai,
                confidence: Some(e.confidence),
                reason: Some(e.reason.clone()),
                model: Some(e.model.clone()),
                created: now,
            });
        } else {
            sc.relations
                .retain(|r| !(r.kind == e.rel && r.target_id == e.dst.as_ulid()));
            sc.rejected
                .retain(|r| !(r.kind == e.rel && r.target_id == e.dst.as_ulid()));
        }
        Ok(added)
    }

    /// Removes an edge in the overlay (see [`EdgeRemove`]). Returns whether it was removed.
    async fn ov_remove_edge(&self, ov: &mut Overlay, rm: &EdgeRemove) -> Result<bool> {
        let (Some(src_path), Some(dst_path)) = (self.ov_path(ov, rm.src), self.ov_path(ov, rm.dst))
        else {
            return Ok(false);
        };
        let at = self.now().fixed_offset();
        let is_ai = {
            let sc = self.ov_sidecar(ov, rm.src).await?;
            sc.relations
                .iter()
                .any(|r| r.kind == rm.rel && r.target_id == rm.dst.as_ulid() && r.by == By::Ai)
        };
        if !is_ai && !rm.reject {
            return Ok(false);
        }
        let index = self.ov_index(ov)?;
        let text = self.ov_text(ov, &src_path).await?;
        let mut doc = Document::parse(&text);
        let n = remove_links(&mut doc, rm.rel, &dst_path, &index, &src_path)?;
        if n > 0 {
            Self::ov_put(ov, &src_path, doc.render());
        }
        let sc = self.ov_sidecar(ov, rm.src).await?;
        sc.relations
            .retain(|r| !(r.kind == rm.rel && r.target_id == rm.dst.as_ulid()));
        if rm.reject
            && !sc
                .rejected
                .iter()
                .any(|r| r.kind == rm.rel && r.target_id == rm.dst.as_ulid())
        {
            sc.rejected.push(vault_format::sidecar::RejectedRelation {
                kind: rm.rel,
                target_id: rm.dst.as_ulid(),
                at,
            });
        }
        Ok(n > 0)
    }

    /// Creates a new note in the overlay from `doc`, stamping id/created/updated.
    fn ov_create(
        ov: &mut Overlay,
        id: NoteId,
        path: &str,
        mut doc: Document,
        now: &chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<()> {
        prepare::stamp(&mut doc, id, Some(now), Some(now))?;
        ov.original.insert(path.to_owned(), None);
        Self::ov_put(ov, path, doc.render());
        ov.created.insert(id, path.to_owned());
        Ok(())
    }

    fn render_bullets(
        &self,
        ov: &Overlay,
        index: &PathIndex,
        items: &[AiBullet],
    ) -> Result<String> {
        let mut out = String::new();
        for b in items {
            let text: String = b
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .trim_start_matches('#')
                .trim()
                .to_owned();
            out.push_str("- ");
            if let Some(d) = b.date {
                out.push_str(&d.format("%Y-%m-%d").to_string());
                out.push_str(" — ");
            }
            out.push_str(&text);
            for c in &b.cites {
                out.push(' ');
                out.push_str(&self.cite_link(ov, index, c)?);
            }
            out.push('\n');
        }
        Ok(out)
    }

    /// Writes an [`AiChangeSet`] as one commit. See the module docs.
    #[allow(clippy::too_many_lines)] // one linear pass over the parts of the change set
    pub async fn ai_apply(&mut self, scope: UserScope, set: AiChangeSet) -> Result<AiApplied> {
        let live = self
            .state()?
            .note(set.subject)
            .map(|(p, m)| (p.to_owned(), m.version.clone()));
        let (subject_path, version) = match live {
            Some(l) => l,
            None => match set.ai_notes.iter().find(|n| n.id == set.subject) {
                Some(n) => (n.path.clone(), String::new()),
                None => return Ok(AiApplied::Stale),
            },
        };
        if set.expect_version.as_ref().is_some_and(|v| *v != version) {
            return Ok(AiApplied::Stale);
        }
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let local_now = self.local_now(tz);
        let now = self.now();
        let mut ov = Overlay::default();

        // New notes first, so edges and citations can point at them.
        for c in &set.concepts {
            let path = self.ov_free_path(&ov, "concepts", &c.name, None)?;
            let stem = paths::file_name(&path).trim_end_matches(".md").to_owned();
            let body = item_render::entity::concept_body(c.summary.as_deref());
            let doc =
                item_render::entity::skeleton(NoteKind::Concept, &c.name, &stem, Vec::new(), &body)
                    .map_err(|e| prepare::render_error(&e))?;
            Self::ov_create(&mut ov, c.id, &path, doc, &local_now)?;
        }
        for e in &set.entities {
            let folder = e.kind.default_folder();
            let path = self.ov_free_path(&ov, folder, &e.name, None)?;
            let stem = paths::file_name(&path).trim_end_matches(".md").to_owned();
            let mut aliases: Vec<String> = Vec::new();
            for a in &e.aliases {
                let a = a.trim();
                if !a.is_empty() && a != e.name.trim() && !aliases.iter().any(|x| x == a) {
                    aliases.push(a.to_owned());
                }
            }
            let doc = item_render::entity::skeleton(
                e.kind,
                &e.name,
                &stem,
                aliases,
                item_render::entity::ENTITY_BODY,
            )
            .map_err(|e| prepare::render_error(&e))?;
            Self::ov_create(&mut ov, e.id, &path, doc, &local_now)?;
        }
        for n in &set.ai_notes {
            if !n.path.starts_with("_ai/") || !n.path.ends_with(".md") {
                return Err(VaultError::invalid("AI notes live under _ai/"));
            }
            paths::validate_note_path(&n.path)?;
            let body = if n.body.ends_with('\n') {
                n.body.clone()
            } else {
                format!("{}\n", n.body)
            };
            if let Some(existing) = self.ov_path(&ov, n.id) {
                let text = self.ov_text(&mut ov, &existing).await?;
                let mut doc = Document::parse(&text);
                doc.set_body(body);
                let fm = doc.frontmatter_mut();
                fm.set_text(KnownKey::Title, n.title.trim())
                    .map_err(frontmatter_err)?;
                Self::ov_put(&mut ov, &existing, doc.render());
            } else {
                if self.state()?.notes.contains_key(&n.path) {
                    return Err(VaultError::invalid("another note has this path"));
                }
                let mut doc = Document::parse(&body);
                doc.frontmatter_mut()
                    .set_text(KnownKey::Title, n.title.trim())
                    .map_err(frontmatter_err)?;
                Self::ov_create(&mut ov, n.id, &n.path, doc, &local_now)?;
            }
        }
        self.ov_block_ids(&mut ov, &set.block_ids).await?;
        for rm in &set.remove {
            self.ov_remove_edge(&mut ov, rm).await?;
        }
        for add in &set.add {
            self.ov_add_edge(&mut ov, add).await?;
        }
        for (entity, aliases) in &set.aliases {
            let Some(path) = self.ov_path(&ov, *entity) else {
                continue;
            };
            let text = self.ov_text(&mut ov, &path).await?;
            let mut doc = Document::parse(&text);
            let title = crate::derive::title_of(&path, &doc);
            let fm = doc.frontmatter_mut();
            let mut list = fm.aliases();
            let mut changed = false;
            for a in aliases {
                let a = a.trim();
                if !a.is_empty() && a != title && !list.iter().any(|x| x == a) {
                    list.push(a.to_owned());
                    changed = true;
                }
            }
            if changed {
                fm.set_list(KnownKey::Aliases, list)
                    .map_err(frontmatter_err)?;
                Self::ov_put(&mut ov, &path, doc.render());
            }
        }
        for (doc_id, date, kind, source) in &set.custody_remove {
            let Some(path) = self.ov_path(&ov, *doc_id) else {
                continue;
            };
            let Some(source_path) = self.ov_path(&ov, *source) else {
                continue;
            };
            let text = self.ov_text(&mut ov, &path).await?;
            let mut doc = Document::parse(&text);
            let index = self.ov_index(&ov)?;
            if remove_custody_line(&mut doc, *date, *kind, &source_path, &index, &path)? {
                Self::ov_put(&mut ov, &path, doc.render());
            }
        }
        for c in &set.custody {
            let Some(path) = self.ov_path(&ov, c.document) else {
                continue;
            };
            let index = self.ov_index(&ov)?;
            let citations = match &c.cite {
                Some(cite) => vec![self.cite_link(&ov, &index, cite)?],
                None => Vec::new(),
            };
            let op = sync_model::ops::DocumentCustody {
                document_id: c.document.as_ulid(),
                event: c.kind,
                at: c.date,
                place_id: c.place.map(|i| i.as_ulid()),
                person_id: c.person.map(|i| i.as_ulid()),
                counterparty_id: c.counterparty.map(|i| i.as_ulid()),
            };
            let link = |u: ulid::Ulid| {
                self.ov_path(&ov, NoteId::from_ulid(u))
                    .map(|p| format!("[[{}]]", index.link_text_for(&p)))
            };
            let event = sync_model::apply::custody_event(&op, link, citations)
                .map_err(|_| VaultError::invalid("the custody event is incomplete"))?;
            let text = self.ov_text(&mut ov, &path).await?;
            let mut doc = Document::parse(&text);
            sync_model::apply::record_custody(&mut doc, event).map_err(|_| {
                VaultError::invalid("the custody section has lines that are not events")
            })?;
            Self::ov_put(&mut ov, &path, doc.render());
        }
        for s in &set.sections {
            let Some(path) = self.ov_path(&ov, s.note) else {
                continue;
            };
            let index = self.ov_index(&ov)?;
            let mut rendered: Vec<(AiSection, String)> = Vec::new();
            if let Some(summary) = &s.summary {
                rendered.push((
                    AiSection::Summary,
                    format!("{}\n", item_render::entity::one_paragraph(summary)),
                ));
            }
            for (section, items) in &s.bullets {
                rendered.push((*section, self.render_bullets(&ov, &index, items)?));
            }
            let text = self.ov_text(&mut ov, &path).await?;
            let mut doc = Document::parse(&text);
            let updates: Vec<(AiSection, &str)> =
                rendered.iter().map(|(a, b)| (*a, b.as_str())).collect();
            let body = sections::replace_ai_sections(doc.body(), s.profile, &updates)
                .map_err(|e| VaultError::Invalid(format!("AI sections: {e}").into()))?;
            if body != doc.body() {
                doc.set_body(body);
                Self::ov_put(&mut ov, &path, doc.render());
            }
        }
        for (note, key, value) in &set.sidecar_extra {
            if self.ov_path(&ov, *note).is_none() {
                continue;
            }
            let sc = self.ov_sidecar(&mut ov, *note).await?;
            sc.extra.insert(key.clone(), value.clone());
        }
        for (note, text, kind) in &set.rejected_mentions {
            let sc = self.ov_sidecar(&mut ov, *note).await?;
            let mut list: Vec<RejectedMention> = sc
                .extra
                .get(REJECTED_MENTIONS_KEY)
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if !list.iter().any(|m| m.text == *text && m.kind == *kind) {
                list.push(RejectedMention {
                    text: text.clone(),
                    kind: kind.clone(),
                    at: now,
                });
                sc.extra.insert(
                    REJECTED_MENTIONS_KEY.to_owned(),
                    serde_json::to_value(&list).map_err(|e| VaultError::Internal(e.to_string()))?,
                );
            }
        }
        for h in &set.hints {
            if self.ov_path(&ov, h.entity).is_none() {
                continue;
            }
            let sc = self.ov_sidecar(&mut ov, h.entity).await?;
            let mut list: Vec<SidecarHint> = sc
                .extra
                .get(HINTS_KEY)
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if !list.iter().any(|x| x.text == h.text) {
                list.push(SidecarHint {
                    id: h.id.to_string(),
                    text: h.text.clone(),
                    at: now,
                });
                sc.extra.insert(
                    HINTS_KEY.to_owned(),
                    serde_json::to_value(&list).map_err(|e| VaultError::Internal(e.to_string()))?,
                );
            }
        }
        if let Some(f) = &set.filing {
            self.ov_file(&mut ov, set.subject, f).await?;
        }
        let final_path = self
            .ov_path(&ov, set.subject)
            .unwrap_or_else(|| subject_path.clone());
        let final_text = match ov.texts.get(&final_path) {
            Some(t) => t.clone(),
            None => self.ov_text(&mut ov, &final_path).await?,
        };
        let final_version = crate::fsio::version_of(final_text.as_bytes());
        if set.mark_linked || set.mark_filed {
            let sc = self.ov_sidecar(&mut ov, set.subject).await?;
            if set.mark_linked {
                sc.last_linked_hash = Some(final_version.clone());
            }
            if set.mark_filed {
                sc.extra.insert(
                    FILED_KEY.to_owned(),
                    serde_json::Value::String(final_version.clone()),
                );
            }
        }
        let changes = Self::ov_changes(&ov)?;

        let hints_known: BTreeSet<NoteId> = set.hints.iter().map(|h| h.entity).collect();
        let rows = RowWork {
            decisions: set.decisions.clone(),
            revert: set.revert_decisions.clone(),
            suggestions: set.suggestions.clone(),
            decide: set.decide.clone(),
            hints: set
                .hints
                .iter()
                .filter(|h| hints_known.contains(&h.entity))
                .cloned()
                .collect(),
            replies: set.ai_replies.clone(),
            jobs: set.jobs.clone(),
            job_id: set.job_id,
            now,
        };
        let subject = if final_path == subject_path {
            subject_path.clone()
        } else {
            format!("{subject_path} -> {final_path}")
        };
        let message = set.author.message(&set.op, &subject);
        let commit = if changes.is_empty() {
            rows.run(&mut tx).await?;
            let pending = crate::receipt::settle(
                self.receipt.as_ref(),
                &mut tx,
                &crate::receipt::AfterWrite::database_only(),
                now,
            )
            .await?;
            tx.commit().await?;
            if let Some(p) = pending {
                p.commit();
            }
            None
        } else {
            let extra: crate::ops::notes::InTx =
                Box::new(move |tx: &mut ScopedTx| Box::pin(async move { rows.run(tx).await }));
            self.finish_then(tx, changes, message, Some(extra)).await?
        };
        if let Some(c) = &commit {
            let applied: Vec<DecisionId> = set
                .decisions
                .iter()
                .filter(|d| d.suggestion.is_none())
                .map(|d| d.id)
                .collect();
            if !applied.is_empty() {
                let mut tx = self.begin(&scope).await?;
                sqlx::query("UPDATE ai_decisions SET git_commit = $1 WHERE id = ANY($2)")
                    .bind(c)
                    .bind(&applied)
                    .execute(tx.conn())
                    .await?;
                tx.commit().await?;
            }
        }
        let mut notice = crate::events::Committed::default();
        for s in &set.suggestions {
            notice.suggestions.push(crate::events::SuggestionEvent {
                id: s.id,
                note_id: s.note,
                kind: s.kind.clone(),
                status: "pending".to_owned(),
                created: true,
            });
        }
        for (s, _, _) in &set.ai_replies {
            if !set.decide.iter().any(|d| d.id == *s) {
                notice.suggestions.push(crate::events::SuggestionEvent {
                    id: *s,
                    note_id: None,
                    kind: String::new(),
                    status: "pending".to_owned(),
                    created: false,
                });
            }
        }
        for d in &set.decide {
            notice.suggestions.push(crate::events::SuggestionEvent {
                id: d.id,
                note_id: d.note,
                kind: d.kind.clone(),
                status: match d.decision {
                    SuggestionDecision::Accepted => "accepted",
                    SuggestionDecision::Rejected => "rejected",
                    SuggestionDecision::Superseded => "superseded",
                }
                .to_owned(),
                created: false,
            });
        }
        self.inner.notify(self.user, &notice);
        let version = self
            .state()?
            .note(set.subject)
            .map_or(final_version, |(_, m)| m.version.clone());
        Ok(AiApplied::Done {
            commit,
            path: final_path,
            version,
        })
    }

    /// Title, tags and folder of `id` in the overlay (the filing of an inbox capture).
    pub(crate) async fn ov_file(&self, ov: &mut Overlay, id: NoteId, f: &Filing) -> Result<()> {
        let path = self.ov_path(ov, id).ok_or(VaultError::NotFound)?;
        let folder = f.folder.trim().trim_matches('/').to_owned();
        let top = folder.split('/').next().unwrap_or_default();
        if folder.is_empty()
            || folder.starts_with('.')
            || SYSTEM_FOLDERS.contains(&top)
            || paths::validate_path(&format!("{folder}/x.md")).is_err()
        {
            return Err(VaultError::invalid("the destination folder is not allowed"));
        }
        let title = f.title.trim();
        if title.is_empty() {
            return Err(VaultError::invalid("the title is empty"));
        }
        let text = self.ov_text(ov, &path).await?;
        let mut doc = Document::parse(&text);
        let fm = doc.frontmatter_mut();
        if fm.error().is_some() {
            return Err(VaultError::invalid("the frontmatter cannot be edited"));
        }
        let mut tags = fm.tags();
        for t in &f.tags {
            let t = t.trim().trim_start_matches('#').trim().to_lowercase();
            if !t.is_empty() && vault_format::body::is_valid_tag(&t) && !tags.contains(&t) {
                tags.push(t);
            }
        }
        if !tags.is_empty() {
            fm.set_list(KnownKey::Tags, tags).map_err(frontmatter_err)?;
        }
        let new_path = self.ov_free_path(ov, &folder, title, Some(&path))?;
        let stem = paths::file_name(&new_path).trim_end_matches(".md");
        if stem == title {
            if fm.title().is_some() {
                fm.remove_key(KnownKey::Title).map_err(frontmatter_err)?;
            }
        } else {
            fm.set_text(KnownKey::Title, title)
                .map_err(frontmatter_err)?;
        }
        Self::ov_put(ov, &path, doc.render());
        self.ov_move(ov, id, &new_path).await
    }
}

/// Top-level folders a capture is never filed into (§6.1: entity, concept and AI folders).
pub const SYSTEM_FOLDERS: &[&str] = &[
    paths::INBOX_DIR,
    "people",
    "companies",
    "documents",
    "places",
    "concepts",
    "_ai",
    "maps",
    "attachments",
];

/// Collapses `s` to one paragraph (the Summary section is prose, never headings).
/// Removes the custody line of `doc` with `date` and `kind` that cites `source_path`, and
/// recomputes the derived frontmatter. Returns whether a line was removed.
fn remove_custody_line(
    doc: &mut Document,
    date: NaiveDate,
    kind: CustodyEventType,
    source_path: &str,
    index: &PathIndex,
    doc_path: &str,
) -> Result<bool> {
    let body_text = doc.body().to_owned();
    let Some(section) = sections::sections(&body_text)
        .into_iter()
        .find(|s| s.level == 2 && s.title == "Custody")
    else {
        return Ok(false);
    };
    let content = &body_text[section.own_content_span.clone()];
    let (events, bad) = vault_format::custody::parse_section(content);
    if !bad.is_empty() {
        return Err(VaultError::invalid(
            "the custody section has lines that are not events",
        ));
    }
    let cites_source = |e: &vault_format::custody::CustodyEvent| {
        e.citations.iter().any(|c| {
            vault_format::WikiLink::parse_exact(c.trim()).is_some_and(|l| {
                index.resolve(&l.path, Some(doc_path))
                    == vault_format::Resolution::Resolved(source_path.to_owned())
            })
        })
    };
    let before = events.len();
    let kept: Vec<_> = events
        .into_iter()
        .filter(|e| !(e.date == date && e.kind == kind && cites_source(e)))
        .collect();
    if kept.len() == before {
        return Ok(false);
    }
    let trailing = &content[content.trim_end_matches(['\n', '\r']).len()..];
    let trailing = if trailing.is_empty() { "\n" } else { trailing };
    let rendered = if kept.is_empty() {
        String::new()
    } else {
        format!("{}{trailing}", vault_format::custody::render_section(&kept))
    };
    let new_body = format!(
        "{}{}{}",
        &body_text[..section.own_content_span.start],
        rendered,
        &body_text[section.own_content_span.end..]
    );
    doc.set_body(new_body);
    let state = vault_format::custody::CustodyState::derive(&kept);
    let fm = doc.frontmatter_mut();
    match state {
        Some(s) => s.write_to(fm).map_err(frontmatter_err)?,
        None => {
            for k in [
                KnownKey::Location,
                KnownKey::Holder,
                KnownKey::LastHolder,
                KnownKey::Status,
            ] {
                fm.remove_key(k).map_err(frontmatter_err)?;
            }
        }
    }
    Ok(true)
}

/// The database rows of one change set, written in the commit's transaction.
#[derive(Debug, Clone)]
struct RowWork {
    decisions: Vec<NewDecision>,
    revert: Vec<DecisionId>,
    suggestions: Vec<NewSuggestion>,
    decide: Vec<DecideSuggestion>,
    hints: Vec<NewHint>,
    replies: Vec<(SuggestionId, ReplyId, String)>,
    jobs: Vec<NewJob>,
    job_id: Option<JobId>,
    now: DateTime<Utc>,
}

impl RowWork {
    async fn run(self, tx: &mut ScopedTx) -> Result<()> {
        let now = self.now;
        for s in &self.suggestions {
            srepo::create_suggestion(tx, s.id, s.note, &s.kind, &s.payload, now).await?;
            log_suggestion(tx, s.id, now).await?;
        }
        for d in &self.decide {
            let status = match d.decision {
                SuggestionDecision::Accepted => strata_index::types::SuggestionStatus::Accepted,
                SuggestionDecision::Rejected => strata_index::types::SuggestionStatus::Rejected,
                SuggestionDecision::Superseded => strata_index::types::SuggestionStatus::Superseded,
            };
            srepo::decide_suggestion(tx, d.id, status, now)
                .await?
                .ok_or_else(|| VaultError::invalid("the suggestion was already decided"))?;
            log_suggestion(tx, d.id, now).await?;
        }
        for d in &self.decisions {
            insert_decision(tx, d, self.job_id, now).await?;
        }
        for id in &self.revert {
            srepo::mark_decision_reverted(tx, *id, now).await?;
        }
        for h in &self.hints {
            erepo::add_hint(
                tx,
                &erepo::Hint {
                    id: h.id,
                    entity_id: h.entity,
                    hint: h.text.clone(),
                    source_decision_id: h.source_decision,
                    created: now,
                },
            )
            .await?;
        }
        for (s, id, body) in &self.replies {
            srepo::add_reply(
                tx,
                &srepo::Reply {
                    id: *id,
                    suggestion_id: *s,
                    author: strata_index::types::ReplyAuthor::Ai,
                    body: body.clone(),
                    created: now,
                },
            )
            .await?;
            log_suggestion(tx, *s, now).await?;
        }
        for j in &self.jobs {
            jobs::enqueue(tx, j, now).await?;
        }
        Ok(())
    }
}

async fn log_suggestion(tx: &mut ScopedTx, id: SuggestionId, now: DateTime<Utc>) -> Result<()> {
    let id_text = id.to_string();
    synclog::append_change(
        tx,
        &NewChange {
            entity_type: "suggestion",
            entity_id: &id_text,
            op: ChangeOp::Upsert,
            version: None,
            at: now,
        },
    )
    .await?;
    Ok(())
}

/// Inserts one `ai_decisions` row (with the pipeline columns of migration `…014`).
pub(crate) async fn insert_decision(
    tx: &mut ScopedTx,
    d: &NewDecision,
    job: Option<JobId>,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO ai_decisions (user_id, id, kind, source_note_id, source_block_id, \
           target_type, target_id, summary, confidence, job_id, suggestion_id, created, \
           rel_type, mention, detail) \
         VALUES (strata_current_user(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(d.id)
    .bind(d.kind)
    .bind(d.source_note)
    .bind(&d.source_block)
    .bind(&d.target_type)
    .bind(&d.target_id)
    .bind(&d.summary)
    .bind(d.confidence.map(|c| c.clamp(0.0, 1.0)))
    .bind(job)
    .bind(d.suggestion)
    .bind(now)
    .bind(&d.rel_type)
    .bind(&d.mention)
    .bind(&d.detail)
    .execute(tx.conn())
    .await?;
    Ok(())
}

impl VaultService {
    /// Writes `set` as one commit on the user's writer (see [`Core::ai_apply`]).
    pub async fn ai_apply(&self, scope: &UserScope, set: AiChangeSet) -> Result<AiApplied> {
        self.exec(scope, move |core, s| {
            Box::pin(async move { core.ai_apply(s, set).await })
        })
        .await
    }
}
