//! Records for `GET /sync/bootstrap` and `GET /sync/changes` (PLAN §7.5 Sync, §12.2).
//!
//! Records are the shared `sync-model` [`Record`]s: notes with their full content and version
//! (entities, documents, places, the task home and inbox notes are notes; tags, links,
//! frontmatter relations, entity fields and tasks are derived from the content with
//! `vault-format` on the device exactly as the server's indexer does), plus the state that is
//! not in note text: relations with provenance, rejected edges, suggestions, cluster
//! assignments and names, user settings, device settings and keep-both pairs.
//!
//! **Bootstrap paging** reads the current state section by section with keyset paging
//! (`after` the last key of the previous page), so an item that exists for the whole bootstrap
//! is returned exactly once however the vault changes meanwhile. The log position (`seq`) is
//! captured on the first page and carried in the cursor: anything that changes after it,
//! including items a later page already shows in their new state, is also in the changes feed
//! after `seq`, and applying it again is idempotent (records are full states). A note whose
//! file moved while a page was read is skipped for the same reason.
//!
//! **Changes** are the log rows after `since`; within one page only the newest row of an
//! entity is kept (its record is the current state anyway). A record that no longer exists is
//! a tombstone.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, FixedOffset, Utc};
use dedupe::KeepBoth;
use domain::{DedupeKind, NoteKind as DNoteKind, RelationOrigin};
use strata_common::{NoteId, SuggestionId};
use strata_index::repo::suggestions::{self as srepo, Suggestion};
use strata_index::repo::sync as log;
use strata_index::types::{By, SuggestionStatus as IStatus};
use strata_index::{AppDb, IndexError, ScopedTx, UserScope};
use strata_vault::{VaultError, VaultService, fsio};
use sync_model::changes::{
    ClusterAssignmentRecord, ClusterNameRecord, DeviceSettingRecord, NoteRecord, RejectedRecord,
    RelationRecord, SuggestionRecord, SuggestionReplyRecord, SuggestionStatus,
};
use sync_model::settings::{self, SettingValue};
use sync_model::{BootstrapPage, ChangeRecord, ChangesPage, EntityType, Record, Version};
use ulid::Ulid;
use uuid::Uuid;
use vault_format::RelationKey;
use vault_format::sidecar::NoteSidecar;

use crate::sync::{BootstrapCursor, epoch_changed};
use crate::vault::problem;
use crate::wire::Problem;

/// Bootstrap sections, in order.
const SECTIONS: u8 = 9;

fn index_problem(e: IndexError) -> Problem {
    problem(&VaultError::Index(e))
}

fn sqlx_problem(e: sqlx::Error) -> Problem {
    index_problem(e.into())
}

fn fixed(t: DateTime<Utc>) -> DateTime<FixedOffset> {
    t.fixed_offset()
}

fn ulid(u: Uuid) -> Ulid {
    Ulid::from(u)
}

/// A note row whose file is read after the transaction.
struct NoteRow {
    id: NoteId,
    path: String,
    kind: DNoteKind,
}

enum Item {
    Note(NoteRow),
    Ready(Record),
}

/// `<src>:<type>:<dst>` → parts.
fn split_edge(key: &str) -> Option<(Uuid, String, Uuid)> {
    let (src, rest) = key.split_once(':')?;
    let (rel, dst) = rest.rsplit_once(':')?;
    let src = Ulid::from_string(src).ok()?;
    let dst = Ulid::from_string(dst).ok()?;
    Some((Uuid::from(src), rel.to_owned(), Uuid::from(dst)))
}

fn edge_key(src: Uuid, rel: &str, dst: Uuid) -> String {
    format!("{}:{rel}:{}", ulid(src), ulid(dst))
}

type RelationRow = (
    Uuid,
    String,
    Uuid,
    By,
    Option<f32>,
    Option<String>,
    DateTime<Utc>,
);

