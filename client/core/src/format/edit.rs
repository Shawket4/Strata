//! Content edits made by intents, all through `vault-format` (the same code the server runs,
//! so an op applied locally and on the server gives the same bytes).

use chrono::{DateTime, FixedOffset};
use ulid::Ulid;
use vault_format::Document;
use vault_format::tasks::TaskError;

use crate::error::{CoreError, CoreResult};

fn fm_err(e: &vault_format::FrontmatterError) -> CoreError {
    CoreError::invalid("frontmatter", &e.to_string())
}

/// Stable reason code of a task error.
pub fn task_error_code(e: &TaskError) -> &'static str {
    match e {
        TaskError::NotATask => "not_a_task",
        TaskError::NotOpen(_) => "not_open",
        TaskError::NotRecurring => "not_recurring",
        TaskError::RecurrenceNotUnderstood(_) => "recurrence_not_understood",
        TaskError::NoReferenceDate => "no_reference_date",
        TaskError::NoNextOccurrence => "no_next_occurrence",
        TaskError::InvalidBlockId(_) => "invalid_block_id",
    }
}

/// `content` with its `id` property set to `id` (added when missing; other bytes kept).
pub fn with_id(content: &str, id: Ulid) -> CoreResult<String> {
    let mut doc = Document::parse(content);
    if let Some(fm) = doc.frontmatter()
        && let Some(e) = fm.error()
    {
        return Err(fm_err(e));
    }
    if doc
        .frontmatter()
        .and_then(|f| f.id().ok().flatten())
        .is_some_and(|existing| existing == id)
    {
        return Ok(content.to_owned());
    }
    doc.frontmatter_mut().set_id(id).map_err(|e| fm_err(&e))?;
    Ok(doc.render())
}

/// The markdown of a new capture (§6.9): frontmatter `id` and `created`, then the text.
pub fn capture_content(id: Ulid, created: &DateTime<FixedOffset>, text: &str) -> CoreResult<String> {
    // The whole text is body, even if it starts with `---`; the new frontmatter uses the
    // text's line ending.
    let mut doc = Document::parse(if text.contains("\r\n") { "\r\n" } else { "" });
    let fm = doc.frontmatter_mut();
    fm.set_id(id).map_err(|e| fm_err(&e))?;
    fm.set_created(created).map_err(|e| fm_err(&e))?;
    doc.set_body(text);
    Ok(doc.render())
}

/// The markdown of a new entity note: `id`, `kind`, `aliases`.
pub fn entity_content(id: Ulid, kind: domain::NoteKind, aliases: &[String]) -> CoreResult<String> {
    let mut doc = Document::parse("");
    let fm = doc.frontmatter_mut();
    fm.set_id(id).map_err(|e| fm_err(&e))?;
    fm.set_kind(kind).map_err(|e| fm_err(&e))?;
    if !aliases.is_empty() {
        fm.set_list(vault_format::KnownKey::Aliases, aliases.to_vec())
            .map_err(|e| fm_err(&e))?;
    }
    Ok(doc.render())
}

/// Appends a task line at the end of a note (a new line is started if needed).
pub fn append_task(content: &str, line: &str) -> String {
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = content.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    out.push_str(line);
    out.push_str(eol);
    out
}

/// The inbox path of a capture created at `created` (`inbox/YYYY-MM-DD-HHmmss.md`, §6.9);
/// `taken` paths get ` 2`, ` 3`, … like `unique_name`.
pub fn capture_path<'a>(
    created: &DateTime<FixedOffset>,
    taken: impl IntoIterator<Item = &'a str>,
) -> String {
    let base = created.format("%Y-%m-%d-%H%M%S").to_string();
    let names: Vec<&str> = taken
        .into_iter()
        .filter_map(|p| p.strip_prefix("inbox/"))
        .filter_map(|p| p.strip_suffix(".md"))
        .collect();
    let name = vault_format::filename::unique_name(&base, names);
    format!("inbox/{name}.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn with_id_adds_an_id_and_keeps_the_rest() {
        let id = Ulid::from_string("01J8ZK3M4X7Q9W2E5R6T8Y0V1H").expect("ulid");
        assert_eq!(
            with_id("# Hello\n", id).expect("ok"),
            "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n---\n# Hello\n"
        );
        let has = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ntags: [a]\n---\nx\n";
        assert_eq!(with_id(has, id).expect("ok"), has);
    }

    #[test]
    fn capture_content_has_id_and_created() {
        let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").expect("ts");
        assert_eq!(
            capture_content(Ulid::from_string("01J8ZK3M4X7Q9W2E5R6T8Y0V1H").expect("ulid"), &created, "كلمت أحمد النهارده").expect("ok"),
            "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ncreated: 2026-09-27T14:32:00+03:00\n---\nكلمت أحمد النهارده"
        );
    }

    #[test]
    fn entity_content_is_canonical() {
        assert_eq!(
            entity_content(
                Ulid::from_string("01J8ZKACME00000000000000AB").expect("ulid"),
                domain::NoteKind::Person,
                &["أحمد سمير".to_owned(), "Ahmed S.".to_owned()]
            )
            .expect("ok"),
            "---\nid: 01J8ZKACME00000000000000AB\nkind: person\naliases: [أحمد سمير, Ahmed S.]\n---\n"
        );
    }

    #[test]
    fn appending_task_lines() {
        assert_eq!(append_task("x", "- [ ] y ^t-1"), "x\n- [ ] y ^t-1\n");
        assert_eq!(append_task("x\r\n", "- [ ] y ^t-1"), "x\r\n- [ ] y ^t-1\r\n");
        assert_eq!(append_task("", "- [ ] y ^t-1"), "- [ ] y ^t-1\n");
    }

    #[test]
    fn capture_paths_are_unique() {
        let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:05+03:00").expect("ts");
        assert_eq!(capture_path(&created, []), "inbox/2026-09-27-143205.md");
        assert_eq!(
            capture_path(&created, ["inbox/2026-09-27-143205.md"]),
            "inbox/2026-09-27-143205 2.md"
        );
    }
}
