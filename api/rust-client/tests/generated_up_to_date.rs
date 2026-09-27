//! The committed generated code equals what the generator produces from the current contract
//! (the in-test twin of `api/generate.sh --check`).
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use strata_codegen::{Options, check, generate};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options(core_path: &str, source_label: &str) -> Options {
    Options {
        core_path: core_path.to_owned(),
        source_label: source_label.to_owned(),
        rustfmt_config: Some(root().join("rustfmt.toml")),
    }
}

#[test]
fn production_client_is_up_to_date() {
    let files = generate(
        &strata_api::openapi::document(),
        &options("crate", "api/openapi.json"),
    )
    .expect("generates");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/generated");
    assert_eq!(check(&dir, &files).expect("checks"), Vec::<String>::new());
}

#[test]
fn demo_client_is_up_to_date() {
    let files = generate(
        &strata_api::testing::demo::document(),
        &options("::strata_client", "the strata-api demo contract"),
    )
    .expect("generates");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/demo/generated");
    assert_eq!(check(&dir, &files).expect("checks"), Vec::<String>::new());
}
