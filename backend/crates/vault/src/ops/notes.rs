//! Note writes: create, update, move/rename (with vault-wide link rewriting), soft delete,
//! restore, purge, revert, whole-commit revert, block-ID append, capture.

use std::collections::BTreeSet;

use strata_common::{NoteId, SuggestionId};
use strata_index::repo::suggestions;
use strata_index::{ScopedTx, UserScope};
use vault_format::canvas::Canvas;
use vault_format::filename::unique_name;
use vault_format::sidecar::{KeepBoth, NoteSidecar};
use vault_format::{Document, PathIndex, rewrite::MoveSet};

use crate::derive;
use crate::dup;
use crate::error::{Candidate, Result, VaultError};
use crate::git::{self, FileChange};
use crate::model::{Captured, NoteView};
use crate::paths::{self, validate_note_path};
use crate::prepare;
use crate::state::name_key;
use crate::store::{Author, Core, blocking};

/// `POST /notes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateNote {
    /// Vault path (`notes/Pricing.md`).
    pub path: String,
    /// Full file content (frontmatter optional; `id`, `created`, `updated` are set).
    pub content: String,
    /// Client-generated ID (offline creates keep their IDs).
    pub id: Option<NoteId>,
    /// Create even if it looks like a duplicate (records keep-both).
    pub force: bool,
}

/// Payload of a `duplicate` suggestion (`MessagePack` in `suggestions.payload`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DuplicatePayload {
    /// The candidates found when the item was saved.
    pub candidates: Vec<DuplicatePayloadItem>,
}

/// One candidate in a [`DuplicatePayload`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DuplicatePayloadItem {
    /// Existing item ID (note ULID, or the ULID of a task).
    pub id: String,
    /// Stored item ID (note ULID or task block ID).
    pub item: String,
    /// Snippet.
    pub snippet: Option<String>,
    /// Kind.
    pub kind: String,
    /// Title.
    pub title: String,
    /// `exact` / `near`.
    pub match_level: String,
    /// Score.
    pub score: f64,
}

impl DuplicatePayloadItem {
    /// Back to a candidate.
    pub fn candidate(&self) -> Option<Candidate> {
        Some(Candidate {
            item: self.item.clone(),
            id: self.id.parse().ok()?,
            kind: self.kind.clone(),
            title: self.title.clone(),
            snippet: self.snippet.clone(),
            level: if self.match_level == "exact" {
                crate::error::MatchLevel::Exact
            } else {
                crate::error::MatchLevel::Near
            },
            score: self.score,
        })
    }
}

impl DuplicatePayload {
    /// From candidates.
    pub fn from_candidates(c: &[Candidate]) -> Self {
        Self {
            candidates: c
                .iter()
                .map(|c| DuplicatePayloadItem {
                    id: c.id.to_string(),
                    item: c.item.clone(),
                    snippet: c.snippet.clone(),
                    kind: c.kind.clone(),
                    title: c.title.clone(),
                    match_level: match c.level {
                        crate::error::MatchLevel::Exact => "exact".into(),
                        crate::error::MatchLevel::Near => "near".into(),
                    },
                    score: c.score,
                })
                .collect(),
        }
    }
}

/// Task block IDs used by other notes among those in `body`.
pub(crate) async fn taken_task_ids(
    tx: &mut ScopedTx,
    body: &str,
    note: NoteId,
) -> Result<BTreeSet<String>> {
    let ids: Vec<String> = vault_format::tasks::extract_tasks(body)
        .iter()
        .filter_map(|t| t.task.block_id().map(str::to_owned))
        .collect();
    if ids.is_empty() {
        return Ok(BTreeSet::new());
    }
    let taken: Vec<String> =
        sqlx::query_scalar("SELECT id FROM tasks WHERE id = ANY($1) AND note_id <> $2")
            .bind(&ids)
            .bind(note)
            .fetch_all(tx.conn())
            .await?;
    Ok(taken.into_iter().collect())
}

impl Core {
    /// The live note `id`: its path and version.
    pub(crate) fn live(&self, id: NoteId) -> Result<(String, String)> {
        self.state()?
            .note(id)
            .map(|(p, m)| (p.to_owned(), m.version.clone()))
            .ok_or(VaultError::NotFound)
    }

