//! A minimal raw HTTP/1.1 client for hostile requests.
//!
//! `reqwest` (through the `url` crate) normalises `..`, `%2e%2e` and backslashes in request
//! targets before sending, which would hide exactly what the traversal tests probe. This
//! client writes the request target byte for byte, sends `Connection: close` and reads the
//! response (Content-Length, chunked or until EOF; for `101 Switching Protocols` only the
//! head).

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A request.
#[derive(Debug, Clone, Default)]
pub struct Req {
    /// `GET`, `POST`, ...
    pub method: String,
    /// The request target exactly as sent (`/api/v1/...?...`).
    pub target: String,
    /// Extra headers.
    pub headers: Vec<(String, String)>,
    /// Bearer token.
    pub token: Option<String>,
    /// Body and its content type.
    pub body: Option<(String, Vec<u8>)>,
    /// Ask for a WebSocket upgrade (streams).
    pub upgrade: bool,
    /// Declare this `Content-Length` instead of the body's (limits tests).
    pub declared_len: Option<usize>,
}

impl Req {
    /// A request without body.
    pub fn new(method: &str, target: impl Into<String>) -> Self {
        Self {
            method: method.to_owned(),
            target: target.into(),
            ..Self::default()
        }
    }

    /// With a bearer token.
    #[must_use]
    pub fn token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    /// With a MessagePack body.
    #[must_use]
    pub fn msgpack(mut self, body: Vec<u8>) -> Self {
        self.body = Some((strata_api::wire::MSGPACK.to_owned(), body));
        self
    }

    /// With a body of any content type.
    #[must_use]
    pub fn body(mut self, content_type: &str, body: Vec<u8>) -> Self {
        self.body = Some((content_type.to_owned(), body));
        self
    }

    /// With a header.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The request bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut head = format!(
            "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: {}\r\n",
            self.method,
            self.target,
            strata_api::wire::MSGPACK
        );
        if self.upgrade {
            head.push_str(
                "Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
                 Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n",
            );
        } else {
            head.push_str("Connection: close\r\n");
        }
        if let Some(token) = &self.token {
            head.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        for (k, v) in &self.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        let mut out;
        if let Some((ct, body)) = &self.body {
            head.push_str(&format!(
                "Content-Type: {ct}\r\nContent-Length: {}\r\n\r\n",
                self.declared_len.unwrap_or(body.len())
            ));
            out = head.into_bytes();
            out.extend_from_slice(body);
        } else {
            head.push_str("Content-Length: 0\r\n\r\n");
            out = head.into_bytes();
        }
        out
    }
}

/// A response.
#[derive(Debug, Clone)]
pub struct Resp {
    /// Status code.
    pub status: u16,
    /// Headers (names lower-cased).
    pub headers: Vec<(String, String)>,
    /// Decoded body (chunked transfer decoded).
    pub body: Vec<u8>,
}

impl Resp {
    /// A header value.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// `Content-Type`.
    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    /// True for `application/problem+msgpack`.
    pub fn is_problem(&self) -> bool {
        self.content_type()
            .is_some_and(|c| c.starts_with(strata_api::wire::PROBLEM_MSGPACK))
    }

    /// The problem `type` slug, if the body is a problem.
    pub fn problem_type(&self) -> Option<String> {
        if !self.is_problem() {
            return None;
        }
        let value = rmpv::decode::read_value(&mut self.body.as_slice()).ok()?;
        value.as_map()?.iter().find_map(|(k, v)| {
            (k.as_str() == Some("type")).then(|| v.as_str().map(str::to_owned))?
        })
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Sends `req` to `addr` and reads the response (30 s timeout; a hang is a failure).
pub async fn send(addr: SocketAddr, req: &Req) -> Resp {
    tokio::time::timeout(Duration::from_secs(30), exchange(addr, req))
        .await
        .unwrap_or_else(|_| panic!("{} {} timed out", req.method, req.target))
}

async fn exchange(addr: SocketAddr, req: &Req) -> Resp {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    // The server may answer (413) and close before reading a declared-but-unsent body.
    let _ = stream.write_all(&req.to_bytes()).await;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i;
        }
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        assert!(n > 0, "connection closed before the response head");
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status: u16 = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("bad status line {status_line:?}"));
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let mut rest = buf[head_end + 4..].to_vec();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    if status == 101 {
        return Resp {
            status,
            headers,
            body: Vec::new(),
        };
    }
    if let Some(len) = header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        while rest.len() < len {
            let n = stream.read(&mut chunk).await.expect("read");
            if n == 0 {
                break;
            }
            rest.extend_from_slice(&chunk[..n]);
        }
        rest.truncate(len);
        return Resp {
            status,
            headers,
            body: rest,
        };
    }
    loop {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        rest.extend_from_slice(&chunk[..n]);
    }
    let chunked = header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
    let body = if chunked { dechunk(&rest) } else { rest };
    Resp {
        status,
        headers,
        body,
    }
}

fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let Some(line_end) = find(data, b"\r\n") else {
            break;
        };
        let size_text = String::from_utf8_lossy(&data[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("0").trim(), 16)
            .unwrap_or(0);
        data = &data[line_end + 2..];
        if size == 0 || data.len() < size {
            break;
        }
        out.extend_from_slice(&data[..size]);
        data = data.get(size + 2..).unwrap_or_default();
    }
    out
}

/// Percent-encodes everything but unreserved characters (RFC 3986).
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
