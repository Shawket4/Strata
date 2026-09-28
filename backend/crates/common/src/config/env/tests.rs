use std::collections::BTreeSet;

use pretty_assertions::assert_eq;

use super::*;
use crate::config::{Argon2Config, RateLimit};

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn from_env(list: &[(&str, &str)]) -> Result<Config, ConfigError> {
    from_vars(&EnvFile::none(), pairs(list))
}

fn file(text: &str) -> EnvFile {
    EnvFile::parse(Path::new("/etc/strata/stratad.env"), text).expect("parses")
}

/// A configuration in which every value differs from its default (every optional one set).
#[allow(clippy::too_many_lines)] // one line per setting
fn every_value_changed() -> Config {
    let mut c = Config::default();
    c.data_root = "/data/strata dir".into();
    c.bind = "[::1]:9443".parse().expect("addr");
    c.default_timezone = "Africa/Cairo".into();
    c.max_future_skew_secs = 60;
    c.database.owner_url = "postgres://strata_owner:o%24w'ner@db:5433/strata".into();
    c.database.app_url = "postgresql://strata_app:a#p p@db/strata?sslmode=disable".into();
    c.database.accounts_url = "postgres://strata_accounts:\"acc\"@db/strata".into();
    c.database.max_connections = 3;
    c.ai.default_provider = AiProviderKind::Disabled;
    c.ai.user_providers = [
        ("owner".to_owned(), AiProviderKind::ClaudeCli),
        ("guest".to_owned(), AiProviderKind::AnthropicApi),
    ]
    .into_iter()
    .collect();
    c.ai.daily_job_limit = 100;
    let cli = &mut c.ai.claude_cli;
    cli.command = ["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]
        .map(str::to_owned)
        .to_vec();
    cli.scratch_dir = "/tmp/scratch".into();
    cli.model = Some("opus".into());
    cli.max_concurrency = 2;
    cli.timeout_secs = 120;
    cli.kill_grace_secs = 9;
    cli.usage_limit_pause_secs = 600;
    let api = &mut c.ai.anthropic_api;
    api.api_key_file = Some("/etc/strata/anthropic.key".into());
    api.model = "claude-opus-5".into();
    api.base_url = "http://localhost:9000".into();
    api.effort = Some("high".into());
    api.max_retries = 5;
    api.timeout_secs = 30;
    api.input_micros_per_mtok = 1;
    api.output_micros_per_mtok = 2;
    let emb = &mut c.ai.embedding;
    emb.model_dir = Some("/opt/models/granite".into());
    emb.onnxruntime_lib = Some("/opt/onnxruntime/lib/libonnxruntime.so.1.30.0".into());
    emb.model_file = "onnx/model_quint8.onnx".into();
    emb.tokenizer_file = "tok.json".into();
    emb.model_id = "other@1".into();
    emb.dims = 768;
    emb.pooling = EmbeddingPooling::Mean;
    emb.max_tokens = 512;
    emb.max_batch_tokens = 1024;
    emb.pad_batches = false;
    emb.nice = 0;
    emb.idle_unload_secs = 10;
    c.thresholds.relation = 0.75;
    c.thresholds.custody = 1.0;
    for (i, t) in c.thresholds.dedupe.values_mut().enumerate() {
        t.near = 0.1 + f64::from(u8::try_from(i).expect("few kinds")) / 100.0;
        t.semantic = Some(0.95);
    }
    c.budgets.timezone = Some("Europe/Berlin".into());
    c.budgets.per_user_daily_tokens = 1;
    c.budgets.per_user_daily_cost_micros = 2;
    c.budgets.global_daily_tokens = 3;
    c.budgets.global_daily_cost_micros = 4;
    c.accounts.deletion_grace_days = 7;
    c.accounts.max_pending_signups = 3;
    c.accounts.purge_interval_secs = 30;
    c.push.fcm_service_account_path = Some("/etc/strata/fcm.json".into());
    c.push.apns_key_path = Some("/etc/strata/apns.p8".into());
    c.push.apns_key_id = Some("ABC123".into());
    c.push.apns_team_id = Some("1234567890".into());
    c.push.apns_topic = Some("app.strata".into());
    c.push.wns_credentials_path = Some("/etc/strata/wns.json".into());
    let auth = &mut c.auth;
    auth.signing_key_file = "/k.pem".into();
    auth.issuer = "iss".into();
    auth.audience = "aud".into();
    auth.access_token_ttl_secs = 600;
    auth.session_ttl_days = 30;
    auth.revocation_reload_secs = 5;
    auth.trust_forwarded_for = true;
    auth.min_password_length = 12;
    auth.argon2 = Argon2Config {
        memory_kib: 64,
        iterations: 3,
        parallelism: 2,
    };
    let limit = |max, window_secs| RateLimit { max, window_secs };
    auth.rate_limits.login_per_ip = limit(101, 102);
    auth.rate_limits.login_per_username = limit(103, 104);
    auth.rate_limits.signup_per_ip = limit(105, 106);
    auth.rate_limits.signup_global = limit(107, 108);
    auth.rate_limits.capture_per_user = limit(109, 110);
    auth.rate_limits.ask_per_user = limit(111, 112);
    let jobs = &mut c.jobs;
    jobs.max_concurrency = 1;
    jobs.poll_interval_secs = 2;
    jobs.idle_recheck_secs = 3;
    jobs.backoff_base_secs = 4;
    jobs.backoff_max_secs = 5;
    jobs.nightly_hour = 23;
    jobs.digest_weekday = "sun".into();
    jobs.shutdown_grace_secs = 6;
    c
}

/// The variable names are exactly the key paths of `Config` (serialised with every optional
/// value set), joined by `__` and upper-cased — one variable per setting, none missing.
#[test]
fn every_setting_has_exactly_one_variable_named_after_its_path() {
    fn leaves(prefix: &str, value: &serde_json::Value, out: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) if prefix != "ai.user_providers" => {
                for (key, v) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    leaves(&path, v, out);
                }
            }
            _ => {
                out.insert(format!(
                    "STRATA_{}",
                    prefix.replace('.', "__").to_ascii_uppercase()
                ));
            }
        }
    }
    let mut from_struct = BTreeSet::new();
    leaves(
        "",
        &serde_json::to_value(every_value_changed()).expect("serialise"),
        &mut from_struct,
    );
    let names = variables();
    let unique: BTreeSet<String> = names.iter().cloned().collect();
    assert_eq!(unique.len(), names.len(), "duplicate variable");
    assert_eq!(unique, from_struct);
    assert_eq!(names.len(), 103);
    assert_eq!(
        names[..5],
        [
            "STRATA_DATA_ROOT",
            "STRATA_BIND",
            "STRATA_DEFAULT_TIMEZONE",
            "STRATA_MAX_FUTURE_SKEW_SECS",
            "STRATA_DATABASE__OWNER_URL"
        ]
    );
}

