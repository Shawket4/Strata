//! The vault zip of `GET /me/export` (D25), streamed while it is written.
//!
//! A blocking task walks the caller's vault directory in sorted order and writes a zip
//! through a non-seeking writer (entries use data descriptors), whose output is forwarded in
//! chunks over a bounded channel to the HTTP response. Memory stays bounded whatever the vault
//! size, and a client that disconnects stops the walk. `.git/` at the vault root is left out
//! (git history is server-internal, PLAN §6.10); symlinks and special files are skipped, so
//! nothing outside the vault can be reached.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use zip::write::SimpleFileOptions;

/// Chunk size forwarded to the response.
const CHUNK: usize = 64 * 1024;

struct ChannelWriter {
    tx: mpsc::Sender<io::Result<Bytes>>,
    buf: Vec<u8>,
}

impl ChannelWriter {
    fn send(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = Bytes::from(std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK)));
        self.tx
            .blocking_send(Ok(chunk))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "export client went away"))
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK {
            self.send()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send()
    }
}

/// Regular files under `root`, as sorted `/`-separated relative paths, excluding the root's
/// `.git/`. Symlinks and special files are skipped.
pub fn vault_files(root: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![(String::new(), root.to_path_buf())];
    while let Some((prefix, dir)) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let kind = entry.file_type()?;
            let relative = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            if kind.is_dir() {
                if !(prefix.is_empty() && name == ".git") {
                    stack.push((relative, entry.path()));
                }
            } else if kind.is_file() {
                out.push((relative, entry.path()));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn write_zip(root: &Path, writer: ChannelWriter) -> io::Result<()> {
    let mut zip = zip::ZipWriter::new_stream(writer);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600)
        .large_file(true);
    for (name, path) in vault_files(root)? {
        zip.start_file(name, options).map_err(io::Error::other)?;
        let mut file = std::fs::File::open(&path)?;
        io::copy(&mut file, &mut zip)?;
    }
    let mut writer = zip.finish().map_err(io::Error::other)?.into_inner();
    writer.flush()
}

/// Starts writing the zip of `vault_dir`. Returns the chunk receiver for the response body
/// and the writer task, which resolves once the whole archive was handed over.
pub fn stream_vault_zip(
    vault_dir: PathBuf,
) -> (mpsc::Receiver<io::Result<Bytes>>, JoinHandle<io::Result<()>>) {
    let (tx, rx) = mpsc::channel(4);
    let task = tokio::task::spawn_blocking(move || {
        let writer = ChannelWriter {
            tx: tx.clone(),
            buf: Vec::with_capacity(CHUNK),
        };
        let result = write_zip(&vault_dir, writer);
        if let Err(err) = &result {
            // Surface the failure to the response stream (the client sees a broken body).
            let _ = tx.blocking_send(Err(io::Error::new(err.kind(), err.to_string())));
        }
        result
    });
    (rx, task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn collect(mut rx: mpsc::Receiver<io::Result<Bytes>>) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = rx.blocking_recv() {
            out.extend_from_slice(&chunk.expect("chunk"));
        }
        out
    }

    #[test]
    fn zips_exactly_the_vault_files_without_git() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(vault.join("notes/sub")).expect("mkdir");
        std::fs::create_dir_all(vault.join(".git/objects")).expect("mkdir");
        std::fs::create_dir_all(vault.join(".meta")).expect("mkdir");
        std::fs::write(vault.join("notes/a.md"), "# A\n").expect("write");
        std::fs::write(vault.join("notes/sub/ب.md"), "عربي").expect("write");
        std::fs::write(vault.join(".meta/x.json"), "{}").expect("write");
        std::fs::write(vault.join(".git/HEAD"), "ref").expect("write");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", vault.join("notes/link.md")).expect("link");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let (rx, task) = rt.block_on(async { stream_vault_zip(vault.clone()) });
        let bytes = collect(rx);
        rt.block_on(task).expect("join").expect("zip written");

        let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes)).expect("valid zip");
        let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, vec![".meta/x.json", "notes/a.md", "notes/sub/ب.md"]);
        let mut content = String::new();
        archive
            .by_name("notes/sub/ب.md")
            .expect("entry")
            .read_to_string(&mut content)
            .expect("read");
        assert_eq!(content, "عربي");
    }

    #[test]
    fn a_missing_vault_is_an_empty_archive() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(vault_files(&tmp.path().join("nope")).expect("ok"), vec![]);
    }
}
