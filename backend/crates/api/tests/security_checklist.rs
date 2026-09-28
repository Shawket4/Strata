//! PLAN §15 security checklist: one dedicated test per item, run against the complete
//! production composition (`hardening` harness). Operations and parameters are read from the
//! OpenAPI document, so an operation added later is swept without editing this file.
//!
//! | §15 item | test |
//! |---|---|
//! | no raw filesystem paths outside the vault | `no_operation_returns_a_filesystem_path` |
//! | path traversal on every path parameter | `path_traversal_on_every_parameter_is_refused` |
//! | import zip: symlinks, absolute paths, `..`, oversized | `import_rejects_hostile_archives_whole`, `import_stops_zip_bombs_while_decompressing` |
//! | rate limits on login, signup, capture, ask | `rate_limit_login`, `rate_limit_signup`, `rate_limit_capture`, `rate_limit_ask` |
//! | content never in logs or error messages | `content_never_reaches_logs_or_error_messages` |
//! | secrets file permission check at startup | `startup_refuses_secret_files_open_to_others` |
//! | MessagePack decode limits | `msgpack_decode_limits_hold_on_every_body` |
//! | tenant isolation, every endpoint | `tenant_isolation_sweep_answers_404_for_foreign_ids` |
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod hardening;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex, OnceLock};

use hardening::generate::{self, field_kind};
use hardening::http::{Req, Resp, encode};
use hardening::ops::{self, Body, Loc, Op};
use hardening::{Fixtures, H, Options, User, canvas, path_kind, populate};
use pretty_assertions::assert_eq;
use rmpv::Value as M;
use strata_api::contract::Contract;
use strata_client::{operations as api, types};
use strata_common::config::RateLimit;

/// Operations this suite drives as their own session (they end it).
const SESSION_ENDING: [&str; 2] = ["logout", "confirm_deletion"];

fn msgpack(v: &M) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, v).expect("encode");
    out
}

fn map(entries: &[(&str, M)]) -> M {
    M::Map(
        entries
            .iter()
            .map(|(k, v)| (M::from(*k), v.clone()))
            .collect(),
    )
}

/// A minimal valid request for `op` as `u`, addressing `fx`'s objects.
fn request(doc: &serde_json::Value, op: &Op, u: &User, fx: &Fixtures) -> Req {
    let path = op.fill(|name| {
        path_kind(op, name).map_or_else(|| "unknown".to_owned(), |k| fx.id_of(k))
    });
    let ids = |field: &str| Some(fx.id_of(field_kind(field)));
    let mut query = Vec::new();
    for p in op.params.iter().filter(|p| p.loc == Loc::Query && p.required) {
        let v = generate::example(doc, &p.schema, &p.name, &ids);
        query.push(format!("{}={}", p.name, encode(&scalar(&v))));
    }
    let target = if query.is_empty() {
        path
    } else {
        format!("{path}?{}", query.join("&"))
    };
    let mut req = Req::new(&op.method, target).token(&u.token);
    req.upgrade = op.stream;
    if op.params.iter().any(|p| p.name == "If-Match") {
        if let Some(v) = if_match(op, fx) {
            req = req.header("If-Match", &v);
        }
    }
    match &op.body {
        Some(Body::MsgPack(schema)) => {
            let body = override_body(op, fx).unwrap_or_else(|| {
                generate::example(doc, schema, "", &ids)
            });
            req = req.msgpack(msgpack(&body));
        }
        Some(Body::Zip) => {
            req = req.body(strata_api::wire::ZIP, zip_of(&[("notes/Imported.md", b"Imported.\n")]));
        }
        None => {}
    }
    req
}

/// The query-string text of a scalar example.
fn scalar(v: &M) -> String {
    match v {
        M::String(s) => s.as_str().unwrap_or_default().to_owned(),
        M::Nil => String::new(),
        other => other.to_string(),
    }
}

fn if_match(op: &Op, fx: &Fixtures) -> Option<String> {
    match path_kind(op, "id") {
        Some("note") => Some(fx.note.version.clone()),
        Some("task") => Some(fx.task.version.clone()),
        _ => None,
    }
}

/// Bodies the minimal example would not make succeed.
fn override_body(op: &Op, fx: &Fixtures) -> Option<M> {
    Some(match op.id.as_str() {
        "put_map" => map(&[("content", M::from(canvas("notes/Plan.md")))]),
        "update_note" => map(&[("content", M::from("Rewritten.\n"))]),
        "move_note" => map(&[("new_path", M::from("notes/Moved.md"))]),
        "revert_note" => map(&[("commit", M::from(fx.commit.as_str()))]),
        "merge_entity" => map(&[("into_id", M::from(fx.company.to_string()))]),
        "add_custody_event" => map(&[
            ("type", M::from("stored-at")),
            ("at", M::from("2026-09-02")),
            ("place_id", M::from(fx.place.to_string())),
        ]),
        _ => return None,
    })
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        z.start_file(*name, options).expect("entry");
        z.write_all(bytes).expect("write");
    }
    z.finish().expect("zip").into_inner()
}

/// Order that keeps fixtures alive as long as possible: reads, writes, then deletions.
fn phase(op: &Op) -> u8 {
    if SESSION_ENDING.contains(&op.id.as_str()) {
        3
    } else if op.method == "DELETE" || op.id.contains("merge") || op.id == "move_note" {
        2
    } else if op.method == "GET" {
        0
    } else {
        1
    }
}

