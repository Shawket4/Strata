//! Documents and places (PLAN §6.12, §7.5 Documents & places): list with filters (`place`
//! includes nested places, via a recursive CTE), the document page with its cited custody
//! history, create (duplicate check), patch of user fields, manual custody events, and the
//! place page with its nested places and every document inside them.

use std::collections::BTreeMap;

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, Responder, web};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_index::repo::entities::{CustodyEvent as CRow, Document as DRow};
use strata_index::repo::vault::DocumentFilter;
use strata_index::types::{By, CustodyType, DocCopy, DocStatus};
use strata_vault::VaultService;
use strata_vault::ops::entities::NewCustodyEvent;
use ulid::Ulid;
use utoipa::ToSchema;
use vault_format::custody::CustodyEventType;

use crate::auth::Authenticated;
use crate::routes::entities::{
    CreateEntityRequest, EntityKind, PatchEntityRequest, entity_patch, new_entity,
};
use crate::routes::notes::Note;
use crate::vault::OrProblem;
use crate::wire::{MsgPack, Problem};

/// Copy kind of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum CopyKind {
    /// The original.
    #[serde(rename = "original")]
    Original,
    /// A certified copy.
    #[serde(rename = "certified copy")]
    CertifiedCopy,
    /// A plain copy.
    #[serde(rename = "copy")]
    Copy,
    /// A digital document.
    #[serde(rename = "digital")]
    Digital,
}

impl CopyKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::CertifiedCopy => "certified copy",
            Self::Copy => "copy",
            Self::Digital => "digital",
        }
    }
}

impl From<DocCopy> for CopyKind {
    fn from(c: DocCopy) -> Self {
        match c {
            DocCopy::Original => Self::Original,
            DocCopy::CertifiedCopy => Self::CertifiedCopy,
            DocCopy::Copy => Self::Copy,
            DocCopy::Digital => Self::Digital,
        }
    }
}

/// Where a document is, per its newest custody event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentStatus {
    /// In a place.
    Stored,
    /// With a person.
    CheckedOut,
    /// With a third party.
    WithThirdParty,
    /// Lost.
    Lost,
    /// Destroyed.
    Destroyed,
}

impl From<DocStatus> for DocumentStatus {
    fn from(s: DocStatus) -> Self {
        match s {
            DocStatus::Stored => Self::Stored,
            DocStatus::CheckedOut => Self::CheckedOut,
            DocStatus::WithThirdParty => Self::WithThirdParty,
            DocStatus::Lost => Self::Lost,
            DocStatus::Destroyed => Self::Destroyed,
        }
    }
}

impl From<DocumentStatus> for DocStatus {
    fn from(s: DocumentStatus) -> Self {
        match s {
            DocumentStatus::Stored => Self::Stored,
            DocumentStatus::CheckedOut => Self::CheckedOut,
            DocumentStatus::WithThirdParty => Self::WithThirdParty,
            DocumentStatus::Lost => Self::Lost,
            DocumentStatus::Destroyed => Self::Destroyed,
        }
    }
}

/// Custody event type (§6.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CustodyEventKind {
    /// Stored at a place.
    StoredAt,
    /// Moved to a place.
    MovedTo,
    /// Handed to a person.
    HandedTo,
    /// Returned by a person.
    ReturnedBy,
    /// Sent to a third party.
    SentTo,
    /// Received from a third party.
    ReceivedFrom,
    /// Lost.
    Lost,
    /// Found.
    Found,
    /// Destroyed.
    Destroyed,
}

impl From<CustodyEventKind> for CustodyEventType {
    fn from(k: CustodyEventKind) -> Self {
        match k {
            CustodyEventKind::StoredAt => Self::StoredAt,
            CustodyEventKind::MovedTo => Self::MovedTo,
            CustodyEventKind::HandedTo => Self::HandedTo,
            CustodyEventKind::ReturnedBy => Self::ReturnedBy,
            CustodyEventKind::SentTo => Self::SentTo,
            CustodyEventKind::ReceivedFrom => Self::ReceivedFrom,
            CustodyEventKind::Lost => Self::Lost,
            CustodyEventKind::Found => Self::Found,
            CustodyEventKind::Destroyed => Self::Destroyed,
        }
    }
}

