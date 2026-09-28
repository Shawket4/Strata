//! The shared half of the linking and filing pipelines (PLAN §9.3, §9.4, §6.6, §6.7, §6.11,
//! §6.12, D13 = b, D30): building the prompt context (the note's blocks, candidate notes,
//! concepts, candidate entities with their disambiguation hints, rejections) and turning the
//! model's extraction into one [`AiChangeSet`]: relations at or above the threshold, stale AI
//! edges removed, concepts linked or created, entity mentions resolved (§6.7) and linked or
//! suggested, entity-to-entity relations, custody events applied or suggested, task
//! suggestions — every one recorded as an AI decision with its ID.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, Utc};
use domain::{EntityRelationType, MentionType, NoteKind, RelationType};
use serde::Serialize;
use sha2::{Digest, Sha256};
use strata_ai::outputs::{
    ConceptProposal, CustodyEvent as AiCustodyEvent, EntityRelation, EntityRelationType as AiErt,
    Mention, MentionKind, RelationProposal, RelationType as AiRt, TaskProposal,
};
use strata_common::{DecisionId, IdGenerator, JobId, NoteId, SuggestionId};
use strata_index::repo::entities as erepo;
use strata_index::repo::jobs::NewJob;
use strata_index::types::DecisionKind;
use strata_index::{AppDb, UserScope};
use strata_vault::VaultService;
use strata_vault::ops::ai_apply::{
    AiChangeSet, AiCustody, BlockIdRequest, Cite, EdgeAdd, EdgeRemove, NewConcept, NewDecision,
    NewSuggestion, REJECTED_MENTIONS_KEY, RejectedMention,
};
use strata_vault::ops::ai_decide::{self as decide, CustodyDetail};
use strata_vault::ops::relations::AiEdge;
use strata_vault::ops::suggestions::SuggestionView;
use text_normalize::{normalize_for_search, transliteration_key, trigram_similarity};
use sync_model::suggestions::{
    CustodyPayload, CustodyTarget, DuplicateItem, DuplicatesPayload, EntityLinkPayload,
    TaskPayload,
};
use vault_format::custody::CustodyEventType;
use vault_format::sidecar::{By, NoteSidecar};
use vault_format::{Document, RelationKey};

use crate::chunk;
use crate::dates;
use crate::handler::JobError;
use crate::thresholds::AiThresholds;

/// Job kind `link` (§9.4).
pub const LINK: &str = "link";
/// Job kind `file_inbox` (§9.3).
pub const FILE_INBOX: &str = "file_inbox";
/// Job kind `entity_insights` (§6.7).
pub const ENTITY_INSIGHTS: &str = "entity_insights";
/// Linking runs this long after the last edit of a note (§9.2).
pub const LINK_DEBOUNCE: Duration = Duration::seconds(30);
/// Entity insights run this long after the last change of a mentioning note (§9.2).
pub const INSIGHTS_DEBOUNCE: Duration = Duration::minutes(5);
/// Candidate notes given to the model (§9.4: ≈15–20).
pub const MAX_CANDIDATES: usize = 20;
/// Entities given to the model.
pub const MAX_ENTITIES: usize = 40;
/// Concepts given to the model.
pub const MAX_CONCEPTS: usize = 200;
/// Body characters given to the model (longer notes are cut at a block boundary).
pub const MAX_BODY_CHARS: usize = 24_000;

/// One block of a note in a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockInput {
    /// The block's ID (existing, or generated: appended when a decision cites it).
    pub block_id: String,
    /// Its text (without the `^id` marker).
    pub text: String,
}

/// A candidate note in a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidateInput {
    /// Note ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: String,
    /// Sidecar summary.
    pub summary: Option<String>,
}

/// A concept in a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptInput {
    /// Concept note ID.
    pub id: String,
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
}

/// An entity in a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityInput {
    /// Entity note ID.
    pub id: String,
    /// `person`, `company`, `document`, `place`.
    pub kind: String,
    /// Name.
    pub name: String,
    /// Aliases (both scripts).
    pub aliases: Vec<String>,
    /// Disambiguation hints (§9.8).
    pub hints: Vec<String>,
    /// Places: the enclosing place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_of: Option<String>,
}

/// A rejection in a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RejectedInput {
    /// Relation type, or the entity kind of a rejected mention.
    #[serde(rename = "type")]
    pub kind: String,
    /// Rejected target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    /// Rejected mention text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention: Option<String>,
}

/// The ID given to a block that has none (`b-` + 6 hex of its text's SHA-256), stable across
/// runs so the same block keeps the same ID until it is appended.
pub fn generated_block_id(text: &str) -> String {
    let h = hex::encode(Sha256::digest(text.trim().as_bytes()));
    format!("b-{}", &h[..6])
}

