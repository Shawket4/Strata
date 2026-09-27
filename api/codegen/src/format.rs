//! Formatting: `prettyplease` for a readable layout, then `rustfmt` so the committed files are
//! exactly what `cargo fmt` would produce (otherwise `cargo fmt` and `--check` would fight).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use proc_macro2::TokenStream;

use crate::Error;

pub(crate) fn format(tokens: TokenStream, rustfmt_config: Option<&Path>) -> Result<String, Error> {
    let file: syn::File = syn::parse2(tokens).map_err(|e| Error::Syntax(e.to_string()))?;
    rustfmt(&prettyplease::unparse(&file), rustfmt_config)
}

fn rustfmt(source: &str, config: Option<&Path>) -> Result<String, Error> {
    let program = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let mut cmd = Command::new(program);
    cmd.args(["--edition", "2024", "--emit", "stdout", "--quiet"]);
    if let Some(config) = config {
        cmd.arg("--config-path").arg(config);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Rustfmt(format!("cannot run rustfmt: {e}")))?;
    child
        .stdin
        .take()
        .ok_or_else(|| Error::Rustfmt("no stdin".to_owned()))?
        .write_all(source.as_bytes())
        .map_err(|e| Error::Rustfmt(e.to_string()))?;
    let output = child
        .wait_with_output()
        .map_err(|e| Error::Rustfmt(e.to_string()))?;
    if !output.status.success() {
        return Err(Error::Rustfmt(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| Error::Rustfmt(e.to_string()))
}
