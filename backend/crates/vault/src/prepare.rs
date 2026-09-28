//! Preparing note content before it is written: IDs, timestamps, task block IDs and sidecar
//! consistency (PLAN §6.4, §6.5, §6.11).

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use strata_common::{IdGenerator, NoteId};
use vault_format::sidecar::NoteSidecar;
use vault_format::tasks::extract_tasks;
use vault_format::{Document, PathIndex, RelationKey, Resolution};

use crate::error::{Result, VaultError};
use crate::state::VaultState;

/// A new task block ID: `t-<lower-case ULID>` (the shared rule).
pub fn new_task_id(ids: &dyn IdGenerator) -> String {
    item_render::task::task_block_id(ids.next_ulid())
}

/// The ULID inside a task block ID `t-<ulid>`, if it has that form.
pub use item_render::task::task_block_ulid as task_ulid;

/// Appends `^t-<ulid>` to every task line without a block ID, and replaces block IDs that
/// repeat within the note or are in `taken` (IDs of tasks in other notes). Returns the new
/// body (unchanged if nothing needed an ID).
pub fn assign_task_ids(body: &str, ids: &dyn IdGenerator, taken: &BTreeSet<String>) -> String {
    let tasks = extract_tasks(body);
    if tasks.is_empty() {
        return body.to_owned();
    }
    let mut out = String::with_capacity(body.len() + tasks.len() * 30);
    let mut last = 0;
    let mut seen = BTreeSet::new();
    for t in &tasks {
        let line = &body[t.line_span.clone()];
        let new_line = match t.task.block_id() {
            Some(id) if seen.insert(id.to_owned()) && !taken.contains(id) => continue,
            Some(_) => t.task.with_block_id(&new_task_id(ids)).as_str().to_owned(),
            None => {
                let id = new_task_id(ids);
                seen.insert(id.clone());
                let trimmed = line.trim_end();
                format!("{trimmed} ^{id}{}", &line[trimmed.len()..])
            }
        };
        out.push_str(&body[last..t.line_span.start]);
        out.push_str(&new_line);
        last = t.line_span.end;
    }
    out.push_str(&body[last..]);
    out
}

/// Frontmatter edits that make `doc` a valid note with `id`: sets `id` (if missing or
/// different), `created` (if missing) and `updated` (if given). The shared rule
/// (`item_render::note::stamp`) with the vault's error messages.
pub fn stamp(
    doc: &mut Document,
    id: NoteId,
    created: Option<&DateTime<Utc>>,
    updated: Option<&DateTime<Utc>>,
) -> Result<()> {
    item_render::note::stamp(doc, id.as_ulid(), created, updated).map_err(|e| render_error(&e))
}

/// A shared rendering error as a vault error (no user content in the message).
pub fn render_error(e: &item_render::RenderError) -> VaultError {
    match e {
        item_render::RenderError::Unreadable(e) => VaultError::Invalid(
            format!("the frontmatter cannot be edited: {}", error_kind(e)).into(),
        ),
        item_render::RenderError::Property { key, .. } => match key.as_str() {
            "id" => VaultError::invalid("the id property could not be set"),
            "created" => VaultError::invalid("the created property could not be set"),
            "updated" => VaultError::invalid("the updated property could not be set"),
            _ => VaultError::invalid("the property could not be set"),
        },
    }
}

fn error_kind(e: &vault_format::FrontmatterError) -> &'static str {
    match e {
        vault_format::FrontmatterError::InvalidYaml(_) => "invalid YAML",
        _ => "unsupported layout",
    }
}

/// The (type, target) edges the frontmatter of the note at `path` declares, resolved.
pub fn frontmatter_edges(
    doc: &Document,
    path: &str,
    index: &PathIndex,
    state: &VaultState,
) -> BTreeSet<(RelationKey, NoteId)> {
    let mut out = BTreeSet::new();
    let Some(fm) = doc.frontmatter().filter(|f| f.error().is_none()) else {
        return out;
    };
    for rk in RelationKey::all() {
        for l in fm.relation_links(rk) {
            if let Resolution::Resolved(p) = index.resolve(&l.path, Some(path))
                && let Some(m) = state.notes.get(&p)
            {
                out.insert((rk, m.id));
            }
        }
    }
    out
}

/// Drops sidecar provenance for edges the frontmatter no longer has (frontmatter wins for the
/// existence of an edge, PLAN §7.3). Returns whether the sidecar changed.
pub fn prune_sidecar(sidecar: &mut NoteSidecar, edges: &BTreeSet<(RelationKey, NoteId)>) -> bool {
    let before = sidecar.relations.len();
    sidecar
        .relations
        .retain(|r| edges.contains(&(r.kind, NoteId::from_ulid(r.target_id))));
    sidecar.relations.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_common::SequentialIdGenerator;

    #[test]
    fn task_ids_are_appended_and_duplicates_replaced() {
        let ids = SequentialIdGenerator::new(1_790_510_400_000);
        let body = "- [ ] one\n- [x] two ^t-a\n- [ ] three ^t-a\n```\n- [ ] code\n```\n- [ ] four ^t-b  \n";
        let taken: BTreeSet<String> = ["t-b".to_owned()].into();
        let out = assign_task_ids(body, &ids, &taken);
        let a = ids.nth(1).to_string().to_lowercase();
        let b = ids.nth(2).to_string().to_lowercase();
        let c = ids.nth(3).to_string().to_lowercase();
        assert_eq!(
            out,
            format!(
                "- [ ] one ^t-{a}\n- [x] two ^t-a\n- [ ] three ^t-{b}\n```\n- [ ] code\n```\n- [ ] four ^t-{c}  \n"
            )
        );
        assert_eq!(task_ulid(&format!("t-{a}")), Some(ids.nth(1)));
        assert_eq!(task_ulid("t-x"), None);
    }
}
