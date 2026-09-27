//! Typed outputs of the structured prompts. Each type mirrors `prompts/<id>.v1.schema.json`
//! (the schema is authoritative and validated first; these types are what jobs consume).

use serde::{Deserialize, Serialize};

/// Dominant language of a note (`lang:` frontmatter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Arabic.
    Ar,
    /// English.
    En,
    /// Neither dominates.
    Mixed,
}

/// Note-to-note relation types (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationType {
    /// Shared subject.
    Related,
    /// Source is part of target.
    PartOf,
    /// Source supports target.
    Supports,
    /// Source contradicts target.
    Contradicts,
    /// Source follows up on target.
    FollowsUp,
    /// Same content (never auto-applied).
    Duplicates,
}

/// A proposed relation to an existing note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationProposal {
    /// Target note ID (from the candidates).
    pub target_id: String,
    /// Relation type.
    #[serde(rename = "type")]
    pub kind: RelationType,
    /// 0..1.
    pub confidence: f64,
    /// One sentence shown to the user.
    pub reason: String,
}

/// A concept to link (existing) or create (new).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConceptProposal {
    /// Concept name.
    pub name: String,
    /// Existing concept note ID, if it matched one.
    pub existing_id: Option<String>,
    /// 0..1.
    pub confidence: f64,
}

/// `inbox_filing.v1` (§9.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxFiling {
    /// Proposed title (file name).
    pub title: String,
    /// Proposed tags.
    pub tags: Vec<String>,
    /// Destination folder.
    pub destination_folder: String,
    /// Dominant language.
    pub lang: Lang,
    /// Relations to candidates.
    pub relations: Vec<RelationProposal>,
    /// Concepts.
    pub concepts: Vec<ConceptProposal>,
    /// People the capture involves.
    pub people: Vec<PersonProposal>,
    /// Companies the capture involves.
    pub companies: Vec<CompanyProposal>,
}

/// A person mention in filing output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonProposal {
    /// Name as written.
    pub name: String,
    /// Existing entity, if exactly one matched.
    pub existing_id: Option<String>,
    /// Nickname or kinship term (never auto-created, §6.7).
    pub is_nickname: bool,
    /// 0..1.
    pub confidence: f64,
}

/// A company mention in filing output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompanyProposal {
    /// Name as written.
    pub name: String,
    /// Existing entity, if exactly one matched.
    pub existing_id: Option<String>,
    /// 0..1.
    pub confidence: f64,
}

/// Entity kinds a mention can resolve to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MentionKind {
    /// Person.
    Person,
    /// Company.
    Company,
    /// Document.
    Document,
    /// Place.
    Place,
}

/// An entity mention (§6.7 resolution rules).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    /// Exact mention text.
    pub text: String,
    /// Kind.
    pub kind: MentionKind,
    /// Resolved entity, when exactly one matched.
    pub existing_id: Option<String>,
    /// Several plausible entities (ambiguous → suggestion).
    pub candidate_ids: Vec<String>,
    /// Nickname or kinship term.
    pub is_nickname: bool,
    /// 0..1.
    pub confidence: f64,
    /// Block stating it (`None` = title).
    pub evidence_block_id: Option<String>,
}

/// Entity-to-entity relation types (§6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntityRelationType {
    /// Person works at company.
    WorksAt,
    /// Person worked at company.
    WorkedAt,
    /// Person reports to person.
    ReportsTo,
    /// Person knows person.
    Knows,
    /// Person introduced by person.
    IntroducedBy,
    /// Company is a client of company.
    ClientOf,
    /// Company supplies company.
    SupplierOf,
    /// Partners.
    PartnerOf,
    /// Competitors.
    CompetitorOf,
    /// Subsidiary.
    SubsidiaryOf,
}

/// A stated relation between two mentions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityRelation {
    /// Source mention text.
    pub from: String,
    /// Type.
    #[serde(rename = "type")]
    pub kind: EntityRelationType,
    /// Target mention text.
    pub to: String,
    /// 0..1.
    pub confidence: f64,
    /// One sentence.
    pub reason: String,
    /// Block stating it.
    pub evidence_block_id: Option<String>,
}

/// Custody event types (§6.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustodyEventType {
    /// Kept at a place.
    StoredAt,
    /// Moved to a place.
    MovedTo,
    /// Given to a person.
    HandedTo,
    /// Given back by a person.
    ReturnedBy,
    /// Sent to a third party.
    SentTo,
    /// Received from a third party.
    ReceivedFrom,
    /// Lost.
    Lost,
    /// Found.
    Found,
    /// Destroyed.
    Destroyed,
}

/// How a date was obtained (§6.7 timeline-date rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateSource {
    /// Written in the text.
    Explicit,
    /// Relative phrase resolved against the note's `created`.
    Relative,
    /// The note's `created` date.
    Created,
}

