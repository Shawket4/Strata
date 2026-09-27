//! People, companies, documents and places (PLAN §6.7, §6.12): manual create (with the
//! duplicate check), patch of user fields and aliases (renames rewrite links in the same
//! commit), merge, manual custody events, and the aggregated read views.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use domain::{CopyKind, NoteKind};
use strata_common::NoteId;
use strata_index::UserScope;
use strata_index::repo::entities::{self as erepo, CustodyEvent, Document as DocRow, Entity};
use strata_index::repo::vault::{self as vrepo, DocumentFilter, EntityHit};
use strata_index::repo::{graph, notes};
use strata_index::types::{DocStatus, EntityKind};
use text_normalize::normalize_for_search;
use vault_format::custody::{self, CustodyEventType, CustodyState, Role};
use vault_format::filename::{sanitize_file_name, unique_name};
use vault_format::frontmatter::KnownKey;
use vault_format::sections::{self, AiProfile, AiSection};
use vault_format::sidecar::NoteSidecar;
use vault_format::{Document, PropertyValue, Resolution, WikiLink};

use crate::dup::{self, NewItem};
use crate::error::{Result, VaultError};
use crate::model::{NoteView, PlaceTree};
use crate::paths;
use crate::prepare;
use crate::store::{Author, Core, VaultService};

/// User-editable fields per kind (contact fields are only ever written by the user).
pub fn user_fields(kind: NoteKind) -> &'static [&'static str] {
    match kind {
        NoteKind::Person => &["role", "phone", "email"],
        NoteKind::Company => &["industry", "website"],
        NoteKind::Document => &["doc-type", "copy", "expires"],
        NoteKind::Place => &["address"],
        NoteKind::Note | NoteKind::Concept => &[],
    }
}

/// `POST /entities`, `/documents`, `/places`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntity {
    /// Kind (person, company, document, place).
    pub kind: NoteKind,
    /// Name (the file name; sanitised).
    pub name: String,
    /// Aliases (both scripts).
    pub aliases: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// User fields (see [`user_fields`]).
    pub fields: BTreeMap<String, String>,
    /// Places: the enclosing place (`part-of`).
    pub parent: Option<NoteId>,
    /// Client-generated ID.
    pub id: Option<NoteId>,
    /// Create even if it looks like a duplicate.
    pub force: bool,
}

/// `PATCH /entities/{id}` (and documents/places).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntityPatch {
    /// New name (renames the file and rewrites links).
    pub name: Option<String>,
    /// Replaces the aliases.
    pub aliases: Option<Vec<String>>,
    /// Replaces the tags.
    pub tags: Option<Vec<String>>,
    /// Sets (`Some`) or removes (`None`) user fields.
    pub fields: BTreeMap<String, Option<String>>,
    /// Places: sets (`Some(Some)`) or clears (`Some(None)`) the enclosing place.
    pub parent: Option<Option<NoteId>>,
    /// Current version, if the client checks it.
    pub if_match: Option<String>,
    /// Save new aliases even if they match other entities.
    pub force: bool,
}

/// A manual custody event (`POST /documents/{id}/custody`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCustodyEvent {
    /// Type.
    pub kind: CustodyEventType,
    /// Date (resolved).
    pub date: NaiveDate,
    /// Place involved.
    pub place: Option<NoteId>,
    /// Person involved.
    pub person: Option<NoteId>,
    /// Third party involved.
    pub counterparty: Option<NoteId>,
    /// The note that states the event (cited); the document itself when absent.
    pub source: Option<NoteId>,
}

fn kind_folder(kind: NoteKind) -> &'static str {
    kind.default_folder()
}

fn validate_field(key: &str, value: &str) -> Result<()> {
    match key {
        "copy" if value.parse::<CopyKind>().is_err() => Err(VaultError::invalid(
            "copy must be original, certified copy, copy or digital",
        )),
        "expires" if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() => {
            Err(VaultError::invalid("expires must be a date YYYY-MM-DD"))
        }
        _ if value.contains(['\n', '\r']) => {
            Err(VaultError::invalid("field values are single lines"))
        }
        _ => Ok(()),
    }
}

fn known(key: &str) -> Result<KnownKey> {
    KnownKey::from_name(key).ok_or(VaultError::invalid("unknown field"))
}

