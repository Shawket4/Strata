//! TLS of `strata-client` against a local rustls server: the production configuration (ring,
//! bundled Mozilla roots) plus a test root makes one HTTPS request and one WSS handshake; the
//! same server without that root is refused as an unknown issuer, over HTTPS and WSS.
//!
//! Fixtures (`tests/fixtures/tls`, DER, valid until 2126): `ca.der` is a self-signed test CA,
//! `localhost.der` / `localhost.key.der` (PKCS#8) its leaf for `localhost` and `127.0.0.1`.
//! Regenerate with openssl (`req -x509` for the CA, `x509 -req` with a `subjectAltName` for
//! the leaf, `-outform DER`).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use futures_util::SinkExt;
use pretty_assertions::assert_eq;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::Serialize;
use strata_client::streaming::{StreamOptions, Subscription};
use strata_client::types::{Health, HealthStatus};
use strata_client::{Client, Error, TransportKind, operations};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::LazyConfigAcceptor;
use tokio_tungstenite::tungstenite::Message;

const CA: &[u8] = include_bytes!("fixtures/tls/ca.der");
const LEAF: &[u8] = include_bytes!("fixtures/tls/localhost.der");
const LEAF_KEY: &[u8] = include_bytes!("fixtures/tls/localhost.key.der");

/// What one accepted connection is expected to be.
#[derive(Debug, Clone, Copy)]
enum Conn {
    /// One HTTP/1.1 request answered with a MessagePack `Health`.
    Http,
    /// A WebSocket handshake followed by an `end` frame.
    Ws,
}

/// The ALPN protocols offered by each `ClientHello`, in order.
type Offered = Arc<std::sync::Mutex<Vec<Vec<String>>>>;

/// A TLS server on `127.0.0.1` serving `script` in order (one entry per completed
/// handshake); returns its port and the ALPN protocols every client hello offered.
async fn server(script: Vec<Conn>) -> (u16, Offered) {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(LEAF.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(LEAF_KEY.to_vec())),
        )
        .expect("server cert");
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let offered = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&offered);
    tokio::spawn(async move {
        let mut script = script.into_iter();
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let Ok(start) = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), tcp).await
            else {
                continue;
            };
            let alpn = start
                .client_hello()
                .alpn()
                .map(|protocols| {
                    protocols
                        .map(|p| String::from_utf8_lossy(p).into_owned())
                        .collect()
                })
                .unwrap_or_default();
            seen.lock().expect("lock").push(alpn);
            // A refused handshake (unknown issuer) ends here; the script entry stays.
            let Ok(mut tls) = start.into_stream(Arc::clone(&config)).await else {
                continue;
            };
            match script.next() {
                Some(Conn::Http) => {
                    let mut head = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                        let n = tls.read(&mut buf).await.expect("read");
                        assert!(n > 0, "request ended early");
                        head.extend_from_slice(&buf[..n]);
                    }
                    let request = String::from_utf8(head).expect("utf-8");
                    assert!(
                        request.starts_with("GET /health HTTP/1.1\r\n"),
                        "{request}"
                    );
                    let body = rmp_serde::to_vec_named(&Health {
                        status: HealthStatus::Ok,
                    })
                    .expect("encode");
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/vnd.msgpack\r\n\
                         content-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    );
                    tls.write_all(head.as_bytes()).await.expect("write");
                    tls.write_all(&body).await.expect("write");
                    tls.shutdown().await.expect("shutdown");
                }
                Some(Conn::Ws) => {
                    let mut ws = tokio_tungstenite::accept_async(tls).await.expect("ws");
                    ws.send(end_frame()).await.expect("send");
                    while let Some(Ok(_)) = futures_util::StreamExt::next(&mut ws).await {}
                }
                None => return,
            }
        }
    });
    (port, offered)
}

#[derive(Serialize)]
struct Envelope<'a> {
    v: u16,
    kind: &'a str,
    seq: u64,
    payload: (),
}

fn end_frame() -> Message {
    Message::Binary(
        rmp_serde::to_vec_named(&Envelope {
            v: 1,
            kind: "end",
            seq: 0,
            payload: (),
        })
        .expect("encode")
        .into(),
    )
}

fn alpn_offers() -> Vec<Vec<String>> {
    vec![
        vec!["h2".to_owned(), "http/1.1".to_owned()],
        vec!["http/1.1".to_owned()],
    ]
}

fn once() -> StreamOptions {
    StreamOptions {
        reconnect: false,
        idle_timeout: Duration::from_secs(30),
        ..StreamOptions::default()
    }
}

#[tokio::test]
async fn https_and_wss_work_with_the_production_config_plus_a_test_root() {
    let (port, offered) = server(vec![Conn::Http, Conn::Ws]).await;
    let client = Client::builder(&format!("https://localhost:{port}"))
        .add_root_certificate(CertificateDer::from(CA.to_vec()))
        .build()
        .expect("client");

    let health = operations::health(&client).await.expect("health over https");
    assert_eq!(health.status, HealthStatus::Ok);

    let mut sub =
        Subscription::<rmpv::Value>::new(&client, "/api/v1/events".into(), "events", false, once());
    assert!(sub.next().await.is_none(), "the end frame closes the stream");
    assert_eq!(sub.connections(), 1);

    // HTTPS offers h2 and http/1.1 (this server has no ALPN, so HTTP/1.1 is used); WSS offers
    // http/1.1 only.
    assert_eq!(*offered.lock().expect("lock"), alpn_offers());
}

#[tokio::test]
async fn an_unknown_issuer_is_refused_over_https_and_wss() {
    let (port, offered) = server(vec![Conn::Http, Conn::Ws]).await;
    let client = Client::new(&format!("https://localhost:{port}")).expect("client");

    let err = operations::health(&client).await.expect_err("unknown issuer");
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert_eq!(err.transport_kind(), Some(TransportKind::Tls));
    assert!(
        format!("{err:?}").contains("UnknownIssuer"),
        "the cause names the refusal: {err:?}"
    );

    let mut sub =
        Subscription::<rmpv::Value>::new(&client, "/api/v1/events".into(), "events", false, once());
    let err = sub.next().await.expect("an error").expect_err("unknown issuer");
    assert!(matches!(err, Error::WebSocket(_)), "{err:?}");
    assert_eq!(err.transport_kind(), Some(TransportKind::Tls));
    assert!(format!("{err:?}").contains("UnknownIssuer"), "{err:?}");

    assert_eq!(*offered.lock().expect("lock"), alpn_offers());
}

#[tokio::test]
async fn unreachable_servers_are_classified() {
    // Port 9 (discard) on localhost is closed in the test environment: connection refused.
    let client = Client::new("https://127.0.0.1:9").expect("client");
    let err = operations::health(&client).await.expect_err("refused");
    assert_eq!(err.transport_kind(), Some(TransportKind::Connect), "{err:?}");
    let mut sub =
        Subscription::<rmpv::Value>::new(&client, "/api/v1/events".into(), "events", false, once());
    let err = sub.next().await.expect("an error").expect_err("refused");
    assert_eq!(err.transport_kind(), Some(TransportKind::Connect), "{err:?}");

    // `.invalid` never resolves (RFC 6761); resolution fails without any network.
    let client = Client::new("https://strata.invalid").expect("client");
    let err = operations::health(&client).await.expect_err("no such host");
    assert_eq!(err.transport_kind(), Some(TransportKind::Dns), "{err:?}");

    // Not a transport failure.
    assert_eq!(Error::Url("x".into()).transport_kind(), None);
}