/// The citable blocks of `body` (headings excluded), with IDs: existing ones, or generated
/// ones (returned in the map with their text).
pub fn note_blocks(body: &str) -> (Vec<BlockInput>, BTreeMap<String, String>) {
    let analysis = vault_format::analyze(body);
    let mut out = Vec::new();
    let mut generated = BTreeMap::new();
    let mut taken: BTreeSet<String> = analysis
        .blocks
        .iter()
        .filter_map(|b| b.id.as_ref().map(|i| i.id.clone()))
        .collect();
    let mut chars = 0usize;
    for b in &analysis.blocks {
        if b.kind == vault_format::body::BlockKind::Heading {
            continue;
        }
        let range = chunk::content_range(body, b);
        let text = body[range].trim().to_owned();
        if text.is_empty() {
            continue;
        }
        chars += text.chars().count();
        if chars > MAX_BODY_CHARS && !out.is_empty() {
            break;
        }
        let id = if let Some(i) = &b.id {
            i.id.clone()
        } else {
            let base = generated_block_id(&text);
            let mut id = base.clone();
            let mut n = 2;
            while taken.contains(&id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            taken.insert(id.clone());
            generated.insert(id.clone(), text.clone());
            id
        };
        out.push(BlockInput { block_id: id, text });
    }
    (out, generated)
}

/// The note a pipeline works on.
#[derive(Debug, Clone)]
pub struct SourceNote {
    /// ID.
    pub id: NoteId,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// Version (content hash).
    pub version: String,
    /// Whole file.
    pub content: String,
    /// Frontmatter `created` in the user's time zone.
    pub created: DateTime<FixedOffset>,
    /// Citable blocks.
    pub blocks: Vec<BlockInput>,
    /// Generated block IDs and their texts.
    pub generated: BTreeMap<String, String>,
    /// Sidecar (empty when absent).
    pub sidecar: NoteSidecar,
}

impl SourceNote {
    /// Loads live note `id` (`None` when missing or trashed).
    pub async fn load(
        vault: &VaultService,
        scope: &UserScope,
        id: NoteId,
    ) -> Result<Option<Self>, JobError> {
        let view = match vault.note(scope, id).await {
            Ok(v) if !v.trashed => v,
            Ok(_) | Err(strata_vault::VaultError::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let sidecar = vault
            .note_sidecar(scope, id)
            .await?
            .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
        let doc = Document::parse(&view.content);
        let created = doc
            .frontmatter()
            .and_then(|f| f.created().ok().flatten())
            .unwrap_or_else(|| view.created.fixed_offset());
        let (blocks, generated) = note_blocks(doc.body());
        Ok(Some(Self {
            id,
            path: view.path,
            title: view.title,
            kind: view.kind,
            version: view.version,
            content: view.content,
            created,
            blocks,
            generated,
            sidecar,
        }))
    }

    /// The text of block `id`.
    pub fn block_text(&self, id: &str) -> Option<&str> {
        self.blocks
            .iter()
            .find(|b| b.block_id == id)
            .map(|b| b.text.as_str())
    }

    /// Body text (for matching).
    pub fn body(&self) -> String {
        Document::parse(&self.content).body().to_owned()
    }

    /// The local date of `created`.
    pub fn created_date(&self) -> NaiveDate {
        self.created.date_naive()
    }

    /// The prompt's `created` (RFC 3339 in the user's time zone).
    pub fn created_rfc3339(&self) -> String {
        self.created.to_rfc3339()
    }

    /// The prompt's rejections: rejected relations and rejected mentions.
    pub fn rejected_input(&self) -> Vec<RejectedInput> {
        let mut out: Vec<RejectedInput> = self
            .sidecar
            .rejected
            .iter()
            .map(|r| RejectedInput {
                kind: r.kind.as_str().to_owned(),
                target_id: Some(r.target_id.to_string()),
                mention: None,
            })
            .collect();
        out.extend(
            rejected_mentions(&self.sidecar)
                .into_iter()
                .map(|m| RejectedInput {
                    kind: m.kind,
                    target_id: None,
                    mention: Some(m.text),
                }),
        );
        out
    }
}

/// The mentions recorded as rejected in `sc`.
pub fn rejected_mentions(sc: &NoteSidecar) -> Vec<RejectedMention> {
    sc.extra
        .get(REJECTED_MENTIONS_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// An entity of the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityInfo {
    /// Entity note.
    pub id: NoteId,
    /// Kind.
    pub kind: NoteKind,
    /// Display name.
    pub name: String,
    /// Aliases (without the name).
    pub aliases: Vec<String>,
    /// Places: the enclosing place.
    pub parent: Option<NoteId>,
}

impl EntityInfo {
    /// Name and aliases.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

/// A concept of the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptInfo {
    /// Concept note.
    pub id: NoteId,
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
}

/// Every live entity and concept of the user.
#[derive(Debug, Clone, Default)]
pub struct Directory {
    /// Entities by name.
    pub entities: Vec<EntityInfo>,
    /// Concepts by name.
    pub concepts: Vec<ConceptInfo>,
    /// Hints per entity.
    pub hints: BTreeMap<NoteId, Vec<String>>,
}

fn domain_kind(k: &str) -> Option<NoteKind> {
    k.parse().ok()
}

impl Directory {
    /// Loads the user's entities, concepts and hints.
    pub async fn load(db: &AppDb, scope: &UserScope) -> Result<Self, JobError> {
        let mut tx = db.begin(scope).await?;
        let rows: Vec<(NoteId, String, String)> = sqlx::query_as(
            "SELECT e.note_id, e.kind, e.display_name FROM entities e \
             JOIN notes n ON n.user_id = e.user_id AND n.id = e.note_id \
             WHERE NOT n.trashed ORDER BY e.display_name, e.note_id",
        )
        .fetch_all(tx.conn())
        .await?;
        let aliases: Vec<(NoteId, String)> =
            sqlx::query_as("SELECT note_id, alias FROM entity_aliases ORDER BY note_id, alias")
                .fetch_all(tx.conn())
                .await?;
        let parents: Vec<(NoteId, Option<NoteId>)> =
            sqlx::query_as("SELECT note_id, parent_id FROM places")
                .fetch_all(tx.conn())
                .await?;
        let concepts: Vec<(NoteId, String)> = sqlx::query_as(
            "SELECT id, title FROM notes WHERE kind = 'concept' AND NOT trashed ORDER BY title, id",
        )
        .fetch_all(tx.conn())
        .await?;
        let concept_aliases: Vec<(NoteId, String)> = sqlx::query_as(
            "SELECT a.note_id, a.alias FROM aliases a JOIN notes n \
               ON n.user_id = a.user_id AND n.id = a.note_id \
             WHERE n.kind = 'concept' ORDER BY a.note_id, a.alias",
        )
        .fetch_all(tx.conn())
        .await?;
        let ids: Vec<NoteId> = rows.iter().map(|r| r.0).collect();
        let hints = erepo::hints_for(&mut tx, &ids).await?;
        tx.commit().await?;
        let mut by_alias: BTreeMap<NoteId, Vec<String>> = BTreeMap::new();
        for (id, a) in aliases {
            by_alias.entry(id).or_default().push(a);
        }
        let parents: BTreeMap<NoteId, Option<NoteId>> = parents.into_iter().collect();
        let entities = rows
            .into_iter()
            .filter_map(|(id, kind, name)| {
                let kind = domain_kind(&kind)?;
                let aliases = by_alias
                    .remove(&id)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|a| *a != name)
                    .collect();
                Some(EntityInfo {
                    id,
                    kind,
                    name,
                    aliases,
                    parent: parents.get(&id).copied().flatten(),
                })
            })
            .collect();
        let mut calias: BTreeMap<NoteId, Vec<String>> = BTreeMap::new();
        for (id, a) in concept_aliases {
            calias.entry(id).or_default().push(a);
        }
        let concepts = concepts
            .into_iter()
            .map(|(id, name)| ConceptInfo {
                id,
                aliases: calias.remove(&id).unwrap_or_default(),
                name,
            })
            .collect();
        let mut hmap: BTreeMap<NoteId, Vec<String>> = BTreeMap::new();
        for h in hints {
            hmap.entry(h.entity_id).or_default().push(h.hint);
        }
        Ok(Self {
            entities,
            concepts,
            hints: hmap,
        })
    }

    /// The entity `id`.
    pub fn entity(&self, id: NoteId) -> Option<&EntityInfo> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Entities of `kind` with a name or alias whose normalised form equals `text`'s.
    pub fn by_alias(&self, text: &str, kinds: &[NoteKind]) -> Vec<NoteId> {
        let n = normalize_for_search(text);
        if n.is_empty() {
            return Vec::new();
        }
        self.entities
            .iter()
            .filter(|e| kinds.contains(&e.kind))
            .filter(|e| e.names().any(|a| normalize_for_search(a) == n))
            .map(|e| e.id)
            .collect()
    }

    /// Entities whose names occur in `text` (normalised, whole words; or by transliteration
    /// key across scripts), plus `extra` (already linked, or similar by embedding). Sorted by
    /// kind then name; at most [`MAX_ENTITIES`].
    pub fn select(&self, text: &str, extra: &BTreeSet<NoteId>) -> Vec<&EntityInfo> {
        let words: Vec<String> = normalize_for_search(text)
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect();
        let padded = format!(" {} ", words.join(" "));
        // Transliteration keys of every 1–3-word window, compared with names of as many words.
        let mut keys: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
        for len in 1..=3 {
            for w in words.windows(len) {
                let k = transliteration_key(&w.join(" "));
                if k.chars().count() >= 2 {
                    keys.entry(len).or_default().insert(k);
                }
            }
        }
        let translit_hit = |a: &str| {
            let n = normalize_for_search(a);
            let len = n.split(' ').filter(|w| !w.is_empty()).count();
            let k = transliteration_key(a);
            k.chars().count() >= 2 && keys.get(&len).is_some_and(|s| s.contains(&k))
        };
        let mut out: Vec<&EntityInfo> = self
            .entities
            .iter()
            .filter(|e| {
                extra.contains(&e.id)
                    || e.names().any(|a| {
                        let n = normalize_for_search(a);
                        (!n.is_empty() && padded.contains(&format!(" {n} "))) || translit_hit(a)
                    })
            })
            .collect();
        // Places nested in (or enclosing) an offered place: "the safe at the office".
        let places: BTreeSet<NoteId> = out
            .iter()
            .filter(|e| e.kind == NoteKind::Place)
            .map(|e| e.id)
            .collect();
        for e in &self.entities {
            let related = e.kind == NoteKind::Place
                && (e.parent.is_some_and(|p| places.contains(&p))
                    || out.iter().any(|o| o.parent == Some(e.id)));
            if related && !out.iter().any(|o| o.id == e.id) {
                out.push(e);
            }
        }
        out.sort_by(|a, b| (a.kind.as_str(), &a.name, a.id).cmp(&(b.kind.as_str(), &b.name, b.id)));
        out.truncate(MAX_ENTITIES);
        out
    }

    /// The prompt form of `entities`.
    pub fn entity_inputs(&self, entities: &[&EntityInfo]) -> Vec<EntityInput> {
        entities
            .iter()
            .map(|e| EntityInput {
                id: e.id.to_string(),
                kind: e.kind.as_str().to_owned(),
                name: e.name.clone(),
                aliases: e.aliases.clone(),
                hints: self.hints.get(&e.id).cloned().unwrap_or_default(),
                part_of: e.parent.map(|p| p.to_string()),
            })
            .collect()
    }

    /// The prompt form of the concepts.
    pub fn concept_inputs(&self) -> Vec<ConceptInput> {
        self.concepts
            .iter()
            .take(MAX_CONCEPTS)
            .map(|c| ConceptInput {
                id: c.id.to_string(),
                name: c.name.clone(),
                aliases: c.aliases.clone(),
            })
            .collect()
    }
}

/// Keyword query for candidate notes: up to 12 distinct normalised words of 4+ characters.
pub fn keyword_query(text: &str) -> Option<String> {
    let mut seen = BTreeSet::new();
    let mut words = Vec::new();
    for w in normalize_for_search(text).split(' ') {
        if w.chars().count() >= 4 && w.chars().all(char::is_alphanumeric) && seen.insert(w) {
            words.push(w.to_owned());
            if words.len() == 12 {
                break;
            }
        }
    }
    (!words.is_empty()).then(|| words.join(" OR "))
}

/// Candidate notes for `note` (§9.4 step 1): the most similar notes by note vector (when an
/// embedding model is loaded) and keyword hits, excluding the note itself, notes of other
/// kinds than `note`, and every note with a rejected edge from this note.
pub async fn candidates(
    db: &AppDb,
    vault: &VaultService,
    scope: &UserScope,
    embedder_model: Option<&str>,
    note: &SourceNote,
) -> Result<Vec<CandidateInput>, JobError> {
    let excluded: BTreeSet<NoteId> = note
        .sidecar
        .rejected
        .iter()
        .map(|r| NoteId::from_ulid(r.target_id))
        .chain(std::iter::once(note.id))
        .collect();
    let mut tx = db.begin(scope).await?;
    let mut ordered: Vec<NoteId> = Vec::new();
    if let Some(model) = embedder_model {
        for n in crate::vectors::similar_notes(&mut tx, note.id, model, 30, 0.3).await? {
            ordered.push(n.note_id);
        }
    }
    if let Some(q) = keyword_query(&format!("{} {}", note.title, note.body())) {
        for h in strata_index::repo::notes::search_notes(&mut tx, &q, 30).await? {
            if !ordered.contains(&h.note_id) {
                ordered.push(h.note_id);
            }
        }
    }
    let mut out = Vec::new();
    for id in ordered {
        if excluded.contains(&id) || out.len() >= MAX_CANDIDATES {
            continue;
        }
        let Some(row) = strata_index::repo::notes::get_note(&mut tx, id).await? else {
            continue;
        };
        if row.trashed || row.kind != strata_index::types::NoteKind::Note {
            continue;
        }
        out.push((id, row.title));
    }
    tx.commit().await?;
    let mut inputs = Vec::with_capacity(out.len());
    for (id, title) in out {
        let summary = vault.note_sidecar(scope, id).await?.and_then(|s| s.summary);
        inputs.push(CandidateInput {
            id: id.to_string(),
            title,
            kind: "note".to_owned(),
            summary,
        });
    }
    Ok(inputs)
}

/// Entities linked from the note's frontmatter (`people`, `companies`) and entities similar
/// to the note by embedding.
pub async fn linked_entities(
    db: &AppDb,
    scope: &UserScope,
    note: NoteId,
    embedder_model: Option<&str>,
) -> Result<BTreeSet<NoteId>, JobError> {
    let mut tx = db.begin(scope).await?;
    let mut out: BTreeSet<NoteId> = strata_index::repo::graph::relations_from(&mut tx, note)
        .await?
        .into_iter()
        .filter(|r| r.rel_type == "people" || r.rel_type == "companies")
        .map(|r| r.dst_id)
        .collect();
    if let Some(model) = embedder_model {
        let similar: Vec<(NoteId,)> = sqlx::query_as(
            "WITH q AS (SELECT embedding FROM note_vectors WHERE note_id = $1 AND model = $2) \
             SELECT v.note_id FROM note_vectors v CROSS JOIN q \
             JOIN entities e ON e.user_id = v.user_id AND e.note_id = v.note_id \
             WHERE v.model = $2 AND v.note_id <> $1 \
               AND 1 - (v.embedding <=> q.embedding)::float8 >= 0.6 \
             ORDER BY v.embedding <=> q.embedding, v.note_id LIMIT 5",
        )
        .bind(note)
        .bind(model)
        .fetch_all(tx.conn())
        .await?;
        out.extend(similar.into_iter().map(|r| r.0));
    }
    tx.commit().await?;
    Ok(out)
}

/// What the model extracted (linking output, or the matching part of the filing output).
#[derive(Debug, Clone, Default)]
pub struct Extraction {
    /// Relations.
    pub relations: Vec<RelationProposal>,
    /// Concepts.
    pub concepts: Vec<ConceptProposal>,
    /// Mentions.
    pub mentions: Vec<Mention>,
    /// Entity relations.
    pub entity_relations: Vec<EntityRelation>,
    /// Custody events.
    pub custody: Vec<AiCustodyEvent>,
    /// Tasks.
    pub tasks: Vec<TaskProposal>,
}

fn rel_type(t: AiRt) -> RelationType {
    match t {
        AiRt::Related => RelationType::Related,
        AiRt::PartOf => RelationType::PartOf,
        AiRt::Supports => RelationType::Supports,
        AiRt::Contradicts => RelationType::Contradicts,
        AiRt::FollowsUp => RelationType::FollowsUp,
        AiRt::Duplicates => RelationType::Duplicates,
    }
}

fn entity_rel_type(t: AiErt) -> EntityRelationType {
    match t {
        AiErt::WorksAt => EntityRelationType::WorksAt,
        AiErt::WorkedAt => EntityRelationType::WorkedAt,
        AiErt::ReportsTo => EntityRelationType::ReportsTo,
        AiErt::Knows => EntityRelationType::Knows,
        AiErt::IntroducedBy => EntityRelationType::IntroducedBy,
        AiErt::ClientOf => EntityRelationType::ClientOf,
        AiErt::SupplierOf => EntityRelationType::SupplierOf,
        AiErt::PartnerOf => EntityRelationType::PartnerOf,
        AiErt::CompetitorOf => EntityRelationType::CompetitorOf,
        AiErt::SubsidiaryOf => EntityRelationType::SubsidiaryOf,
    }
}

/// The kind a relation of type `t` points at.
fn entity_rel_target(t: EntityRelationType) -> NoteKind {
    match t {
        EntityRelationType::ReportsTo
        | EntityRelationType::Knows
        | EntityRelationType::IntroducedBy => NoteKind::Person,
        _ => NoteKind::Company,
    }
}

fn mention_kind(k: MentionKind) -> NoteKind {
    match k {
        MentionKind::Person => NoteKind::Person,
        MentionKind::Company => NoteKind::Company,
        MentionKind::Document => NoteKind::Document,
        MentionKind::Place => NoteKind::Place,
    }
}

fn mention_rel(k: NoteKind) -> Option<RelationKey> {
    match k {
        NoteKind::Person => Some(RelationKey::Mention(MentionType::People)),
        NoteKind::Company => Some(RelationKey::Mention(MentionType::Companies)),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)] // confidences are in 0..=1
fn conf32(c: f64) -> Option<f32> {
    let c = c.clamp(0.0, 1.0) as f32;
    c.is_finite().then_some(c)
}

/// Existing suggestions of the note, to avoid proposing the same thing twice.
#[derive(Debug, Default, Clone)]
pub struct PendingKeys {
    keys: BTreeSet<String>,
}

impl PendingKeys {
    /// From the note's pending suggestions.
    pub fn from_views(views: &[SuggestionView]) -> Self {
        let mut keys = BTreeSet::new();
        for v in views {
            let s = &v.suggestion;
            let key = match s.kind.as_str() {
                decide::KIND_ENTITY_LINK => rmp_serde::from_slice::<EntityLinkPayload>(&s.payload)
                    .ok()
                    .map(|p| entity_key(&p.kind, &p.mention)),
                decide::KIND_TASK => rmp_serde::from_slice::<TaskPayload>(&s.payload)
                    .ok()
                    .map(|p| task_key(&p.title)),
                decide::KIND_CUSTODY => rmp_serde::from_slice::<CustodyPayload>(&s.payload)
                    .ok()
                    .map(|p| custody_key(&p.document.mention, &p.event, p.date)),
                decide::KIND_FILING => Some("filing".to_owned()),
                _ => None,
            };
            keys.extend(key);
        }
        Self { keys }
    }

    /// Whether `key` is pending; records it otherwise.
    fn seen(&mut self, key: String) -> bool {
        !self.keys.insert(key)
    }
}

fn entity_key(kind: &str, mention: &str) -> String {
    format!("entity:{kind}:{}", normalize_for_search(mention))
}

fn task_key(title: &str) -> String {
    format!("task:{}", normalize_for_search(title))
}

fn custody_key(document: &str, event: &str, date: NaiveDate) -> String {
    format!("custody:{}:{event}:{date}", normalize_for_search(document))
}

/// A role of a custody event after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Role {
    target: CustodyTarget,
}

impl Role {
    fn resolved(&self) -> Option<NoteId> {
        self.target.id.map(NoteId::from_ulid)
    }
}

/// Existing custody events of documents (conflict and repetition checks, §6.12).
pub type DocumentEvents = BTreeMap<NoteId, Vec<vault_format::custody::CustodyEvent>>;

/// Loads the custody events of every document in `dir`.
pub async fn document_events(
    vault: &VaultService,
    scope: &UserScope,
    dir: &Directory,
) -> Result<DocumentEvents, JobError> {
    let mut out = DocumentEvents::new();
    for e in dir.entities.iter().filter(|e| e.kind == NoteKind::Document) {
        let view = match vault.note(scope, e.id).await {
            Ok(v) => v,
            Err(strata_vault::VaultError::NotFound) => continue,
            Err(err) => return Err(err.into()),
        };
        let doc = Document::parse(&view.content);
        let events = strata_vault::derive::custody_events(doc.body());
        out.insert(e.id, events);
    }
    Ok(out)
}

/// Turns an [`Extraction`] into a change set (see the module docs).
#[derive(Debug)]
pub struct Planner<'a> {
    /// The note.
    pub note: &'a SourceNote,
    /// Entities and concepts.
    pub dir: &'a Directory,
    /// Candidate note IDs given to the model.
    pub candidates: BTreeSet<NoteId>,
    /// Entities given to the model.
    pub offered: BTreeSet<NoteId>,
    /// Thresholds.
    pub thresholds: AiThresholds,
    /// `provider/model`.
    pub model: String,
    /// IDs.
    pub ids: &'a dyn IdGenerator,
    /// Now.
    pub now: DateTime<Utc>,
    /// Existing custody events.
    pub events: &'a DocumentEvents,
    /// The change set being built.
    pub set: AiChangeSet,
    pending: PendingKeys,
    applied: BTreeSet<(RelationKey, NoteId)>,
    resolved: BTreeMap<String, NoteId>,
    ambiguous: BTreeMap<String, Vec<NoteId>>,
    needs_block: BTreeSet<String>,
    touched_entities: BTreeSet<NoteId>,
    /// Decisions recorded, by kind (for tests and logs).
    pub decided: Vec<DecisionId>,
}

impl<'a> Planner<'a> {
    /// A planner for `note`.
    #[allow(clippy::too_many_arguments)] // every input of a plan is explicit
    pub fn new(
        job: &str,
        job_id: Option<JobId>,
        note: &'a SourceNote,
        dir: &'a Directory,
        candidates: &[CandidateInput],
        offered: &[EntityInput],
        thresholds: AiThresholds,
        model: String,
        ids: &'a dyn IdGenerator,
        now: DateTime<Utc>,
        events: &'a DocumentEvents,
        pending: PendingKeys,
    ) -> Self {
        let mut set = AiChangeSet::new(job, note.id);
        set.job_id = job_id;
        set.expect_version = Some(note.version.clone());
        set.mark_linked = true;
        Self {
            note,
            dir,
            candidates: candidates
                .iter()
                .filter_map(|c| c.id.parse().ok())
                .collect(),
            offered: offered.iter().filter_map(|e| e.id.parse().ok()).collect(),
            thresholds,
            model,
            ids,
            now,
            events,
            set,
            pending,
            applied: BTreeSet::new(),
            resolved: BTreeMap::new(),
            ambiguous: BTreeMap::new(),
            needs_block: BTreeSet::new(),
            touched_entities: BTreeSet::new(),
            decided: Vec::new(),
        }
    }

    fn decision(&mut self, d: NewDecision) {
        self.decided.push(d.id);
        self.set.decisions.push(d);
    }

    fn new_decision(&self, kind: DecisionKind) -> NewDecision {
        NewDecision {
            id: DecisionId::generate(self.ids),
            kind,
            source_note: Some(self.note.id),
            source_block: None,
            target_type: String::new(),
            target_id: String::new(),
            summary: String::new(),
            confidence: None,
            rel_type: None,
            mention: None,
            detail: Vec::new(),
            suggestion: None,
        }
    }

    fn suggest(&mut self, kind: &str, payload: Vec<u8>) -> SuggestionId {
        let id = SuggestionId::generate(self.ids);
        self.set.suggestions.push(NewSuggestion {
            id,
            note: Some(self.note.id),
            kind: kind.to_owned(),
            payload,
        });
        id
    }

    fn edge(&mut self, src: NoteId, rel: RelationKey, dst: NoteId, confidence: f64, reason: &str) {
        self.set.add.push(EdgeAdd {
            src,
            edge: AiEdge {
                rel,
                dst,
                confidence,
                reason: reason.to_owned(),
                model: self.model.clone(),
            },
            by: By::Ai,
        });
        if src == self.note.id {
            self.applied.insert((rel, dst));
        }
    }

    fn title(&self, id: NoteId) -> String {
        self.dir
            .entity(id)
            .map(|e| e.name.clone())
            .or_else(|| {
                self.dir
                    .concepts
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
            })
            .unwrap_or_else(|| id.to_string())
    }

    fn cite_block(&mut self, block: Option<&str>) -> Option<String> {
        let b = block?;
        self.note.block_text(b)?;
        if self.note.generated.contains_key(b) {
            self.needs_block.insert(b.to_owned());
        }
        Some(b.to_owned())
    }

    /// Relations to candidate notes (§9.4 step 3); `duplicates` becomes a suggestion.
    pub async fn relations(
        &mut self,
        db: &AppDb,
        scope: &UserScope,
        vault: &VaultService,
        rels: &[RelationProposal],
        titles: &BTreeMap<NoteId, String>,
    ) -> Result<(), JobError> {
        for r in rels {
            let Ok(dst) = r.target_id.parse::<NoteId>() else {
                continue;
            };
            if dst == self.note.id || !self.candidates.contains(&dst) {
                continue;
            }
            let t = rel_type(r.kind);
            let rel = RelationKey::Note(t);
            if self.note.sidecar.is_blocked(rel, dst.as_ulid()) {
                continue;
            }
            let title = titles.get(&dst).cloned().unwrap_or_default();
            if t == RelationType::Duplicates {
                if duplicates_known(db, scope, vault, self.note.id, dst).await? {
                    continue;
                }
                let payload = DuplicatesPayload {
                    a: note_item(self.note.id, &self.note.title, r.confidence),
                    b: note_item(dst, &title, r.confidence),
                    reason: Some(r.reason.clone()),
                };
                let sid = self.suggest(decide::KIND_DUPLICATES, rmp(&payload)?);
                let mut d = self.new_decision(DecisionKind::Relation);
                d.target_type = "note".into();
                d.target_id = dst.to_string();
                d.summary = format!("duplicates → {title}: {}", r.reason);
                d.confidence = conf32(r.confidence);
                d.rel_type = Some(rel.as_str().into());
                d.suggestion = Some(sid);
                self.decision(d);
                continue;
            }
            if r.confidence < self.thresholds.relation {
                continue;
            }
            self.edge(self.note.id, rel, dst, r.confidence, &r.reason);
            let mut d = self.new_decision(DecisionKind::Relation);
            d.target_type = "note".into();
            d.target_id = dst.to_string();
            d.summary = format!("{} → {title}: {}", rel.as_str(), r.reason);
            d.confidence = conf32(r.confidence);
            d.rel_type = Some(rel.as_str().into());
            self.decision(d);
        }
        Ok(())
    }

    /// Concepts (§6.6): link existing ones, create new ones (reusing a concept with the same
    /// or a very similar name).
    pub fn concepts(&mut self, concepts: &[ConceptProposal]) {
        let rel = RelationKey::Mention(MentionType::Concepts);
        for c in concepts {
            if c.confidence < self.thresholds.relation || c.name.trim().is_empty() {
                continue;
            }
            let by_id = c
                .existing_id
                .as_deref()
                .and_then(|i| i.parse::<NoteId>().ok())
                .filter(|i| self.dir.concepts.iter().any(|x| x.id == *i));
            let n = normalize_for_search(&c.name);
            let by_name = || {
                self.dir
                    .concepts
                    .iter()
                    .find(|x| {
                        std::iter::once(&x.name).chain(x.aliases.iter()).any(|a| {
                            let an = normalize_for_search(a);
                            an == n || trigram_similarity(&an, &n) >= 0.8
                        })
                    })
                    .map(|x| x.id)
            };
            let created_here = self
                .set
                .concepts
                .iter()
                .find(|x| normalize_for_search(&x.name) == n)
                .map(|x| x.id);
            let target = if let Some(t) = by_id.or_else(by_name).or(created_here) {
                t
            } else {
                let id = NoteId::generate(self.ids);
                self.set.concepts.push(NewConcept {
                    id,
                    name: c.name.trim().to_owned(),
                    summary: c.summary.clone(),
                });
                id
            };
            if self.note.sidecar.is_blocked(rel, target.as_ulid())
                || self.applied.contains(&(rel, target))
            {
                continue;
            }
            self.edge(self.note.id, rel, target, c.confidence, "");
            let mut d = self.new_decision(DecisionKind::Concept);
            d.target_type = "concept".into();
            d.target_id = target.to_string();
            d.summary = format!("concept: {}", c.name.trim());
            d.confidence = conf32(c.confidence);
            d.rel_type = Some(rel.as_str().into());
            self.decision(d);
        }
    }

    /// Entity mentions (§6.7 resolution rules, D13 = b).
    #[allow(clippy::too_many_lines)] // one pass over the resolution cases
    pub fn mentions(&mut self, mentions: &[Mention]) {
        let rejected = rejected_mentions(&self.note.sidecar);
        for m in mentions {
            let kind = mention_kind(m.kind);
            let norm = normalize_for_search(&m.text);
            if norm.is_empty()
                || rejected
                    .iter()
                    .any(|r| r.kind == kind.as_str() && normalize_for_search(&r.text) == norm)
            {
                continue;
            }
            let valid = |id: &str| {
                id.parse::<NoteId>()
                    .ok()
                    .filter(|i| self.dir.entity(*i).is_some_and(|e| e.kind == kind))
            };
            let alias_hits = self.dir.by_alias(&m.text, &[kind]);
            let existing = m.existing_id.as_deref().and_then(valid);
            let mut candidates: Vec<NoteId> =
                m.candidate_ids.iter().filter_map(|c| valid(c)).collect();
            if m.is_nickname {
                for a in &alias_hits {
                    if !candidates.contains(a) {
                        candidates.push(*a);
                    }
                }
            }
            if candidates.len() > 1 {
                self.ambiguous.insert(norm.clone(), candidates.clone());
            }
            let rel = mention_rel(kind);
            let blocked =
                |t: NoteId| rel.is_some_and(|r| self.note.sidecar.is_blocked(r, t.as_ulid()));
            // Nicknames and kinship terms link only through an existing alias (§6.7).
            let auto = if m.is_nickname {
                match alias_hits.as_slice() {
                    [one] => Some(*one),
                    _ => None,
                }
            } else {
                existing.filter(|e| {
                    m.confidence >= self.thresholds.relation
                        && candidates.iter().all(|c| c == e)
                        && self.offered.contains(e)
                })
            };
            if let Some(t) = auto {
                if blocked(t) {
                    continue;
                }
                self.resolved.insert(norm.clone(), t);
                self.touched_entities.insert(t);
                let Some(rel) = rel else { continue };
                if self.applied.contains(&(rel, t)) {
                    continue;
                }
                self.edge(self.note.id, rel, t, m.confidence, "");
                let mut d = self.new_decision(DecisionKind::EntityMention);
                d.source_block.clone_from(&m.evidence_block_id);
                d.target_type = "entity".into();
                d.target_id = t.to_string();
                d.summary = format!("\"{}\" → {}", m.text, self.title(t));
                d.confidence = conf32(m.confidence);
                d.rel_type = Some(rel.as_str().into());
                d.mention = Some(m.text.clone());
                self.decision(d);
                continue;
            }
            if rel.is_none() {
                // Documents and places matter for custody only; a single alias match
                // resolves them there.
                if let Some(e) = existing.filter(|_| m.confidence >= self.thresholds.relation) {
                    self.resolved.insert(norm.clone(), e);
                }
                continue;
            }
            let proposed = existing.or(match candidates.as_slice() {
                [one] => Some(*one),
                _ => None,
            });
            if proposed.is_some_and(blocked) && candidates.len() <= 1 {
                continue;
            }
            let reason = if m.is_nickname {
                "nickname"
            } else if candidates.len() > 1 {
                "ambiguous"
            } else if proposed.is_none() {
                "new"
            } else {
                "low_confidence"
            };
            if self.pending.seen(entity_key(kind.as_str(), &m.text)) {
                continue;
            }
            let decision_id = DecisionId::generate(self.ids);
            let payload = EntityLinkPayload {
                decision_id: decision_id.as_ulid(),
                mention: m.text.clone(),
                kind: kind.as_str().to_owned(),
                source_note: self.note.id.as_ulid(),
                block_id: m.evidence_block_id.clone(),
                proposed: proposed.map(|n| n.as_ulid()),
                candidates: candidates.iter().map(NoteId::as_ulid).collect(),
                is_nickname: m.is_nickname,
                confidence: m.confidence,
                reason: reason.to_owned(),
            };
            let Ok(bytes) = rmp(&payload) else { continue };
            let sid = self.suggest(decide::KIND_ENTITY_LINK, bytes);
            let mut d = self.new_decision(DecisionKind::EntityMention);
            d.id = decision_id;
            d.source_block.clone_from(&m.evidence_block_id);
            d.target_type = "entity".into();
            d.target_id = proposed.map_or_else(String::new, |p| p.to_string());
            d.summary = match proposed {
                Some(p) => format!("\"{}\" → {}? ({reason})", m.text, self.title(p)),
                None => format!("\"{}\": link or create ({reason})", m.text),
            };
            d.confidence = conf32(m.confidence);
            d.rel_type = rel.map(|r| r.as_str().to_owned());
            d.mention = Some(m.text.clone());
            d.suggestion = Some(sid);
            self.decision(d);
        }
    }

    fn resolve_text(&self, text: &str, kinds: &[NoteKind]) -> Result<NoteId, Vec<NoteId>> {
        let norm = normalize_for_search(text);
        if let Some(id) = self.resolved.get(&norm)
            && self
                .dir
                .entity(*id)
                .is_some_and(|e| kinds.contains(&e.kind))
        {
            return Ok(*id);
        }
        match self.dir.by_alias(text, kinds).as_slice() {
            [one] => Ok(*one),
            [] => Err(self
                .ambiguous
                .get(&norm)
                .map(|c| {
                    c.iter()
                        .copied()
                        .filter(|i| self.dir.entity(*i).is_some_and(|e| kinds.contains(&e.kind)))
                        .collect()
                })
                .unwrap_or_default()),
            many => Err(many.to_vec()),
        }
    }

    /// Entity-to-entity relations the note states.
    pub fn entity_relations(&mut self, rels: &[EntityRelation]) {
        for r in rels {
            if r.confidence < self.thresholds.relation {
                continue;
            }
            let t = entity_rel_type(r.kind);
            let (Ok(from), Ok(to)) = (
                self.resolve_text(&r.from, &[t.subject_kind()]),
                self.resolve_text(&r.to, &[entity_rel_target(t)]),
            ) else {
                continue;
            };
            if from == to {
                continue;
            }
            let rel = RelationKey::Entity(t);
            self.edge(from, rel, to, r.confidence, &r.reason);
            self.touched_entities.insert(from);
            let mut d = self.new_decision(DecisionKind::Relation);
            d.source_note = Some(from);
            d.source_block.clone_from(&r.evidence_block_id);
            d.target_type = "entity".into();
            d.target_id = to.to_string();
            d.summary = format!(
                "{} {} {} ({})",
                self.title(from),
                rel.as_str(),
                self.title(to),
                self.note.title
            );
            d.confidence = conf32(r.confidence);
            d.rel_type = Some(rel.as_str().into());
            self.decision(d);
        }
    }

    fn custody_role(
        &self,
        mention: Option<&str>,
        existing: Option<&str>,
        kinds: &[NoteKind],
        nickname_ok: bool,
    ) -> Option<Role> {
        let mention = mention?.trim();
        if mention.is_empty() {
            return None;
        }
        let by_id = existing
            .and_then(|i| i.parse::<NoteId>().ok())
            .filter(|i| self.dir.entity(*i).is_some_and(|e| kinds.contains(&e.kind)));
        let (id, candidates) = match by_id {
            Some(i) if nickname_ok => (Some(i), Vec::new()),
            _ => match self.resolve_text(mention, kinds) {
                Ok(i) => (Some(i), Vec::new()),
                Err(c) => (None, c),
            },
        };
        Some(Role {
            target: CustodyTarget {
                mention: mention.to_owned(),
                id: id.map(|n| n.as_ulid()),
                candidates: candidates.iter().map(NoteId::as_ulid).collect(),
            },
        })
    }

    /// Custody events (D30): applied at or above the custody threshold when every entity
    /// resolves unambiguously and nothing newer contradicts it; otherwise a suggestion.
    #[allow(clippy::too_many_lines)] // one pass over the D30 cases
    pub fn custody(&mut self, events: &[AiCustodyEvent], nicknames: &BTreeSet<String>) {
        for e in events {
            let kind = map_custody(e.kind);
            let person_nick = e
                .person
                .as_deref()
                .is_some_and(|p| nicknames.contains(&normalize_for_search(p)));
            let Some(document) = self.custody_role(
                Some(&e.document),
                e.document_existing_id.as_deref(),
                &[NoteKind::Document],
                true,
            ) else {
                continue;
            };
            let place = self.custody_role(
                e.place.as_deref(),
                e.place_existing_id.as_deref(),
                &[NoteKind::Place],
                true,
            );
            let person = self.custody_role(
                e.person.as_deref(),
                e.person_existing_id.as_deref(),
                &[NoteKind::Person],
                !person_nick,
            );
            let counterparty = self.custody_role(
                e.counterparty.as_deref(),
                None,
                &[NoteKind::Person, NoteKind::Company],
                true,
            );
            let block_text = e
                .evidence_block_id
                .as_deref()
                .and_then(|b| self.note.block_text(b))
                .unwrap_or(&e.quote)
                .to_owned();
            let date = dates::event_date(
                &block_text,
                self.note.created_date(),
                Some(&e.date),
                Some(e.date_source),
            );
            let roles = [&Some(document.clone()), &place, &person, &counterparty];
            let unresolved: Vec<&Role> = roles
                .iter()
                .filter_map(|r| r.as_ref())
                .filter(|r| r.resolved().is_none())
                .collect();
            let doc_id = document.resolved();
            let existing_events = doc_id.and_then(|d| self.events.get(&d));
            let stem = self
                .note
                .path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .trim_end_matches(".md")
                .to_owned();
            let repeated = existing_events.is_some_and(|evs| {
                evs.iter().any(|x| {
                    x.date == date
                        && x.kind == kind
                        && x.citations.iter().any(|c| {
                            vault_format::WikiLink::parse_exact(c.trim())
                                .is_some_and(|l| l.path.rsplit('/').next() == Some(stem.as_str()))
                        })
                })
            });
            if repeated {
                continue;
            }
            let conflict = existing_events.is_some_and(|evs| evs.iter().any(|x| x.date > date));
            let reason = if !unresolved.is_empty() {
                if unresolved.iter().any(|r| r.target.candidates.len() > 1) {
                    Some("ambiguous")
                } else {
                    Some("unknown")
                }
            } else if e.confidence < self.thresholds.custody {
                Some("low_confidence")
            } else if conflict {
                Some("conflict")
            } else {
                None
            };
            let block = self.cite_block(e.evidence_block_id.as_deref());
            let summary = format!(
                "{}: {} {} ({date})",
                e.document,
                kind.as_str(),
                e.place
                    .as_deref()
                    .or(e.person.as_deref())
                    .or(e.counterparty.as_deref())
                    .unwrap_or("")
            );
            let decision_id = DecisionId::generate(self.ids);
            let mut d = self.new_decision(DecisionKind::CustodyEvent);
            d.id = decision_id;
            d.source_block.clone_from(&block);
            d.target_type = "document".into();
            d.target_id = doc_id.map_or_else(String::new, |i| i.to_string());
            d.summary = summary;
            d.confidence = conf32(e.confidence);
            d.rel_type = Some(kind.as_str().to_owned());
            d.mention = Some(e.document.clone());
            d.detail = rmp(&CustodyDetail { date }).unwrap_or_default();
            match (reason, doc_id) {
                (None, Some(document_id)) => {
                    self.set.custody.push(AiCustody {
                        document: document_id,
                        kind,
                        date,
                        place: place.as_ref().and_then(Role::resolved),
                        person: person.as_ref().and_then(Role::resolved),
                        counterparty: counterparty.as_ref().and_then(Role::resolved),
                        cite: Some(Cite {
                            note: self.note.id,
                            block: block.clone(),
                        }),
                    });
                    self.touched_entities.insert(document_id);
                    self.decision(d);
                }
                (reason, _) => {
                    if self
                        .pending
                        .seen(custody_key(&e.document, kind.as_str(), date))
                    {
                        continue;
                    }
                    let payload = CustodyPayload {
                        decision_id: decision_id.as_ulid(),
                        source_note: self.note.id.as_ulid(),
                        block_id: block,
                        event: kind.as_str().to_owned(),
                        date,
                        document: document.target,
                        place: place.map(|r| r.target),
                        place_part_of: e.place_part_of.clone(),
                        person: person.map(|r| r.target),
                        counterparty: counterparty.map(|r| r.target),
                        confidence: e.confidence,
                        reason: reason.unwrap_or("unknown").to_owned(),
                        quote: e.quote.clone(),
                    };
                    let Ok(bytes) = rmp(&payload) else { continue };
                    let sid = self.suggest(decide::KIND_CUSTODY, bytes);
                    d.suggestion = Some(sid);
                    self.decision(d);
                }
            }
        }
    }

    /// Task suggestions (§6.11): always suggestions; the line is written on accept.
    pub fn tasks(&mut self, tasks: &[TaskProposal]) {
        for t in tasks {
            let title = t.title.trim();
            if title.is_empty() || self.pending.seen(task_key(title)) {
                continue;
            }
            let block_text = t
                .evidence_block_id
                .as_deref()
                .and_then(|b| self.note.block_text(b))
                .unwrap_or_default()
                .to_owned();
            let due = t.due.as_deref().map(|d| {
                dates::event_date(
                    &block_text,
                    self.note.created_date(),
                    Some(d),
                    t.date_source,
                )
            });
            let reminders: Vec<NaiveDateTime> = t
                .reminders
                .iter()
                .filter_map(|r| NaiveDateTime::parse_from_str(r, "%Y-%m-%d %H:%M").ok())
                .collect();
            let entities: Vec<NoteId> = t
                .entities
                .iter()
                .filter_map(|m| {
                    self.resolve_text(
                        m,
                        &[
                            NoteKind::Person,
                            NoteKind::Company,
                            NoteKind::Document,
                            NoteKind::Place,
                        ],
                    )
                    .ok()
                })
                .collect();
            let decision_id = DecisionId::generate(self.ids);
            let payload = TaskPayload {
                decision_id: decision_id.as_ulid(),
                source_note: self.note.id.as_ulid(),
                block_id: t.evidence_block_id.clone(),
                title: title.to_owned(),
                due,
                recurrence: t.recurrence.clone().filter(|r| !r.trim().is_empty()),
                reminders,
                entities: entities.iter().map(NoteId::as_ulid).collect(),
                confidence: t.confidence,
            };
            let Ok(bytes) = rmp(&payload) else { continue };
            let sid = self.suggest(decide::KIND_TASK, bytes);
            let mut d = self.new_decision(DecisionKind::TaskSuggestion);
            d.id = decision_id;
            d.source_block.clone_from(&t.evidence_block_id);
            d.target_type = "task".into();
            title.clone_into(&mut d.target_id);
            d.summary = format!("task: {title}");
            d.confidence = conf32(t.confidence);
            d.suggestion = Some(sid);
            self.decision(d);
        }
    }

    /// Removes the note's AI edges (note relations and mentions) this run did not return
    /// (§9.4 step 4); user edges are never touched.
    pub fn stale(&mut self) {
        for r in &self.note.sidecar.relations {
            let removable = matches!(r.kind, RelationKey::Note(_) | RelationKey::Mention(_));
            let dst = NoteId::from_ulid(r.target_id);
            if r.by == By::Ai && removable && !self.applied.contains(&(r.kind, dst)) {
                self.set.remove.push(EdgeRemove {
                    src: self.note.id,
                    rel: r.kind,
                    dst,
                    reject: false,
                });
                if matches!(r.kind, RelationKey::Mention(_)) {
                    self.touched_entities.insert(dst);
                }
            }
        }
    }

    /// Finishes the change set: block IDs to append, entity insights to refresh (debounced).
    pub fn finish(mut self, linked_before: &BTreeSet<NoteId>) -> AiChangeSet {
        for id in &self.needs_block {
            if let Some(text) = self.note.generated.get(id) {
                self.set.block_ids.push(BlockIdRequest {
                    note: self.note.id,
                    block_text: text.clone(),
                    id: id.clone(),
                });
            }
        }
        let mut entities = self.touched_entities.clone();
        entities.extend(linked_before.iter().copied());
        for e in entities {
            if self.dir.entity(e).is_none() {
                continue;
            }
            self.set.jobs.push(NewJob {
                id: JobId::generate(self.ids),
                kind: ENTITY_INSIGHTS.to_owned(),
                note_id: Some(e),
                payload: Vec::new(),
                run_after: self.now + INSIGHTS_DEBOUNCE,
                max_attempts: 5,
                dedupe_key: Some(e.to_string()),
            });
        }
        self.set
    }
}

fn map_custody(k: strata_ai::outputs::CustodyEventType) -> CustodyEventType {
    use strata_ai::outputs::CustodyEventType as A;
    match k {
        A::StoredAt => CustodyEventType::StoredAt,
        A::MovedTo => CustodyEventType::MovedTo,
        A::HandedTo => CustodyEventType::HandedTo,
        A::ReturnedBy => CustodyEventType::ReturnedBy,
        A::SentTo => CustodyEventType::SentTo,
        A::ReceivedFrom => CustodyEventType::ReceivedFrom,
        A::Lost => CustodyEventType::Lost,
        A::Found => CustodyEventType::Found,
        A::Destroyed => CustodyEventType::Destroyed,
    }
}

fn note_item(id: NoteId, title: &str, score: f64) -> DuplicateItem {
    DuplicateItem {
        id: id.as_ulid(),
        item: id.to_string(),
        snippet: None,
        kind: "note".to_owned(),
        title: title.to_owned(),
        match_level: dedupe::MatchLevel::Semantic,
        score,
    }
}

/// Whether the pair was already suggested as duplicates (any status) or kept as both.
async fn duplicates_known(
    db: &AppDb,
    scope: &UserScope,
    vault: &VaultService,
    a: NoteId,
    b: NoteId,
) -> Result<bool, JobError> {
    let mut tx = db.begin(scope).await?;
    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT payload FROM suggestions WHERE kind = 'duplicates' AND note_id = ANY($1)",
    )
    .bind([a, b])
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    let pair: BTreeSet<String> = [a.to_string(), b.to_string()].into();
    if rows.iter().any(|(p,)| {
        rmp_serde::from_slice::<DuplicatesPayload>(p)
            .is_ok_and(|d| [d.a.item, d.b.item].into_iter().collect::<BTreeSet<_>>() == pair)
    }) {
        return Ok(true);
    }
    for (x, y) in [(a, b), (b, a)] {
        if vault
            .note_sidecar(scope, x)
            .await?
            .is_some_and(|s| s.keep_both.iter().any(|k| k.other_id == y.as_ulid()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Named-map `MessagePack`.
pub fn rmp<T: Serialize>(v: &T) -> Result<Vec<u8>, JobError> {
    rmp_serde::to_vec_named(v).map_err(|e| JobError::Fatal(format!("payload: {e}")))
}

/// Normalised nickname mentions of an extraction (custody participants that are nicknames
/// resolve only through an existing alias).
pub fn nickname_set(mentions: &[Mention]) -> BTreeSet<String> {
    mentions
        .iter()
        .filter(|m| m.is_nickname)
        .map(|m| normalize_for_search(&m.text))
        .collect()
}

/// Titles of candidate notes by ID.
pub fn candidate_titles(c: &[CandidateInput]) -> BTreeMap<NoteId, String> {
    c.iter()
        .filter_map(|c| Some((c.id.parse().ok()?, c.title.clone())))
        .collect()
}

/// Entities currently linked from the note's frontmatter mention keys (for insight refresh).
pub fn linked_mentions(note: &SourceNote, dir: &Directory) -> BTreeSet<NoteId> {
    let _ = dir;
    note.sidecar
        .relations
        .iter()
        .filter(|r| {
            matches!(
                r.kind,
                RelationKey::Mention(MentionType::People | MentionType::Companies)
            )
        })
        .map(|r| NoteId::from_ulid(r.target_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_get_stable_generated_ids_and_keep_existing_ones() {
        let body = "## Call\nAhmed from Acme called.\n\nPrefers weekly invoicing. ^a1b2\n\n- [ ] Send it\n";
        let (blocks, generated) = note_blocks(body);
        let first = generated_block_id("Ahmed from Acme called.");
        let task = generated_block_id("- [ ] Send it");
        assert_eq!(
            blocks,
            vec![
                BlockInput {
                    block_id: first.clone(),
                    text: "Ahmed from Acme called.".into()
                },
                BlockInput {
                    block_id: "a1b2".into(),
                    text: "Prefers weekly invoicing.".into()
                },
                BlockInput {
                    block_id: task.clone(),
                    text: "- [ ] Send it".into()
                },
            ]
        );
        assert_eq!(generated.keys().cloned().collect::<Vec<_>>(), {
            let mut v = vec![first.clone(), task];
            v.sort();
            v
        });
        assert!(first.starts_with("b-") && first.len() == 8);
    }

    #[test]
    fn entity_selection_matches_names_across_scripts() {
        let e = |id: u128, kind: NoteKind, name: &str, aliases: &[&str]| EntityInfo {
            id: NoteId::from_ulid(ulid::Ulid::from_parts(1, id)),
            kind,
            name: name.into(),
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            parent: None,
        };
        let dir = Directory {
            entities: vec![
                e(1, NoteKind::Person, "Ahmed Samir", &["أحمد سمير"]),
                e(2, NoteKind::Company, "Watanya", &["وطنية"]),
                e(3, NoteKind::Person, "Shady", &[]),
            ],
            ..Directory::default()
        };
        let names = |text: &str| {
            dir.select(text, &BTreeSet::new())
                .into_iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names("كلمت أحمد سمير عن عقد وطنية"),
            vec!["Watanya", "Ahmed Samir"]
        );
        // Transliteration key: Latin spelling of an Arabic-only alias and vice versa.
        assert_eq!(names("Called Ahmad Sameer today"), vec!["Ahmed Samir"]);
        assert_eq!(names("اتكلمت مع شادي"), vec!["Shady"]);
        assert_eq!(names("nothing relevant"), Vec::<String>::new());
        assert_eq!(dir.by_alias("وطنيه", &[NoteKind::Company]).len(), 1);
    }

    #[test]
    fn keyword_queries_use_distinct_long_words() {
        assert_eq!(
            keyword_query("Pricing tiers: pricing for the big tiers"),
            Some("pricing OR tiers".into())
        );
        assert_eq!(keyword_query("a b c"), None);
    }
}