    /// A view of the live or trashed note `id` as it is on disk now.
    pub(crate) async fn view(&self, id: NoteId) -> Result<NoteView> {
        let state = self.state()?;
        let (path, trashed) = match state.note(id) {
            Some((p, _)) => (p.to_owned(), false),
            None => (
                state
                    .trash_by_id
                    .get(&id)
                    .cloned()
                    .ok_or(VaultError::NotFound)?,
                true,
            ),
        };
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let doc = Document::parse(&text);
        let (created, updated) = timestamps(&doc, id);
        Ok(NoteView::from_text(
            id, &path, text, created, updated, trashed,
        ))
    }

    /// Writes the changes, commits, indexes, and commits `tx`.
    pub(crate) async fn finish(
        &mut self,
        mut tx: ScopedTx,
        changes: Vec<FileChange>,
        message: String,
    ) -> Result<Option<String>> {
        let paths: BTreeSet<String> = changes.iter().map(|(p, _)| p.clone()).collect();
        let commit = self.apply(changes, message).await?;
        let synced = self.sync_paths(&mut tx, &paths).await;
        match synced {
            Ok(_) => {}
            Err(e) => {
                self.state = None;
                self.repair = true;
                return Err(e);
            }
        }
        if let Err(e) = tx.commit().await {
            self.state = None;
            self.repair = true;
            return Err(e.into());
        }
        Ok(commit)
    }

    /// Prepares new note content: stamps `id`/`created`/`updated` and gives task lines block
    /// IDs. `created` is kept when present.
    pub(crate) async fn prepare(
        &self,
        tx: &mut ScopedTx,
        content: &str,
        id: NoteId,
        created_fallback: Option<chrono::DateTime<chrono::FixedOffset>>,
        stamp_updated: bool,
    ) -> Result<Document> {
        let tz = self.tz(tx).await?;
        let now = self.local_now(tz);
        let mut doc = Document::parse(content);
        let created = created_fallback.unwrap_or(now);
        prepare::stamp(&mut doc, id, Some(&created), stamp_updated.then_some(&now))?;
        let taken = taken_task_ids(tx, doc.body(), id).await?;
        let body = prepare::assign_task_ids(doc.body(), self.ids(), &taken);
        if body != doc.body() {
            doc.set_body(body);
        }
        Ok(doc)
    }

    /// Runs the duplicate check for a note about to be created at `path` with `id`.
    pub(crate) async fn check_duplicates(
        &self,
        tx: &mut ScopedTx,
        path: &str,
        doc: &Document,
        id: NoteId,
    ) -> Result<Vec<Candidate>> {
        let item = dup::note_item(path, Some(id), doc);
        dup::find(tx, &item, &self.inner.config.near_thresholds).await
    }

    /// Records keep-both for `item` (of the note `sidecar` belongs to) and `candidates`:
    /// note–note pairs in `keep_both`, any other pair in the `keep_both_items` extension.
    pub(crate) fn keep_both(
        &self,
        sidecar: &mut NoteSidecar,
        item: &dedupe::Item,
        candidates: &[Candidate],
    ) {
        let at = self.now().fixed_offset();
        let own_is_note = item.id.as_deref() == Some(sidecar.id.to_string().as_str());
        for pair in dup::forced_pairs(item, candidates) {
            let other = if Some(pair.a_id.as_str()) == item.id.as_deref() {
                pair.b_id.clone()
            } else {
                pair.a_id.clone()
            };
            let other_note = other.parse::<NoteId>().ok();
            match other_note {
                Some(o) if own_is_note => {
                    if !sidecar.keep_both.iter().any(|k| k.other_id == o.as_ulid()) {
                        sidecar.keep_both.push(KeepBoth {
                            other_id: o.as_ulid(),
                            at,
                        });
                    }
                }
                _ => {
                    let entry = sidecar
                        .extra
                        .entry(derive::KEEP_BOTH_ITEMS_KEY.to_owned())
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                    if let serde_json::Value::Array(items) = entry {
                        items.push(serde_json::json!({
                            "kind": pair.kind.as_str(),
                            "a": pair.a_id,
                            "b": pair.b_id,
                            "at": vault_format::frontmatter::format_timestamp(&at),
                        }));
                    }
                }
            }
        }
    }