/// Every variable parses into its setting: a config with every value changed survives
/// `to_vars` → `from_vars`, from the environment and through env-file text.
#[test]
fn every_variable_round_trips_through_the_environment_and_a_file() {
    let config = every_value_changed();
    let vars = to_vars(&config);
    assert_eq!(from_vars(&EnvFile::none(), vars.clone()), Ok(config.clone()));
    let text = to_env_file(&config);
    let parsed = file(&text);
    assert_eq!(parsed.vars, vars);
    assert_eq!(from_vars(&parsed, Vec::new()), Ok(config.clone()));
    // Each variable on its own changes exactly its setting, except the changes that are only
    // valid together with another one.
    let mut invalid_alone = Vec::new();
    for (name, value) in &vars {
        let Ok(single) = from_env(&[(name.as_str(), value.as_str())]) else {
            invalid_alone.push(name.as_str());
            continue;
        };
        let changed: Vec<String> = to_vars(&single)
            .into_iter()
            .zip(to_vars(&Config::default()))
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a.0)
            .collect();
        assert_eq!(changed, [name.clone()], "{name}");
    }
    assert_eq!(
        invalid_alone,
        [
            "STRATA_AI__USER_PROVIDERS",
            "STRATA_AI__EMBEDDING__MAX_BATCH_TOKENS",
            "STRATA_JOBS__BACKOFF_MAX_SECS",
        ]
    );
}

