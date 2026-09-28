//! Saved mind-map layouts (PLAN §6.8, §7.5 `GET/PUT /maps`): JSON Canvas 1.0 files in
//! `maps/`, so they open in Obsidian. Only layouts the user saves are written; ephemeral
//! layouts stay on the device.
//!
//! - A map's ID is its file name without `.canvas` (`maps/<id>.canvas`); IDs follow the vault
//!   file-name rules.
//! - `PUT` validates the canvas (`vault_format::canvas`: JSON Canvas structure, unique IDs, no
//!   dangling edges, colours, sizes, subpaths) and that every file node references an
//!   existing, visible vault file (a note or an attachment), then stores it in Obsidian's
//!   layout through the writer actor as one `user: save map maps/<id>.canvas` commit. A new
//!   map is created without `If-Match`; replacing one requires its current version.
//! - Renaming or moving a note rewrites the file nodes of every canvas in the same commit
//!   (the vault's move path uses `Canvas::rename_file`).

use std::collections::{BTreeMap, BTreeSet};

use strata_common::NoteId;
use strata_index::{AppDb, UserScope};
use strata_vault::ops::files::{Expect, FileWrite};
use strata_vault::{Author, VaultError, VaultService, fsio, paths};
use vault_format::canvas::{Canvas, CanvasIssue, NodeKind};

use crate::error::{GraphError, MapIssue, Result};

/// Folder of saved maps.
pub const MAPS_DIR: &str = "maps";

/// A saved map in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSummary {
    /// ID (file name without `.canvas`).
    pub id: String,
    /// Vault path.
    pub path: String,
    /// Version (`sha256:` of the file).
    pub version: String,
    /// Node count (`None` when the file is not a valid canvas).
    pub nodes: Option<u32>,
    /// Edge count (`None` when the file is not a valid canvas).
    pub edges: Option<u32>,
}

/// A file node of a map, resolved against the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFile {
    /// Canvas node ID.
    pub node_id: String,
    /// Referenced vault path.
    pub path: String,
    /// The note at that path, if it is a live note.
    pub note_id: Option<NoteId>,
}

/// A saved map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapView {
    /// ID.
    pub id: String,
    /// Vault path.
    pub path: String,
    /// Version.
    pub version: String,
    /// The `.canvas` file (JSON Canvas 1.0 text, as stored).
    pub content: String,
    /// File nodes in canvas order.
    pub files: Vec<MapFile>,
}

/// The vault path of map `id` (validated).
pub fn map_path(id: &str) -> Result<String> {
    paths::validate_title(id).map_err(|_| GraphError::NotFound)?;
    Ok(format!("{MAPS_DIR}/{id}.canvas"))
}

fn issue(i: &CanvasIssue) -> MapIssue {
    match i {
        CanvasIssue::DuplicateId(id) => MapIssue {
            code: "duplicate_id",
            message: format!("ID `{id}` is used twice"),
        },
        CanvasIssue::DanglingEdge { edge, node } => MapIssue {
            code: "dangling_edge",
            message: format!("edge `{edge}` points at missing node `{node}`"),
        },
        CanvasIssue::BadColor(c) => MapIssue {
            code: "bad_color",
            message: format!("colour `{c}` is neither a preset 1-6 nor #RRGGBB"),
        },
        CanvasIssue::BadSubpath(s) => MapIssue {
            code: "bad_subpath",
            message: format!("subpath `{s}` must start with #"),
        },
        CanvasIssue::BadSize(n) => MapIssue {
            code: "bad_size",
            message: format!("node `{n}` needs a positive width and height"),
        },
    }
}

/// Live notes at `paths`, by path.
async fn notes_at(
    db: &AppDb,
    scope: &UserScope,
    paths: &BTreeSet<String>,
) -> Result<BTreeMap<String, NoteId>> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let list: Vec<String> = paths.iter().cloned().collect();
    let mut tx = db.begin(scope).await?;
    let rows: Vec<(String, NoteId)> =
        sqlx::query_as("SELECT path, id FROM notes WHERE NOT trashed AND path = ANY($1)")
            .bind(&list)
            .fetch_all(tx.conn())
            .await?;
    tx.commit().await?;
    Ok(rows.into_iter().collect())
}

fn resolve_files(canvas: &Canvas, notes: &BTreeMap<String, NoteId>) -> Vec<MapFile> {
    canvas
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::File { file, .. } => Some(MapFile {
                node_id: n.id.clone(),
                path: file.clone(),
                note_id: notes.get(file).copied(),
            }),
            _ => None,
        })
        .collect()
}

fn view(id: &str, path: String, bytes: &[u8], notes: &BTreeMap<String, NoteId>) -> MapView {
    let content = String::from_utf8_lossy(bytes).into_owned();
    let files = Canvas::from_json(&content)
        .map(|c| resolve_files(&c, notes))
        .unwrap_or_default();
    MapView {
        id: id.to_owned(),
        path,
        version: fsio::version_of(bytes),
        content,
        files,
    }
}