fn clean_list(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in items {
        let t = i.trim().trim_start_matches('#').trim().to_owned();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// The entity kind of a note kind.
pub fn entity_kind(kind: NoteKind) -> Option<EntityKind> {
    crate::derive::entity_kind(kind)
}

impl Core {
    fn entity_meta(&self, id: NoteId) -> Result<(String, String, NoteKind)> {
        let state = self.state()?;
        let (path, meta) = state.note(id).ok_or(VaultError::NotFound)?;
        if entity_kind(meta.kind).is_none() {
            return Err(VaultError::NotFound);
        }
        Ok((path.to_owned(), meta.version.clone(), meta.kind))
    }

    fn link_to(&self, id: NoteId) -> Result<String> {
        let state = self.state()?;
        let (path, _) = state.note(id).ok_or(VaultError::NotFound)?;
        Ok(format!("[[{}]]", state.path_index().link_text_for(path)))
    }

    fn kind_of_id(&self, id: NoteId) -> Option<NoteKind> {
        self.state().ok()?.note(id).map(|(_, m)| m.kind)
    }

    /// Creates an entity note (`people/`, `companies/`, `documents/`, `places/`).
    pub async fn create_entity(&mut self, scope: UserScope, req: NewEntity) -> Result<NoteView> {
        if entity_kind(req.kind).is_none() {
            return Err(VaultError::invalid("kind must be person, company, document or place"));
        }
        let name = req.name.trim();
        if name.is_empty() {
            return Err(VaultError::invalid("the name is empty"));
        }
        for (k, v) in &req.fields {
            if !user_fields(req.kind).contains(&k.as_str()) {
                return Err(VaultError::invalid("a field is not editable for this kind"));
            }
            validate_field(k, v)?;
        }
        if let Some(p) = req.parent
            && (req.kind != NoteKind::Place || self.kind_of_id(p) != Some(NoteKind::Place))
        {
            return Err(VaultError::invalid("parent must be a place (and only places nest)"));
        }
        if let Some(id) = req.id
            && self.state()?.contains_id(id)
        {
            return Err(VaultError::invalid("a note with this id already exists"));
        }
        let aliases = clean_list(&req.aliases);
        let mut tx = self.begin(&scope).await?;
        let candidates = dup::find(
            &mut tx,
            &NewItem {
                kind: req.kind.as_str(),
                id: req.id.map(|i| i.to_string()),
                text: name,
                exact: None,
                aliases: &aliases,
            },
            &self.inner.config.near_thresholds,
        )
        .await?;
        if !candidates.is_empty() && !req.force {
            return Err(VaultError::Duplicate(candidates));
        }
        let folder = kind_folder(req.kind);
        let stem = sanitize_file_name(name);
        let state = self.state()?;
        let taken: Vec<&str> = state
            .notes
            .keys()
            .filter(|p| paths::parent(p) == folder)
            .filter_map(|p| paths::file_name(p).strip_suffix(".md"))
            .collect();
        let path = format!("{folder}/{}.md", unique_name(&stem, taken));
        let id = req.id.unwrap_or_else(|| NoteId::generate(self.ids()));
        let mut doc = Document::parse("## Notes\n");
        {
            let fm = doc.frontmatter_mut();
            let err = |_| VaultError::invalid("the property could not be set");
            fm.set_kind(req.kind).map_err(err)?;
            if stem != name {
                fm.set_text(KnownKey::Title, name).map_err(err)?;
            }
            if !aliases.is_empty() {
                fm.set_list(KnownKey::Aliases, aliases.clone()).map_err(err)?;
            }
            let tags = clean_list(&req.tags);
            if !tags.is_empty() {
                fm.set_list(KnownKey::Tags, tags).map_err(err)?;
            }
            for (k, v) in &req.fields {
                fm.set_text(known(k)?, v.clone()).map_err(err)?;
            }
        }
        if let Some(p) = req.parent {
            let link = self.link_to(p)?;
            doc.frontmatter_mut()
                .set_relation(
                    vault_format::RelationKey::Note(domain::RelationType::PartOf),
                    vec![link],
                )
                .map_err(|_| VaultError::invalid("the property could not be set"))?;
        }
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        prepare::stamp(&mut doc, id, Some(&now), Some(&now))?;
        let mut changes = vec![(path.clone(), Some(doc.render().into_bytes()))];
        if !candidates.is_empty() {
            let mut sc = NoteSidecar::new(id.as_ulid());
            self.keep_both(&mut sc, &candidates);
            changes.push(Core::sidecar_change(&sc)?);
        }
        self.finish(tx, changes, Author::User.message("create", &path))
            .await?;
        self.view(id).await
    }

    /// Edits user fields, aliases, tags, the name (rename) and, for places, the parent.
    pub async fn patch_entity(
        &mut self,
        scope: UserScope,
        id: NoteId,
        patch: EntityPatch,
    ) -> Result<NoteView> {
        let (path, version, kind) = self.entity_meta(id)?;
        if let Some(m) = &patch.if_match
            && *m != version
        {
            return Err(VaultError::VersionConflict { current: version });
        }
        for (k, v) in &patch.fields {
            if !user_fields(kind).contains(&k.as_str()) {
                return Err(VaultError::invalid("a field is not editable for this kind"));
            }
            if let Some(v) = v {
                validate_field(k, v)?;
            }
        }
        if patch.parent.is_some() && kind != NoteKind::Place {
            return Err(VaultError::invalid("only places have a parent"));
        }
        if let Some(Some(p)) = patch.parent {
            if self.kind_of_id(p) != Some(NoteKind::Place) || p == id {
                return Err(VaultError::invalid("parent must be another place"));
            }
            let mut tx = self.begin(&scope).await?;
            if erepo::place_subtree(&mut tx, id).await?.contains(&p) {
                return Err(VaultError::invalid("a place cannot be nested inside itself"));
            }
            tx.commit().await?;
        }
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let mut doc = Document::parse(&text);
        let mut tx = self.begin(&scope).await?;
        let mut keep: Vec<crate::error::Candidate> = Vec::new();
        {
            let old_aliases = doc.frontmatter().map(|f| f.aliases()).unwrap_or_default();
            let fm = doc.frontmatter_mut();
            if fm.error().is_some() {
                return Err(VaultError::invalid("the frontmatter cannot be edited"));
            }
            let err = |_| VaultError::invalid("the property could not be set");
            if let Some(aliases) = &patch.aliases {
                let aliases = clean_list(aliases);
                let added: Vec<String> = aliases
                    .iter()
                    .filter(|a| !old_aliases.contains(a))
                    .cloned()
                    .collect();
                for a in &added {
                    let found = dup::find(
                        &mut tx,
                        &NewItem {
                            kind: kind.as_str(),
                            id: Some(id.to_string()),
                            text: a,
                            exact: None,
                            aliases: &[],
                        },
                        &self.inner.config.near_thresholds,
                    )
                    .await?;
                    keep.extend(found);
                }
                keep.sort_by(|a, b| a.id.cmp(&b.id));
                keep.dedup_by(|a, b| a.id == b.id);
                if !keep.is_empty() && !patch.force {
                    return Err(VaultError::Duplicate(keep));
                }
                if aliases.is_empty() {
                    fm.remove_key(KnownKey::Aliases).map_err(err)?;
                } else {
                    fm.set_list(KnownKey::Aliases, aliases).map_err(err)?;
                }
            }
            if let Some(tags) = &patch.tags {
                let tags = clean_list(tags);
                if tags.is_empty() {
                    fm.remove_key(KnownKey::Tags).map_err(err)?;
                } else {
                    fm.set_list(KnownKey::Tags, tags).map_err(err)?;
                }
            }
            for (k, v) in &patch.fields {
                match v {
                    Some(v) => fm.set_text(known(k)?, v.clone()).map_err(err)?,
                    None => {
                        fm.remove_key(known(k)?).map_err(err)?;
                    }
                }
            }
        }
        if let Some(parent) = patch.parent {
            let rel = vault_format::RelationKey::Note(domain::RelationType::PartOf);
            let fm = doc.frontmatter_mut();
            match parent {
                Some(p) => {
                    let link = self.link_to(p)?;
                    fm.set_relation(rel, vec![link])
                        .map_err(|_| VaultError::invalid("the property could not be set"))?;
                }
                None => {
                    fm.remove_key(rel.key())
                        .map_err(|_| VaultError::invalid("the property could not be set"))?;
                }
            }
        }
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        prepare::stamp(&mut doc, id, None, Some(&now))?;
        let mut new_path = path.clone();
        if let Some(name) = &patch.name {
            let name = name.trim();
            if name.is_empty() {
                return Err(VaultError::invalid("the name is empty"));
            }
            let stem = sanitize_file_name(name);
            let fm = doc.frontmatter_mut();
            if stem == name {
                fm.remove_key(KnownKey::Title)
                    .map_err(|_| VaultError::invalid("the property could not be set"))?;
            } else {
                fm.set_text(KnownKey::Title, name)
                    .map_err(|_| VaultError::invalid("the property could not be set"))?;
            }
            new_path = paths::join(paths::parent(&path), &format!("{stem}.md"));
            let state = self.state()?;
            if new_path != path
                && (state.notes.contains_key(&new_path) || state.attachments.contains(&new_path))
            {
                return Err(VaultError::PathTaken);
            }
        }
        let rendered = doc.render();
        let mut changes = if new_path == path {
            vec![(path.clone(), Some(rendered.into_bytes()))]
        } else {
            self.plan_move(&path, &new_path, Some(rendered)).await?
        };
        if !keep.is_empty() {
            let mut sc = self
                .sidecar(id)
                .await?
                .unwrap_or_else(|| NoteSidecar::new(id.as_ulid()));
            self.keep_both(&mut sc, &keep);
            changes.push(Core::sidecar_change(&sc)?);
        }
        let subject = if new_path == path {
            path.clone()
        } else {
            format!("{path} -> {new_path}")
        };
        self.finish(tx, changes, Author::User.message("update", &subject))
            .await?;
        self.view(id).await
    }

    /// Merges entity `loser` into `survivor` (PLAN §6.7): every link and relation to the
    /// loser is rewritten to the survivor (sidecar references too), aliases are unioned (the
    /// loser's name becomes an alias), the loser's `## Notes` move under a dated sub-heading
    /// of the survivor's `## Notes`, and the loser goes to the trash — one commit.
    pub async fn merge_entities(
        &mut self,
        scope: UserScope,
        loser: NoteId,
        survivor: NoteId,
    ) -> Result<NoteView> {
        if loser == survivor {
            return Err(VaultError::invalid("an entity cannot be merged into itself"));
        }
        let (lpath, _, lkind) = self.entity_meta(loser)?;
        let (spath, _, skind) = self.entity_meta(survivor)?;
        if lkind != skind {
            return Err(VaultError::invalid("only entities of the same kind can be merged"));
        }
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        let state = self.state()?;
        let index = state.path_index();
        let ltext = self.read_text(&lpath).await?.ok_or(VaultError::NotFound)?;
        let stext = self.read_text(&spath).await?.ok_or(VaultError::NotFound)?;
        let ldoc = Document::parse(&ltext);
        let mut sdoc = Document::parse(&stext);
        let ltitle = crate::derive::title_of(&lpath, &ldoc);
        let stitle = crate::derive::title_of(&spath, &sdoc);

        // Aliases.
        let mut aliases = sdoc.frontmatter().map(|f| f.aliases()).unwrap_or_default();
        for a in std::iter::once(ltitle.clone())
            .chain(ldoc.frontmatter().map(|f| f.aliases()).unwrap_or_default())
        {
            if a != stitle && !aliases.contains(&a) {
                aliases.push(a);
            }
        }
        // Entity relations of the loser that the survivor lacks.
        let mut rel_lists: Vec<(vault_format::RelationKey, Vec<String>)> = Vec::new();
        if let Some(lfm) = ldoc.frontmatter() {
            for rk in vault_format::RelationKey::all() {
                let items = lfm.relation(rk);
                if !items.is_empty() {
                    rel_lists.push((rk, items));
                }
            }
        }
        {
            let fm = sdoc.frontmatter_mut();
            let err = |_| VaultError::invalid("the survivor's frontmatter cannot be edited");
            if fm.error().is_some() {
                return Err(VaultError::invalid("the survivor's frontmatter cannot be edited"));
            }
            fm.set_list(KnownKey::Aliases, aliases).map_err(err)?;
            for (rk, items) in rel_lists {
                let mut list = fm.relation(rk);
                for item in items {
                    let target = WikiLink::parse_exact(item.trim())
                        .map(|l| index.resolve(&l.path, Some(&lpath)));
                    let points_home = matches!(&target, Some(Resolution::Resolved(p)) if *p == spath || *p == lpath);
                    let dup = list.iter().any(|x| {
                        let a = WikiLink::parse_exact(x.trim()).map(|l| index.resolve(&l.path, Some(&spath)));
                        a.is_some() && a == target
                    });
                    if !points_home && !dup && !list.contains(&item) {
                        list.push(item);
                    }
                }
                fm.set_relation(rk, list).map_err(err)?;
            }
        }
        // Notes section.
        let lnotes = section_text(ldoc.body(), "Notes").unwrap_or_default();
        if !lnotes.trim().is_empty() {
            let heading = format!("### Merged from {ltitle} ({})", now.format("%Y-%m-%d"));
            let body = sdoc.body().to_owned();
            let secs = sections::sections(&body);
            let insert = format!("{heading}\n\n{}\n", lnotes.trim_end());
            let new_body = match secs
                .iter()
                .find(|s| s.level == 2 && s.title.trim().eq_ignore_ascii_case("Notes"))
            {
                Some(s) => {
                    let end = s.content_span.end;
                    let mut b = body[..end].to_owned();
                    if !b.ends_with('\n') {
                        b.push('\n');
                    }
                    if !b.ends_with("\n\n") {
                        b.push('\n');
                    }
                    b.push_str(&insert);
                    if end < body.len() {
                        b.push('\n');
                        b.push_str(&body[end..]);
                    }
                    b
                }
                None => {
                    let mut b = body.clone();
                    if !b.is_empty() && !b.ends_with('\n') {
                        b.push('\n');
                    }
                    if !b.is_empty() {
                        b.push('\n');
                    }
                    b.push_str("## Notes\n\n");
                    b.push_str(&insert);
                    b
                }
            };
            sdoc.set_body(new_body);
        }
        prepare::stamp(&mut sdoc, survivor, None, Some(&now))?;
        let mut changes: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        changes.insert(spath.clone(), Some(sdoc.render().into_bytes()));

        // Rewrite links to the loser in every other note.
        let survivor_link = index.link_text_for(&spath);
        let names: BTreeSet<String> = [crate::state::name_key(&lpath)].into();
        let sources: Vec<String> = state
            .linking_to(&names)
            .into_iter()
            .filter_map(|i| state.note(i).map(|(p, _)| p.to_owned()))
            .filter(|p| *p != lpath)
            .collect();
        for p in sources {
            let text = match changes.get(&p) {
                Some(Some(b)) => String::from_utf8_lossy(b).into_owned(),
                _ => self.read_text(&p).await?.unwrap_or_default(),
            };
            let mut d = Document::parse(&text);
            if retarget_links(&mut d, &p, &lpath, &spath, &survivor_link, &index) {
                changes.insert(p, Some(d.render().into_bytes()));
            }
        }
        // Sidecars referencing the loser by ID.
        let dir = self.dir.clone();
        let sidecar_files = crate::store::blocking(move || {
            let mut out = Vec::new();
            for f in crate::fsio::scan(&dir)? {
                if f.starts_with(".meta/notes/")
                    && let Some(b) = crate::fsio::read(&dir, &f)?
                {
                    out.push((f, String::from_utf8_lossy(&b).into_owned()));
                }
            }
            Ok(out)
        })
        .await?;
        for (f, text) in sidecar_files {
            let Ok(mut sc) = NoteSidecar::from_json(&text) else {
                continue;
            };
            if sc.id == loser.as_ulid() {
                continue;
            }
            let mut changed = false;
            for r in &mut sc.relations {
                if r.target_id == loser.as_ulid() {
                    r.target_id = survivor.as_ulid();
                    changed = true;
                }
            }
            for r in &mut sc.rejected {
                if r.target_id == loser.as_ulid() {
                    r.target_id = survivor.as_ulid();
                    changed = true;
                }
            }
            for k in &mut sc.keep_both {
                if k.other_id == loser.as_ulid() {
                    k.other_id = survivor.as_ulid();
                    changed = true;
                }
            }
            if changed {
                let (_, content) = Core::sidecar_change(&sc)?;
                changes.insert(f, content);
            }
        }
        // Soft-delete the loser.
        let trash = paths::trash_path(&lpath);
        let trash = {
            let st = self.state()?;
            let folder = paths::parent(&trash);
            let stem = paths::file_name(&trash).trim_end_matches(".md");
            let taken: Vec<&str> = st
                .trash
                .keys()
                .filter(|p| paths::parent(p) == folder)
                .map(|p| paths::file_name(p).trim_end_matches(".md"))
                .collect();
            paths::join(folder, &format!("{}.md", unique_name(stem, taken)))
        };
        changes.insert(lpath.clone(), None);
        changes.insert(trash, Some(ltext.into_bytes()));
        self.finish(
            tx,
            changes.into_iter().collect(),
            Author::User.message("merge", &format!("{lpath} -> {spath}")),
        )
        .await?;
        self.view(survivor).await
    }

    /// Records a manual custody event on a document: the `## Custody` line (cited) and the
    /// frontmatter state derived from the newest event, one `user:` commit.
    pub async fn add_custody_event(
        &mut self,
        scope: UserScope,
        document: NoteId,
        ev: NewCustodyEvent,
    ) -> Result<NoteView> {
        let (path, _, kind) = self.entity_meta(document)?;
        if kind != NoteKind::Document {
            return Err(VaultError::NotFound);
        }
        let check = |id: Option<NoteId>, allowed: &[NoteKind]| -> Result<Option<String>> {
            match id {
                None => Ok(None),
                Some(i) => {
                    let k = self.kind_of_id(i).ok_or(VaultError::NotFound)?;
                    if !allowed.contains(&k) {
                        return Err(VaultError::invalid("an event argument has the wrong kind"));
                    }
                    self.link_to(i).map(Some)
                }
            }
        };
        let place = check(ev.place, &[NoteKind::Place])?;
        let person = check(ev.person, &[NoteKind::Person])?;
        let counterparty = check(ev.counterparty, &[NoteKind::Person, NoteKind::Company])?;
        let primary_ok = match custody::primary(ev.kind) {
            Some(Role::Place) => place.is_some(),
            Some(Role::Person) => person.is_some(),
            Some(Role::Counterparty) => counterparty.is_some(),
            None => true,
        };
        if !primary_ok {
            return Err(VaultError::invalid("the event type's primary argument is missing"));
        }
        let citation = match ev.source {
            Some(s) => {
                self.state()?.note(s).ok_or(VaultError::NotFound)?;
                self.link_to(s)?
            }
            None => self.link_to(document)?,
        };
        let text = self.read_text(&path).await?.ok_or(VaultError::NotFound)?;
        let mut doc = Document::parse(&text);
        let mut events = crate::derive::custody_events(doc.body());
        let new = custody::CustodyEvent {
            date: ev.date,
            kind: ev.kind,
            place,
            person,
            counterparty,
            citations: vec![citation],
        };
        // Newest first; a new event on an existing date goes above the older ones.
        let at = events.iter().position(|e| e.date <= ev.date).unwrap_or(events.len());
        events.insert(at, new);
        let content = custody::render_section(&events);
        let body = sections::replace_ai_sections(
            doc.body(),
            AiProfile::Document,
            &[(AiSection::Custody, &content)],
        )
        .map_err(|_| VaultError::invalid("the custody section could not be written"))?;
        doc.set_body(body);
        let events = crate::derive::custody_events(doc.body());
        let mut tx = self.begin(&scope).await?;
        let tz = self.tz(&mut tx).await?;
        let now = self.local_now(tz);
        if let Some(state) = CustodyState::derive(&events) {
            state
                .write_to(doc.frontmatter_mut())
                .map_err(|_| VaultError::invalid("the frontmatter cannot be edited"))?;
        }
        prepare::stamp(&mut doc, document, None, Some(&now))?;
        self.finish(
            tx,
            vec![(path.clone(), Some(doc.render().into_bytes()))],
            Author::User.message("custody", &path),
        )
        .await?;
        self.view(document).await
    }
}

/// The own content of `## <title>` in `body`.
pub fn section_text(body: &str, title: &str) -> Option<String> {
    sections::sections(body)
        .into_iter()
        .find(|s| s.level == 2 && s.title.trim().eq_ignore_ascii_case(title))
        .map(|s| body[s.content_span].to_owned())
}

/// Points every link (body and link-holding frontmatter) that resolves to `from` at `to`.
/// Returns whether anything changed.
fn retarget_links(
    doc: &mut Document,
    source: &str,
    from: &str,
    to: &str,
    to_link: &str,
    index: &vault_format::PathIndex,
) -> bool {
    let resolves = |l: &WikiLink| index.resolve(&l.path, Some(source)) == Resolution::Resolved(from.to_owned());
    let rewritten = vault_format::rewrite::rewrite_body(doc.body(), |l| {
        resolves(l).then(|| l.with_path(to_link))
    });
    let mut changed = rewritten.changed > 0;
    if changed {
        doc.set_body(rewritten.text);
    }
    let targets_to = |s: &str| {
        WikiLink::parse_exact(s.trim())
            .is_some_and(|l| index.resolve(&l.path, Some(source)) == Resolution::Resolved(to.to_owned()))
    };
    let fm = doc.frontmatter_mut();
    if fm.error().is_some() {
        return changed;
    }
    for key in KnownKey::all().filter(|k| k.holds_links()) {
        let Some(value) = fm.get(key.as_str()).cloned() else {
            continue;
        };
        let map = |s: &str| {
            WikiLink::parse_exact(s.trim())
                .filter(|l| resolves(l))
                .map(|l| l.with_path(to_link))
        };
        match value {
            PropertyValue::Text(s) => {
                if let Some(new) = map(&s)
                    && fm.set(key.as_str(), PropertyValue::Text(new)).is_ok()
                {
                    changed = true;
                }
            }
            PropertyValue::List(items) => {
                let mut out: Vec<String> = Vec::new();
                let mut any = false;
                for s in &items {
                    match map(s) {
                        Some(new) => {
                            any = true;
                            if !out.iter().any(|x| targets_to(x)) {
                                out.push(new);
                            }
                        }
                        None => {
                            if !(targets_to(s) && out.iter().any(|x| targets_to(x))) {
                                out.push(s.clone());
                            }
                        }
                    }
                }
                if any && fm.set(key.as_str(), PropertyValue::List(out)).is_ok() {
                    changed = true;
                }
            }
            _ => {}
        }
    }
    changed
}

/// One entity in a list.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySummary {
    /// Entity row.
    pub entity: Entity,
    /// Note path.
    pub path: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Match score (0 without a query).
    pub score: f32,
}

/// A note mentioning an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentioningNote {
    /// Note.
    pub id: NoteId,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Last update.
    pub updated: chrono::DateTime<chrono::Utc>,
    /// The first line linking to the entity.
    pub snippet: Option<String>,
}