#[test]
fn values_are_written_the_documented_way() {
    let config = from_env(&[
        ("STRATA_BIND", "0.0.0.0:9000"),
        (
            "STRATA_AI__CLAUDE_CLI__COMMAND",
            "  sudo -n   -u strata-ai /usr/local/lib/strata/claude-ai ",
        ),
        (
            "STRATA_AI__USER_PROVIDERS",
            " owner = claude_cli ,kid=disabled,",
        ),
        ("STRATA_AI__EMBEDDING__PAD_BATCHES", "FALSE"),
        ("STRATA_THRESHOLDS__DEDUPE__ALIAS__SEMANTIC", "0.9"),
        ("STRATA_THRESHOLDS__DEDUPE__NOTE__SEMANTIC", ""),
        ("STRATA_THRESHOLDS__RELATION", "1"),
        ("STRATA_PUSH__APNS_TEAM_ID", "1234567890"),
        ("STRATA_AI__CLAUDE_CLI__MODEL", ""),
    ])
    .expect("valid");
    let mut expected = Config::default();
    expected.bind = "0.0.0.0:9000".parse().expect("addr");
    expected.ai.claude_cli.command = ["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]
        .map(str::to_owned)
        .to_vec();
    expected.ai.user_providers = [
        ("kid".to_owned(), AiProviderKind::Disabled),
        ("owner".to_owned(), AiProviderKind::ClaudeCli),
    ]
    .into_iter()
    .collect();
    expected.ai.embedding.pad_batches = false;
    expected.thresholds.dedupe.insert(
        "alias".into(),
        DedupeThreshold {
            near: 0.6,
            semantic: Some(0.9),
        },
    );
    expected.thresholds.dedupe.insert(
        "note".into(),
        DedupeThreshold {
            near: 0.6,
            semantic: None,
        },
    );
    expected.thresholds.relation = 1.0;
    expected.push.apns_team_id = Some("1234567890".into());
    assert_eq!(config, expected);
}

#[test]
fn the_environment_overrides_the_file_which_overrides_the_defaults() {
    let env_file = file(
        "# comment\n\
         STRATA_DATABASE__MAX_CONNECTIONS=3\n\
         STRATA_ACCOUNTS__DELETION_GRACE_DAYS=30 # trailing comment\n\
         export STRATA_DEFAULT_TIMEZONE=\"Africa/Cairo\"\n\
         \n\
         STRATA_DATABASE__APP_URL='postgres://strata_app:pa$$ #word@db/strata'\n",
    );
    let config = from_vars(
        &env_file,
        pairs(&[
            ("STRATA_ACCOUNTS__DELETION_GRACE_DAYS", "7"),
            ("STRATA_BIND", "127.0.0.1:7000"),
            ("UNRELATED", "ignored"),
            ("STRATA_TEST_DATABASE_URL", "not a setting, ignored"),
            ("STRATA_FUZZ_SEED", "ignored too"),
        ]),
    )
    .expect("valid");
    let mut expected = Config::default();
    expected.database.max_connections = 3;
    expected.accounts.deletion_grace_days = 7;
    expected.default_timezone = "Africa/Cairo".into();
    expected.database.app_url = "postgres://strata_app:pa$$ #word@db/strata".into();
    expected.bind = "127.0.0.1:7000".parse().expect("addr");
    assert_eq!(config, expected);
}

#[test]
fn malformed_values_name_the_variable_and_what_is_expected() {
    let value = |name: &str, raw: &str, message: &str| {
        assert_eq!(
            from_env(&[(name, raw)]),
            Err(ConfigError::Value {
                name: name.into(),
                message: message.into()
            }),
            "{name}={raw}"
        );
    };
    value(
        "STRATA_JOBS__MAX_CONCURRENCY",
        "many",
        "expected a whole number from 0 to 4294967295",
    );
    value(
        "STRATA_JOBS__MAX_CONCURRENCY",
        "-1",
        "expected a whole number from 0 to 4294967295",
    );
    value(
        "STRATA_BUDGETS__GLOBAL_DAILY_TOKENS",
        "1e6",
        "expected a whole number from 0 to 18446744073709551615",
    );
    value(
        "STRATA_AI__EMBEDDING__NICE",
        "low",
        "expected a whole number from -2147483648 to 2147483647",
    );
    value("STRATA_THRESHOLDS__RELATION", "high", "expected a number, e.g. 0.75");
    value("STRATA_THRESHOLDS__RELATION", "NaN", "expected a number, e.g. 0.75");
    value(
        "STRATA_THRESHOLDS__DEDUPE__NOTE__SEMANTIC",
        "x",
        "expected a number, e.g. 0.75, or nothing to leave it unset",
    );
    value("STRATA_AUTH__TRUST_FORWARDED_FOR", "yes", "expected true or false");
    value(
        "STRATA_BIND",
        "localhost:8080",
        "expected an IP address and port, e.g. 127.0.0.1:8080",
    );
    value(
        "STRATA_AI__DEFAULT_PROVIDER",
        "gpt",
        "expected one of claude_cli, anthropic_api, disabled",
    );
    value("STRATA_AI__EMBEDDING__POOLING", "max", "expected cls or mean");
    let providers = "expected comma-separated username=provider pairs, e.g. \
                     owner=claude_cli,guest=anthropic_api (providers: claude_cli, \
                     anthropic_api, disabled; each username once)";
    value("STRATA_AI__USER_PROVIDERS", "owner", providers);
    value("STRATA_AI__USER_PROVIDERS", "=claude_cli", providers);
    value("STRATA_AI__USER_PROVIDERS", "owner=gpt", providers);
    value(
        "STRATA_AI__USER_PROVIDERS",
        "owner=disabled,owner=claude_cli",
        providers,
    );
    // Missing (empty) required values say how to get the default back.
    value(
        "STRATA_DATABASE__MAX_CONNECTIONS",
        "",
        "must not be empty; remove the line to use the default `5`",
    );
    value(
        "STRATA_AUTH__SIGNING_KEY_FILE",
        "",
        "must not be empty; remove the line to use the default `/etc/strata/token-signing-key.pem`",
    );
    value(
        "STRATA_AI__CLAUDE_CLI__COMMAND",
        "",
        "must not be empty; remove the line to use the default `/usr/local/bin/claude`",
    );
}

#[test]
fn unknown_variables_are_errors_with_a_correction() {
    let env_file = file("STRATA_DATABSE__APP_URL=postgres://x@db/strata\n");
    assert_eq!(
        from_vars(&env_file, Vec::new()),
        Err(ConfigError::Unknown {
            name: "STRATA_DATABSE__APP_URL".into(),
            origin: "/etc/strata/stratad.env".into(),
            hint: Some("did you mean STRATA_DATABASE__APP_URL?".into()),
        })
    );
    assert_eq!(
        from_vars(&EnvFile::none(), Vec::new()).map(|_| ()),
        Ok(())
    );
    // In the file, every name must be a setting.
    for (text, name, hint) in [
        ("STRATA_BINDD=127.0.0.1:1\n", "STRATA_BINDD", Some("did you mean STRATA_BIND?")),
        ("STRATA_SOMETHING=1\n", "STRATA_SOMETHING", None),
        (
            "RUST_LOG=debug\n",
            "RUST_LOG",
            Some("only STRATA_ settings belong in the env file"),
        ),
    ] {
        assert_eq!(
            from_vars(&file(text), Vec::new()),
            Err(ConfigError::Unknown {
                name: name.into(),
                origin: "/etc/strata/stratad.env".into(),
                hint: hint.map(str::to_owned),
            }),
            "{text}"
        );
    }
    // In the environment, nested-looking names must be settings; others are ignored.
    assert_eq!(
        from_env(&[("STRATA_AI__CLAUDE_CLI__TYPO", "1")]),
        Err(ConfigError::Unknown {
            name: "STRATA_AI__CLAUDE_CLI__TYPO".into(),
            origin: "the environment".into(),
            hint: None,
        })
    );
    let old = from_env(&[("STRATA__DATABASE__APP_URL", "postgres://x@db/strata")])
        .expect_err("old prefix");
    assert_eq!(
        old.to_string(),
        "unknown variable STRATA__DATABASE__APP_URL in the environment; the old STRATA__ \
         prefix is now STRATA_ (STRATA_DATABASE__APP_URL)"
    );
    assert_eq!(
        from_env(&[("STRATA_BINDD", "x"), ("STRATA_CONFIG", "/etc/strata/stratad.toml")]),
        Ok(Config::default())
    );
}

#[test]
fn env_file_syntax_errors_give_the_line_and_never_its_content() {
    let path = Path::new("/srv/.env");
    let err = EnvFile::parse(
        path,
        "STRATA_BIND=127.0.0.1:1\n# ok\nSTRATA_DATABASE__APP_URL=postgres://a:secret pw@db/s\n",
    )
    .expect_err("unquoted space");
    assert_eq!(err, ConfigError::Syntax {
        path: path.into(),
        line: 3
    });
    let message = err.to_string();
    assert_eq!(
        message,
        "env file /srv/.env, line 3: expected NAME=value; quote a value that contains spaces, \
         `#`, `$` or quotes in single quotes (NAME='value')"
    );
    assert!(!message.contains("secret"));
    assert_eq!(
        EnvFile::parse(path, "STRATA_BIND=1\nSTRATA_AUTH__ISSUER='unterminated\n"),
        Err(ConfigError::Syntax {
            path: path.into(),
            line: 2
        })
    );
    assert_eq!(
        EnvFile::parse(path, "STRATA_BIND=1\nSTRATA_BIND=2\n"),
        Err(ConfigError::Duplicate {
            path: path.into(),
            name: "STRATA_BIND".into()
        })
    );
    // A byte-order mark is ignored.
    assert_eq!(
        EnvFile::parse(path, "\u{feff}STRATA_BIND=127.0.0.1:1\n").map(|f| f.vars),
        Ok(pairs(&[("STRATA_BIND", "127.0.0.1:1")]))
    );
}

/// No error message repeats a configured value, so a secret in a malformed or misnamed
/// variable never reaches the log or the terminal.
#[test]
fn errors_never_contain_values() {
    let secret = "hunter2-SECRET";
    let cases: Vec<Result<Config, ConfigError>> = vec![
        from_env(&[("STRATA_DATABASE__MAX_CONNECTIONS", secret)]),
        from_env(&[("STRATA_DATABASE__APP_URL", &format!("mysql://u:{secret}@db/s"))]),
        from_env(&[("STRATA_DATABASE__APP_URLL", &format!("postgres://u:{secret}@db/s"))]),
        from_env(&[("STRATA_AI__USER_PROVIDERS", secret)]),
        from_env(&[("STRATA_BIND", secret)]),
        from_vars(
            &file(&format!("STRATA_DATABASE__PASSWORD='{secret}'\n")),
            Vec::new(),
        ),
        EnvFile::parse(Path::new("/e"), &format!("STRATA_X=a {secret}\n")).map(|_| Config::default()),
    ];
    for case in cases {
        let err = case.expect_err("error");
        assert!(!err.to_string().contains(secret), "{err}");
        assert!(!format!("{err:?}").contains(secret), "{err:?}");
    }
}

#[test]
fn load_reads_the_named_file_and_reports_a_missing_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stratad.env");
    std::fs::write(&path, "STRATA_ACCOUNTS__MAX_PENDING_SIGNUPS=7\n").expect("write");
    let config = load(Some(&path)).expect("valid");
    assert_eq!(config.accounts.max_pending_signups, 7);
    let missing = dir.path().join("missing.env");
    assert_eq!(
        load(Some(&missing)),
        Err(ConfigError::Read {
            path: missing.clone(),
            message: "No such file or directory (os error 2)".into()
        })
    );
    assert_eq!(
        load(Some(&missing)).expect_err("missing").to_string(),
        format!(
            "cannot read env file {}: No such file or directory (os error 2)",
            missing.display()
        )
    );
}

