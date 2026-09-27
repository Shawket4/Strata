//! The schema-enumeration test (PLAN §5.2, §16.3) and proof that the checker catches mistakes.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use pretty_assertions::assert_eq;
use sqlx::AssertSqlSafe;
use strata_index::schema_audit::{self, GLOBAL_TABLES, Violation};
use strata_testkit::TestDb;

#[tokio::test]
async fn every_non_global_table_is_user_owned_with_forced_rls_and_the_standard_policy() {
    let db = TestDb::new().await.expect("db");
    let mut conn = db.superuser.acquire().await.expect("conn");
    assert_eq!(schema_audit::audit(&mut conn).await.expect("audit"), vec![]);

    let all = schema_audit::all_tables(&mut conn).await.expect("tables");
    let global: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|t| GLOBAL_TABLES.contains(t))
        .collect();
    assert_eq!(global, GLOBAL_TABLES);
    assert_eq!(
        schema_audit::user_owned_tables(&mut conn)
            .await
            .expect("owned")
            .len(),
        38
    );
}

#[tokio::test]
async fn global_tables_have_no_rls_and_no_app_access_beyond_the_allow_list() {
    let db = TestDb::new().await.expect("db");
    let rows: Vec<(String, bool, bool, bool, bool)> = sqlx::query_as(
        "SELECT c.relname::text, c.relrowsecurity, \
                has_table_privilege('strata_app', c.oid, 'SELECT'), \
                has_any_column_privilege('strata_app', c.oid, 'SELECT'), \
                has_table_privilege('strata_accounts', c.oid, 'SELECT') \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'strata' AND c.relname = ANY($1) ORDER BY 1",
    )
    .bind(GLOBAL_TABLES)
    .fetch_all(&db.superuser)
    .await
    .expect("rows");
    assert_eq!(
        rows,
        vec![
            ("_sqlx_migrations".into(), false, false, false, false),
            // ai_usage_global: per-day counters only, updated by the app role.
            ("ai_usage_global".into(), false, true, true, false),
            ("audit_log".into(), false, false, false, true),
            ("invites".into(), false, false, false, true),
            ("job_wakeups".into(), false, true, true, false),
            // users: column-level SELECT on non-secret columns only.
            ("users".into(), false, false, true, true),
        ]
    );
}

#[tokio::test]
async fn audit_reports_each_kind_of_mistake() {
    let db = TestDb::new().await.expect("db");
    for stmt in [
        // No user_id, no RLS at all.
        "CREATE TABLE rogue_plain (id int PRIMARY KEY)",
        // RLS + policy but not forced.
        "CREATE TABLE rogue_unforced (user_id uuid NOT NULL, id int, PRIMARY KEY (user_id, id))",
        "ALTER TABLE rogue_unforced ENABLE ROW LEVEL SECURITY",
        "CREATE POLICY strata_user_isolation ON rogue_unforced USING (user_id = strata_current_user()) WITH CHECK (user_id = strata_current_user())",
        // user_id not first in the primary key, and nullable.
        "CREATE TABLE rogue_pk (id int, user_id uuid, PRIMARY KEY (id), UNIQUE (user_id))",
        "SELECT strata_make_user_owned('rogue_pk')",
        // Extra permissive policy that would leak.
        "CREATE TABLE rogue_policy (user_id uuid NOT NULL, id int, PRIMARY KEY (user_id, id))",
        "SELECT strata_make_user_owned('rogue_policy')",
        "CREATE POLICY leak ON rogue_policy USING (true)",
        // Standard policy with the wrong expression (no WITH CHECK).
        "CREATE TABLE rogue_expr (user_id uuid NOT NULL, id int, PRIMARY KEY (user_id, id))",
        "ALTER TABLE rogue_expr ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE rogue_expr FORCE ROW LEVEL SECURITY",
        "CREATE POLICY strata_user_isolation ON rogue_expr USING (user_id = strata_current_user())",
        // Granted to the accounts role.
        "CREATE TABLE rogue_grant (user_id uuid NOT NULL, id int, PRIMARY KEY (user_id, id))",
        "SELECT strata_make_user_owned('rogue_grant')",
        "GRANT SELECT ON rogue_grant TO strata_accounts, PUBLIC",
    ] {
        sqlx::query(AssertSqlSafe(stmt))
            .execute(&db.owner)
            .await
            .expect(stmt);
    }
    let mut conn = db.superuser.acquire().await.expect("conn");
    let s = |x: &str| x.to_owned();
    let mut expected = vec![
        Violation::ForbiddenPrivilege(s("rogue_grant"), s("public"), s("SELECT")),
        Violation::ForbiddenPrivilege(s("rogue_grant"), s("strata_accounts"), s("SELECT")),
        Violation::MissingStandardPolicy(s("rogue_expr")),
        Violation::MissingStandardPolicy(s("rogue_plain")),
        Violation::MissingUserId(s("rogue_plain")),
        Violation::RlsNotEnabled(s("rogue_plain")),
        Violation::RlsNotForced(s("rogue_plain")),
        Violation::RlsNotForced(s("rogue_unforced")),
        Violation::UnexpectedPolicy(s("rogue_expr"), s("strata_user_isolation")),
        Violation::UnexpectedPolicy(s("rogue_policy"), s("leak")),
        Violation::UserIdNotFirstInPrimaryKey(s("rogue_pk")),
        Violation::UserIdNotFirstInPrimaryKey(s("rogue_plain")),
        Violation::UserIdWrongType(s("rogue_pk")),
    ];
    expected.sort();
    assert_eq!(
        schema_audit::audit(&mut conn).await.expect("audit"),
        expected
    );
}
