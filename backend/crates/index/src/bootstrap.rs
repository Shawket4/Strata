//! Cluster- and database-level setup that must run as a superuser before migrations
//! (PLAN §5.2 roles, §14 database). `stratad` setup runs [`ensure_roles`] then
//! [`prepare_database`]; operators who prefer psql can print [`bootstrap_script`].
//!
//! Everything here is idempotent: re-running it converges roles and grants to the same state.

use sqlx::{AssertSqlSafe, PgConnection};

use crate::error::{IndexError, Result};

/// Owns the schema and runs migrations only.
pub const OWNER_ROLE: &str = "strata_owner";
/// All request and job work: `NOSUPERUSER`, `NOBYPASSRLS`, owns nothing.
pub const APP_ROLE: &str = "strata_app";
/// Account endpoints only: global tables and account-bridge tables, no vault data.
pub const ACCOUNTS_ROLE: &str = "strata_accounts";
/// The schema holding every Strata table (owned by [`OWNER_ROLE`]).
pub const SCHEMA: &str = "strata";
/// `search_path` every Strata role uses (`public` holds the `vector` and `pg_trgm` extensions).
pub const SEARCH_PATH: &str = "strata, public";

/// Optional login passwords for the three roles (`None` leaves the password unchanged, e.g.
/// with peer/trust authentication).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolePasswords {
    /// Password for [`OWNER_ROLE`].
    pub owner: Option<String>,
    /// Password for [`APP_ROLE`].
    pub app: Option<String>,
    /// Password for [`ACCOUNTS_ROLE`].
    pub accounts: Option<String>,
}

