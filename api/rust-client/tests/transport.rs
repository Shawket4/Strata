//! The hand-written transport of `strata-client` against scripted servers: how every HTTP
//! outcome maps to [`Error`] (undocumented media types, trailing bytes, undecodable problems,
//! every problem type), what a request puts on the wire (query, headers, zip bodies, `Accept`),
//! and the WebSocket subscription's handling of frames the demo server never sends (error
//! frames, undecodable frames, control frames, unknown kinds) and of a `401` handshake.
// Tests: expect/unwrap, long tables, local helper items; the handshake callback's error type
// is tungstenite's.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::type_complexity,
    clippy::result_large_err
)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, web};
use futures_util::SinkExt;
use futures_util::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use strata_api::testing::TestServer;
use strata_client::streaming::{Frame, StreamEvent, StreamOptions, Subscription, decode_frame};
use strata_client::types::{DuplicateCandidate, MatchLevel, Problem};
use strata_client::{
    ApiError, Client, Error, Method, Request, StaticToken, TokenProvider, param_string,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request as WsRequest};

const MSGPACK: &str = "application/vnd.msgpack";
const PROBLEM: &str = "application/problem+msgpack";

fn problem(type_: &str, status: u32) -> Problem {
    Problem {
        candidates: Vec::new(),
        current_version: None,
        detail: Some("Detail.".to_owned()),
        errors: Vec::new(),
        instance: None,
        status,
        title: format!("Title of {type_}"),
        type_: type_.to_owned(),
    }
}

/// What the echo route saw.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Echo {
    method: String,
    path: String,
    query: String,
    accept: Option<String>,
    content_type: Option<String>,
    header: Option<String>,
    authorization: Option<String>,
    body: Vec<u8>,
}

fn header(req: &HttpRequest, name: &str) -> Option<String> {
    req.headers()
        .get(name)
        .map(|v| v.to_str().expect("ascii header").to_owned())
}

fn scripted(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/echo",
        web::route().to(|req: HttpRequest, body: web::Bytes| async move {
            let echo = Echo {
                method: req.method().to_string(),
                path: req.path().to_owned(),
                query: req.query_string().to_owned(),
                accept: header(&req, "accept"),
                content_type: header(&req, "content-type"),
                header: header(&req, "x-strata-test"),
                authorization: header(&req, "authorization"),
                body: body.to_vec(),
            };
            HttpResponse::Ok()
                .content_type(MSGPACK)
                .body(rmp_serde::to_vec_named(&echo).expect("encode"))
        }),
    )
    .route(
        "/json",
        web::get().to(|| async {
            HttpResponse::Ok()
                .content_type("application/json")
                .body("{}")
        }),
    )
    .route(
        "/untyped",
        web::get().to(|| async { HttpResponse::Ok().body(vec![0x07]) }),
    )
    .route(
        "/trailing",
        web::get().to(|| async {
            HttpResponse::Ok()
                .content_type("application/vnd.msgpack; charset=binary")
                .body(vec![0x07, 0xc0, 0xc0])
        }),
    )
    .route(
        "/wrong-shape",
        web::get().to(|| async {
            HttpResponse::Ok()
                .content_type(MSGPACK)
                .body(rmp_serde::to_vec_named("text").expect("encode"))
        }),
    )
    .route(
        "/problem/{type}/{status}",
        web::get().to(|path: web::Path<(String, u16)>| async move {
            let (type_, status) = path.into_inner();
            let mut p = problem(&type_, u32::from(status));
            if type_ == "version_conflict" {
                p.current_version = Some("v7".to_owned());
            }
            if type_ == "duplicate_candidates" {
                p.candidates = vec![candidate()];
            }
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status).expect("status"))
                .content_type(PROBLEM)
                .body(rmp_serde::to_vec_named(&p).expect("encode"))
        }),
    )
    .route(
        "/garbled-problem",
        web::get().to(|| async {
            HttpResponse::BadGateway()
                .content_type(PROBLEM)
                .body(vec![0xc1])
        }),
    )
    .route(
        "/html-error",
        web::get().to(|| async {
            HttpResponse::ServiceUnavailable()
                .content_type("text/html")
                .body("<h1>down</h1>")
        }),
    )
    .route(
        "/no-content",
        web::delete().to(|| async { HttpResponse::NoContent().finish() }),
    )
    .route(
        "/export.zip",
        web::get().to(|req: HttpRequest| async move {
            assert_eq!(
                header(&req, "accept").as_deref(),
                Some("application/zip, application/vnd.msgpack;q=0.5")
            );
            HttpResponse::Ok()
                .content_type("application/zip")
                .body(b"PK\x05\x06zip".to_vec())
        }),
    );
}

