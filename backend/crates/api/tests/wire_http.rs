//! Extractor, responder, negotiation and problem conversion through a real Actix service,
//! using an in-test router (no production demo code).
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use actix_web::http::StatusCode;
use actix_web::http::header::{ACCEPT, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::{App, test, web};
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use strata_api::app::{api_v1, routes};
use strata_api::contract::Contract;
use strata_api::wire::{
    DecodeLimits, MSGPACK, MsgPack, MsgPackConfig, PROBLEM_MSGPACK, Problem, ProblemFieldError,
    ProblemType, encode,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Echo {
    name: String,
    count: u32,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Page {
    limit: u32,
}

async fn echo(body: MsgPack<Echo>) -> MsgPack<Echo> {
    body
}

async fn item(id: web::Path<u32>, page: web::Query<Page>) -> MsgPack<(u32, u32)> {
    MsgPack((*id, page.limit))
}

const LIMITS: DecodeLimits = DecodeLimits {
    max_depth: 64,
    max_str_len: 16,
    max_bin_len: 16,
    max_array_len: 4,
    max_map_len: 8,
};

fn test_routes(cfg: &mut web::ServiceConfig) {
    routes(cfg);
    cfg.service(
        web::resource("/echo")
            .app_data(MsgPackConfig {
                body_limit: 256,
                limits: LIMITS,
            })
            .route(web::post().to(echo)),
    )
    .route("/items/{id}", web::get().to(item));
}

struct Reply {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl Reply {
    fn problem(&self) -> Problem {
        assert_eq!(self.content_type.as_deref(), Some(PROBLEM_MSGPACK));
        rmp_serde::from_slice(&self.body).expect("problem decodes")
    }
}

async fn send(req: test::TestRequest) -> Reply {
    let app = test::init_service(App::new().service(api_v1(test_routes))).await;
    let res = test::call_service(&app, req.to_request()).await;
    let status = res.status();
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = test::read_body(res).await.to_vec();
    Reply {
        status,
        content_type,
        body,
    }
}

fn post_echo(body: Vec<u8>) -> test::TestRequest {
    test::TestRequest::post()
        .uri("/api/v1/echo")
        .insert_header((CONTENT_TYPE, MSGPACK))
        .set_payload(body)
}

fn echo_value() -> Echo {
    Echo {
        name: "ada".to_owned(),
        count: 3,
        tags: vec!["x".to_owned()],
    }
}

fn invalid_body(code: &str, pointer: Option<&str>, message: &str) -> Problem {
    Problem::new(ProblemType::InvalidBody)
        .with_detail(message)
        .with_error(ProblemFieldError {
            code: code.to_owned(),
            pointer: pointer.map(str::to_owned),
            message: message.to_owned(),
        })
}

#[actix_web::test]
async fn valid_body_round_trips_as_named_map() {
    let value = echo_value();
    let reply = send(post_echo(encode(&value).expect("encodes"))).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type.as_deref(), Some(MSGPACK));
    // {"name": "ada", "count": 3, "tags": ["x"]}
    assert_eq!(
        hex::encode(&reply.body),
        "83a46e616d65a3616461a5636f756e7403a47461677391a178"
    );
}

#[actix_web::test]
async fn media_type_parameters_are_ignored() {
    let req = test::TestRequest::post()
        .uri("/api/v1/echo")
        .insert_header((CONTENT_TYPE, "Application/VND.MsgPack; charset=binary"))
        .set_payload(encode(&echo_value()).expect("encodes"));
    assert_eq!(send(req).await.status, StatusCode::OK);
}

#[actix_web::test]
async fn missing_or_wrong_content_type_is_415() {
    let expected = Problem::new(ProblemType::UnsupportedMediaType)
        .with_detail("request bodies must be application/vnd.msgpack");
    let body = encode(&echo_value()).expect("encodes");
    let none = test::TestRequest::post()
        .uri("/api/v1/echo")
        .set_payload(body.clone());
    let json = test::TestRequest::post()
        .uri("/api/v1/echo")
        .insert_header((CONTENT_TYPE, "application/json"))
        .set_payload(body.clone());
    // The unregistered pre-IANA name is not an alias.
    let unregistered = test::TestRequest::post()
        .uri("/api/v1/echo")
        .insert_header((CONTENT_TYPE, "application/msgpack"))
        .set_payload(body);
    for req in [none, json, unregistered] {
        let reply = send(req).await;
        assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(reply.problem(), expected);
    }
}

#[actix_web::test]
async fn content_encoded_bodies_are_415() {
    let req = post_echo(encode(&echo_value()).expect("encodes"))
        .insert_header((CONTENT_ENCODING, "gzip"));
    let reply = send(req).await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        reply.problem(),
        Problem::new(ProblemType::UnsupportedMediaType)
            .with_detail("content-encoded request bodies are not accepted")
    );
}

#[actix_web::test]
async fn bodies_over_the_route_limit_are_413_whether_declared_or_streamed() {
    let expected = Problem::new(ProblemType::PayloadTooLarge)
        .with_detail("the body limit for this route is 256 bytes");
    let declared = post_echo(vec![0xc0; 257]);
    // Understated Content-Length: the limit is enforced while reading.
    let streamed = post_echo(vec![0xc0; 257]).insert_header((CONTENT_LENGTH, "10"));
    for req in [declared, streamed] {
        let reply = send(req).await;
        assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(reply.problem(), expected);
    }
}

#[actix_web::test]
async fn trailing_bytes_are_422() {
    let mut body = encode(&echo_value()).expect("encodes");
    body.extend_from_slice(&[0x01, 0x02]);
    let reply = send(post_echo(body)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reply.problem(),
        invalid_body(
            "trailing_bytes",
            None,
            "2 trailing bytes after the MessagePack value"
        )
    );
    // Exact wire bytes of this problem (see docs/WIRE_FORMAT.md).
    assert_eq!(reply.body, reply.problem().to_msgpack());
}

#[actix_web::test]
async fn depth_bomb_is_422() {
    let mut body = vec![0x91u8; 65];
    body.push(0x01);
    let reply = send(post_echo(body)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reply.problem(),
        invalid_body(
            "depth_exceeded",
            None,
            "nesting exceeds the maximum depth of 64"
        )
    );
}

#[actix_web::test]
async fn oversized_strings_and_arrays_are_422() {
    let long_name = Echo {
        name: "a".repeat(17),
        ..echo_value()
    };
    let many_tags = Echo {
        tags: vec![String::new(); 5],
        ..echo_value()
    };
    let cases = [
        (
            long_name,
            invalid_body(
                "string_too_long",
                None,
                "a string exceeds the maximum length of 16 bytes",
            ),
        ),
        (
            many_tags,
            invalid_body(
                "array_too_long",
                None,
                "an array exceeds the maximum of 4 elements",
            ),
        ),
    ];
    for (value, expected) in cases {
        let reply = send(post_echo(encode(&value).expect("encodes"))).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(reply.problem(), expected);
    }
}

#[actix_web::test]
async fn unknown_fields_are_ignored() {
    #[derive(Serialize)]
    struct Newer<'a> {
        name: &'a str,
        extra: &'a str,
        count: u32,
    }
    let body = encode(&Newer {
        name: "ada",
        extra: "newer",
        count: 3,
    })
    .expect("encodes");
    let reply = send(post_echo(body)).await;
    assert_eq!(reply.status, StatusCode::OK);
    let echoed: Echo = rmp_serde::from_slice(&reply.body).expect("decodes");
    assert_eq!(
        echoed,
        Echo {
            name: "ada".to_owned(),
            count: 3,
            tags: vec![],
        }
    );
}

#[actix_web::test]
async fn type_mismatches_point_at_the_field_without_echoing_content() {
    #[derive(Serialize)]
    struct Wrong<'a> {
        name: &'a str,
        count: &'a str,
    }
    let body = encode(&Wrong {
        name: "ada",
        count: "s3cret",
    })
    .expect("encodes");
    let reply = send(post_echo(body)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reply.problem(),
        invalid_body("schema_mismatch", Some("/count"), "invalid type")
    );
    assert!(!String::from_utf8_lossy(&reply.body).contains("s3cret"));

    let missing = encode(&serde_json::json!({ "name": "ada" })).expect("encodes");
    assert_eq!(
        send(post_echo(missing)).await.problem(),
        invalid_body("schema_mismatch", Some(""), "missing field `count`")
    );
}

