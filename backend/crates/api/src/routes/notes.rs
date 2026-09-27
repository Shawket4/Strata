//! Notes (PLAN §7.5 Notes): tree, CRUD, lookup by path, move/rename (inbound links are
//! rewritten in the same commit), soft delete, restore, purge, backlinks, history, file at a
//! revision, revert of a note and of a whole commit.

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strata_common::NoteId;
use strata_vault::VaultService;
use strata_vault::model::{self, NoteView, TreeEntry};
use strata_vault::ops::notes::CreateNote;
use ulid::Ulid;
use utoipa::ToSchema;
use vault_format::PropertyValue;

use crate::auth::Authenticated;
use crate::vault::{OrProblem, required_if_match};
use crate::wire::{MsgPack, MsgPackConfig, Problem};

/// Kind of a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// An ordinary note.
    Note,
    /// A concept note.
    Concept,
    /// A person.
    Person,
    /// A company.
    Company,
    /// A document.
    Document,
    /// A place.
    Place,
}

impl From<domain::NoteKind> for NoteKind {
    fn from(k: domain::NoteKind) -> Self {
        match k {
            domain::NoteKind::Note => Self::Note,
            domain::NoteKind::Concept => Self::Concept,
            domain::NoteKind::Person => Self::Person,
            domain::NoteKind::Company => Self::Company,
            domain::NoteKind::Document => Self::Document,
            domain::NoteKind::Place => Self::Place,
        }
    }
}

/// A frontmatter property value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PropertyValueDto {
    /// `key:` with no value.
    Null,
    /// A scalar (numbers and booleans keep their source spelling).
    Text {
        /// The text.
        value: String,
    },
    /// A flat list of scalars (relations are wikilink strings).
    List {
        /// The items.
        items: Vec<String>,
    },
    /// A nested value (read the raw content).
    Other,
}

/// One frontmatter property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Property {
    /// Key.
    pub key: String,
    /// Value.
    pub value: PropertyValueDto,
}

/// A note with its content (`version` is the value for `If-Match`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Note {
    /// Note ID.
    #[schema(value_type = String, format = "ulid")]
    pub id: Ulid,
    /// Vault path (`.trash/…` while deleted).
    pub path: String,
    /// Display title (frontmatter `title`, else the file name).
    pub title: String,
    /// Kind.
    pub kind: NoteKind,
    /// The whole file (frontmatter and body).
    pub content: String,
    /// Content hash (`sha256:<hex>`).
    pub version: String,
    /// Parsed frontmatter, in file order (empty when it is not readable).
    pub properties: Vec<Property>,
    /// Frontmatter `created`.
    pub created: DateTime<Utc>,
    /// Frontmatter `updated`.
    pub updated: DateTime<Utc>,
    /// In the trash.
    pub trashed: bool,
}

impl From<NoteView> for Note {
    fn from(v: NoteView) -> Self {
        Self {
            id: v.id.as_ulid(),
            path: v.path,
            title: v.title,
            kind: v.kind.into(),
            content: v.content,
            version: v.version,
            properties: v
                .properties
                .into_iter()
                .map(|p| Property {
                    key: p.key,
                    value: match p.value {
                        PropertyValue::Null => PropertyValueDto::Null,
                        PropertyValue::Text(value) => PropertyValueDto::Text { value },
                        PropertyValue::List(items) => PropertyValueDto::List { items },
                        PropertyValue::Other => PropertyValueDto::Other,
                    },
                })
                .collect(),
            created: v.created,
            updated: v.updated,
            trashed: v.trashed,
        }
    }
}

/// One entry of the vault tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TreeItem {
    /// A folder.
    Folder {
        /// Path.
        path: String,
    },
    /// A note.
    Note {
        /// Path.
        path: String,
        /// Note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Title.
        title: String,
        /// Kind.
        kind: NoteKind,
        /// Last update.
        updated: DateTime<Utc>,
    },
    /// Any other file (attachment, canvas).
    File {
        /// Path.
        path: String,
    },
}

/// The vault tree (hidden folders excluded), sorted by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Tree {
    /// Entries.
    pub entries: Vec<TreeItem>,
}

/// `POST /notes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateNoteRequest {
    /// Vault path ending in `.md` (Obsidian-safe names, no hidden folders).
    pub path: String,
    /// Full content; `id`, `created` and `updated` are set by the server.
    pub content: String,
    /// Client-generated ID (offline creates keep their IDs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "ulid")]
    pub id: Option<Ulid>,
    /// Create even if it looks like a duplicate (records keep-both).
    #[serde(default)]
    pub force: bool,
}

/// `PUT /notes/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct UpdateNoteRequest {
    /// Full new content (the `id` property cannot change).
    pub content: String,
}

/// `POST /notes/{id}/move`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MoveNoteRequest {
    /// New vault path ending in `.md`.
    pub new_path: String,
}

