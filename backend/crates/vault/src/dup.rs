//! Duplicate check on create (PLAN §9.7, exact + near levels; semantic is Phase 4).
//!
//! Candidates come from SQL: equal exact keys and `pg_trgm` similarity over `dedupe_keys`,
//! plus alias similarity over `entity_aliases` for named kinds (both scripts: every alias is
//! stored normalised). Keys are computed by `text-normalize` exactly as the index stores
//! them (see [`crate::derive`]), so the client core's offline check agrees.

use std::collections::BTreeMap;

use domain::{DedupeKind, DedupeThresholds};
use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::repo::{dedupe, notes, tasks, vault as vrepo};
use strata_index::types::EntityKind;
use text_normalize::{dedupe_key, normalize_for_search};

use crate::error::{Candidate, MatchLevel, Result};
use crate::prepare::task_ulid;

/// The item being created.
#[derive(Debug, Clone)]
pub struct NewItem<'a> {
    /// Dedupe kind (`note`, `capture`, `task`, `person`, …).
    pub kind: &'a str,
    /// Its ID, when known (client-supplied): keep-both pairs with it are skipped.
    pub id: Option<String>,
    /// Compared text (title, capture text, task description).
    pub text: &'a str,
    /// Exact key override (tasks: text + recurrence + entities).
    pub exact: Option<String>,
    /// Further names (entity aliases).
    pub aliases: &'a [String],
}

fn compatible(kind: &str) -> &'static [&'static str] {
    match kind {
        "capture" => &["capture", "note"],
        "note" => &["note"],
        "task" => &["task"],
        "person" => &["person"],
        "company" => &["company"],
        "document" => &["document"],
        "place" => &["place"],
        "concept" => &["concept"],
        _ => &[],
    }
}

fn entity_kind(kind: &str) -> Option<EntityKind> {
    match kind {
        "person" => Some(EntityKind::Person),
        "company" => Some(EntityKind::Company),
        "document" => Some(EntityKind::Document),
        "place" => Some(EntityKind::Place),
        _ => None,
    }
}

/// The near threshold of a kind: configured override, else the `domain` default.
pub fn near_threshold(kind: &str, overrides: &BTreeMap<String, f32>) -> f32 {
    if let Some(t) = overrides.get(kind) {
        return *t;
    }
    kind.parse::<DedupeKind>()
        .map_or(0.6, |k| DedupeThresholds::default_for(k).near)
}

fn snippet_of(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let mut s: String = line.chars().take(120).collect();
    if line.chars().count() > 120 {
        s.push('…');
    }
    Some(s)
}

/// Existing items the new one resembles, exact matches first, then by score and ID.
pub async fn find(
    tx: &mut ScopedTx,
    item: &NewItem<'_>,
    overrides: &BTreeMap<String, f32>,
) -> Result<Vec<Candidate>> {
    let threshold = near_threshold(item.kind, overrides);
    let exact = item
        .exact
        .clone()
        .unwrap_or_else(|| dedupe_key(item.text));
    let trigram = normalize_for_search(&crate::derive::strip_links(item.text));
    // (kind, item id) -> (level, score)
    let mut found: BTreeMap<(String, String), (MatchLevel, f64)> = BTreeMap::new();
    for kind in compatible(item.kind) {
        if !exact.trim_matches('|').is_empty() {
            for id in dedupe::exact_matches(tx, kind, &exact).await? {
                found.insert(((*kind).to_owned(), id), (MatchLevel::Exact, 1.0));
            }
        }
        if !trigram.is_empty() {
            for m in dedupe::near_matches(tx, kind, &trigram, threshold, 20).await? {
                found
                    .entry(((*kind).to_owned(), m.item_id))
                    .or_insert((MatchLevel::Near, f64::from(m.score)));
            }
        }
    }
    if let Some(ek) = entity_kind(item.kind) {
        let names = std::iter::once(item.text).chain(item.aliases.iter().map(String::as_str));
        for name in names {
            let q = normalize_for_search(name);
            if q.is_empty() {
                continue;
            }
            for hit in vrepo::search_entities(tx, Some(ek), Some(&q), None, threshold, 20).await? {
                let score = f64::from(hit.score);
                if score + 1e-6 < f64::from(threshold) {
                    continue;
                }
                let level = if score >= 0.999 {
                    MatchLevel::Exact
                } else {
                    MatchLevel::Near
                };
                let key = (item.kind.to_owned(), hit.note_id.to_string());
                let entry = found.entry(key).or_insert((level, score));
                if level == MatchLevel::Exact {
                    *entry = (MatchLevel::Exact, 1.0);
                } else if entry.0 == MatchLevel::Near && score > entry.1 {
                    entry.1 = score;
                }
            }
        }
    }
    let mut out = Vec::new();
    for ((kind, id), (level, score)) in found {
        if item.id.as_deref() == Some(id.as_str()) {
            continue;
        }
        if let Some(own) = &item.id
            && dedupe::is_keep_both(tx, &kind, own, &id).await?
        {
            continue;
        }
        let candidate = if kind == "task" {
            let Some(t) = tasks::get_task(tx, &id).await? else {
                continue;
            };
            Candidate {
                id: task_ulid(&t.id).unwrap_or_else(|| t.note_id.as_ulid()),
                kind,
                title: crate::derive::strip_links(&t.text),
                snippet: t.due.map(|d| format!("due {}", d.format("%Y-%m-%d"))),
                level,
                score,
            }
        } else {
            let Ok(note_id) = id.parse::<NoteId>() else {
                continue;
            };
            let Some(n) = notes::get_note(tx, note_id).await? else {
                continue;
            };
            if n.trashed {
                continue;
            }
            Candidate {
                id: note_id.as_ulid(),
                snippet: None,
                kind,
                title: n.title,
                level,
                score,
            }
        };
        out.push(candidate);
    }
    out.sort_by(|a, b| {
        let rank = |c: &Candidate| u8::from(c.level != MatchLevel::Exact);
        rank(a)
            .cmp(&rank(b))
            .then(b.score.total_cmp(&a.score))
            .then(a.id.cmp(&b.id))
    });
    let _ = snippet_of;
    Ok(out)
}

/// The capture snippet of a candidate text.
pub fn capture_snippet(text: &str) -> Option<String> {
    snippet_of(text)
}