impl From<CustodyType> for CustodyEventKind {
    fn from(k: CustodyType) -> Self {
        match k {
            CustodyType::StoredAt => Self::StoredAt,
            CustodyType::MovedTo => Self::MovedTo,
            CustodyType::HandedTo => Self::HandedTo,
            CustodyType::ReturnedBy => Self::ReturnedBy,
            CustodyType::SentTo => Self::SentTo,
            CustodyType::ReceivedFrom => Self::ReceivedFrom,
            CustodyType::Lost => Self::Lost,
            CustodyType::Found => Self::Found,
            CustodyType::Destroyed => Self::Destroyed,
        }
    }
}

fn ulid_of(id: Option<NoteId>) -> Option<Ulid> {
    id.map(|i| i.as_ulid())
}

/// A document in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocumentSummary {
    /// Document (note) ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Note path.
    pub path: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// `doc-type` (contract, id, licence, deed, invoice, certificate, other, or free text).
    pub doc_type: String,
    /// Copy kind.
    pub copy: CopyKind,
    /// The document this is a copy of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub copy_of: Option<Ulid>,
    /// Current place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub location_id: Option<Ulid>,
    /// Current holder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub holder_id: Option<Ulid>,
    /// Last holder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub last_holder_id: Option<Ulid>,
    /// Status.
    pub status: DocumentStatus,
    /// Expiry date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<NaiveDate>,
}

fn summary(d: &DRow, name: String, path: String, aliases: Vec<String>) -> DocumentSummary {
    DocumentSummary {
        id: d.note_id.as_ulid(),
        name,
        path,
        aliases,
        doc_type: d.doc_type.clone(),
        copy: d.copy.into(),
        copy_of: ulid_of(d.copy_of),
        location_id: ulid_of(d.location_id),
        holder_id: ulid_of(d.holder_id),
        last_holder_id: ulid_of(d.last_holder_id),
        status: d.status.into(),
        expires: d.expires,
    }
}

/// Documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocumentList {
    /// By name.
    pub items: Vec<DocumentSummary>,
}

/// `GET /documents` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DocumentsQuery {
    /// Name or alias, any script.
    #[serde(default)]
    pub q: Option<String>,
    /// Only documents in this place or any place nested inside it.
    #[serde(default)]
    #[param(value_type = Option<String>, format = "ulid")]
    pub place: Option<Ulid>,
    /// Only documents this person holds now.
    #[serde(default)]
    #[param(value_type = Option<String>, format = "ulid")]
    pub holder: Option<Ulid>,
    /// Only this status.
    #[serde(default)]
    pub status: Option<DocumentStatus>,
    /// Only documents expiring strictly before this date.
    #[serde(default)]
    pub expiring_before: Option<NaiveDate>,
}

/// A custody event (newest first in the history).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CustodyEvent {
    /// Event ID (stable across rebuilds).
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Type.
    #[serde(rename = "type")]
    pub event: CustodyEventKind,
    /// Date (midnight UTC of the resolved date).
    pub at: DateTime<Utc>,
    /// Place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub place_id: Option<Ulid>,
    /// Person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub person_id: Option<Ulid>,
    /// Third party.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub counterparty_id: Option<Ulid>,
    /// `user` / `ai`.
    pub by: String,
    /// The cited note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub source_note_id: Option<Ulid>,
    /// The cited block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_block_id: Option<String>,
}

impl From<CRow> for CustodyEvent {
    fn from(e: CRow) -> Self {
        Self {
            id: e.id.as_ulid(),
            event: e.event_type.into(),
            at: e.at,
            place_id: ulid_of(e.place_id),
            person_id: ulid_of(e.person_id),
            counterparty_id: ulid_of(e.counterparty_id),
            by: match e.by {
                By::User => "user".to_owned(),
                By::Ai => "ai".to_owned(),
            },
            source_note_id: ulid_of(e.source_note_id),
            source_block_id: e.source_block_id,
        }
    }
}

