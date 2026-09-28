//! `stratad` startup checks and maintenance commands against the test cluster (PLAN §14:
//! refuse a database that is not UTF-8 with a non-`C` character locale; §15: secrets file
//! permissions).
#![allow(clippy::expect_used)] // tests: expect with messages

use std::path::Path;

use pretty_assertions::assert_eq;
use sqlx::Connection;
use strata_common::Config;
use strata_testkit::{ROLE_PASSWORD_ENV, TestDb, admin_url};
use stratad::checks::{StartupError, check_database_locale};
use stratad::commands::{self, CommandError, CreateUser};

fn role_url(role: &str, database: &str) -> String {
    let mut url = url::Url::parse(&admin_url()).expect("admin url");
    url.set_username(role).expect("username");
    url.set_password(std::env::var(ROLE_PASSWORD_ENV).ok().as_deref())
        .expect("password");
    url.set_path(database);
    url.to_string()
}

fn superuser_url(database: &str) -> String {
    let mut url = url::Url::parse(&admin_url()).expect("admin url");
    url.set_path(database);
    url.to_string()
}

fn write_key(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("token.pem");
    commands::keygen(&path, false).expect("keygen");
    path
}

fn config_for(db: &str, dir: &Path) -> Config {
    let mut config = Config {
        data_root: dir.to_path_buf(),
        ..Config::default()
    };
    config.auth.signing_key_file = write_key(dir);
    config.auth.argon2.memory_kib = 64;
    config.auth.argon2.iterations = 1;
    config.database.owner_url = role_url("strata_owner", db);
    config.database.app_url = role_url("strata_app", db);
    config.database.accounts_url = role_url("strata_accounts", db);
    config.database.max_connections = 2;
    config
}

/// A throwaway database with the given encoding and locale, dropped at the end.
struct ScratchDb(String);

impl ScratchDb {
    async fn create(encoding: &str, ctype: &str) -> Self {
        let name = format!(
            "strata_test_{}_{}_locale_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_secs(),
            std::process::id(),
            ctype.to_ascii_lowercase().replace(['.', '-'], "_")
        );
        let mut admin = sqlx::PgConnection::connect(&admin_url())
            .await
            .expect("admin");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE \"{name}\" TEMPLATE template0 ENCODING '{encoding}' \
             LC_COLLATE '{ctype}' LC_CTYPE '{ctype}'"
        )))
        .execute(&mut admin)
        .await
        .expect("create database");
        admin.close().await.expect("close");
        Self(name)
    }

    async fn drop_now(self) {
        let mut admin = sqlx::PgConnection::connect(&admin_url())
            .await
            .expect("admin");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
            self.0
        )))
        .execute(&mut admin)
        .await
        .expect("drop");
        admin.close().await.expect("close");
    }
}

#[tokio::test]
async fn startup_refuses_a_c_locale_database() {
    let scratch = ScratchDb::create("UTF8", "C").await;
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = config_for(&scratch.0, dir.path());
    // The superuser can connect to the bare database; the check runs before anything else.
    config.database.app_url = superuser_url(&scratch.0);
    config.database.accounts_url = superuser_url(&scratch.0);
    let err = stratad::serve::prepare(&config)
        .await
        .expect_err("must refuse");
    assert_eq!(
        err,
        StartupError::DatabaseLocale {
            database: scratch.0.clone(),
            ctype: "C".to_owned(),
        }
    );
    assert!(
        err.to_string().starts_with(&format!(
            "database `{}` has LC_CTYPE `C`; Strata requires",
            scratch.0
        )),
        "{err}"
    );
    scratch.drop_now().await;
}

#[tokio::test]
async fn startup_refuses_a_non_utf8_database() {
    let scratch = ScratchDb::create("SQL_ASCII", "C").await;
    let pool = sqlx::PgPool::connect(&superuser_url(&scratch.0))
        .await
        .expect("pool");
    assert_eq!(
        check_database_locale(&pool).await,
        Err(StartupError::DatabaseEncoding {
            database: scratch.0.clone(),
            encoding: "SQL_ASCII".to_owned(),
        })
    );
    pool.close().await;
    scratch.drop_now().await;
}

