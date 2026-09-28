//! Duplicate check on create (PLAN §9.7): exact and near levels always; the semantic level
//! when a [`SemanticDuplicates`] source is registered and can embed the new text cheaply.
//!
//! The decision is the shared `dedupe` crate's (L16), so the server's check and the client
//! core's offline check always agree:
//!
//! 1. Every note and open task is stored as a `dedupe::Item` with its
//!    [`keys`](dedupe::Item::keys) in `dedupe_keys` (one row per name) — see
//!    [`note_item`], [`task_item`], [`rows`].
//! 2. A new item's [`CandidateQuery`] selects candidates with SQL (exact and transliteration
//!    keys, `pg_trgm` similarity above the query's floor).
//! 3. With a [`SemanticDuplicates`] source (the AI subsystem), stored items whose vector is
//!    close to the new text's get [`dedupe::SemanticEvidence`] (cosine only: a borderline
//!    score is not confirmed by an LLM on the synchronous path, so it does not block).
//! 4. [`dedupe::check`] decides, with keep-both pairs from `dedupe_keep_both`.

use std::collections::BTreeMap;

use dedupe::{CandidateQuery, DedupeKind, DedupeThresholds, Existing, Item, KeepBoth, KeepBothSet};
use domain::NoteKind;
use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::repo::tasks;
use strata_index::repo::vault::{self as vrepo, DedupeKeyRow};
use vault_format::Document;
use vault_format::tasks::TaskLine;

use crate::derive::{DedupeRow, dedupe_kind, kind_of, title_of};
use crate::error::{Candidate, MatchLevel, Result};
use crate::prepare::task_ulid;
pub use crate::semantic::{SemanticDuplicates, SemanticMatch};

/// The first non-empty line of `text`, at most 120 characters.
pub fn snippet_of(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut s: String = line.chars().take(120).collect();
    if line.chars().count() > 120 {
        s.push('…');
    }
    Some(s)
}

/// The dedupe item of the note at `path`: notes by title, captures by their text, concepts
/// and entities by name and aliases.
pub fn note_item(path: &str, id: Option<NoteId>, doc: &Document) -> Item {
    let id_text = id.map(|i| i.to_string());
    let id = id_text.as_deref();
    let kind = kind_of(doc);
    let title = title_of(path, doc);
    let aliases: Vec<String> = doc
        .frontmatter()
        .map(vault_format::Frontmatter::aliases)
        .unwrap_or_default();
    let alias_refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
    match kind {
        NoteKind::Note if dedupe_kind(path, kind) == "capture" => {
            let text = doc.body().trim();
            let item = Item::capture(id, text);
            match snippet_of(text) {
                Some(s) => item.with_snippet(&s),
                None => item,
            }
        }
        NoteKind::Note => Item::note(id, &title),
        NoteKind::Concept => Item::concept(id, &title, &alias_refs),
        NoteKind::Person => Item::entity(DedupeKind::Person, id, &title, &alias_refs),
        NoteKind::Company => Item::entity(DedupeKind::Company, id, &title, &alias_refs),
        NoteKind::Document => Item::entity(DedupeKind::Document, id, &title, &alias_refs),
        NoteKind::Place => Item::entity(DedupeKind::Place, id, &title, &alias_refs),
    }
}

/// The dedupe item of a task line: description, compiled recurrence and linked entities.
pub fn task_item(id: &str, task: &TaskLine) -> Item {
    let description = task.description();
    let rrule = crate::ops::tasks::rrule_of(task);
    let entities: Vec<String> = vault_format::wikilink::find_all(description)
        .iter()
        .map(|l| l.target().to_owned())
        .collect();
    let refs: Vec<&str> = entities.iter().map(String::as_str).collect();
    let item = Item::task(Some(id), description, rrule.as_deref(), &refs);
    match task.date(vault_format::tasks::DateKind::Due) {
        Some(d) => item.with_snippet(&format!("due {}", d.format("%Y-%m-%d"))),
        None => item,
    }
}

/// The `dedupe_keys` rows of an item (it must have an ID).
pub fn rows(item: &Item) -> DedupeRow {
    let keys = item.keys();
    let blob = rmp_serde::to_vec_named(item).unwrap_or_default();
    let n = keys.exact_keys.len().max(keys.trigram_texts.len()).max(1);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(DedupeKeyRow {
            key_no: i16::try_from(i).unwrap_or(i16::MAX),
            exact_key: keys.exact_keys.get(i).cloned().unwrap_or_default(),
            trigram_text: keys.trigram_texts.get(i).cloned().unwrap_or_default(),
            phonetic_key: keys.phonetic_keys.get(i).cloned(),
            item: blob.clone(),
        });
    }
    DedupeRow {
        kind: item.kind.as_str().to_owned(),
        item_id: item.id.clone().unwrap_or_default(),
        rows: out,
    }
}

