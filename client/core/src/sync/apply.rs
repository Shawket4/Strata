//! The database side of sync: applying pulled records, recording push results, resolving
//! conflicts and duplicates. Each function runs inside the caller's transaction and returns
//! the view topics it changed; the engine commits one transaction per step so a crash at any
//! point leaves a consistent database.

use rusqlite::{Connection, params};

use crate::error::{CoreError, CoreResult};
use crate::store::conflicts::{self, ConflictRow, RejectionRow};
use crate::store::index::Reindex;
use crate::store::notes::{self, NoteBase};
use crate::store::outbox::{self, OpStatus, OutboxOp};
use crate::store::write::{self, LocalEntity};
use crate::store::{settings, sync_state, to_msgpack};
use crate::sync::merge::NoteMerger;
use crate::sync::model::{ChangeRecord, OpPayload, OpResult};
use crate::view::Topics;

fn seen(conn: &Connection, kind: &str, id: &str) -> CoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO bootstrap_seen (kind, id) VALUES (?1, ?2)",
        params![kind, id],
    )?;
    Ok(())
}

/// Applies one pulled record. `bootstrapping` records the ID for the final sweep.
/// Re-applying the same record is a no-op (idempotent).
pub fn apply_record(
    conn: &Connection,
    record: &ChangeRecord,
    bootstrapping: bool,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    match record {
        ChangeRecord::NoteUpsert {
            id,
            path,
            content,
            version,
        } => {
            if bootstrapping {
                seen(conn, "note", id)?;
            }
            let base = notes::base(conn, id)?;
            let unchanged = base.as_ref().is_some_and(|b| {
                &b.version == version && &b.path == path && &b.content == content
            });
            if !unchanged {
                notes::set_base(
                    conn,
                    id,
                    Some(&NoteBase {
                        path: path.clone(),
                        content: content.clone(),
                        version: version.clone(),
                    }),
                    now,
                )?;
                write::rebuild_note(conn, id, now, re)?;
            }
        }
        ChangeRecord::NoteDelete { id } => {
            if notes::get(conn, id)?.is_some_and(|n| n.base_exists) {
                notes::set_base(conn, id, None, now)?;
                write::rebuild_note(conn, id, now, re)?;
            }
        }
        ChangeRecord::SuggestionUpsert {
            id,
            note_id,
            payload,
            status,
            created,
        } => {
            if bootstrapping {
                seen(conn, "suggestion", id)?;
            }
            conn.execute(
                "INSERT INTO suggestions (id, note_id, kind, payload, status, base_status, created)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)
                 ON CONFLICT (id) DO UPDATE SET note_id = excluded.note_id, kind = excluded.kind,
                    payload = excluded.payload, base_status = excluded.base_status,
                    created = excluded.created",
                params![
                    id,
                    note_id,
                    suggestion_kind(payload),
                    to_msgpack(payload)?,
                    status,
                    created
                ],
            )?;
            write::rebuild_suggestion(conn, id, re)?;
        }
        ChangeRecord::SuggestionDelete { id } => {
            conn.execute("DELETE FROM suggestions WHERE id = ?1", [id])?;
            re.topics(Topics::INBOX | Topics::SUGGESTIONS);
        }
        ChangeRecord::RelationMeta {
            src_id,
            dst_id,
            rel_type,
            by,
            confidence,
            reason,
        } => {
            conn.execute(
                "INSERT OR REPLACE INTO relation_meta (src_id, dst_id, rel_type, by, confidence, reason)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![src_id, dst_id, rel_type, by, confidence, reason],
            )?;
            re.topics(Topics::NOTES | Topics::ENTITIES);
        }
        ChangeRecord::RejectedEdge {
            src_id,
            dst_id,
            rel_type,
            at,
        } => {
            conn.execute(
                "INSERT OR REPLACE INTO rejected (src_id, dst_id, rel_type, at) VALUES (?1, ?2, ?3, ?4)",
                params![src_id, dst_id, rel_type, at],
            )?;
            re.topics(Topics::NOTES);
        }
        ChangeRecord::ClusterAssign {
            note_id,
            cluster_id,
        } => {
            conn.execute(
                "INSERT OR REPLACE INTO clusters (note_id, cluster_id) VALUES (?1, ?2)",
                params![note_id, cluster_id],
            )?;
            re.topics(Topics::NOTES);
        }
        ChangeRecord::ClusterName { cluster_id, name } => {
            conn.execute(
                "INSERT OR REPLACE INTO cluster_names (cluster_id, name) VALUES (?1, ?2)",
                params![cluster_id, name],
            )?;
            re.topics(Topics::NOTES);
        }
        ChangeRecord::DeviceSettings { reminders_enabled } => {
            if settings::set(
                conn,
                settings::REMINDERS_ENABLED,
                if *reminders_enabled { "true" } else { "false" },
            )? {
                re.topics(Topics::SETTINGS);
            }
        }
    }
    Ok(())
}

