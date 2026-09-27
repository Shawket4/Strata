//! Short hover summaries (§10 node payload) from the AI summaries in the notes' sidecars
//! (`.meta/notes/<id>.json`, §6.5). The sidecar folder is listed once and only existing
//! sidecars of requested notes are read, on the blocking pool; files are replaced atomically
//! by the writer, so reads need no lock.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use strata_common::NoteId;
use vault_format::sidecar::NoteSidecar;

/// Longest hover summary, in characters (a longer one is cut and ends with `…`).
pub const SHORT_SUMMARY_CHARS: usize = 200;

/// `s` with runs of whitespace collapsed, cut to [`SHORT_SUMMARY_CHARS`].
pub fn short(s: &str) -> Option<String> {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= SHORT_SUMMARY_CHARS {
        return Some(collapsed);
    }
    let mut out: String = collapsed.chars().take(SHORT_SUMMARY_CHARS - 1).collect();
    out.truncate(out.trim_end().len());
    out.push('…');
    Some(out)
}

/// Short summaries of `ids` found in `vault_dir`'s sidecars.
pub async fn load(vault_dir: PathBuf, ids: BTreeSet<NoteId>) -> HashMap<NoteId, String> {
    tokio::task::spawn_blocking(move || {
        let dir = vault_dir.join(".meta").join("notes");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            let Ok(id) = stem.parse::<NoteId>() else {
                continue;
            };
            if !ids.contains(&id) || !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            if let Some(s) = NoteSidecar::from_json(&text)
                .ok()
                .and_then(|sc| sc.summary)
                .and_then(|s| short(&s))
            {
                out.insert(id, s);
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_are_collapsed_and_cut() {
        assert_eq!(short("  a\n\n b  "), Some("a b".into()));
        assert_eq!(short(" \n "), None);
        let long = "word ".repeat(100);
        let s = short(&long).expect("some");
        assert_eq!(s.chars().count(), 199);
        assert!(s.ends_with("word…"));
        let arabic = "ب".repeat(250);
        assert_eq!(short(&arabic).expect("some").chars().count(), 200);
    }
}