fn scan(h: &H, what: &str, resp: &Resp, leaks: &mut Vec<String>, forbidden: &[String]) {
    let mut texts = hardening::strings_in(&resp.body);
    texts.extend(resp.headers.iter().map(|(_, v)| v.clone()));
    if resp.content_type() == Some(strata_api::wire::ZIP) {
        // Archive entry names are vault-relative; the bytes are the user's own files.
        texts.clear();
    }
    for t in &texts {
        for leak in hardening::path_leaks(t, h.data.path()) {
            leaks.push(format!("{what}: {leak}"));
        }
        for f in forbidden {
            if t.contains(f.as_str()) {
                leaks.push(format!("{what}: forbidden text {f:?}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// §15: no endpoint returns raw filesystem paths outside the vault
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn no_operation_returns_a_filesystem_path() {
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let fx = populate(&h, &alice, "alice-secret").await;
    let admin = h.admin("root").await;
    let victim = h.user("victim").await;
    let contract = Contract::production();
    let doc = contract.document().clone();
    let mut all = ops::operations(&contract);
    all.sort_by_key(|op| (phase(op), op.id.clone()));
    let mut leaks = Vec::new();
    let mut covered = BTreeMap::new();
    for op in &all {
        let caller = if op.is_admin() { &admin } else { &alice };
        let mut req = request(&doc, op, caller, &fx);
        if SESSION_ENDING.contains(&op.id.as_str()) {
            req.token = Some(h.login(&alice.name, &alice.password).await);
        }
        if op.is_admin() {
            let path = op.fill(|_| victim.id.to_string());
            req.target = match req.target.split_once('?') {
                Some((_, q)) => format!("{path}?{q}"),
                None => path,
            };
        }
        let resp = h.send(Some(&op.id), &req).await;
        assert!(resp.status < 500, "{} → {}", op.id, resp.status);
        scan(&h, &op.id, &resp, &mut leaks, &[]);
        covered.insert(op.id.clone(), resp.status);
    }
    // Error paths that name files: a missing path, an invalid path, a taken path, a stale
    // version, a broken archive, a canvas pointing outside the vault.
    let extra: Vec<(&str, Req)> = vec![
        ("get_note_by_path", Req::new("GET", format!("/api/v1/notes/by-path?path={}", encode("notes/Missing.md")))),
        ("get_note_by_path", Req::new("GET", format!("/api/v1/notes/by-path?path={}", encode("../../etc/passwd")))),
        ("create_note", Req::new("POST", "/api/v1/notes").msgpack(msgpack(&map(&[("path", M::from("notes/Plan.md")), ("content", M::from("x"))])))),
        ("create_note", Req::new("POST", "/api/v1/notes").msgpack(msgpack(&map(&[("path", M::from("../outside.md")), ("content", M::from("x"))])))),
        ("create_note", Req::new("POST", "/api/v1/notes").msgpack(msgpack(&map(&[("path", M::from("/etc/cron.d/x.md")), ("content", M::from("x"))])))),
        ("put_map", Req::new("PUT", "/api/v1/maps/Outside").msgpack(msgpack(&map(&[("content", M::from(canvas("../../../etc/passwd")))])))),
        ("import_vault", Req::new("POST", "/api/v1/import").body(strata_api::wire::ZIP, zip_of(&[("../evil.md", b"x")]))),
    ];
    for (op, req) in extra {
        let resp = h.send(Some(op), &req.token(&alice.token)).await;
        assert!(resp.status < 500, "{op} → {}", resp.status);
        scan(&h, op, &resp, &mut leaks, &[]);
    }
    for (op, status) in &covered {
        eprintln!("paths-scan {op} → {status}");
    }
    assert_eq!(leaks, Vec::<String>::new());
    assert_eq!(
        covered.keys().cloned().collect::<BTreeSet<_>>(),
        all.iter().map(|o| o.id.clone()).collect::<BTreeSet<_>>(),
        "every documented operation was called"
    );
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: path traversal tests on every path parameter (and every query/header parameter)
// ---------------------------------------------------------------------------------------------

/// Text planted outside every vault; no response may ever contain it.
const OUTSIDE: &str = "OUTSIDE-THE-VAULT-7c1e";

/// Hostile parameter values, as they go on the wire (label, text).
fn attacks(data_root: &std::path::Path, bob: strata_common::UserId) -> Vec<(&'static str, String)> {
    vec![
        ("dotdot", "../../../secret-outside.md".to_owned()),
        ("dotdot-encoded", "..%2F..%2F..%2Fsecret-outside.md".to_owned()),
        ("dot-encoded", "%2e%2e%2f%2e%2e%2f%2e%2e%2fsecret-outside.md".to_owned()),
        ("dot-encoded-mixed", "%2e%2e/%2e%2e/secret-outside.md".to_owned()),
        ("double-encoded", "%252e%252e%252fsecret-outside.md".to_owned()),
        ("backslash", "..\\..\\..\\secret-outside.md".to_owned()),
        ("backslash-encoded", "..%5C..%5C..%5Csecret-outside.md".to_owned()),
        ("absolute", "%2Fetc%2Fpasswd".to_owned()),
        ("absolute-data-root", encode(&data_root.join("secret-outside.md").to_string_lossy())),
        ("drive", "C:%5CWindows%5Cwin.ini".to_owned()),
        ("nul", "notes%2FPlan.md%00.png".to_owned()),
        ("nul-dotdot", "..%00%2F..%2Fsecret-outside.md".to_owned()),
        ("fullwidth-dots", "%EF%BC%8E%EF%BC%8E%EF%BC%8Fsecret-outside.md".to_owned()),
        ("two-dot-leader", "%E2%80%A5%2Fsecret-outside.md".to_owned()),
        ("division-slash", "..%E2%88%95..%E2%88%95secret-outside.md".to_owned()),
        ("overlong-utf8", "%C0%AE%C0%AE%C0%AFsecret-outside.md".to_owned()),
        ("notes-escape", encode("notes/../../../secret-outside.md")),
        ("neighbour-vault", encode(&format!("../../{bob}/vault/notes/Plan.md"))),
    ]
}

/// What a hostile value may produce, by parameter.
fn allowed(op: &Op, param: &ops::Param) -> BTreeSet<u16> {
    match param.loc {
        // A malformed or unknown ID/name is not found (also when `..` changes the route).
        Loc::Path => [404].into(),
        // Free text is searched for, and finds nothing.
        Loc::Query if ["q", "tag"].contains(&param.name.as_str()) => [200].into(),
        // A vault path or an ID that does not exist.
        Loc::Query if op.statuses.contains(&404) => [404, 422].into(),
        Loc::Query => [422].into(),
        // An unknown version: a conflict, or 404 for an ID the attack replaced.
        Loc::Header => [409, 412, 422].into(),
    }
}

#[tokio::test]
async fn path_traversal_on_every_parameter_is_refused() {
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let fx = populate(&h, &alice, "alice-secret").await;
    populate(&h, &bob, OUTSIDE).await;
    std::fs::write(h.data.path().join("secret-outside.md"), OUTSIDE).expect("plant");
    std::fs::write(
        h.data.path().join("users").join(alice.id.to_string()).join("secret-outside.md"),
        OUTSIDE,
    )
    .expect("plant");
    let vaults = [h.dir(alice.id), h.dir(bob.id)];
    let outside_before = hardening::files_under(h.data.path(), &vaults);
    let bob_log = h.log(bob.id);
    let contract = Contract::production();
    let doc = contract.document().clone();
    let mut failures = Vec::new();
    let mut probes = 0;
    let mut outcomes: BTreeMap<(String, u16, Option<String>), u32> = BTreeMap::new();
    for op in ops::operations(&contract) {
        if op.is_admin() || SESSION_ENDING.contains(&op.id.as_str()) || !op.secured {
            continue;
        }
        for param in &op.params {
            for (label, attack) in attacks(h.data.path(), bob.id) {
                let mut req = request(&doc, &op, &alice, &fx);
                match param.loc {
                    Loc::Path => {
                        let path = op.fill(|name| {
                            if name == param.name {
                                attack.clone()
                            } else {
                                path_kind(&op, name).map_or_else(String::new, |k| fx.id_of(k))
                            }
                        });
                        let query = req.target.split_once('?').map(|(_, q)| q.to_owned());
                        req.target = query.map_or(path.clone(), |q| format!("{path}?{q}"));
                    }
                    Loc::Query => {
                        let (path, query) = req
                            .target
                            .split_once('?')
                            .map_or((req.target.clone(), String::new()), |(p, q)| {
                                (p.to_owned(), q.to_owned())
                            });
                        let mut pairs: Vec<String> = query
                            .split('&')
                            .filter(|kv| !kv.is_empty() && !kv.starts_with(&format!("{}=", param.name)))
                            .map(str::to_owned)
                            .collect();
                        pairs.push(format!("{}={attack}", param.name));
                        req.target = format!("{path}?{}", pairs.join("&"));
                    }
                    Loc::Header => {
                        req.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&param.name));
                        req.headers.push((param.name.clone(), attack.replace('%', "%25")));
                    }
                }
                probes += 1;
                let resp = h.send(None, &req).await;
                let ptype = resp.problem_type();
                *outcomes
                    .entry((format!("{}:{}", op.id, param.name), resp.status, ptype.clone()))
                    .or_default() += 1;
                let what = format!("{} {}={label} → {} {ptype:?}", op.id, param.name, resp.status);
                if resp.status >= 500 || !allowed(&op, param).contains(&resp.status) {
                    failures.push(format!("{what}: status not allowed"));
                }
                if ptype.as_deref() != Some("route_not_found") && resp.status != 101 {
                    if let Some(v) = h.conformance.check(&op.id, resp.status, resp.content_type(), &resp.body) {
                        failures.push(format!("{what}: {v}"));
                    }
                }
                if resp.body.windows(OUTSIDE.len()).any(|w| w == OUTSIDE.as_bytes()) {
                    failures.push(format!("{what}: content from outside the vault"));
                }
                let mut leaks = Vec::new();
                scan(&h, &what, &resp, &mut leaks, &[]);
                failures.extend(leaks);
            }
        }
    }
    for ((op, status, ptype), n) in &outcomes {
        eprintln!("traversal {op} → {status} {ptype:?} ×{n}");
    }
    assert_eq!(failures, Vec::<String>::new());
    assert!(probes > 1_000, "{probes} probes");
    assert_eq!(hardening::files_under(h.data.path(), &vaults), outside_before, "nothing written outside the vaults");
    assert_eq!(h.log(bob.id), bob_log, "the neighbour's vault is untouched");
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: import zip — reject symlinks, absolute paths, `..`, oversized entries (and zip bombs)
// ---------------------------------------------------------------------------------------------

fn problem_of(resp: &Resp) -> types::Problem {
    rmp_serde::from_slice(&resp.body).expect("problem body")
}

fn plain(slug: &str, title: &str, status: u32, detail: &str) -> types::Problem {
    types::Problem {
        candidates: vec![],
        current_version: None,
        detail: Some(detail.to_owned()),
        errors: vec![],
        instance: None,
        status,
        title: title.to_owned(),
        type_: slug.to_owned(),
    }
}

fn invalid_archive(detail: &str) -> types::Problem {
    plain("invalid_archive", "Invalid archive", 422, detail)
}

fn too_large(detail: &str) -> types::Problem {
    plain("payload_too_large", "Request body too large", 413, detail)
}

/// Small import limits so bombs stay cheap: 50 entries, 1 MiB per entry, 4 MiB in total.
fn small_import() -> Options {
    Options {
        import: strata_vault::ImportLimits {
            max_entries: 50,
            max_entry_bytes: 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024,
        },
        ..Options::default()
    }
}

async fn import(h: &H, u: &User, body: Vec<u8>) -> Resp {
    h.send(
        Some("import_vault"),
        &Req::new("POST", "/api/v1/import")
            .token(&u.token)
            .body(strata_api::wire::ZIP, body),
    )
    .await
}

#[tokio::test]
async fn import_rejects_hostile_archives_whole() {
    let h = H::with(small_import()).await;
    let alice = h.user("alice").await;
    let ok: &[u8] = b"fine\n";
    let options = zip::write::SimpleFileOptions::default();
    let symlink = |name: &str, target: &str| {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("notes/ok.md", options).expect("entry");
        z.write_all(ok).expect("write");
        z.add_symlink(name, target, options).expect("symlink");
        z.finish().expect("zip").into_inner()
    };
    let many: Vec<(String, &[u8])> = (0..51).map(|i| (format!("notes/n{i}.md"), ok)).collect();
    let many: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    let big = vec![b'a'; 1024 * 1024 + 1];
    let cases: Vec<(&str, Vec<u8>, types::Problem)> = vec![
        ("symlink out of the vault", symlink("notes/link.md", "/etc/passwd"), invalid_archive("the archive contains a symlink")),
        ("symlink to a sibling", symlink("notes/link.md", "../../other/vault/notes/x.md"), invalid_archive("the archive contains a symlink")),
        ("absolute path", zip_of(&[("notes/ok.md", ok), ("/etc/cron.d/evil.md", ok)]), invalid_archive("an entry has an absolute path")),
        ("dot-dot first", zip_of(&[("../evil.md", ok)]), invalid_archive("an entry path contains ..")),
        ("dot-dot inside", zip_of(&[("notes/ok.md", ok), ("notes/../../evil.md", ok)]), invalid_archive("an entry path contains ..")),
        ("dot-dot at the end", zip_of(&[("notes/..", ok)]), invalid_archive("an entry path contains ..")),
        ("backslash", zip_of(&[("notes\\..\\evil.md", ok)]), invalid_archive("an entry path contains a backslash")),
        ("drive prefix", zip_of(&[("C:/evil.md", ok)]), invalid_archive("an entry path has a drive prefix")),
        ("oversized entry", zip_of(&[("notes/big.md", &big)]), too_large("an entry is larger than 1048576 bytes")),
        ("too many entries", zip_of(&many), too_large("the archive has more than 50 entries")),
    ];
    let log = h.log(alice.id);
    let outside = hardening::files_under(h.data.path(), &[h.dir(alice.id)]);
    let tree = api::get_tree(&alice.client).await.expect("tree");
    let mut got = Vec::new();
    for (label, body, expected) in &cases {
        let resp = import(&h, &alice, body.clone()).await;
        got.push((*label, problem_of(&resp)));
        assert_eq!(u32::from(resp.status), expected.status, "{label}");
    }
    let expected: Vec<(&str, types::Problem)> = cases.iter().map(|(l, _, p)| (*l, p.clone())).collect();
    assert_eq!(got, expected);
    assert_eq!(h.log(alice.id), log, "nothing committed");
    assert_eq!(api::get_tree(&alice.client).await.expect("tree"), tree, "nothing imported");
    assert_eq!(hardening::files_under(h.data.path(), &[h.dir(alice.id)]), outside, "nothing written outside");
    for name in ["evil.md", "../evil.md", "notes/link.md"] {
        assert!(!h.dir(alice.id).join(name).exists(), "{name}");
    }
    h.finish().await;
}

/// A deflated zip of `entries` whose headers declare `declared` uncompressed bytes per entry.
fn lying_zip(entries: &[(&str, usize)], declared: u32) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, size) in entries {
        z.start_file(*name, options).expect("entry");
        z.write_all(&vec![0u8; *size]).expect("write");
    }
    let mut bytes = z.finish().expect("zip").into_inner();
    let d = declared.to_le_bytes();
    let mut i = 0;
    while i + 30 <= bytes.len() {
        match bytes[i..i + 4] {
            // Local file header: uncompressed size at +22.
            [0x50, 0x4b, 0x03, 0x04] => bytes[i + 22..i + 26].copy_from_slice(&d),
            // Central directory header: uncompressed size at +24.
            [0x50, 0x4b, 0x01, 0x02] => bytes[i + 24..i + 28].copy_from_slice(&d),
            _ => {}
        }
        i += 1;
    }
    bytes
}

#[tokio::test]
async fn import_stops_zip_bombs_while_decompressing() {
    let h = H::with(small_import()).await;
    let alice = h.user("alice").await;
    let mib = 1024 * 1024;
    // Headers that lie (declare 10 bytes, inflate to 8 MiB): stopped by the entry limit
    // while decompressing, not trusted from the header.
    let liar = lying_zip(&[("notes/bomb.md", 8 * mib)], 10);
    // A high-ratio archive whose every entry is within limits (1 MiB of zeros each, ~1 KiB
    // compressed) but whose total is not.
    let ratio = lying_zip(
        &(0..6).map(|i| (["notes/b0.md", "notes/b1.md", "notes/b2.md", "notes/b3.md", "notes/b4.md", "notes/b5.md"][i], mib)).collect::<Vec<_>>(),
        u32::try_from(mib).expect("fits"),
    );
    assert!(ratio.len() < 64 * 1024, "compression ratio above 90:1 ({} bytes)", ratio.len());
    let log = h.log(alice.id);
    let liar_resp = import(&h, &alice, liar).await;
    let ratio_resp = import(&h, &alice, ratio).await;
    assert_eq!(
        (liar_resp.status, problem_of(&liar_resp)),
        (413, too_large("an entry is larger than 1048576 bytes"))
    );
    assert_eq!(
        (ratio_resp.status, problem_of(&ratio_resp)),
        (413, too_large("the archive is larger than 4194304 bytes uncompressed"))
    );
    assert_eq!(h.log(alice.id), log, "nothing committed");
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: rate limits on login, signup, capture, ask
// ---------------------------------------------------------------------------------------------

fn limits(tweak: impl FnOnce(&mut strata_common::config::RateLimits) + 'static) -> Options {
    Options {
        config: Box::new(move |c| {
            hardening::generous_limits(c);
            tweak(&mut c.auth.rate_limits);
        }),
        ..Options::default()
    }
}

fn rate_limited(detail: &str) -> types::Problem {
    plain("rate_limited", "Too many requests", 429, detail)
}

fn login_body(name: &str, password: &str) -> Vec<u8> {
    msgpack(&map(&[
        ("username", M::from(name)),
        ("password", M::from(password)),
        ("device_name", M::from("d")),
        ("platform", M::from("linux")),
    ]))
}

#[tokio::test]
async fn rate_limit_login() {
    let h = H::with(limits(|l| {
        l.login_per_username = RateLimit { max: 3, window_secs: 60 };
    }))
    .await;
    let alice = h.user("alice").await; // one login spent
    let attempt = |password: &str| {
        Req::new("POST", "/api/v1/auth/login").msgpack(login_body("alice", password))
    };
    let wrong = h.send(Some("login"), &attempt("wrong-password-1")).await;
    assert_eq!((wrong.status, problem_of(&wrong).type_), (401, "invalid_credentials".to_owned()));
    let wrong = h.send(Some("login"), &attempt("wrong-password-1")).await;
    assert_eq!(wrong.status, 401);
    // The fourth attempt in the window is refused even with the right password.
    let limited = h.send(Some("login"), &attempt(&alice.password)).await;
    assert_eq!(
        (limited.status, limited.header("retry-after"), problem_of(&limited)),
        (429, Some("60"), rate_limited("too many login attempts for this account"))
    );
    h.clock.advance(chrono::Duration::seconds(60));
    let ok = h.send(Some("login"), &attempt(&alice.password)).await;
    assert_eq!(ok.status, 200);
    h.finish().await;
}

fn signup_body(name: &str) -> Vec<u8> {
    msgpack(&map(&[
        ("username", M::from(name)),
        ("password", M::from("a-long-password-1")),
        ("display_name", M::from(name)),
    ]))
}

#[tokio::test]
async fn rate_limit_signup() {
    let h = H::with(limits(|l| {
        l.signup_per_ip = RateLimit { max: 2, window_secs: 3600 };
    }))
    .await;
    for name in ["carol", "dave"] {
        let r = h.send(Some("signup"), &Req::new("POST", "/api/v1/auth/signup").msgpack(signup_body(name))).await;
        assert_eq!(r.status, 201, "{name}");
    }
    let limited = h
        .send(Some("signup"), &Req::new("POST", "/api/v1/auth/signup").msgpack(signup_body("erin")))
        .await;
    assert_eq!(
        (limited.status, limited.header("retry-after"), problem_of(&limited)),
        (429, Some("3600"), rate_limited("too many sign-ups from this address"))
    );
    h.clock.advance(chrono::Duration::seconds(3600));
    let r = h.send(Some("signup"), &Req::new("POST", "/api/v1/auth/signup").msgpack(signup_body("erin"))).await;
    assert_eq!(r.status, 201);
    h.finish().await;
}

fn capture_req(u: &User, text: &str) -> Req {
    Req::new("POST", "/api/v1/capture")
        .token(&u.token)
        .msgpack(msgpack(&map(&[("text", M::from(text))])))
}

#[tokio::test]
async fn rate_limit_capture() {
    let h = H::with(limits(|l| {
        l.capture_per_user = RateLimit { max: 3, window_secs: 60 };
    }))
    .await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    for i in 0..3 {
        let r = h.send(Some("capture"), &capture_req(&alice, &format!("thought {i}"))).await;
        assert_eq!(r.status, 201);
    }
    let log = h.log(alice.id);
    let limited = h.send(Some("capture"), &capture_req(&alice, "one too many")).await;
    assert_eq!(
        (limited.status, limited.header("retry-after"), problem_of(&limited)),
        (429, Some("60"), rate_limited("too many captures; try again later"))
    );
    assert_eq!(h.log(alice.id), log, "a refused capture writes nothing");
    // Per user: another user is not affected.
    assert_eq!(h.send(Some("capture"), &capture_req(&bob, "bob's thought")).await.status, 201);
    h.clock.advance(chrono::Duration::seconds(60));
    assert_eq!(h.send(Some("capture"), &capture_req(&alice, "later")).await.status, 201);
    h.finish().await;
}

fn ask_req(u: &User, question: &str) -> Req {
    Req::new("POST", "/api/v1/ask")
        .token(&u.token)
        .msgpack(msgpack(&map(&[("question", M::from(question))])))
}

#[tokio::test]
async fn rate_limit_ask() {
    let h = H::with(limits(|l| {
        l.ask_per_user = RateLimit { max: 2, window_secs: 60 };
    }))
    .await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    for q in ["first?", "second?"] {
        assert_eq!(h.send(Some("ask"), &ask_req(&alice, q)).await.status, 200, "{q}");
    }
    let limited = h.send(Some("ask"), &ask_req(&alice, "third?")).await;
    assert_eq!(
        (limited.status, limited.header("retry-after"), problem_of(&limited)),
        (429, Some("60"), rate_limited("too many questions; try again later"))
    );
    assert_eq!(h.send(Some("ask"), &ask_req(&bob, "bob?")).await.status, 200);
    h.clock.advance(chrono::Duration::seconds(60));
    assert_eq!(h.send(Some("ask"), &ask_req(&alice, "later?")).await.status, 200);
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: content never included in logs or error messages
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct LogSink(Arc<Mutex<Vec<u8>>>);

impl Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every `tracing` event of this process at TRACE level, as `stratad` formats them (JSON).
fn captured_logs() -> Arc<Mutex<Vec<u8>>> {
    static LOGS: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let sink = LogSink(buf.clone());
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::TRACE)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(move || sink.clone())
            .try_init()
            .expect("the only global subscriber");
        buf
    })
    .clone()
}

#[tokio::test]
async fn content_never_reaches_logs_or_error_messages() {
    const S: &str = "Zebra7Sentinel";
    let logs = captured_logs();
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let t = &alice.token;
    let body = |entries: &[(&str, M)]| msgpack(&map(entries));
    let mut problems = Vec::new();
    let run = |op: &'static str, req: Req| (op, req);
    let note_content = format!("Private {S} plans [[Watanya]]\n");
    let steps: Vec<(&str, Req)> = vec![
        run("signup", Req::new("POST", "/api/v1/auth/signup").msgpack(body(&[("username", M::from(format!("u{S}"))), ("password", M::from(format!("pw-{S}-long"))), ("display_name", M::from(S))]))),
        run("signup", Req::new("POST", "/api/v1/auth/signup").msgpack(body(&[("username", M::from(format!("{S}/../x"))), ("password", M::from("short")), ("display_name", M::from(S))]))),
        run("login", Req::new("POST", "/api/v1/auth/login").msgpack(login_body("alice", &format!("wrong-{S}")))),
        run("login", Req::new("POST", "/api/v1/auth/login").msgpack(login_body(S, S))),
        run("refresh", Req::new("POST", "/api/v1/auth/refresh").msgpack(body(&[("refresh_token", M::from(S))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(format!("notes/{S}.md"))), ("content", M::from(note_content.as_str()))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(format!("notes/{S}.md"))), ("content", M::from(note_content.as_str()))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(format!("notes/{S}|?.md"))), ("content", M::from(S))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(format!("../{S}.md"))), ("content", M::from(S))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(7)), ("content", M::from(S))]))),
        run("create_note", Req::new("POST", "/api/v1/notes").token(t).msgpack(body(&[("path", M::from(S)), ("content", M::from(7)), ("force", M::from(S))]))),
        run("get_note_by_path", Req::new("GET", format!("/api/v1/notes/by-path?path=notes%2F{S}.md")).token(t)),
        run("get_note_by_path", Req::new("GET", format!("/api/v1/notes/by-path?path=notes%2F{S}-missing.md")).token(t)),
        run("search", Req::new("GET", format!("/api/v1/search?q={S}")).token(t)),
        run("search", Req::new("GET", format!("/api/v1/search?q={S}&mode={S}")).token(t)),
        run("search", Req::new("GET", format!("/api/v1/search?q={S}&limit={S}")).token(t)),
        run("capture", capture_req(&alice, &format!("Call {S} tomorrow"))),
        run("create_task", Req::new("POST", "/api/v1/tasks").token(t).msgpack(body(&[("text", M::from(format!("Pay {S}"))), ("recurrence", M::from(format!("every {S}")))]))),
        run("create_entity", Req::new("POST", "/api/v1/entities").token(t).msgpack(body(&[("kind", M::from("person")), ("name", M::from(S))]))),
        run("create_entity", Req::new("POST", "/api/v1/entities").token(t).msgpack(body(&[("kind", M::from(S)), ("name", M::from(S))]))),
        run("put_map", Req::new("PUT", "/api/v1/maps/Private").token(t).msgpack(body(&[("content", M::from(format!("{{\"nodes\":[{{\"id\":\"{S}\",\"type\":\"file\",\"file\":\"notes/{S}-gone.md\",\"x\":0,\"y\":0,\"width\":1,\"height\":1}}],\"edges\":[]}}")))]))),
        run("put_map", Req::new("PUT", "/api/v1/maps/Broken").token(t).msgpack(body(&[("content", M::from(format!("not json {S}")))]))),
        run("update_me", Req::new("PATCH", "/api/v1/me").token(t).msgpack(body(&[("timezone", M::from(S)), ("display_name", M::from(S))]))),
        run("update_me", Req::new("PATCH", "/api/v1/me").token(t).msgpack(body(&[("current_password", M::from(S)), ("new_password", M::from(format!("{S}-new-pass")))]))),
        run("import_vault", Req::new("POST", "/api/v1/import").token(t).body(strata_api::wire::ZIP, zip_of(&[(&format!("../{S}.md"), S.as_bytes())]))),
        run("import_vault", Req::new("POST", "/api/v1/import").token(t).body(strata_api::wire::ZIP, zip_of(&[(&format!("notes/{S} import.md"), format!("{S}\n").as_bytes())]))),
        run("ask", ask_req(&alice, &format!("What about {S}?"))),
        run("sync_push", Req::new("POST", "/api/v1/sync/push").token(t).msgpack(body(&[("ops", M::Array(vec![map(&[("op_id", M::from("01K5Z00000000000000000000A")), ("kind", M::from("note.create")), ("payload", map(&[("path", M::from(format!("notes/{S} pushed.md"))), ("content", M::from(S))]))]), map(&[("op_id", M::from(S)), ("kind", M::from(S))])]))]))),
    ];
    for (op, req) in steps {
        let resp = h.send(Some(op), &req).await;
        assert!(resp.status < 500, "{op} → {}", resp.status);
        if resp.is_problem() {
            let p = problem_of(&resp);
            let mut texts = vec![p.title.clone(), p.detail.clone().unwrap_or_default(), p.type_.clone()];
            texts.extend(p.errors.iter().map(|e| format!("{} {:?} {}", e.code, e.pointer, e.message)));
            if texts.iter().any(|t| t.contains(S)) {
                problems.push(format!("{op} {}: {texts:?}", resp.status));
            }
        }
    }
    // Updates with a stale version, a note read, then the background work of the flow.
    let note = api::get_note_by_path(&alice.client, &format!("notes/{S}.md")).await.expect("note");
    let stale = h
        .send(
            Some("update_note"),
            &Req::new("PUT", format!("/api/v1/notes/{}", note.id))
                .token(t)
                .header("If-Match", &format!("sha256:{S}"))
                .msgpack(body(&[("content", M::from(format!("Stale {S}")))])),
        )
        .await;
    assert_eq!(stale.status, 409);
    let p = problem_of(&stale);
    assert!(!p.detail.unwrap_or_default().contains(S));
    h.embed_all().await;
    assert_eq!(problems, Vec::<String>::new(), "error messages carry no content");
    let text = String::from_utf8_lossy(&logs.lock().expect("lock")).into_owned();
    assert!(text.contains("\"message\":\"request\""), "the request logger ran");
    let hits: Vec<&str> = text.lines().filter(|l| l.contains(S) || l.contains(&S.to_lowercase())).collect();
    assert_eq!(hits, Vec::<&str>::new(), "no log line carries content");
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: secrets file permission check at startup
// ---------------------------------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn startup_refuses_secret_files_open_to_others() {
    use std::os::unix::fs::PermissionsExt;
    use stratad::checks::StartupError;

    let db = strata_testkit::TestDb::new().await.expect("db");
    let dir = tempfile::tempdir().expect("tempdir");
    let role_url = |role: &str| {
        let mut url = url::Url::parse(&strata_testkit::admin_url()).expect("admin url");
        url.set_username(role).expect("username");
        url.set_password(std::env::var(strata_testkit::ROLE_PASSWORD_ENV).ok().as_deref())
            .expect("password");
        url.set_path(db.name());
        url.to_string()
    };
    let secret = |name: &str, content: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, content).expect("write");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        p
    };
    let mut config = strata_common::Config {
        data_root: dir.path().to_path_buf(),
        ..strata_common::Config::default()
    };
    config.auth.signing_key_file = dir.path().join("signing.pem");
    stratad::commands::keygen(&config.auth.signing_key_file, false).expect("keygen");
    config.auth.argon2.memory_kib = 64;
    config.auth.argon2.iterations = 1;
    config.database.owner_url = role_url("strata_owner");
    config.database.app_url = role_url("strata_app");
    config.database.accounts_url = role_url("strata_accounts");
    config.database.max_connections = 2;
    config.ai.anthropic_api.api_key_file = Some(secret("anthropic.key", "sk-ant-SECRETVALUE"));
    config.push.fcm_service_account_path = Some(secret("fcm.json", "{\"private_key\":\"SECRETVALUE\"}"));
    config.push.apns_key_path = Some(secret("apns.p8", "SECRETVALUE"));
    config.push.wns_credentials_path = Some(secret("wns.json", "SECRETVALUE"));
    let files = stratad::checks::secret_files(&config);
    assert_eq!(files.len(), 5, "every configured secret is checked");
    assert_eq!(stratad::serve::prepare(&config).await.map(|_| ()), Ok(()), "0600 everywhere starts");
    for file in &files {
        for mode in [0o640, 0o604, 0o620, 0o602, 0o660, 0o644, 0o666, 0o700 | 0o040] {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(mode)).expect("chmod");
            let err = stratad::serve::prepare(&config).await.map(|_| ()).expect_err("refused");
            assert_eq!(err, StartupError::SecretPermissions { path: file.clone(), mode }, "{} {mode:o}", file.display());
            assert!(!err.to_string().contains("SECRETVALUE"), "the message never carries the secret");
        }
        for mode in [0o400, 0o600] {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(mode)).expect("chmod");
            assert_eq!(stratad::serve::prepare(&config).await.map(|_| ()), Ok(()), "{mode:o}");
        }
    }
    // A directory, a missing file, and a symlink to an open file are refused too.
    let open_target = dir.path().join("open.key");
    std::fs::write(&open_target, "SECRETVALUE").expect("write");
    std::fs::set_permissions(&open_target, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let link = dir.path().join("link.key");
    std::os::unix::fs::symlink(&open_target, &link).expect("symlink");
    let cases = [
        (dir.path().to_path_buf(), StartupError::NotAFile { path: dir.path().to_path_buf() }),
        (dir.path().join("absent.key"), StartupError::MissingSecret { path: dir.path().join("absent.key") }),
        (link.clone(), StartupError::SecretPermissions { path: link.clone(), mode: 0o644 }),
    ];
    for (path, expected) in cases {
        let mut c = config.clone();
        c.push.apns_key_path = Some(path);
        assert_eq!(stratad::serve::prepare(&c).await.map(|_| ()), Err(expected));
    }
    db.cleanup().await.expect("cleanup");
}