fn suggestion_kind(p: &crate::sync::model::SuggestionPayload) -> &'static str {
    use crate::sync::model::SuggestionPayload as S;
    match p {
        S::Filing { .. } => "filing",
        S::EntityLinkOrCreate { .. } => "entity_link_or_create",
        S::Custody { .. } => "custody",
        S::Duplicate { .. } => "duplicate",
        S::Relation { .. } => "relation",
        S::Task { .. } => "task",
        S::Other { .. } => "other",
    }
}

/// Starts a bootstrap: forgets what earlier pages saw and resets derived server-only tables
/// that are fully re-sent.
pub fn begin_bootstrap(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch(
        "DELETE FROM bootstrap_seen; DELETE FROM relation_meta; DELETE FROM rejected;
         DELETE FROM clusters; DELETE FROM cluster_names;",
    )?;
    Ok(())
}

/// Finishes a bootstrap: server rows not seen are gone (kept only if live ops still need
/// them), and every entity with live ops is rebuilt on the fresh data.
pub fn finish_bootstrap(conn: &Connection, now: &str, re: &mut Reindex) -> CoreResult<()> {
    let stale: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT id FROM notes WHERE base_exists = 1
               AND id NOT IN (SELECT id FROM bootstrap_seen WHERE kind = 'note') ORDER BY id",
        )?;
        st.query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for id in stale {
        notes::set_base(conn, &id, None, now)?;
        write::rebuild_note(conn, &id, now, re)?;
    }
    conn.execute(
        "DELETE FROM suggestions WHERE id NOT IN (SELECT id FROM bootstrap_seen WHERE kind = 'suggestion')",
        [],
    )?;
    re.topics(Topics::INBOX | Topics::SUGGESTIONS);
    let live: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT DISTINCT local_entity FROM outbox
             WHERE status IN ('pending', 'inflight', 'conflict', 'duplicate') ORDER BY 1",
        )?;
        st.query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for key in live {
        write::rebuild(conn, &LocalEntity::parse(&key), now, re)?;
    }
    conn.execute("DELETE FROM bootstrap_seen", [])?;
    Ok(())
}