fn relation_record(r: RelationRow) -> Option<Record> {
    let (src, rel, dst, by, confidence, reason, created) = r;
    Some(Record::Relation(RelationRecord {
        src_id: ulid(src),
        dst_id: ulid(dst),
        relation: rel.parse::<RelationKey>().ok()?,
        by: match by {
            By::User => RelationOrigin::User,
            By::Ai => RelationOrigin::Ai,
        },
        confidence,
        reason,
        created: Some(fixed(created)),
    }))
}

fn rejected_record(r: (Uuid, String, Uuid, DateTime<Utc>)) -> Option<Record> {
    let (src, rel, dst, at) = r;
    Some(Record::Rejected(RejectedRecord {
        src_id: ulid(src),
        dst_id: ulid(dst),
        relation: rel.parse::<RelationKey>().ok()?,
        at: fixed(at),
    }))
}

async fn suggestion_record(tx: &mut ScopedTx, s: Suggestion) -> Result<Record, Problem> {
    let replies = srepo::replies(tx, s.id).await.map_err(index_problem)?;
    Ok(Record::Suggestion(SuggestionRecord {
        id: s.id.as_ulid(),
        note_id: s.note_id.map(|n| n.as_ulid()),
        kind: s.kind,
        status: match s.status {
            IStatus::Pending => SuggestionStatus::Pending,
            IStatus::Accepted => SuggestionStatus::Accepted,
            IStatus::Rejected => SuggestionStatus::Rejected,
            IStatus::Superseded => SuggestionStatus::Superseded,
        },
        payload: s.payload,
        created: fixed(s.created),
        replies: replies
            .into_iter()
            .map(|r| SuggestionReplyRecord {
                id: r.id.as_ulid(),
                text: r.body,
                at: fixed(r.created),
            })
            .collect(),
    }))
}

/// A stored setting value (`MessagePack`) as the `sync-model` [`SettingValue`] its records are
/// built from (see [`sync_model::settings`]).
pub fn setting_value(bytes: &[u8]) -> SettingValue {
    rmpv::decode::read_value(&mut &bytes[..]).map_or(SettingValue::Other, |v| value_of(&v, true))
}

fn value_of(v: &rmpv::Value, top: bool) -> SettingValue {
    match v {
        rmpv::Value::String(s) => s
            .as_str()
            .map_or(SettingValue::Other, |t| SettingValue::Text(t.to_owned())),
        rmpv::Value::Boolean(b) => SettingValue::Bool(*b),
        rmpv::Value::Integer(i) => i
            .as_i64()
            .map(i128::from)
            .or_else(|| i.as_u64().map(i128::from))
            .map_or(SettingValue::Other, SettingValue::Integer),
        rmpv::Value::F32(f) => SettingValue::Float(f64::from(*f)),
        rmpv::Value::F64(f) => SettingValue::Float(*f),
        rmpv::Value::Map(entries) if top => {
            let mut out = BTreeMap::new();
            for (k, v) in entries {
                if let Some(k) = k.as_str() {
                    out.insert(k.to_owned(), value_of(v, false));
                }
            }
            SettingValue::Map(out)
        }
        _ => SettingValue::Other,
    }
}

/// Stores setting `key` = `value` (`MessagePack`) and appends a `setting` change-log row for
/// every record that changed (upserts, and tombstones for removed map entries), so devices
/// pull it (`PATCH /me`). Runs in the caller's transaction.
pub async fn put_setting_logged(
    tx: &mut ScopedTx,
    key: &str,
    value: &[u8],
    now: DateTime<Utc>,
) -> Result<(), IndexError> {
    let before = strata_index::repo::settings::get_setting(tx, key).await?;
    strata_index::repo::settings::put_setting(tx, key, value, now).await?;
    let before = before.map(|b| setting_value(&b));
    let after = setting_value(value);
    for change in settings::changes(key, before.as_ref(), Some(&after)) {
        let (op, entity_id) = match &change {
            settings::RecordChange::Upsert(k) => (strata_index::types::ChangeOp::Upsert, k),
            settings::RecordChange::Delete(k) => (strata_index::types::ChangeOp::Delete, k),
        };
        log::append_change(
            tx,
            &log::NewChange {
                entity_type: EntityType::Setting.as_str(),
                entity_id,
                op,
                version: None,
                at: now,
            },
        )
        .await?;
    }
    Ok(())
}