/// One extracted custody event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustodyEvent {
    /// Event type.
    #[serde(rename = "type")]
    pub kind: CustodyEventType,
    /// Document mention.
    pub document: String,
    /// Resolved document.
    pub document_existing_id: Option<String>,
    /// Place mention (innermost).
    pub place: Option<String>,
    /// Resolved place.
    pub place_existing_id: Option<String>,
    /// Enclosing place mention.
    pub place_part_of: Option<String>,
    /// Person mention.
    pub person: Option<String>,
    /// Resolved person.
    pub person_existing_id: Option<String>,
    /// Third party mention.
    pub counterparty: Option<String>,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// How the date was obtained.
    pub date_source: DateSource,
    /// 0..1.
    pub confidence: f64,
    /// Block stating it.
    pub evidence_block_id: Option<String>,
    /// Exact span stating it.
    pub quote: String,
}

/// A task suggestion (§6.11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskProposal {
    /// Imperative title.
    pub title: String,
    /// `YYYY-MM-DD`.
    pub due: Option<String>,
    /// How `due` was obtained.
    pub date_source: Option<DateSource>,
    /// Tasks-plugin recurrence phrase.
    pub recurrence: Option<String>,
    /// `YYYY-MM-DD HH:mm` local reminder times.
    pub reminders: Vec<String>,
    /// Mention texts the task concerns.
    pub entities: Vec<String>,
    /// 0..1.
    pub confidence: f64,
    /// Block stating it.
    pub evidence_block_id: Option<String>,
}

/// `linking.v1` (§9.4, plus concepts, entities, custody and task suggestions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Linking {
    /// Relations to candidates.
    pub relations: Vec<RelationProposal>,
    /// Concepts.
    pub concepts: Vec<ConceptProposal>,
    /// Entity mentions.
    pub mentions: Vec<Mention>,
    /// Relations between mentioned entities.
    pub entity_relations: Vec<EntityRelation>,
    /// Custody events.
    pub custody: Vec<CustodyEvent>,
    /// Task suggestions.
    pub tasks: Vec<TaskProposal>,
}

/// `summary.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// One paragraph.
    pub summary: String,
    /// Dominant language.
    pub lang: Lang,
}

/// A citation of one block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    /// Note ID.
    pub note_id: String,
    /// Block ID.
    pub block_id: String,
}

/// A bullet with at least one citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitedItem {
    /// Bullet text.
    pub text: String,
    /// Sources (non-empty).
    pub citations: Vec<Citation>,
}

/// A dated, cited timeline entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// One line.
    pub text: String,
    /// Sources (non-empty).
    pub citations: Vec<Citation>,
}

/// `entity_insights.v1` (§6.7 AI sections).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityInsights {
    /// 2–4 sentences.
    pub summary: String,
    /// Durable, cited facts.
    pub insights: Vec<CitedItem>,
    /// Unresolved asks and promises.
    pub open_items: Vec<CitedItem>,
    /// Newest first.
    pub timeline: Vec<TimelineEntry>,
}

/// `custody.v1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustodyExtraction {
    /// Events, oldest first.
    pub events: Vec<CustodyEvent>,
}

/// Correction actions (§9.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CorrectionAction {
    /// Same mention, different target.
    Repoint,
    /// Different relation type.
    Retype,
    /// Remove the decision.
    Reject,
}

/// One proposed fix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionFix {
    /// The AI decision to fix.
    pub decision_id: String,
    /// Action.
    pub action: CorrectionAction,
    /// New target for `repoint`.
    pub new_target_id: Option<String>,
    /// New type for `retype`.
    pub new_type: Option<String>,
    /// 0..1.
    pub confidence: f64,
    /// One sentence.
    pub reason: String,
}

/// A disambiguation hint to remember (§9.8 memory).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisambiguationHint {
    /// Entity the hint is about.
    pub entity_id: String,
    /// Hint text.
    pub text: String,
}

/// `correction.v1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Correction {
    /// Whether the message corrects a decision.
    pub is_correction: bool,
    /// Fixes.
    pub fixes: Vec<CorrectionFix>,
    /// Hints.
    pub hints: Vec<DisambiguationHint>,
    /// Several decisions or targets fit.
    pub ambiguous: bool,
    /// Question to ask the user when ambiguous.
    pub clarification_question: Option<String>,
}

/// Duplicate verdicts (§9.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateVerdict {
    /// Same thing.
    Duplicate,
    /// Different things.
    Distinct,
    /// Cannot tell.
    Uncertain,
}

/// `duplicate_confirm.v1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateConfirmation {
    /// Verdict.
    pub verdict: DuplicateVerdict,
    /// 0..1.
    pub confidence: f64,
    /// One sentence shown to the user.
    pub reason: String,
}

/// One cluster name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterName {
    /// Cluster.
    pub cluster_id: String,
    /// 1–4 words.
    pub name: String,
}

