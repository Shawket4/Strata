//! The generated client against an in-process server: typed round trips, typed problems,
//! token refresh, and streaming with reconnect/resume. Every HTTP response is validated
//! against the contract by a response observer.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

#[path = "demo/generated/mod.rs"]
mod generated;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::web::Data;
use futures_util::future::BoxFuture;
use generated::{operations as ops, streams, types};
use pretty_assertions::assert_eq;
use serde_bytes::ByteBuf;
use strata_api::contract::Contract;
use strata_api::testing::TestServer;
use strata_api::testing::demo::{self, DemoState, GOOD_TOKEN};
use strata_client::streaming::{StreamEvent, StreamOptions};
use strata_client::{
    ApiError, Client, Error, ObservedResponse, ResponseObserver, StaticToken, TokenProvider,
};

/// Validates every response against the demo contract and records statuses.
#[derive(Debug)]
struct Conformance {
    contract: Contract,
    seen: Mutex<Vec<(String, u16)>>,
    violations: Mutex<Vec<String>>,
}

impl Conformance {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            contract: Contract::new(demo::document()),
            seen: Mutex::new(Vec::new()),
            violations: Mutex::new(Vec::new()),
        })
    }

    fn seen(&self) -> Vec<(String, u16)> {
        self.seen.lock().expect("lock").clone()
    }

    fn assert_clean(&self) {
        assert_eq!(*self.violations.lock().expect("lock"), Vec::<String>::new());
    }
}

impl ResponseObserver for Conformance {
    fn observe(&self, r: &ObservedResponse<'_>) {
        self.seen
            .lock()
            .expect("lock")
            .push((r.operation_id.to_owned(), r.status));
        if let Err(e) =
            self.contract
                .validate_response(r.operation_id, r.status, r.content_type, r.body)
        {
            self.violations.lock().expect("lock").push(e.to_string());
        }
    }
}

struct Setup {
    _server: TestServer,
    state: Data<DemoState>,
    client: Client,
    conformance: Arc<Conformance>,
}

fn setup_with(state: DemoState, tokens: Arc<dyn TokenProvider>) -> Setup {
    let state = Data::new(state);
    let server = TestServer::start(demo::configure(state.clone())).expect("server starts");
    let conformance = Conformance::new();
    let client = Client::builder(&server.base_url())
        .tokens(tokens)
        .observer(conformance.clone())
        .build()
        .expect("client");
    Setup {
        _server: server,
        state,
        client,
        conformance,
    }
}

fn setup() -> Setup {
    setup_with(
        DemoState::new(5, 16),
        Arc::new(StaticToken(GOOD_TOKEN.to_owned())),
    )
}

fn create(name: &str, force: Option<bool>) -> types::CreateWidget {
    types::CreateWidget {
        name: name.to_owned(),
        shape: types::Shape::Rect {
            width: 3,
            height: 4,
        },
        tags: vec!["t".to_owned()],
        blob: ByteBuf::from(vec![1, 2, 3]),
        due: None,
        force,
    }
}

fn widget(n: u64, name: &str, tags: Vec<String>, version: &str) -> types::Widget {
    types::Widget {
        id: ulid::Ulid::from_parts(1_790_000_000_000 + n, u128::from(n)),
        name: name.to_owned(),
        shape: types::Shape::Rect {
            width: 3,
            height: 4,
        },
        tags,
        blob: ByteBuf::from(vec![1, 2, 3]),
        due: None,
        version: version.to_owned(),
        created: chrono::DateTime::from_timestamp(
            1_790_000_000 + i64::try_from(n).expect("small"),
            0,
        )
        .expect("valid"),
    }
}