/// The record `record_key` of the user's settings: a setting's own record, or an entry of a
/// map setting (`<setting>.<entry>`).
async fn setting_record(
    tx: &mut ScopedTx,
    record_key: &str,
) -> Result<Option<Record>, Problem> {
    let get = strata_index::repo::settings::get_setting;
    if let Some(bytes) = get(tx, record_key).await.map_err(index_problem)? {
        return Ok(settings::record(record_key, &setting_value(&bytes), record_key)
            .map(Record::Setting));
    }
    let Some((setting, _)) = settings::split_entry_key(record_key) else {
        return Ok(None);
    };
    Ok(get(tx, setting)
        .await
        .map_err(index_problem)?
        .and_then(|bytes| settings::record(setting, &setting_value(&bytes), record_key))
        .map(Record::Setting))
}

fn keep_both_record(kind: &str, a: String, b: String) -> Option<Record> {
    Some(Record::KeepBoth(KeepBoth {
        kind: kind.parse::<DedupeKind>().ok()?,
        a_id: a,
        b_id: b,
    }))
}

fn device_record(id: Uuid, reminders: bool) -> Record {
    Record::DeviceSetting(DeviceSettingRecord {
        device_id: ulid(id),
        key: "reminders_enabled".to_owned(),
        value: reminders.to_string(),
    })
}

const SUGGESTION_COLS: &str = "id, note_id, kind, payload, status, created, updated, decided_at";

