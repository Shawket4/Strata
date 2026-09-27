//! The database side of sync: applying pulled records, recording push results, resolving
//! conflicts and duplicates. Each function runs inside the caller's transaction; the engine
//! commits one transaction per step so a crash at any point leaves a consistent database.

use rusqlite::{Connection, params};
use sync_model::{Change, EntityType, MergeOutcome, Record};

use crate::error::{CoreError, CoreResult};
use crate::store::conflicts::{self, ConflictRow, RejectionRow};
use crate::store::index::Reindex;
use crate::store::notes::{self, NoteBase};
use crate::store::outbox::{self, OpStatus, OutboxOp};
use crate::store::settings;
use crate::store::write::{self, DbLinks, LocalEntity};
use crate::sync::model::{Op, OpResult, Version};
use crate::view::Topics;

/// A local 3-way merge preview (D19) with `sync_model::merge`: ours = the local edit,
/// theirs = the server's content.
pub fn merge_preview(
    base: &str,
    local: &str,
    server: &str,
) -> (Option<String>, bool, MergeOutcome) {
    let outcome = sync_model::merge(base, local, server);
    match &outcome {
        MergeOutcome::Clean(text) => (Some(text.clone()), true, outcome.clone()),
        MergeOutcome::Conflicted(c) => (c.merged_with_markers.clone(), false, outcome.clone()),
    }
}

fn seen(conn: &Connection, entity_type: EntityType, id: &str) -> CoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO bootstrap_seen (kind, id) VALUES (?1, ?2)",
        params![entity_type.as_str(), id],
    )?;
    Ok(())
}

/// After a note's server base changed: conflicts on it get the server content and a fresh
/// merge preview.
fn refresh_conflicts(conn: &Connection, note_id: &str, server: &NoteBase) -> CoreResult<bool> {
    let mut changed = false;
    for mut c in conflicts::conflicts_of(conn, note_id)? {
        if c.server_content.as_deref() == Some(server.content.as_str()) {
            continue;
        }
        c.server_content = Some(server.content.clone());
        c.server_version = Some(server.version.clone());
        if let (Some(base), Some(local)) = (&c.base_content, &c.local_content) {
            let (merged, clean, outcome) = merge_preview(base, local, &server.content);
            c.merged_preview = merged;
            c.merge_clean = Some(clean);
            c.merge_outcome = Some(outcome);
        }
        conflicts::put_conflict(conn, &c)?;
        changed = true;
    }
    Ok(changed)
}

/// This device's ID (device settings records are per device).
pub fn device_id(conn: &Connection) -> CoreResult<Option<String>> {
    Ok(crate::store::tokens::get(conn)?.map(|t| t.device_id))
}

