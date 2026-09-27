//! Shared helpers for the index integration tests.
#![allow(dead_code, clippy::expect_used)]

use sqlx::AssertSqlSafe;
use strata_common::UserId;
use strata_index::IndexError;
use strata_testkit::TestDb;

/// Seeds exactly one row into every user-owned table for `user`, as `strata_owner` with the
/// user's scope applied (the owner is subject to forced RLS too). The RLS suites assert every
/// user-owned table got a row, so a new table without a seed row fails loudly.
pub async fn seed_every_table(db: &TestDb, user: UserId) {
    let u = user.as_uuid();
    let sql = format!(
        r"
BEGIN;
SELECT set_config('strata.user_id', '{u}', true);
INSERT INTO notes (user_id, id, path, title, kind, created, updated, content_hash) VALUES
  ('{u}', md5('{u}n1')::uuid, 'notes/A.md', 'A', 'note', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', 'sha256:a'),
  ('{u}', md5('{u}n2')::uuid, 'people/P.md', 'P', 'person', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', 'sha256:p'),
  ('{u}', md5('{u}n3')::uuid, 'places/Safe.md', 'Safe', 'place', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', 'sha256:s'),
  ('{u}', md5('{u}n4')::uuid, 'documents/D.md', 'D', 'document', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', 'sha256:d');
INSERT INTO aliases VALUES ('{u}', md5('{u}n1')::uuid, 'alias-a');
INSERT INTO tags VALUES ('{u}', md5('{u}n1')::uuid, 'tag');
INSERT INTO links VALUES ('{u}', md5('{u}n1')::uuid, 0, md5('{u}n2')::uuid, 'P', 'link', NULL, NULL);
INSERT INTO relations VALUES ('{u}', md5('{u}n1')::uuid, md5('{u}n2')::uuid, 'related', 'ai', 0.9, 'r', '2026-09-27T12:00:00Z', NULL);
INSERT INTO rejected VALUES ('{u}', md5('{u}n1')::uuid, md5('{u}n2')::uuid, 'supports', '2026-09-27T12:00:00Z');
INSERT INTO blocks VALUES ('{u}', md5('{u}n1')::uuid, 'b1', '', 'text', 0, 4);
INSERT INTO chunks VALUES ('{u}', md5('{u}c1')::uuid, md5('{u}n1')::uuid, 'b1', 'text', 1, array_fill(0.1::real, ARRAY[384])::vector, 'model');
INSERT INTO entities VALUES
  ('{u}', md5('{u}n2')::uuid, 'person', 'P', NULL, NULL),
  ('{u}', md5('{u}n3')::uuid, 'place', 'Safe', NULL, NULL),
  ('{u}', md5('{u}n4')::uuid, 'document', 'D', NULL, NULL);
INSERT INTO entity_aliases VALUES ('{u}', md5('{u}n2')::uuid, 'P', 'p');
INSERT INTO mentions VALUES ('{u}', md5('{u}n2')::uuid, md5('{u}n1')::uuid, '', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z');
INSERT INTO clusters VALUES ('{u}', md5('{u}n1')::uuid, 1);
INSERT INTO cluster_names VALUES ('{u}', 1, 'C');
INSERT INTO places VALUES ('{u}', md5('{u}n3')::uuid, NULL);
INSERT INTO documents VALUES ('{u}', md5('{u}n4')::uuid, 'contract', 'original', NULL, md5('{u}n3')::uuid, NULL, md5('{u}n2')::uuid, 'stored', NULL);
INSERT INTO custody_events VALUES ('{u}', md5('{u}e1')::uuid, md5('{u}n4')::uuid, 'stored-at', '2026-09-27T12:00:00Z', md5('{u}n3')::uuid, NULL, NULL, 'user', NULL, md5('{u}n1')::uuid, NULL, '2026-09-27T12:00:00Z');
INSERT INTO disambiguation_hints VALUES ('{u}', md5('{u}h1')::uuid, md5('{u}n2')::uuid, 'hint', NULL, '2026-09-27T12:00:00Z');
INSERT INTO tasks (user_id, id, note_id, text, status, line_start, line_end) VALUES ('{u}', 't-1', md5('{u}n1')::uuid, 'do it', 'open', 0, 0);
INSERT INTO task_reminders VALUES ('{u}', 't-1', '2026-09-28T09:00:00Z');
INSERT INTO notification_log VALUES ('{u}', md5('{u}x1')::uuid, 't-1', '2026-09-28T09:00:00Z', md5('{u}d1')::uuid, 'fcm', '2026-09-28T09:00:01Z', 'sent');
INSERT INTO jobs (user_id, id, kind, note_id, status, max_attempts, run_after, created, updated) VALUES
  ('{u}', md5('{u}j1')::uuid, 'embed', md5('{u}n1')::uuid, 'queued', 3, '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z');
INSERT INTO suggestions VALUES ('{u}', md5('{u}s1')::uuid, md5('{u}n1')::uuid, 'filing', '\x80', 'pending', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', NULL);
INSERT INTO suggestion_replies VALUES ('{u}', md5('{u}s1')::uuid, md5('{u}r1')::uuid, 'user', 'no', '2026-09-27T12:00:00Z');
INSERT INTO ai_decisions (user_id, id, kind, target_type, target_id, created) VALUES ('{u}', md5('{u}a1')::uuid, 'relation', 'note', 'x', '2026-09-27T12:00:00Z');
INSERT INTO ai_usage VALUES ('{u}', '2026-09-27', 'claude_cli', 'm', 1, 10, 20, 30);
INSERT INTO settings VALUES ('{u}', 'timezone', '\xa4', '2026-09-27T12:00:00Z');
INSERT INTO sync_epochs VALUES ('{u}', 1, 1, '2026-09-27T12:00:00Z');
INSERT INTO change_log VALUES ('{u}', 1, 1, 'note', 'x', 'upsert', 'v1', '2026-09-27T12:00:00Z');
INSERT INTO idempotency VALUES ('{u}', md5('{u}o1')::uuid, md5('{u}d1')::uuid, '\x01', '2026-09-27T12:00:00Z');
INSERT INTO integrity_warnings VALUES ('{u}', md5('{u}w1')::uuid, 'out_of_band_edit', 'notes/A.md', 'changed outside the API', '2026-09-27T12:00:00Z');
INSERT INTO dedupe_keys VALUES ('{u}', 'note', 'x', 'a', 'a');
INSERT INTO dedupe_keep_both VALUES ('{u}', 'note', 'a', 'b', '2026-09-27T12:00:00Z');
INSERT INTO devices VALUES ('{u}', md5('{u}d1')::uuid, 'phone', 'android', '2026-09-27T12:00:00Z', '2026-09-27T12:00:00Z', 'none', NULL, NULL);
INSERT INTO sessions VALUES ('{u}', md5('{u}se1')::uuid, md5('{u}d1')::uuid, '2026-09-27T12:00:00Z', '2026-10-27T12:00:00Z', NULL, NULL, false);
INSERT INTO refresh_tokens VALUES ('{u}', decode(md5('{u}rt1'), 'hex'), md5('{u}se1')::uuid, '2026-09-27T12:00:00Z', '2026-10-27T12:00:00Z', NULL);
COMMIT;
"
    );
    sqlx::raw_sql(AssertSqlSafe(sql))
        .execute(&db.owner)
        .await
        .expect("seed every user-owned table");
}

/// Rows per table for `user`, counted by the superuser (bypasses RLS).
pub async fn superuser_count(db: &TestDb, table: &str, user: Option<UserId>) -> i64 {
    let sql = match user {
        Some(u) => format!(
            "SELECT count(*) FROM strata.{table} WHERE user_id = '{}'",
            u.as_uuid()
        ),
        None => format!("SELECT count(*) FROM strata.{table}"),
    };
    sqlx::query_scalar(AssertSqlSafe(sql))
        .fetch_one(&db.superuser)
        .await
        .expect("superuser count")
}

/// The user-owned tables, enumerated from the catalog.
pub async fn user_owned_tables(db: &TestDb) -> Vec<String> {
    let mut conn = db.superuser.acquire().await.expect("conn");
    strata_index::schema_audit::user_owned_tables(&mut conn)
        .await
        .expect("enumerate")
}

/// Whether `role` has `privilege` on `table`.
pub async fn has_privilege(db: &TestDb, role: &str, table: &str, privilege: &str) -> bool {
    sqlx::query_scalar("SELECT has_table_privilege($1, 'strata.' || $2, $3)")
        .bind(role)
        .bind(table)
        .bind(privilege)
        .fetch_one(&db.superuser)
        .await
        .expect("privilege check")
}

/// Wraps an sqlx error for the `IndexError` classifiers.
pub fn classify(err: sqlx::Error) -> IndexError {
    IndexError::Db(err)
}