/// An edge of an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityEdge {
    /// Relation type.
    pub rel: String,
    /// Other note.
    pub other: NoteId,
    /// Other note title.
    pub title: String,
    /// `out` (this entity → other) or `in`.
    pub outgoing: bool,
    /// `user` / `ai`.
    pub by: String,
}

/// The aggregated entity page.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    /// The note.
    pub note: NoteView,
    /// Entity row.
    pub entity: Entity,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// AI sections and `## Notes`: (title, content), in body order.
    pub sections: Vec<(String, String)>,
    /// Mentioning notes, newest first (first page).
    pub mentions: Vec<MentioningNote>,
    /// Total mentioning notes.
    pub mention_count: usize,
    /// Relations to and from other notes (excluding `people`/`companies` mentions).
    pub relations: Vec<EntityEdge>,
    /// Documents held now / last handled / concerning this entity.
    pub documents: EntityDocuments,
}

/// Documents related to an entity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntityDocuments {
    /// Held now.
    pub holds: Vec<NoteId>,
    /// Last handled.
    pub last_handled: Vec<NoteId>,
    /// Listing the entity under `companies:` / `people:`.
    pub concerning: Vec<NoteId>,
}

/// A document with its custody.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentView {
    /// Entity page.
    pub note: NoteView,
    /// Document row.
    pub document: DocRow,
    /// Custody events, newest first.
    pub custody: Vec<CustodyEvent>,
    /// Location breadcrumb, outermost first (the location last).
    pub location_path: Vec<NoteId>,
    /// Copies of this document.
    pub copies: Vec<NoteId>,
}

