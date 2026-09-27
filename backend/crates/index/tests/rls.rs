//! Row-level security at the database level (PLAN §5.2, §16.3), exercised generically over every
//! user-owned table enumerated from the catalog.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

mod common;

use common::{classify, has_privilege, seed_every_table, superuser_count, user_owned_tables};
use pretty_assertions::assert_eq;
use sqlx::{AssertSqlSafe, Connection};
use strata_index::schema_audit::ACCOUNT_BRIDGE_TABLES;
use strata_testkit::{TestDb, TestUser};

struct Fixture {
    db: TestDb,
    a: strata_common::UserId,
    b: strata_common::UserId,
    tables: Vec<String>,
}

async fn fixture() -> Fixture {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("alice").id;
    let b = TestUser::new("bob").create(&db).await.expect("bob").id;
    seed_every_table(&db, a).await;
    seed_every_table(&db, b).await;
    let tables = user_owned_tables(&db).await;
    Fixture { db, a, b, tables }
}

#[tokio::test]
async fn seed_covers_every_user_owned_table_for_both_users() {
    let f = fixture().await;
    assert_eq!(
        f.tables,
        [
            "ai_decisions",
            "ai_usage",
            "aliases",
            "blocks",
            "change_log",
            "chunks",
            "cluster_names",
            "clusters",
            "custody_events",
            "dedupe_keep_both",
            "dedupe_keys",
            "dedupe_vectors",
            "devices",
            "disambiguation_hints",
            "documents",
            "entities",
            "entity_aliases",
            "idempotency",
            "integrity_warnings",
            "jobs",
            "links",
            "mentions",
            "note_vectors",
            "notes",
            "notification_log",
            "places",
            "refresh_tokens",
            "rejected",
            "relations",
            "sessions",
            "settings",
            "suggestion_replies",
            "suggestions",
            "sync_epochs",
            "tags",
            "task_reminders",
            "tasks",
        ]
    );
    for t in &f.tables {
        let a = superuser_count(&f.db, t, Some(f.a)).await;
        let b = superuser_count(&f.db, t, Some(f.b)).await;
        assert!(
            a >= 1 && a == b,
            "{t}: a={a} b={b} (add a seed row for new tables)"
        );
    }
}

#[tokio::test]
async fn unscoped_app_transaction_sees_zero_rows_everywhere() {
    let f = fixture().await;
    let mut tx = f.db.app.begin().await.expect("tx");
    for t in &f.tables {
        let res: Result<i64, _> =
            sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {t}")))
                .fetch_one(&mut *tx)
                .await;
        if has_privilege(&f.db, "strata_app", t, "SELECT").await {
            assert_eq!(res.expect("count"), 0, "{t} leaked rows without a scope");
        } else {
            // refresh_tokens: the app role has no grant at all.
            assert_eq!(t, "refresh_tokens");
            let err = classify(res.expect_err("no grant"));
            assert!(err.is_permission_denied(), "{t}: {err}");
            // The failed statement aborted the transaction; start a new one.
            tx.rollback().await.expect("rollback");
            tx = f.db.app.begin().await.expect("tx");
        }
    }
}

#[tokio::test]
async fn unscoped_owner_also_sees_zero_rows_because_rls_is_forced() {
    let f = fixture().await;
    for t in &f.tables {
        let n: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {t}")))
            .fetch_one(&f.db.owner)
            .await
            .expect("owner count");
        assert_eq!(
            n, 0,
            "{t}: FORCE ROW LEVEL SECURITY must bind the table owner"
        );
    }
}

#[tokio::test]
async fn scoped_transaction_sees_exactly_its_own_rows() {
    let f = fixture().await;
    for (me, other) in [(f.a, f.b), (f.b, f.a)] {
        let mut tx = f.db.begin(me).await.expect("scoped tx");
        for t in f.tables.iter().filter(|t| t.as_str() != "refresh_tokens") {
            let (mine, foreign): (i64, i64) = sqlx::query_as(AssertSqlSafe(format!(
                "SELECT count(*) FILTER (WHERE user_id = $1), count(*) FILTER (WHERE user_id <> $1) FROM {t}"
            )))
            .bind(me)
            .fetch_one(tx.conn())
            .await
            .expect("count");
            assert_eq!(mine, superuser_count(&f.db, t, Some(me)).await, "{t}");
            assert_eq!(foreign, 0, "{t} leaked {other}'s rows");
        }
        tx.commit().await.expect("commit");
    }
}

