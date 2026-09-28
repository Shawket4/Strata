//! The HTTP client core: every generated operation ends in [`Client::send`] or
//! [`Client::send_empty`].

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;

use crate::types::Problem;
use crate::{ApiError, Error, MSGPACK, PROBLEM_MSGPACK, Request, TokenProvider, ZIP};

/// A raw response as seen by a [`ResponseObserver`].
#[derive(Debug, Clone, Copy)]
pub struct ObservedResponse<'a> {
    /// Operation id of the request.
    pub operation_id: &'a str,
    /// HTTP status.
    pub status: u16,
    /// `Content-Type`, if any.
    pub content_type: Option<&'a str>,
    /// Raw body bytes.
    pub body: &'a [u8],
}

/// Sees every HTTP response before decoding (contract-conformance tests, diagnostics).
pub trait ResponseObserver: Send + Sync + fmt::Debug {
    /// Called once per HTTP response, including a `401` that triggers a token refresh.
    fn observe(&self, response: &ObservedResponse<'_>);
}

struct Inner {
    base_url: String,
    http: reqwest::Client,
    /// TLS of WebSocket handshakes (same roots as `http`, ALPN `http/1.1`).
    ws_tls: Arc<rustls::ClientConfig>,
    tokens: Option<Arc<dyn TokenProvider>>,
    observer: Option<Arc<dyn ResponseObserver>>,
}

/// Strata API client. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base_url)
            .field("tokens", &self.inner.tokens.is_some())
            .finish_non_exhaustive()
    }
}

/// Builds a [`Client`].
#[derive(Debug)]
pub struct ClientBuilder {
    base_url: String,
    tokens: Option<Arc<dyn TokenProvider>>,
    observer: Option<Arc<dyn ResponseObserver>>,
    timeout: Option<Duration>,
    connect_timeout: Duration,
    extra_roots: Vec<rustls::pki_types::CertificateDer<'static>>,
}

