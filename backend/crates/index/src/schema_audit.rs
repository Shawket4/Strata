//! Schema enumeration check (PLAN §5.2): every table in the `strata` schema that is not on the
//! explicit global allow-list must be user-owned — `user_id uuid NOT NULL` first in the primary
//! key, RLS enabled and forced, exactly the standard policy — and unreachable by
//! `strata_accounts` and `PUBLIC`. Account-bridge tables additionally carry the one
//! `strata_accounts_access` policy.

use sqlx::PgConnection;

use crate::bootstrap::SCHEMA;
use crate::error::Result;

/// Tables without per-user RLS. Anything else must pass the user-owned checks.
pub const GLOBAL_TABLES: &[&str] = &["_sqlx_migrations", "audit_log", "invites", "job_wakeups", "users"];

/// User-owned tables that `strata_accounts` may also reach (login, refresh, revocation).
pub const ACCOUNT_BRIDGE_TABLES: &[&str] = &["devices", "refresh_tokens", "sessions"];

/// Name of the standard per-user policy.
pub const STANDARD_POLICY: &str = "strata_user_isolation";
/// Name of the extra policy on account-bridge tables.
pub const ACCOUNTS_POLICY: &str = "strata_accounts_access";
/// The standard policy's `USING` and `WITH CHECK` expression, as `pg_get_expr` prints it.
pub const STANDARD_EXPR: &str = "(user_id = strata_current_user())";

/// A failed check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// No `user_id` column.
    MissingUserId(String),
    /// `user_id` is not `uuid NOT NULL`.
    UserIdWrongType(String),
    /// No primary key, or `user_id` is not its first column.
    UserIdNotFirstInPrimaryKey(String),
    /// `ENABLE ROW LEVEL SECURITY` missing.
    RlsNotEnabled(String),
    /// `FORCE ROW LEVEL SECURITY` missing.
    RlsNotForced(String),
    /// The standard policy is missing or differs.
    MissingStandardPolicy(String),
    /// A policy other than the allowed ones exists: `(table, policy)`.
    UnexpectedPolicy(String, String),
    /// A role that must have no access has a privilege: `(table, role, privilege)`.
    ForbiddenPrivilege(String, String, String),
}

#[derive(sqlx::FromRow)]
struct TableRow {
    oid: sqlx::postgres::types::Oid,
    relname: String,
    relrowsecurity: bool,
    relforcerowsecurity: bool,
}

#[derive(sqlx::FromRow)]
struct PolicyRow {
    polname: String,
    cmd: String,
    permissive: bool,
    roles: Vec<String>,
    qual: Option<String>,
    with_check: Option<String>,
}

/// Names of all tables in the Strata schema, sorted.
pub async fn all_tables(conn: &mut PgConnection) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_catalog.pg_class c \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') ORDER BY c.relname",
    )
    .bind(SCHEMA)
    .fetch_all(conn)
    .await?)
}

/// Tables that must be user-owned (everything not in [`GLOBAL_TABLES`]), sorted.
pub async fn user_owned_tables(conn: &mut PgConnection) -> Result<Vec<String>> {
    Ok(all_tables(conn)
        .await?
        .into_iter()
        .filter(|t| !GLOBAL_TABLES.contains(&t.as_str()))
        .collect())
}

/// Runs every check; an empty result means the schema is compliant. Results are sorted.
pub async fn audit(conn: &mut PgConnection) -> Result<Vec<Violation>> {
    let tables: Vec<TableRow> = sqlx::query_as(
        "SELECT c.oid, c.relname::text AS relname, c.relrowsecurity, c.relforcerowsecurity \
         FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') ORDER BY c.relname",
    )
    .bind(SCHEMA)
    .fetch_all(&mut *conn)
    .await?;

    let mut violations = Vec::new();
    for t in tables {
        if GLOBAL_TABLES.contains(&t.relname.as_str()) {
            continue;
        }
        let bridge = ACCOUNT_BRIDGE_TABLES.contains(&t.relname.as_str());
        check_user_id(conn, &t, &mut violations).await?;
        if !t.relrowsecurity {
            violations.push(Violation::RlsNotEnabled(t.relname.clone()));
        }
        if !t.relforcerowsecurity {
            violations.push(Violation::RlsNotForced(t.relname.clone()));
        }
        check_policies(conn, &t, bridge, &mut violations).await?;
        check_privileges(conn, &t, bridge, &mut violations).await?;
    }
    violations.sort();
    Ok(violations)
}