#[tokio::test]
async fn inserting_a_row_for_another_user_fails_the_with_check() {
    let f = fixture().await;
    for t in &f.tables {
        let mut tx = f.db.begin(f.a).await.expect("scoped tx");
        // Copy one of A's rows, swapping in B's user_id.
        let res = sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO {t} SELECT (jsonb_populate_record(NULL::{t}, to_jsonb(x) || jsonb_build_object('user_id', $1::text))).* \
             FROM {t} x LIMIT 1"
        )))
        .bind(f.b.as_uuid().to_string())
        .execute(tx.conn())
        .await;
        let err = classify(res.expect_err("must fail"));
        assert_eq!(err.sqlstate().as_deref(), Some("42501"), "{t}: {err}");
        if has_privilege(&f.db, "strata_app", t, "INSERT").await {
            assert!(
                err.is_rls_violation(),
                "{t}: expected WITH CHECK violation, got {err}"
            );
        } else {
            assert!(err.is_permission_denied(), "{t}: {err}");
        }
    }
    // Nothing was written for B.
    for t in &f.tables {
        assert_eq!(
            superuser_count(&f.db, t, Some(f.b)).await,
            superuser_count(&f.db, t, Some(f.a)).await,
            "{t}"
        );
    }
}

#[tokio::test]
async fn updating_a_row_to_another_user_fails_the_with_check() {
    let f = fixture().await;
    let mut checked = 0;
    for t in &f.tables {
        if !has_privilege(&f.db, "strata_app", t, "UPDATE").await {
            continue;
        }
        let mut tx = f.db.begin(f.a).await.expect("scoped tx");
        let res = sqlx::query(AssertSqlSafe(format!("UPDATE {t} SET user_id = $1")))
            .bind(f.b)
            .execute(tx.conn())
            .await;
        let err = classify(res.expect_err("must fail"));
        assert_eq!(err.sqlstate().as_deref(), Some("42501"), "{t}");
        assert!(err.is_rls_violation(), "{t}: {err}");
        checked += 1;
    }
    // Every user-owned table except refresh_tokens is updatable by the app role.
    assert_eq!(checked, f.tables.len() - 1);
}

#[tokio::test]
async fn foreign_rows_cannot_be_updated_or_deleted_even_by_explicit_id() {
    let f = fixture().await;
    let mut tx = f.db.begin(f.a).await.expect("scoped tx");
    let updated = sqlx::query("UPDATE notes SET title = 'pwned' WHERE user_id = $1")
        .bind(f.b)
        .execute(tx.conn())
        .await
        .expect("update");
    assert_eq!(updated.rows_affected(), 0);
    let deleted = sqlx::query("DELETE FROM notes WHERE user_id = $1")
        .bind(f.b)
        .execute(tx.conn())
        .await
        .expect("delete");
    assert_eq!(deleted.rows_affected(), 0);
    tx.commit().await.expect("commit");
    assert_eq!(superuser_count(&f.db, "notes", Some(f.b)).await, 4);
    let title: String = sqlx::query_scalar(
        "SELECT title FROM strata.notes WHERE user_id = $1 AND path = 'notes/A.md'",
    )
    .bind(f.b)
    .fetch_one(&f.db.superuser)
    .await
    .expect("title");
    assert_eq!(title, "A");
}

#[tokio::test]
async fn accounts_role_is_denied_on_every_user_owned_table() {
    let f = fixture().await;
    let mut denied_tables = 0;
    for t in &f.tables {
        if ACCOUNT_BRIDGE_TABLES.contains(&t.as_str()) {
            // Bridge tables: the accounts role reaches every user's rows (login/refresh).
            let n: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {t}")))
                .fetch_one(&f.db.accounts)
                .await
                .expect("bridge count");
            assert_eq!(n, 2, "{t}");
            continue;
        }
        for stmt in [
            format!("SELECT * FROM {t}"),
            format!("INSERT INTO {t} (user_id) VALUES (NULL)"),
            format!("UPDATE {t} SET user_id = user_id"),
            format!("DELETE FROM {t}"),
            format!("TRUNCATE {t}"),
        ] {
            let err = classify(
                sqlx::query(AssertSqlSafe(stmt.clone()))
                    .execute(&f.db.accounts)
                    .await
                    .expect_err("must be denied"),
            );
            assert_eq!(err.sqlstate().as_deref(), Some("42501"), "{stmt}");
            assert!(err.is_permission_denied(), "{stmt}: {err}");
        }
        denied_tables += 1;
    }
    assert_eq!(denied_tables, f.tables.len() - ACCOUNT_BRIDGE_TABLES.len());
}