/// Up to `n` items of `section` after `after`; returns them with the last key read, or `None`
/// as the key when the section has no more rows.
#[allow(clippy::too_many_lines)] // one arm per section
async fn fetch(
    tx: &mut ScopedTx,
    section: u8,
    after: Option<&str>,
    n: i64,
) -> Result<(Vec<Item>, Option<String>), Problem> {
    let mut items = Vec::new();
    let mut last: Option<String> = None;
    let mut count = 0_i64;
    match section {
        0 => {
            let after: Option<Uuid> = after
                .and_then(|a| Ulid::from_string(a).ok())
                .map(Uuid::from);
            let rows: Vec<(NoteId, String, String)> = sqlx::query_as(
                "SELECT id, path, kind FROM notes WHERE NOT trashed \
                 AND ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT $2",
            )
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (id, path, kind) in rows {
                count += 1;
                last = Some(id.to_string());
                items.push(Item::Note(NoteRow {
                    id,
                    path,
                    kind: kind.parse().unwrap_or(DNoteKind::Note),
                }));
            }
        }
        1 => {
            let a = after.and_then(split_edge);
            let rows: Vec<RelationRow> = sqlx::query_as(
                "SELECT r.src_id, r.type, r.dst_id, r.by, r.confidence, r.reason, r.created \
                 FROM relations r \
                 JOIN notes s ON s.id = r.src_id AND NOT s.trashed \
                 JOIN notes d ON d.id = r.dst_id AND NOT d.trashed \
                 WHERE ($1::uuid IS NULL OR (r.src_id, r.type COLLATE \"C\", r.dst_id) > ($1, $2, $3)) \
                 ORDER BY r.src_id, r.type COLLATE \"C\", r.dst_id LIMIT $4",
            )
            .bind(a.as_ref().map(|x| x.0))
            .bind(a.as_ref().map(|x| x.1.clone()))
            .bind(a.as_ref().map(|x| x.2))
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for r in rows {
                count += 1;
                last = Some(edge_key(r.0, &r.1, r.2));
                if let Some(rec) = relation_record(r) {
                    items.push(Item::Ready(rec));
                }
            }
        }
        2 => {
            let a = after.and_then(split_edge);
            let rows: Vec<(Uuid, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
                "SELECT src_id, type, dst_id, at FROM rejected \
                 WHERE ($1::uuid IS NULL OR (src_id, type COLLATE \"C\", dst_id) > ($1, $2, $3)) \
                 ORDER BY src_id, type COLLATE \"C\", dst_id LIMIT $4",
            )
            .bind(a.as_ref().map(|x| x.0))
            .bind(a.as_ref().map(|x| x.1.clone()))
            .bind(a.as_ref().map(|x| x.2))
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for r in rows {
                count += 1;
                last = Some(edge_key(r.0, &r.1, r.2));
                if let Some(rec) = rejected_record(r) {
                    items.push(Item::Ready(rec));
                }
            }
        }
        3 => {
            let after: Option<Uuid> = after
                .and_then(|a| Ulid::from_string(a).ok())
                .map(Uuid::from);
            let rows: Vec<Suggestion> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {SUGGESTION_COLS} FROM suggestions \
                 WHERE ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT $2"
            )))
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for s in rows {
                count += 1;
                last = Some(s.id.to_string());
                items.push(Item::Ready(suggestion_record(tx, s).await?));
            }
        }
        4 => {
            let after: Option<Uuid> = after
                .and_then(|a| Ulid::from_string(a).ok())
                .map(Uuid::from);
            let rows: Vec<(Uuid, i64)> = sqlx::query_as(
                "SELECT c.note_id, c.cluster_id FROM clusters c \
                 JOIN notes n ON n.id = c.note_id AND NOT n.trashed \
                 WHERE ($1::uuid IS NULL OR c.note_id > $1) ORDER BY c.note_id LIMIT $2",
            )
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (note, cluster) in rows {
                count += 1;
                last = Some(ulid(note).to_string());
                items.push(Item::Ready(Record::ClusterAssignment(
                    ClusterAssignmentRecord {
                        note_id: ulid(note),
                        cluster_id: cluster.to_string(),
                    },
                )));
            }
        }
        5 => {
            let after: Option<i64> = after.and_then(|a| a.parse().ok());
            let rows: Vec<(i64, String)> = sqlx::query_as(
                "SELECT cluster_id, name FROM cluster_names \
                 WHERE ($1::bigint IS NULL OR cluster_id > $1) ORDER BY cluster_id LIMIT $2",
            )
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (cluster, name) in rows {
                count += 1;
                last = Some(cluster.to_string());
                items.push(Item::Ready(Record::ClusterName(ClusterNameRecord {
                    cluster_id: cluster.to_string(),
                    name,
                })));
            }
        }
        6 => {
            let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
                "SELECT key, value FROM settings \
                 WHERE ($1::text IS NULL OR key COLLATE \"C\" > $1) \
                 ORDER BY key COLLATE \"C\" LIMIT $2",
            )
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (key, value) in rows {
                count += 1;
                last = Some(key.clone());
                if key.starts_with("device.") {
                    continue;
                }
                for record in settings::records(&key, &setting_value(&value)).into_values() {
                    items.push(Item::Ready(Record::Setting(record)));
                }
            }
        }
        7 => {
            let after: Option<Uuid> = after
                .and_then(|a| Ulid::from_string(a).ok())
                .map(Uuid::from);
            let rows: Vec<(Uuid, bool)> = sqlx::query_as(
                "SELECT id, reminders_enabled FROM devices \
                 WHERE ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT $2",
            )
            .bind(after)
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (id, reminders) in rows {
                count += 1;
                last = Some(ulid(id).to_string());
                items.push(Item::Ready(device_record(id, reminders)));
            }
        }
        _ => {
            let a: Option<Vec<String>> =
                after.map(|a| a.splitn(3, ':').map(str::to_owned).collect());
            let a = a.filter(|v| v.len() == 3);
            let rows: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT kind, a_id, b_id FROM dedupe_keep_both \
                 WHERE ($1::text IS NULL OR (kind COLLATE \"C\", a_id COLLATE \"C\", b_id COLLATE \"C\") > ($1, $2, $3)) \
                 ORDER BY kind COLLATE \"C\", a_id COLLATE \"C\", b_id COLLATE \"C\" LIMIT $4",
            )
            .bind(a.as_ref().map(|v| v[0].clone()))
            .bind(a.as_ref().map(|v| v[1].clone()))
            .bind(a.as_ref().map(|v| v[2].clone()))
            .bind(n)
            .fetch_all(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            for (kind, x, y) in rows {
                count += 1;
                last = Some(format!("{kind}:{x}:{y}"));
                if let Some(rec) = keep_both_record(&kind, x, y) {
                    items.push(Item::Ready(rec));
                }
            }
        }
    }
    Ok((items, if count < n { None } else { last }))
}

