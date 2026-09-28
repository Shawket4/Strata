//! Strata API client (§7.6, D15, D24).
//!
//! - [`generated`]: produced by `strata-codegen` from `api/openapi.json` (never edit by hand):
//!   [`types`] for every contract schema, [`operations`] with one `async fn` per endpoint and
//!   [`streams`] with one constructor per WebSocket stream.
//! - The hand-written core: [`Client`] (base URL, rustls HTTP client trusting the bundled
//!   Mozilla roots, bearer tokens from a [`TokenProvider`] with one refresh-and-retry on
//!   `401`), MessagePack encode/decode in
//!   [`Client::send`], problem details decoded into [`ApiError`], and [`streaming`] (WebSocket
//!   frames with reconnect and resume-from-seq).
//!
//! ```no_run
//! # async fn demo() -> Result<(), strata_client::Error> {
//! let client = strata_client::Client::new("https://strata.example")?;
//! let health = strata_client::operations::health(&client).await?;
//! # let _ = health; Ok(()) }
//! ```

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(test, allow(clippy::expect_used, clippy::float_cmp))]

mod auth;
mod client;
mod error;
mod request;
pub mod streaming;
mod tls;

pub mod generated;

pub use auth::{StaticToken, TokenProvider};
pub use client::{Client, ClientBuilder, ObservedResponse, ResponseObserver};
pub use error::{ApiError, Error, TransportKind};
pub use tls::ensure_crypto_provider;
pub use generated::{operations, streams, types};
pub use request::{Method, Request, encode_path_segment, param_string};

/// Media type of request and response bodies.
pub const MSGPACK: &str = "application/vnd.msgpack";

/// Media type of problem details.
pub const PROBLEM_MSGPACK: &str = "application/problem+msgpack";

/// Media type of binary downloads (vault export).
pub const ZIP: &str = "application/zip";
