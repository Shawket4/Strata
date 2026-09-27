//! The offline duplicate check (§9.7 "Offline"): before queuing a create, the core runs the
//! shared `dedupe` exact + near check against its local cache and shows the same prompt the
//! server would. Candidate generation mirrors the server's SQL lookup with
//! [`dedupe::CandidateQuery`]: equal exact or phonetic keys, or trigram similarity of the
//! prepared texts above the query's floor; the decision is `dedupe::check`.

use rusqlite::Connection;
use dedupe::{CandidateQuery, DedupeKind, DuplicateCandidate, Existing, Item, KeepBoth, KeepBothSet, Thresholds};

use crate::error::CoreResult;

fn items_of_kind(conn: &Connection, kind: DedupeKind) -> CoreResult<Vec<Item>> {
    let mut out = Vec::new();
    match kind {
        DedupeKind::Note | DedupeKind::Concept | DedupeKind::Capture => {
            let (note_kind, inbox) = match kind {
                DedupeKind::Note => ("note", false),
                DedupeKind::Concept => ("concept", false),
                _ => ("note", true),
            };
            let mut st = conn.prepare(
                "SELECT id, title, content FROM notes
                 WHERE deleted = 0 AND kind = ?1 AND (path LIKE 'inbox/%') = ?2 ORDER BY id",
            )?;
            let rows: Vec<(String, String, String)> = st
                .query_map(rusqlite::params![note_kind, inbox], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<Result<_, _>>()?;
            for (id, title, content) in rows {
                out.push(match kind {
                    DedupeKind::Note => Item::note(Some(&id), &title),
                    DedupeKind::Concept => Item::concept(Some(&id), &title, &[]),
                    _ => {
                        let doc = vault_format::Document::parse(&content);
                        Item::capture(Some(&id), doc.body().trim())
                    }
                });
            }
        }
        DedupeKind::Task => {
            let mut st = conn.prepare(
                "SELECT t.id, t.description, t.rrule, t.line FROM tasks t
                 JOIN notes n ON n.id = t.note_id
                 WHERE n.deleted = 0 AND t.status = 'open' ORDER BY t.id",
            )?;
            let rows: Vec<(String, String, Option<String>, String)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<_, _>>()?;
            for (id, text, rrule, line) in rows {
                let links: Vec<String> = vault_format::wikilink::find_all(&line)
                    .iter()
                    .map(|l| vault_format::resolve::link_name(l.target()).to_owned())
                    .collect();
                let entities: Vec<&str> = links.iter().map(String::as_str).collect();
                out.push(Item::task(Some(&id), &text, rrule.as_deref(), &entities));
            }
        }
        DedupeKind::Person | DedupeKind::Company | DedupeKind::Document | DedupeKind::Place => {
            let mut st = conn.prepare(
                "SELECT e.note_id, e.display_name FROM entities e JOIN notes n ON n.id = e.note_id
                 WHERE n.deleted = 0 AND e.kind = ?1 ORDER BY e.note_id",
            )?;
            let rows: Vec<(String, String)> = st
                .query_map([kind.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let mut aliases_st = conn.prepare(
                "SELECT alias FROM entity_aliases WHERE note_id = ?1 AND alias != ?2 ORDER BY alias",
            )?;
            for (id, name) in rows {
                let aliases: Vec<String> = aliases_st
                    .query_map(rusqlite::params![id, name], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
                out.push(Item::entity(kind, Some(&id), &name, &refs));
            }
        }
        DedupeKind::Alias => {}
    }
    Ok(out)
}

fn keep_both(conn: &Connection) -> CoreResult<KeepBothSet> {
    let mut set = KeepBothSet::new();
    let mut st = conn.prepare("SELECT kind, a_id, b_id FROM keep_both")?;
    let rows: Vec<(String, String, String)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (kind, a, b) in rows {
        if let Ok(kind) = kind.parse::<DedupeKind>() {
            set.insert(KeepBoth::new(kind, &a, &b));
        }
    }
    Ok(set)
}

fn matches_query(q: &CandidateQuery, cand: &Item) -> bool {
    let keys = cand.keys();
    keys.exact_keys.iter().any(|k| q.exact_keys.contains(k))
        || keys.phonetic_keys.iter().any(|k| q.phonetic_keys.contains(k))
        || keys.trigram_texts.iter().any(|t| {
            q.trigram_texts
                .iter()
                .any(|qt| text_normalize::trigram_similarity(qt, t) >= q.trigram_floor)
        })
}

/// Duplicate candidates of `item` among the locally cached items, best first.
pub fn check(conn: &Connection, item: &Item) -> CoreResult<Vec<DuplicateCandidate>> {
    let thresholds = Thresholds::new();
    let query = CandidateQuery::for_item(item, &thresholds);
    let mut existing = Vec::new();
    for kind in &query.kinds {
        for cand in items_of_kind(conn, *kind)? {
            if matches_query(&query, &cand) {
                existing.push(Existing {
                    item: cand,
                    semantic: None,
                });
            }
        }
    }
    Ok(dedupe::check(item, &existing, &thresholds, &keep_both(conn)?).candidates)
}