#[tokio::test]
async fn startup_accepts_the_prepared_database_and_refuses_open_secrets() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_for(db.name(), dir.path());
    let state = stratad::serve::prepare(&config).await.expect("starts");
    assert_eq!(state.revocations().revoked_session_count(), 0);
    drop(state);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            &config.auth.signing_key_file,
            std::fs::Permissions::from_mode(0o640),
        )
        .expect("chmod");
        assert_eq!(
            stratad::serve::prepare(&config).await.map(|_| ()),
            Err(StartupError::SecretPermissions {
                path: config.auth.signing_key_file.clone(),
                mode: 0o640,
            })
        );
        std::fs::set_permissions(
            &config.auth.signing_key_file,
            std::fs::Permissions::from_mode(0o600),
        )
        .expect("chmod");
    }
    let mut missing = config.clone();
    missing.auth.signing_key_file = dir.path().join("absent.pem");
    assert_eq!(
        stratad::serve::prepare(&missing).await.map(|_| ()),
        Err(StartupError::MissingSecret {
            path: dir.path().join("absent.pem"),
        })
    );
    let mut garbage = config.clone();
    garbage.auth.signing_key_file = dir.path().join("garbage.pem");
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options
            .open(&garbage.auth.signing_key_file)
            .expect("create")
            .write_all(b"not a key")
            .expect("write");
    }
    assert_eq!(
        stratad::serve::prepare(&garbage).await.map(|_| ()),
        Err(StartupError::SigningKey {
            path: garbage.auth.signing_key_file.clone(),
            message: "invalid signing key: not an Ed25519 PKCS#8 PEM private key".to_owned(),
        })
    );
    let mut no_root = config.clone();
    no_root.data_root = dir.path().join("nope");
    assert_eq!(
        stratad::serve::prepare(&no_root).await.map(|_| ()),
        Err(StartupError::DataRoot {
            path: dir.path().join("nope"),
        })
    );
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn create_user_makes_an_active_admin_with_a_vault() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_for(db.name(), dir.path());
    let id = commands::create_user(
        &config,
        &CreateUser {
            username: "owner",
            display_name: "The Owner",
            password: "owner-password-1",
            admin: true,
        },
    )
    .await
    .expect("created");
    let user = db
        .accounts_db
        .user_by_id(id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(user.username, "owner");
    assert_eq!(user.display_name, "The Owner");
    assert_eq!(user.role, strata_index::types::UserRole::Admin);
    assert_eq!(user.status, strata_index::types::UserStatus::Active);
    assert!(
        user.password_hash
            .starts_with("$argon2id$v=19$m=64,t=1,p=1$")
    );
    assert!(
        dir.path()
            .join("users")
            .join(id.to_string())
            .join("vault/.git/HEAD")
            .is_file()
    );
    let audit = db.accounts_db.list_audit(10).await.expect("audit");
    assert_eq!(
        audit
            .iter()
            .map(|e| (e.actor_id, e.action.as_str(), e.target.clone()))
            .collect::<Vec<_>>(),
        vec![(None, "user.create", format!("user:{id}"))]
    );
    let dup = commands::create_user(
        &config,
        &CreateUser {
            username: "OWNER",
            display_name: "x",
            password: "owner-password-1",
            admin: false,
        },
    )
    .await
    .expect_err("taken");
    assert_eq!(dup.to_string(), "username taken");
    let short = commands::create_user(
        &config,
        &CreateUser {
            username: "other",
            display_name: "x",
            password: "short",
            admin: false,
        },
    )
    .await
    .expect_err("short password");
    assert!(matches!(short, CommandError::Account(_)), "{short:?}");
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn migrate_and_bootstrap_roles_prepare_an_empty_database() {
    let db = TestDb::new_unmigrated().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_for(db.name(), dir.path());
    // Idempotent re-bootstrap as superuser, then migrations as strata_owner.
    commands::bootstrap_roles(&config, &superuser_url("postgres"))
        .await
        .expect("bootstrap");
    commands::migrate(&config).await.expect("migrate");
    commands::migrate(&config).await.expect("re-run is a no-op");
    let users: Option<String> = sqlx::query_scalar("SELECT to_regclass('strata.users')::text")
        .fetch_one(&db.superuser)
        .await
        .expect("query");
    assert_eq!(users.as_deref(), Some("users"));
    let script = commands::bootstrap_script(&config).expect("script");
    assert_eq!(
        script,
        strata_index::bootstrap::bootstrap_script(
            db.name(),
            &strata_index::bootstrap::RolePasswords {
                owner: std::env::var(ROLE_PASSWORD_ENV).ok(),
                app: std::env::var(ROLE_PASSWORD_ENV).ok(),
                accounts: std::env::var(ROLE_PASSWORD_ENV).ok(),
            }
        )
        .expect("script")
    );
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn verify_and_reindex_work_on_a_created_users_vault() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_for(db.name(), dir.path());
    let id = commands::create_user(
        &config,
        &CreateUser {
            username: "owner",
            display_name: "Owner",
            password: "owner-password-1",
            admin: false,
        },
    )
    .await
    .expect("created");
    let vault = dir.path().join("users").join(id.to_string()).join("vault");
    assert!(vault.join("inbox/.gitkeep").is_file(), "§6.1 skeleton");
    let clean = commands::verify(&config, "OWNER").await.expect("verify");
    assert!(clean.is_clean(), "{clean:?}");
    // A note written behind the server's back: reported, not repaired.
    std::fs::write(vault.join("notes/Outside.md"), "Hello.\n").expect("write");
    let dirty = commands::verify(&config, &id.to_string())
        .await
        .expect("verify");
    assert_eq!(dirty.recovered, vec!["notes/Outside.md".to_owned()]);
    assert_eq!(dirty.ids_assigned, vec!["notes/Outside.md".to_owned()]);
    assert_eq!(
        std::fs::read_to_string(vault.join("notes/Outside.md")).expect("read"),
        "Hello.\n",
        "verify changes nothing"
    );
    // Reindex reconciles first (ID + recovery commit) and derives every note.
    assert_eq!(
        commands::reindex(&config, "owner").await.expect("reindex"),
        1
    );
    assert!(
        commands::verify(&config, "owner")
            .await
            .expect("verify")
            .is_clean()
    );
    assert!(matches!(
        commands::verify(&config, "nobody").await,
        Err(CommandError::Invalid(m)) if m == "no such user: nobody"
    ));
    db.cleanup().await.expect("cleanup");
}

/// `(hint id, entity id, text, source decision, created)` rows of `user`, as text.
async fn hint_rows(
    db: &TestDb,
    user: strata_common::UserId,
) -> Vec<(String, String, String, Option<String>, String)> {
    sqlx::query_as(
        "SELECT id::text, entity_id::text, hint, source_decision_id::text, \
           to_char(created AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') \
         FROM disambiguation_hints WHERE user_id = $1 ORDER BY created, id",
    )
    .bind(user.as_uuid())
    .fetch_all(&db.superuser)
    .await
    .expect("hints")
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one scenario: build the vault, reindex twice
async fn reindex_rebuilds_disambiguation_hints_from_entity_sidecars() {
    use strata_common::{DecisionId, HintId, NoteId};

    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_for(db.name(), dir.path());
    let user = commands::create_user(
        &config,
        &CreateUser {
            username: "owner",
            display_name: "Owner",
            password: "owner-password-1",
            admin: false,
        },
    )
    .await
    .expect("created");
    let vault = dir
        .path()
        .join("users")
        .join(user.to_string())
        .join("vault");
    let fathy = NoteId::from_ulid(ulid::Ulid(0x0199_2222_0000_0000_0000_0000_0000_0001));
    let note = NoteId::from_ulid(ulid::Ulid(0x0199_2222_0000_0000_0000_0000_0000_0002));
    let h1 = HintId::from_ulid(ulid::Ulid(0x0199_3333_0000_0000_0000_0000_0000_0001));
    let h2 = HintId::from_ulid(ulid::Ulid(0x0199_3333_0000_0000_0000_0000_0000_0002));
    let other = HintId::from_ulid(ulid::Ulid(0x0199_3333_0000_0000_0000_0000_0000_0003));
    // A person with two hints mirrored in its sidecar (plus an unreadable entry and a repeated
    // ID), and a plain note whose sidecar carries a hints key (not an entity: ignored).
    std::fs::write(
        vault.join("people/Ahmed Fathy.md"),
        format!("---\nid: {fathy}\nkind: person\n---\n## Notes\n"),
    )
    .expect("person");
    std::fs::write(
        vault.join("notes/Acme call.md"),
        format!("---\nid: {note}\n---\nAhmed called.\n"),
    )
    .expect("note");
    let hint = |id: &HintId, text: &str, at: &str| {
        format!(r#"{{"id": "{id}", "text": "{text}", "at": "{at}"}}"#)
    };
    std::fs::create_dir_all(vault.join(".meta/notes")).expect("meta");
    std::fs::write(
        vault.join(format!(".meta/notes/{fathy}.json")),
        format!(
            r#"{{"id": "{fathy}", "relations": [], "rejected": [], "hints": [{}, {}, {{"text": "no id"}}, {}]}}"#,
            hint(&h1, "Ahmed at Acme = Ahmed Fathy", "2026-09-20T10:00:00Z"),
            hint(&h2, "Ahmed Fathy works at Petrol Arrows", "2026-09-21T11:30:00Z"),
            hint(&h1, "a repeated ID", "2026-09-22T00:00:00Z"),
        ),
    )
    .expect("sidecar");
    std::fs::write(
        vault.join(format!(".meta/notes/{note}.json")),
        format!(
            r#"{{"id": "{note}", "relations": [], "rejected": [], "hints": [{}]}}"#,
            hint(&other, "not an entity", "2026-09-20T10:00:00Z"),
        ),
    )
    .expect("sidecar");
    assert_eq!(
        commands::reindex(&config, "owner").await.expect("reindex"),
        2
    );
    let rows = |decision: Option<&DecisionId>| {
        vec![
            (
                h1.as_uuid().to_string(),
                fathy.as_uuid().to_string(),
                "Ahmed at Acme = Ahmed Fathy".to_owned(),
                decision.map(|d| d.as_uuid().to_string()),
                "2026-09-20T10:00:00Z".to_owned(),
            ),
            (
                h2.as_uuid().to_string(),
                fathy.as_uuid().to_string(),
                "Ahmed Fathy works at Petrol Arrows".to_owned(),
                None,
                "2026-09-21T11:30:00Z".to_owned(),
            ),
        ]
    };
    assert_eq!(hint_rows(&db, user).await, rows(None));

    // A hint's correction decision (app state) survives the next reindex; a row that is not
    // in the vault (no sidecar entry) does not.
    let decision = DecisionId::from_ulid(ulid::Ulid(0x0199_4444_0000_0000_0000_0000_0000_0001));
    sqlx::query("UPDATE disambiguation_hints SET source_decision_id = $1 WHERE id = $2")
        .bind(decision.as_uuid())
        .bind(h1.as_uuid())
        .execute(&db.superuser)
        .await
        .expect("decision");
    sqlx::query(
        "INSERT INTO disambiguation_hints (user_id, id, entity_id, hint, created) \
         VALUES ($1, $2, $3, 'only in the table', now())",
    )
    .bind(user.as_uuid())
    .bind(other.as_uuid())
    .bind(fathy.as_uuid())
    .execute(&db.superuser)
    .await
    .expect("stray row");
    assert_eq!(
        commands::reindex(&config, "owner").await.expect("reindex"),
        2
    );
    assert_eq!(hint_rows(&db, user).await, rows(Some(&decision)));
    db.cleanup().await.expect("cleanup");
}
