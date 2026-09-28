//! New entity, document, place (PLAN §6.7, §6.12) and concept (§6.6) notes.
//!
//! **Title rule** (user and AI creates alike): `title` is written whenever the note's file stem
//! differs from the item's trimmed name — the name had characters a file name cannot hold, or
//! the name was taken and the note is `Ahmed 2.md` (`title: Ahmed`).
//!
//! A user-created entity note is, in this order: `kind`; `title` (the title rule); `aliases` and `tags`
//! (cleaned, [`clean_list`]); the user fields; `part-of` for a place inside another place;
//! `id`, `created`, `updated`; then the user relations (`companies`, `people`, `copy-of`, …).
//! The body is the user-owned `## Notes` section (the AI adds its sections above it later).
//! Frontmatter keys are written in the canonical order (§6.4) whatever the order of the calls.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use domain::{NoteKind, RelationType};
use sync_model::ops::{DocumentCreate, EntityCreate, PlaceCreate};
use ulid::Ulid;
use vault_format::{Document, KnownKey, PropertyValue, RelationKey};

use crate::RenderError;
use crate::note::{clean_list, set_list_if_any, stamp};
use crate::paths::{entity_path, entity_stem, file_name};

/// Body of a new entity, document or place note: the user-owned section only.
pub const ENTITY_BODY: &str = "## Notes\n";

/// A summary as one paragraph: whitespace runs (line breaks included) become one space and
/// leading `#` are dropped, so it can never start a heading.
pub fn one_paragraph(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches('#')
        .trim()
        .to_owned()
}

/// The body of a new concept note: its AI-maintained `## Summary` (§6.6), with the summary
/// as one paragraph when there is one.
pub fn concept_body(summary: Option<&str>) -> String {
    match summary {
        Some(s) if !s.trim().is_empty() => format!("## Summary\n{}\n", one_paragraph(s)),
        _ => "## Summary\n".to_owned(),
    }
}

/// The file stem of the note at `path` (`people/Ahmed 2.md` → `Ahmed 2`).
pub fn path_stem(path: &str) -> &str {
    let name = file_name(path);
    name.strip_suffix(".md").unwrap_or(name)
}

/// The unstamped skeleton of a new note of `kind` named `name` stored under the file stem
/// `stem` (the stem of the path it is written at, [`path_stem`]): `body`, `kind`, `title`
/// (only when `stem` differs from the trimmed name — the title rule) and `aliases` (as given;
/// omitted when empty).
pub fn skeleton(
    kind: NoteKind,
    name: &str,
    stem: &str,
    aliases: Vec<String>,
    body: &str,
) -> Result<Document, RenderError> {
    let mut doc = Document::parse(body);
    let name = name.trim();
    let fm = doc.frontmatter_mut();
    fm.set_kind(kind).map_err(RenderError::property("kind"))?;
    if stem != name {
        fm.set_text(KnownKey::Title, name)
            .map_err(RenderError::property("title"))?;
    }
    set_list_if_any(&mut doc, KnownKey::Aliases, aliases)?;
    Ok(doc)
}

/// A new entity, document or place note created by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySpec {
    /// Kind.
    pub kind: NoteKind,
    /// Name (the file name, sanitised; trimmed).
    pub name: String,
    /// Aliases (cleaned when written).
    pub aliases: Vec<String>,
    /// Tags (cleaned when written).
    pub tags: Vec<String>,
    /// User fields (`role`, `doc-type`, `address`, …) as scalar text.
    pub fields: BTreeMap<String, String>,
    /// Link text (`Nasr City office`, without brackets) of the enclosing place.
    pub part_of: Option<String>,
    /// User relations to existing notes: type and link text (without brackets), in order.
    /// A repeated target is written once.
    pub relations: Vec<(RelationKey, String)>,
}