fn candidate() -> DuplicateCandidate {
    DuplicateCandidate {
        id: "01J8ZK3M4X7Q9W2E5R6T8Y0V1B".parse().expect("ulid"),
        kind: "note".to_owned(),
        match_level: MatchLevel::Exact,
        score: 1.0,
        snippet: None,
        title: "Pricing".to_owned(),
    }
}

fn server() -> (TestServer, Client) {
    let server = TestServer::start(scripted).expect("server");
    let client = Client::builder(&server.base_url())
        .tokens(Arc::new(StaticToken("tok-1".to_owned())))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client");
    (server, client)
}

fn get(path: &str) -> Request {
    Request::new(Method::GET, path.to_owned(), "scripted")
}

#[tokio::test]
async fn requests_carry_query_headers_bodies_and_the_bearer_token() {
    let (_server, client) = server();

    let echo: Echo = client
        .send(
            Request::new(Method::POST, "/echo".to_owned(), "echo")
                .authenticated()
                .query("q", "a b&c".to_owned())
                .query("limit", param_string(&20u32).expect("scalar"))
                .header("x-strata-test", "yes".to_owned())
                .body(&serde_json::json!({"name": "ك"}))
                .expect("encodes"),
        )
        .await
        .expect("echo");
    assert_eq!(
        echo,
        Echo {
            method: "POST".to_owned(),
            path: "/echo".to_owned(),
            query: "q=a+b%26c&limit=20".to_owned(),
            accept: Some(MSGPACK.to_owned()),
            content_type: Some(MSGPACK.to_owned()),
            header: Some("yes".to_owned()),
            authorization: Some("Bearer tok-1".to_owned()),
            body: vec![0x81, 0xa4, b'n', b'a', b'm', b'e', 0xa2, 0xd9, 0x83],
        }
    );

    // Unauthenticated requests never send the token; zip bodies carry their media type.
    let echo: Echo = client
        .send(
            Request::new(Method::PUT, "/echo".to_owned(), "echo")
                .zip_body(b"PK".to_vec())
                .accept("application/zip"),
        )
        .await
        .expect("echo");
    assert_eq!(
        echo,
        Echo {
            method: "PUT".to_owned(),
            path: "/echo".to_owned(),
            query: String::new(),
            accept: Some("application/zip".to_owned()),
            content_type: Some("application/zip".to_owned()),
            header: None,
            authorization: None,
            body: b"PK".to_vec(),
        }
    );
}

