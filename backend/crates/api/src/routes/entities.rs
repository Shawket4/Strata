//! People, companies, documents and places as entities (PLAN §6.7, §7.5 People & companies):
//! list/search (aliases in both scripts), create (duplicate check), the aggregated page,
//! patch of user fields and aliases, merge, mentioning notes, related documents.

use std::collections::BTreeMap;

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, Responder, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_index::types::EntityKind as IKind;
use strata_vault::VaultService;
use strata_vault::ops::entities::{
    EntityDocuments as VDocs, EntityPatch, EntitySummary as VSummary, EntityView as VView,
    MentioningNote as VMention, NewEntity,
};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::Authenticated;
use crate::routes::notes::{Note, Property};
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem};

/// Kind of an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A person (`people/`).
    Person,
    /// A company or organisation (`companies/`).
    Company,
    /// A document (`documents/`).
    Document,
    /// A place (`places/`).
    Place,
}

impl From<IKind> for EntityKind {
    fn from(k: IKind) -> Self {
        match k {
            IKind::Person => Self::Person,
            IKind::Company => Self::Company,
            IKind::Document => Self::Document,
            IKind::Place => Self::Place,
        }
    }
}

impl From<EntityKind> for IKind {
    fn from(k: EntityKind) -> Self {
        match k {
            EntityKind::Person => Self::Person,
            EntityKind::Company => Self::Company,
            EntityKind::Document => Self::Document,
            EntityKind::Place => Self::Place,
        }
    }
}

impl From<EntityKind> for domain::NoteKind {
    fn from(k: EntityKind) -> Self {
        match k {
            EntityKind::Person => Self::Person,
            EntityKind::Company => Self::Company,
            EntityKind::Document => Self::Document,
            EntityKind::Place => Self::Place,
        }
    }
}

/// One entity in a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct EntitySummary {
    /// Entity (note) ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Kind.
    pub kind: EntityKind,
    /// Display name.
    pub name: String,
    /// Note path.
    pub path: String,
    /// Aliases (both scripts).
    pub aliases: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Person role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Company industry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub industry: Option<String>,
    /// Match score for `q` (0 without a query).
    pub score: f64,
}

impl From<VSummary> for EntitySummary {
    fn from(s: VSummary) -> Self {
        Self {
            id: s.entity.note_id.as_ulid(),
            kind: s.entity.kind.into(),
            name: s.entity.display_name,
            path: s.path,
            aliases: s.aliases,
            tags: s.tags,
            role: s.entity.role,
            industry: s.entity.industry,
            score: f64::from(s.score),
        }
    }
}

/// Entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct EntityList {
    /// Best match first (by name without a query).
    pub items: Vec<EntitySummary>,
}

/// `GET /entities` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EntitiesQuery {
    /// Only this kind.
    #[serde(default)]
    pub kind: Option<EntityKind>,
    /// Name or alias, any script (normalised, substring or trigram match).
    #[serde(default)]
    pub q: Option<String>,
    /// Only entities with this tag.
    #[serde(default)]
    pub tag: Option<String>,
    /// Maximum results (1–500, default 100).
    #[serde(default)]
    #[param(minimum = 1, maximum = 500)]
    pub limit: Option<u32>,
}

/// `POST /entities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateEntityRequest {
    /// Kind.
    pub kind: EntityKind,
    /// Name (becomes the file name, sanitised; the exact name is kept as `title` if needed).
    pub name: String,
    /// Aliases (both scripts).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// User fields: person `role`, `phone`, `email`; company `industry`, `website`;
    /// document `doc-type`, `copy`, `expires`; place `address`.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Places: the enclosing place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
    /// Client-generated ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub id: Option<Ulid>,
    /// When the item was created on the device (UTC; required, never the time the server
    /// receives it). More than `max_future_skew_secs` ahead of the server's clock is
    /// `422 created_in_future`.
    pub created: DateTime<Utc>,
    /// Create even if it looks like a duplicate (records keep-both).
    #[serde(default)]
    pub force: bool,
}

