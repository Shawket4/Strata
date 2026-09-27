//! Maintenance subcommands: `keygen`, `create-user`, `openapi`, `migrate`, `bootstrap-roles`.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use sqlx::Connection;
use strata_api::auth::password::PasswordHasher;
use strata_api::auth::service::{NewAccount, create_active_account};
use strata_api::auth::tokens::generate_key_pem;
use strata_api::auth::{DataRoot, GitVaultProvisioner, SigningKeys};
use strata_common::{Clock, Config, SystemClock, SystemIdGenerator, UserId};
use strata_index::AccountsDb;
use strata_index::bootstrap::{self, RolePasswords};
use strata_index::types::UserRole;

use crate::checks::StartupError;

/// Errors of a maintenance command.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// The target file exists and `--force` was not given.
    #[error("{} already exists; pass --force to replace it", .0.display())]
    Exists(std::path::PathBuf),
    /// Startup-style failure (database, key, config).
    #[error(transparent)]
    Startup(#[from] StartupError),
    /// The account could not be created.
    #[error("{0}")]
    Account(String),
    /// I/O.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Invalid input.
    #[error("{0}")]
    Invalid(String),
}

impl From<strata_index::IndexError> for CommandError {
    fn from(e: strata_index::IndexError) -> Self {
        Self::Startup(e.into())
    }
}

impl From<sqlx::Error> for CommandError {
    fn from(e: sqlx::Error) -> Self {
        Self::Startup(e.into())
    }
}

/// `stratad keygen`: writes a new Ed25519 signing key (PKCS#8 PEM) with mode 0600. Refuses to
/// replace an existing file unless `force` (rotating the key signs every device out within
/// one access-token lifetime: their refresh tokens still work).
pub fn keygen(path: &Path, force: bool) -> Result<(), CommandError> {
    if path.exists() && !force {
        return Err(CommandError::Exists(path.to_path_buf()));
    }
    let pem = generate_key_pem().map_err(|e| CommandError::Invalid(e.to_string()))?;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir)?;
    }
    // Write to a private temp file in the same directory, then rename into place.
    let tmp = path.with_extension("pem.tmp");
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let _ = std::fs::remove_file(&tmp);
        let mut file = options.open(&tmp)?;
        file.write_all(pem.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    // Prove the file loads.
    SigningKeys::from_pem(&std::fs::read_to_string(path)?)
        .map_err(|e| CommandError::Invalid(e.to_string()))?;
    Ok(())
}

/// Input of `stratad create-user`.
#[derive(Debug, Clone)]
pub struct CreateUser<'a> {
    /// Username.
    pub username: &'a str,
    /// Display name (defaults to the username).
    pub display_name: &'a str,
    /// Password.
    pub password: &'a str,
    /// Create an admin.
    pub admin: bool,
}

/// `stratad create-user`: an active account with its vault, through the `strata_accounts`
/// role (the first admin, PLAN §8).
pub async fn create_user(config: &Config, input: &CreateUser<'_>) -> Result<UserId, CommandError> {
    let pool = strata_index::pool::connect(
        strata_index::pool::connect_options(&config.database.accounts_url)?,
        1,
    )
    .await?;
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ids = SystemIdGenerator::new(clock.clone());
    let hasher = PasswordHasher::new(&config.auth.argon2)
        .map_err(|e| CommandError::Invalid(e.to_string()))?;
    let vaults = GitVaultProvisioner::new(DataRoot::new(&config.data_root));
    let user = create_active_account(
        &AccountsDb::new(pool.clone()),
        &hasher,
        &vaults,
        &ids,
        usize::try_from(config.auth.min_password_length).unwrap_or(usize::MAX),
        &NewAccount {
            username: input.username,
            display_name: input.display_name,
            password: input.password,
            role: if input.admin {
                UserRole::Admin
            } else {
                UserRole::Member
            },
        },
        None,
        clock.now(),
    )
    .await
    .map_err(|e| CommandError::Account(e.to_string()))?;
    pool.close().await;
    Ok(user.id)
}

/// `stratad openapi`: writes the contract (normally `api/openapi.json`).
pub fn openapi(path: &Path) -> Result<(), CommandError> {
    strata_api::openapi::write(path)?;
    Ok(())
}