/// The AI summary in a note's sidecar.
async fn summary(vault: &VaultService, scope: &UserScope, id: NoteId) -> Option<String> {
    let dir = vault.vault_dir(scope.user_id());
    let rel = NoteSidecar::path_for(id.as_ulid());
    let bytes = tokio::task::spawn_blocking(move || fsio::read(&dir, &rel))
        .await
        .ok()?
        .ok()??;
    NoteSidecar::from_json(&String::from_utf8_lossy(&bytes))
        .ok()?
        .summary
}

/// The current record of a live note, reading its file (the writer answers when the file
/// moved meanwhile); `None` when the note is gone or trashed.
pub async fn note_record(
    vault: &VaultService,
    scope: &UserScope,
    id: NoteId,
    known: Option<(&str, DNoteKind)>,
) -> Result<Option<Record>, Problem> {
    let from_file = match known {
        Some((path, kind)) => {
            let dir = vault.vault_dir(scope.user_id());
            let rel = path.to_owned();
            let bytes = tokio::task::spawn_blocking(move || fsio::read(&dir, &rel))
                .await
                .map_err(|e| Problem::internal(&e))?
                .map_err(|e| Problem::internal(&e))?;
            bytes.map(|b| (path.to_owned(), kind, b))
        }
        None => None,
    };
    let (path, kind, bytes) = match from_file {
        Some(x) => x,
        None => match vault.note(scope, id).await {
            Ok(v) if !v.trashed => (v.path, v.kind, v.content.into_bytes()),
            Ok(_) | Err(VaultError::NotFound) => return Ok(None),
            Err(e) => return Err(problem(&e)),
        },
    };
    Ok(Some(Record::Note(NoteRecord {
        id: id.as_ulid(),
        path,
        version: Version::of(&bytes),
        content: String::from_utf8_lossy(&bytes).into_owned(),
        kind,
        summary: summary(vault, scope, id).await,
    })))
}

/// One page of `GET /sync/bootstrap`.
pub async fn bootstrap(
    db: &AppDb,
    vault: &VaultService,
    scope: &UserScope,
    cursor: Option<BootstrapCursor>,
    limit: u32,
) -> Result<BootstrapPage, Problem> {
    vault.ready(scope).await.map_err(|e| problem(&e))?;
    let mut tx = db.begin(scope).await.map_err(index_problem)?;
    let pos = log::sync_position(&mut tx).await.map_err(index_problem)?;
    let epoch = u64::try_from(pos.epoch).unwrap_or(0);
    let mut cur = match cursor {
        Some(c) => {
            if c.epoch != epoch {
                return Err(epoch_changed(pos.epoch));
            }
            c
        }
        None => BootstrapCursor {
            epoch,
            seq: u64::try_from(pos.last_seq).unwrap_or(0),
            section: 0,
            after: None,
        },
    };
    let limit = i64::from(limit.max(1));
    let mut items = Vec::new();
    let mut done = false;
    loop {
        if cur.section >= SECTIONS {
            done = true;
            break;
        }
        let want = limit - i64::try_from(items.len()).unwrap_or(limit);
        if want <= 0 {
            break;
        }
        let (got, last) = fetch(&mut tx, cur.section, cur.after.as_deref(), want).await?;
        items.extend(got);
        if let Some(key) = last {
            cur.after = Some(key);
        } else {
            cur.section += 1;
            cur.after = None;
        }
    }
    tx.commit().await.map_err(index_problem)?;
    let mut records = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Item::Ready(r) => records.push(r),
            Item::Note(n) => {
                if let Some(r) = note_record(vault, scope, n.id, Some((&n.path, n.kind))).await? {
                    records.push(r);
                }
            }
        }
    }
    Ok(BootstrapPage {
        epoch,
        seq: cur.seq,
        records,
        next_cursor: (!done).then(|| cur.encode()),
    })
}