    /// Creates a note.
    pub async fn create_note(
        &mut self,
        scope: UserScope,
        req: CreateNote,
        author: Author,
    ) -> Result<NoteView> {
        validate_note_path(&req.path)?;
        let state = self.state()?;
        if state.notes.contains_key(&req.path) || state.attachments.contains(&req.path) {
            return Err(VaultError::PathTaken);
        }
        if let Some(id) = req.id
            && state.contains_id(id)
        {
            return Err(VaultError::invalid("a note with this id already exists"));
        }
        let mut tx = self.begin(&scope).await?;
        let given = Document::parse(&req.content)
            .frontmatter()
            .and_then(|f| f.id().ok().flatten())
            .map(NoteId::from_ulid)
            .filter(|i| !self.state().is_ok_and(|s| s.contains_id(*i)));
        let id = req
            .id
            .or(given)
            .unwrap_or_else(|| NoteId::generate(self.ids()));
        let doc = self.prepare(&mut tx, &req.content, id, None, true).await?;
        let candidates = self.check_duplicates(&mut tx, &req.path, &doc, id).await?;
        let mut changes = Vec::new();
        if !candidates.is_empty() {
            if !req.force {
                return Err(VaultError::Duplicate(candidates));
            }
            let mut sc = self
                .sidecar(id)
                .await?
                .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
            self.keep_both(
                &mut sc,
                &dup::note_item(&req.path, Some(id), &doc),
                &candidates,
            );
            changes.push(Core::sidecar_change(&sc)?);
        }
        let text = doc.render();
        changes.insert(0, (req.path.clone(), Some(text.into_bytes())));
        self.finish(tx, changes, author.message("create", &req.path))
            .await?;
        self.view(id).await
    }

    /// Replaces a note's content (`If-Match` = its current version).
    pub async fn update_note(
        &mut self,
        scope: UserScope,
        id: NoteId,
        content: String,
        if_match: &str,
        author: Author,
    ) -> Result<NoteView> {
        let (path, version) = self.live(id)?;
        if version != if_match {
            return Err(VaultError::VersionConflict { current: version });
        }
        let old = self.read_text(&path).await?.unwrap_or_default();
        if old == content {
            return self.view(id).await;
        }
        let new_doc = Document::parse(&content);
        if let Some(other) = new_doc.frontmatter().and_then(|f| f.id().ok().flatten())
            && other != id.as_ulid()
        {
            return Err(VaultError::invalid("the id property cannot change"));
        }
        let old_created = Document::parse(&old)
            .frontmatter()
            .and_then(|f| f.created().ok().flatten());
        let mut tx = self.begin(&scope).await?;
        // `updated` records the user's edits; AI edits (frontmatter keys, AI sections) keep it.
        let doc = self
            .prepare(&mut tx, &content, id, old_created, author == Author::User)
            .await?;
        let mut changes = vec![(path.clone(), Some(doc.render().into_bytes()))];
        if let Some(mut sc) = self.sidecar(id).await? {
            let state = self.state()?;
            let edges = prepare::frontmatter_edges(&doc, &path, &state.path_index(), state);
            if prepare::prune_sidecar(&mut sc, &edges) {
                changes.push(Core::sidecar_change(&sc)?);
            }
        }
        self.finish(tx, changes, author.message("update", &path))
            .await?;
        self.view(id).await
    }