/// Applies one pulled upsert. `bootstrapping` records the ID for the final sweep.
/// Re-applying the same record is a no-op (idempotent).
#[allow(clippy::too_many_lines)] // one arm per record kind
pub fn apply_record(
    conn: &Connection,
    record: &Record,
    bootstrapping: bool,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    if bootstrapping {
        seen(conn, record.entity_type(), &record.entity_id())?;
    }
    match record {
        Record::Note(n) => {
            let id = n.id.to_string();
            let base = notes::base(conn, &id)?;
            let next = NoteBase {
                path: n.path.clone(),
                content: n.content.clone(),
                version: n.version.as_str().to_owned(),
            };
            if base.as_ref() != Some(&next) {
                notes::set_base(conn, &id, Some(&next), now)?;
                write::rebuild_note(conn, &id, now, re)?;
                if refresh_conflicts(conn, &id, &next)? {
                    re.topics(Topics::SYNC);
                }
            }
        }
        Record::Suggestion(s) => {
            let status = serde_plain(&s.status);
            conn.execute(
                "INSERT INTO suggestions (id, note_id, kind, payload, status, base_status, created)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)
                 ON CONFLICT (id) DO UPDATE SET note_id = excluded.note_id, kind = excluded.kind,
                    payload = excluded.payload, base_status = excluded.base_status,
                    created = excluded.created",
                params![
                    s.id.to_string(),
                    s.note_id.map(|n| n.to_string()),
                    s.kind,
                    s.payload,
                    status,
                    s.created.to_rfc3339()
                ],
            )?;
            write::rebuild_suggestion(conn, &s.id.to_string(), re)?;
        }
        Record::Relation(r) => {
            conn.execute(
                "INSERT OR REPLACE INTO relation_meta (src_id, dst_id, rel_type, by, confidence, reason)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    r.src_id.to_string(),
                    r.dst_id.to_string(),
                    r.relation.as_str(),
                    r.by.as_str(),
                    r.confidence.map(f64::from),
                    r.reason
                ],
            )?;
            re.topics(Topics::NOTES | Topics::ENTITIES);
        }
        Record::Rejected(r) => {
            conn.execute(
                "INSERT OR REPLACE INTO rejected (src_id, dst_id, rel_type, at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    r.src_id.to_string(),
                    r.dst_id.to_string(),
                    r.relation.as_str(),
                    r.at.to_rfc3339()
                ],
            )?;
            re.topics(Topics::NOTES);
        }
        Record::ClusterAssignment(c) => {
            conn.execute(
                "INSERT OR REPLACE INTO clusters (note_id, cluster_id) VALUES (?1, ?2)",
                params![c.note_id.to_string(), c.cluster_id],
            )?;
            re.topics(Topics::NOTES);
        }
        Record::ClusterName(c) => {
            conn.execute(
                "INSERT OR REPLACE INTO cluster_names (cluster_id, name) VALUES (?1, ?2)",
                params![c.cluster_id, c.name],
            )?;
            re.topics(Topics::NOTES);
        }
        Record::Setting(s) => {
            conn.execute(
                "INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
                params![s.key, s.value],
            )?;
            re.topics(Topics::SETTINGS);
        }
        Record::DeviceSetting(d) => {
            if device_id(conn)?.as_deref() == Some(d.device_id.to_string().as_str())
                && settings::set(conn, &d.key, &d.value)?
            {
                re.topics(Topics::SETTINGS);
            }
        }
        Record::KeepBoth(k) => {
            conn.execute(
                "INSERT OR IGNORE INTO keep_both (kind, a_id, b_id) VALUES (?1, ?2, ?3)",
                params![k.kind.as_str(), k.a_id, k.b_id],
            )?;
        }
    }
    Ok(())
}

fn serde_plain<T: serde::Serialize>(v: &T) -> String {
    // Unit enums serialise as their snake_case name.
    rmp_serde::to_vec(v)
        .ok()
        .and_then(|b| rmp_serde::from_slice::<String>(&b).ok())
        .unwrap_or_default()
}

/// Applies a tombstone.
pub fn apply_delete(
    conn: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    match entity_type {
        EntityType::Note => {
            if notes::get(conn, entity_id)?.is_some_and(|n| n.base_exists) {
                notes::set_base(conn, entity_id, None, now)?;
                write::rebuild_note(conn, entity_id, now, re)?;
            }
        }
        EntityType::Suggestion => {
            conn.execute("DELETE FROM suggestions WHERE id = ?1", [entity_id])?;
            re.topics(Topics::INBOX | Topics::SUGGESTIONS);
        }
        EntityType::Relation | EntityType::Rejected => {
            let mut parts = entity_id.splitn(3, ':');
            let (Some(src), Some(rel), Some(dst)) = (parts.next(), parts.next(), parts.next())
            else {
                return Err(CoreError::Storage(format!("bad relation key {entity_id}")));
            };
            let table = if entity_type == EntityType::Relation {
                "relation_meta"
            } else {
                "rejected"
            };
            conn.execute(
                &format!("DELETE FROM {table} WHERE src_id = ?1 AND rel_type = ?2 AND dst_id = ?3"),
                params![src, rel, dst],
            )?;
            re.topics(Topics::NOTES);
        }
        EntityType::ClusterAssignment => {
            conn.execute("DELETE FROM clusters WHERE note_id = ?1", [entity_id])?;
            re.topics(Topics::NOTES);
        }
        EntityType::ClusterName => {
            conn.execute(
                "DELETE FROM cluster_names WHERE cluster_id = ?1",
                [entity_id],
            )?;
            re.topics(Topics::NOTES);
        }
        EntityType::Setting => {
            conn.execute("DELETE FROM user_settings WHERE key = ?1", [entity_id])?;
            re.topics(Topics::SETTINGS);
        }
        EntityType::DeviceSetting => {
            if let Some((device, key)) = entity_id.split_once(':')
                && device_id(conn)?.as_deref() == Some(device)
            {
                conn.execute("DELETE FROM device_settings WHERE key = ?1", [key])?;
                re.topics(Topics::SETTINGS);
            }
        }
        EntityType::KeepBoth => {
            let mut parts = entity_id.splitn(3, ':');
            if let (Some(kind), Some(a), Some(b)) = (parts.next(), parts.next(), parts.next()) {
                conn.execute(
                    "DELETE FROM keep_both WHERE kind = ?1 AND a_id = ?2 AND b_id = ?3",
                    params![kind, a, b],
                )?;
            }
        }
    }
    Ok(())
}

