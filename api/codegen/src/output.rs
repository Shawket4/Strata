//! Writing generated files, and `--check` comparison against committed ones.

use std::path::Path;

use crate::Error;

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// File name inside the output directory.
    pub name: &'static str,
    /// Full contents.
    pub contents: String,
}

impl GeneratedFile {
    pub(crate) fn new(name: &'static str, contents: String) -> Self {
        Self { name, contents }
    }
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Writes `files` into `dir` (created if needed) and removes stale `.rs` files.
pub fn write(dir: &Path, files: &[GeneratedFile]) -> Result<(), Error> {
    std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    for stale in stale_files(dir, files)? {
        let path = dir.join(stale);
        std::fs::remove_file(&path).map_err(|e| io(&path, e))?;
    }
    for file in files {
        let path = dir.join(file.name);
        std::fs::write(&path, &file.contents).map_err(|e| io(&path, e))?;
    }
    Ok(())
}

/// Names of files in `dir` that differ from `files`, are missing, or should not exist.
/// Empty when the committed output is up to date.
pub fn check(dir: &Path, files: &[GeneratedFile]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    for file in files {
        let path = dir.join(file.name);
        match std::fs::read_to_string(&path) {
            Ok(existing) if existing == file.contents => {}
            Ok(_) => out.push(format!("{} differs", file.name)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                out.push(format!("{} is missing", file.name));
            }
            Err(e) => return Err(io(&path, e)),
        }
    }
    for stale in stale_files(dir, files)? {
        out.push(format!("{stale} should not exist"));
    }
    out.sort();
    Ok(out)
}

fn stale_files(dir: &Path, files: &[GeneratedFile]) -> Result<Vec<String>, Error> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(dir, e)),
    };
    let mut stale = Vec::new();
    for entry in entries {
        let name = entry.map_err(|e| io(dir, e))?.file_name();
        let name = name.to_string_lossy().into_owned();
        let is_rust = Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"));
        if is_rust && !files.iter().any(|f| f.name == name) {
            stale.push(name);
        }
    }
    stale.sort();
    Ok(stale)
}