    /// The file changes of moving `old` to `new`: the moved file (with `moved_text` if
    /// given, else its current text), every note whose links change, and canvas nodes.
    pub(crate) async fn plan_move(
        &self,
        old: &str,
        new: &str,
        moved_text: Option<String>,
    ) -> Result<Vec<FileChange>> {
        let state = self.state()?;
        let before = state.path_index();
        let after = PathIndex::new(
            before
                .paths()
                .iter()
                .map(|p| if p == old { new.to_owned() } else { p.clone() }),
        );
        let moves = MoveSet::new(&before, &after, [(old.to_owned(), new.to_owned())]);
        let names: BTreeSet<String> = [name_key(old), name_key(new)].into();
        let mut sources: Vec<String> = state
            .linking_to(&names)
            .into_iter()
            .filter_map(|id| state.note(id).map(|(p, _)| p.to_owned()))
            .filter(|p| p != old)
            .collect();
        sources.sort();
        let canvases: Vec<String> = state
            .attachments
            .iter()
            .filter(|p| p.ends_with(".canvas"))
            .cloned()
            .collect();
        let mut changes = Vec::new();
        let moved_body = match moved_text {
            Some(t) => t,
            None => self.read_text(old).await?.ok_or(VaultError::NotFound)?,
        };
        let mut doc = Document::parse(&moved_body);
        doc.rewrite_links(&moves, old)
            .map_err(|_| VaultError::invalid("the note's frontmatter cannot be edited"))?;
        changes.push((old.to_owned(), None));
        changes.push((new.to_owned(), Some(doc.render().into_bytes())));
        for p in sources {
            let Some(text) = self.read_text(&p).await? else {
                continue;
            };
            let mut d = Document::parse(&text);
            if d.rewrite_links(&moves, &p).unwrap_or(0) > 0 {
                changes.push((p, Some(d.render().into_bytes())));
            }
        }
        for c in canvases {
            let Some(text) = self.read_text(&c).await? else {
                continue;
            };
            if let Ok(mut canvas) = Canvas::from_json(&text)
                && canvas.rename_file(old, new) > 0
            {
                changes.push((c, Some(canvas.to_json().into_bytes())));
            }
        }
        Ok(changes)
    }

    /// Moves/renames a note, rewriting every inbound link and relation in the same commit.
    pub async fn move_note(
        &mut self,
        scope: UserScope,
        id: NoteId,
        new_path: String,
        if_match: Option<&str>,
        author: Author,
    ) -> Result<NoteView> {
        validate_note_path(&new_path)?;
        let (old, version) = self.live(id)?;
        if let Some(m) = if_match
            && m != version
        {
            return Err(VaultError::VersionConflict { current: version });
        }
        if old == new_path {
            return self.view(id).await;
        }
        let state = self.state()?;
        let taken = |p: &str| state.notes.contains_key(p) || state.attachments.contains(p);
        let case_only = old.to_lowercase() == new_path.to_lowercase();
        if taken(&new_path) && !case_only {
            return Err(VaultError::PathTaken);
        }
        let tx = self.begin(&scope).await?;
        let changes = self.plan_move(&old, &new_path, None).await?;
        self.finish(
            tx,
            changes,
            author.message("move", &format!("{old} -> {new_path}")),
        )
        .await?;
        self.view(id).await
    }

    fn free_path(&self, wanted: &str, trash: bool) -> Result<String> {
        let state = self.state()?;
        let folder = paths::parent(wanted);
        let stem = paths::file_name(wanted)
            .strip_suffix(".md")
            .unwrap_or(paths::file_name(wanted));
        let taken: Vec<&str> = if trash {
            state
                .trash
                .keys()
                .filter(|p| paths::parent(p) == folder)
                .map(|p| paths::file_name(p).strip_suffix(".md").unwrap_or(p))
                .collect()
        } else {
            state
                .notes
                .keys()
                .chain(state.attachments.iter())
                .filter(|p| paths::parent(p) == folder)
                .map(|p| paths::file_name(p).strip_suffix(".md").unwrap_or(p))
                .collect()
        };
        Ok(paths::join(
            folder,
            &format!("{}.md", unique_name(stem, taken)),
        ))
    }

    /// Moves a note to `.trash/` (soft delete).
    pub async fn delete_note(
        &mut self,
        scope: UserScope,
        id: NoteId,
        author: Author,
    ) -> Result<NoteView> {
        let (path, _) = self.live(id)?;
        let trash = self.free_path(&paths::trash_path(&path), true)?;
        let text = self.read(&path).await?.ok_or(VaultError::NotFound)?;
        let tx = self.begin(&scope).await?;
        let changes = vec![(path.clone(), None), (trash, Some(text))];
        self.finish(tx, changes, author.message("delete", &path))
            .await?;
        self.view(id).await
    }

