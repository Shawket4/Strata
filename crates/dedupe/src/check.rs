//! The duplicate decision: thresholds, keep-both suppression, ranking.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use domain::SemanticThresholds;
use serde::{Deserialize, Serialize};

use crate::score::score_pair;
use crate::{DedupeKind, DedupeThresholds, Item, MatchLevel};

/// Stored kinds a new item of `kind` is compared with.
///
/// - Notes ↔ notes and captures; captures also ↔ tasks (a capture "remind me to …" duplicates
///   an existing task, §16.6).
/// - Tasks ↔ tasks and captures.
/// - People, companies, concepts, documents and places only ↔ their own kind.
/// - A new alias ↔ every named kind (its entity's own names are excluded by ID by the
///   caller).
pub const fn compatible_kinds(kind: DedupeKind) -> &'static [DedupeKind] {
    use DedupeKind as K;
    match kind {
        K::Note => &[K::Note, K::Capture],
        K::Capture => &[K::Capture, K::Note, K::Task],
        K::Task => &[K::Task, K::Capture],
        K::Person => &[K::Person],
        K::Company => &[K::Company],
        K::Concept => &[K::Concept],
        K::Document => &[K::Document],
        K::Place => &[K::Place],
        K::Alias => &[K::Person, K::Company, K::Concept, K::Document, K::Place],
    }
}

fn compatible(a: DedupeKind, b: DedupeKind) -> bool {
    compatible_kinds(a).contains(&b) || compatible_kinds(b).contains(&a)
}

/// Per-kind thresholds: the `domain` defaults, overridable from user settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    overrides: BTreeMap<DedupeKind, (f32, Option<(f32, f32)>)>,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self::new()
    }
}

impl Thresholds {
    /// The defaults from [`DedupeThresholds::default_for`].
    pub fn new() -> Self {
        Self {
            overrides: BTreeMap::new(),
        }
    }

    /// Overrides the thresholds of one kind (a user setting).
    #[must_use]
    pub fn with(mut self, kind: DedupeKind, t: DedupeThresholds) -> Self {
        self.overrides.insert(
            kind,
            (t.near, t.semantic.map(|s| (s.candidate, s.confirmed))),
        );
        self
    }

    /// The thresholds of `kind`.
    pub fn get(&self, kind: DedupeKind) -> DedupeThresholds {
        match self.overrides.get(&kind) {
            Some(&(near, semantic)) => DedupeThresholds {
                near,
                semantic: semantic.map(|(candidate, confirmed)| SemanticThresholds {
                    candidate,
                    confirmed,
                }),
            },
            None => DedupeThresholds::default_for(kind),
        }
    }

    /// Near threshold for a pair of kinds: the stricter of the two.
    pub fn near_between(&self, a: DedupeKind, b: DedupeKind) -> f32 {
        self.get(a).near.max(self.get(b).near)
    }

    /// Semantic thresholds for a pair of kinds: the stricter of each bound, `None` when
    /// either kind has no semantic matching.
    pub fn semantic_between(&self, a: DedupeKind, b: DedupeKind) -> Option<SemanticThresholds> {
        let (x, y) = (self.get(a).semantic?, self.get(b).semantic?);
        Some(SemanticThresholds {
            candidate: x.candidate.max(y.candidate),
            confirmed: x.confirmed.max(y.confirmed),
        })
    }
}

/// Embedding evidence the backend supplies for one candidate (the client core has no
/// embeddings and passes `None`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SemanticEvidence {
    /// Cosine similarity between the new item and the candidate.
    pub cosine: f32,
    /// Result of the LLM confirmation for a borderline score, when one was made.
    pub llm_confirmed: Option<bool>,
}

/// A stored candidate with optional semantic evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Existing {
    /// The stored item.
    pub item: Item,
    /// Embedding evidence against the new item.
    pub semantic: Option<SemanticEvidence>,
}

impl From<Item> for Existing {
    fn from(item: Item) -> Self {
        Self {
            item,
            semantic: None,
        }
    }
}

/// A "create anyway" decision for one pair (`dedupe_keep_both`). The IDs are stored ordered
/// (`a_id < b_id` in byte order, as the database does).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KeepBoth {
    /// Kind of the item that was force-created.
    pub kind: DedupeKind,
    /// Smaller ID.
    pub a_id: String,
    /// Larger ID.
    pub b_id: String,
}