async fn check_user_id(
    conn: &mut PgConnection,
    t: &TableRow,
    violations: &mut Vec<Violation>,
) -> Result<()> {
    let user_id: Option<(i16, bool, String)> = sqlx::query_as(
        "SELECT attnum, attnotnull, pg_catalog.format_type(atttypid, atttypmod) \
         FROM pg_catalog.pg_attribute WHERE attrelid = $1 AND attname = 'user_id' AND NOT attisdropped",
    )
    .bind(t.oid)
    .fetch_optional(&mut *conn)
    .await?;
    match &user_id {
        None => violations.push(Violation::MissingUserId(t.relname.clone())),
        Some((_, notnull, ty)) if !notnull || ty != "uuid" => {
            violations.push(Violation::UserIdWrongType(t.relname.clone()));
        }
        Some(_) => {}
    }
    let first_pk_col: Option<i16> = sqlx::query_scalar(
        "SELECT conkey[1] FROM pg_catalog.pg_constraint WHERE conrelid = $1 AND contype = 'p'",
    )
    .bind(t.oid)
    .fetch_optional(&mut *conn)
    .await?;
    if first_pk_col.is_none() || first_pk_col != user_id.as_ref().map(|u| u.0) {
        violations.push(Violation::UserIdNotFirstInPrimaryKey(t.relname.clone()));
    }
    Ok(())
}

async fn check_policies(
    conn: &mut PgConnection,
    t: &TableRow,
    bridge: bool,
    violations: &mut Vec<Violation>,
) -> Result<()> {
    let policies: Vec<PolicyRow> = sqlx::query_as(
        "SELECT polname::text AS polname, polcmd::text AS cmd, polpermissive AS permissive, \
                ARRAY(SELECT CASE WHEN r = 0 THEN 'public' ELSE r::regrole::text END \
                      FROM unnest(polroles) r ORDER BY 1) AS roles, \
                replace(pg_catalog.pg_get_expr(polqual, polrelid), 'strata.', '') AS qual, \
                replace(pg_catalog.pg_get_expr(polwithcheck, polrelid), 'strata.', '') AS with_check \
         FROM pg_catalog.pg_policy WHERE polrelid = $1 ORDER BY polname",
    )
    .bind(t.oid)
    .fetch_all(&mut *conn)
    .await?;
    let is_standard = |p: &PolicyRow| {
        p.polname == STANDARD_POLICY
            && p.cmd == "*"
            && p.permissive
            && p.roles == ["public"]
            && p.qual.as_deref() == Some(STANDARD_EXPR)
            && p.with_check.as_deref() == Some(STANDARD_EXPR)
    };
    let is_accounts = |p: &PolicyRow| {
        p.polname == ACCOUNTS_POLICY
            && p.cmd == "*"
            && p.permissive
            && p.roles == ["strata_accounts"]
            && p.qual.as_deref() == Some("true")
            && p.with_check.as_deref() == Some("true")
    };
    if !policies.iter().any(is_standard) {
        violations.push(Violation::MissingStandardPolicy(t.relname.clone()));
    }
    for p in &policies {
        if !(is_standard(p) || (bridge && is_accounts(p))) {
            violations.push(Violation::UnexpectedPolicy(t.relname.clone(), p.polname.clone()));
        }
    }
    Ok(())
}

async fn check_privileges(
    conn: &mut PgConnection,
    t: &TableRow,
    bridge: bool,
    violations: &mut Vec<Violation>,
) -> Result<()> {
    let mut forbidden = vec!["public"];
    if !bridge {
        forbidden.push("strata_accounts");
    }
    for role in forbidden {
        for privilege in ["SELECT", "INSERT", "UPDATE", "DELETE", "TRUNCATE", "REFERENCES", "TRIGGER"] {
            let has: bool = sqlx::query_scalar(
                "SELECT pg_catalog.has_table_privilege($1, $2, $3) \
                     OR ($3 IN ('SELECT', 'INSERT', 'UPDATE', 'REFERENCES') \
                         AND pg_catalog.has_any_column_privilege($1, $2, $3))",
            )
            .bind(role)
            .bind(t.oid)
            .bind(privilege)
            .fetch_one(&mut *conn)
            .await?;
            if has {
                violations.push(Violation::ForbiddenPrivilege(
                    t.relname.clone(),
                    role.to_owned(),
                    privilege.to_owned(),
                ));
            }
        }
    }
    Ok(())
}