impl VaultService {
    async fn entity_summaries(
        &self,
        scope: &UserScope,
        hits: Vec<EntityHit>,
    ) -> Result<Vec<EntitySummary>> {
        let mut tx = self.inner.db.begin(scope).await?;
        let mut out = Vec::new();
        for h in hits {
            let Some(n) = notes::get_note(&mut tx, h.note_id).await? else {
                continue;
            };
            out.push(EntitySummary {
                aliases: graph::aliases_for(&mut tx, h.note_id).await?,
                tags: graph::tags_for(&mut tx, h.note_id).await?,
                path: n.path,
                score: h.score,
                entity: h.entity(),
            });
        }
        tx.commit().await?;
        Ok(out)
    }

    /// Entities (optionally of one kind, matching `query` in any script, with `tag`).
    pub async fn list_entities(
        &self,
        scope: &UserScope,
        kind: Option<EntityKind>,
        query: Option<&str>,
        tag: Option<&str>,
        limit: u32,
    ) -> Result<Vec<EntitySummary>> {
        self.ready(scope).await?;
        let q = query.map(normalize_for_search).filter(|q| !q.is_empty());
        let mut tx = self.inner.db.begin(scope).await?;
        let hits = vrepo::search_entities(&mut tx, kind, q.as_deref(), tag, 0.3, i64::from(limit.clamp(1, 500))).await?;
        tx.commit().await?;
        self.entity_summaries(scope, hits).await
    }