/// The document page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Document {
    /// Properties as in lists.
    pub document: DocumentSummary,
    /// Custody history, newest first.
    pub custody: Vec<CustodyEvent>,
    /// Place breadcrumb, outermost first (the location last).
    #[schema(value_type = Vec<String>)]
    pub location_path: Vec<Ulid>,
    /// Copies of this document.
    #[schema(value_type = Vec<String>)]
    pub copies: Vec<Ulid>,
    /// The note.
    pub note: Note,
}

/// `POST /documents`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateDocumentRequest {
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// `doc-type`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    /// Copy kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopyKind>,
    /// Expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<NaiveDate>,
    /// Client-generated ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub id: Option<Ulid>,
    /// When the item was created on the device (UTC; required, never the time the server
    /// receives it). More than `max_future_skew_secs` ahead of the server's clock is
    /// `422 created_in_future`.
    pub created: DateTime<Utc>,
    /// Create even if it looks like a duplicate.
    #[serde(default)]
    pub force: bool,
}

/// `PATCH /documents/{id}` (location, holder and status change only through custody
/// events).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct PatchDocumentRequest {
    /// New name (rename; links rewritten).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Replaces the aliases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    /// Replaces the tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// `doc-type`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    /// Copy kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<CopyKind>,
    /// Expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<NaiveDate>,
    /// Remove the expiry.
    #[serde(default)]
    pub clear_expires: bool,
    /// Save new aliases even if they match other documents.
    #[serde(default)]
    pub force: bool,
}

/// `POST /documents/{id}/custody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CustodyEventRequest {
    /// Type.
    #[serde(rename = "type")]
    pub event: CustodyEventKind,
    /// Date of the event.
    pub at: NaiveDate,
    /// Place (required for `stored-at`, `moved-to`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub place_id: Option<Ulid>,
    /// Person (required for `handed-to`, `returned-by`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub person_id: Option<Ulid>,
    /// Third party, person or company (required for `sent-to`, `received-from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub counterparty_id: Option<Ulid>,
    /// The note stating the event (cited on the line); when absent the line has no citation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub source_note_id: Option<Ulid>,
}

/// A place in a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PlaceSummary {
    /// Place (note) ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Name.
    pub name: String,
    /// Note path.
    pub path: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Enclosing place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
}

/// Places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PlaceList {
    /// By name.
    pub items: Vec<PlaceSummary>,
}

/// `GET /places` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PlacesQuery {
    /// Name or alias, any script.
    #[serde(default)]
    pub q: Option<String>,
}

/// The place page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Place {
    /// The place.
    pub place: PlaceSummary,
    /// Enclosing places, nearest first.
    #[schema(value_type = Vec<String>)]
    pub ancestors: Vec<Ulid>,
    /// Places directly inside.
    #[schema(value_type = Vec<String>)]
    pub children: Vec<Ulid>,
    /// Documents here or in any place nested inside.
    #[schema(value_type = Vec<String>)]
    pub documents: Vec<Ulid>,
    /// The note.
    pub note: Note,
}

/// `POST /places`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreatePlaceRequest {
    /// Name.
    pub name: String,
    /// Aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Enclosing place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
    /// Address (user-entered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Client-generated ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub id: Option<Ulid>,
    /// When the item was created on the device (UTC; required, never the time the server
    /// receives it). More than `max_future_skew_secs` ahead of the server's clock is
    /// `422 created_in_future`.
    pub created: DateTime<Utc>,
    /// Create even if it looks like a duplicate.
    #[serde(default)]
    pub force: bool,
}

/// `PATCH /places/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub struct PatchPlaceRequest {
    /// New name (rename; links rewritten).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Replaces the aliases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    /// Replaces the tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// New enclosing place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub parent_id: Option<Ulid>,
    /// Remove the enclosing place.
    #[serde(default)]
    pub clear_parent: bool,
    /// Address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Remove the address.
    #[serde(default)]
    pub clear_address: bool,
    /// Save new aliases even if they match other places.
    #[serde(default)]
    pub force: bool,
}

