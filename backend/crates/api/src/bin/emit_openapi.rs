//! Writes the OpenAPI contract as canonical pretty JSON.
//!
//! `emit-openapi <path>` writes the production contract (normally `api/openapi.json`).
//! `emit-openapi --demo <path>` (built with the `test-support` feature) writes the demo
//! contract used to generate the client test fixture.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (demo, path) = match args.as_slice() {
        [path] if !path.starts_with('-') => (false, path),
        [flag, path] if flag == "--demo" => (true, path),
        _ => {
            eprintln!("usage: emit-openapi [--demo] <path>");
            return ExitCode::from(2);
        }
    };
    let doc = if demo { demo_document() } else { Some(strata_api::openapi::document()) };
    let Some(doc) = doc else {
        eprintln!("--demo requires building with `--features test-support`");
        return ExitCode::from(2);
    };
    match std::fs::write(path, strata_api::openapi::to_pretty_json(&doc)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cannot write {path}: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "test-support")]
fn demo_document() -> Option<serde_json::Value> {
    Some(strata_api::testing::demo::document())
}

#[cfg(not(feature = "test-support"))]
fn demo_document() -> Option<serde_json::Value> {
    None
}
