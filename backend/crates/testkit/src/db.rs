//! Per-test databases cloned from a migrated template.
//!
//! Lifecycle, once per test process (guarded by a Postgres advisory lock so parallel test
//! binaries never race):
//! 1. drop stale `strata_test_%` databases older than one hour (their names carry a timestamp);
//! 2. create/normalise the cluster-wide roles `strata_owner`, `strata_app`, `strata_accounts`;
//! 3. build `strata_tpl_<hash>` if missing: bootstrap + migrate a scratch database, then rename
//!    it into place and mark it `IS_TEMPLATE`, `ALLOW_CONNECTIONS false`. The hash covers every
//!    migration checksum and the bootstrap SQL, so a schema change yields a new template.
//!
//! Per test: `CREATE DATABASE strata_test_<secs>_<pid>_<n> TEMPLATE strata_tpl_<hash>`, run the
//! database-level bootstrap (database ACLs and per-database role settings are not copied by
//! `TEMPLATE`), open one pool per role, and drop the database with `WITH (FORCE)` on drop.
//!
//! Roles are shared cluster-wide (role names are global in Postgres); a test database grants
//! them nothing outside itself. Passwords are only set when `STRATA_TEST_ROLE_PASSWORD` is set,
//! so a developer's real `strata_*` role passwords are never overwritten (trust auth assumed
//! otherwise, as in the dev container).

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};
use strata_common::{FakeClock, SequentialIdGenerator, UserId};
use strata_index::bootstrap::{
    self, ACCOUNTS_ROLE, APP_ROLE, OWNER_ROLE, RolePasswords, quote_ident,
};
use strata_index::{AccountsDb, AppDb, ScopeIssuer, ScopedTx, UserScope};
use tokio::sync::OnceCell;

use crate::error::TestkitError;

/// Environment variable holding the admin (superuser) URL.
pub const ADMIN_URL_ENV: &str = "STRATA_TEST_DATABASE_URL";
/// Admin URL used when [`ADMIN_URL_ENV`] is unset.
pub const DEFAULT_ADMIN_URL: &str = "postgres://postgres@127.0.0.1:5432/postgres";
/// Environment variable with a password to set on the three roles (optional).
pub const ROLE_PASSWORD_ENV: &str = "STRATA_TEST_ROLE_PASSWORD";
/// Name prefix of per-test databases.
pub const TEST_DB_PREFIX: &str = "strata_test_";
/// Name prefix of template databases.
pub const TEMPLATE_PREFIX: &str = "strata_tpl_";
/// Bump when the template preparation below changes in a way the hash can't see.
const TEMPLATE_RECIPE: &str = "testkit-template-v1";
/// Stale test databases are dropped after this many seconds.
const STALE_AFTER_SECS: u64 = 3600;
/// Advisory lock key serialising template setup across processes.
const SETUP_LOCK_KEY: i64 = 0x5354_5241_5441_0001;

static TEMPLATE: OnceCell<String> = OnceCell::const_new();
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// The admin URL ([`ADMIN_URL_ENV`] or [`DEFAULT_ADMIN_URL`]).
pub fn admin_url() -> String {
    std::env::var(ADMIN_URL_ENV).unwrap_or_else(|_| DEFAULT_ADMIN_URL.to_owned())
}

fn role_passwords() -> RolePasswords {
    let pw = std::env::var(ROLE_PASSWORD_ENV).ok();
    RolePasswords {
        owner: pw.clone(),
        app: pw.clone(),
        accounts: pw,
    }
}

