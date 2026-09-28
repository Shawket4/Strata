//! Schema-driven fuzzer (PLAN §7.6, §16.3): requests for every operation of the production
//! contract generated from its OpenAPI schemas — valid bodies, query strings and path
//! parameters (fixture IDs, random ULIDs, hostile text), and mutated ones (dropped or
//! mistyped members, unknown members, huge strings, depth bombs, lying MessagePack headers,
//! truncation, trailing bytes, wrong content type, random bytes) — sent to the complete
//! production composition as an authenticated user.
//!
//! Every response must be below 500, conform to the contract for its operation, never carry
//! another user's data, and never reveal the server's data root.
//!
//! Deterministic: a fixed seed (`ChaCha`) and operations visited round-robin, so a failure
//! reproduces with the same seed and case count. Normal runs use `STRATA_FUZZ_CASES` = 4 per
//! operation; long runs set it (and optionally `STRATA_FUZZ_SEED`), e.g.
//! `STRATA_FUZZ_CASES=20000 cargo test -p strata-api --test schema_fuzz -- --nocapture`.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod hardening;

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::sync::Arc;
use std::time::Instant;

use hardening::generate::{self, Ctx, Mutation, Pools};
use hardening::http::{Req, encode};
use hardening::ops::{self, Body, Loc, Op};
use hardening::{H, Options, path_kind, populate};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use rmpv::Value as M;
use strata_api::contract::Contract;
use strata_common::config::RateLimit;

const DEFAULT_SEED: u64 = 0x5354_5241_5441_0016;

/// A generated request body.
#[derive(Debug, Clone)]
enum BodyCase {
    None,
    MsgPack(M, Mutation),
    Zip(Vec<(String, String)>),
    Raw(Vec<u8>),
}

/// One generated request (before tokens).
#[derive(Debug, Clone)]
struct Case {
    path: BTreeMap<String, String>,
    query: Vec<(String, String)>,
    if_match: Option<String>,
    body: BodyCase,
}

fn garbage() -> BoxedStrategy<String> {
    generate::text().prop_map(|t| encode(&t)).boxed()
}

fn path_param(pools: &Pools, kind: Option<&str>) -> BoxedStrategy<String> {
    let known: Vec<String> = kind
        .map(|k| pools.get(k).iter().map(|v| encode(v)).collect())
        .unwrap_or_default();
    let ulid = generate::ulid_for(pools, kind.unwrap_or("note"))
        .prop_map(|v| encode(&v))
        .boxed();
    if known.is_empty() {
        prop_oneof![3 => ulid, 1 => garbage()].boxed()
    } else {
        prop_oneof![
            6 => proptest::sample::select(known),
            2 => ulid,
            1 => garbage(),
        ]
        .boxed()
    }
}

