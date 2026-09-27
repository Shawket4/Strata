//! WebSocket streaming (D24): one binary MessagePack frame per event or token batch.
//!
//! Every frame is the envelope `{v, kind, seq, payload}` (see `docs/WIRE_FORMAT.md`):
//!
//! | kind    | payload                  | seq                                        |
//! |---------|--------------------------|--------------------------------------------|
//! | `data`  | the stream's item type   | the item's sequence number (resume key)    |
//! | `error` | [`Problem`]              | last data seq sent; the stream then closes |
//! | `reset` | nil                      | current head seq; client must refetch      |
//! | `end`   | nil                      | last data seq; the stream then closes      |
//!
//! Clients resume by reconnecting with `?resume_from=<last seq received>`. The server replays
//! frames after that seq from a [`ReplayBuffer`] or, if they were evicted, sends `reset`.
//!
//! [`serve`] runs one connection: it pulls frames only as fast as the socket drains
//! (backpressure; a send blocked longer than [`WsConfig::send_timeout`] closes the stream as a
//! slow consumer), pings every [`WsConfig::heartbeat_interval`], and closes after
//! [`WsConfig::idle_timeout`] without any client traffic (pongs count).

use std::collections::VecDeque;
use std::future::Future;
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, web};
use actix_ws::{CloseCode, CloseReason, Closed, Message, ProtocolError, Session};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, MissedTickBehavior};
use utoipa::{IntoParams, ToSchema};

use super::{DecodeError, DecodeLimits, Problem, decode, encode};

/// Envelope version sent in every frame (`v`). Clients reject frames with a newer version.
pub const FRAME_VERSION: u16 = 1;

/// Query parameter carrying the last seq a reconnecting client received.
pub const RESUME_PARAM: &str = "resume_from";

/// OpenAPI extension marking a stream operation; its value names the payload and frame
/// component schemas: `{"payload": "<Name>", "frame": "<Name>Frame"}`.
pub const STREAM_EXTENSION: &str = "x-strata-stream";

/// Envelope `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = StreamFrameKind)]
pub enum FrameKind {
    /// A stream item.
    Data,
    /// The stream failed; payload is a problem. Terminal.
    Error,
    /// Resume impossible; refetch state, then continue from `seq`.
    Reset,
    /// The stream finished normally. Terminal.
    End,
}

/// A typed frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame<P> {
    /// A stream item.
    Data {
        /// Sequence number (resume key).
        seq: u64,
        /// The item.
        payload: P,
    },
    /// Terminal failure.
    Error {
        /// Last data seq sent.
        seq: u64,
        /// What went wrong.
        problem: Problem,
    },
    /// Resume impossible.
    Reset {
        /// Current head seq.
        seq: u64,
    },
    /// Normal end.
    End {
        /// Last data seq.
        seq: u64,
    },
}

#[derive(Serialize)]
struct EnvelopeOut<'a, T: Serialize> {
    v: u16,
    kind: FrameKind,
    seq: u64,
    payload: &'a T,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    v: u16,
    kind: FrameKind,
    seq: u64,
    #[serde(default)]
    payload: rmpv::Value,
}

impl<P> Frame<P> {
    /// The frame's kind.
    pub fn kind(&self) -> FrameKind {
        match self {
            Self::Data { .. } => FrameKind::Data,
            Self::Error { .. } => FrameKind::Error,
            Self::Reset { .. } => FrameKind::Reset,
            Self::End { .. } => FrameKind::End,
        }
    }

    /// The frame's seq.
    pub fn seq(&self) -> u64 {
        match self {
            Self::Data { seq, .. }
            | Self::Error { seq, .. }
            | Self::Reset { seq }
            | Self::End { seq } => *seq,
        }
    }
}

impl<P: Serialize> Frame<P> {
    /// Encodes the frame as a binary MessagePack envelope.
    pub fn encode(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        let (kind, seq) = (self.kind(), self.seq());
        match self {
            Self::Data { payload, .. } => encode(&EnvelopeOut {
                v: FRAME_VERSION,
                kind,
                seq,
                payload,
            }),
            Self::Error { problem, .. } => encode(&EnvelopeOut {
                v: FRAME_VERSION,
                kind,
                seq,
                payload: problem,
            }),
            Self::Reset { .. } | Self::End { .. } => encode(&EnvelopeOut {
                v: FRAME_VERSION,
                kind,
                seq,
                payload: &(),
            }),
        }
    }
}