#[actix_web::test]
async fn accept_excluding_msgpack_is_406() {
    let req = test::TestRequest::get()
        .uri("/api/v1/health")
        .insert_header((ACCEPT, "application/json"));
    let reply = send(req).await;
    assert_eq!(reply.status, StatusCode::NOT_ACCEPTABLE);
    assert_eq!(
        reply.problem(),
        Problem::new(ProblemType::NotAcceptable)
            .with_detail("responses are application/vnd.msgpack")
    );
}

#[actix_web::test]
async fn unknown_routes_and_methods_become_problems() {
    let reply = send(test::TestRequest::get().uri("/api/v1/nope")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.problem(), Problem::new(ProblemType::RouteNotFound));

    let reply = send(test::TestRequest::get().uri("/api/v1/echo")).await;
    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.problem(), Problem::new(ProblemType::MethodNotAllowed));
}

#[actix_web::test]
async fn malformed_path_is_404_and_malformed_query_is_422() {
    let reply = send(test::TestRequest::get().uri("/api/v1/items/abc?limit=1")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.problem(), Problem::new(ProblemType::NotFound));

    let reply = send(test::TestRequest::get().uri("/api/v1/items/7?limit=lots")).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = "invalid query string";
    assert_eq!(
        reply.problem(),
        Problem::new(ProblemType::InvalidParameter)
            .with_detail(message)
            .with_error(ProblemFieldError {
                code: "invalid_query".to_owned(),
                pointer: None,
                message: message.to_owned(),
            })
    );

    let reply = send(test::TestRequest::get().uri("/api/v1/items/7?limit=9")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, encode(&(7u32, 9u32)).expect("encodes"));
}

