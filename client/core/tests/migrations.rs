//! Store migrations (PLAN §7.4 "each migration tested from the previous schema with fixture
//! data", §12.2): exact schema, data preserved across versions, forward-only.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use pretty_assertions::assert_eq;
use rusqlite::Connection;
use strata_core::CoreError;
use strata_core::store::migrations::{self, ACCOUNT, REGISTRY};

fn tables(conn: &Connection) -> Vec<String> {
    let mut st = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table'
               AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'notes_fts_%' ORDER BY name",
        )
        .expect("prepare");
    st.query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

fn columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut st = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("prepare");
    st.query_map([], |r| r.get(1))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

#[test]
fn fresh_account_database_has_the_full_schema() {
    let conn = Connection::open_in_memory().expect("db");
    assert_eq!(migrations::migrate(&conn, ACCOUNT).expect("migrates"), 3);
    assert_eq!(migrations::version(&conn).expect("version"), 3);
    assert_eq!(
        tables(&conn),
        [
            "account",
            "acknowledged_suggestions",
            "auth_tokens",
            "bootstrap_seen",
            "cluster_names",
            "clusters",
            "conflicts",
            "custody_events",
            "device_settings",
            "documents",
            "duplicates",
            "entities",
            "entity_aliases",
            "graph_positions",
            "inbox",
            "keep_both",
            "links",
            "notes",
            "notes_fts",
            "outbox",
            "pinned_notes",
            "places",
            "rejected",
            "rejections",
            "relation_meta",
            "relations",
            "remote_cache",
            "scheduled_notifications",
            "suggestions",
            "sync_log",
            "sync_state",
            "tags",
            "task_reminders",
            "tasks",
            "user_settings",
        ]
    );
    assert_eq!(
        columns(&conn, "outbox"),
        [
            "op_id",
            "ord",
            "kind",
            "entity_id",
            "local_entity",
            "base_version",
            "payload",
            "status",
            "attempts",
            "last_error",
            "created",
            "base_content"
        ]
    );
    assert_eq!(
        columns(&conn, "notes"),
        [
            "id",
            "path",
            "title",
            "kind",
            "content",
            "frontmatter",
            "created",
            "updated",
            "deleted",
            "base_exists",
            "base_path",
            "base_content",
            "base_version",
            "local_updated_at",
            "summary"
        ]
    );
    assert_eq!(
        columns(&conn, "sync_state"),
        [
            "singleton",
            "epoch",
            "cursor_seq",
            "bootstrap_cursor",
            "bootstrap_complete",
            "bootstrap_pages",
            "last_pull_at",
            "last_push_at",
            "consecutive_failures",
            "last_error",
            "events_seq",
            "paused"
        ]
    );
    // The sync state row exists from the start.
    let row: (Option<i64>, i64, bool) = conn
        .query_row(
            "SELECT epoch, cursor_seq, bootstrap_complete FROM sync_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("row");
    assert_eq!(row, (None, 0, false));
}

#[test]
fn fts5_is_available_and_matches_normalised_text() {
    let conn = Connection::open_in_memory().expect("db");
    migrations::migrate(&conn, ACCOUNT).expect("migrates");
    conn.execute(
        "INSERT INTO notes_fts (note_id, title, body, tags) VALUES ('n1', ?1, ?2, '')",
        [
            text_normalize::normalize_for_search("تجارب التسعير"),
            text_normalize::normalize_for_search("الأسعار"),
        ],
    )
    .expect("insert");
    let hit: String = conn
        .query_row(
            "SELECT note_id FROM notes_fts WHERE notes_fts MATCH ?1",
            [format!(
                "\"{}\"*",
                text_normalize::normalize_for_search("الاسعار")
            )],
            |r| r.get(0),
        )
        .expect("match");
    assert_eq!(hit, "n1");
}

#[test]
fn v1_to_v2_keeps_every_row_and_adds_the_notification_table() {
    let conn = Connection::open_in_memory().expect("db");
    assert_eq!(migrations::migrate_to(&conn, ACCOUNT, 1).expect("v1"), 1);
    // Fixture data at v1.
    conn.execute_batch(
        "INSERT INTO notes (id, path, title, kind, content, frontmatter, local_updated_at)
           VALUES ('n1', 'notes/a.md', 'a', 'note', 'body', x'90', '2026-09-27T10:00:00+00:00');
         INSERT INTO outbox (op_id, ord, kind, entity_id, local_entity, payload, status, created)
           VALUES ('op1', 1, 'note.update', 'n1', 'note:n1', x'80', 'pending', 'now');
         INSERT INTO tasks (id, note_id, line_no, line, description, status, priority)
           VALUES ('t-1', 'n1', 0, '- [ ] x ^t-1', 'x', 'open', 'normal');
         INSERT INTO device_settings (key, value) VALUES ('reminders_enabled', 'false');
         UPDATE sync_state SET epoch = 3, cursor_seq = 42, bootstrap_complete = 1;",
    )
    .expect("fixture");
    assert!(!tables(&conn).contains(&"scheduled_notifications".to_owned()));

    assert_eq!(migrations::migrate_to(&conn, ACCOUNT, 2).expect("v2"), 2);

    assert!(tables(&conn).contains(&"scheduled_notifications".to_owned()));
    let note: (String, String) = conn
        .query_row("SELECT path, content FROM notes WHERE id = 'n1'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("note kept");
    assert_eq!(note, ("notes/a.md".to_owned(), "body".to_owned()));
    let op: (String, String) = conn
        .query_row("SELECT op_id, status FROM outbox", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("op kept");
    assert_eq!(op, ("op1".to_owned(), "pending".to_owned()));
    let state: (i64, i64, bool) = conn
        .query_row(
            "SELECT epoch, cursor_seq, bootstrap_complete FROM sync_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("state kept");
    assert_eq!(state, (3, 42, true));
    let setting: String = conn
        .query_row(
            "SELECT value FROM device_settings WHERE key = 'reminders_enabled'",
            [],
            |r| r.get(0),
        )
        .expect("setting kept");
    assert_eq!(setting, "false");
}

#[test]
fn v2_to_v3_keeps_every_row_and_adds_server_columns() {
    let conn = Connection::open_in_memory().expect("db");
    assert_eq!(migrations::migrate_to(&conn, ACCOUNT, 2).expect("v2"), 2);
    conn.execute_batch(
        "INSERT INTO notes (id, path, title, kind, content, frontmatter, local_updated_at)
           VALUES ('n1', 'notes/a.md', 'a', 'note', 'body', x'90', '2026-09-27T10:00:00+00:00');
         INSERT INTO outbox (op_id, ord, kind, entity_id, local_entity, payload, status, created)
           VALUES ('op1', 1, 'note.update', 'n1', 'note:n1', x'80', 'pending', 'now');
         INSERT INTO suggestions (id, note_id, kind, payload, status, base_status, created)
           VALUES ('s1', 'n1', 'filing', x'80', 'pending', 'pending', 'now');
         INSERT INTO relation_meta (src_id, dst_id, rel_type, by) VALUES ('n1', 'n2', 'related', 'ai');
         UPDATE sync_state SET epoch = 3, cursor_seq = 42, bootstrap_complete = 1;",
    )
    .expect("fixture");
    assert_eq!(migrations::migrate(&conn, ACCOUNT).expect("v3"), 3);
    let note: (String, Option<String>) = conn
        .query_row(
            "SELECT content, summary FROM notes WHERE id = 'n1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("note kept");
    assert_eq!(note, ("body".to_owned(), None));
    let op: (String, Option<String>) = conn
        .query_row("SELECT status, base_content FROM outbox", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("op kept");
    assert_eq!(op, ("pending".to_owned(), None));
    let suggestion: (String, Option<Vec<u8>>) = conn
        .query_row("SELECT status, replies FROM suggestions", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("suggestion kept");
    assert_eq!(suggestion, ("pending".to_owned(), None));
    let rel: (String, Option<String>) = conn
        .query_row("SELECT by, created FROM relation_meta", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("relation kept");
    assert_eq!(rel, ("ai".to_owned(), None));
    let state: (i64, i64, Option<i64>, bool) = conn
        .query_row(
            "SELECT epoch, cursor_seq, events_seq, paused FROM sync_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("state kept");
    assert_eq!(state, (3, 42, None, false));
}

#[test]
fn migrating_twice_is_a_no_op_and_newer_databases_are_refused() {
    let conn = Connection::open_in_memory().expect("db");
    migrations::migrate(&conn, ACCOUNT).expect("first");
    let before = tables(&conn);
    assert_eq!(migrations::migrate(&conn, ACCOUNT).expect("second"), 3);
    assert_eq!(tables(&conn), before);

    conn.pragma_update(None, "user_version", 9).expect("bump");
    assert_eq!(
        migrations::migrate(&conn, ACCOUNT),
        Err(CoreError::Storage(
            "database schema v9 is newer than this app (v3)".into()
        ))
    );
}

#[test]
fn registry_schema() {
    let conn = Connection::open_in_memory().expect("db");
    assert_eq!(migrations::migrate(&conn, REGISTRY).expect("migrates"), 1);
    assert_eq!(tables(&conn), ["accounts", "device"]);
    assert_eq!(
        columns(&conn, "accounts"),
        [
            "user_id",
            "username",
            "display_name",
            "server_url",
            "last_active_at",
            "active"
        ]
    );
}

#[test]
fn a_failing_migration_leaves_the_previous_version() {
    const BROKEN: &[migrations::Migration] = &[
        migrations::Migration {
            version: 1,
            name: "one",
            sql: "CREATE TABLE a (x INTEGER);",
        },
        migrations::Migration {
            version: 2,
            name: "two",
            sql: "CREATE TABLE b (x INTEGER); CREATE TABLE a (x INTEGER);",
        },
    ];
    let conn = Connection::open_in_memory().expect("db");
    let err = migrations::migrate(&conn, BROKEN).expect_err("fails");
    assert_eq!(
        err,
        CoreError::Storage(
            "migration two failed: table a already exists in  CREATE TABLE a (x INTEGER); at offset 14"
                .into()
        )
    );
    assert_eq!(migrations::version(&conn).expect("version"), 1);
    assert_eq!(tables(&conn), ["a"]);
}