    /// Restores a note from `.trash/` to its original path (or a free name next to it).
    pub async fn restore_note(
        &mut self,
        scope: UserScope,
        id: NoteId,
        author: Author,
    ) -> Result<NoteView> {
        let trash = self
            .state()?
            .trash_by_id
            .get(&id)
            .cloned()
            .ok_or(VaultError::NotFound)?;
        let original = paths::untrash_path(&trash).ok_or(VaultError::NotFound)?;
        let target = self.free_path(original, false)?;
        let text = self.read(&trash).await?.ok_or(VaultError::NotFound)?;
        let tx = self.begin(&scope).await?;
        let changes = vec![(trash, None), (target.clone(), Some(text))];
        self.finish(tx, changes, author.message("restore", &target))
            .await?;
        self.view(id).await
    }

    /// Permanently deletes a trashed note and its sidecar.
    pub async fn purge_note(&mut self, scope: UserScope, id: NoteId, author: Author) -> Result<()> {
        let trash = self
            .state()?
            .trash_by_id
            .get(&id)
            .cloned()
            .ok_or(VaultError::NotFound)?;
        let tx = self.begin(&scope).await?;
        let changes = vec![
            (trash.clone(), None),
            (NoteSidecar::path_for(id.as_ulid()), None),
        ];
        self.finish(tx, changes, author.message("purge", &trash))
            .await?;
        Ok(())
    }

