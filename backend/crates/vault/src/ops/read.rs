//! Reads: note by ID or path, tree, backlinks, history, file at revision, keyword search,
//! inbox, integrity warnings. They read the index and the files directly (not through the
//! writer), so they never wait for writes; atomic renames guarantee whole files.

use std::collections::{BTreeMap, HashMap};

use strata_common::NoteId;
use strata_index::UserScope;
use strata_index::repo::notes::{self, Note};
use strata_index::repo::suggestions::{self, Suggestion};
use strata_index::repo::vault::{self as vrepo, IntegrityWarning};
use strata_index::repo::graph;
use strata_index::types::{LinkKind, SuggestionStatus};
use text_normalize::normalize_for_search;
use vault_format::Document;

use crate::error::{Result, VaultError};
use crate::model::{
    Backlink, BacklinkGroup, NoteAtRevision, NoteView, Revision, SearchHit, TreeEntry,
};
use crate::ops::notes::timestamps;
use crate::store::{VaultService, blocking};
use crate::{fsio, git, paths};

/// Search modes of `GET /search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Postgres full-text over normalised text.
    Keyword,
    /// Embeddings (Phase 4).
    Semantic,
    /// Both (Phase 4).
    Hybrid,
}

fn kind_of(n: &Note) -> domain::NoteKind {
    n.kind.as_str().parse().unwrap_or(domain::NoteKind::Note)
}

impl VaultService {
    /// Makes sure the user's vault is loaded (and reconciled) before a read.
    pub async fn ready(&self, scope: &UserScope) -> Result<()> {
        let user = scope.user_id();
        let loaded = |inner: &crate::store::Inner| {
            inner
                .loaded
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains(&user)
        };
        if loaded(&self.inner) {
            return Ok(());
        }
        self.exec(scope, |_, _| Box::pin(async { Ok(()) })).await?;
        self.inner
            .loaded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(user);
        Ok(())
    }

