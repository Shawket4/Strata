//! `strata-codegen`: generates the Rust client module from the OpenAPI contract.
//!
//! ```text
//! strata-codegen --input api/openapi.json --output api/rust-client/src/generated \
//!     [--core-path crate] [--source-label api/openapi.json] [--check]
//! ```
//!
//! `--check` writes nothing and exits 1 when the files in `--output` differ from what would be
//! generated (CI).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use strata_codegen::{Options, check, generate, write};

struct Args {
    input: PathBuf,
    output: PathBuf,
    options: Options,
    check: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut input = None;
    let mut output = None;
    let mut options = Options::default();
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "--output" => output = Some(PathBuf::from(value("--output")?)),
            "--core-path" => options.core_path = value("--core-path")?,
            "--source-label" => options.source_label = value("--source-label")?,
            "--check" => check = true,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    let output = output.ok_or("--output is required")?;
    options.rustfmt_config = find_rustfmt_config(&output);
    Ok(Args {
        input: input.ok_or("--input is required")?,
        output,
        options,
        check,
    })
}

/// The `rustfmt.toml` that `cargo fmt` would use for files in `dir`.
fn find_rustfmt_config(dir: &Path) -> Option<PathBuf> {
    let start = std::path::absolute(dir).ok()?;
    start.ancestors().find_map(|d| {
        ["rustfmt.toml", ".rustfmt.toml"]
            .iter()
            .map(|n| d.join(n))
            .find(|p| p.is_file())
    })
}

fn run(args: &Args) -> Result<bool, String> {
    let text = std::fs::read_to_string(&args.input)
        .map_err(|e| format!("{}: {e}", args.input.display()))?;
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", args.input.display()))?;
    let files = generate(&doc, &args.options).map_err(|e| e.to_string())?;
    if args.check {
        let problems = check(&args.output, &files).map_err(|e| e.to_string())?;
        for p in &problems {
            eprintln!("{}: {p}", args.output.display());
        }
        if !problems.is_empty() {
            eprintln!("generated client is stale: run api/generate.sh");
        }
        Ok(problems.is_empty())
    } else {
        write(&args.output, &files).map_err(|e| e.to_string())?;
        Ok(true)
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("strata-codegen: {e}");
            eprintln!(
                "usage: strata-codegen --input <openapi.json> --output <dir> [--core-path <path>] [--source-label <text>] [--check]"
            );
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("strata-codegen: {e}");
            ExitCode::FAILURE
        }
    }
}
