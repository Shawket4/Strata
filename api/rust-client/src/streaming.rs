//! WebSocket streams (D24): binary MessagePack frames `{v, kind, seq, payload}` decoded into
//! generated payload types, with automatic reconnect and resume-from-seq.
//!
//! A [`Subscription`] connects lazily on the first [`Subscription::next`]. When the connection
//! drops (error, close without `end`, or no traffic within [`StreamOptions::idle_timeout`]; the
//! server pings every 15 s), it reconnects with exponential backoff and
//! `?resume_from=<last seq>`. Duplicate data frames (seq already seen) are skipped. A `reset`
//! frame is surfaced as [`StreamEvent::Reset`]: refetch state, then keep reading. `end` finishes
//! the stream (`None`); an `error` frame finishes it with [`Error::Api`].

use std::marker::PhantomData;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
};

use crate::client::decode_exact;
use crate::types::Problem;
use crate::{ApiError, Client, Error};

/// Newest envelope version this client understands.
pub const FRAME_VERSION: u16 = 1;

/// Query parameter carrying the last received seq.
pub const RESUME_PARAM: &str = "resume_from";

/// Reconnect and liveness settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOptions {
    /// Resume after this seq on the first connection (e.g. persisted from a previous run).
    pub resume_from: Option<u64>,
    /// Reconnect after a drop.
    pub reconnect: bool,
    /// Give up after this many consecutive failed attempts (`None`: never).
    pub max_reconnect_attempts: Option<u32>,
    /// First backoff delay; doubles per consecutive failure.
    pub backoff_initial: Duration,
    /// Backoff ceiling.
    pub backoff_max: Duration,
    /// Treat the connection as dead after this long without any message.
    pub idle_timeout: Duration,
    /// Largest accepted frame.
    pub max_frame_size: usize,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            resume_from: None,
            reconnect: true,
            max_reconnect_attempts: None,
            backoff_initial: Duration::from_millis(500),
            backoff_max: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(45),
            max_frame_size: 1024 * 1024,
        }
    }
}

/// What a subscription yields.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent<P> {
    /// A stream item.
    Data {
        /// Its seq.
        seq: u64,
        /// The item.
        payload: P,
    },
    /// Items were missed and cannot be replayed: refetch state; the stream continues after
    /// `seq`.
    Reset {
        /// The server's head seq.
        seq: u64,
    },
}

#[derive(Deserialize)]
struct Envelope {
    v: u16,
    kind: String,
    seq: u64,
    #[serde(default = "nil")]
    payload: rmpv::Value,
}

fn nil() -> rmpv::Value {
    rmpv::Value::Nil
}

/// A decoded frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame<P> {
    /// `data`.
    Data {
        /// Seq.
        seq: u64,
        /// Payload.
        payload: P,
    },
    /// `error`.
    Error {
        /// Seq.
        seq: u64,
        /// Problem.
        problem: Problem,
    },
    /// `reset`.
    Reset {
        /// Seq.
        seq: u64,
    },
    /// `end`.
    End {
        /// Seq.
        seq: u64,
    },
    /// A kind this client does not know (newer server); ignored.
    Unknown,
}

fn from_payload<T: DeserializeOwned>(value: rmpv::Value) -> Result<T, String> {
    rmpv::ext::from_value(value).map_err(|e| format!("payload: {e}"))
}