impl KeepBoth {
    /// A pair in canonical order.
    pub fn new(kind: DedupeKind, x: &str, y: &str) -> Self {
        let (a, b) = if x <= y { (x, y) } else { (y, x) };
        Self {
            kind,
            a_id: a.to_owned(),
            b_id: b.to_owned(),
        }
    }

    /// The pairs to record when `new_id` is force-created despite `candidates`.
    pub fn for_forced_create(
        kind: DedupeKind,
        new_id: &str,
        candidates: &[DuplicateCandidate],
    ) -> Vec<Self> {
        let set: BTreeSet<Self> = candidates
            .iter()
            .filter(|c| c.id != new_id)
            .map(|c| Self::new(kind, new_id, &c.id))
            .collect();
        set.into_iter().collect()
    }
}

/// Keep-both pairs passed in by the caller. A pair suppresses a match when it was recorded
/// under the kind of either item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeepBothSet {
    pairs: BTreeSet<KeepBoth>,
}

impl KeepBothSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a pair.
    pub fn insert(&mut self, pair: KeepBoth) {
        self.pairs.insert(pair);
    }

    /// Whether `x` (of `kx`) and `y` (of `ky`) were marked "keep both".
    pub fn contains(&self, kx: DedupeKind, x: &str, ky: DedupeKind, y: &str) -> bool {
        self.pairs.contains(&KeepBoth::new(kx, x, y)) || self.pairs.contains(&KeepBoth::new(ky, x, y))
    }
}

impl FromIterator<KeepBoth> for KeepBothSet {
    fn from_iter<T: IntoIterator<Item = KeepBoth>>(iter: T) -> Self {
        Self {
            pairs: iter.into_iter().collect(),
        }
    }
}

/// One ranked duplicate candidate: the payload of the `duplicate_candidates` problem
/// (§7.5: id, kind, title, snippet, match level, score) and of the offline prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    /// ID of the existing item.
    pub id: String,
    /// Kind of the existing item.
    pub kind: DedupeKind,
    /// Title of the existing item.
    pub title: String,
    /// Snippet of the existing item.
    pub snippet: Option<String>,
    /// Strongest level that matched.
    pub level: MatchLevel,
    /// Score of that level (`1.0` exact, similarity for near, cosine for semantic).
    pub score: f32,
}

/// Result of [`check`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckOutcome {
    /// Duplicates, strongest first: by level (exact, near, semantic), then score
    /// (descending), then ID.
    pub candidates: Vec<DuplicateCandidate>,
    /// IDs with a borderline cosine and no LLM verdict yet (backend: confirm with one LLM
    /// call and check again). Sorted.
    pub needs_confirmation: Vec<String>,
    /// IDs that matched but were suppressed by a keep-both pair. Sorted.
    pub suppressed: Vec<String>,
}

impl CheckOutcome {
    /// Whether the create should be refused (`409 duplicate_candidates`) unless forced.
    pub fn is_duplicate(&self) -> bool {
        !self.candidates.is_empty()
    }
}

fn decide(
    new: &Item,
    existing: &Existing,
    thresholds: &Thresholds,
) -> Option<(MatchLevel, f32)> {
    let cand = &existing.item;
    let pair = score_pair(new, cand);
    if pair.exact {
        return Some((MatchLevel::Exact, 1.0));
    }
    let near = thresholds.near_between(new.kind, cand.kind);
    if pair.trigram >= near || pair.phonetic.is_some() {
        return Some((MatchLevel::Near, pair.score()));
    }
    let semantic = thresholds.semantic_between(new.kind, cand.kind)?;
    let evidence = existing.semantic?;
    let confirmed = evidence.cosine >= semantic.confirmed
        || (evidence.cosine >= semantic.candidate && evidence.llm_confirmed == Some(true));
    confirmed.then_some((MatchLevel::Semantic, evidence.cosine))
}

