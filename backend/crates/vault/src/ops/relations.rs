//! Relations (PLAN §6.4, §6.5, §7.5 Relations): typed edges stored as wikilink lists in the
//! source note's frontmatter, with provenance in its sidecar. Users add, retype and remove
//! them; removing or retyping an AI edge records a rejection (sidecar + `rejected` table) so
//! the AI never re-adds it. AI jobs add edges with provenance in one `ai:` commit.

use strata_common::NoteId;
use strata_index::UserScope;
use vault_format::sidecar::{By, NoteSidecar, RejectedRelation, SidecarRelation};
use vault_format::{Document, PathIndex, RelationKey, Resolution, WikiLink};

use crate::error::{Result, VaultError};
use crate::store::{Author, Core};

/// An edge proposed by an AI job.
#[derive(Debug, Clone, PartialEq)]
pub struct AiEdge {
    /// Relation type.
    pub rel: RelationKey,
    /// Target note.
    pub dst: NoteId,
    /// Confidence 0–1.
    pub confidence: f64,
    /// One-line reason.
    pub reason: String,
    /// `provider/model`.
    pub model: String,
}

/// Whether frontmatter list item `item` (a wikilink) resolves to `dst_path`.
fn resolves_to(item: &str, dst_path: &str, index: &PathIndex, src_path: &str) -> bool {
    WikiLink::parse_exact(item.trim()).is_some_and(|l| {
        index.resolve(&l.path, Some(src_path)) == Resolution::Resolved(dst_path.to_owned())
    })
}

/// Removes every link to `dst_path` from relation `rel`. Returns how many were removed.
fn remove_links(
    doc: &mut Document,
    rel: RelationKey,
    dst_path: &str,
    index: &PathIndex,
    src_path: &str,
) -> Result<usize> {
    let fm = doc.frontmatter_mut();
    let items = fm.relation(rel);
    let kept: Vec<String> = items
        .iter()
        .filter(|i| !resolves_to(i, dst_path, index, src_path))
        .cloned()
        .collect();
    let removed = items.len() - kept.len();
    if removed > 0 {
        if kept.is_empty() {
            fm.remove_key(rel.key())
                .map_err(|_| VaultError::invalid("the frontmatter cannot be edited"))?;
        } else {
            fm.set_relation(rel, kept)
                .map_err(|_| VaultError::invalid("the frontmatter cannot be edited"))?;
        }
    }
    Ok(removed)
}

/// Adds a link to `dst_path` to relation `rel` unless one already resolves there.
fn add_link(
    doc: &mut Document,
    rel: RelationKey,
    dst_path: &str,
    index: &PathIndex,
    src_path: &str,
) -> Result<bool> {
    let fm = doc.frontmatter_mut();
    if fm.error().is_some() {
        return Err(VaultError::invalid("the frontmatter cannot be edited"));
    }
    let mut items = fm.relation(rel);
    if items
        .iter()
        .any(|i| resolves_to(i, dst_path, index, src_path))
    {
        return Ok(false);
    }
    items.push(format!("[[{}]]", index.link_text_for(dst_path)));
    fm.set_relation(rel, items)
        .map_err(|_| VaultError::invalid("the frontmatter cannot be edited"))?;
    Ok(true)
}

impl Core {
    fn edge_paths(&self, src: NoteId, dst: NoteId) -> Result<(String, String, String)> {
        if src == dst {
            return Err(VaultError::invalid("a note cannot relate to itself"));
        }
        let (src_path, version) = self.live(src)?;
        let (dst_path, _) = self.live(dst)?;
        Ok((src_path, dst_path, version))
    }

    async fn load(&self, id: NoteId, path: &str) -> Result<(Document, NoteSidecar)> {
        let text = self.read_text(path).await?.ok_or(VaultError::NotFound)?;
        let sc = self
            .sidecar(id)
            .await?
            .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
        Ok((Document::parse(&text), sc))
    }

    async fn save(
        &mut self,
        scope: UserScope,
        path: &str,
        doc: &Document,
        sc: &NoteSidecar,
        message: String,
    ) -> Result<Option<String>> {
        let tx = self.begin(&scope).await?;
        let changes = vec![
            (path.to_owned(), Some(doc.render().into_bytes())),
            Core::sidecar_change(sc)?,
        ];
        self.finish(tx, changes, message).await
    }