/// `PATCH /entities/{id}`; absent fields are unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct PatchEntityRequest {
    /// New name (renames the note; every link is rewritten in the same commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Replaces the aliases (new aliases run the duplicate check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    /// Replaces the tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// User fields to set.
    #[serde(default)]
    pub set_fields: BTreeMap<String, String>,
    /// User fields to remove.
    #[serde(default)]
    pub unset_fields: Vec<String>,
    /// Places: the new enclosing place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
    /// Places: remove the enclosing place.
    #[serde(default)]
    pub clear_parent: bool,
    /// Save new aliases even if they match other entities (records keep-both).
    #[serde(default)]
    pub force: bool,
}

/// `POST /entities/{id}/merge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MergeRequest {
    /// The surviving entity (same kind).
    #[schema(value_type = String, format = "ulid")]
    pub into_id: Ulid,
}

/// A section of an entity note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EntitySection {
    /// Heading (`Summary`, `Insights`, `Open items`, `Timeline`, `Custody`, `Notes`, …).
    pub title: String,
    /// Its own content (trimmed).
    pub content: String,
}

/// A note mentioning an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MentioningNote {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Last update.
    pub updated: DateTime<Utc>,
    /// The first line linking to the entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

impl From<VMention> for MentioningNote {
    fn from(m: VMention) -> Self {
        Self {
            id: m.id.as_ulid(),
            path: m.path,
            title: m.title,
            updated: m.updated,
            snippet: m.snippet,
        }
    }
}

/// Notes mentioning an entity, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MentioningNotes {
    /// Notes.
    pub items: Vec<MentioningNote>,
}

/// Direction of an entity relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// This entity → the other.
    Out,
    /// The other → this entity.
    In,
}

/// A relation of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EntityRelation {
    /// Relation type.
    #[serde(rename = "type")]
    pub rel_type: String,
    /// The other note.
    #[schema(value_type = String, format = "ulid")]
    pub other_id: Ulid,
    /// Its title.
    pub other_title: String,
    /// Direction.
    pub direction: Direction,
    /// `user` / `ai`.
    pub by: String,
}

/// Documents related to an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EntityDocuments {
    /// Held now.
    #[schema(value_type = Vec<String>)]
    pub holds: Vec<Ulid>,
    /// Last handled.
    #[schema(value_type = Vec<String>)]
    pub last_handled: Vec<Ulid>,
    /// Concerning the entity (`companies:` / `people:`).
    #[schema(value_type = Vec<String>)]
    pub concerning: Vec<Ulid>,
}

impl From<VDocs> for EntityDocuments {
    fn from(d: VDocs) -> Self {
        let ids = |v: Vec<NoteId>| v.into_iter().map(|i| i.as_ulid()).collect();
        Self {
            holds: ids(d.holds),
            last_handled: ids(d.last_handled),
            concerning: ids(d.concerning),
        }
    }
}

/// The aggregated entity page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Entity {
    /// Entity (note) ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Kind.
    pub kind: EntityKind,
    /// Display name.
    pub name: String,
    /// Note path.
    pub path: String,
    /// Note version (for `If-Match`).
    pub version: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Frontmatter properties.
    pub properties: Vec<Property>,
    /// Level-2 sections in body order (AI sections and the user's `## Notes`).
    pub sections: Vec<EntitySection>,
    /// Mentioning notes, newest first (first 50).
    pub mentions: Vec<MentioningNote>,
    /// Total mentioning notes.
    pub mention_count: u32,
    /// Relations to and from other notes (not counting `people`/`companies`/`concepts`).
    pub relations: Vec<EntityRelation>,
    /// Related documents.
    pub documents: EntityDocuments,
}

