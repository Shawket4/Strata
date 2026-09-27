//! Startup checks: secret file permissions and the database locale (PLAN §8, §14, §15).

use std::path::{Path, PathBuf};

use sqlx::PgPool;

/// Why `stratad` refuses to start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartupError {
    /// A configured secret file does not exist.
    #[error("secret file {} is missing", path.display())]
    MissingSecret {
        /// The path.
        path: PathBuf,
    },
    /// A secret file is not a regular file.
    #[error("secret file {} is not a regular file", path.display())]
    NotAFile {
        /// The path.
        path: PathBuf,
    },
    /// A secret file is accessible to group or others.
    #[error(
        "secret file {} has mode {mode:03o}; it must be accessible by its owner only (chmod 600)",
        path.display()
    )]
    SecretPermissions {
        /// The path.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
    /// The signing key cannot be used.
    #[error("signing key {}: {message}", path.display())]
    SigningKey {
        /// The path.
        path: PathBuf,
        /// What is wrong (never key material).
        message: String,
    },
    /// The database is not UTF-8.
    #[error("database `{database}` has encoding {encoding}; Strata requires UTF8")]
    DatabaseEncoding {
        /// Database name.
        database: String,
        /// Its encoding.
        encoding: String,
    },
    /// The database uses the `C`/`POSIX` character locale.
    #[error(
        "database `{database}` has LC_CTYPE `{ctype}`; Strata requires a UTF-8 character locale \
         other than C/POSIX (e.g. C.UTF-8), otherwise pg_trgm treats Arabic letters as \
         non-word characters. Recreate it: CREATE DATABASE … ENCODING 'UTF8' \
         LC_COLLATE 'C.UTF-8' LC_CTYPE 'C.UTF-8' TEMPLATE template0"
    )]
    DatabaseLocale {
        /// Database name.
        database: String,
        /// Its `LC_CTYPE`.
        ctype: String,
    },
    /// The data root is missing or not a directory.
    #[error("data root {} is not a directory", path.display())]
    DataRoot {
        /// The path.
        path: PathBuf,
    },
    /// A database error during startup.
    #[error("database: {0}")]
    Db(String),
    /// Configuration could not be loaded.
    #[error("{0}")]
    Config(String),
    /// Any other I/O failure.
    #[error("{0}")]
    Io(String),
}

impl From<sqlx::Error> for StartupError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<strata_index::IndexError> for StartupError {
    fn from(e: strata_index::IndexError) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<std::io::Error> for StartupError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// A secret file must exist, be a regular file, and be accessible by its owner only
/// (no group/other permission bits; `0600` or `0400`).
pub fn check_secret_file(path: &Path) -> Result<(), StartupError> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(StartupError::MissingSecret {
                path: path.to_path_buf(),
            });
        }
        Err(e) => return Err(e.into()),
    };
    if !meta.is_file() {
        return Err(StartupError::NotAFile {
            path: path.to_path_buf(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(StartupError::SecretPermissions {
                path: path.to_path_buf(),
                mode,
            });
        }
    }
    Ok(())
}

/// Every secret file the configuration names (the signing key always; the others if set).
pub fn secret_files(config: &strata_common::Config) -> Vec<PathBuf> {
    let mut out = vec![config.auth.signing_key_file.clone()];
    out.extend(config.ai.api_key_file.iter().cloned());
    out.extend(config.push.fcm_service_account_path.iter().cloned());
    out.extend(config.push.apns_key_path.iter().cloned());
    out.extend(config.push.wns_credentials_path.iter().cloned());
    out
}

/// The connected database must be UTF-8 with a character locale other than `C`/`POSIX`
/// (PLAN §14: `pg_trgm` word boundaries for Arabic, and server/offline duplicate scores).
pub async fn check_database_locale(pool: &PgPool) -> Result<(), StartupError> {
    let (database, encoding, ctype): (String, String, String) = sqlx::query_as(
        "SELECT datname::text, pg_encoding_to_char(encoding)::text, datctype::text \
         FROM pg_catalog.pg_database WHERE datname = current_database()",
    )
    .fetch_one(pool)
    .await?;
    if encoding != "UTF8" {
        return Err(StartupError::DatabaseEncoding { database, encoding });
    }
    if ctype.eq_ignore_ascii_case("C") || ctype.eq_ignore_ascii_case("POSIX") {
        return Err(StartupError::DatabaseLocale { database, ctype });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn secret_files_must_be_owner_only_regular_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("key.pem");
        assert_eq!(
            check_secret_file(&path),
            Err(StartupError::MissingSecret { path: path.clone() })
        );
        std::fs::write(&path, "x").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let err = check_secret_file(&path).expect_err("too open");
        assert_eq!(
            err,
            StartupError::SecretPermissions {
                path: path.clone(),
                mode: 0o644
            }
        );
        assert_eq!(
            err.to_string(),
            format!(
                "secret file {} has mode 644; it must be accessible by its owner only (chmod 600)",
                path.display()
            )
        );
        for ok in [0o600, 0o400] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(ok)).expect("chmod");
            assert_eq!(check_secret_file(&path), Ok(()));
        }
        assert_eq!(
            check_secret_file(dir.path()),
            Err(StartupError::NotAFile {
                path: dir.path().to_path_buf()
            })
        );
    }

    #[test]
    fn every_configured_secret_is_checked() {
        let mut config = strata_common::Config::default();
        assert_eq!(
            secret_files(&config),
            vec![PathBuf::from("/etc/strata/token-signing-key.pem")]
        );
        config.ai.api_key_file = Some("/etc/strata/anthropic.key".into());
        config.push.apns_key_path = Some("/etc/strata/apns.p8".into());
        assert_eq!(
            secret_files(&config),
            vec![
                PathBuf::from("/etc/strata/token-signing-key.pem"),
                PathBuf::from("/etc/strata/anthropic.key"),
                PathBuf::from("/etc/strata/apns.p8"),
            ]
        );
    }
}