/// Quotes an SQL identifier (`"…"`, doubling embedded quotes). Rejects empty names and NUL.
pub fn quote_ident(name: &str) -> Result<String> {
    if name.is_empty() || name.contains('\0') || name.len() > 63 {
        return Err(IndexError::InvalidIdentifier(name.to_owned()));
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

/// Quotes an SQL string literal (`'…'`, doubling embedded quotes; assumes
/// `standard_conforming_strings = on`, the default since Postgres 9.1). Rejects NUL.
pub fn quote_literal(value: &str) -> Result<String> {
    if value.contains('\0') {
        return Err(IndexError::InvalidArgument("literal contains NUL".into()));
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}

/// Statements creating/normalising the three cluster-wide roles.
pub fn role_statements(passwords: &RolePasswords) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (role, password) in [
        (OWNER_ROLE, &passwords.owner),
        (APP_ROLE, &passwords.app),
        (ACCOUNTS_ROLE, &passwords.accounts),
    ] {
        // Tolerates a concurrent creator (duplicate_object / unique_violation on pg_authid).
        out.push(format!(
            "DO $$ BEGIN \
               IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE rolname = {lit}) THEN \
                 BEGIN CREATE ROLE {ident}; \
                 EXCEPTION WHEN duplicate_object OR unique_violation THEN NULL; END; \
               END IF; \
             END $$",
            lit = quote_literal(role)?,
            ident = quote_ident(role)?,
        ));
        // Always re-assert the attributes, so a pre-existing role can't carry extra powers.
        out.push(format!(
            "ALTER ROLE {} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION INHERIT",
            quote_ident(role)?
        ));
        if let Some(pw) = password {
            out.push(format!(
                "ALTER ROLE {} PASSWORD {}",
                quote_ident(role)?,
                quote_literal(pw)?
            ));
        }
    }
    Ok(out)
}

/// Statements preparing database `database` (run while connected to that database).
pub fn database_statements(database: &str) -> Result<Vec<String>> {
    let db = quote_ident(database)?;
    let (owner, app, accounts, schema) = (
        quote_ident(OWNER_ROLE)?,
        quote_ident(APP_ROLE)?,
        quote_ident(ACCOUNTS_ROLE)?,
        quote_ident(SCHEMA)?,
    );
    let mut out = vec![
        format!("REVOKE ALL ON DATABASE {db} FROM PUBLIC"),
        format!("GRANT CONNECT, TEMPORARY ON DATABASE {db} TO {owner}"),
        format!("GRANT CONNECT ON DATABASE {db} TO {app}, {accounts}"),
        // pgvector is not a trusted extension, so the superuser creates both here.
        "CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public".to_owned(),
        "CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public".to_owned(),
        "REVOKE CREATE ON SCHEMA public FROM PUBLIC".to_owned(),
        format!("CREATE SCHEMA IF NOT EXISTS {schema} AUTHORIZATION {owner}"),
        format!("ALTER SCHEMA {schema} OWNER TO {owner}"),
        format!("REVOKE ALL ON SCHEMA {schema} FROM PUBLIC"),
        format!("GRANT USAGE ON SCHEMA {schema} TO {app}, {accounts}"),
    ];
    for role in [&owner, &app, &accounts] {
        out.push(format!(
            "ALTER ROLE {role} IN DATABASE {db} SET search_path = {SEARCH_PATH}"
        ));
    }
    Ok(out)
}

/// The full setup as one psql-runnable script (roles, then database; connect to `database`).
pub fn bootstrap_script(database: &str, passwords: &RolePasswords) -> Result<String> {
    let mut script = String::from(
        "-- Strata database bootstrap: run as a superuser connected to the target database.\n",
    );
    for stmt in role_statements(passwords)?
        .into_iter()
        .chain(database_statements(database)?)
    {
        script.push_str(&stmt);
        script.push_str(";\n");
    }
    Ok(script)
}

/// Creates the three roles if missing and normalises their attributes. Superuser only.
pub async fn ensure_roles(conn: &mut PgConnection, passwords: &RolePasswords) -> Result<()> {
    for stmt in role_statements(passwords)? {
        sqlx::query(AssertSqlSafe(stmt)).execute(&mut *conn).await?;
    }
    Ok(())
}

/// Prepares `database` for Strata: database grants, extensions, the owned schema, and each
/// role's `search_path`. `conn` must be a superuser connection **to that database**.
pub async fn prepare_database(conn: &mut PgConnection, database: &str) -> Result<()> {
    let current: String = sqlx::query_scalar("SELECT current_database()::text")
        .fetch_one(&mut *conn)
        .await?;
    if current != database {
        return Err(IndexError::InvalidArgument(format!(
            "prepare_database({database}) must run while connected to that database, not {current}"
        )));
    }
    for stmt in database_statements(database)? {
        sqlx::query(AssertSqlSafe(stmt)).execute(&mut *conn).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_escapes_embedded_quotes() {
        assert_eq!(quote_ident("strata").expect("valid"), "\"strata\"");
        assert_eq!(quote_ident("we\"ird").expect("valid"), "\"we\"\"ird\"");
        assert!(matches!(quote_ident(""), Err(IndexError::InvalidIdentifier(_))));
        assert!(matches!(quote_ident(&"x".repeat(64)), Err(IndexError::InvalidIdentifier(_))));
        assert_eq!(quote_literal("it's").expect("valid"), "'it''s'");
    }

    #[test]
    fn database_statements_are_parameterised_by_name() {
        let stmts = database_statements("strata_prod").expect("valid");
        assert_eq!(stmts[0], "REVOKE ALL ON DATABASE \"strata_prod\" FROM PUBLIC");
        assert_eq!(
            stmts.last().map(String::as_str),
            Some("ALTER ROLE \"strata_accounts\" IN DATABASE \"strata_prod\" SET search_path = strata, public")
        );
        assert_eq!(stmts.len(), 13);
    }

    #[test]
    fn role_statements_set_passwords_only_when_given() {
        let none = role_statements(&RolePasswords::default()).expect("valid");
        assert_eq!(none.len(), 6);
        assert!(none.iter().all(|s| !s.contains("PASSWORD")));
        let some = role_statements(&RolePasswords {
            app: Some("pa'ss".into()),
            ..RolePasswords::default()
        })
        .expect("valid");
        assert_eq!(some.len(), 7);
        assert_eq!(some[4], "ALTER ROLE \"strata_app\" PASSWORD 'pa''ss'");
        assert_eq!(
            some[3],
            "ALTER ROLE \"strata_app\" LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION INHERIT"
        );
    }
}