impl From<VView> for Entity {
    fn from(v: VView) -> Self {
        let note: Note = v.note.into();
        Self {
            id: note.id,
            kind: v.entity.kind.into(),
            name: v.entity.display_name,
            path: note.path,
            version: note.version,
            aliases: v.aliases,
            tags: v.tags,
            properties: note.properties,
            sections: v
                .sections
                .into_iter()
                .map(|(title, content)| EntitySection { title, content })
                .collect(),
            mentions: v.mentions.into_iter().map(MentioningNote::from).collect(),
            mention_count: u32::try_from(v.mention_count).unwrap_or(u32::MAX),
            relations: v
                .relations
                .into_iter()
                .map(|r| EntityRelation {
                    rel_type: r.rel,
                    other_id: r.other.as_ulid(),
                    other_title: r.title,
                    direction: if r.outgoing {
                        Direction::Out
                    } else {
                        Direction::In
                    },
                    by: r.by,
                })
                .collect(),
            documents: v.documents.into(),
        }
    }
}

/// Mention page size of the entity page.
pub const MENTION_PAGE: usize = 50;

pub(crate) fn new_entity(kind: EntityKind, b: CreateEntityRequest) -> NewEntity {
    NewEntity {
        kind: kind.into(),
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        fields: b.fields,
        parent: b.parent_id.map(NoteId::from_ulid),
        id: b.id.map(NoteId::from_ulid),
        created: b.created,
        force: b.force,
    }
}

pub(crate) fn entity_patch(b: PatchEntityRequest, if_match: Option<String>) -> EntityPatch {
    let mut fields: BTreeMap<String, Option<String>> = b
        .set_fields
        .into_iter()
        .map(|(k, v)| (k, Some(v)))
        .collect();
    for k in b.unset_fields {
        fields.insert(k, None);
    }
    EntityPatch {
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        fields,
        parent: if b.clear_parent {
            Some(None)
        } else {
            b.parent_id.map(|p| Some(NoteId::from_ulid(p)))
        },
        if_match,
        force: b.force,
    }
}

/// Entities of any or one kind, matching `q` in any script.
#[utoipa::path(
    get, path = "/entities", tag = "entities", operation_id = "list_entities",
    params(EntitiesQuery),
    responses((status = 200, description = "Entities.", body = EntityList)),
)]
pub async fn list_entities(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<EntitiesQuery>,
) -> Result<MsgPack<EntityList>, Problem> {
    if q.tag.as_deref().is_some_and(|t| t.contains('\0')) {
        // No stored tag can contain NUL (PostgreSQL text cannot hold it).
        return Ok(MsgPack(EntityList { items: Vec::new() }));
    }
    let items = vault
        .list_entities(
            auth.scope(),
            q.kind.map(Into::into),
            q.q.as_deref(),
            q.tag.as_deref(),
            q.limit.unwrap_or(100),
        )
        .await
        .or_problem()?;
    Ok(MsgPack(EntityList {
        items: items.into_iter().map(EntitySummary::from).collect(),
    }))
}

/// Create a person, company, document or place (duplicate check unless `force`).
#[utoipa::path(
    post, path = "/entities", tag = "entities", operation_id = "create_entity",
    request_body = CreateEntityRequest,
    responses(
        (status = 201, description = "Created; one `user: create <path>` commit.", body = Entity),
        (status = 409, description = "`duplicate_candidates` (resend with `force`).", body = Problem),
    ),
)]
pub async fn create_entity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CreateEntityRequest>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let note = vault
        .create_entity(auth.scope(), new_entity(b.kind, b))
        .await
        .or_problem()?;
    let view = vault
        .entity(auth.scope(), note.id, MENTION_PAGE)
        .await
        .or_problem()?;
    Ok(MsgPack(Entity::from(view))
        .customize()
        .with_status(StatusCode::CREATED))
}

/// The aggregated entity page.
#[utoipa::path(
    get, path = "/entities/{id}", tag = "entities", operation_id = "get_entity",
    params(("id" = Ulid, Path, description = "Entity ID (ULID).")),
    responses((status = 200, description = "The entity.", body = Entity)),
)]
pub async fn get_entity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Entity>, Problem> {
    let view = vault
        .entity(auth.scope(), NoteId::from_ulid(*id), MENTION_PAGE)
        .await
        .or_problem()?;
    Ok(MsgPack(view.into()))
}

