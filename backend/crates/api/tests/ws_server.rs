//! WebSocket streaming over a real socket: frames, contract conformance, resume, reset, pong.

use futures_util::{SinkExt, StreamExt};
use pretty_assertions::assert_eq;
use strata_api::contract::Contract;
use strata_api::testing::TestServer;
use strata_api::testing::demo::{self, DemoState, DemoTick};
use strata_api::wire::DecodeLimits;
use strata_api::wire::ws::Frame;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn tick(n: u64) -> Frame<DemoTick> {
    Frame::Data {
        seq: n,
        payload: DemoTick {
            n,
            label: format!("tick {n}"),
        },
    }
}

fn server() -> TestServer {
    let state = actix_web::web::Data::new(DemoState::new(5, 3));
    TestServer::start(demo::configure(state)).expect("server starts")
}

/// Every frame until the server closes, validated against the contract.
async fn frames(server: &TestServer, query: &str) -> Vec<Frame<DemoTick>> {
    let url = format!(
        "ws://{}/api/v1/demo/ticks{query}",
        server.addr()
    );
    let (mut socket, _) = connect_async(url).await.expect("connects");
    let contract = Contract::new(demo::document());
    let mut out = Vec::new();
    while let Some(msg) = socket.next().await {
        match msg.expect("message") {
            Message::Binary(bytes) => {
                contract
                    .validate_frame("demo_ticks", &bytes)
                    .expect("frame conforms");
                out.push(Frame::decode(&bytes, &DecodeLimits::default()).expect("decodes"));
            }
            Message::Close(_) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    out
}

#[tokio::test]
async fn resume_after_a_retained_seq_replays_the_rest_then_ends() {
    let server = server();
    // Buffer capacity 3 retains seqs 3..=5.
    assert_eq!(
        frames(&server, "?resume_from=2").await,
        vec![tick(3), tick(4), tick(5), Frame::End { seq: 5 }]
    );
    assert_eq!(
        frames(&server, "?resume_from=4").await,
        vec![tick(5), Frame::End { seq: 5 }]
    );
}

#[tokio::test]
async fn resume_from_an_evicted_or_future_seq_resets() {
    let server = server();
    assert_eq!(frames(&server, "").await, vec![Frame::Reset { seq: 5 }]);
    assert_eq!(
        frames(&server, "?resume_from=9").await,
        vec![Frame::Reset { seq: 5 }]
    );
}

#[tokio::test]
async fn client_pings_get_pongs() {
    let state = actix_web::web::Data::new(DemoState::new(0, 3));
    state.hold_next_stream_after(0);
    let server = TestServer::start(demo::configure(state)).expect("server starts");
    let url = format!("ws://{}/api/v1/demo/ticks?resume_from=0", server.addr());
    let (mut socket, _) = connect_async(url).await.expect("connects");
    socket
        .send(Message::Ping(b"hb".to_vec().into()))
        .await
        .expect("sends");
    let mut got_pong = false;
    while let Some(Ok(msg)) = socket.next().await {
        if let Message::Pong(payload) = msg {
            assert_eq!(payload.as_ref(), b"hb");
            got_pong = true;
            break;
        }
    }
    assert!(got_pong, "server answered the ping");
}
