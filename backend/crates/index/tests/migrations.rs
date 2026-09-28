//! Migrations: clean apply from empty, idempotent re-run, step-by-step forward upgrades with
//! fixture data, ownership, and idempotent bootstrap.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use pretty_assertions::assert_eq;
use strata_index::{MIGRATOR, bootstrap, migrate, schema_audit};
use strata_testkit::TestDb;

async fn tables(db: &TestDb) -> Vec<String> {
    let mut conn = db.superuser.acquire().await.expect("conn");
    schema_audit::all_tables(&mut conn).await.expect("tables")
}

#[tokio::test]
async fn migrations_apply_cleanly_from_empty_and_rerun_is_a_no_op() {
    let db = TestDb::new_unmigrated().await.expect("db");
    assert_eq!(tables(&db).await, Vec::<String>::new());

    migrate(&db.owner).await.expect("first run");
    let after_first: Vec<(i64, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT version, success, installed_on FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&db.owner)
    .await
    .expect("history");
    let versions: Vec<i64> = after_first.iter().map(|r| r.0).collect();
    assert_eq!(
        versions,
        vec![
            20_260_927_000_001,
            20_260_927_000_002,
            20_260_927_000_003,
            20_260_927_000_004,
            20_260_927_000_005,
            20_260_927_000_006,
            20_260_927_000_007,
            20_260_927_000_008,
            20_260_927_000_010,
            20_260_927_000_011,
            20_260_927_000_013,
            20_260_927_000_014,
            20_260_927_000_015,
            20_260_927_000_016,
            20_260_927_000_017,
            20_260_927_000_018
        ]
    );
    assert!(after_first.iter().all(|r| r.1));
    assert_eq!(tables(&db).await.len(), 45);

    migrate(&db.owner).await.expect("second run");
    let after_second: Vec<(i64, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT version, success, installed_on FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&db.owner)
    .await
    .expect("history");
    assert_eq!(after_second, after_first);

    let mut conn = db.superuser.acquire().await.expect("conn");
    assert_eq!(schema_audit::audit(&mut conn).await.expect("audit"), vec![]);
}

#[tokio::test]
async fn each_migration_upgrades_the_previous_schema_and_keeps_data() {
    let db = TestDb::new_unmigrated().await.expect("db");
    let expected_new_tables: [&[&str]; 16] = [
        &[
            "_sqlx_migrations",
            "audit_log",
            "invites",
            "job_wakeups",
            "users",
        ],
        &["devices", "refresh_tokens", "sessions"],
        &[
            "aliases",
            "blocks",
            "chunks",
            "links",
            "notes",
            "rejected",
            "relations",
            "tags",
        ],
        &[
            "cluster_names",
            "clusters",
            "custody_events",
            "disambiguation_hints",
            "documents",
            "entities",
            "entity_aliases",
            "mentions",
            "places",
        ],
        &["notification_log", "task_reminders", "tasks"],
        &[
            "ai_decisions",
            "ai_usage",
            "change_log",
            "dedupe_keep_both",
            "dedupe_keys",
            "idempotency",
            "jobs",
            "settings",
            "suggestion_replies",
            "suggestions",
            "sync_epochs",
        ],
        &["integrity_warnings"],
        &["ai_usage_global"],
        // 010: columns only (devices.reminders_enabled, users.must_change_password).
        &[],
        &["dedupe_vectors", "dedupe_verdicts", "note_vectors"],
        // 013: column only (sync_epochs.vault_head).
        &[],
        // 014: columns only (ai_decisions.rel_type, mention, detail).
        &[],
        // 015: indexes only (dedupe_keys by item, dedupe_keep_both by either side).
        &[],
        // 016: the wakeup trigger function's search_path only.
        &[],
        // 017: column and index only (jobs.provider_failure, jobs_failed).
        &[],
        &["note_threads"],
    ];
    let mut before = tables(&db).await;
    let mut user = None;
    for (step, migration) in MIGRATOR.iter().enumerate() {
        MIGRATOR
            .run_to(migration.version, &db.owner)
            .await
            .expect("step");
        let after = tables(&db).await;
        let mut added: Vec<String> = after
            .iter()
            .filter(|t| !before.contains(t))
            .cloned()
            .collect();
        added.sort();
        assert_eq!(
            added, expected_new_tables[step],
            "migration {}",
            migration.version
        );
        before = after;
        match step {
            // Fixture data written right after the step that creates its table.
            // Raw SQL: `AccountsDb` reads columns that later migrations add.
            0 => {
                let id = strata_common::UserId::generate(db.ids.as_ref());
                sqlx::query(
                    "INSERT INTO users (id, username, username_normalized, display_name, \
                     password_hash, role, status, created, updated, approved_at) VALUES \
                     ($1, 'keeper', 'keeper', 'Keeper', '$argon2id$x', 'member', 'active', \
                     now(), now(), now())",
                )
                .bind(id)
                .execute(&db.accounts)
                .await
                .expect("user");
                user = Some(id);
            }
            2 => {
                let u = user.expect("user from step 0");
                let mut tx = db.begin(u).await.expect("tx");
                sqlx::query(
                    "INSERT INTO notes (user_id, id, path, title, created, updated, content_hash) \
                     VALUES (strata_current_user(), $1, 'notes/Keep.md', 'Keep', now(), now(), 'sha256:k')",
                )
                .bind(u.as_uuid())
                .execute(tx.conn())
                .await
                .expect("note");
                tx.commit().await.expect("commit");
            }
            // 006 creates sync_epochs; 013 adds vault_head to the existing row (NULL).
            5 => {
                let u = user.expect("user from step 0");
                let mut tx = db.begin(u).await.expect("tx");
                sqlx::query(
                    "INSERT INTO sync_epochs (user_id, epoch, last_seq, updated) \
                     VALUES (strata_current_user(), 3, 7, now())",
                )
                .execute(tx.conn())
                .await
                .expect("sync position");
                tx.commit().await.expect("commit");
            }
            _ => {}
        }
    }
    let u = user.expect("user");
    let user_row = db
        .accounts_db
        .user_by_id(u)
        .await
        .expect("lookup")
        .expect("user survived");
    assert_eq!(user_row.username, "keeper");
    let mut tx = db.begin(u).await.expect("tx");
    let note = strata_index::repo::notes::get_note_by_path(&mut tx, "notes/Keep.md")
        .await
        .expect("query")
        .expect("note survived");
    assert_eq!(
        (note.title.as_str(), note.word_count, note.trashed),
        ("Keep", 0, false)
    );
    assert_eq!(
        strata_index::repo::sync::sync_position(&mut tx)
            .await
            .expect("position"),
        strata_index::repo::sync::SyncPosition {
            epoch: 3,
            last_seq: 7
        }
    );
    assert_eq!(
        strata_index::repo::sync::vault_head(&mut tx)
            .await
            .expect("head"),
        None
    );
}