fn entity_type(s: &str) -> Option<EntityType> {
    EntityType::ALL.iter().copied().find(|t| t.as_str() == s)
}

/// The current record of one change-log entity; `None` = tombstone.
#[allow(clippy::too_many_lines)] // one arm per entity type
async fn current(tx: &mut ScopedTx, ty: EntityType, id: &str) -> Result<Option<Record>, Problem> {
    Ok(match ty {
        EntityType::Note => None, // read after the transaction
        EntityType::Suggestion => {
            let Ok(sid) = id.parse::<SuggestionId>() else {
                return Ok(None);
            };
            match srepo::get_suggestion(tx, sid)
                .await
                .map_err(index_problem)?
            {
                Some(s) => Some(suggestion_record(tx, s).await?),
                None => None,
            }
        }
        EntityType::Relation => {
            let Some((s, t, d)) = split_edge(id) else {
                return Ok(None);
            };
            let row: Option<RelationRow> = sqlx::query_as(
                "SELECT r.src_id, r.type, r.dst_id, r.by, r.confidence, r.reason, r.created \
                 FROM relations r \
                 JOIN notes s ON s.id = r.src_id AND NOT s.trashed \
                 JOIN notes d ON d.id = r.dst_id AND NOT d.trashed \
                 WHERE r.src_id = $1 AND r.type = $2 AND r.dst_id = $3",
            )
            .bind(s)
            .bind(t)
            .bind(d)
            .fetch_optional(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            row.and_then(relation_record)
        }
        EntityType::Rejected => {
            let Some((s, t, d)) = split_edge(id) else {
                return Ok(None);
            };
            let row: Option<(Uuid, String, Uuid, DateTime<Utc>)> = sqlx::query_as(
                "SELECT src_id, type, dst_id, at FROM rejected \
                 WHERE src_id = $1 AND type = $2 AND dst_id = $3",
            )
            .bind(s)
            .bind(t)
            .bind(d)
            .fetch_optional(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            row.and_then(rejected_record)
        }
        EntityType::KeepBoth => {
            let parts: Vec<&str> = id.splitn(3, ':').collect();
            let [kind, a, b] = parts.as_slice() else {
                return Ok(None);
            };
            let exists: Option<i32> = sqlx::query_scalar(
                "SELECT 1 FROM dedupe_keep_both WHERE kind = $1 AND a_id = $2 AND b_id = $3",
            )
            .bind(kind)
            .bind(a)
            .bind(b)
            .fetch_optional(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            exists.and_then(|_| keep_both_record(kind, (*a).to_owned(), (*b).to_owned()))
        }
        EntityType::DeviceSetting => {
            let Some((device, key)) = id.split_once(':') else {
                return Ok(None);
            };
            let Ok(device) = Ulid::from_string(device) else {
                return Ok(None);
            };
            if key != "reminders_enabled" {
                return Ok(None);
            }
            let row: Option<bool> =
                sqlx::query_scalar("SELECT reminders_enabled FROM devices WHERE id = $1")
                    .bind(Uuid::from(device))
                    .fetch_optional(tx.conn())
                    .await
                    .map_err(sqlx_problem)?;
            row.map(|r| device_record(Uuid::from(device), r))
        }
        EntityType::Setting => setting_record(tx, id).await?,
        EntityType::ClusterAssignment => {
            let Ok(note) = Ulid::from_string(id) else {
                return Ok(None);
            };
            let row: Option<i64> = sqlx::query_scalar(
                "SELECT c.cluster_id FROM clusters c JOIN notes n ON n.id = c.note_id \
                 AND NOT n.trashed WHERE c.note_id = $1",
            )
            .bind(Uuid::from(note))
            .fetch_optional(tx.conn())
            .await
            .map_err(sqlx_problem)?;
            row.map(|c| {
                Record::ClusterAssignment(ClusterAssignmentRecord {
                    note_id: note,
                    cluster_id: c.to_string(),
                })
            })
        }
        EntityType::ClusterName => {
            let Ok(cluster) = id.parse::<i64>() else {
                return Ok(None);
            };
            let row: Option<String> =
                sqlx::query_scalar("SELECT name FROM cluster_names WHERE cluster_id = $1")
                    .bind(cluster)
                    .fetch_optional(tx.conn())
                    .await
                    .map_err(sqlx_problem)?;
            row.map(|name| {
                Record::ClusterName(ClusterNameRecord {
                    cluster_id: id.to_owned(),
                    name,
                })
            })
        }
    })
}

/// One page of `GET /sync/changes`: at most `limit` log rows after `since` in `epoch`.
pub async fn changes(
    db: &AppDb,
    vault: &VaultService,
    scope: &UserScope,
    since: u64,
    epoch: u64,
    limit: u32,
) -> Result<ChangesPage, Problem> {
    vault.ready(scope).await.map_err(|e| problem(&e))?;
    let mut tx = db.begin(scope).await.map_err(index_problem)?;
    let epoch_i = i32::try_from(epoch).unwrap_or(-1);
    let since_i = i64::try_from(since).unwrap_or(i64::MAX);
    let limit = i64::from(limit.max(1));
    let mut rows = match log::changes_since(&mut tx, epoch_i, since_i, limit + 1).await {
        Ok(rows) => rows,
        Err(IndexError::EpochChanged { current }) => return Err(epoch_changed(current)),
        Err(e) => return Err(index_problem(e)),
    };
    let has_more = rows.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    let next_seq = rows
        .last()
        .map_or(since, |r| u64::try_from(r.seq).unwrap_or(since));
    // Keep only the newest row of each entity in this page.
    let mut newest: HashMap<(String, String), i64> = HashMap::new();
    for r in &rows {
        newest.insert((r.entity_type.clone(), r.entity_id.clone()), r.seq);
    }
    let mut kept = BTreeMap::new();
    for r in rows {
        let Some(ty) = entity_type(&r.entity_type) else {
            continue;
        };
        if newest.get(&(r.entity_type.clone(), r.entity_id.clone())) != Some(&r.seq) {
            continue;
        }
        let record = current(&mut tx, ty, &r.entity_id).await?;
        kept.insert(r.seq, (ty, r.entity_id, record));
    }
    tx.commit().await.map_err(index_problem)?;
    let mut out = Vec::with_capacity(kept.len());
    for (seq, (ty, id, record)) in kept {
        let seq = u64::try_from(seq).unwrap_or(0);
        let record = match (ty, record) {
            (EntityType::Note, _) => match id.parse::<NoteId>() {
                Ok(note) => note_record(vault, scope, note, None).await?,
                Err(_) => None,
            },
            (_, r) => r,
        };
        out.push(match record {
            Some(r) => ChangeRecord::upsert(seq, epoch, r),
            None => ChangeRecord::delete(seq, epoch, ty, &id),
        });
    }
    Ok(ChangesPage {
        epoch,
        changes: out,
        next_seq,
        has_more,
    })
}
