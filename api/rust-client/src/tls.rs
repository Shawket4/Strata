//! TLS of the client: one rustls configuration for HTTPS (reqwest) and WSS (tokio-tungstenite).
//!
//! The apps trust the bundled Mozilla roots ([`webpki_roots`]), not the platform store: the
//! platform verifier needs per-platform set-up (JNI initialisation and a Kotlin component on
//! Android) and `rustls-native-certs` cannot read the Android store. User-installed CAs are
//! therefore not trusted. The crypto provider is ring (TLS 1.3 and 1.2).

use std::sync::Arc;

use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore};

use crate::Error;

/// ALPN of HTTPS requests (HTTP/2 when the server offers it).
const HTTP_ALPN: [&[u8]; 2] = [b"h2", b"http/1.1"];

/// ALPN of WebSocket handshakes (HTTP/1.1 upgrade only).
const WS_ALPN: [&[u8]; 1] = [b"http/1.1"];

/// Installs ring as the process-wide default rustls provider. Idempotent: a provider that is
/// already installed (ring or another) stays. Only code that builds its own `reqwest` client
/// or rustls configuration without an explicit provider needs this; [`crate::Client`] calls
/// it anyway.
pub fn ensure_crypto_provider() {
    // `Err` means a provider is already installed: nothing to do.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The HTTPS and WSS configurations of one client.
#[derive(Debug, Clone)]
pub(crate) struct TlsConfigs {
    /// For reqwest (moved into its builder).
    pub(crate) http: ClientConfig,
    /// For WebSocket handshakes.
    pub(crate) ws: Arc<ClientConfig>,
}

/// Builds both configurations: the bundled Mozilla roots plus `extra_roots`.
pub(crate) fn configs(extra_roots: &[CertificateDer<'static>]) -> Result<TlsConfigs, Error> {
    let mut roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    for root in extra_roots {
        roots
            .add(root.clone())
            .map_err(|e| Error::Tls(format!("extra root certificate: {e}")))?;
    }
    let roots = Arc::new(roots);
    Ok(TlsConfigs {
        http: config(&roots, &HTTP_ALPN)?,
        ws: Arc::new(config(&roots, &WS_ALPN)?),
    })
}

fn config(roots: &Arc<RootCertStore>, alpn: &[&[u8]]) -> Result<ClientConfig, Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|e| Error::Tls(e.to_string()))?
        .with_root_certificates(Arc::clone(roots))
        .with_no_client_auth();
    config.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_carry_the_bundled_roots_and_alpn() {
        let c = configs(&[]).expect("configs");
        assert_eq!(
            c.http.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
        assert_eq!(c.ws.alpn_protocols, vec![b"http/1.1".to_vec()]);
    }

    #[test]
    fn a_malformed_extra_root_is_refused() {
        let e = configs(&[CertificateDer::from(vec![1, 2, 3])]).expect_err("bad root");
        assert!(
            matches!(&e, Error::Tls(m) if m.starts_with("extra root certificate: ")),
            "{e:?}"
        );
    }
}