/// One backlink.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Backlink {
    /// Linking note.
    #[schema(value_type = String, format = "ulid")]
    pub source_id: Ulid,
    /// Its path.
    pub source_path: String,
    /// Its title.
    pub source_title: String,
    /// Relation provenance (`user` / `ai`), for relations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// AI confidence, for AI relations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Heading anchor of a body link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    /// Block anchor of a body link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
}

/// Backlinks of one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BacklinkGroup {
    /// `link`, `embed`, or a relation type (`related`, `part-of`, `people`, …).
    pub kind: String,
    /// Backlinks, by source path.
    pub items: Vec<Backlink>,
}

/// Body links and relations pointing at a note, grouped by kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Backlinks {
    /// Groups: `link`, `embed`, then relation types alphabetically.
    pub groups: Vec<BacklinkGroup>,
}

/// Who made a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitAuthor {
    /// `user: …`
    User,
    /// `ai: …`
    Ai,
    /// `system: …`
    System,
}

/// How a commit changed the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    /// Created.
    Added,
    /// Content changed.
    Modified,
    /// Moved/renamed.
    Renamed,
    /// Removed.
    Deleted,
}

/// A commit that touched the note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Revision {
    /// Commit ID (40 hex characters).
    pub commit: String,
    /// Commit message.
    pub message: String,
    /// Who made it.
    pub author: CommitAuthor,
    /// When.
    pub at: DateTime<Utc>,
    /// The note's path in that commit.
    pub path: String,
    /// How it changed the note.
    pub change: FileChange,
}

/// A note's history, newest first (follows moves and trash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct History {
    /// Revisions.
    pub revisions: Vec<Revision>,
}

/// A note as of a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NoteRevision {
    /// Commit ID.
    pub commit: String,
    /// Path in that commit.
    pub path: String,
    /// Content in that commit.
    pub content: String,
    /// Version (hash) of that content.
    pub version: String,
}

/// `POST /notes/{id}/revert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RevertNoteRequest {
    /// A commit from the note's history.
    pub commit: String,
}

/// Result of `POST /commits/{commit}/revert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CommitReverted {
    /// The new commit (absent if nothing had to change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Vault paths the revert changed, sorted.
    pub paths: Vec<String>,
}

/// `GET /notes/by-path` query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ByPathQuery {
    /// Vault path, e.g. `notes/Pricing.md`.
    pub path: String,
}

fn note_id(id: &web::Path<Ulid>) -> NoteId {
    NoteId::from_ulid(**id)
}

fn created(note: Note) -> impl Responder {
    MsgPack(note)
        .customize()
        .with_status(StatusCode::CREATED)
}

/// The vault tree: folders, notes and other files (hidden folders excluded).
#[utoipa::path(
    get, path = "/tree", tag = "notes", operation_id = "get_tree",
    responses((status = 200, description = "Every folder, note and file, sorted by path.", body = Tree)),
)]
pub async fn tree(auth: Authenticated, vault: web::Data<VaultService>) -> Result<MsgPack<Tree>, Problem> {
    let entries = vault.tree(auth.scope()).await.or_problem()?;
    Ok(MsgPack(Tree {
        entries: entries
            .into_iter()
            .map(|e| match e {
                TreeEntry::Folder { path } => TreeItem::Folder { path },
                TreeEntry::File { path } => TreeItem::File { path },
                TreeEntry::Note { path, id, title, kind, updated } => TreeItem::Note {
                    path,
                    id: id.as_ulid(),
                    title,
                    kind: kind.into(),
                    updated,
                },
            })
            .collect(),
    }))
}

/// Create a note (duplicate check unless `force`).
#[utoipa::path(
    post, path = "/notes", tag = "notes", operation_id = "create_note",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, description = "Created; one `user: create <path>` commit.", body = Note),
        (status = 409, description = "`duplicate_candidates` (resend with `force`) or `path_taken`.", body = Problem),
        (status = 422, description = "`invalid_name` (path) or `invalid_body`.", body = Problem),
    ),
)]
pub async fn create_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    body: MsgPack<CreateNoteRequest>,
) -> Result<impl Responder, Problem> {
    let b = body.into_inner();
    let note = vault
        .create_note(
            auth.scope(),
            CreateNote {
                path: b.path,
                content: b.content,
                id: b.id.map(NoteId::from_ulid),
                force: b.force,
            },
        )
        .await
        .or_problem()?;
    Ok(created(note.into()))
}

/// A note (live or trashed) with its content and version.
#[utoipa::path(
    get, path = "/notes/{id}", tag = "notes", operation_id = "get_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses((status = 200, description = "The note.", body = Note)),
)]
pub async fn get_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Note>, Problem> {
    Ok(MsgPack(vault.note(auth.scope(), note_id(&id)).await.or_problem()?.into()))
}