    /// Notes mentioning the entity (frontmatter `people:`/`companies:` and body links),
    /// newest first, with the first line linking to it.
    pub async fn entity_notes(&self, scope: &UserScope, id: NoteId) -> Result<Vec<MentioningNote>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        if erepo::get_entity(&mut tx, id).await?.is_none() {
            return Err(VaultError::NotFound);
        }
        let me = notes::get_note(&mut tx, id).await?.ok_or(VaultError::NotFound)?;
        let mut ids: BTreeSet<NoteId> = erepo::mentions_of(&mut tx, id)
            .await?
            .into_iter()
            .map(|m| m.note_id)
            .collect();
        ids.extend(graph::backlinks(&mut tx, id).await?.into_iter().map(|l| l.src_id));
        ids.remove(&id);
        let mut rows = Vec::new();
        for i in ids {
            if let Some(n) = notes::get_note(&mut tx, i).await?
                && !n.trashed
            {
                rows.push(n);
            }
        }
        tx.commit().await?;
        let name = crate::state::name_key(&me.path);
        let mut out = Vec::new();
        for n in rows {
            let text = self.note_text(scope, &n.path).await?.unwrap_or_default();
            let doc = Document::parse(&text);
            let snippet = doc
                .body()
                .lines()
                .find(|line| {
                    vault_format::wikilink::find_all(line)
                        .iter()
                        .any(|l| crate::state::name_key(l.target()) == name)
                })
                .map(|l| l.trim().chars().take(200).collect());
            out.push(MentioningNote {
                id: n.id,
                path: n.path,
                title: n.title,
                updated: n.updated,
                snippet,
            });
        }
        out.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    pub(crate) async fn note_text(&self, scope: &UserScope, rel: &str) -> Result<Option<String>> {
        let dir = self.vault_dir(scope.user_id());
        let rel = rel.to_owned();
        Ok(crate::store::blocking(move || Ok(crate::fsio::read(&dir, &rel)?))
            .await?
            .map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// Documents held now, last handled, or concerning the entity.
    pub async fn entity_documents(&self, scope: &UserScope, id: NoteId) -> Result<EntityDocuments> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        if erepo::get_entity(&mut tx, id).await?.is_none() {
            return Err(VaultError::NotFound);
        }
        let out = EntityDocuments {
            holds: erepo::documents_held_by(&mut tx, id).await?,
            last_handled: vrepo::documents_last_held_by(&mut tx, id).await?,
            concerning: vrepo::documents_concerning(&mut tx, id).await?,
        };
        tx.commit().await?;
        Ok(out)
    }

    /// The aggregated entity page.
    pub async fn entity(&self, scope: &UserScope, id: NoteId, mention_limit: usize) -> Result<EntityView> {
        let note = self.note(scope, id).await?;
        if note.trashed {
            return Err(VaultError::NotFound);
        }
        let mut tx = self.inner.db.begin(scope).await?;
        let entity = erepo::get_entity(&mut tx, id).await?.ok_or(VaultError::NotFound)?;
        let aliases = graph::aliases_for(&mut tx, id).await?;
        let tags = graph::tags_for(&mut tx, id).await?;
        let mut relations = Vec::new();
        let skip = ["people", "companies", "concepts"];
        for r in graph::relations_from(&mut tx, id).await? {
            if skip.contains(&r.rel_type.as_str()) {
                continue;
            }
            let title = notes::get_note(&mut tx, r.dst_id).await?.map(|n| n.title).unwrap_or_default();
            relations.push(EntityEdge { rel: r.rel_type, other: r.dst_id, title, outgoing: true, by: r.by.as_str().to_owned() });
        }
        for r in graph::relations_to(&mut tx, id).await? {
            if skip.contains(&r.rel_type.as_str()) {
                continue;
            }
            let title = notes::get_note(&mut tx, r.src_id).await?.map(|n| n.title).unwrap_or_default();
            relations.push(EntityEdge { rel: r.rel_type, other: r.src_id, title, outgoing: false, by: r.by.as_str().to_owned() });
        }
        tx.commit().await?;
        let doc = Document::parse(&note.content);
        let sections: Vec<(String, String)> = sections::sections(doc.body())
            .into_iter()
            .filter(|s| s.level == 2)
            .map(|s| (s.title.clone(), doc.body()[s.own_content_span].trim().to_owned()))
            .collect();
        let mut mentions = self.entity_notes(scope, id).await?;
        let mention_count = mentions.len();
        mentions.truncate(mention_limit);
        let documents = self.entity_documents(scope, id).await?;
        Ok(EntityView { note, entity, aliases, tags, sections, mentions, mention_count, relations, documents })
    }

    /// Documents matching the filter (`place` includes nested places).
    pub async fn list_documents(&self, scope: &UserScope, mut filter: DocumentFilter) -> Result<Vec<(DocRow, EntitySummary)>> {
        self.ready(scope).await?;
        filter.query = filter.query.map(|q| normalize_for_search(&q)).filter(|q| !q.is_empty());
        let mut tx = self.inner.db.begin(scope).await?;
        let docs = vrepo::list_documents(&mut tx, &filter, 0.3).await?;
        let mut hits = Vec::new();
        for d in &docs {
            if let Some(e) = erepo::get_entity(&mut tx, d.note_id).await? {
                hits.push(EntityHit { note_id: e.note_id, kind: e.kind, display_name: e.display_name, role: e.role, industry: e.industry, score: 0.0 });
            }
        }
        tx.commit().await?;
        let summaries = self.entity_summaries(scope, hits).await?;
        Ok(docs.into_iter().zip(summaries).collect())
    }

    /// A document with custody history, location breadcrumb and copies.
    pub async fn document(&self, scope: &UserScope, id: NoteId) -> Result<DocumentView> {
        let note = self.note(scope, id).await?;
        if note.trashed {
            return Err(VaultError::NotFound);
        }
        let mut tx = self.inner.db.begin(scope).await?;
        let document = erepo::get_document(&mut tx, id).await?.ok_or(VaultError::NotFound)?;
        let custody = erepo::custody_history(&mut tx, id).await?;
        let mut location_path = Vec::new();
        if let Some(loc) = document.location_id {
            location_path = vrepo::place_ancestors(&mut tx, loc).await?;
            location_path.reverse();
            location_path.push(loc);
        }
        let copies = vrepo::copies_of(&mut tx, id).await?;
        tx.commit().await?;
        Ok(DocumentView { note, document, custody, location_path, copies })
    }

    /// A place's ancestors, children and every document inside it (recursively).
    pub async fn place(&self, scope: &UserScope, id: NoteId) -> Result<(NoteView, PlaceTree)> {
        let note = self.note(scope, id).await?;
        if note.trashed || note.kind != NoteKind::Place {
            return Err(VaultError::NotFound);
        }
        let mut tx = self.inner.db.begin(scope).await?;
        if erepo::place_parent(&mut tx, id).await?.is_none() {
            return Err(VaultError::NotFound);
        }
        let tree = PlaceTree {
            ancestors: vrepo::place_ancestors(&mut tx, id).await?,
            children: vrepo::place_children(&mut tx, id).await?,
            documents: erepo::documents_in_place(&mut tx, id).await?,
        };
        tx.commit().await?;
        Ok((note, tree))
    }

    /// The status of a document status string (for filters).
    pub fn parse_status(s: &str) -> Option<DocStatus> {
        s.parse().ok()
    }
}