/// Per-kind thresholds: the `domain` defaults with configured near-threshold overrides.
pub fn thresholds(overrides: &BTreeMap<String, f32>) -> dedupe::Thresholds {
    let mut t = dedupe::Thresholds::new();
    for (kind, near) in overrides {
        if let Ok(k) = kind.parse::<DedupeKind>() {
            let d = DedupeThresholds::default_for(k);
            t = t.with(k, DedupeThresholds { near: *near, ..d });
        }
    }
    t
}

/// The duplicate candidates of `item` (strongest first), keep-both pairs excluded: exact and
/// near levels (see [`find_with`] for the semantic level).
pub async fn find(
    tx: &mut ScopedTx,
    item: &Item,
    overrides: &BTreeMap<String, f32>,
) -> Result<Vec<Candidate>> {
    find_with(tx, item, overrides, None).await
}

/// [`find`] plus the semantic level from `semantic`, when given.
pub async fn find_with(
    tx: &mut ScopedTx,
    item: &Item,
    overrides: &BTreeMap<String, f32>,
    semantic: Option<&dyn SemanticDuplicates>,
) -> Result<Vec<Candidate>> {
    let thresholds = thresholds(overrides);
    let q = CandidateQuery::for_item(item, &thresholds);
    let kinds: Vec<String> = q.kinds.iter().map(|k| k.as_str().to_owned()).collect();
    let pt = crate::prof::g("dup.candidates_sql");
    let blobs = vrepo::dedupe_candidates(
        tx,
        &kinds,
        &q.exact_keys,
        &q.phonetic_keys,
        &q.trigram_texts,
        q.trigram_floor.clamp(0.0, 1.0),
        200,
    )
    .await?;
    drop(pt);
    crate::prof::add(
        "dup.blobs(us=count)",
        std::time::Duration::from_micros(blobs.len() as u64),
    );
    let pt = crate::prof::g("dup.semantic");
    let mut existing: Vec<Existing> = blobs
        .iter()
        .filter_map(|b| rmp_serde::from_slice::<Item>(b).ok())
        .map(Existing::from)
        .collect();
    if let Some(source) = semantic {
        for m in source.evidence(tx, item).await {
            let evidence = dedupe::SemanticEvidence {
                cosine: m.cosine,
                llm_confirmed: None,
            };
            match existing
                .iter_mut()
                .find(|e| e.item.kind == m.item.kind && e.item.id == m.item.id)
            {
                Some(e) => e.semantic = Some(evidence),
                None => existing.push(Existing {
                    item: m.item,
                    semantic: Some(evidence),
                }),
            }
        }
    }
    drop(pt);
    let pt = crate::prof::g("dup.keepboth+check");
    let mut keep = KeepBothSet::new();
    if let Some(own) = &item.id {
        for (kind, a, b) in vrepo::keep_both_pairs_of(tx, own).await? {
            if let Ok(k) = kind.parse::<DedupeKind>() {
                keep.insert(KeepBoth::new(k, &a, &b));
            }
        }
    }
    let outcome = dedupe::check(item, &existing, &thresholds, &keep);
    drop(pt);
    let mut out = Vec::with_capacity(outcome.candidates.len());
    for c in outcome.candidates {
        let id = match c.id.parse::<NoteId>() {
            Ok(n) => n.as_ulid(),
            Err(_) => match task_ulid(&c.id) {
                Some(u) => u,
                None => match tasks::get_task(tx, &c.id).await? {
                    Some(t) => t.note_id.as_ulid(),
                    None => continue,
                },
            },
        };
        out.push(Candidate {
            item: c.id,
            id,
            kind: c.kind.as_str().to_owned(),
            title: c.title,
            snippet: c.snippet,
            level: match c.level {
                dedupe::MatchLevel::Exact => MatchLevel::Exact,
                dedupe::MatchLevel::Near | dedupe::MatchLevel::Semantic => MatchLevel::Near,
            },
            score: f64::from(c.score),
            semantic: c.level == dedupe::MatchLevel::Semantic,
        });
    }
    Ok(out)
}

/// The keep-both pairs to record when `item` is created despite `candidates`.
pub fn forced_pairs(item: &Item, candidates: &[Candidate]) -> Vec<KeepBoth> {
    let own = item.id.clone().unwrap_or_default();
    let dc: Vec<dedupe::DuplicateCandidate> = candidates
        .iter()
        .map(|c| dedupe::DuplicateCandidate {
            id: c.item.clone(),
            kind: c.kind.parse().unwrap_or(item.kind),
            title: c.title.clone(),
            snippet: c.snippet.clone(),
            level: match c.level {
                MatchLevel::Exact => dedupe::MatchLevel::Exact,
                MatchLevel::Near if c.semantic => dedupe::MatchLevel::Semantic,
                MatchLevel::Near => dedupe::MatchLevel::Near,
            },
            #[allow(clippy::cast_possible_truncation)]
            score: c.score as f32,
        })
        .collect();
    KeepBoth::for_forced_create(item.kind, &own, &dc)
}
