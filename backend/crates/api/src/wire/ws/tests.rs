use std::time::Duration;

use actix_ws::{CloseCode, Closed, Message, ProtocolError};
use bytes::Bytes;
use futures_util::{Stream, StreamExt, stream};
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::*;
use crate::wire::{Problem, ProblemType};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ev {
    Binary(Vec<u8>),
    Ping,
    Pong(Vec<u8>),
    Close(u16, &'static str),
}

/// Records every call with the (fake) time since `start`.
struct RecordingSink {
    start: Instant,
    events: mpsc::UnboundedSender<(Duration, Ev)>,
    stall_binary: bool,
}

impl RecordingSink {
    fn record(&self, ev: Ev) {
        let _ = self.events.send((self.start.elapsed(), ev));
    }
}

impl FrameSink for RecordingSink {
    async fn binary(&mut self, bytes: Bytes) -> Result<(), Closed> {
        if self.stall_binary {
            std::future::pending::<()>().await;
        }
        self.record(Ev::Binary(bytes.to_vec()));
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), Closed> {
        self.record(Ev::Ping);
        Ok(())
    }

    async fn pong(&mut self, payload: &[u8]) -> Result<(), Closed> {
        self.record(Ev::Pong(payload.to_vec()));
        Ok(())
    }

    async fn close(self, code: CloseCode, reason: &'static str) {
        self.record(Ev::Close(u16::from(code), reason));
    }
}

fn sink(stall_binary: bool) -> (RecordingSink, mpsc::UnboundedReceiver<(Duration, Ev)>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        RecordingSink {
            start: Instant::now(),
            events: tx,
            stall_binary,
        },
        rx,
    )
}

fn drain(mut rx: mpsc::UnboundedReceiver<(Duration, Ev)>) -> Vec<(u64, Ev)> {
    let mut out = Vec::new();
    while let Ok((at, ev)) = rx.try_recv() {
        out.push((at.as_secs(), ev));
    }
    out
}

type Incoming = std::pin::Pin<Box<dyn Stream<Item = Result<Message, ProtocolError>>>>;

fn silent() -> Incoming {
    Box::pin(stream::pending())
}

/// Yields `messages` at the given offsets (seconds), then stays open.
fn scripted(messages: Vec<(u64, Message)>) -> Incoming {
    let start = Instant::now();
    Box::pin(
        stream::iter(messages)
            .then(move |(at, msg)| async move {
                tokio::time::sleep_until(start + Duration::from_secs(at)).await;
                Ok(msg)
            })
            .chain(stream::pending()),
    )
}

fn no_frames() -> impl Stream<Item = Frame<u8>> + Unpin {
    stream::pending()
}