#[tokio::test]
async fn every_table_is_owned_by_strata_owner_and_app_owns_nothing() {
    let db = TestDb::new().await.expect("db");
    let owners: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT pg_get_userbyid(c.relowner)::text, n.nspname::text FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'strata'",
    )
    .fetch_all(&db.superuser)
    .await
    .expect("owners");
    assert_eq!(
        owners,
        vec![("strata_owner".to_owned(), "strata".to_owned())]
    );
}

#[tokio::test]
async fn bootstrap_is_idempotent() {
    let db = TestDb::new().await.expect("db");
    let mut conn = db.superuser.acquire().await.expect("conn");
    for _ in 0..2 {
        bootstrap::ensure_roles(&mut conn, &bootstrap::RolePasswords::default())
            .await
            .expect("roles");
        bootstrap::prepare_database(&mut conn, db.name())
            .await
            .expect("database");
    }
    migrate(&db.owner)
        .await
        .expect("migrate after re-bootstrap");
    assert_eq!(schema_audit::audit(&mut conn).await.expect("audit"), vec![]);
    let err = bootstrap::prepare_database(&mut conn, "some_other_db")
        .await
        .expect_err("wrong database");
    assert_eq!(
        err.to_string(),
        format!(
            "invalid argument: prepare_database(some_other_db) must run while connected to that database, not {}",
            db.name()
        )
    );
}

/// Regression: concurrent `ALTER ROLE` from two processes failed with "tuple concurrently
/// updated" (XX000). `ensure_roles` now serialises on a cluster-wide catalog lock.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ensure_roles_is_safe_under_concurrent_callers() {
    use sqlx::Connection;
    let db = TestDb::new().await.expect("db");
    // Same password the harness uses (or any, under trust auth), so other tests are unaffected;
    // setting it forces a catalog write on every call.
    let pw =
        std::env::var(strata_testkit::ROLE_PASSWORD_ENV).unwrap_or_else(|_| "strata-test".into());
    let passwords = bootstrap::RolePasswords {
        owner: Some(pw.clone()),
        app: Some(pw.clone()),
        accounts: Some(pw),
    };
    let opts = strata_index::pool::connect_options(&strata_testkit::admin_url())
        .expect("admin url")
        .database(db.name());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let opts = opts.clone();
        let passwords = passwords.clone();
        handles.push(tokio::spawn(async move {
            let mut conn = sqlx::PgConnection::connect_with(&opts)
                .await
                .expect("connect");
            bootstrap::ensure_roles(&mut conn, &passwords).await
        }));
    }
    for h in handles {
        h.await
            .expect("task")
            .expect("ensure_roles must not fail under concurrency");
    }
}

#[tokio::test]
async fn bootstrap_script_is_the_statements_joined() {
    let script = bootstrap::bootstrap_script("strata", &bootstrap::RolePasswords::default())
        .expect("script");
    let expected: String = std::iter::once(
        "-- Strata database bootstrap: run as a superuser connected to the target database.\n"
            .to_owned(),
    )
    .chain(
        bootstrap::role_statements(&bootstrap::RolePasswords::default())
            .expect("roles")
            .into_iter()
            .chain(bootstrap::database_statements("strata").expect("db"))
            .map(|s| format!("{s};\n")),
    )
    .collect();
    assert_eq!(script, expected);
    assert!(
        script.contains("CREATE SCHEMA IF NOT EXISTS \"strata\" AUTHORIZATION \"strata_owner\";\n")
    );
}
