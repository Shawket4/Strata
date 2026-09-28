//! Sign-in through the facade when the server cannot be reached: a certificate the bundled
//! roots do not trust (the Android failure mode of a platform-store verifier) and a refused
//! connection each come back as the `offline` failure with its reason, never as a panic or an
//! untyped error.
//!
//! The core is a process-wide singleton, so this binary holds one test. The TLS server uses
//! strata-client's test certificate (`api/rust-client/tests/fixtures/tls`), issued by a test
//! CA that is not a Mozilla root.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use pretty_assertions::assert_eq;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use strata_core::api::app;
use strata_core::view::model::{
    CoreConfig, CoreFailure, DeviceFacts, Platform, SessionKind, SignInRequest,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

const LEAF: &[u8] = include_bytes!("../../../api/rust-client/tests/fixtures/tls/localhost.der");
const LEAF_KEY: &[u8] =
    include_bytes!("../../../api/rust-client/tests/fixtures/tls/localhost.key.der");

fn offline(reason: &str) -> CoreFailure {
    CoreFailure {
        code: "offline".to_owned(),
        message_key: "error.offline".to_owned(),
        field: None,
        reason: Some(reason.to_owned()),
        count: None,
        status: None,
    }
}

fn sign_in_request() -> SignInRequest {
    SignInRequest {
        username: "alice".to_owned(),
        password: "correct horse battery staple".to_owned(),
        device_name: "Phone".to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_to_an_unreachable_server_is_an_offline_failure_with_its_reason() {
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
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = tokio::spawn(async move {
        // Serve TLS handshakes (the client refuses each) until the test aborts this task.
        loop {
            let (tcp, _) = listener.accept().await.expect("accept");
            let refused = acceptor.accept(tcp).await;
            assert!(refused.is_err(), "the client must refuse the certificate");
        }
    });

    let dir = tempfile::TempDir::new().expect("app data");
    let state = app::init_core(CoreConfig {
        app_data_dir: dir.path().to_str().expect("utf-8").to_owned(),
        platform: Platform::Android,
        device: DeviceFacts {
            device_name: "Phone".to_owned(),
            manufacturer: String::new(),
            model: String::new(),
            model_name: String::new(),
            host_name: String::new(),
        },
        server_url: format!("https://localhost:{port}"),
        release_build: true,
    })
    .await
    .expect("init");
    assert_eq!(state.kind, SessionKind::SignedOut);

    // The test CA is not among the bundled roots: unknown issuer.
    assert_eq!(app::sign_in(sign_in_request()).await, Err(offline("tls")));

    // Nothing listens on the port any more: connection refused.
    server.abort();
    let _ = server.await;
    assert_eq!(
        app::sign_in(sign_in_request()).await,
        Err(offline("connect"))
    );
}