impl<P: DeserializeOwned> Frame<P> {
    /// Decodes a binary frame within `limits`.
    pub fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Self, DecodeError> {
        let env: EnvelopeIn = decode(bytes, limits)?;
        if env.v == 0 || env.v > FRAME_VERSION {
            return Err(DecodeError::Schema {
                pointer: "/v".to_owned(),
                message: format!("unsupported frame version (max {FRAME_VERSION})"),
            });
        }
        let payload_err = |_| DecodeError::Schema {
            pointer: "/payload".to_owned(),
            message: "payload does not match the frame kind".to_owned(),
        };
        Ok(match env.kind {
            FrameKind::Data => Self::Data {
                seq: env.seq,
                payload: rmpv::ext::from_value(env.payload).map_err(payload_err)?,
            },
            FrameKind::Error => Self::Error {
                seq: env.seq,
                problem: rmpv::ext::from_value(env.payload).map_err(payload_err)?,
            },
            FrameKind::Reset => Self::Reset { seq: env.seq },
            FrameKind::End => Self::End { seq: env.seq },
        })
    }
}

/// Query of a stream endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ResumeQuery {
    /// Last seq the client received; frames after it are replayed.
    #[param(minimum = 0)]
    pub resume_from: Option<u64>,
}

/// Connection settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsConfig {
    /// Server ping period.
    pub heartbeat_interval: Duration,
    /// Close when the client sent nothing (not even a pong) for this long.
    pub idle_timeout: Duration,
    /// Close as a slow consumer when one send blocks this long.
    pub send_timeout: Duration,
    /// Maximum size of a frame received from the client.
    pub max_incoming_frame: usize,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(45),
            send_timeout: Duration::from_secs(10),
            max_incoming_frame: 64 * 1024,
        }
    }
}

/// Why [`serve`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// The frame source finished; the socket was closed normally.
    SourceDone,
    /// The client closed the connection.
    ClientClosed,
    /// No client traffic within the idle timeout.
    IdleTimeout,
    /// A send blocked longer than the send timeout.
    SlowConsumer,
    /// The client violated the protocol (text frame, bad framing).
    ProtocolError,
    /// The socket was already gone.
    SinkClosed,
    /// A frame could not be encoded (a bug; logged).
    EncodeFailed,
}

/// Outgoing half of a WebSocket, abstracted so [`serve`] can run against a test double.
pub trait FrameSink {
    /// Sends one binary message; waits while the outgoing buffer is full.
    fn binary(&mut self, bytes: Bytes) -> impl Future<Output = Result<(), Closed>>;
    /// Sends a ping.
    fn ping(&mut self) -> impl Future<Output = Result<(), Closed>>;
    /// Answers a ping.
    fn pong(&mut self, payload: &[u8]) -> impl Future<Output = Result<(), Closed>>;
    /// Closes the socket.
    fn close(self, code: CloseCode, reason: &'static str) -> impl Future<Output = ()>;
}

impl FrameSink for Session {
    fn binary(&mut self, bytes: Bytes) -> impl Future<Output = Result<(), Closed>> {
        Session::binary(self, bytes)
    }

    fn ping(&mut self) -> impl Future<Output = Result<(), Closed>> {
        Session::ping(self, b"")
    }

    fn pong(&mut self, payload: &[u8]) -> impl Future<Output = Result<(), Closed>> {
        Session::pong(self, payload)
    }