/// `stratad migrate`: applies pending migrations as `strata_owner`.
pub async fn migrate(config: &Config) -> Result<(), CommandError> {
    let pool = strata_index::pool::connect(
        strata_index::pool::connect_options(&config.database.owner_url)?,
        1,
    )
    .await?;
    strata_index::migrate(&pool).await?;
    pool.close().await;
    Ok(())
}

fn url_part(url: &str, what: &str) -> Result<url::Url, CommandError> {
    url::Url::parse(url).map_err(|e| CommandError::Invalid(format!("{what}: {e}")))
}

fn password_of(url: &str, what: &str) -> Result<Option<String>, CommandError> {
    let parsed = url_part(url, what)?;
    Ok(parsed.password().map(|p| {
        percent_encoding::percent_decode_str(p)
            .decode_utf8_lossy()
            .into_owned()
    }))
}

/// The database name and role passwords taken from the configured role URLs.
pub fn bootstrap_inputs(config: &Config) -> Result<(String, RolePasswords), CommandError> {
    let app = url_part(&config.database.app_url, "database.app_url")?;
    let database = app.path().trim_start_matches('/').to_owned();
    if database.is_empty() {
        return Err(CommandError::Invalid(
            "database.app_url names no database".into(),
        ));
    }
    Ok((
        database,
        RolePasswords {
            owner: password_of(&config.database.owner_url, "database.owner_url")?,
            app: password_of(&config.database.app_url, "database.app_url")?,
            accounts: password_of(&config.database.accounts_url, "database.accounts_url")?,
        },
    ))
}

/// `stratad bootstrap-roles --print`: the superuser script (roles, grants, extensions,
/// schema) for the configured database.
pub fn bootstrap_script(config: &Config) -> Result<String, CommandError> {
    let (database, passwords) = bootstrap_inputs(config)?;
    Ok(bootstrap::bootstrap_script(&database, &passwords)?)
}

/// `stratad bootstrap-roles`: applies the script, connected as a superuser
/// (`superuser_url` names any database of the cluster; the configured one must exist).
pub async fn bootstrap_roles(config: &Config, superuser_url: &str) -> Result<(), CommandError> {
    let (database, passwords) = bootstrap_inputs(config)?;
    let options = strata_index::pool::connect_options(superuser_url)?;
    let mut conn = sqlx::PgConnection::connect_with(&options).await?;
    bootstrap::ensure_roles(&mut conn, &passwords).await?;
    conn.close().await?;
    let mut conn = sqlx::PgConnection::connect_with(&options.database(&database)).await?;
    bootstrap::prepare_database(&mut conn, &database).await?;
    conn.close().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_inputs_come_from_the_role_urls() {
        let mut config = Config::default();
        config.database.owner_url = "postgres://strata_owner:o%40pw@db/strata".into();
        config.database.app_url = "postgres://strata_app:apw@db/strata".into();
        let (database, passwords) = bootstrap_inputs(&config).expect("valid");
        assert_eq!(database, "strata");
        assert_eq!(
            passwords,
            RolePasswords {
                owner: Some("o@pw".into()),
                app: Some("apw".into()),
                accounts: None,
            }
        );
        config.database.app_url = "postgres://strata_app@db".into();
        assert!(matches!(
            bootstrap_inputs(&config),
            Err(CommandError::Invalid(_))
        ));
    }

    #[test]
    fn keygen_writes_a_private_loadable_key_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("keys/token.pem");
        keygen(&path, false).expect("keygen");
        let first = std::fs::read_to_string(&path).expect("read");
        assert!(first.starts_with("-----BEGIN PRIVATE KEY-----"));
        crate::checks::check_secret_file(&path).expect("0600");
        assert!(matches!(keygen(&path, false), Err(CommandError::Exists(_))));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), first);
        keygen(&path, true).expect("forced");
        assert_ne!(std::fs::read_to_string(&path).expect("read"), first);
        assert!(!path.with_extension("pem.tmp").exists());
    }

    #[test]
    fn openapi_writes_the_production_contract() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("openapi.json");
        openapi(&path).expect("write");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            strata_api::openapi::to_pretty_json(&strata_api::openapi::document())
        );
    }
}