#[tokio::test]
async fn successes_that_break_the_contract_are_typed_errors() {
    let (_server, client) = server();

    let err = client.send::<u8>(get("/json")).await.expect_err("json");
    assert!(
        matches!(
            &err,
            Error::UnexpectedResponse { operation: "scripted", status: 200, content_type }
                if content_type.as_deref() == Some("application/json")
        ),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "unexpected 200 response (Some(\"application/json\")) from scripted"
    );
    assert!(err.api().is_none());

    let err = client
        .send::<u8>(get("/untyped"))
        .await
        .expect_err("no type");
    assert!(
        matches!(
            err,
            Error::UnexpectedResponse {
                status: 200,
                content_type: None,
                ..
            }
        ),
        "{err:?}"
    );

    // Media type parameters are ignored, but trailing bytes are not.
    let err = client
        .send::<u8>(get("/trailing"))
        .await
        .expect_err("trailing");
    assert_eq!(
        err.to_string(),
        "cannot decode the response of scripted: 2 trailing bytes"
    );
    let err = client
        .send::<u8>(get("/wrong-shape"))
        .await
        .expect_err("shape");
    assert!(
        matches!(
            &err,
            Error::Decode {
                operation: "scripted",
                ..
            }
        ),
        "{err:?}"
    );

    // A body-less success.
    client
        .send_empty(Request::new(
            Method::DELETE,
            "/no-content".to_owned(),
            "delete",
        ))
        .await
        .expect("204");

    // Zip downloads ask for the archive and return its bytes; anything else is unexpected.
    let bytes = client.send_zip(get("/export.zip")).await.expect("archive");
    assert_eq!(bytes.as_ref(), b"PK\x05\x06zip");
    let err = client.send_zip(get("/json")).await.expect_err("not a zip");
    assert!(
        matches!(err, Error::UnexpectedResponse { status: 200, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn every_problem_type_decodes_into_its_variant() {
    let (_server, client) = server();
    let cases: Vec<(&str, u16, fn(&ApiError) -> bool)> = vec![
        ("not_found", 404, |e| matches!(e, ApiError::NotFound(_))),
        ("unauthorized", 401, |e| {
            matches!(e, ApiError::Unauthorized(_))
        }),
        ("forbidden", 403, |e| matches!(e, ApiError::Forbidden(_))),
        ("account_pending", 403, |e| {
            matches!(e, ApiError::AccountPending(_))
        }),
        ("account_disabled", 403, |e| {
            matches!(e, ApiError::AccountDisabled(_))
        }),
        ("account_deletion_pending", 403, |e| {
            matches!(e, ApiError::AccountDeletionPending(_))
        }),
        (
            "version_conflict",
            409,
            |e| matches!(e, ApiError::VersionConflict { current_version: Some(v), .. } if v == "v7"),
        ),
        (
            "duplicate_candidates",
            409,
            |e| matches!(e, ApiError::DuplicateCandidates { candidates, .. } if candidates.len() == 1),
        ),
        ("invalid_body", 422, |e| {
            matches!(e, ApiError::InvalidBody(_))
        }),
        ("invalid_parameter", 422, |e| {
            matches!(e, ApiError::InvalidParameter(_))
        }),
        ("epoch_changed", 410, |e| {
            matches!(e, ApiError::EpochChanged(_))
        }),
        ("rate_limited", 429, |e| {
            matches!(e, ApiError::RateLimited(_))
        }),
        ("payload_too_large", 413, |e| {
            matches!(e, ApiError::Other(_))
        }),
    ];
    for (type_, status, is_variant) in cases {
        let err = client
            .send::<u8>(get(&format!("/problem/{type_}/{status}")))
            .await
            .expect_err("problem");
        let api = err.api().unwrap_or_else(|| panic!("{type_}: {err:?}"));
        assert!(is_variant(api), "{type_}: {api:?}");
        assert_eq!(api.status(), u32::from(status), "{type_}");
        let mut expected = problem(type_, u32::from(status));
        if type_ == "version_conflict" {
            expected.current_version = Some("v7".to_owned());
        }
        if type_ == "duplicate_candidates" {
            expected.candidates = vec![candidate()];
        }
        assert_eq!(api.problem(), &expected, "{type_}");
        assert_eq!(
            err.to_string(),
            format!("{status} {type_}: Title of {type_}")
        );
    }

    // Problem details that do not decode, and errors without them, keep the status.
    let err = client
        .send::<u8>(get("/garbled-problem"))
        .await
        .expect_err("garbled");
    assert!(
        matches!(
            &err,
            Error::UnexpectedResponse { status: 502, content_type, .. }
                if content_type.as_deref() == Some(PROBLEM)
        ),
        "{err:?}"
    );
    let err = client
        .send_empty(get("/html-error"))
        .await
        .expect_err("html");
    assert!(
        matches!(err, Error::UnexpectedResponse { status: 503, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn unreachable_servers_and_bad_inputs_fail_before_decoding() {
    // Nothing listens on the port the listener held.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").port()
    };
    let client = Client::new(&format!("http://127.0.0.1:{port}")).expect("client");
    let err = client.send::<u8>(get("/x")).await.expect_err("refused");
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert!(err.to_string().starts_with("transport: "), "{err}");

    // A path that does not form a URL with the base.
    let err = client
        .send::<u8>(Request::new(Method::GET, "@:bad".to_owned(), "bad"))
        .await
        .expect_err("url");
    assert!(matches!(err, Error::Url(_)), "{err:?}");

    for bad in ["http://strata.example/?q=1", "mailto:x@strata.example"] {
        assert_eq!(
            Client::new(bad).expect_err("rejected").to_string(),
            format!(
                "invalid URL: {}: expected an http(s) URL without query",
                bad.trim_end_matches('/')
            )
        );
    }
    assert_eq!(
        format!("{client:?}"),
        format!("Client {{ base_url: \"http://127.0.0.1:{port}\", tokens: false, .. }}")
    );

    // Bodies that MessagePack cannot encode, and non-scalar parameters.
    struct Unencodable;
    impl Serialize for Unencodable {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("no wire form"))
        }
    }
    let err = get("/x").body(&Unencodable).expect_err("encode");
    assert_eq!(
        err.to_string(),
        "cannot encode the request of scripted: no wire form"
    );
    assert_eq!(
        param_string(&serde_json::json!({"a": 1}))
            .expect_err("object")
            .to_string(),
        "invalid parameter: only scalar parameters are supported, got an object"
    );
    assert_eq!(
        param_string(&Option::<u8>::None)
            .expect_err("null")
            .to_string(),
        "invalid parameter: only scalar parameters are supported, got null"
    );
    assert_eq!(
        param_string(&Unencodable)
            .expect_err("unserialisable")
            .to_string(),
        "invalid parameter: no wire form"
    );
    assert_eq!(get("/x").operation_id(), "scripted");
}

#[tokio::test]
async fn the_static_token_is_redacted_and_never_refreshes() {
    let token = StaticToken("secret-token".to_owned());
    assert_eq!(format!("{token:?}"), "StaticToken(<redacted>)");
    assert_eq!(
        token.access_token().await.expect("token"),
        Some("secret-token".to_owned())
    );
    assert_eq!(token.refresh().await.expect("refresh"), None);
}

// --- WebSocket ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Envelope<'a, P: Serialize> {
    v: u16,
    kind: &'a str,
    seq: u64,
    payload: P,
}

fn frame<P: Serialize>(kind: &str, seq: u64, payload: P) -> Message {
    Message::Binary(
        rmp_serde::to_vec_named(&Envelope {
            v: 1,
            kind,
            seq,
            payload,
        })
        .expect("encode")
        .into(),
    )
}

/// One scripted connection: the handshake answer and the messages sent after it.
struct Script {
    /// Reject the handshake with this status.
    reject: Option<u16>,
    messages: Vec<Message>,
}

/// What a WebSocket connection asked for.
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    uri: String,
    authorization: Option<String>,
}

/// A WebSocket server that plays one script per connection, in order.
async fn ws_server(scripts: Vec<Script>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        let mut scripts = VecDeque::from(scripts);
        while let Some(script) = scripts.pop_front() {
            let (stream, _) = listener.accept().await.expect("accept");
            let log = log.clone();
            let reject = script.reject;
            let callback = move |req: &WsRequest, resp| -> Result<_, ErrorResponse> {
                log.lock().expect("lock").push(Seen {
                    uri: req.uri().to_string(),
                    authorization: req
                        .headers()
                        .get("authorization")
                        .map(|v| v.to_str().expect("ascii").to_owned()),
                });
                match reject {
                    Some(status) => {
                        let mut r = ErrorResponse::new(None);
                        *r.status_mut() = status.try_into().expect("status");
                        Err(r)
                    }
                    None => Ok(resp),
                }
            };
            let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await else {
                continue;
            };
            for m in script.messages {
                if ws.send(m).await.is_err() {
                    break;
                }
            }
            // Keep the connection open until the client goes away.
            while let Some(Ok(_)) = futures_util::StreamExt::next(&mut ws).await {}
        }
    });
    (base, seen)
}

fn fast() -> StreamOptions {
    StreamOptions {
        backoff_initial: Duration::from_millis(1),
        backoff_max: Duration::from_millis(1),
        idle_timeout: Duration::from_secs(30),
        ..StreamOptions::default()
    }
}

#[tokio::test]
async fn control_frames_and_unknown_kinds_are_skipped_and_an_error_frame_ends_the_stream() {
    let (base, seen) = ws_server(vec![Script {
        reject: None,
        messages: vec![
            Message::Text("hello".into()),
            Message::Ping(vec![1].into()),
            frame("data", 5, 50u32),
            frame("future-kind", 6, "ignored"),
            frame("data", 5, 99u32),
            frame("reset", 9, ()),
            frame("data", 10, 100u32),
            frame("error", 11, problem("forbidden", 403)),
            frame("data", 12, 120u32),
        ],
    }])
    .await;
    let client = Client::new(&base).expect("client");
    let mut sub: Subscription<u32> = Subscription::new(
        &client,
        "/api/v1/stream".to_owned(),
        "stream",
        false,
        StreamOptions {
            resume_from: Some(3),
            ..fast()
        },
    );
    assert_eq!(
        sub.next().await.expect("item").expect("data"),
        StreamEvent::Data {
            seq: 5,
            payload: 50
        }
    );
    // The replayed seq 5 is skipped as a duplicate.
    assert_eq!(
        sub.next().await.expect("item").expect("reset"),
        StreamEvent::Reset { seq: 9 }
    );
    assert_eq!(
        sub.next().await.expect("item").expect("data"),
        StreamEvent::Data {
            seq: 10,
            payload: 100
        }
    );
    let err = sub.next().await.expect("item").expect_err("error frame");
    assert_eq!(
        err.api().map(ApiError::problem),
        Some(&problem("forbidden", 403))
    );
    assert!(sub.next().await.is_none(), "an error frame is final");
    assert_eq!(sub.last_seq(), Some(10));
    assert_eq!(sub.connections(), 1);
    assert_eq!(
        *seen.lock().expect("lock"),
        [Seen {
            uri: "/api/v1/stream?resume_from=3".to_owned(),
            authorization: None,
        }]
    );
}

#[tokio::test]
async fn an_undecodable_frame_is_a_decode_error_and_closes_the_stream() {
    let (base, _seen) = ws_server(vec![Script {
        reject: None,
        messages: vec![frame("data", 1, "not a number")],
    }])
    .await;
    let client = Client::new(&base).expect("client");
    let mut sub: Subscription<u32> =
        Subscription::new(&client, "/s".to_owned(), "numbers", false, fast());
    let err = sub.next().await.expect("item").expect_err("decode");
    assert!(
        matches!(&err, Error::Decode { operation: "numbers", message } if message.starts_with("payload: ")),
        "{err:?}"
    );
    assert!(sub.next().await.is_none());
    // Closing twice is harmless.
    sub.close().await;
    assert!(sub.next().await.is_none());
}

/// Hands out `old` until refreshed, then `new-<n>`; refreshing fails after `max` times.
#[derive(Debug, Default)]
struct Refreshing {
    refreshes: AtomicU32,
    max: u32,
}

impl TokenProvider for Refreshing {
    fn access_token(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async { Ok(Some("old".to_owned())) })
    }

    fn refresh(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async move {
            let n = self.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
            Ok((n <= self.max).then(|| format!("new-{n}")))
        })
    }
}

