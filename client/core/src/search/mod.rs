//! Local full-text search (PLAN §12.1 `search/`): `SQLite` `FTS5` over text normalised with the
//! shared `text-normalize` crate, applied identically at index time
//! ([`crate::store::index`]) and query time, so Arabic letter variants, tashkeel, tatweel
//! and case never prevent a match — the same rules the server's `tsvector` uses.

pub mod duplicates;

use rusqlite::{Connection, params};

use crate::error::CoreResult;
use crate::format::direction::dir_of;
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

/// Modes usable now: keyword always; semantic and hybrid when the server is reachable.
pub fn available_modes(ctx: &ViewCtx) -> Vec<SearchMode> {
    if ctx.connectivity == Connectivity::Offline {
        vec![SearchMode::Keyword]
    } else {
        vec![
            SearchMode::Keyword,
            SearchMode::Semantic,
            SearchMode::Hybrid,
        ]
    }
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_owned())
        .filter(|t| !t.is_empty())
        .collect()
}

fn in_folder(path: &str, folder: Option<&str>) -> bool {
    match folder
        .map(|f| f.trim_matches('/'))
        .filter(|f| !f.is_empty())
    {
        None => true,
        Some(f) => path.starts_with(&format!("{f}/")),
    }
}

fn hit(
    note_id: String,
    title: String,
    path: String,
    kind: String,
    snippet: String,
    score: f64,
    terms: &[String],
) -> SearchHit {
    SearchHit {
        title_dir: dir_of(&title),
        snippet_dir: dir_of(&snippet),
        highlights: crate::view::build::highlight_spans(&snippet, terms),
        score,
        note_id,
        title,
        path,
        kind,
        snippet,
    }
}

/// Runs a local keyword search (works offline), optionally limited to a folder (and its
/// subfolders). Semantic and hybrid run on the server (`Session::search_remote`); asked here
/// they report their availability with keyword results.
pub fn search(
    conn: &Connection,
    ctx: &ViewCtx,
    query: &str,
    mode: SearchMode,
    folder: Option<&str>,
) -> CoreResult<SearchView> {
    let availability = match mode {
        SearchMode::Keyword => Availability::Available,
        SearchMode::Semantic | SearchMode::Hybrid if ctx.connectivity == Connectivity::Offline => {
            Availability::Offline
        }
        SearchMode::Semantic | SearchMode::Hybrid => Availability::Available,
    };
    let folder_owned = folder
        .map(str::to_owned)
        .filter(|f| !f.trim_matches('/').is_empty());
    let Some(q) = fts_query(query) else {
        return Ok(SearchView {
            query: query.to_owned(),
            mode,
            results: Vec::new(),
            availability,
            available_modes: available_modes(ctx),
            folder: folder_owned,
        });
    };
    let rows: Vec<(String, String, String, String, String, f64)> = {
        let mut st = conn.prepare(
            "SELECT n.id, n.title, n.path, n.kind, n.content,
                    bm25(notes_fts, 0.0, 10.0, 1.0, 5.0)
             FROM notes_fts f JOIN notes n ON n.id = f.note_id
             WHERE notes_fts MATCH ?1 AND n.deleted = 0
             ORDER BY bm25(notes_fts, 0.0, 10.0, 1.0, 5.0), n.title, n.id",
        )?;
        st.query_map(params![q], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let terms = query_terms(query);
    Ok(SearchView {
        query: query.to_owned(),
        mode,
        results: rows
            .into_iter()
            .filter(|r| in_folder(&r.2, folder))
            .take(LIMIT)
            .map(|(note_id, title, path, kind, content, rank)| {
                hit(
                    note_id,
                    title,
                    path,
                    kind,
                    snippet_for(&content, query),
                    -rank,
                    &terms,
                )
            })
            .collect(),
        availability,
        available_modes: available_modes(ctx),
        folder: folder_owned,
    })
}

/// A server result list as the search view (folder applied here).
pub fn remote_view(
    conn: &Connection,
    ctx: &ViewCtx,
    query: &str,
    mode: SearchMode,
    folder: Option<String>,
    hits: Vec<crate::net::RemoteHit>,
) -> CoreResult<SearchView> {
    let terms = query_terms(query);
    let mut results = Vec::new();
    for h in hits {
        if !in_folder(&h.path, folder.as_deref()) {
            continue;
        }
        let snippet = match h.snippet {
            Some(s) => s,
            None => crate::store::notes::current(conn, &h.id)?
                .map(|n| snippet_for(&n.content, query))
                .unwrap_or_default(),
        };
        results.push(hit(h.id, h.title, h.path, h.kind, snippet, h.score, &terms));
    }
    Ok(SearchView {
        query: query.to_owned(),
        mode,
        results,
        availability: Availability::Available,
        available_modes: available_modes(ctx),
        folder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_normalised_prefix_terms() {
        assert_eq!(
            fts_query("Pricing  TIERS").as_deref(),
            Some("\"pricing\"* \"tiers\"*")
        );
        assert_eq!(fts_query("   "), None);
        // Punctuation (quotes included) separates tokens after normalisation.
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"a\"* \"b\"*"));
    }
}