#[tokio::test]
async fn pooled_connection_reuse_never_leaks_a_scope() {
    let db = TestDb::with_pool_size(1).await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("alice").id;
    seed_every_table(&db, a).await;
    let pid = |pool: sqlx::PgPool| async move {
        sqlx::query_scalar::<_, i32>("SELECT pg_backend_pid()")
            .fetch_one(&pool)
            .await
            .expect("pid")
    };
    let first_pid = pid(db.app.clone()).await;

    // 1. Committed scoped transaction.
    let mut tx = db.begin(a).await.expect("tx");
    let inside: i64 = sqlx::query_scalar("SELECT count(*) FROM notes")
        .fetch_one(tx.conn())
        .await
        .expect("count");
    assert_eq!(inside, 4);
    tx.commit().await.expect("commit");
    // 2. Rolled-back scoped transaction.
    let tx = db.begin(a).await.expect("tx");
    tx.rollback().await.expect("rollback");
    // 3. Scoped transaction dropped without commit.
    drop(db.begin(a).await.expect("tx"));

    // Same physical connection (pool of one), and it carries no scope.
    assert_eq!(pid(db.app.clone()).await, first_pid);
    let (setting, current, notes): (String, Option<uuid::Uuid>, i64) = sqlx::query_as(
        "SELECT current_setting('strata.user_id', true), strata_current_user(), (SELECT count(*) FROM notes)",
    )
    .fetch_one(&db.app)
    .await
    .expect("probe");
    assert_eq!((setting.as_str(), current, notes), ("", None, 0));
}

#[tokio::test]
async fn scope_is_applied_per_transaction_and_reported() {
    let f = fixture().await;
    let mut tx = f.db.begin(f.a).await.expect("tx");
    assert_eq!(tx.user_id(), f.a);
    let (setting, current): (String, uuid::Uuid) =
        sqlx::query_as("SELECT current_setting('strata.user_id'), strata_current_user()")
            .fetch_one(tx.conn())
            .await
            .expect("probe");
    assert_eq!(setting, f.a.as_uuid().to_string());
    assert_eq!(current, f.a.as_uuid());
    tx.commit().await.expect("commit");
    assert_eq!(f.db.scope(f.b).user_id(), f.b);
}

#[tokio::test]
async fn app_role_reads_only_non_secret_user_columns() {
    let f = fixture().await;
    let names: Vec<String> = sqlx::query_scalar("SELECT username FROM users ORDER BY username")
        .fetch_all(&f.db.app)
        .await
        .expect("public columns");
    assert_eq!(names, vec!["alice", "bob"]);
    let err = classify(
        sqlx::query("SELECT password_hash FROM users")
            .execute(&f.db.app)
            .await
            .expect_err("secret column"),
    );
    assert!(err.is_permission_denied(), "{err}");
    let err = classify(
        sqlx::query("UPDATE users SET status = 'active'")
            .execute(&f.db.app)
            .await
            .expect_err("no write"),
    );
    assert!(err.is_permission_denied(), "{err}");
    let err = classify(
        sqlx::query("SELECT * FROM invites")
            .execute(&f.db.app)
            .await
            .expect_err("no invites"),
    );
    assert!(err.is_permission_denied(), "{err}");
}

#[tokio::test]
async fn a_raw_session_level_scope_is_never_needed_and_setting_garbage_fails_closed() {
    let f = fixture().await;
    let mut conn = f.db.app.acquire().await.expect("conn");
    let mut tx = conn.begin().await.expect("tx");
    sqlx::query("SELECT set_config('strata.user_id', 'not-a-uuid', true)")
        .execute(&mut *tx)
        .await
        .expect("set");
    let err = classify(
        sqlx::query("SELECT count(*) FROM notes")
            .execute(&mut *tx)
            .await
            .expect_err("invalid uuid must error, not widen"),
    );
    assert_eq!(err.sqlstate().as_deref(), Some("22P02"));
}

#[tokio::test]
async fn purging_a_user_removes_every_row_of_that_user_and_nothing_else() {
    let f = fixture().await;
    let now = strata_testkit::default_test_epoch();
    let admin = TestUser::new("root")
        .admin()
        .create(&f.db)
        .await
        .expect("admin")
        .id;
    f.db.accounts_db
        .schedule_deletion(f.a, admin, now, now)
        .await
        .expect("schedule")
        .expect("was active");
    assert!(f.db.accounts_db.purge_user(f.a).await.expect("purge"));
    for t in &f.tables {
        assert_eq!(
            superuser_count(&f.db, t, Some(f.a)).await,
            0,
            "{t} kept purged rows"
        );
        assert!(
            superuser_count(&f.db, t, Some(f.b)).await >= 1,
            "{t} lost the other user's rows"
        );
    }
    assert_eq!(
        f.db.accounts_db.user_by_id(f.a).await.expect("lookup"),
        None
    );
}