// ---------------------------------------------------------------------------------------------
// §15 / §7.7: MessagePack decoding limits enforced on every request body
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn msgpack_decode_limits_hold_on_every_body() {
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let admin = h.admin("root").await;
    let fx = populate(&h, &alice, "alice-secret").await;
    let contract = Contract::production();
    let doc = contract.document().clone();
    let mut depth_bomb = vec![0x91u8; 65];
    depth_bomb.push(0x01);
    let mut nested_maps = Vec::new();
    for _ in 0..200 {
        nested_maps.extend_from_slice(&[0x81, 0xa1, b'a']);
    }
    nested_maps.push(0xc0);
    // (label, body, expected status, expected `errors[0].code` for 422)
    let attacks: Vec<(&str, Vec<u8>, u16, &str)> = vec![
        ("depth bomb (arrays)", depth_bomb, 422, "depth_exceeded"),
        ("depth bomb (maps)", nested_maps, 422, "depth_exceeded"),
        ("str32 header 4 GiB", vec![0xdb, 0xff, 0xff, 0xff, 0xff, b'a'], 422, "string_too_long"),
        ("bin32 header 4 GiB", vec![0xc6, 0xff, 0xff, 0xff, 0xff, 0x00], 422, "binary_too_long"),
        ("array32 header 4G", vec![0xdd, 0xff, 0xff, 0xff, 0xff, 0x01], 422, "array_too_long"),
        ("map32 header 4G", vec![0xdf, 0xff, 0xff, 0xff, 0xff, 0xa1, b'a', 0x01], 422, "map_too_long"),
        ("trailing bytes", vec![0x80, 0x01, 0x02], 422, "trailing_bytes"),
        ("truncated", vec![0x92, 0x01], 422, "truncated"),
        ("extension type", vec![0xd4, 0x01, 0x00], 422, "extension_not_allowed"),
        ("invalid UTF-8", vec![0x81, 0xa1, b'a', 0xa2, 0xff, 0xfe], 422, "invalid_utf8"),
        ("reserved marker", vec![0xc1], 422, "invalid_marker"),
        ("empty body", vec![], 422, "empty_body"),
    ];
    let mut got = Vec::new();
    let mut expected = Vec::new();
    let mut checked = 0;
    for op in ops::operations(&contract) {
        let Some(Body::MsgPack(_)) = &op.body else {
            continue;
        };
        let caller = if op.is_admin() { &admin } else { &alice };
        let base = request(&doc, &op, caller, &fx);
        for (label, bytes, status, code) in &attacks {
            let mut req = base.clone();
            req.body = Some((strata_api::wire::MSGPACK.to_owned(), bytes.clone()));
            let resp = h.send(Some(&op.id), &req).await;
            let p = resp.is_problem().then(|| problem_of(&resp));
            got.push((op.id.clone(), *label, resp.status, p.and_then(|p| p.errors.first().map(|e| e.code.clone()))));
            expected.push((op.id.clone(), *label, *status, Some((*code).to_owned())));
        }
        // A declared body over any route's limit (16 MiB is the largest) is refused before
        // it is read.
        let mut req = base.clone();
        req.body = Some((strata_api::wire::MSGPACK.to_owned(), vec![0xc0; 16]));
        req.declared_len = Some(16 * 1024 * 1024 + 1);
        let resp = h.send(Some(&op.id), &req).await;
        got.push((op.id.clone(), "declared 16 MiB + 1", resp.status, resp.problem_type()));
        expected.push((op.id.clone(), "declared 16 MiB + 1", 413, Some("payload_too_large".to_owned())));
        checked += 1;
    }
    assert_eq!(got, expected);
    assert!(checked >= 25, "{checked} operations with MessagePack bodies");
    h.finish().await;
}