/// Decodes one binary frame.
pub fn decode_frame<P: DeserializeOwned>(bytes: &[u8]) -> Result<Frame<P>, String> {
    let env: Envelope = decode_exact(bytes)?;
    if env.v == 0 || env.v > FRAME_VERSION {
        return Err(format!("unsupported frame version {}", env.v));
    }
    Ok(match env.kind.as_str() {
        "data" => Frame::Data {
            seq: env.seq,
            payload: from_payload(env.payload)?,
        },
        "error" => Frame::Error {
            seq: env.seq,
            problem: from_payload(env.payload)?,
        },
        "reset" => Frame::Reset { seq: env.seq },
        "end" => Frame::End { seq: env.seq },
        _ => Frame::Unknown,
    })
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A resumable stream subscription.
#[derive(Debug)]
pub struct Subscription<P> {
    client: Client,
    path: String,
    operation_id: &'static str,
    auth: bool,
    options: StreamOptions,
    last_seq: Option<u64>,
    socket: Option<Socket>,
    failures: u32,
    connections: u32,
    finished: bool,
    _payload: PhantomData<fn() -> P>,
}

impl<P: DeserializeOwned> Subscription<P> {
    /// A subscription to the stream at `path` (absolute, e.g. `/api/v1/events`). Used by the
    /// generated `streams` module.
    pub fn new(
        client: &Client,
        path: String,
        operation_id: &'static str,
        auth: bool,
        options: StreamOptions,
    ) -> Self {
        Self {
            client: client.clone(),
            path,
            operation_id,
            auth,
            last_seq: options.resume_from,
            options,
            socket: None,
            failures: 0,
            connections: 0,
            finished: false,
            _payload: PhantomData,
        }
    }

    /// Seq of the last data or reset frame received (persist it to resume later).
    pub fn last_seq(&self) -> Option<u64> {
        self.last_seq
    }

    /// Number of successful connections so far (1 + reconnects).
    pub fn connections(&self) -> u32 {
        self.connections
    }

    /// The next event; `None` once the server ended the stream or [`Self::close`] was called.
    pub async fn next(&mut self) -> Option<Result<StreamEvent<P>, Error>> {
        loop {
            if self.finished {
                return None;
            }
            let Some(socket) = self.socket.as_mut() else {
                if let Err(err) = self.connect().await
                    && let Some(fatal) = self.on_failure(err).await
                {
                    return Some(Err(fatal));
                }
                continue;
            };
            let message = tokio::time::timeout(self.options.idle_timeout, socket.next()).await;
            let bytes = match message {
                Ok(Some(Ok(Message::Binary(bytes)))) => bytes,
                Ok(Some(Ok(Message::Close(_)) | Err(_)) | None) | Err(_) => {
                    // Dropped, closed without `end`, or idle: reconnect and resume.
                    self.socket = None;
                    let dropped = Error::StreamClosed {
                        operation: self.operation_id,
                        attempts: self.failures,
                    };
                    if let Some(fatal) = self.on_failure(dropped).await {
                        return Some(Err(fatal));
                    }
                    continue;
                }
                Ok(Some(Ok(_))) => continue,
            };
            match decode_frame::<P>(&bytes) {
                Ok(Frame::Data { seq, payload }) => {
                    self.failures = 0;
                    if self.last_seq.is_some_and(|last| seq <= last) {
                        continue;
                    }
                    self.last_seq = Some(seq);
                    return Some(Ok(StreamEvent::Data { seq, payload }));
                }
                Ok(Frame::Reset { seq }) => {
                    self.failures = 0;
                    self.last_seq = Some(seq);
                    return Some(Ok(StreamEvent::Reset { seq }));
                }
                Ok(Frame::End { .. }) => {
                    self.close().await;
                    return None;
                }
                Ok(Frame::Error { problem, .. }) => {
                    self.close().await;
                    return Some(Err(ApiError::from_problem(problem).into()));
                }
                Ok(Frame::Unknown) => {}
                Err(message) => {
                    self.close().await;
                    return Some(Err(Error::Decode {
                        operation: self.operation_id,
                        message,
                    }));
                }
            }
        }
    }

    /// Closes the connection; further [`Self::next`] calls return `None`.
    pub async fn close(&mut self) {
        self.finished = true;
        if let Some(mut socket) = self.socket.take() {
            // Best effort: the peer may already be gone.
            let _ = socket.close(None).await;
        }
    }

    /// Records a failure; returns the error to surface when giving up, after sleeping the
    /// backoff otherwise.
    async fn on_failure(&mut self, err: Error) -> Option<Error> {
        let gave_up = !self.options.reconnect
            || self
                .options
                .max_reconnect_attempts
                .is_some_and(|max| self.failures >= max);
        if gave_up {
            self.finished = true;
            return Some(match err {
                Error::StreamClosed { operation, .. } => Error::StreamClosed {
                    operation,
                    attempts: self.failures,
                },
                other => other,
            });
        }
        let factor = 2u32.saturating_pow(self.failures.min(16));
        let delay = self
            .options
            .backoff_initial
            .saturating_mul(factor)
            .min(self.options.backoff_max);
        self.failures += 1;
        tracing::debug!(operation = self.operation_id, ?delay, "stream reconnecting");
        tokio::time::sleep(delay).await;
        None
    }

    async fn connect(&mut self) -> Result<(), Error> {
        let token = match (self.auth, self.client.tokens()) {
            (true, Some(tokens)) => tokens.access_token().await?,
            _ => None,
        };
        match self.handshake(token.as_deref()).await {
            Err(Error::WebSocket(e))
                if self.auth
                    && matches!(&*e, tokio_tungstenite::tungstenite::Error::Http(r) if r.status() == 401) =>
            {
                let fresh = match self.client.tokens() {
                    Some(tokens) => tokens.refresh().await?,
                    None => None,
                };
                match fresh {
                    Some(fresh) => self.handshake(Some(&fresh)).await,
                    None => Err(Error::WebSocket(e)),
                }
            }
            other => other,
        }
    }

    async fn handshake(&mut self, token: Option<&str>) -> Result<(), Error> {
        let base = self.client.base_url();
        let ws_base = if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            return Err(Error::Url(base.to_owned()));
        };
        let mut url = url::Url::parse(&format!("{ws_base}{}", self.path))
            .map_err(|e| Error::Url(format!("{}: {e}", self.path)))?;
        if let Some(seq) = self.last_seq {
            url.query_pairs_mut()
                .append_pair(RESUME_PARAM, &seq.to_string());
        }
        let mut request = url.as_str().into_client_request()?;
        if let Some(token) = token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| Error::Auth("token is not a valid header value".to_owned()))?;
            request.headers_mut().insert(AUTHORIZATION, value);
        }
        let config = WebSocketConfig::default()
            .max_message_size(Some(self.options.max_frame_size))
            .max_frame_size(Some(self.options.max_frame_size));
        // The client's own TLS configuration (bundled roots), never the platform store.
        let connector = Connector::Rustls(self.client.ws_tls());
        let (socket, _) =
            connect_async_tls_with_config(request, Some(config), true, Some(connector)).await?;
        self.socket = Some(socket);
        self.connections += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_decode_and_unknown_kinds_are_ignored() {
        // {"v":1,"kind":"end","seq":42,"payload":nil}
        let end = [
            0x84, 0xa1, 0x76, 0x01, 0xa4, 0x6b, 0x69, 0x6e, 0x64, 0xa3, 0x65, 0x6e, 0x64, 0xa3,
            0x73, 0x65, 0x71, 0x2a, 0xa7, 0x70, 0x61, 0x79, 0x6c, 0x6f, 0x61, 0x64, 0xc0,
        ];
        assert_eq!(decode_frame::<u8>(&end), Ok(Frame::End { seq: 42 }));
        let mut future = end;
        future[10..13].copy_from_slice(b"new");
        assert_eq!(decode_frame::<u8>(&future), Ok(Frame::Unknown));
        let mut v2 = end;
        v2[3] = 0x02;
        assert_eq!(
            decode_frame::<u8>(&v2),
            Err("unsupported frame version 2".to_owned())
        );
    }
}