    /// Restores the note's content as of `commit` (a commit in its history).
    pub async fn revert_note(
        &mut self,
        scope: UserScope,
        id: NoteId,
        commit: &str,
        author: Author,
    ) -> Result<NoteView> {
        let (path, _) = self.live(id)?;
        let dir = self.dir.clone();
        let p = path.clone();
        let c = commit.to_owned();
        let content = blocking(move || {
            let history = git::history(&dir, &p)?;
            let Some(rev) = history
                .iter()
                .find(|r| r.commit.id == c && r.change != git::Change::Deleted)
            else {
                return Ok(None);
            };
            git::blob_at(&dir, &c, &rev.path)
        })
        .await?
        .ok_or(VaultError::NotFound)?;
        let text = String::from_utf8_lossy(&content).into_owned();
        if Document::parse(&text)
            .frontmatter()
            .and_then(|f| f.id().ok().flatten())
            != Some(id.as_ulid())
        {
            return Err(VaultError::NotFound);
        }
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            vec![(path.clone(), Some(content))],
            author.message("revert", &path),
        )
        .await?;
        self.view(id).await
    }

    /// Reverts a whole commit (typically an `ai:` commit): a three-way revert on top of the
    /// current state, one new commit. Returns the new commit ID and the paths it changed.
    pub async fn revert_commit(
        &mut self,
        scope: UserScope,
        commit: &str,
        author: Author,
    ) -> Result<(Option<String>, Vec<String>)> {
        let dir = self.dir.clone();
        let c = commit.to_owned();
        let changes = blocking(move || {
            if git::find_commit(&dir, &c)?.is_none() {
                return Err(VaultError::NotFound);
            }
            match git::revert_changes(&dir, &c) {
                Err(VaultError::RevertConflict) => crate::revert::structured(&dir, &c),
                other => other,
            }
        })
        .await?;
        let paths: Vec<String> = changes.iter().map(|(p, _)| p.clone()).collect();
        let tx = self.begin(&scope).await?;
        let short: String = commit.chars().take(12).collect();
        let id = self
            .finish(
                tx,
                changes,
                author.message("revert", &format!("commit {short}")),
            )
            .await?;
        Ok((id, paths))
    }

    /// Appends block ID `block_id` to the block starting at body byte `block_start` of note
    /// `id` (the only automated body edit, PLAN §6.3), e.g. before citing it.
    pub async fn append_block_id(
        &mut self,
        scope: UserScope,
        id: NoteId,
        block_start: usize,
        block_id: &str,
        author: Author,
    ) -> Result<NoteView> {
        let (path, _) = self.live(id)?;
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let mut doc = Document::parse(&text);
        let appended = vault_format::blocks::append_block_id(doc.body(), block_start, block_id)
            .map_err(|_| VaultError::invalid("a block id cannot be appended there"))?;
        doc.set_body(appended.body);
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            vec![(path.clone(), Some(doc.render().into_bytes()))],
            author.message("cite", &path),
        )
        .await?;
        self.view(id).await
    }

    /// Saves a capture in `inbox/YYYY-MM-DD-HHmmss.md` before anything else (PLAN §6.9,
    /// principle 5). Never refused as a duplicate: likely duplicates become a `duplicate`
    /// suggestion on the inbox note.
    pub async fn capture(&mut self, scope: UserScope, text: String) -> Result<Captured> {
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        let stamp = now.format("%Y-%m-%d-%H%M%S").to_string();
        let path = self.free_path(&format!("{}/{stamp}.md", paths::INBOX_DIR), false)?;
        let id = NoteId::generate(self.ids());
        let mut body = text;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        let mut doc = Document::parse("");
        doc.set_body(body);
        prepare::stamp(&mut doc, id, Some(&now), None)?;
        let content = doc.render();
        // The capture hits disk (and git) first; the duplicate check runs afterwards.
        let candidates = self
            .check_duplicates(&mut tx, &path, &Document::parse(&content), id)
            .await?;
        self.finish(
            tx,
            vec![(path.clone(), Some(content.into_bytes()))],
            Author::User.message("capture", &path),
        )
        .await?;
        let mut suggestion = None;
        if !candidates.is_empty() {
            let payload = rmp_serde::to_vec_named(&DuplicatePayload::from_candidates(&candidates))
                .map_err(|e| VaultError::Internal(e.to_string()))?;
            let mut tx = self.begin(&scope).await?;
            let sid = SuggestionId::generate(self.ids());
            suggestions::create_suggestion(
                &mut tx,
                sid,
                Some(id),
                "duplicate",
                &payload,
                self.now(),
            )
            .await?;
            let sid_text = sid.to_string();
            strata_index::repo::sync::append_change(
                &mut tx,
                &strata_index::repo::sync::NewChange {
                    entity_type: "suggestion",
                    entity_id: &sid_text,
                    op: strata_index::types::ChangeOp::Upsert,
                    version: None,
                    at: self.now(),
                },
            )
            .await?;
            tx.commit().await?;
            suggestion = Some(sid);
        }
        Ok(Captured {
            note: self.view(id).await?,
            duplicates: candidates,
            suggestion,
        })
    }

    /// Records keep-both between note `id` and `others` (accepting a duplicate suggestion).
    pub async fn keep_both_notes(
        &mut self,
        scope: UserScope,
        id: NoteId,
        others: &[Candidate],
        author: Author,
    ) -> Result<()> {
        let (path, _) = self.live(id)?;
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let item = dup::note_item(&path, Some(id), &Document::parse(&text));
        let mut sc = self
            .sidecar(id)
            .await?
            .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
        self.keep_both(&mut sc, &item, others);
        let tx = self.begin(&scope).await?;
        self.finish(
            tx,
            vec![Core::sidecar_change(&sc)?],
            author.message("keep-both", &path),
        )
        .await?;
        Ok(())
    }
}

/// Created/updated of a parsed note (fallback: the ULID's time).
pub(crate) fn timestamps(
    doc: &Document,
    id: NoteId,
) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    let fm = doc.frontmatter();
    let created = fm.and_then(|f| f.created().ok().flatten()).map_or_else(
        || {
            chrono::DateTime::from_timestamp_millis(
                i64::try_from(id.as_ulid().timestamp_ms()).unwrap_or(0),
            )
            .unwrap_or_default()
        },
        |c| c.with_timezone(&chrono::Utc),
    );
    let updated = fm
        .and_then(|f| f.updated().ok().flatten())
        .map_or(created, |u| u.with_timezone(&chrono::Utc))
        .max(created);
    (created, updated)
}