async fn document_page(
    vault: &VaultService,
    auth: &Authenticated,
    id: NoteId,
) -> Result<Document, Problem> {
    let d = vault.document(auth.scope(), id).await.or_problem()?;
    let note: Note = d.note.into();
    let aliases = note
        .properties
        .iter()
        .find(|p| p.key == "aliases")
        .map(|p| match &p.value {
            crate::routes::notes::PropertyValueDto::List { items } => items.clone(),
            crate::routes::notes::PropertyValueDto::Text { value } => vec![value.clone()],
            _ => Vec::new(),
        })
        .unwrap_or_default();
    Ok(Document {
        document: summary(&d.document, note.title.clone(), note.path.clone(), aliases),
        custody: d.custody.into_iter().map(CustodyEvent::from).collect(),
        location_path: d.location_path.into_iter().map(|i| i.as_ulid()).collect(),
        copies: d.copies.into_iter().map(|i| i.as_ulid()).collect(),
        note,
    })
}

async fn place_page(
    vault: &VaultService,
    auth: &Authenticated,
    id: NoteId,
) -> Result<Place, Problem> {
    let (note, tree) = vault.place(auth.scope(), id).await.or_problem()?;
    let note: Note = note.into();
    let aliases = note
        .properties
        .iter()
        .find(|p| p.key == "aliases")
        .map(|p| match &p.value {
            crate::routes::notes::PropertyValueDto::List { items } => items.clone(),
            crate::routes::notes::PropertyValueDto::Text { value } => vec![value.clone()],
            _ => Vec::new(),
        })
        .unwrap_or_default();
    Ok(Place {
        place: PlaceSummary {
            id: note.id,
            name: note.title.clone(),
            path: note.path.clone(),
            aliases,
            parent_id: tree.ancestors.first().map(strata_common::NoteId::as_ulid),
        },
        ancestors: tree.ancestors.into_iter().map(|i| i.as_ulid()).collect(),
        children: tree.children.into_iter().map(|i| i.as_ulid()).collect(),
        documents: tree.documents.into_iter().map(|i| i.as_ulid()).collect(),
        note,
    })
}

/// Documents, filtered (`place` includes nested places).
#[utoipa::path(
    get, path = "/documents", tag = "entities", operation_id = "list_documents",
    params(DocumentsQuery),
    responses((status = 200, description = "Documents.", body = DocumentList)),
)]
pub async fn list_documents(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<DocumentsQuery>,
) -> Result<MsgPack<DocumentList>, Problem> {
    let q = q.into_inner();
    let filter = DocumentFilter {
        place: q.place.map(NoteId::from_ulid),
        holder: q.holder.map(NoteId::from_ulid),
        status: q.status.map(Into::into),
        expiring_before: q.expiring_before,
        query: q.q,
    };
    let docs = vault
        .list_documents(auth.scope(), filter)
        .await
        .or_problem()?;
    Ok(MsgPack(DocumentList {
        items: docs
            .into_iter()
            .map(|(d, s)| summary(&d, s.entity.display_name, s.path, s.aliases))
            .collect(),
    }))
}

/// Create a document (duplicate check unless `force`).
#[utoipa::path(
    post, path = "/documents", tag = "entities", operation_id = "create_document",
    request_body = CreateDocumentRequest,
    responses(
        (status = 201, description = "Created.", body = Document),
        (status = 409, description = "`duplicate_candidates`.", body = Problem),
    ),
)]
pub async fn create_document(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CreateDocumentRequest>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let mut fields = BTreeMap::new();
    if let Some(t) = b.doc_type {
        fields.insert("doc-type".to_owned(), t);
    }
    if let Some(c) = b.copy {
        fields.insert("copy".to_owned(), c.as_str().to_owned());
    }
    if let Some(e) = b.expires {
        fields.insert("expires".to_owned(), e.format("%Y-%m-%d").to_string());
    }
    let req = CreateEntityRequest {
        kind: EntityKind::Document,
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        fields,
        parent_id: None,
        id: b.id,
        created: b.created,
        force: b.force,
    };
    let note = vault
        .create_entity(auth.scope(), new_entity(EntityKind::Document, req))
        .await
        .or_problem()?;
    let page = document_page(&vault, &auth, note.id).await?;
    Ok(MsgPack(page).customize().with_status(StatusCode::CREATED))
}