    async fn read_file(&self, scope: &UserScope, rel: &str) -> Result<Option<String>> {
        let dir = self.vault_dir(scope.user_id());
        let rel = rel.to_owned();
        Ok(blocking(move || Ok(fsio::read(&dir, &rel)?))
            .await?
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    async fn note_row(&self, scope: &UserScope, id: NoteId) -> Result<Note> {
        let mut tx = self.inner.db.begin(scope).await?;
        let n = notes::get_note(&mut tx, id).await?;
        tx.commit().await?;
        n.ok_or(VaultError::NotFound)
    }

    fn view_of(row: &Note, text: String) -> NoteView {
        let doc = Document::parse(&text);
        let (created, updated) = timestamps(&doc, row.id);
        NoteView::from_text(row.id, &row.path, text, created, updated, row.trashed)
    }

    /// A note (live or trashed) with its content.
    pub async fn note(&self, scope: &UserScope, id: NoteId) -> Result<NoteView> {
        self.ready(scope).await?;
        let row = self.note_row(scope, id).await?;
        match self.read_file(scope, &row.path).await? {
            Some(text) => Ok(Self::view_of(&row, text)),
            // Moved while we looked: ask the writer.
            None => self.exec(scope, move |core, _| Box::pin(core.view(id))).await,
        }
    }

    /// A live note by vault path.
    pub async fn note_by_path(&self, scope: &UserScope, path: &str) -> Result<NoteView> {
        paths::validate_note_path(path)?;
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let row = notes::get_note_by_path(&mut tx, path).await?;
        tx.commit().await?;
        let row = row.filter(|r| !r.trashed).ok_or(VaultError::NotFound)?;
        let text = self
            .read_file(scope, &row.path)
            .await?
            .ok_or(VaultError::NotFound)?;
        Ok(Self::view_of(&row, text))
    }

    /// Folders, notes and other files, sorted by path (hidden folders excluded).
    pub async fn tree(&self, scope: &UserScope) -> Result<Vec<TreeEntry>> {
        self.ready(scope).await?;
        let dir = self.vault_dir(scope.user_id());
        let (dirs, files) = blocking(move || Ok((fsio::scan_dirs(&dir)?, fsio::scan(&dir)?))).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let rows = notes::list_notes(&mut tx, false).await?;
        tx.commit().await?;
        let mut out: BTreeMap<String, TreeEntry> = BTreeMap::new();
        for d in dirs {
            out.insert(d.clone(), TreeEntry::Folder { path: d });
        }
        for f in files.into_iter().filter(|f| paths::is_content(f) && !f.ends_with(".md")) {
            out.insert(f.clone(), TreeEntry::File { path: f });
        }
        for n in rows {
            out.insert(
                n.path.clone(),
                TreeEntry::Note {
                    kind: kind_of(&n),
                    path: n.path,
                    id: n.id,
                    title: n.title,
                    updated: n.updated,
                },
            );
        }
        Ok(out.into_values().collect())
    }

    /// Body links and relations pointing at a live note, grouped by kind (`link`, `embed`,
    /// then relation types alphabetically); self-links excluded.
    pub async fn backlinks(&self, scope: &UserScope, id: NoteId) -> Result<Vec<BacklinkGroup>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let me = notes::get_note(&mut tx, id).await?;
        if me.is_none_or(|n| n.trashed) {
            return Err(VaultError::NotFound);
        }
        let links = graph::backlinks(&mut tx, id).await?;
        let relations = graph::relations_to(&mut tx, id).await?;
        let mut sources: HashMap<NoteId, Note> = HashMap::new();
        for src in links
            .iter()
            .map(|l| l.src_id)
            .chain(relations.iter().map(|r| r.src_id))
        {
            if src != id
                && !sources.contains_key(&src)
                && let Some(n) = notes::get_note(&mut tx, src).await?
            {
                sources.insert(src, n);
            }
        }
        tx.commit().await?;
        let mut groups: BTreeMap<(u8, String), Vec<Backlink>> = BTreeMap::new();
        for l in links {
            let Some(src) = sources.get(&l.src_id) else { continue };
            let kind = match l.kind {
                LinkKind::Link => "link",
                LinkKind::Embed => "embed",
            };
            let rank = u8::from(l.kind == LinkKind::Embed);
            groups.entry((rank, kind.to_owned())).or_default().push(Backlink {
                source_id: src.id,
                source_path: src.path.clone(),
                source_title: src.title.clone(),
                kind: kind.to_owned(),
                by: None,
                confidence: None,
                anchor: l.anchor,
                block_id: l.block_id,
            });
        }
        for r in relations {
            let Some(src) = sources.get(&r.src_id) else { continue };
            groups
                .entry((2, r.rel_type.clone()))
                .or_default()
                .push(Backlink {
                    source_id: src.id,
                    source_path: src.path.clone(),
                    source_title: src.title.clone(),
                    kind: r.rel_type.clone(),
                    by: Some(r.by.as_str().to_owned()),
                    confidence: r.confidence,
                    anchor: None,
                    block_id: None,
                });
        }
        Ok(groups
            .into_iter()
            .map(|((_, kind), mut items)| {
                items.sort_by(|a, b| {
                    (&a.source_path, &a.anchor, &a.block_id)
                        .cmp(&(&b.source_path, &b.anchor, &b.block_id))
                });
                items.dedup();
                BacklinkGroup { kind, items }
            })
            .collect())
    }

    /// Git history of a note (newest first), following moves and trash.
    pub async fn history(&self, scope: &UserScope, id: NoteId) -> Result<Vec<Revision>> {
        self.ready(scope).await?;
        let row = self.note_row(scope, id).await?;
        let dir = self.vault_dir(scope.user_id());
        let path = row.path.clone();
        let revs = blocking(move || git::history(&dir, &path)).await?;
        Ok(revs
            .into_iter()
            .map(|r| Revision {
                author: r
                    .commit
                    .message
                    .split(':')
                    .next()
                    .filter(|a| matches!(*a, "user" | "ai" | "system"))
                    .unwrap_or("system")
                    .to_owned(),
                commit: r.commit.id,
                message: r.commit.message,
                at: r.commit.at,
                path: r.path,
                change: match r.change {
                    git::Change::Added => "added",
                    git::Change::Modified => "modified",
                    git::Change::Renamed => "renamed",
                    git::Change::Deleted => "deleted",
                }
                .to_owned(),
            })
            .collect())
    }

    /// The note as of `commit` (a commit in its history).
    pub async fn note_at(&self, scope: &UserScope, id: NoteId, commit: &str) -> Result<NoteAtRevision> {
        self.ready(scope).await?;
        let row = self.note_row(scope, id).await?;
        let dir = self.vault_dir(scope.user_id());
        let path = row.path.clone();
        let c = commit.to_owned();
        let found = blocking(move || {
            let history = git::history(&dir, &path)?;
            let Some(rev) = history
                .into_iter()
                .find(|r| r.commit.id == c && r.change != git::Change::Deleted)
            else {
                return Ok(None);
            };
            Ok(git::blob_at(&dir, &c, &rev.path)?.map(|b| (rev.path, b)))
        })
        .await?;
        let (path, bytes) = found.ok_or(VaultError::NotFound)?;
        Ok(NoteAtRevision {
            commit: commit.to_owned(),
            path,
            version: fsio::version_of(&bytes),
            content: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }

    /// Keyword search (Postgres full-text over Arabic/Latin-normalised text).
    pub async fn search(
        &self,
        scope: &UserScope,
        query: &str,
        mode: SearchMode,
        limit: u32,
    ) -> Result<Vec<SearchHit>> {
        if mode != SearchMode::Keyword {
            return Err(VaultError::AiUnavailable);
        }
        let q = normalize_for_search(query);
        if q.is_empty() {
            return Err(VaultError::invalid("the query is empty after normalisation"));
        }
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let hits = notes::search_notes(&mut tx, &q, i64::from(limit.clamp(1, 100))).await?;
        let mut rows = Vec::new();
        for h in hits {
            if let Some(n) = notes::get_note(&mut tx, h.note_id).await? {
                rows.push((n, h.rank));
            }
        }
        tx.commit().await?;
        let terms: Vec<String> = q
            .split_whitespace()
            .filter(|t| !t.starts_with('-') && *t != "or")
            .map(|t| t.trim_matches('"').to_owned())
            .filter(|t| !t.is_empty())
            .collect();
        let mut out = Vec::new();
        for (n, rank) in rows {
            let text = self.read_file(scope, &n.path).await?.unwrap_or_default();
            let doc = Document::parse(&text);
            let snippet = doc
                .body()
                .lines()
                .map(str::trim)
                .find(|l| {
                    let nl = normalize_for_search(l);
                    !nl.is_empty() && terms.iter().any(|t| nl.contains(t.as_str()))
                })
                .map(|l| {
                    let s: String = l.chars().take(160).collect();
                    if l.chars().count() > 160 { format!("{s}…") } else { s }
                });
            out.push(SearchHit {
                kind: kind_of(&n),
                id: n.id,
                path: n.path,
                title: n.title,
                score: rank,
                snippet,
            });
        }
        Ok(out)
    }

    /// Inbox notes (newest first) with their pending suggestions.
    pub async fn inbox(&self, scope: &UserScope) -> Result<Vec<(NoteView, Vec<Suggestion>)>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let rows: Vec<Note> = notes::list_notes(&mut tx, false)
            .await?
            .into_iter()
            .filter(|n| crate::derive::is_inbox(&n.path))
            .collect();
        let pending = suggestions::list_suggestions(&mut tx, SuggestionStatus::Pending).await?;
        tx.commit().await?;
        let mut out = Vec::new();
        for n in rows {
            let Some(text) = self.read_file(scope, &n.path).await? else {
                continue;
            };
            let s: Vec<Suggestion> = pending
                .iter()
                .filter(|s| s.note_id == Some(n.id))
                .cloned()
                .collect();
            out.push((Self::view_of(&n, text), s));
        }
        out.sort_by(|a, b| b.0.created.cmp(&a.0.created).then(b.0.id.cmp(&a.0.id)));
        Ok(out)
    }

    /// Integrity warnings, newest first.
    pub async fn integrity(&self, scope: &UserScope, limit: i64) -> Result<Vec<IntegrityWarning>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let w = vrepo::list_warnings(&mut tx, limit).await?;
        tx.commit().await?;
        Ok(w)
    }
}