/// Edit user fields, aliases, tags or the name (contact fields are only ever user-written).
#[utoipa::path(
    patch, path = "/entities/{id}", tag = "entities", operation_id = "patch_entity",
    params(
        ("id" = Ulid, Path, description = "Entity ID (ULID)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the note's current version."),
    ),
    request_body = PatchEntityRequest,
    responses(
        (status = 200, description = "Updated in one commit.", body = Entity),
        (status = 409, description = "`duplicate_candidates` (new aliases), `version_conflict` or `path_taken`.", body = Problem),
    ),
)]
pub async fn patch_entity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<Ulid>,
    body: MsgPack<PatchEntityRequest>,
) -> Result<MsgPack<Entity>, Problem> {
    let if_match = crate::vault::if_match(req.headers())?;
    let id = NoteId::from_ulid(*id);
    vault
        .patch_entity(auth.scope(), id, entity_patch(body.into_inner(), if_match))
        .await
        .or_problem()?;
    let view = vault
        .entity(auth.scope(), id, MENTION_PAGE)
        .await
        .or_problem()?;
    Ok(MsgPack(view.into()))
}

/// Merge this entity into another of the same kind (links rewritten, aliases unioned, `##
/// Notes` moved under a dated sub-heading, this one trashed — one commit).
#[utoipa::path(
    post, path = "/entities/{id}/merge", tag = "entities", operation_id = "merge_entity",
    params(("id" = Ulid, Path, description = "The entity merged away (ULID).")),
    request_body = MergeRequest,
    responses((status = 200, description = "The survivor.", body = Entity)),
)]
pub async fn merge_entity(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<MergeRequest>,
) -> Result<MsgPack<Entity>, Problem> {
    let into = NoteId::from_ulid(body.into_inner().into_id);
    vault
        .merge_entities(auth.scope(), NoteId::from_ulid(*id), into)
        .await
        .or_problem()?;
    let view = vault
        .entity(auth.scope(), into, MENTION_PAGE)
        .await
        .or_problem()?;
    Ok(MsgPack(view.into()))
}

/// Notes mentioning the entity, newest first, with the mention line.
#[utoipa::path(
    get, path = "/entities/{id}/notes", tag = "entities", operation_id = "get_entity_notes",
    params(("id" = Ulid, Path, description = "Entity ID (ULID).")),
    responses((status = 200, description = "Mentioning notes.", body = MentioningNotes)),
)]
pub async fn entity_notes(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<MentioningNotes>, Problem> {
    let items = vault
        .entity_notes(auth.scope(), NoteId::from_ulid(*id))
        .await
        .or_problem()?;
    Ok(MsgPack(MentioningNotes {
        items: items.into_iter().map(MentioningNote::from).collect(),
    }))
}

/// Documents the entity holds now, last handled, or that concern it.
#[utoipa::path(
    get, path = "/entities/{id}/documents", tag = "entities", operation_id = "get_entity_documents",
    params(("id" = Ulid, Path, description = "Entity ID (ULID).")),
    responses((status = 200, description = "Related documents.", body = EntityDocuments)),
)]
pub async fn entity_documents(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<EntityDocuments>, Problem> {
    let d = vault
        .entity_documents(auth.scope(), NoteId::from_ulid(*id))
        .await
        .or_problem()?;
    Ok(MsgPack(d.into()))
}

/// Mounts the entity routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/entities")
            .route(web::get().to(list_entities))
            .route(web::post().to(create_entity)),
    )
    .service(
        web::resource("/entities/{id}")
            .route(web::get().to(get_entity))
            .route(web::patch().to(patch_entity)),
    )
    .route("/entities/{id}/merge", web::post().to(merge_entity))
    .route("/entities/{id}/notes", web::get().to(entity_notes))
    .route("/entities/{id}/documents", web::get().to(entity_documents));
}