/// Records the result of one pushed op (idempotent: a result for an op that is no longer
/// `inflight` is ignored).
pub fn record_result(
    conn: &Connection,
    op: &OutboxOp,
    result: &OpResult,
    merger: &dyn NoteMerger,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    let Some(current) = outbox::get(conn, &op.op_id)? else {
        return Ok(());
    };
    if current.status != OpStatus::Inflight {
        return Ok(());
    }
    let local = LocalEntity::parse(&op.local_entity);
    match result {
        OpResult::Applied { new_version } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Done)?;
            match &local {
                LocalEntity::Note(id) => {
                    if let Some(v) = new_version {
                        let prev = notes::base(conn, id)?;
                        if let Some(next) =
                            write::base_after_applied(id, &op.entity_id, prev, &op.payload, v)
                        {
                            notes::set_base(conn, id, next.as_ref(), now)?;
                        }
                    }
                    write::rebuild_note(conn, id, now, re)?;
                }
                LocalEntity::Suggestion(id) => {
                    let next = write::apply_to_suggestion(Some(String::new()), &op.payload)
                        .filter(|s| !s.is_empty());
                    if let Some(s) = next {
                        conn.execute(
                            "UPDATE suggestions SET base_status = ?2 WHERE id = ?1",
                            params![id, s],
                        )?;
                    }
                    write::rebuild_suggestion(conn, id, re)?;
                }
                LocalEntity::Nothing => {}
            }
        }
        OpResult::Conflict {
            server_version,
            server_content,
            ..
        } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Conflict)?;
            let LocalEntity::Note(id) = &local else {
                return Err(CoreError::Internal("conflict on a non-note op".into()));
            };
            let base = notes::base(conn, id)?;
            let local_content = notes::current(conn, id)?.map(|s| s.content);
            let preview = match (&base, &local_content, server_content) {
                (Some(b), Some(l), Some(s)) => merger.merge(&b.content, l, s),
                _ => None,
            };
            conflicts::put_conflict(
                conn,
                &ConflictRow {
                    op_id: op.op_id.clone(),
                    entity_id: id.clone(),
                    local_payload: op.payload.clone(),
                    base_content: base.as_ref().map(|b| b.content.clone()),
                    local_content,
                    server_version: server_version.clone(),
                    server_content: server_content.clone(),
                    merged_preview: preview.as_ref().map(|p| p.merged.clone()),
                    merge_clean: preview.as_ref().map(|p| p.clean),
                    created: now.to_owned(),
                },
            )?;
            if let (Some(content), Some(b)) = (server_content, &base) {
                notes::set_base(
                    conn,
                    id,
                    Some(&NoteBase {
                        path: b.path.clone(),
                        content: content.clone(),
                        version: server_version.clone(),
                    }),
                    now,
                )?;
            }
            write::rebuild_note(conn, id, now, re)?;
        }
        OpResult::Duplicate { candidates } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Duplicate)?;
            conflicts::put_duplicate(conn, &op.op_id, candidates, now)?;
        }
        OpResult::Rejected {
            problem_type,
            status,
        } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Rejected)?;
            conflicts::put_rejection(
                conn,
                &RejectionRow {
                    op_id: op.op_id.clone(),
                    kind: op.kind().as_str().to_owned(),
                    entity_id: op.entity_id.clone(),
                    problem_type: problem_type.clone(),
                    status: *status,
                    created: now.to_owned(),
                },
            )?;
            write::rebuild(conn, &local, now, re)?;
        }
    }
    re.topics(Topics::SYNC);
    Ok(())
}

/// Resolves a conflict (D19): drop the local edit, or push the local/merged content over the
/// server version with a new op.
pub fn resolve_conflict(
    conn: &Connection,
    op_id: &str,
    keep: Option<String>,
    new_op_id: &str,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    let Some(c) = conflicts::conflict(conn, op_id)? else {
        return Err(CoreError::not_found("conflict"));
    };
    let Some(op) = outbox::get(conn, op_id)? else {
        return Err(CoreError::not_found("op"));
    };
    outbox::set_status(conn, op_id, OpStatus::Done)?;
    conflicts::delete_conflict(conn, op_id)?;
    if let Some(content) = keep {
        outbox::append(
            conn,
            &outbox::NewOp {
                op_id: new_op_id,
                entity_id: &op.entity_id,
                local_entity: &op.local_entity,
                base_version: Some(&c.server_version),
                payload: &OpPayload::NoteUpdate { content },
                created: now,
            },
        )?;
    }
    write::rebuild(conn, &LocalEntity::parse(&op.local_entity), now, re)?;
    outbox::compact(conn)?;
    re.topics(Topics::SYNC);
    Ok(())
}

/// Resolves a duplicate prompt: create anyway (same op resent under `new_op_id` with
/// `force`, keeping its place in the queue) or discard (the local item disappears).
pub fn resolve_duplicate(
    conn: &Connection,
    op_id: &str,
    create_anyway: bool,
    new_op_id: &str,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    let Some(op) = outbox::get(conn, op_id)? else {
        return Err(CoreError::not_found("op"));
    };
    if op.status != OpStatus::Duplicate {
        return Err(CoreError::not_found("duplicate"));
    }
    conflicts::delete_duplicate(conn, op_id)?;
    if create_anyway {
        conn.execute(
            "UPDATE outbox SET op_id = ?2, payload = ?3, status = 'pending' WHERE op_id = ?1",
            params![op_id, new_op_id, to_msgpack(&op.payload.forced())?],
        )?;
    } else {
        outbox::delete(conn, op_id)?;
    }
    write::rebuild(conn, &LocalEntity::parse(&op.local_entity), now, re)?;
    re.topics(Topics::SYNC);
    Ok(())
}

/// Marks sync progress.
pub fn touch_state(conn: &Connection, f: impl FnOnce(&mut sync_state::SyncState)) -> CoreResult<()> {
    sync_state::update(conn, f).map(|_| ())
}
