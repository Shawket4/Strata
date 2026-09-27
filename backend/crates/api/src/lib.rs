//! Strata HTTP API (`/api/v1`): the Actix application, the MessagePack wire layer
//! (PLAN §7.7, L21), WebSocket streaming (D24) and the utoipa OpenAPI contract (L13, §7.6).
//!
//! Endpoint authors use:
//! - [`wire::MsgPack`] as request extractor and response type, [`wire::Problem`] for errors;
//! - [`app::api_v1`] to mount routes under `/api/v1` with the wire conventions applied;
//! - `#[utoipa::path]` on every handler, listed in [`openapi::ApiDoc`];
//! - [`wire::ws`] for streaming endpoints.
//!
//! With the `test-support` feature, [`contract`] validates responses against the contract and
//! [`testing`] provides an in-process server and a demo router exercising the machinery.

// Tests assert exact values and may `expect` with a message stating the invariant.
#![cfg_attr(test, allow(clippy::expect_used, clippy::float_cmp))]

pub mod ai;
pub mod app;
pub mod auth;
pub mod events;
pub mod health;
pub mod openapi;
pub mod routes;
pub mod sync;
pub mod vault;
pub mod wire;

#[cfg(feature = "test-support")]
pub mod contract;
#[cfg(feature = "test-support")]
pub mod testing;
