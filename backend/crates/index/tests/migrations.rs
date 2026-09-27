//! Migrations: clean apply from empty, idempotent re-run, step-by-step forward upgrades with
//! fixture data, ownership, and idempotent bootstrap.

use pretty_assertions::assert_eq;
use strata_index::{MIGRATOR, bootstrap, migrate, schema_audit};
use strata_testkit::{TestDb, TestUser};

async fn tables(db: &TestDb) -> Vec<String> {
    let mut conn = db.superuser.acquire().await.expect("conn");
    schema_audit::all_tables(&mut conn).await.expect("tables")
}

#[tokio::test]
async fn migrations_apply_cleanly_from_empty_and_rerun_is_a_no_op() {
    let db = TestDb::new_unmigrated().await.expect("db");
    assert_eq!(tables(&db).await, Vec::<String>::new());

    migrate(&db.owner).await.expect("first run");
    let after_first: Vec<(i64, bool, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT version, success, installed_on FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.owner)
            .await
            .expect("history");
    let versions: Vec<i64> = after_first.iter().map(|r| r.0).collect();
    assert_eq!(
        versions,
        vec![20_260_927_000_001, 20_260_927_000_002, 20_260_927_000_003, 20_260_927_000_004, 20_260_927_000_005, 20_260_927_000_006]
    );
    assert!(after_first.iter().all(|r| r.1));
    assert_eq!(tables(&db).await.len(), 39);

    migrate(&db.owner).await.expect("second run");
    let after_second: Vec<(i64, bool, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT version, success, installed_on FROM _sqlx_migrations ORDER BY version")
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
    let expected_new_tables: [&[&str]; 6] = [
        &["_sqlx_migrations", "audit_log", "invites", "job_wakeups", "users"],
        &["devices", "refresh_tokens", "sessions"],
        &["aliases", "blocks", "chunks", "links", "notes", "rejected", "relations", "tags"],
        &[
            "cluster_names", "clusters", "custody_events", "disambiguation_hints", "documents",
            "entities", "entity_aliases", "mentions", "places",
        ],
        &["notification_log", "task_reminders", "tasks"],
        &[
            "ai_decisions", "ai_usage", "change_log", "dedupe_keep_both", "dedupe_keys",
            "idempotency", "jobs", "settings", "suggestion_replies", "suggestions", "sync_epochs",
        ],
    ];
    let mut before = tables(&db).await;
    let mut user = None;
    for (step, migration) in MIGRATOR.iter().enumerate() {
        MIGRATOR.run_to(migration.version, &db.owner).await.expect("step");
        let after = tables(&db).await;
        let mut added: Vec<String> = after.iter().filter(|t| !before.contains(t)).cloned().collect();
        added.sort();
        assert_eq!(added, expected_new_tables[step], "migration {}", migration.version);
        before = after;
        match step {
            // Fixture data written right after the step that creates its table.
            0 => user = Some(TestUser::new("keeper").create(&db).await.expect("user").id),
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
            _ => {}
        }
    }
    let u = user.expect("user");
    let user_row = db.accounts_db.user_by_id(u).await.expect("lookup").expect("user survived");
    assert_eq!(user_row.username, "keeper");
    let mut tx = db.begin(u).await.expect("tx");
    let note = strata_index::repo::notes::get_note_by_path(&mut tx, "notes/Keep.md")
        .await
        .expect("query")
        .expect("note survived");
    assert_eq!((note.title.as_str(), note.word_count, note.trashed), ("Keep", 0, false));
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
    assert_eq!(owners, vec![("strata_owner".to_owned(), "strata".to_owned())]);
}

#[tokio::test]
async fn bootstrap_is_idempotent() {
    let db = TestDb::new().await.expect("db");
    let mut conn = db.superuser.acquire().await.expect("conn");
    for _ in 0..2 {
        bootstrap::ensure_roles(&mut conn, &bootstrap::RolePasswords::default())
            .await
            .expect("roles");
        bootstrap::prepare_database(&mut conn, db.name()).await.expect("database");
    }
    migrate(&db.owner).await.expect("migrate after re-bootstrap");
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

#[tokio::test]
async fn bootstrap_script_is_the_statements_joined() {
    let script = bootstrap::bootstrap_script("strata", &bootstrap::RolePasswords::default())
        .expect("script");
    let expected: String = std::iter::once(
        "-- Strata database bootstrap: run as a superuser connected to the target database.\n".to_owned(),
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
    assert!(script.contains("CREATE SCHEMA IF NOT EXISTS \"strata\" AUTHORIZATION \"strata_owner\";\n"));
}
