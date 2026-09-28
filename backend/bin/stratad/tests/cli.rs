//! The `stratad` binary itself (PLAN §14): argument parsing, configuration loading, every
//! maintenance subcommand's output and exit status, and `serve` from start-up to a graceful
//! `SIGTERM` shutdown, run as a child process against a per-test database.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)] // tests

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;
use strata_common::Config;
use strata_testkit::{ROLE_PASSWORD_ENV, TestDb, admin_url};

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

/// A configuration file for `db` with its data root and key under `dir`; returns its path.
fn write_config(db: &str, dir: &Path, bind: &str) -> PathBuf {
    let mut config = Config {
        data_root: dir.join("data"),
        bind: bind.parse().expect("bind address"),
        ..Config::default()
    };
    std::fs::create_dir_all(&config.data_root).expect("data root");
    config.auth.signing_key_file = dir.join("token.pem");
    config.auth.argon2.memory_kib = 64;
    config.auth.argon2.iterations = 1;
    config.auth.argon2.parallelism = 1;
    config.database.owner_url = role_url("strata_owner", db);
    config.database.app_url = role_url("strata_app", db);
    config.database.accounts_url = role_url("strata_accounts", db);
    config.database.max_connections = 2;
    let path = dir.join("stratad.toml");
    std::fs::write(&path, toml::to_string(&config).expect("toml")).expect("write config");
    path
}

/// `stratad` with a clean environment (no inherited config or superuser URL).
fn stratad(config: Option<&Path>) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stratad"));
    cmd.env_remove("STRATA_CONFIG")
        .env_remove("STRATA_SUPERUSER_URL")
        .env("RUST_LOG", "info");
    for (name, _) in std::env::vars() {
        if name.starts_with("STRATA__") {
            cmd.env_remove(name);
        }
    }
    if let Some(config) = config {
        cmd.arg("--config").arg(config);
    }
    cmd
}

fn run(cmd: &mut Command) -> Output {
    cmd.stdin(Stdio::null()).output().expect("stratad runs")
}

fn run_with_stdin(cmd: &mut Command, stdin: &str) -> Output {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("stratad starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("stratad exits")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).expect("utf-8 stdout")
}

fn stderr(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).expect("utf-8 stderr")
}

/// The human-readable lines of stdout (JSON log lines left out).
fn printed(o: &Output) -> Vec<String> {
    stdout(o)
        .lines()
        .filter(|l| !l.starts_with('{'))
        .map(str::to_owned)
        .collect()
}

/// The `message` of every JSON log line on stdout.
fn log_messages(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            Some(v["fields"]["message"].as_str()?.to_owned())
        })
        .collect()
}

#[test]
fn an_unreadable_or_invalid_config_exits_with_status_2() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("absent.toml");
    let out = run(stratad(Some(&missing)).arg("migrate"));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        stderr(&out),
        format!(
            "stratad: cannot read config file {}: No such file or directory (os error 2)\n",
            missing.display()
        )
    );

    let bad = dir.path().join("bad.toml");
    std::fs::write(&bad, "data_root = 7\n").expect("write");
    let out = run(stratad(Some(&bad)).arg("migrate"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).starts_with("stratad: invalid config: "),
        "{}",
        stderr(&out)
    );

    // An environment override is applied after the file and validated the same way.
    let out = run(stratad(None)
        .env("STRATA__JOBS__MAX_CONCURRENCY", "many")
        .arg("migrate"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).starts_with("stratad: invalid environment override STRATA__JOBS__MAX_CONCURRENCY: "),
        "{}",
        stderr(&out)
    );
}

#[test]
fn keygen_writes_the_configured_key_and_refuses_to_overwrite_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config("unused", dir.path(), "127.0.0.1:0");
    let key = dir.path().join("token.pem");

    let out = run(stratad(Some(&config)).arg("keygen"));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(printed(&out), [format!("wrote {}", key.display())]);
    stratad::checks::check_secret_file(&key).expect("private key");
    let first = std::fs::read_to_string(&key).expect("key");

    let out = run(stratad(Some(&config)).arg("keygen"));
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        format!(
            "stratad: {} already exists; pass --force to replace it\n",
            key.display()
        )
    );
    assert_eq!(std::fs::read_to_string(&key).expect("key"), first);

    let other = dir.path().join("keys/other.pem");
    let out = run(stratad(Some(&config))
        .args(["keygen", "--out"])
        .arg(&other)
        .arg("--force"));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(printed(&out), [format!("wrote {}", other.display())]);
    assert_ne!(std::fs::read_to_string(&other).expect("key"), first);
}