/// A live note by vault path.
#[utoipa::path(
    get, path = "/notes/by-path", tag = "notes", operation_id = "get_note_by_path",
    params(ByPathQuery),
    responses(
        (status = 200, description = "The note.", body = Note),
        (status = 404, description = "`not_found`.", body = Problem),
    ),
)]
pub async fn get_note_by_path(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    q: web::Query<ByPathQuery>,
) -> Result<MsgPack<Note>, Problem> {
    Ok(MsgPack(vault.note_by_path(auth.scope(), &q.path).await.or_problem()?.into()))
}

/// Replace a note's content (`If-Match`: its current version).
#[utoipa::path(
    put, path = "/notes/{id}", tag = "notes", operation_id = "update_note",
    params(
        ("id" = Ulid, Path, description = "Note ID (ULID)."),
        ("If-Match" = String, Header, description = "The note's current version."),
    ),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, description = "Updated; one `user: update <path>` commit.", body = Note),
        (status = 409, description = "`version_conflict` with `current_version`.", body = Problem),
    ),
)]
pub async fn update_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<Ulid>,
    body: MsgPack<UpdateNoteRequest>,
) -> Result<MsgPack<Note>, Problem> {
    let version = required_if_match(req.headers())?;
    let note = vault
        .update_note(auth.scope(), note_id(&id), body.into_inner().content, version)
        .await
        .or_problem()?;
    Ok(MsgPack(note.into()))
}

/// Move or rename a note: every inbound link and relation is rewritten in the same commit.
#[utoipa::path(
    post, path = "/notes/{id}/move", tag = "notes", operation_id = "move_note",
    params(
        ("id" = Ulid, Path, description = "Note ID (ULID)."),
        ("If-Match" = Option<String>, Header, nullable = false, description = "Optional: the note's current version."),
    ),
    request_body = MoveNoteRequest,
    responses(
        (status = 200, description = "Moved; one `user: move <old> -> <new>` commit.", body = Note),
        (status = 409, description = "`path_taken` or `version_conflict`.", body = Problem),
    ),
)]
pub async fn move_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    req: HttpRequest,
    id: web::Path<Ulid>,
    body: MsgPack<MoveNoteRequest>,
) -> Result<MsgPack<Note>, Problem> {
    let version = crate::vault::if_match(req.headers())?;
    let note = vault
        .move_note(auth.scope(), note_id(&id), body.into_inner().new_path, version)
        .await
        .or_problem()?;
    Ok(MsgPack(note.into()))
}

/// Soft-delete a note (moved to `.trash/`).
#[utoipa::path(
    delete, path = "/notes/{id}", tag = "notes", operation_id = "delete_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses((status = 200, description = "Trashed; one `user: delete <path>` commit.", body = Note)),
)]
pub async fn delete_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Note>, Problem> {
    Ok(MsgPack(vault.delete_note(auth.scope(), note_id(&id)).await.or_problem()?.into()))
}

/// Restore a trashed note to its original path (or a free name next to it).
#[utoipa::path(
    post, path = "/trash/{id}/restore", tag = "notes", operation_id = "restore_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses((status = 200, description = "Restored; one `user: restore <path>` commit.", body = Note)),
)]
pub async fn restore_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Note>, Problem> {
    Ok(MsgPack(vault.restore_note(auth.scope(), note_id(&id)).await.or_problem()?.into()))
}

/// Permanently delete a trashed note and its sidecar (history keeps it).
#[utoipa::path(
    delete, path = "/trash/{id}", tag = "notes", operation_id = "purge_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID) of a trashed note.")),
    responses((status = 204, description = "Purged; one `user: purge <path>` commit.")),
)]
pub async fn purge_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<HttpResponse, Problem> {
    vault.purge_note(auth.scope(), note_id(&id)).await.or_problem()?;
    Ok(HttpResponse::NoContent().finish())
}