fn scalar(v: &M) -> String {
    match v {
        M::String(s) => s.as_str().unwrap_or_default().to_owned(),
        M::Nil => String::new(),
        M::Array(items) => items.iter().map(scalar).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

fn query_param(ctx: &Arc<Ctx>, p: &ops::Param) -> BoxedStrategy<Option<(String, String)>> {
    let name = p.name.clone();
    let value = prop_oneof![
        6 => generate::strategy(ctx, &p.schema, &p.name, 0).prop_map(|v| encode(&scalar(&v))),
        1 => garbage(),
    ];
    let pair = value.prop_map(move |v| Some((name.clone(), v)));
    if p.required {
        pair.boxed()
    } else {
        prop_oneof![2 => Just(None), 3 => pair].boxed()
    }
}

fn zip_case() -> BoxedStrategy<BodyCase> {
    let entry = (
        prop_oneof![
            4 => ("[A-Za-z][A-Za-z0-9 ]{0,8}").prop_map(|w| format!("notes/{w}.md")),
            1 => generate::text(),
            1 => proptest::sample::select(vec![
                "../x.md".to_owned(), "/abs.md".to_owned(), ".obsidian/app.json".to_owned(),
                ".meta/notes/x.json".to_owned(), "notes\\x.md".to_owned(), "C:/x.md".to_owned(),
            ]),
        ],
        generate::text(),
    );
    prop_oneof![
        4 => prop::collection::vec(entry, 0..4).prop_map(BodyCase::Zip),
        1 => prop::collection::vec(any::<u8>(), 0..64).prop_map(BodyCase::Raw),
    ]
    .boxed()
}

fn case_strategy(ctx: &Arc<Ctx>, op: &Op) -> BoxedStrategy<Case> {
    let path: Vec<BoxedStrategy<(String, String)>> = op
        .path_params()
        .map(|p| {
            let name = p.name.clone();
            path_param(&ctx.pools, path_kind(op, &p.name))
                .prop_map(move |v| (name.clone(), v))
                .boxed()
        })
        .collect();
    let query: Vec<BoxedStrategy<Option<(String, String)>>> = op
        .params
        .iter()
        .filter(|p| p.loc == Loc::Query)
        .map(|p| query_param(ctx, p))
        .collect();
    let versions = ctx.pools.get("version").to_vec();
    let if_match = if op.params.iter().any(|p| p.name == "If-Match") && !versions.is_empty() {
        prop_oneof![
            2 => Just(None),
            3 => proptest::sample::select(versions).prop_map(Some),
            1 => garbage().prop_map(Some),
        ]
        .boxed()
    } else {
        Just(None).boxed()
    };
    let body = match &op.body {
        None => Just(BodyCase::None).boxed(),
        Some(Body::Zip) => zip_case(),
        Some(Body::MsgPack(schema)) => {
            (generate::strategy(ctx, schema, "", 0), generate::mutation())
                .prop_map(|(v, m)| BodyCase::MsgPack(v, m))
                .boxed()
        }
    };
    (path, query, if_match, body)
        .prop_map(|(path, query, if_match, body)| Case {
            path: path.into_iter().collect(),
            query: query.into_iter().flatten().collect(),
            if_match,
            body,
        })
        .boxed()
}

fn zip_bytes(entries: &[(String, String)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        if z.start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .is_ok()
        {
            let _ = z.write_all(content.as_bytes());
        }
    }
    z.finish().map(Cursor::into_inner).unwrap_or_default()
}

fn to_request(op: &Op, case: &Case, token: &str) -> (Req, &'static str) {
    let path = op.fill(|name| case.path.get(name).cloned().unwrap_or_default());
    let target = if case.query.is_empty() {
        path
    } else {
        let q: Vec<String> = case.query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{path}?{}", q.join("&"))
    };
    let mut req = Req::new(&op.method, target).token(token);
    req.upgrade = op.stream;
    if let Some(v) = &case.if_match {
        req = req.header("If-Match", v);
    }
    let kind = match &case.body {
        BodyCase::None => "none",
        BodyCase::MsgPack(v, m) => {
            let (ct, bytes) = generate::apply(v, m);
            req = req.body(&ct, bytes);
            if *m == Mutation::None {
                "valid"
            } else {
                "mutated"
            }
        }
        BodyCase::Zip(entries) => {
            req = req.body(strata_api::wire::ZIP, zip_bytes(entries));
            "zip"
        }
        BodyCase::Raw(bytes) => {
            req = req.body(strata_api::wire::ZIP, bytes.clone());
            "raw"
        }
    };
    (req, kind)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

#[tokio::test]
async fn every_operation_survives_schema_driven_fuzzing() {
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        import: strata_vault::ImportLimits {
            max_entries: 20,
            max_entry_bytes: 64 * 1024,
            max_total_bytes: 256 * 1024,
        },
        recluster: RateLimit {
            max: 1_000_000,
            window_secs: 60,
        },
    })
    .await;
    let fuzzer = h.user("fuzzer").await;
    let bystander = h.user("bystander").await;
    let admin = h.admin("root").await;
    let victim = h.user("victim").await;
    let fx = populate(&h, &fuzzer, "fuzzer-own-text").await;
    let other = populate(&h, &bystander, "bystander-secret-5e7d").await;
    let private = other.private_strings();
    let mut pools = fx.pools();
    pools
        .by_kind
        .insert("user".to_owned(), vec![victim.id.to_string()]);
    pools.add("version", fx.note.version.clone());
    pools.add("version", fx.task.version.clone());
    let contract = Contract::production();
    let ctx = Arc::new(Ctx {
        doc: contract.document().clone(),
        pools,
    });
    let all = ops::operations(&contract);
    let strategies: Vec<BoxedStrategy<Case>> =
        all.iter().map(|op| case_strategy(&ctx, op)).collect();
    let seed = env_u64("STRATA_FUZZ_SEED").unwrap_or(DEFAULT_SEED);
    let cases = env_u64("STRATA_FUZZ_CASES")
        .map_or(all.len() * 4, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut runner = TestRunner::new_with_rng(
        Config::default(),
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    );
    let mut token = fuzzer.token.clone();
    let mut stats: BTreeMap<String, BTreeMap<u16, u32>> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, u32> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut relogins = 0;
    let started = Instant::now();
    for i in 0..cases {
        let op = &all[i % all.len()];
        let case = strategies[i % all.len()]
            .new_tree(&mut runner)
            .expect("generate")
            .current();
        let caller = if op.is_admin() {
            admin.token.clone()
        } else if op.id == "logout" {
            h.login(&fuzzer.name, &fuzzer.password).await
        } else {
            token.clone()
        };
        let (req, kind) = to_request(op, &case, &caller);
        *kinds.entry(kind).or_default() += 1;
        let resp = h.send(Some(&op.id), &req).await;
        *stats
            .entry(op.id.clone())
            .or_default()
            .entry(resp.status)
            .or_default() += 1;
        let repro = || {
            let body = req.body.as_ref().map(|(ct, b)| {
                let hex = hex::encode(&b[..b.len().min(256)]);
                format!("{ct} {hex}")
            });
            format!(
                "case {i} (seed {seed:#x}) {} {} if-match={:?} body={body:?}",
                req.method, req.target, case.if_match
            )
        };
        if resp.status >= 500 {
            failures.push(format!(
                "{} → {} {:?}: {}",
                op.id,
                resp.status,
                resp.problem_type(),
                repro()
            ));
        }
        let texts = hardening::strings_in(&resp.body);
        for t in &texts {
            if let Some(p) = private.iter().find(|p| t.contains(p.as_str())) {
                failures.push(format!(
                    "{} leaked bystander data {p:?}: {}",
                    op.id,
                    repro()
                ));
            }
            let mut echo = req.target.as_bytes().to_vec();
            echo.extend(hardening::http::decode(&req.target));
            if let Some((_, b)) = &req.body {
                echo.extend_from_slice(b);
            }
            let root = h.data.path().to_string_lossy();
            if t.contains(root.as_ref()) {
                failures.push(format!("{} revealed the data root: {}", op.id, repro()));
            }
        }
        // A generated request may legitimately end the fuzzer's own session (e.g. removing
        // its current device); sign in again and continue.
        if resp.status == 401 && op.secured && !op.is_admin() && op.id != "logout" {
            let check = h
                .send(None, &Req::new("GET", "/api/v1/me").token(&token))
                .await;
            if check.status == 401 {
                token = h.login(&fuzzer.name, &fuzzer.password).await;
                relogins += 1;
            }
        }
    }
    let elapsed = started.elapsed();
    let mut total_by_class: BTreeMap<String, u32> = BTreeMap::new();
    for (op, statuses) in &stats {
        let line: Vec<String> = statuses.iter().map(|(s, n)| format!("{s}×{n}")).collect();
        eprintln!("fuzz {op}: {}", line.join(" "));
        for (s, n) in statuses {
            *total_by_class.entry(format!("{}xx", s / 100)).or_default() += n;
        }
    }
    let ok_ops = stats
        .values()
        .filter(|s| s.keys().any(|k| (200..300).contains(k) || *k == 101))
        .count();
    eprintln!(
        "fuzz summary: seed {seed:#x}, {cases} cases over {} operations in {:.1}s; by class {total_by_class:?}; bodies {kinds:?}; operations with a success {ok_ops}; re-logins {relogins}",
        all.len(),
        elapsed.as_secs_f64()
    );
    assert_eq!(failures, Vec::<String>::new());
    assert_eq!(h.conformance.violations(), Vec::<String>::new());
    assert_eq!(stats.len(), all.len(), "every operation was fuzzed");
    h.finish().await;
}

/// Regression (found by this fuzzer; seeds and cases below): a NUL character in a name,
/// alias, tag, field, task text or reply reached `PostgreSQL` (which cannot store it) and
/// answered `500`. Every such input is now `422 invalid_body` and nothing is written; a
/// username with NUL is an unknown user (`401 invalid_credentials`).
#[tokio::test]
async fn regression_nul_characters_in_text_are_422_not_500() {
    fn mp(v: &M) -> Vec<u8> {
        let mut o = Vec::new();
        rmpv::encode::write_value(&mut o, v).expect("encode");
        o
    }
    fn map(e: &[(&str, M)]) -> M {
        M::Map(e.iter().map(|(k, v)| (M::from(*k), v.clone())).collect())
    }
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let u = h.user("alice").await;
    let fx = populate(&h, &u, "alice text").await;
    let z = "a\u{0}b";
    let list = |v: &str| M::Array(vec![M::from(v)]);
    let place = format!("/api/v1/places/{}", fx.place);
    let person = format!("/api/v1/entities/{}", fx.person);
    let reply = format!("/api/v1/suggestions/{}/reply", fx.suggestion);
    let task = format!("/api/v1/tasks/{}", fx.task.id);
    let cases: Vec<(&str, &str, &str, M)> = vec![
        (
            "create_place",
            "POST",
            "/api/v1/places",
            map(&[("name", M::from(z))]),
        ),
        (
            "create_place",
            "POST",
            "/api/v1/places",
            map(&[("name", M::from("A")), ("aliases", list(z))]),
        ),
        (
            "create_place",
            "POST",
            "/api/v1/places",
            map(&[("name", M::from("A")), ("tags", list(z))]),
        ),
        (
            "create_entity",
            "POST",
            "/api/v1/entities",
            map(&[("kind", M::from("person")), ("name", M::from(z))]),
        ),
        (
            "create_entity",
            "POST",
            "/api/v1/entities",
            map(&[
                ("kind", M::from("person")),
                ("name", M::from("F")),
                ("fields", map(&[("role", M::from(z))])),
            ]),
        ),
        (
            "create_document",
            "POST",
            "/api/v1/documents",
            map(&[("name", M::from("G")), ("doc_type", M::from(z))]),
        ),
        ("patch_place", "PATCH", &place, map(&[("name", M::from(z))])),
        (
            "patch_entity",
            "PATCH",
            &person,
            map(&[("aliases", list(z))]),
        ),
        ("patch_entity", "PATCH", &person, map(&[("tags", list(z))])),
        (
            "patch_entity",
            "PATCH",
            &person,
            map(&[("set_fields", map(&[("role", M::from(z))]))]),
        ),
        (
            "reply_suggestion",
            "POST",
            &reply,
            map(&[("body", M::from(z))]),
        ),
        // Seed 0x12d687, case 1494: a forced task create.
        (
            "create_task",
            "POST",
            "/api/v1/tasks",
            map(&[("text", M::from("\u{0} rent")), ("force", M::from(true))]),
        ),
        ("patch_task", "PATCH", &task, map(&[("text", M::from(z))])),
    ];
    let log = h.log(u.id);
    let mut got = Vec::new();
    for (op, method, path, body) in &cases {
        let resp = h
            .send(
                Some(op),
                &Req::new(method, *path).token(&u.token).msgpack(mp(body)),
            )
            .await;
        got.push((
            *op,
            resp.status,
            resp.problem_type(),
            rmp_serde::from_slice::<strata_client::types::Problem>(&resp.body)
                .ok()
                .and_then(|p| p.detail),
        ));
    }
    let expected: Vec<_> = cases
        .iter()
        .map(|(op, ..)| {
            (
                *op,
                422,
                Some("invalid_body".to_owned()),
                Some("text must not contain NUL characters".to_owned()),
            )
        })
        .collect();
    assert_eq!(got, expected);
    assert_eq!(h.log(u.id), log, "nothing written");
    // Seed 0x16, case 4748: a username with NUL is simply unknown.
    let login = h
        .send(
            Some("login"),
            &Req::new("POST", "/api/v1/auth/login").msgpack(mp(&map(&[
                ("username", M::from("alice\u{0}")),
                ("password", M::from("whatever-password")),
                ("device_name", M::from("d")),
                ("platform", M::from("android")),
            ]))),
        )
        .await;
    assert_eq!(
        (login.status, login.problem_type()),
        (401, Some("invalid_credentials".to_owned()))
    );
    h.finish().await;
}

/// Regression (found by this fuzzer, seed `0x5354524154410016`): `POST /tasks` with a
/// `note_id` that names no note of the caller answers `404 not_found`, which the contract did
/// not document (a conformance violation).
#[tokio::test]
async fn regression_create_task_documents_404_for_an_unknown_note() {
    let h = H::with(Options::default()).await;
    let u = h.user("alice").await;
    let body = {
        let mut o = Vec::new();
        rmpv::encode::write_value(
            &mut o,
            &M::Map(vec![
                (M::from("text"), M::from("Pay the rent")),
                (M::from("note_id"), M::from(generate::FALLBACK_ULID)),
            ]),
        )
        .expect("encode");
        o
    };
    let resp = h
        .send(
            Some("create_task"),
            &Req::new("POST", "/api/v1/tasks")
                .token(&u.token)
                .msgpack(body),
        )
        .await;
    assert_eq!(
        (resp.status, resp.problem_type()),
        (404, Some("not_found".to_owned()))
    );
    assert_eq!(h.conformance.violations(), Vec::<String>::new());
    h.finish().await;
}