/// A document with its custody history, location breadcrumb and copies.
#[utoipa::path(
    get, path = "/documents/{id}", tag = "entities", operation_id = "get_document",
    params(("id" = Ulid, Path, description = "Document ID (ULID).")),
    responses((status = 200, description = "The document.", body = Document)),
)]
pub async fn get_document(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Document>, Problem> {
    Ok(MsgPack(
        document_page(&vault, &auth, NoteId::from_ulid(*id)).await?,
    ))
}

/// Edit a document's user fields (not location/holder/status: those follow custody events).
#[utoipa::path(
    patch, path = "/documents/{id}", tag = "entities", operation_id = "patch_document",
    params(
        ("id" = Ulid, Path, description = "Document ID (ULID)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the note's current version."),
    ),
    request_body = PatchDocumentRequest,
    responses(
        (status = 200, description = "Updated.", body = Document),
        (status = 409, description = "`duplicate_candidates`, `version_conflict` or `path_taken`.", body = Problem),
    ),
)]
pub async fn patch_document(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<Ulid>,
    body: MsgPack<PatchDocumentRequest>,
) -> Result<MsgPack<Document>, Problem> {
    let if_match = crate::vault::if_match(req.headers())?;
    let b = body.into_inner();
    let mut set = BTreeMap::new();
    let mut unset = Vec::new();
    if let Some(t) = b.doc_type {
        set.insert("doc-type".to_owned(), t);
    }
    if let Some(c) = b.copy {
        set.insert("copy".to_owned(), c.as_str().to_owned());
    }
    if let Some(e) = b.expires {
        set.insert("expires".to_owned(), e.format("%Y-%m-%d").to_string());
    }
    if b.clear_expires {
        unset.push("expires".to_owned());
    }
    let patch = PatchEntityRequest {
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        set_fields: set,
        unset_fields: unset,
        parent_id: None,
        clear_parent: false,
        force: b.force,
    };
    let id = NoteId::from_ulid(*id);
    ensure_kind(&vault, &auth, id, domain::NoteKind::Document).await?;
    vault
        .patch_entity(auth.scope(), id, entity_patch(patch, if_match))
        .await
        .or_problem()?;
    Ok(MsgPack(document_page(&vault, &auth, id).await?))
}

async fn ensure_kind(
    vault: &VaultService,
    auth: &Authenticated,
    id: NoteId,
    kind: domain::NoteKind,
) -> Result<(), Problem> {
    let note = vault.note(auth.scope(), id).await.or_problem()?;
    if note.kind != kind || note.trashed {
        return Err(Problem::new(crate::wire::ProblemType::NotFound));
    }
    Ok(())
}

/// Record a manual custody event (`by: user`); the frontmatter follows the newest event.
#[utoipa::path(
    post, path = "/documents/{id}/custody", tag = "entities", operation_id = "add_custody_event",
    params(("id" = Ulid, Path, description = "Document ID (ULID).")),
    request_body = CustodyEventRequest,
    responses((status = 200, description = "Recorded in one `user: custody <path>` commit.", body = Document)),
)]
pub async fn add_custody_event(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<CustodyEventRequest>,
) -> Result<MsgPack<Document>, Problem> {
    let b = body.into_inner();
    let id = NoteId::from_ulid(*id);
    vault
        .add_custody_event(
            auth.scope(),
            id,
            NewCustodyEvent {
                kind: b.event.into(),
                date: b.at,
                place: b.place_id.map(NoteId::from_ulid),
                person: b.person_id.map(NoteId::from_ulid),
                counterparty: b.counterparty_id.map(NoteId::from_ulid),
                source: b.source_note_id.map(NoteId::from_ulid),
            },
        )
        .await
        .or_problem()?;
    Ok(MsgPack(document_page(&vault, &auth, id).await?))
}