/// Applies one change of a changes page.
pub fn apply_change(
    conn: &Connection,
    change: &sync_model::ChangeRecord,
    now: &str,
    re: &mut Reindex,
) -> CoreResult<()> {
    match &change.change {
        Change::Upsert { record } => apply_record(conn, record, false, now, re),
        Change::Delete => apply_delete(conn, change.entity_type, &change.entity_id, now, re),
    }
}

/// Starts a bootstrap: forgets what earlier pages saw and clears server-only tables that the
/// snapshot re-sends in full.
pub fn begin_bootstrap(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch(
        "DELETE FROM bootstrap_seen; DELETE FROM relation_meta; DELETE FROM rejected;
         DELETE FROM clusters; DELETE FROM cluster_names; DELETE FROM user_settings;
         DELETE FROM keep_both;",
    )?;
    Ok(())
}

/// Finishes a bootstrap: server rows not seen are gone (kept only while live ops need them),
/// and every entity with live ops is rebuilt on the fresh data.
pub fn finish_bootstrap(conn: &Connection, now: &str, re: &mut Reindex) -> CoreResult<()> {
    let stale: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT id FROM notes WHERE base_exists = 1
               AND id NOT IN (SELECT id FROM bootstrap_seen WHERE kind = 'note') ORDER BY id",
        )?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for id in stale {
        notes::set_base(conn, &id, None, now)?;
        write::rebuild_note(conn, &id, now, re)?;
    }
    conn.execute(
        "DELETE FROM suggestions WHERE id NOT IN (SELECT id FROM bootstrap_seen WHERE kind = 'suggestion')",
        [],
    )?;
    re.topics(Topics::INBOX | Topics::SUGGESTIONS | Topics::SETTINGS);
    let live: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT DISTINCT local_entity FROM outbox
             WHERE status IN ('pending', 'inflight', 'conflict', 'duplicate') ORDER BY 1",
        )?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for key in live {
        write::rebuild(conn, &LocalEntity::parse(&key), now, re)?;
    }
    conn.execute("DELETE FROM bootstrap_seen", [])?;
    Ok(())
}

