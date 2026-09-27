//! Shared helpers for strata-ai integration tests.
#![allow(dead_code, clippy::expect_used)]

use std::path::PathBuf;

use strata_ai::AiCaller;
use strata_common::UserId;

/// Deterministic user IDs.
pub fn user_id(n: u8) -> UserId {
    format!("01M3HBS0G000000000000000{n:02}")
        .parse()
        .expect("valid ULID")
}

/// A caller with a DB-less scope (tests that use the in-memory usage store).
pub fn caller(name: &str, n: u8) -> AiCaller {
    AiCaller {
        scope: strata_testkit::detached_scope(user_id(n)).expect("detached scope"),
        username: name.to_owned(),
    }
}

/// `tests/fixtures/<rel>`.
pub fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}