/// `deploy/stratad.env.example` loads to the defaults and documents every variable once,
/// either set to its default or commented out (`# NAME=value`, where the value is the default
/// whenever the setting has one).
#[test]
fn example_file_documents_every_variable_with_its_default() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../deploy/stratad.env.example"
    );
    let text = std::fs::read_to_string(path).expect("example file");
    let example = EnvFile::parse(Path::new(path), &text).expect("parses");
    assert_eq!(from_vars(&example, Vec::new()), Ok(Config::default()));
    let defaults: HashMap<String, String> = to_vars(&Config::default()).into_iter().collect();
    let mut documented = Vec::new();
    for line in text.lines() {
        let line = line.trim_start_matches("# ");
        if !line.starts_with(PREFIX) || !line.contains('=') {
            continue;
        }
        let parsed = EnvFile::parse(Path::new(path), line).expect("documented line parses");
        let [(name, value)] = parsed.vars.as_slice() else {
            panic!("one variable per line: {line}");
        };
        let default = &defaults[name];
        if !default.is_empty() {
            assert_eq!(value, default, "{name} documents a wrong default");
        }
        documented.push(name.clone());
    }
    let mut sorted = documented.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), documented.len(), "a variable is documented twice");
    assert_eq!(documented, variables(), "documented in the variable order");
}

#[test]
fn quoting_and_url_redaction() {
    assert_eq!(quote("/srv/strata"), "/srv/strata");
    assert_eq!(quote(""), "");
    assert_eq!(quote("a b"), "'a b'");
    assert_eq!(quote("it's $x \"q\" \\"), "\"it's \\$x \\\"q\\\" \\\\\"");
    for value in ["a b", "it's $x \"q\" \\", "#x", "p$$w"] {
        let parsed = file(&format!("STRATA_AUTH__ISSUER={}\n", quote(value)));
        assert_eq!(parsed.vars, pairs(&[("STRATA_AUTH__ISSUER", value)]), "{value}");
    }
    assert_eq!(
        redact_url("postgres://strata_app:pa:ss@db:5432/strata"),
        "postgres://strata_app:***@db:5432/strata"
    );
    assert_eq!(
        redact_url("postgres://strata_app@db/strata"),
        "postgres://strata_app@db/strata"
    );
    assert_eq!(
        redact_url("postgres://db/strata?user=a@b"),
        "postgres://db/strata?user=a@b"
    );
    assert_eq!(redact_url("not a url"), "not a url");
}