#[tokio::test]
async fn typed_crud_round_trip() {
    let s = setup();
    let c = &s.client;
    let tags = vec!["t".to_owned()];

    let created = ops::create_widget(c, &create("alpha", None))
        .await
        .expect("created");
    assert_eq!(created, widget(1, "alpha", tags.clone(), "v1.1"));
    let id = created.id.to_string();

    assert_eq!(
        ops::get_widget(c, &id, Some(true)).await.expect("read"),
        widget(1, "alpha", tags.clone(), "v1.1")
    );
    assert_eq!(
        ops::get_widget(c, &id, None).await.expect("read"),
        widget(1, "alpha", vec![], "v1.1")
    );

    let renamed = ops::rename_widget(
        c,
        &id,
        "v1.1",
        &types::RenameWidget {
            name: "beta".to_owned(),
        },
    )
    .await
    .expect("renamed");
    assert_eq!(renamed, widget(1, "beta", tags, "v1.2"));

    ops::delete_widget(c, &id).await.expect("deleted");
    let err = ops::get_widget(c, &id, None).await.expect_err("gone");
    let Some(ApiError::NotFound(problem)) = err.api() else {
        panic!("expected not_found, got {err:?}");
    };
    assert_eq!(
        (
            problem.type_.as_str(),
            problem.status,
            problem.title.as_str()
        ),
        ("not_found", 404, "Not found")
    );

    assert_eq!(
        s.conformance.seen(),
        [
            ("create_widget", 201),
            ("get_widget", 200),
            ("get_widget", 200),
            ("rename_widget", 200),
            ("delete_widget", 204),
            ("get_widget", 404),
        ]
        .map(|(op, status)| (op.to_owned(), status))
    );
    s.conformance.assert_clean();
}

#[tokio::test]
async fn conflicts_and_duplicates_are_typed() {
    let s = setup();
    let c = &s.client;
    let first = ops::create_widget(c, &create("same", None))
        .await
        .expect("created");

    let err = ops::create_widget(c, &create("same", None))
        .await
        .expect_err("duplicate");
    let Some(ApiError::DuplicateCandidates {
        candidates,
        problem,
    }) = err.api()
    else {
        panic!("expected duplicate_candidates, got {err:?}");
    };
    assert_eq!(
        candidates,
        &vec![strata_client::types::DuplicateCandidate {
            id: first.id,
            kind: "widget".to_owned(),
            title: "same".to_owned(),
            snippet: None,
            match_level: strata_client::types::MatchLevel::Exact,
            score: 1.0,
        }]
    );
    assert_eq!(problem.status, 409);

    let forced = ops::create_widget(c, &create("same", Some(true)))
        .await
        .expect("forced");
    assert_eq!(forced.id, ulid::Ulid::from_parts(1_790_000_000_002, 2));

    let err = ops::rename_widget(
        c,
        &first.id.to_string(),
        "stale",
        &types::RenameWidget {
            name: "x".to_owned(),
        },
    )
    .await
    .expect_err("conflict");
    assert!(
        matches!(err.api(), Some(ApiError::VersionConflict { current_version: Some(v), .. }) if v == "v1.1"),
        "{err:?}"
    );
    s.conformance.assert_clean();
}

#[tokio::test]
async fn wire_limits_and_bad_ids_surface_as_problems() {
    let s = setup();
    let mut big = create("big", None);
    big.blob = ByteBuf::from(vec![0; demo::CREATE_BODY_LIMIT]);
    let err = ops::create_widget(&s.client, &big)
        .await
        .expect_err("too large");
    let Some(ApiError::Other(problem)) = err.api() else {
        panic!("expected a generic problem, got {err:?}");
    };
    assert_eq!(
        (problem.type_.as_str(), problem.status),
        ("payload_too_large", 413)
    );

    let err = ops::get_widget(&s.client, "not-a-ulid", None)
        .await
        .expect_err("bad id");
    assert!(matches!(err.api(), Some(ApiError::NotFound(_))), "{err:?}");

    let err = ops::get_widget(&s.client, "..", None)
        .await
        .expect_err("dot segment");
    assert!(matches!(err, Error::Param(_)), "{err:?}");
    s.conformance.assert_clean();
}

/// Hands out an expired token first, then a good one on refresh.
#[derive(Debug, Default)]
struct Rotating {
    refreshes: AtomicU32,
    can_refresh: bool,
}

impl TokenProvider for Rotating {
    fn access_token(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async { Ok(Some("expired".to_owned())) })
    }

    fn refresh(&self) -> BoxFuture<'_, Result<Option<String>, Error>> {
        Box::pin(async move {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(self.can_refresh.then(|| GOOD_TOKEN.to_owned()))
        })
    }
}