#[actix_web::test]
async fn health_is_exact_and_conforms_to_the_contract() {
    let reply = send(test::TestRequest::get().uri("/api/v1/health")).await;
    assert_eq!(reply.status, StatusCode::OK);
    // {"status": "ok"}
    assert_eq!(hex::encode(&reply.body), "81a6737461747573a26f6b");
    let json = Contract::production()
        .validate_response("health", 200, reply.content_type.as_deref(), &reply.body)
        .expect("conforms");
    assert_eq!(json, serde_json::json!({ "status": "ok" }));
}

#[actix_web::test]
async fn health_answers_head_like_get() {
    // Uptime monitors and `curl -I` probe with HEAD; it must not be a 404 or 405.
    let req = test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri("/api/v1/health");
    let reply = send(req).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.content_type.as_deref(),
        Some("application/vnd.msgpack")
    );
}

#[actix_web::test]
async fn every_problem_response_conforms_to_the_problem_schema() {
    let contract = Contract::production();
    let reply = send(test::TestRequest::get().uri("/api/v1/nope")).await;
    contract
        .validate_component("Problem", &reply.body)
        .expect("route_not_found conforms");
    let reply = send(post_echo(vec![0x01, 0x02])).await;
    contract
        .validate_component("Problem", &reply.body)
        .expect("invalid_body conforms");
    let reply = send(
        test::TestRequest::get()
            .uri("/api/v1/health")
            .insert_header((ACCEPT, "text/html")),
    )
    .await;
    contract
        .validate_response("health", 406, reply.content_type.as_deref(), &reply.body)
        .expect("406 conforms");
}