/// Records the result of one pushed op (idempotent: a result for an op that is no longer
/// `inflight` is ignored).
#[allow(clippy::too_many_lines)] // one arm per result kind
pub fn record_result(
    conn: &Connection,
    op: &OutboxOp,
    result: &OpResult,
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
        OpResult::Applied { new_version, .. } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Done)?;
            match &local {
                LocalEntity::Note(id) => {
                    let links = DbLinks::load(conn)?;
                    let prev = notes::base(conn, id)?;
                    if let Some(next) =
                        write::base_after_applied(prev, &op.op, new_version.as_ref(), &links)
                    {
                        notes::set_base(conn, id, next.as_ref(), now)?;
                    }
                    write::rebuild_note(conn, id, now, re)?;
                }
                LocalEntity::Suggestion(id) => {
                    if let Some(s) = write::apply_to_suggestion(Some(String::new()), &op.op)
                        .filter(|s| !s.is_empty())
                    {
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
            resolution,
        } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Conflict)?;
            let LocalEntity::Note(id) = &local else {
                return Err(CoreError::Internal("conflict on a non-note op".into()));
            };
            let base = notes::base(conn, id)?;
            let local_content = notes::current(conn, id)?.map(|s| s.content);
            conflicts::put_conflict(
                conn,
                &ConflictRow {
                    op_id: op.op_id.clone(),
                    entity_id: id.clone(),
                    local_op: op.op.clone(),
                    base_content: base.as_ref().map(|b| b.content.clone()),
                    local_content,
                    server_version: server_version.as_ref().map(|v| v.as_str().to_owned()),
                    // The server's content arrives with the next pull (refresh_conflicts).
                    server_content: None,
                    merged_preview: None,
                    merge_clean: None,
                    merge_outcome: None,
                    resolution: Some(resolution.clone()),
                    created: now.to_owned(),
                },
            )?;
            write::rebuild_note(conn, id, now, re)?;
        }
        OpResult::Duplicate { candidates } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Duplicate)?;
            conflicts::put_duplicate(conn, &op.op_id, candidates, now)?;
        }
        OpResult::Rejected { problem } => {
            outbox::set_status(conn, &op.op_id, OpStatus::Rejected)?;
            conflicts::put_rejection(
                conn,
                &RejectionRow {
                    op_id: op.op_id.clone(),
                    kind: op.kind().as_str().to_owned(),
                    entity_id: op.entity_id.clone(),
                    problem_type: problem.problem_type.clone(),
                    status: problem.status,
                    created: now.to_owned(),
                },
            )?;
            write::rebuild(conn, &local, now, re)?;
        }
    }
    re.topics(Topics::SYNC);
    Ok(())
}

/// Resolves a conflict (D19): drop the local edit (`keep = None`), or push `keep` over the
/// server's version with a new `note.update`.
pub fn resolve_conflict(
    conn: &Connection,
    op_id: &str,
    keep: Option<String>,
    new_op_id: ulid::Ulid,
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
    let local = LocalEntity::parse(&op.local_entity);
    write::rebuild(conn, &local, now, re)?;
    if let Some(content) = keep {
        let note_id = crate::store::parse_ulid(&c.entity_id)?;
        let current = notes::current(conn, &c.entity_id)?.map(|s| s.content);
        outbox::append(
            conn,
            &outbox::NewOp {
                op_id: &new_op_id.to_string(),
                local_entity: &op.local_entity,
                base_version: current.as_deref().map(Version::of_text).as_ref(),
                op: &Op::NoteUpdate(sync_model::ops::NoteUpdate {
                    id: note_id,
                    content,
                }),
                created: now,
            },
        )?;
        write::rebuild(conn, &local, now, re)?;
    }
    outbox::compact(conn)?;
    re.topics(Topics::SYNC);
    Ok(())
}

/// The same create op with the duplicate check skipped (`force: true`).
pub fn forced(op: &Op) -> Op {
    let mut op = op.clone();
    match &mut op {
        Op::NoteCreate(p) => p.force = true,
        Op::EntityCreate(p) => p.force = true,
        Op::DocumentCreate(p) => p.force = true,
        Op::PlaceCreate(p) => p.force = true,
        Op::TaskCreate(p) => p.force = true,
        _ => {}
    }
    op
}

/// Resolves a duplicate prompt: create anyway (the same op resent under `new_op_id` with
/// `force`, keeping its place in the queue) or discard (the local item disappears).
pub fn resolve_duplicate(
    conn: &Connection,
    op_id: &str,
    create_anyway: bool,
    new_op_id: ulid::Ulid,
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
            params![
                op_id,
                new_op_id.to_string(),
                forced(&op.op).payload_bytes()?
            ],
        )?;
    } else {
        outbox::delete(conn, op_id)?;
    }
    write::rebuild(conn, &LocalEntity::parse(&op.local_entity), now, re)?;
    re.topics(Topics::SYNC);
    Ok(())
}