#[tokio::test]
async fn a_401_refreshes_the_token_once_and_retries() {
    let tokens = Arc::new(Rotating {
        can_refresh: true,
        ..Rotating::default()
    });
    let s = setup_with(DemoState::default(), tokens.clone());
    let me = ops::whoami(&s.client).await.expect("after refresh");
    assert_eq!(
        me,
        types::WhoAmI {
            subject: "demo-user".to_owned()
        }
    );
    assert_eq!(tokens.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.conformance.seen(),
        vec![("whoami".to_owned(), 401), ("whoami".to_owned(), 200)]
    );
    s.conformance.assert_clean();
}

#[tokio::test]
async fn a_failed_refresh_surfaces_unauthorized() {
    let tokens = Arc::new(Rotating::default());
    let s = setup_with(DemoState::default(), tokens.clone());
    let err = ops::whoami(&s.client).await.expect_err("unauthorized");
    assert!(
        matches!(err.api(), Some(ApiError::Unauthorized(_))),
        "{err:?}"
    );
    assert_eq!(tokens.refreshes.load(Ordering::SeqCst), 1);
    s.conformance.assert_clean();
}

fn fast(options: StreamOptions) -> StreamOptions {
    StreamOptions {
        backoff_initial: Duration::ZERO,
        ..options
    }
}

async fn collect(
    sub: &mut strata_client::streaming::Subscription<types::DemoTick>,
) -> Vec<Result<StreamEvent<types::DemoTick>, String>> {
    let mut out = Vec::new();
    while let Some(event) = sub.next().await {
        out.push(event.map_err(|e| e.to_string()));
    }
    out
}

fn ticks(
    range: std::ops::RangeInclusive<u64>,
) -> Vec<Result<StreamEvent<types::DemoTick>, String>> {
    range
        .map(|n| {
            Ok(StreamEvent::Data {
                seq: n,
                payload: types::DemoTick {
                    n,
                    label: format!("tick {n}"),
                },
            })
        })
        .collect()
}

#[tokio::test]
async fn a_dropped_stream_reconnects_and_resumes_after_the_last_seq() {
    let s = setup();
    s.state.cut_next_stream_after(2);
    let mut sub =
        streams::demo_ticks(&s.client, fast(StreamOptions::default())).expect("subscribes");
    assert_eq!(collect(&mut sub).await, ticks(1..=5));
    assert_eq!(sub.connections(), 2);
    assert_eq!(sub.last_seq(), Some(5));
}

#[tokio::test]
async fn an_idle_stream_is_abandoned_and_resumed() {
    let s = setup();
    s.state.hold_next_stream_after(1);
    let options = fast(StreamOptions {
        idle_timeout: Duration::from_millis(300),
        ..StreamOptions::default()
    });
    let mut sub = streams::demo_ticks(&s.client, options).expect("subscribes");
    assert_eq!(collect(&mut sub).await, ticks(1..=5));
    assert_eq!(sub.connections(), 2);
}

#[tokio::test]
async fn resuming_past_the_retained_window_yields_a_reset() {
    let s = setup_with(
        DemoState::new(5, 3),
        Arc::new(StaticToken(GOOD_TOKEN.to_owned())),
    );
    let options = fast(StreamOptions {
        resume_from: Some(1),
        ..StreamOptions::default()
    });
    let mut sub = streams::demo_ticks(&s.client, options).expect("subscribes");
    assert_eq!(
        collect(&mut sub).await,
        vec![Ok(StreamEvent::Reset { seq: 5 })]
    );
    // After the reset the client resumed from 5 and the server ended the stream.
    assert_eq!(sub.connections(), 2);
}

#[tokio::test]
async fn without_reconnect_a_drop_is_an_error() {
    let s = setup();
    s.state.cut_next_stream_after(1);
    let options = StreamOptions {
        reconnect: false,
        ..StreamOptions::default()
    };
    let mut sub = streams::demo_ticks(&s.client, options).expect("subscribes");
    let mut expected = ticks(1..=1);
    expected.push(Err(
        "stream demo_ticks disconnected after 0 reconnect attempts".to_owned(),
    ));
    assert_eq!(collect(&mut sub).await, expected);
}

#[tokio::test]
async fn production_health_through_the_real_generated_client() {
    let server = TestServer::start(strata_api::app::configure).expect("server starts");
    let client = Client::new(&server.base_url()).expect("client");
    assert_eq!(
        strata_client::operations::health(&client)
            .await
            .expect("healthy"),
        strata_client::types::Health {
            status: strata_client::types::HealthStatus::Ok
        }
    );
}