/// Places.
#[utoipa::path(
    get, path = "/places", tag = "entities", operation_id = "list_places",
    params(PlacesQuery),
    responses((status = 200, description = "Places.", body = PlaceList)),
)]
pub async fn list_places(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<PlacesQuery>,
) -> Result<MsgPack<PlaceList>, Problem> {
    let items = vault
        .places(auth.scope(), q.q.as_deref())
        .await
        .or_problem()?;
    Ok(MsgPack(PlaceList {
        items: items
            .into_iter()
            .map(|(s, parent)| PlaceSummary {
                id: s.entity.note_id.as_ulid(),
                name: s.entity.display_name,
                path: s.path,
                aliases: s.aliases,
                parent_id: parent.map(|p| p.as_ulid()),
            })
            .collect(),
    }))
}

/// Create a place (optionally inside another).
#[utoipa::path(
    post, path = "/places", tag = "entities", operation_id = "create_place",
    request_body = CreatePlaceRequest,
    responses(
        (status = 201, description = "Created.", body = Place),
        (status = 409, description = "`duplicate_candidates`.", body = Problem),
    ),
)]
pub async fn create_place(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CreatePlaceRequest>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let mut fields = BTreeMap::new();
    if let Some(a) = b.address {
        fields.insert("address".to_owned(), a);
    }
    let req = CreateEntityRequest {
        kind: EntityKind::Place,
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        fields,
        parent_id: b.parent_id,
        id: b.id,
        created: b.created,
        force: b.force,
    };
    let note = vault
        .create_entity(auth.scope(), new_entity(EntityKind::Place, req))
        .await
        .or_problem()?;
    let page = place_page(&vault, &auth, note.id).await?;
    Ok(MsgPack(page).customize().with_status(StatusCode::CREATED))
}

/// A place with its nesting and every document inside it (recursively).
#[utoipa::path(
    get, path = "/places/{id}", tag = "entities", operation_id = "get_place",
    params(("id" = Ulid, Path, description = "Place ID (ULID).")),
    responses((status = 200, description = "The place.", body = Place)),
)]
pub async fn get_place(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Place>, Problem> {
    Ok(MsgPack(
        place_page(&vault, &auth, NoteId::from_ulid(*id)).await?,
    ))
}

/// Edit a place (name, aliases, tags, address, enclosing place).
#[utoipa::path(
    patch, path = "/places/{id}", tag = "entities", operation_id = "patch_place",
    params(
        ("id" = Ulid, Path, description = "Place ID (ULID)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the note's current version."),
    ),
    request_body = PatchPlaceRequest,
    responses(
        (status = 200, description = "Updated.", body = Place),
        (status = 409, description = "`duplicate_candidates`, `version_conflict` or `path_taken`.", body = Problem),
    ),
)]
pub async fn patch_place(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<Ulid>,
    body: MsgPack<PatchPlaceRequest>,
) -> Result<MsgPack<Place>, Problem> {
    let if_match = crate::vault::if_match(req.headers())?;
    let b = body.into_inner();
    let mut set = BTreeMap::new();
    let mut unset = Vec::new();
    if let Some(a) = b.address {
        set.insert("address".to_owned(), a);
    }
    if b.clear_address {
        unset.push("address".to_owned());
    }
    let patch = PatchEntityRequest {
        name: b.name,
        aliases: b.aliases,
        tags: b.tags,
        set_fields: set,
        unset_fields: unset,
        parent_id: b.parent_id,
        clear_parent: b.clear_parent,
        force: b.force,
    };
    let id = NoteId::from_ulid(*id);
    ensure_kind(&vault, &auth, id, domain::NoteKind::Place).await?;
    vault
        .patch_entity(auth.scope(), id, entity_patch(patch, if_match))
        .await
        .or_problem()?;
    Ok(MsgPack(place_page(&vault, &auth, id).await?))
}

/// Mounts the document and place routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/documents")
            .route(web::get().to(list_documents))
            .route(web::post().to(create_document)),
    )
    .service(
        web::resource("/documents/{id}")
            .route(web::get().to(get_document))
            .route(web::patch().to(patch_document)),
    )
    .route("/documents/{id}/custody", web::post().to(add_custody_event))
    .service(
        web::resource("/places")
            .route(web::get().to(list_places))
            .route(web::post().to(create_place)),
    )
    .service(
        web::resource("/places/{id}")
            .route(web::get().to(get_place))
            .route(web::patch().to(patch_place)),
    );
}