/// Body links and relations pointing at a note, grouped by kind.
#[utoipa::path(
    get, path = "/notes/{id}/backlinks", tag = "notes", operation_id = "get_backlinks",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses((status = 200, description = "Backlinks.", body = Backlinks)),
)]
pub async fn backlinks(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<Backlinks>, Problem> {
    let groups = vault.backlinks(auth.scope(), note_id(&id)).await.or_problem()?;
    Ok(MsgPack(Backlinks {
        groups: groups.into_iter().map(group).collect(),
    }))
}

fn group(g: model::BacklinkGroup) -> BacklinkGroup {
    BacklinkGroup {
        kind: g.kind,
        items: g
            .items
            .into_iter()
            .map(|b| Backlink {
                source_id: b.source_id.as_ulid(),
                source_path: b.source_path,
                source_title: b.source_title,
                by: b.by,
                confidence: b.confidence.map(f64::from),
                anchor: b.anchor,
                block_id: b.block_id,
            })
            .collect(),
    }
}

/// The git history of a note, newest first.
#[utoipa::path(
    get, path = "/notes/{id}/history", tag = "notes", operation_id = "get_note_history",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    responses((status = 200, description = "Commits that touched the note.", body = History)),
)]
pub async fn note_history(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
) -> Result<MsgPack<History>, Problem> {
    let revs = vault.history(auth.scope(), note_id(&id)).await.or_problem()?;
    Ok(MsgPack(History {
        revisions: revs
            .into_iter()
            .map(|r| Revision {
                author: match r.author.as_str() {
                    "user" => CommitAuthor::User,
                    "ai" => CommitAuthor::Ai,
                    _ => CommitAuthor::System,
                },
                change: match r.change.as_str() {
                    "added" => FileChange::Added,
                    "renamed" => FileChange::Renamed,
                    "deleted" => FileChange::Deleted,
                    _ => FileChange::Modified,
                },
                commit: r.commit,
                message: r.message,
                at: r.at,
                path: r.path,
            })
            .collect(),
    }))
}

/// A note's content as of a commit in its history.
#[utoipa::path(
    get, path = "/notes/{id}/history/{commit}", tag = "notes", operation_id = "get_note_revision",
    params(
        ("id" = Ulid, Path, description = "Note ID (ULID)."),
        ("commit" = String, Path, description = "Commit ID (40 hex characters) from the note's history."),
    ),
    responses((status = 200, description = "The note in that commit.", body = NoteRevision)),
)]
pub async fn note_revision(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    path: web::Path<(Ulid, String)>,
) -> Result<MsgPack<NoteRevision>, Problem> {
    let (id, commit) = path.into_inner();
    let r = vault
        .note_at(auth.scope(), NoteId::from_ulid(id), &commit)
        .await
        .or_problem()?;
    Ok(MsgPack(NoteRevision {
        commit: r.commit,
        path: r.path,
        content: r.content,
        version: r.version,
    }))
}

/// Restore a note's content from a commit in its history (a new `user: revert` commit).
#[utoipa::path(
    post, path = "/notes/{id}/revert", tag = "notes", operation_id = "revert_note",
    params(("id" = Ulid, Path, description = "Note ID (ULID).")),
    request_body = RevertNoteRequest,
    responses((status = 200, description = "Reverted.", body = Note)),
)]
pub async fn revert_note(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    id: web::Path<Ulid>,
    body: MsgPack<RevertNoteRequest>,
) -> Result<MsgPack<Note>, Problem> {
    let note = vault
        .revert_note(auth.scope(), note_id(&id), body.into_inner().commit)
        .await
        .or_problem()?;
    Ok(MsgPack(note.into()))
}

/// Revert a whole commit (typically an `ai:` commit) on top of the current state.
#[utoipa::path(
    post, path = "/commits/{commit}/revert", tag = "notes", operation_id = "revert_commit",
    params(("commit" = String, Path, description = "Commit ID (40 hex characters).")),
    responses(
        (status = 200, description = "Reverted in one new commit.", body = CommitReverted),
        (status = 409, description = "`revert_conflict`: later changes conflict with the revert.", body = Problem),
    ),
)]
pub async fn revert_commit(
    auth: Authenticated,
    vault: web::Data<VaultService>,
    commit: web::Path<String>,
) -> Result<MsgPack<CommitReverted>, Problem> {
    let (commit, paths) = vault
        .revert_commit(auth.scope(), commit.into_inner())
        .await
        .or_problem()?;
    Ok(MsgPack(CommitReverted { commit, paths }))
}

/// Mounts the note routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    let notes_limit = MsgPackConfig::default().with_body_limit(4 * 1024 * 1024);
    cfg.route("/tree", web::get().to(tree))
        .service(
            web::resource("/notes")
                .app_data(notes_limit)
                .route(web::post().to(create_note)),
        )
        .route("/notes/by-path", web::get().to(get_note_by_path))
        .service(
            web::resource("/notes/{id}")
                .app_data(notes_limit)
                .route(web::get().to(get_note))
                .route(web::put().to(update_note))
                .route(web::delete().to(delete_note)),
        )
        .route("/notes/{id}/move", web::post().to(move_note))
        .route("/notes/{id}/backlinks", web::get().to(backlinks))
        .route("/notes/{id}/history", web::get().to(note_history))
        .route("/notes/{id}/history/{commit}", web::get().to(note_revision))
        .route("/notes/{id}/revert", web::post().to(revert_note))
        .route("/trash/{id}/restore", web::post().to(restore_note))
        .route("/trash/{id}", web::delete().to(purge_note))
        .route("/commits/{commit}/revert", web::post().to(revert_commit));
}