#[tokio::test(start_paused = true)]
async fn silent_client_is_pinged_then_closed_at_the_idle_timeout() {
    let (sink, rx) = sink(false);
    let end = serve(sink, silent(), no_frames(), &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::IdleTimeout);
    assert_eq!(
        drain(rx),
        vec![
            (15, Ev::Ping),
            (30, Ev::Ping),
            (45, Ev::Close(1001, "idle timeout")),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn pongs_keep_the_connection_alive_until_they_stop() {
    let (sink, rx) = sink(false);
    let pongs = (1..=5)
        .map(|i| (i * 10, Message::Pong(Bytes::new())))
        .collect();
    let end = serve(sink, scripted(pongs), no_frames(), &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::IdleTimeout);
    // Last pong at 50 s; the tick at 105 s is the first with 45 s of silence.
    assert_eq!(
        drain(rx),
        vec![
            (15, Ev::Ping),
            (30, Ev::Ping),
            (45, Ev::Ping),
            (60, Ev::Ping),
            (75, Ev::Ping),
            (90, Ev::Ping),
            (105, Ev::Close(1001, "idle timeout")),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn client_pings_are_answered_with_the_same_payload() {
    let (sink, rx) = sink(false);
    let incoming = scripted(vec![
        (1, Message::Ping(Bytes::from_static(b"hi"))),
        (2, Message::Close(None)),
    ]);
    let end = serve(sink, incoming, no_frames(), &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::ClientClosed);
    assert_eq!(
        drain(rx),
        vec![(1, Ev::Pong(b"hi".to_vec())), (2, Ev::Close(1000, "bye"))]
    );
}

#[tokio::test(start_paused = true)]
async fn text_frames_are_a_protocol_violation() {
    let (sink, rx) = sink(false);
    let incoming = scripted(vec![(3, Message::Text("hello".into()))]);
    let end = serve(sink, incoming, no_frames(), &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::ProtocolError);
    assert_eq!(drain(rx), vec![(3, Ev::Close(1003, "binary frames only"))]);
}

#[tokio::test(start_paused = true)]
async fn frames_are_sent_in_order_then_the_socket_closes_normally() {
    let (sink, rx) = sink(false);
    let frames = vec![
        Frame::Data {
            seq: 1,
            payload: 7u8,
        },
        Frame::End { seq: 1 },
    ];
    let expected: Vec<(u64, Ev)> = frames
        .iter()
        .map(|f| (0, Ev::Binary(f.encode().expect("encodes"))))
        .chain([(0, Ev::Close(1000, "end of stream"))])
        .collect();
    let end = serve(sink, silent(), stream::iter(frames), &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::SourceDone);
    assert_eq!(drain(rx), expected);
}

#[tokio::test(start_paused = true)]
async fn a_send_blocked_past_the_send_timeout_closes_as_slow_consumer() {
    let (sink, rx) = sink(true);
    let frames = stream::iter(vec![Frame::Data {
        seq: 1,
        payload: 1u8,
    }]);
    let end = serve(sink, silent(), frames, &WsConfig::default()).await;
    assert_eq!(end, StreamEnd::SlowConsumer);
    assert_eq!(drain(rx), vec![(10, Ev::Close(1013, "slow consumer"))]);
}

#[test]
fn frames_round_trip_through_encode_and_decode() {
    let limits = DecodeLimits::default();
    let frames: Vec<Frame<String>> = vec![
        Frame::Data {
            seq: 42,
            payload: "x".to_owned(),
        },
        Frame::Error {
            seq: 42,
            problem: Problem::new(ProblemType::Internal),
        },
        Frame::Reset { seq: 7 },
        Frame::End { seq: 42 },
    ];
    for frame in frames {
        let bytes = frame.encode().expect("encodes");
        assert_eq!(Frame::<String>::decode(&bytes, &limits), Ok(frame));
    }
}

#[test]
fn newer_frame_versions_and_mismatched_payloads_are_rejected() {
    let limits = DecodeLimits::default();
    let v2 = crate::wire::encode(&serde_json::json!({"v": 2, "kind": "end", "seq": 1}))
        .expect("encodes");
    assert_eq!(
        Frame::<u8>::decode(&v2, &limits),
        Err(DecodeError::Schema {
            pointer: "/v".to_owned(),
            message: "unsupported frame version (max 1)".to_owned(),
        })
    );
    let bad = Frame::Data {
        seq: 1,
        payload: "text",
    }
    .encode()
    .expect("encodes");
    assert_eq!(
        Frame::<u8>::decode(&bad, &limits),
        Err(DecodeError::Schema {
            pointer: "/payload".to_owned(),
            message: "payload does not match the frame kind".to_owned(),
        })
    );
}

fn data(seqs: &[u64]) -> Vec<Frame<u64>> {
    seqs.iter()
        .map(|&seq| Frame::Data {
            seq,
            payload: seq * 10,
        })
        .collect()
}

#[test]
fn replay_buffer_resumes_or_resets() {
    let mut buffer = ReplayBuffer::new(3);
    for n in 1..=5 {
        assert_eq!(buffer.push(n * 10), n);
    }
    // Retained: 3, 4, 5.
    assert_eq!(buffer.resume(None), vec![]);
    assert_eq!(buffer.resume(Some(2)), data(&[3, 4, 5]));
    assert_eq!(buffer.resume(Some(4)), data(&[5]));
    assert_eq!(buffer.resume(Some(5)), vec![]);
    assert_eq!(buffer.resume(Some(1)), vec![Frame::Reset { seq: 5 }]);
    assert_eq!(buffer.resume(Some(9)), vec![Frame::Reset { seq: 5 }]);
    assert_eq!(buffer.last_seq(), 5);

    let empty: ReplayBuffer<u64> = ReplayBuffer::starting_after(4, 100);
    assert_eq!(empty.resume(Some(100)), vec![]);
    assert_eq!(empty.resume(Some(99)), vec![Frame::Reset { seq: 100 }]);
}
