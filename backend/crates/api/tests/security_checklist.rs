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
            req.target = op.fill(|_| alice.id.to_string());
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
    assert_eq!(leaks, Vec::<String>::new());
    assert_eq!(
        covered.keys().cloned().collect::<BTreeSet<_>>(),
        all.iter().map(|o| o.id.clone()).collect::<BTreeSet<_>>(),
        "every documented operation was called"
    );
    h.finish().await;
}
