//! Captures (PLAN §6.9): saved first, in `inbox/YYYY-MM-DD-HHmmss.md` (see
//! [`crate::paths::capture_path`]).

use chrono::{DateTime, FixedOffset};
use ulid::Ulid;
use vault_format::Document;

use crate::RenderError;
use crate::note::stamp;

/// The markdown of a new capture: frontmatter `id` and `created`, then the captured text as
/// the whole body (even when it starts with `---`), ending with a line break.
pub fn capture_content(
    id: Ulid,
    created: &DateTime<FixedOffset>,
    text: &str,
) -> Result<String, RenderError> {
    let mut body = text.to_owned();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let mut doc = Document::parse("");
    doc.set_body(body);
    stamp(&mut doc, id, Some(created), None)?;
    Ok(doc.render())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn capture_is_id_created_and_text() {
        let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").expect("ts");
        let id = Ulid::from_string("01J8ZK3M4X7Q9W2E5R6T8Y0V1H").expect("ulid");
        assert_eq!(
            capture_content(id, &created, "كلمت أحمد النهارده"),
            Ok("---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ncreated: 2026-09-27T14:32:00+03:00\n---\nكلمت أحمد النهارده\n".to_owned())
        );
        assert_eq!(
            capture_content(id, &created, "---\nnot: frontmatter\n---\n"),
            Ok("---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ncreated: 2026-09-27T14:32:00+03:00\n---\n---\nnot: frontmatter\n---\n".to_owned())
        );
    }
}