// ---------------------------------------------------------------------------------------------
// §15: tenant isolation — every endpoint with another user's IDs/paths → 404
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn tenant_isolation_sweep_answers_404_for_foreign_ids() {
    let h = H::with(Options {
        config: Box::new(hardening::generous_limits),
        ..Options::default()
    })
    .await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let a = populate(&h, &alice, "alice-private-4f2a").await;
    let b = populate(&h, &bob, "bob-private-91c3").await;
    let alice_log = h.log(alice.id);
    let export_before = api::export_vault(&alice.client).await.expect("export");
    let contract = Contract::production();
    let doc = contract.document().clone();
    let private = a.private_strings();
    let mut failures = Vec::new();
    let mut swept = BTreeSet::new();
    let mut all = ops::operations(&contract);
    all.sort_by_key(|op| (phase(op), op.id.clone()));
    for op in &all {
        if op.is_admin() {
            // Account administration addresses users, not vault data; admins have no route
            // into a member's vault (covered by auth_flow and account_deletion).
            continue;
        }
        let token = if SESSION_ENDING.contains(&op.id.as_str()) {
            h.login(&bob.name, &bob.password).await
        } else {
            bob.token.clone()
        };
        // 1. Bob's own valid request: nothing of Alice's may appear.
        let mut own = request(&doc, op, &bob, &b);
        own.token = Some(token.clone());
        let resp = h.send(Some(&op.id), &own).await;
        let mut leaks = Vec::new();
        scan(&h, &format!("{} (own)", op.id), &resp, &mut leaks, &private);
        failures.extend(leaks);
        // 2. Alice's IDs in every ID position (path, query, body) of Bob's request.
        let has_path_id = op.path_params().next().is_some();
        let body_ids = matches!(&op.body, Some(Body::MsgPack(s)) if schema_has_ulid(&doc, s, 0));
        let query_ids: Vec<&ops::Param> = op
            .params
            .iter()
            .filter(|p| p.loc == Loc::Query && p.schema.get("format").and_then(|f| f.as_str()) == Some("ulid"))
            .collect();
        if !has_path_id && !body_ids && query_ids.is_empty() {
            continue;
        }
        swept.insert(op.id.clone());
        let mut foreign = request(&doc, op, &bob, &a);
        foreign.token = Some(token.clone());
        if !query_ids.is_empty() {
            let path = foreign.target.split('?').next().unwrap_or_default().to_owned();
            let q: Vec<String> = query_ids.iter().map(|p| format!("{}={}", p.name, a.id_of(field_kind(&p.name)))).collect();
            foreign.target = format!("{path}?{}", q.join("&"));
        }
        let resp = h.send(Some(&op.id), &foreign).await;
        let what = format!("{} with alice's ids → {} {:?}", op.id, resp.status, resp.problem_type());
        let expected_404 = has_path_id || body_ids || op.statuses.contains(&404);
        if expected_404 && (resp.status, resp.problem_type().as_deref()) != (404, Some("not_found")) {
            failures.push(format!("{what}: expected 404 not_found"));
        }
        if !expected_404 && resp.status != 200 {
            failures.push(format!("{what}: expected an empty 200"));
        }
        let mut leaks = Vec::new();
        scan(&h, &what, &resp, &mut leaks, &private);
        failures.extend(leaks);
    }
    assert_eq!(failures, Vec::<String>::new());
    let expected_swept: BTreeSet<String> = all
        .iter()
        .filter(|op| !op.is_admin() && (op.path_params().next().is_some() || matches!(&op.body, Some(Body::MsgPack(s)) if schema_has_ulid(&doc, s, 0))))
        .map(|op| op.id.clone())
        .collect();
    assert!(swept.is_superset(&expected_swept), "missing {:?}", expected_swept.difference(&swept).collect::<Vec<_>>());
    assert!(swept.len() >= 40, "{} operations swept", swept.len());
    assert_eq!(h.log(alice.id), alice_log, "alice's vault untouched");
    assert_eq!(api::export_vault(&alice.client).await.expect("export"), export_before);
    h.finish().await;
}

/// True if `schema` has a ULID-typed member within the first few levels (an ID the request
/// addresses).
fn schema_has_ulid(doc: &serde_json::Value, schema: &serde_json::Value, depth: u32) -> bool {
    if depth > 3 {
        return false;
    }
    let s = generate::resolve(doc, schema);
    if s.get("format").and_then(|f| f.as_str()) == Some("ulid") {
        return true;
    }
    let props = s.get("properties").and_then(|p| p.as_object());
    let required: BTreeSet<&str> = s
        .get("required")
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    props.is_some_and(|props| {
        props.iter().any(|(k, p)| required.contains(k.as_str()) && k != "id" && k != "op_id" && schema_has_ulid(doc, p, depth + 1))
    })
}