/// The template name for the current migrations and bootstrap recipe.
pub fn template_name() -> Result<String, TestkitError> {
    let mut hasher = Sha256::new();
    hasher.update(TEMPLATE_RECIPE.as_bytes());
    for m in strata_index::MIGRATOR.iter() {
        hasher.update(m.version.to_be_bytes());
        hasher.update(&*m.checksum);
    }
    for stmt in bootstrap::database_statements("template")? {
        hasher.update(stmt.as_bytes());
    }
    let digest = hex::encode(hasher.finalize());
    Ok(format!("{TEMPLATE_PREFIX}{}", &digest[..16]))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn admin_options() -> Result<PgConnectOptions, TestkitError> {
    Ok(PgConnectOptions::from_str(&admin_url())?)
}

async fn admin_conn(database: Option<&str>) -> Result<PgConnection, TestkitError> {
    let mut opts = admin_options()?;
    if let Some(db) = database {
        opts = opts.database(db);
    }
    Ok(PgConnection::connect_with(&opts).await?)
}

async fn exec(conn: &mut PgConnection, sql: String) -> Result<(), TestkitError> {
    sqlx::query(AssertSqlSafe(sql)).execute(conn).await?;
    Ok(())
}

async fn ensure_template() -> Result<String, TestkitError> {
    TEMPLATE
        .get_or_try_init(|| async {
            let mut admin = admin_conn(None).await?;
            sqlx::query("SELECT pg_advisory_lock($1)")
                .bind(SETUP_LOCK_KEY)
                .execute(&mut admin)
                .await?;
            let result = build_template(&mut admin).await;
            // Unlock even if building failed; the connection closing would release it anyway.
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(SETUP_LOCK_KEY)
                .execute(&mut admin)
                .await;
            let _ = admin.close().await;
            result
        })
        .await
        .cloned()
}

async fn build_template(admin: &mut PgConnection) -> Result<String, TestkitError> {
    drop_stale_databases(admin).await?;
    bootstrap::ensure_roles(admin, &role_passwords()).await?;

    let name = template_name()?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&name)
            .fetch_one(&mut *admin)
            .await?;
    if exists {
        return Ok(name);
    }
    let build = format!("{name}_build");
    exec(
        admin,
        format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            quote_ident(&build)?
        ),
    )
    .await?;
    exec(admin, format!("CREATE DATABASE {}", quote_ident(&build)?)).await?;
    {
        let mut su = admin_conn(Some(&build)).await?;
        bootstrap::prepare_database(&mut su, &build).await?;
        su.close().await?;
        let owner = role_pool(&build, OWNER_ROLE, 1).await?;
        strata_index::migrate(&owner).await?;
        owner.close().await;
    }
    exec(
        admin,
        format!(
            "ALTER DATABASE {} RENAME TO {}",
            quote_ident(&build)?,
            quote_ident(&name)?
        ),
    )
    .await?;
    exec(
        admin,
        format!(
            "ALTER DATABASE {} WITH IS_TEMPLATE true ALLOW_CONNECTIONS false",
            quote_ident(&name)?
        ),
    )
    .await?;
    Ok(name)
}

/// Drops `strata_test_<secs>_…` databases whose timestamp is more than an hour old.
async fn drop_stale_databases(admin: &mut PgConnection) -> Result<(), TestkitError> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT datname::text FROM pg_database WHERE datname LIKE 'strata\\_test\\_%' ORDER BY 1",
    )
    .fetch_all(&mut *admin)
    .await?;
    let now = unix_now();
    for name in names {
        let created = name
            .strip_prefix(TEST_DB_PREFIX)
            .and_then(|rest| rest.split('_').next())
            .and_then(|secs| secs.parse::<u64>().ok());
        if created.is_some_and(|c| now.saturating_sub(c) > STALE_AFTER_SECS) {
            exec(
                admin,
                format!(
                    "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                    quote_ident(&name)?
                ),
            )
            .await?;
        }
    }
    Ok(())
}

async fn role_pool(database: &str, role: &str, max: u32) -> Result<PgPool, TestkitError> {
    let mut opts = strata_index::pool::connect_options(&admin_url())?
        .username(role)
        .database(database);
    // The admin URL's password belongs to the admin, not to the role.
    opts = match std::env::var(ROLE_PASSWORD_ENV) {
        Ok(pw) => opts.password(&pw),
        Err(_) => opts.password(""),
    };
    Ok(PgPoolOptions::new()
        .max_connections(max)
        .connect_with(opts)
        .await?)
}