    /// Adds a user edge `src -[rel]-> dst` (no-op if it exists). Returns whether it was added.
    pub async fn add_relation(
        &mut self,
        scope: UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
    ) -> Result<bool> {
        let (src_path, dst_path, _) = self.edge_paths(src, dst)?;
        let (mut doc, sc) = self.load(src, &src_path).await?;
        let index = self.state()?.path_index();
        if !add_link(&mut doc, rel, &dst_path, &index, &src_path)? {
            return Ok(false);
        }
        self.save(
            scope,
            &src_path,
            &doc,
            &sc,
            Author::User.message("relation add", &src_path),
        )
        .await?;
        Ok(true)
    }

    fn reject(&self, sc: &mut NoteSidecar, rel: RelationKey, dst: NoteId) -> bool {
        let was_ai = sc
            .relations
            .iter()
            .any(|r| r.kind == rel && r.target_id == dst.as_ulid() && r.by == By::Ai);
        sc.relations
            .retain(|r| !(r.kind == rel && r.target_id == dst.as_ulid()));
        if was_ai
            && !sc
                .rejected
                .iter()
                .any(|r| r.kind == rel && r.target_id == dst.as_ulid())
        {
            sc.rejected.push(RejectedRelation {
                kind: rel,
                target_id: dst.as_ulid(),
                at: self.now().fixed_offset(),
            });
        }
        was_ai
    }

    /// Removes the edge; an AI edge is recorded as rejected. Returns whether it was by AI.
    pub async fn remove_relation(
        &mut self,
        scope: UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
    ) -> Result<bool> {
        let (src_path, dst_path, _) = self.edge_paths(src, dst)?;
        let (mut doc, mut sc) = self.load(src, &src_path).await?;
        let index = self.state()?.path_index();
        if remove_links(&mut doc, rel, &dst_path, &index, &src_path)? == 0 {
            return Err(VaultError::NotFound);
        }
        let was_ai = self.reject(&mut sc, rel, dst);
        self.save(
            scope,
            &src_path,
            &doc,
            &sc,
            Author::User.message("relation remove", &src_path),
        )
        .await?;
        Ok(was_ai)
    }

    /// Changes an edge's type (a retyped AI edge is recorded as rejected under its old type).
    pub async fn retype_relation(
        &mut self,
        scope: UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
        new_rel: RelationKey,
    ) -> Result<()> {
        if rel == new_rel {
            return Err(VaultError::invalid("the new type equals the old type"));
        }
        let (src_path, dst_path, _) = self.edge_paths(src, dst)?;
        let (mut doc, mut sc) = self.load(src, &src_path).await?;
        let index = self.state()?.path_index();
        if remove_links(&mut doc, rel, &dst_path, &index, &src_path)? == 0 {
            return Err(VaultError::NotFound);
        }
        add_link(&mut doc, new_rel, &dst_path, &index, &src_path)?;
        self.reject(&mut sc, rel, dst);
        sc.relations
            .retain(|r| !(r.kind == new_rel && r.target_id == dst.as_ulid()));
        self.save(
            scope,
            &src_path,
            &doc,
            &sc,
            Author::User.message("relation retype", &src_path),
        )
        .await?;
        Ok(())
    }

    /// Applies AI edges to `src` (frontmatter + sidecar provenance) in one `ai: <job>`
    /// commit. Rejected (blocked) edges and edges to missing notes are skipped. Returns the
    /// commit, or `None` if nothing changed.
    pub async fn ai_add_relations(
        &mut self,
        scope: UserScope,
        job: &str,
        src: NoteId,
        edges: Vec<AiEdge>,
    ) -> Result<Option<String>> {
        let (src_path, _) = self.live(src)?;
        let (mut doc, mut sc) = self.load(src, &src_path).await?;
        let index = self.state()?.path_index();
        let mut changed = false;
        for e in edges {
            if e.dst == src || sc.is_blocked(e.rel, e.dst.as_ulid()) {
                continue;
            }
            let Ok((dst_path, _)) = self.live(e.dst) else {
                continue;
            };
            if add_link(&mut doc, e.rel, &dst_path, &index, &src_path)? {
                sc.relations
                    .retain(|r| !(r.kind == e.rel && r.target_id == e.dst.as_ulid()));
                sc.relations.push(SidecarRelation {
                    kind: e.rel,
                    target_id: e.dst.as_ulid(),
                    by: By::Ai,
                    confidence: Some(e.confidence),
                    reason: Some(e.reason),
                    model: Some(e.model),
                    created: self.now().fixed_offset(),
                });
                changed = true;
            }
        }
        if !changed {
            return Ok(None);
        }
        self.save(
            scope,
            &src_path,
            &doc,
            &sc,
            Author::Ai(job.to_owned()).message("", &src_path),
        )
        .await
    }
}