/// `cluster_naming.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNames {
    /// One per input cluster.
    pub names: Vec<ClusterName>,
}

/// `digest.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Digest {
    /// Title.
    pub title: String,
    /// Dominant language.
    pub lang: Lang,
    /// Most important new information.
    pub highlights: Vec<CitedItem>,
    /// Open questions and items.
    pub open_questions: Vec<CitedItem>,
    /// Contradictions, neutrally stated.
    pub contradictions: Vec<CitedItem>,
}

/// Citations `[[ref]]` in an Ask answer, in order of appearance (duplicates kept). Refs are
/// `Note title#^block-id` strings as given in the prompt's `sources`.
pub fn ask_citations(answer: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = answer;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        if !inner.contains('\n') && !inner.is_empty() {
            out.push(inner);
        }
        rest = &after[end + 2..];
    }
    out
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::prompts::{self, ids};

    fn check<T: serde::de::DeserializeOwned + Serialize>(id: &str, sample: &str) -> T {
        let p = prompts::latest(id).expect("registered");
        let schema = p.schema_value().expect("schema");
        let v = crate::schema::compile(&schema).expect("compiles");
        let value: serde_json::Value = serde_json::from_str(sample).expect("sample is JSON");
        crate::schema::validate(&v, &value).expect("sample matches the schema");
        let typed: T = serde_json::from_value(value.clone()).expect("sample fits the type");
        // The type round-trips to exactly the validated JSON (no field lost or renamed).
        assert_eq!(serde_json::to_value(&typed).expect("serialize"), value, "{id}");
        typed
    }

    #[test]
    fn sample_outputs_validate_and_round_trip_through_the_types() {
        let f: InboxFiling = check(ids::INBOX_FILING, include_str!("../tests/fixtures/outputs/inbox_filing.v1.json"));
        assert_eq!(f.lang, Lang::Mixed);
        let l: Linking = check(ids::LINKING, include_str!("../tests/fixtures/outputs/linking.v1.json"));
        assert_eq!(l.custody[1].kind, CustodyEventType::StoredAt);
        let s: Summary = check(ids::SUMMARY, include_str!("../tests/fixtures/outputs/summary.v1.json"));
        assert_eq!(s.lang, Lang::Ar);
        let e: EntityInsights = check(ids::ENTITY_INSIGHTS, include_str!("../tests/fixtures/outputs/entity_insights.v1.json"));
        assert_eq!(e.timeline[0].date, "2026-09-28");
        let c: CustodyExtraction = check(ids::CUSTODY, include_str!("../tests/fixtures/outputs/custody.v1.json"));
        assert_eq!(c.events.len(), 2);
        let k: Correction = check(ids::CORRECTION, include_str!("../tests/fixtures/outputs/correction.v1.json"));
        assert_eq!(k.fixes[0].action, CorrectionAction::Repoint);
        let d: DuplicateConfirmation = check(ids::DUPLICATE_CONFIRM, include_str!("../tests/fixtures/outputs/duplicate_confirm.v1.json"));
        assert_eq!(d.verdict, DuplicateVerdict::Duplicate);
        let n: ClusterNames = check(ids::CLUSTER_NAMING, include_str!("../tests/fixtures/outputs/cluster_naming.v1.json"));
        assert_eq!(n.names.len(), 2);
        let g: Digest = check(ids::DIGEST, include_str!("../tests/fixtures/outputs/digest.v1.json"));
        assert_eq!(g.title, "Weekly digest 2026-W39");
    }

    #[test]
    fn uncited_insight_fails_schema_validation() {
        let p = prompts::latest(ids::ENTITY_INSIGHTS).expect("registered");
        let v = crate::schema::compile(&p.schema_value().expect("schema")).expect("compiles");
        let value = serde_json::json!({
            "summary": "Operations manager at Acme.",
            "insights": [{"text": "Prefers weekly invoicing", "citations": []}],
            "open_items": [],
            "timeline": []
        });
        let errs = crate::schema::validate(&v, &value).expect_err("uncited");
        assert_eq!(
            errs.iter().map(crate::schema::Violation::summary).collect::<Vec<_>>(),
            vec!["/insights/0/citations: value has less than 1 item".to_owned()]
        );
    }

    #[test]
    fn ask_citations_are_extracted_in_order() {
        let answer = "The contract is in the safe [[Capture 2026-09-20#^c1d2]]. Shady had it last \
                      [[Call 2026-09-12#^a1b2]][[Capture 2026-09-20#^c1d2]]. Broken [[x";
        assert_eq!(
            ask_citations(answer),
            vec![
                "Capture 2026-09-20#^c1d2",
                "Call 2026-09-12#^a1b2",
                "Capture 2026-09-20#^c1d2"
            ]
        );
        assert_eq!(ask_citations("no refs [[]] here"), Vec::<&str>::new());
    }
}