fn needs_llm(new: &Item, existing: &Existing, thresholds: &Thresholds) -> bool {
    let (Some(semantic), Some(evidence)) = (
        thresholds.semantic_between(new.kind, existing.item.kind),
        existing.semantic,
    ) else {
        return false;
    };
    evidence.llm_confirmed.is_none()
        && evidence.cosine >= semantic.candidate
        && evidence.cosine < semantic.confirmed
}

fn rank(a: &DuplicateCandidate, b: &DuplicateCandidate) -> Ordering {
    a.level
        .cmp(&b.level)
        .then_with(|| b.score.total_cmp(&a.score))
        .then_with(|| a.id.cmp(&b.id))
}

/// Decides which of `existing` duplicate `new`.
///
/// Candidates without an ID, with `new`'s own ID, or of an incompatible kind
/// ([`compatible_kinds`]) are ignored. A candidate listed twice counts once (its best match).
pub fn check(
    new: &Item,
    existing: &[Existing],
    thresholds: &Thresholds,
    keep_both: &KeepBothSet,
) -> CheckOutcome {
    let mut best: BTreeMap<String, DuplicateCandidate> = BTreeMap::new();
    let mut needs_confirmation = BTreeSet::new();
    let mut suppressed = BTreeSet::new();
    for e in existing {
        let cand = &e.item;
        let Some(id) = cand.id.as_deref() else {
            continue;
        };
        if new.id.as_deref() == Some(id) || !compatible(new.kind, cand.kind) {
            continue;
        }
        let Some((level, score)) = decide(new, e, thresholds) else {
            if needs_llm(new, e, thresholds) {
                needs_confirmation.insert(id.to_owned());
            }
            continue;
        };
        if let Some(new_id) = new.id.as_deref()
            && keep_both.contains(new.kind, new_id, cand.kind, id)
        {
            suppressed.insert(id.to_owned());
            continue;
        }
        let candidate = DuplicateCandidate {
            id: id.to_owned(),
            kind: cand.kind,
            title: cand.title.clone(),
            snippet: cand.snippet.clone(),
            level,
            score,
        };
        match best.get(id) {
            Some(prev) if rank(prev, &candidate) != Ordering::Greater => {}
            _ => {
                best.insert(id.to_owned(), candidate);
            }
        }
    }
    let mut candidates: Vec<DuplicateCandidate> = best.into_values().collect();
    candidates.sort_by(rank);
    for c in &candidates {
        needs_confirmation.remove(&c.id);
    }
    CheckOutcome {
        candidates,
        needs_confirmation: needs_confirmation.into_iter().collect(),
        suppressed: suppressed.into_iter().collect(),
    }
}

/// A duplicate pair found by [`sweep`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicatePair {
    /// First item ID (the smaller one).
    pub a_id: String,
    /// Second item ID.
    pub b_id: String,
    /// Level.
    pub level: MatchLevel,
    /// Score.
    pub score: f32,
}

/// Exact and near duplicate pairs within a (caller-blocked) candidate set, for the nightly
/// sweep (§9.2 `dedupe`), skipping keep-both pairs. Pairs are ordered like
/// [`CheckOutcome::candidates`], then by IDs. Quadratic in `items.len()`.
pub fn sweep(items: &[Item], thresholds: &Thresholds, keep_both: &KeepBothSet) -> Vec<DuplicatePair> {
    let mut out = Vec::new();
    for (i, x) in items.iter().enumerate() {
        for y in &items[i + 1..] {
            let (Some(xi), Some(yi)) = (x.id.as_deref(), y.id.as_deref()) else {
                continue;
            };
            if xi == yi || !compatible(x.kind, y.kind) || keep_both.contains(x.kind, xi, y.kind, yi) {
                continue;
            }
            if let Some((level, score)) = decide(x, &Existing::from(y.clone()), thresholds) {
                let (a, b) = if xi <= yi { (xi, yi) } else { (yi, xi) };
                out.push(DuplicatePair {
                    a_id: a.to_owned(),
                    b_id: b.to_owned(),
                    level,
                    score,
                });
            }
        }
    }
    out.sort_by(|p, q| {
        p.level
            .cmp(&q.level)
            .then_with(|| q.score.total_cmp(&p.score))
            .then_with(|| p.a_id.cmp(&q.a_id))
            .then_with(|| p.b_id.cmp(&q.b_id))
    });
    out
}