/// A fresh, migrated database for one test, with a pool per role. Dropped (best effort) when
/// the value is dropped; prefer [`TestDb::cleanup`] at the end of a test to drop it eagerly.
#[derive(Debug)]
pub struct TestDb {
    name: String,
    /// Superuser pool on this database (bypasses RLS: for fixtures and inspection only).
    pub superuser: PgPool,
    /// `strata_owner` pool (subject to forced RLS like everyone else).
    pub owner: PgPool,
    /// Raw `strata_app` pool, for unscoped probes in tests. Production code uses [`AppDb`].
    pub app: PgPool,
    /// Raw `strata_accounts` pool.
    pub accounts: PgPool,
    /// The scoped app handle.
    pub app_db: AppDb,
    /// The scope issuer paired with `app_db`.
    pub issuer: ScopeIssuer,
    /// The accounts handle.
    pub accounts_db: AccountsDb,
    /// Fake clock starting at the test epoch.
    pub clock: FakeClock,
    /// Deterministic ULIDs at the test epoch.
    pub ids: Arc<SequentialIdGenerator>,
    dropped: bool,
}

impl TestDb {
    /// A new database with the default pool size (4 connections per role).
    pub async fn new() -> Result<Self, TestkitError> {
        Self::with_pool_size(4).await
    }

    /// A new database with `app_connections` connections in the `strata_app` pool (others: 2).
    pub async fn with_pool_size(app_connections: u32) -> Result<Self, TestkitError> {
        let template = ensure_template().await?;
        Self::create(Some(&template), app_connections).await
    }

    /// A new **empty** database: bootstrapped (roles, grants, extensions, schema) but not
    /// migrated. For migration tests; everything else should use [`TestDb::new`].
    pub async fn new_unmigrated() -> Result<Self, TestkitError> {
        // The template step also creates the roles and cleans stale databases.
        ensure_template().await?;
        Self::create(None, 4).await
    }

    async fn create(template: Option<&str>, app_connections: u32) -> Result<Self, TestkitError> {
        let name = format!(
            "{TEST_DB_PREFIX}{}_{}_{}",
            unix_now(),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let mut admin = admin_conn(None).await?;
        let create = match template {
            Some(t) => format!(
                "CREATE DATABASE {} TEMPLATE {}",
                quote_ident(&name)?,
                quote_ident(t)?
            ),
            None => format!("CREATE DATABASE {} TEMPLATE template0", quote_ident(&name)?),
        };
        exec(&mut admin, create).await?;
        admin.close().await?;

        let superuser = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(strata_index::pool::connect_options(&admin_url())?.database(&name))
            .await?;
        {
            let mut conn = superuser.acquire().await?;
            bootstrap::prepare_database(&mut conn, &name).await?;
        }
        let owner = role_pool(&name, OWNER_ROLE, 2).await?;
        let app = role_pool(&name, APP_ROLE, app_connections).await?;
        let accounts = role_pool(&name, ACCOUNTS_ROLE, 2).await?;
        let (app_db, issuer) = AppDb::new(app.clone());
        let accounts_db = AccountsDb::new(accounts.clone());
        Ok(Self {
            name,
            superuser,
            owner,
            app,
            accounts,
            app_db,
            issuer,
            accounts_db,
            clock: FakeClock::at_default_epoch(),
            ids: Arc::new(SequentialIdGenerator::default()),
            dropped: false,
        })
    }

    /// The database name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Mints a scope for `user` (tests stand in for the auth middleware).
    pub fn scope(&self, user: UserId) -> UserScope {
        self.issuer.issue(user)
    }

    /// Begins a transaction scoped to `user`.
    pub async fn begin(&self, user: UserId) -> Result<ScopedTx, TestkitError> {
        Ok(self.app_db.begin(&self.scope(user)).await?)
    }

    /// Closes the pools and drops the database now.
    pub async fn cleanup(mut self) -> Result<(), TestkitError> {
        self.dropped = true;
        for pool in [&self.superuser, &self.owner, &self.app, &self.accounts] {
            pool.close().await;
        }
        let mut admin = admin_conn(None).await?;
        exec(
            &mut admin,
            format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                quote_ident(&self.name)?
            ),
        )
        .await?;
        admin.close().await?;
        Ok(())
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.dropped {
            return;
        }
        let Ok(name) = quote_ident(&self.name) else {
            return;
        };
        // Drop on a separate thread with its own runtime: `Drop` can't await, and the test's
        // runtime may be a current-thread one. `WITH (FORCE)` ends the pools' connections.
        let handle = std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async move {
                if let Ok(mut admin) = admin_conn(None).await {
                    let _ = exec(
                        &mut admin,
                        format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"),
                    )
                    .await;
                    let _ = admin.close().await;
                }
            });
        });
        let _ = handle.join();
    }
}
