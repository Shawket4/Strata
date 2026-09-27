//! Local full-text search (PLAN §12.1 `search/`): `SQLite` `FTS5` over text normalised with the
//! shared `text-normalize` crate, applied identically at index time
//! ([`crate::store::index`]) and query time, so Arabic letter variants, tashkeel, tatweel
//! and case never prevent a match — the same rules the server's `tsvector` uses.

pub mod duplicates;

use rusqlite::{Connection, params};

use crate::error::CoreResult;
use crate::view::ViewCtx;
use crate::view::model::{Availability, Connectivity, SearchHit, SearchMode, SearchView};

/// Maximum results.
pub const LIMIT: usize = 50;

/// The FTS5 query for `text`: every normalised token as a prefix term, all required.
/// `None` when nothing searchable remains.
pub fn fts_query(text: &str) -> Option<String> {
    let norm = text_normalize::normalize_for_search(text);
    let terms: Vec<String> = norm
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// The first line of `content`'s body that contains a query token (after normalisation),
/// trimmed to 160 characters; else the first body line.
pub fn snippet_for(content: &str, query: &str) -> String {
    let tokens: Vec<String> = text_normalize::normalize_for_search(query)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let doc = vault_format::Document::parse(content);
    let hit = doc.body().lines().map(str::trim).find(|l| {
        let n = text_normalize::normalize_for_search(l);
        tokens.iter().any(|t| n.contains(t.as_str()))
    });
    match hit {
        Some(line) => {
            let mut s: String = line.chars().take(160).collect();
            if line.chars().count() > 160 {
                s.push('…');
            }
            s
        }
        None => crate::view::build::snippet(content, 160),
    }
}

/// Runs a search. Keyword search is local and works offline; semantic and hybrid need the
/// server (`GET /search`, not in the contract yet), so they fall back to keyword results
/// and report their availability.
pub fn search(conn: &Connection, ctx: &ViewCtx, query: &str, mode: SearchMode) -> CoreResult<SearchView> {
    let availability = match mode {
        SearchMode::Keyword => Availability::Available,
        SearchMode::Semantic | SearchMode::Hybrid if ctx.connectivity == Connectivity::Offline => {
            Availability::Offline
        }
        SearchMode::Semantic | SearchMode::Hybrid => Availability::NotYetAvailable {
            feature: "semantic_search".to_owned(),
        },
    };
    let Some(q) = fts_query(query) else {
        return Ok(SearchView {
            query: query.to_owned(),
            mode,
            results: Vec::new(),
            availability,
        });
    };
    let rows: Vec<(String, String, String, String, String)> = {
        let mut st = conn.prepare(
            "SELECT n.id, n.title, n.path, n.kind, n.content
             FROM notes_fts f JOIN notes n ON n.id = f.note_id
             WHERE notes_fts MATCH ?1 AND n.deleted = 0
             ORDER BY bm25(notes_fts, 0.0, 10.0, 1.0, 5.0), n.title, n.id
             LIMIT ?2",
        )?;
        st.query_map(
            params![q, i64::try_from(LIMIT).unwrap_or(50)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?
        .collect::<Result<_, _>>()?
    };
    Ok(SearchView {
        query: query.to_owned(),
        mode,
        results: rows
            .into_iter()
            .map(|(note_id, title, path, kind, content)| SearchHit {
                snippet: snippet_for(&content, query),
                note_id,
                title,
                path,
                kind,
            })
            .collect(),
        availability,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_normalised_prefix_terms() {
        assert_eq!(fts_query("Pricing  TIERS").as_deref(), Some("\"pricing\"* \"tiers\"*"));
        assert_eq!(fts_query("   "), None);
        // Quotes are escaped.
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"a\"\"b\"*"));
    }
}