#[test]
fn openapi_writes_the_contract_to_the_given_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out_path = dir.path().join("openapi.json");
    let out = run(stratad(None).args(["openapi", "--out"]).arg(&out_path));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(printed(&out), [format!("wrote {}", out_path.display())]);
    assert_eq!(
        std::fs::read_to_string(&out_path).expect("contract"),
        strata_api::openapi::to_pretty_json(&strata_api::openapi::document())
    );

    let out = run(stratad(None)
        .args(["openapi", "--out"])
        .arg(dir.path().join("missing/dir/openapi.json")));
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("stratad: "), "{}", stderr(&out));
}

#[tokio::test]
async fn bootstrap_roles_and_migrate_prepare_an_empty_database() {
    let db = TestDb::new_unmigrated().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(db.name(), dir.path(), "127.0.0.1:0");

    let out = run(stratad(Some(&config)).args(["bootstrap-roles", "--print"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let expected = strata_index::bootstrap::bootstrap_script(
        db.name(),
        &strata_index::bootstrap::RolePasswords {
            owner: std::env::var(ROLE_PASSWORD_ENV).ok(),
            app: std::env::var(ROLE_PASSWORD_ENV).ok(),
            accounts: std::env::var(ROLE_PASSWORD_ENV).ok(),
        },
    )
    .expect("script");
    let script: String = stdout(&out)
        .lines()
        .filter(|l| !l.starts_with("{\"timestamp\""))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(script.trim_end(), expected.trim_end());

    let out = run(stratad(Some(&config)).arg("bootstrap-roles"));
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        "stratad: bootstrap-roles needs --superuser-url (or STRATA_SUPERUSER_URL)\n"
    );

    let out = run(stratad(Some(&config))
        .arg("bootstrap-roles")
        .env("STRATA_SUPERUSER_URL", superuser_url("postgres")));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let out = run(stratad(Some(&config)).arg("migrate"));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let users: Option<String> = sqlx::query_scalar("SELECT to_regclass('strata.users')::text")
        .fetch_one(&db.superuser)
        .await
        .expect("query");
    assert_eq!(users.as_deref(), Some("users"));
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn create_user_verify_and_reindex_report_on_the_console() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(db.name(), dir.path(), "127.0.0.1:0");

    // The password comes from standard input, without its line ending.
    let out = run_with_stdin(
        stratad(Some(&config)).args(["create-user", "--username", "owner", "--admin"]),
        "owner-password-1\r\n",
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let lines = printed(&out);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let id = lines[0].strip_prefix("created ").expect("created <id>");
    let user = db
        .accounts_db
        .user_by_id(id.parse().expect("user id"))
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(
        (user.username.as_str(), user.display_name.as_str(), user.role),
        ("owner", "owner", strata_index::types::UserRole::Admin)
    );

    let out = run_with_stdin(
        stratad(Some(&config)).args([
            "create-user",
            "--username",
            "OWNER",
            "--display-name",
            "Other",
        ]),
        "owner-password-1\n",
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stderr(&out), "stratad: username taken\n");

    let out = run(stratad(Some(&config)).args(["verify", "--user", "owner"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(printed(&out), ["vault is consistent"]);

    let vault = dir.path().join("data/users").join(id).join("vault");
    std::fs::write(vault.join("notes/Outside.md"), "Hello.\n").expect("write");
    let out = run(stratad(Some(&config)).args(["verify", "--user", id]));
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        printed(&out),
        [
            "uncommitted change: notes/Outside.md",
            "note without an ID: notes/Outside.md",
        ]
    );
    assert_eq!(
        stderr(&out),
        "stratad: the vault needs reconciliation (it runs when stratad serve starts)\n"
    );

    let out = run(stratad(Some(&config)).args(["reindex", "--user", "owner"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(printed(&out), ["reindexed 1 notes"]);

    let out = run(stratad(Some(&config)).args(["reindex", "--user", "nobody"]));
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stderr(&out), "stratad: no such user: nobody\n");
    db.cleanup().await.expect("cleanup");
}

/// A free local port (the listener is closed again before `stratad` binds it).
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// Sends `GET <path>` and returns the status line.
fn get_status_line(port: u16, path: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/vnd.msgpack\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    Ok(response.trim_end().to_owned())
}

/// Waits for `child` to exit (up to `limit`) without polling.
fn wait_exit(mut child: Child, limit: Duration) -> (Option<i32>, String, String) {
    let mut out = child.stdout.take().expect("stdout");
    let mut err = child.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut o = String::new();
        let mut e = String::new();
        out.read_to_string(&mut o).expect("stdout");
        err.read_to_string(&mut e).expect("stderr");
        let status = child.wait().expect("wait");
        let _ = tx.send((status.code(), o, e));
    });
    rx.recv_timeout(limit).expect("stratad exits in time")
}

#[tokio::test]
async fn serve_answers_requests_and_stops_gracefully_on_sigterm() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let port = free_port();
    let config = write_config(db.name(), dir.path(), &format!("127.0.0.1:{port}"));
    let out = run(stratad(Some(&config)).arg("keygen"));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let mut child = stratad(Some(&config))
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve starts");
    // Read the JSON log until the server announces itself, keeping every line.
    let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
    let mut log = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).expect("log line");
        assert!(n > 0, "stratad exited before listening:\n{log}");
        log.push_str(&line);
        if line.contains("\"stratad listening\"") {
            break;
        }
    }
    // The socket is bound right after the announcement: retry refused connections until it
    // is (bounded).
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match get_status_line(port, "/api/v1/health") {
            Ok(line) => break line,
            Err(e) if Instant::now() < deadline => {
                assert_eq!(e.kind(), std::io::ErrorKind::ConnectionRefused, "{e}");
                std::thread::yield_now();
            }
            Err(e) => panic!("stratad never accepted a connection: {e}"),
        }
    };
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(
        get_status_line(port, "/api/v1/me?q=secret").expect("request"),
        "HTTP/1.1 401 Unauthorized"
    );

    let kill = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(kill.success());
    child.stdout = Some(reader.into_inner());
    let (code, rest, err) = wait_exit(child, Duration::from_secs(90));
    log.push_str(&rest);
    assert_eq!(code, Some(0), "stderr: {err}\nlog: {log}");

    let messages = log_messages(&log);
    for expected in [
        "job runner started",
        "stratad listening",
        "request",
        "stratad stopped",
    ] {
        assert!(
            messages.iter().any(|m| m == expected),
            "{expected:?} missing from {messages:?}"
        );
    }
    // Request lines carry the path without its query.
    let requests: Vec<(String, u64)> = log
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["fields"]["message"] == "request")
        .map(|v| {
            (
                v["fields"]["path"].as_str().expect("path").to_owned(),
                v["fields"]["status"].as_u64().expect("status"),
            )
        })
        .collect();
    assert_eq!(
        requests,
        [
            ("/api/v1/health".to_owned(), 200),
            ("/api/v1/me".to_owned(), 401)
        ]
    );
    assert!(!log.contains("secret"), "the query never reaches the log");
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn serve_fails_on_an_occupied_address_after_the_checks() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let occupied = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = occupied.local_addr().expect("addr").port();
    let config = write_config(db.name(), dir.path(), &format!("127.0.0.1:{port}"));
    let out = run(stratad(Some(&config)).arg("keygen"));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));

    let child = stratad(Some(&config))
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve starts");
    let (code, log, err) = wait_exit(child, Duration::from_secs(90));
    assert_eq!(code, Some(1), "{log}");
    assert_eq!(err, "stratad: Address already in use (os error 98)\n");
    let messages = log_messages(&log);
    assert_eq!(
        messages.iter().rev().take(2).rev().cloned().collect::<Vec<_>>(),
        ["stratad stopped", "stratad failed"]
    );
    drop(occupied);
    db.cleanup().await.expect("cleanup");
}

#[tokio::test]
async fn serve_refuses_a_missing_key_before_touching_the_network() {
    let db = TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(db.name(), dir.path(), "127.0.0.1:0");
    let out = run(stratad(Some(&config)).arg("serve"));
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stderr(&out),
        format!(
            "stratad: secret file {} is missing\n",
            dir.path().join("token.pem").display()
        )
    );
    db.cleanup().await.expect("cleanup");
}