/// `GET /maps`: every `maps/*.canvas`, by ID.
pub async fn list(vault: &VaultService, scope: &UserScope) -> Result<Vec<MapSummary>> {
    vault.ready(scope).await?;
    let dir = vault.vault_dir(scope.user_id()).join(MAPS_DIR);
    let out = tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return out;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".canvas")) else {
                continue;
            };
            if paths::validate_title(id).is_err() || !e.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let Ok(bytes) = std::fs::read(e.path()) else {
                continue;
            };
            let parsed = Canvas::from_json(&String::from_utf8_lossy(&bytes)).ok();
            let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
            out.push(MapSummary {
                id: id.to_owned(),
                path: format!("{MAPS_DIR}/{id}.canvas"),
                version: fsio::version_of(&bytes),
                nodes: parsed.as_ref().map(|c| count(c.nodes.len())),
                edges: parsed.as_ref().map(|c| count(c.edges.len())),
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    })
    .await
    .map_err(|e| GraphError::Vault(VaultError::Internal(format!("listing maps: {e}"))))?;
    Ok(out)
}

/// `GET /maps/{id}`.
pub async fn get(vault: &VaultService, db: &AppDb, scope: &UserScope, id: &str) -> Result<MapView> {
    let path = map_path(id)?;
    let bytes = vault
        .read_file_bytes(scope, &path)
        .await?
        .ok_or(GraphError::NotFound)?;
    let files: BTreeSet<String> = Canvas::from_json(&String::from_utf8_lossy(&bytes))
        .map(|c| c.files().into_iter().map(str::to_owned).collect())
        .unwrap_or_default();
    let notes = notes_at(db, scope, &files).await?;
    Ok(view(id, path, &bytes, &notes))
}

/// Validates `content` for `scope`'s vault; returns the canvas and the notes its file nodes
/// reference.
async fn validate(
    vault: &VaultService,
    db: &AppDb,
    scope: &UserScope,
    content: &str,
) -> Result<(Canvas, BTreeMap<String, NoteId>)> {
    let canvas = Canvas::from_json(content).map_err(|e| {
        GraphError::InvalidMap(vec![MapIssue {
            code: "invalid_canvas",
            message: e.to_string(),
        }])
    })?;
    let mut issues: Vec<MapIssue> = canvas.validate().iter().map(issue).collect();
    let files: BTreeSet<String> = canvas.files().into_iter().map(str::to_owned).collect();
    let notes = notes_at(db, scope, &files).await?;
    let dir = vault.vault_dir(scope.user_id());
    let others: Vec<String> = files
        .iter()
        .filter(|f| !notes.contains_key(*f))
        .cloned()
        .collect();
    let missing = tokio::task::spawn_blocking(move || {
        others
            .into_iter()
            .map(|f| {
                let ok = paths::validate_path(&f).is_ok()
                    && paths::is_content(&f)
                    && std::fs::symlink_metadata(dir.join(&f)).is_ok_and(|m| m.is_file());
                (f, ok)
            })
            .filter(|(_, ok)| !ok)
            .map(|(f, _)| f)
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| GraphError::Vault(VaultError::Internal(format!("checking files: {e}"))))?;
    // The messages never quote the reference: it is user content (PLAN §15).
    for f in missing {
        issues.push(
            if paths::validate_path(&f).is_ok() && paths::is_content(&f) {
                MapIssue {
                    code: "unknown_file",
                    message: "a file node references a file that does not exist in this vault"
                        .to_owned(),
                }
            } else {
                MapIssue {
                    code: "invalid_file",
                    message: "a file node references a path that is not a visible vault file"
                        .to_owned(),
                }
            },
        );
    }
    if issues.is_empty() {
        Ok((canvas, notes))
    } else {
        Err(GraphError::InvalidMap(issues))
    }
}

/// `PUT /maps/{id}`: creates (no `if_match`) or replaces (`if_match` = current version) a
/// map. Returns the stored map and whether it was created.
pub async fn put(
    vault: &VaultService,
    db: &AppDb,
    scope: &UserScope,
    id: &str,
    content: &str,
    if_match: Option<String>,
) -> Result<(MapView, bool)> {
    let path = map_path(id)?;
    let (canvas, notes) = validate(vault, db, scope, content).await?;
    let bytes = canvas.to_json().into_bytes();
    let created = if_match.is_none();
    vault
        .write_file(
            scope,
            FileWrite {
                path: path.clone(),
                content: Some(bytes.clone()),
                expect: if_match.map_or(Expect::Absent, Expect::Version),
                author: Author::User,
                op: "save map".into(),
                index: None,
            },
        )
        .await?;
    Ok((view(id, path, &bytes, &notes), created))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_ids_follow_file_name_rules() {
        assert_eq!(map_path("Plan").expect("ok"), "maps/Plan.canvas");
        assert_eq!(map_path("خطة").expect("ok"), "maps/خطة.canvas");
        for bad in ["", "a/b", ".hidden", "a:b", ".."] {
            assert!(matches!(map_path(bad), Err(GraphError::NotFound)), "{bad}");
        }
    }

    #[test]
    fn canvas_issues_have_stable_codes() {
        let codes: Vec<&str> = [
            CanvasIssue::DuplicateId("a".into()),
            CanvasIssue::DanglingEdge {
                edge: "e".into(),
                node: "x".into(),
            },
            CanvasIssue::BadColor("red".into()),
            CanvasIssue::BadSubpath("h".into()),
            CanvasIssue::BadSize("n".into()),
        ]
        .iter()
        .map(|i| issue(i).code)
        .collect();
        assert_eq!(
            codes,
            [
                "duplicate_id",
                "dangling_edge",
                "bad_color",
                "bad_subpath",
                "bad_size"
            ]
        );
    }
}