impl ClientBuilder {
    /// Supplies bearer tokens for authenticated operations and streams.
    #[must_use]
    pub fn tokens(mut self, tokens: Arc<dyn TokenProvider>) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// Observes every raw response.
    #[must_use]
    pub fn observer(mut self, observer: Arc<dyn ResponseObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Overall request timeout (default: none).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Also trusts `root` (DER), in addition to the bundled Mozilla roots. For tests against a
    /// local TLS server; the apps never call it.
    #[must_use]
    pub fn add_root_certificate(
        mut self,
        root: rustls::pki_types::CertificateDer<'static>,
    ) -> Self {
        self.extra_roots.push(root);
        self
    }

    /// Builds the client: rustls with ring, trusting the bundled Mozilla roots (plus any
    /// [`Self::add_root_certificate`]), never the platform store.
    pub fn build(self) -> Result<Client, Error> {
        let base_url = self.base_url.trim_end_matches('/').to_owned();
        let parsed =
            url::Url::parse(&base_url).map_err(|e| Error::Url(format!("{base_url}: {e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.query().is_some() {
            return Err(Error::Url(format!(
                "{base_url}: expected an http(s) URL without query"
            )));
        }
        crate::tls::ensure_crypto_provider();
        let tls = crate::tls::configs(&self.extra_roots)?;
        let mut http = reqwest::Client::builder()
            .tls_backend_preconfigured(tls.http)
            .user_agent(concat!("strata-client/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(self.connect_timeout);
        if let Some(timeout) = self.timeout {
            http = http.timeout(timeout);
        }
        Ok(Client {
            inner: Arc::new(Inner {
                base_url,
                http: http.build()?,
                ws_tls: tls.ws,
                tokens: self.tokens,
                observer: self.observer,
            }),
        })
    }
}

/// `Accept` of zip downloads: the archive, or MessagePack problem details.
const ZIP_ACCEPT: &str = "application/zip, application/vnd.msgpack;q=0.5";

/// A buffered HTTP response.
struct Raw {
    status: u16,
    content_type: Option<String>,
    body: bytes::Bytes,
}

impl Client {
    /// A client without tokens for `base_url` (e.g. `https://strata.example`).
    pub fn new(base_url: &str) -> Result<Self, Error> {
        Self::builder(base_url).build()
    }

    /// A builder for `base_url`.
    pub fn builder(base_url: &str) -> ClientBuilder {
        ClientBuilder {
            base_url: base_url.to_owned(),
            tokens: None,
            observer: None,
            timeout: None,
            connect_timeout: Duration::from_secs(10),
            extra_roots: Vec::new(),
        }
    }

    /// The base URL (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// TLS configuration of WebSocket handshakes.
    pub(crate) fn ws_tls(&self) -> Arc<rustls::ClientConfig> {
        Arc::clone(&self.inner.ws_tls)
    }

    pub(crate) fn tokens(&self) -> Option<&Arc<dyn TokenProvider>> {
        self.inner.tokens.as_ref()
    }

    /// Sends `request` and decodes a MessagePack success body into `R`.
    pub async fn send<R: DeserializeOwned>(&self, request: Request) -> Result<R, Error> {
        let raw = self.execute(&request).await?;
        if !is_media(raw.content_type.as_deref(), MSGPACK) {
            return Err(Error::UnexpectedResponse {
                operation: request.operation_id,
                status: raw.status,
                content_type: raw.content_type,
            });
        }
        decode_exact(&raw.body).map_err(|message| Error::Decode {
            operation: request.operation_id,
            message,
        })
    }

    /// Sends `request` whose success response is an `application/zip` download and returns
    /// its bytes.
    pub async fn send_zip(&self, request: Request) -> Result<bytes::Bytes, Error> {
        let request = request.accept(ZIP_ACCEPT);
        let raw = self.execute(&request).await?;
        if !is_media(raw.content_type.as_deref(), ZIP) {
            return Err(Error::UnexpectedResponse {
                operation: request.operation_id,
                status: raw.status,
                content_type: raw.content_type,
            });
        }
        Ok(raw.body)
    }

    /// Sends `request` whose success response has no body.
    pub async fn send_empty(&self, request: Request) -> Result<(), Error> {
        self.execute(&request).await.map(|_| ())
    }

    /// Sends, refreshing the token once on `401`; maps error statuses to [`Error::Api`].
    async fn execute(&self, request: &Request) -> Result<Raw, Error> {
        let provider = if request.auth {
            self.inner.tokens.as_ref()
        } else {
            None
        };
        let mut token = match provider {
            Some(p) => p.access_token().await?,
            None => None,
        };
        let mut refreshed = false;
        loop {
            let raw = self.once(request, token.as_deref()).await?;
            if let Some(observer) = &self.inner.observer {
                observer.observe(&ObservedResponse {
                    operation_id: request.operation_id,
                    status: raw.status,
                    content_type: raw.content_type.as_deref(),
                    body: &raw.body,
                });
            }
            if raw.status == 401
                && !refreshed
                && let Some(p) = provider
            {
                refreshed = true;
                if let Some(fresh) = p.refresh().await? {
                    token = Some(fresh);
                    continue;
                }
            }
            if (200..300).contains(&raw.status) {
                return Ok(raw);
            }
            return Err(problem_error(request.operation_id, &raw));
        }
    }

    async fn once(&self, request: &Request, token: Option<&str>) -> Result<Raw, Error> {
        let mut url = url::Url::parse(&format!("{}{}", self.inner.base_url, request.path))
            .map_err(|e| Error::Url(format!("{}: {e}", request.path)))?;
        if !request.query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in &request.query {
                pairs.append_pair(k, v);
            }
        }
        let mut builder = self
            .inner
            .http
            .request(request.method.clone(), url)
            .header(ACCEPT, request.accept.unwrap_or(MSGPACK));
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        if let Some(token) = token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(body) = &request.body {
            builder = builder
                .header(CONTENT_TYPE, request.content_type.unwrap_or(MSGPACK))
                .body(body.clone());
        }
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await?;
        Ok(Raw {
            status,
            content_type,
            body,
        })
    }
}

fn is_media(content_type: Option<&str>, expected: &str) -> bool {
    content_type.is_some_and(|c| {
        c.split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case(expected)
    })
}

fn problem_error(operation: &'static str, raw: &Raw) -> Error {
    if is_media(raw.content_type.as_deref(), PROBLEM_MSGPACK)
        && let Ok(problem) = decode_exact::<Problem>(&raw.body)
    {
        return ApiError::from_problem(problem).into();
    }
    Error::UnexpectedResponse {
        operation,
        status: raw.status,
        content_type: raw.content_type.clone(),
    }
}

/// Decodes exactly one MessagePack value (trailing bytes are an error).
pub(crate) fn decode_exact<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    let mut de = rmp_serde::Deserializer::new(std::io::Cursor::new(bytes));
    let value = serde::Deserialize::deserialize(&mut de).map_err(|e| e.to_string())?;
    let used = usize::try_from(de.position()).unwrap_or(usize::MAX);
    if used == bytes.len() {
        Ok(value)
    } else {
        Err(format!(
            "{} trailing bytes",
            bytes.len().saturating_sub(used)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decoding_rejects_trailing_bytes() {
        assert_eq!(decode_exact::<u8>(&[0x07]), Ok(7));
        assert_eq!(
            decode_exact::<u8>(&[0x07, 0x00]),
            Err("1 trailing bytes".to_owned())
        );
    }

    #[test]
    fn base_urls_are_validated() {
        assert!(matches!(Client::new("ftp://x"), Err(Error::Url(_))));
        assert!(matches!(Client::new("not a url"), Err(Error::Url(_))));
        assert_eq!(
            Client::new("https://strata.example/")
                .expect("valid")
                .base_url(),
            "https://strata.example"
        );
    }
}