    async fn close(self, code: CloseCode, reason: &'static str) {
        let reason = CloseReason {
            code,
            description: Some(reason.to_owned()),
        };
        // Already closed is fine.
        let _ = Session::close(self, Some(reason)).await;
    }
}

/// Runs one stream connection until it ends; see the module docs.
pub async fn serve<P, K, I, F>(mut sink: K, incoming: I, frames: F, config: &WsConfig) -> StreamEnd
where
    P: Serialize,
    K: FrameSink,
    I: Stream<Item = Result<Message, ProtocolError>> + Unpin,
    F: Stream<Item = Frame<P>> + Unpin,
{
    let mut incoming = incoming;
    let mut frames = frames;
    let mut heartbeat = tokio::time::interval_at(
        Instant::now() + config.heartbeat_interval,
        config.heartbeat_interval,
    );
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_seen = Instant::now();
    loop {
        tokio::select! {
            biased;
            msg = incoming.next() => match msg {
                Some(Ok(Message::Ping(payload))) => {
                    last_seen = Instant::now();
                    if sink.pong(&payload).await.is_err() {
                        return StreamEnd::SinkClosed;
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    sink.close(CloseCode::Normal, "bye").await;
                    return StreamEnd::ClientClosed;
                }
                Some(Ok(Message::Text(_))) => {
                    sink.close(CloseCode::Unsupported, "binary frames only").await;
                    return StreamEnd::ProtocolError;
                }
                Some(Ok(_)) => last_seen = Instant::now(),
                Some(Err(_)) => {
                    sink.close(CloseCode::Protocol, "protocol error").await;
                    return StreamEnd::ProtocolError;
                }
            },
            _ = heartbeat.tick() => {
                if last_seen.elapsed() >= config.idle_timeout {
                    sink.close(CloseCode::Away, "idle timeout").await;
                    return StreamEnd::IdleTimeout;
                }
                match tokio::time::timeout(config.send_timeout, sink.ping()).await {
                    Err(_) => {
                        sink.close(CloseCode::Again, "slow consumer").await;
                        return StreamEnd::SlowConsumer;
                    }
                    Ok(Err(_)) => return StreamEnd::SinkClosed,
                    Ok(Ok(())) => {}
                }
            }
            frame = frames.next() => {
                let Some(frame) = frame else {
                    sink.close(CloseCode::Normal, "end of stream").await;
                    return StreamEnd::SourceDone;
                };
                let bytes = match frame.encode() {
                    Ok(bytes) => Bytes::from(bytes),
                    Err(err) => {
                        tracing::error!(error = %err, "failed to encode a stream frame");
                        sink.close(CloseCode::Error, "internal error").await;
                        return StreamEnd::EncodeFailed;
                    }
                };
                match tokio::time::timeout(config.send_timeout, sink.binary(bytes)).await {
                    Err(_) => {
                        sink.close(CloseCode::Again, "slow consumer").await;
                        return StreamEnd::SlowConsumer;
                    }
                    Ok(Err(_)) => return StreamEnd::SinkClosed,
                    Ok(Ok(())) => {}
                }
            }
        }
    }
}

/// Upgrades `req` to a WebSocket and serves `frames` on the current Actix worker.
pub fn start<P, F>(
    req: &HttpRequest,
    body: web::Payload,
    config: WsConfig,
    frames: F,
) -> Result<HttpResponse, actix_web::Error>
where
    P: Serialize + 'static,
    F: Stream<Item = Frame<P>> + Unpin + 'static,
{
    let (response, session, incoming) = actix_ws::handle(req, body)?;
    let incoming = incoming.max_frame_size(config.max_incoming_frame);
    actix_web::rt::spawn(async move {
        let end = serve(session, incoming, frames, &config).await;
        tracing::debug!(?end, "stream connection ended");
    });
    Ok(response)
}

/// The last `capacity` data frames of a stream, for resuming clients.
#[derive(Debug, Clone)]
pub struct ReplayBuffer<P> {
    capacity: usize,
    frames: VecDeque<(u64, P)>,
    last_seq: u64,
}

impl<P: Clone> ReplayBuffer<P> {
    /// An empty buffer whose first frame gets seq 1.
    pub fn new(capacity: usize) -> Self {
        Self::starting_after(capacity, 0)
    }

    /// An empty buffer whose first frame gets seq `last_seq + 1` (e.g. a persisted change-log
    /// position).
    pub fn starting_after(capacity: usize, last_seq: u64) -> Self {
        Self {
            capacity: capacity.max(1),
            frames: VecDeque::new(),
            last_seq,
        }
    }

    /// Appends an item, evicting the oldest beyond capacity. Returns its seq.
    pub fn push(&mut self, payload: P) -> u64 {
        self.last_seq += 1;
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back((self.last_seq, payload));
        self.last_seq
    }

    /// Seq of the newest item (0 when none was ever pushed).
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Frames a client should receive when connecting with `resume_from`:
    /// nothing for a fresh subscription, the missed data frames when they are all retained,
    /// otherwise a single `reset` (evicted frames, or a seq from a previous server life).
    pub fn resume(&self, resume_from: Option<u64>) -> Vec<Frame<P>> {
        let Some(after) = resume_from else {
            return Vec::new();
        };
        let oldest = self.frames.front().map_or(self.last_seq + 1, |(seq, _)| *seq);
        if after > self.last_seq || after.saturating_add(1) < oldest {
            return vec![Frame::Reset { seq: self.last_seq }];
        }
        self.frames
            .iter()
            .filter(|(seq, _)| *seq > after)
            .map(|(seq, payload)| Frame::Data {
                seq: *seq,
                payload: payload.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