impl EntitySpec {
    /// A note of `kind` named `name` with nothing else.
    pub fn new(kind: NoteKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            aliases: Vec::new(),
            tags: Vec::new(),
            fields: BTreeMap::new(),
            part_of: None,
            relations: Vec::new(),
        }
    }

    /// `entity.create` (person, company or concept); no relations.
    pub fn from_entity_create(op: &EntityCreate) -> Self {
        Self {
            aliases: op.aliases.clone(),
            fields: op.fields.clone(),
            ..Self::new(op.kind, op.name.clone())
        }
    }

    /// `document.create`; `relations` are the link texts of [`document_relations`], in that
    /// order.
    pub fn from_document_create(
        op: &DocumentCreate,
        relations: Vec<(RelationKey, String)>,
    ) -> Self {
        Self {
            aliases: op.aliases.clone(),
            fields: document_fields(op),
            relations,
            ..Self::new(NoteKind::Document, op.name.clone())
        }
    }

    /// `place.create`; `part_of` is the link text of the enclosing place (`parent_id`).
    pub fn from_place_create(op: &PlaceCreate, part_of: Option<String>) -> Self {
        Self {
            aliases: op.aliases.clone(),
            fields: place_fields(op),
            part_of,
            ..Self::new(NoteKind::Place, op.name.clone())
        }
    }

    /// The file stem of the note when its name is free (see [`Self::path`]).
    pub fn stem(&self) -> String {
        entity_stem(&self.name)
    }

    /// The path of the note given the vault paths already `taken`
    /// ([`crate::paths::entity_path`]).
    pub fn path<'a>(&self, taken: impl IntoIterator<Item = &'a str>) -> String {
        entity_path(self.kind, &self.name, taken)
    }

    /// The markdown of the note with `id` written at `path` ([`Self::path`]; its stem decides
    /// `title`), created at `created` (the device's creation time): `created` and `updated`
    /// are both that time, in UTC.
    pub fn render(
        &self,
        id: Ulid,
        path: &str,
        created: &DateTime<Utc>,
    ) -> Result<String, RenderError> {
        let mut doc = skeleton(
            self.kind,
            &self.name,
            path_stem(path),
            clean_list(&self.aliases),
            ENTITY_BODY,
        )?;
        set_list_if_any(&mut doc, KnownKey::Tags, clean_list(&self.tags))?;
        let fm = doc.frontmatter_mut();
        for (k, v) in &self.fields {
            fm.set(k, PropertyValue::Text(v.clone()))
                .map_err(RenderError::property(k))?;
        }
        if let Some(parent) = &self.part_of {
            let key = RelationKey::Note(RelationType::PartOf);
            fm.set_relation(key, vec![format!("[[{parent}]]")])
                .map_err(RenderError::property(key.as_str()))?;
        }
        stamp(&mut doc, id, Some(created), Some(created))?;
        let fm = doc.frontmatter_mut();
        for (rel, target) in &self.relations {
            fm.add_relation_link(*rel, target)
                .map_err(RenderError::property(rel.as_str()))?;
        }
        Ok(doc.render())
    }
}

/// The user fields a `document.create` sets: `doc-type`, `copy`, `expires` (`YYYY-MM-DD`).
pub fn document_fields(op: &DocumentCreate) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    if let Some(t) = &op.doc_type {
        fields.insert(KnownKey::DocType.as_str().to_owned(), t.clone());
    }
    if let Some(c) = op.copy {
        fields.insert(KnownKey::Copy.as_str().to_owned(), c.as_str().to_owned());
    }
    if let Some(e) = op.expires {
        fields.insert(
            KnownKey::Expires.as_str().to_owned(),
            e.format("%Y-%m-%d").to_string(),
        );
    }
    fields
}

/// The user relations a `document.create` adds, in order: `copy-of`, then `companies`, then
/// `people`.
pub fn document_relations(op: &DocumentCreate) -> Vec<(RelationKey, Ulid)> {
    let copy_of = RelationKey::Document(domain::DocumentRelationType::CopyOf);
    let companies = RelationKey::Mention(domain::MentionType::Companies);
    let people = RelationKey::Mention(domain::MentionType::People);
    op.copy_of
        .map(|c| (copy_of, c))
        .into_iter()
        .chain(op.companies.iter().map(|c| (companies, *c)))
        .chain(op.people.iter().map(|p| (people, *p)))
        .collect()
}

/// The user fields a `place.create` sets: `address`.
pub fn place_fields(op: &PlaceCreate) -> BTreeMap<String, String> {
    op.address
        .iter()
        .map(|a| (KnownKey::Address.as_str().to_owned(), a.clone()))
        .collect()
}