#[tokio::test]
async fn a_401_handshake_refreshes_the_token_once() {
    let (base, seen) = ws_server(vec![
        Script {
            reject: Some(401),
            messages: vec![],
        },
        Script {
            reject: None,
            messages: vec![frame("data", 1, 7u32), frame("end", 2, ())],
        },
    ])
    .await;
    let tokens = Arc::new(Refreshing {
        max: 1,
        ..Refreshing::default()
    });
    let client = Client::builder(&base)
        .tokens(tokens.clone())
        .build()
        .expect("client");
    let mut sub: Subscription<u32> =
        Subscription::new(&client, "/events".to_owned(), "events", true, fast());
    assert_eq!(
        sub.next().await.expect("item").expect("data"),
        StreamEvent::Data { seq: 1, payload: 7 }
    );
    assert!(sub.next().await.is_none(), "end");
    assert_eq!(
        *seen.lock().expect("lock"),
        [
            Seen {
                uri: "/events".to_owned(),
                authorization: Some("Bearer old".to_owned()),
            },
            Seen {
                uri: "/events".to_owned(),
                authorization: Some("Bearer new-1".to_owned()),
            },
        ]
    );
}

#[tokio::test]
async fn a_401_handshake_without_a_fresh_token_gives_up_with_the_handshake_error() {
    let (base, seen) = ws_server(vec![Script {
        reject: Some(401),
        messages: vec![],
    }])
    .await;
    let tokens = Arc::new(Refreshing::default());
    let client = Client::builder(&base)
        .tokens(tokens.clone())
        .build()
        .expect("client");
    let mut sub: Subscription<u32> = Subscription::new(
        &client,
        "/events".to_owned(),
        "events",
        true,
        StreamOptions {
            reconnect: false,
            ..fast()
        },
    );
    let err = sub.next().await.expect("item").expect_err("401");
    assert!(
        matches!(&err, Error::WebSocket(e) if matches!(&**e, tokio_tungstenite::tungstenite::Error::Http(r) if r.status() == 401)),
        "{err:?}"
    );
    assert!(err.to_string().starts_with("websocket: "), "{err}");
    assert!(sub.next().await.is_none());
    assert_eq!(tokens.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(seen.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn a_capped_reconnect_reports_the_attempts_made() {
    // Nothing listens: every connection attempt fails.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").port()
    };
    let client = Client::new(&format!("http://127.0.0.1:{port}")).expect("client");
    let mut sub: Subscription<u32> = Subscription::new(
        &client,
        "/s".to_owned(),
        "s",
        false,
        StreamOptions {
            max_reconnect_attempts: Some(2),
            ..fast()
        },
    );
    let err = sub.next().await.expect("item").expect_err("gave up");
    assert!(matches!(err, Error::WebSocket(_)), "{err:?}");
    assert!(sub.next().await.is_none());
    assert_eq!(sub.connections(), 0);
}

#[test]
fn frames_default_a_missing_payload_to_nil_and_reject_bad_error_payloads() {
    #[derive(Serialize)]
    struct Bare<'a> {
        v: u16,
        kind: &'a str,
        seq: u64,
    }
    let reset = rmp_serde::to_vec_named(&Bare {
        v: 1,
        kind: "reset",
        seq: 4,
    })
    .expect("encode");
    assert_eq!(decode_frame::<u8>(&reset), Ok(Frame::Reset { seq: 4 }));
    let Message::Binary(bad_error) = frame("error", 5, 1u8) else {
        panic!("binary frame")
    };
    assert!(
        decode_frame::<u8>(&bad_error)
            .expect_err("problem expected")
            .starts_with("payload: "),
    );
    let Message::Binary(v0) = frame("data", 1, 1u8) else {
        panic!("binary frame")
    };
    let mut v0 = v0.to_vec();
    v0[3] = 0x00;
    assert_eq!(
        decode_frame::<u8>(&v0),
        Err("unsupported frame version 0".to_owned())
    );
}
