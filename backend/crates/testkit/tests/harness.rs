//! The harness itself: fresh databases per test, template reuse, roles, cleanup, fixtures.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use pretty_assertions::assert_eq;
use strata_index::types::{UserRole, UserStatus};
use strata_testkit::{TempDataRoot, TestDb, TestUser, template_name};

#[tokio::test]
async fn each_test_db_is_fresh_migrated_and_dropped_on_cleanup() {
    let a = TestDb::new().await.expect("db a");
    let b = TestDb::new().await.expect("db b");
    assert_ne!(a.name(), b.name());
    assert!(a.name().starts_with("strata_test_"));

    // Migrated: every migration recorded, none failed.
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(&a.owner)
            .await
            .expect("migrations table");
    let expected: Vec<i64> = strata_index::MIGRATOR.iter().map(|m| m.version).collect();
    assert_eq!(applied, expected);

    // Writes to one database are invisible to the other.
    TestUser::new("alice").create(&a).await.expect("alice");
    let in_b: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&b.accounts)
        .await
        .expect("count");
    assert_eq!(in_b, 0);

    let name = a.name().to_owned();
    a.cleanup().await.expect("cleanup");
    let still_there: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&name)
            .fetch_one(&b.superuser)
            .await
            .expect("query");
    assert!(!still_there, "{name} must be dropped");

    // Dropping without cleanup also removes the database.
    let c = TestDb::new().await.expect("db c");
    let c_name = c.name().to_owned();
    drop(c);
    let c_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&c_name)
            .fetch_one(&b.superuser)
            .await
            .expect("query");
    assert!(!c_exists, "{c_name} must be dropped by Drop");
}

#[tokio::test]
async fn template_is_marked_and_connections_disallowed() {
    let db = TestDb::new().await.expect("db");
    let (is_template, allow_conn): (bool, bool) =
        sqlx::query_as("SELECT datistemplate, datallowconn FROM pg_database WHERE datname = $1")
            .bind(template_name().expect("name"))
            .fetch_one(&db.superuser)
            .await
            .expect("template row");
    assert_eq!((is_template, allow_conn), (true, false));
}

#[tokio::test]
async fn roles_have_exactly_the_documented_attributes() {
    let db = TestDb::new().await.expect("db");
    let rows: Vec<(String, bool, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT rolname::text, rolsuper, rolbypassrls, rolcreatedb, rolcreaterole, rolcanlogin \
         FROM pg_roles WHERE rolname LIKE 'strata\\_%' AND rolname IN ('strata_owner', 'strata_app', 'strata_accounts') \
         ORDER BY rolname",
    )
    .fetch_all(&db.superuser)
    .await
    .expect("roles");
    assert_eq!(
        rows,
        vec![
            ("strata_accounts".into(), false, false, false, false, true),
            ("strata_app".into(), false, false, false, false, true),
            ("strata_owner".into(), false, false, false, false, true),
        ]
    );
    // strata_app owns nothing in this database.
    let owned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pg_class WHERE relowner = 'strata_app'::regrole")
            .fetch_one(&db.superuser)
            .await
            .expect("count");
    assert_eq!(owned, 0);
    // Connected identities are what the pools claim.
    for (pool, role) in [
        (&db.owner, "strata_owner"),
        (&db.app, "strata_app"),
        (&db.accounts, "strata_accounts"),
    ] {
        let who: String = sqlx::query_scalar("SELECT current_user::text")
            .fetch_one(pool)
            .await
            .expect("whoami");
        assert_eq!(who, role);
    }
}

#[tokio::test]
async fn test_user_builder_creates_deterministic_rows() {
    let db = TestDb::new().await.expect("db");
    let alice = TestUser::new("Alice").create(&db).await.expect("alice");
    let bob = TestUser::new("bob")
        .admin()
        .pending()
        .display_name("Bob B")
        .create(&db)
        .await
        .expect("bob");
    assert_eq!(alice.id.to_string(), "01M3HBS0G00000000000000001");
    assert_eq!(alice.username_normalized, "alice");
    assert_eq!(alice.status, UserStatus::Active);
    assert_eq!(
        alice.approved_at,
        Some(strata_testkit::default_test_epoch())
    );
    assert_eq!(bob.id.to_string(), "01M3HBS0G00000000000000002");
    assert_eq!(
        (bob.role, bob.status, bob.display_name.as_str()),
        (UserRole::Admin, UserStatus::Pending, "Bob B")
    );
    assert_eq!(bob.approved_at, None);
    assert_eq!(bob.created, strata_testkit::default_test_epoch());
}

#[test]
fn temp_data_root_lays_out_user_vaults() {
    let root = TempDataRoot::new().expect("root");
    let user: strata_common::UserId = "01M3HBS0G00000000000000001".parse().expect("id");
    let vault = root.vault_dir(user).expect("vault");
    assert_eq!(
        vault,
        root.path().join("users/01M3HBS0G00000000000000001/vault")
    );
    assert!(vault.is_dir());
    let path = root.path().to_path_buf();
    drop(root);
    assert!(!path.exists());
}
