//! Content edits made by intents, all through `vault-format` (the same code the server runs,
//! so an op applied locally and on the server gives the same bytes).

use chrono::{DateTime, FixedOffset, NaiveDate};
use ulid::Ulid;
use vault_format::tasks::{self, TaskError, TaskLine};
use vault_format::{Document, LineEnding, RelationKey};

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

fn task_err(e: &TaskError) -> CoreError {
    CoreError::TaskChange {
        reason: task_error_code(e).to_owned(),
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

fn relation_key(rel_type: &str) -> CoreResult<RelationKey> {
    rel_type
        .parse::<RelationKey>()
        .map_err(|_| CoreError::invalid("rel_type", "unknown_relation"))
}

fn edit_frontmatter(
    content: &str,
    f: impl FnOnce(&mut vault_format::Frontmatter) -> Result<(), vault_format::FrontmatterError>,
) -> CoreResult<String> {
    let mut doc = Document::parse(content);
    if let Some(e) = doc.frontmatter().and_then(|f| f.error()) {
        return Err(fm_err(e));
    }
    f(doc.frontmatter_mut()).map_err(|e| fm_err(&e))?;
    Ok(doc.render())
}

/// Adds `[[dst_link]]` to the relation `rel_type`.
pub fn add_relation(content: &str, rel_type: &str, dst_link: &str) -> CoreResult<String> {
    let rk = relation_key(rel_type)?;
    edit_frontmatter(content, |fm| fm.add_relation_link(rk, dst_link).map(|_| ()))
}

/// Removes every link to `dst_link` from the relation `rel_type`.
pub fn remove_relation(content: &str, rel_type: &str, dst_link: &str) -> CoreResult<String> {
    let rk = relation_key(rel_type)?;
    edit_frontmatter(content, |fm| {
        fm.remove_relation_link(rk, dst_link).map(|_| ())
    })
}

/// Moves `dst_link` from `rel_type` to `new_type`.
pub fn retype_relation(
    content: &str,
    rel_type: &str,
    new_type: &str,
    dst_link: &str,
) -> CoreResult<String> {
    let old = relation_key(rel_type)?;
    let new = relation_key(new_type)?;
    edit_frontmatter(content, |fm| {
        fm.remove_relation_link(old, dst_link)?;
        fm.add_relation_link(new, dst_link).map(|_| ())
    })
}

/// Line spans of `text` (content range, with the line terminator range following it).
fn lines_with_eol(text: &str) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    while start < text.len() {
        let nl = text[start..].find('\n').map(|i| start + i);
        let (content_end, next) = match nl {
            Some(i) if i > start && bytes[i - 1] == b'\r' => (i - 1, i + 1),
            Some(i) => (i, i + 1),
            None => (text.len(), text.len()),
        };
        out.push((start, content_end, next));
        start = next;
    }
    out
}

/// Finds the line of task `task_id` (block ID, or `line:<note>:<n>` synthetic ID) and
/// replaces it with the lines `f` returns (empty = delete the line).
pub fn replace_task_line(
    content: &str,
    task_id: &str,
    f: impl FnOnce(&str) -> CoreResult<Vec<String>>,
) -> CoreResult<String> {
    let doc = Document::parse(content);
    let eol = match doc.line_ending() {
        LineEnding::CrLf => "\r\n",
        LineEnding::Lf => "\n",
    };
    let offset = doc.body_offset();
    let synthetic_line = task_id
        .strip_prefix("line:")
        .and_then(|rest| rest.rsplit(':').next())
        .and_then(|n| n.parse::<usize>().ok());
    let body_tasks = tasks::extract_tasks(doc.body());
    let body_line0 = content[..offset.min(content.len())].matches('\n').count();
    let target = body_tasks.iter().find(|t| match synthetic_line {
        Some(n) => body_line0 + t.line_number == n,
        None => t.task.block_id() == Some(task_id),
    });
    let Some(target) = target else {
        return Err(CoreError::not_found("task"));
    };
    let start = offset + target.line_span.start;
    let end = offset + target.line_span.end;
    let new_lines = f(&content[start..end])?;
    let mut out = String::with_capacity(content.len() + 64);
    out.push_str(&content[..start]);
    if new_lines.is_empty() {
        // Remove the line together with its terminator.
        let next = lines_with_eol(&content[start..])
            .first()
            .map_or(end, |(_, _, next)| start + next);
        out.push_str(&content[next..]);
    } else {
        out.push_str(&new_lines.join(eol));
        out.push_str(&content[end..]);
    }
    Ok(out)
}

/// Completes a task; a recurring task also gets its next occurrence above it (Tasks plugin
/// semantics, §6.11).
pub fn complete_task(
    content: &str,
    task_id: &str,
    done_date: NaiveDate,
    next_task_id: Option<&str>,
) -> CoreResult<String> {
    replace_task_line(content, task_id, |line| {
        let recurring = TaskLine::parse(line).is_some_and(|t| t.recurrence_text().is_some());
        match (recurring, next_task_id) {
            (true, Some(next)) => tasks::complete_recurring(line, done_date, next)
                .map(|[new, done]| vec![new, done])
                .map_err(|e| task_err(&e)),
            _ => tasks::complete(line, done_date)
                .map(|l| vec![l])
                .map_err(|e| task_err(&e)),
        }
    })
}

/// Cancels a task.
pub fn cancel_task(content: &str, task_id: &str, date: NaiveDate) -> CoreResult<String> {
    replace_task_line(content, task_id, |line| {
        tasks::cancel(line, date)
            .map(|l| vec![l])
            .map_err(|e| task_err(&e))
    })
}

/// Reopens a task.
pub fn reopen_task(content: &str, task_id: &str) -> CoreResult<String> {
    replace_task_line(content, task_id, |line| {
        tasks::reopen(line)
            .map(|l| vec![l])
            .map_err(|e| task_err(&e))
    })
}

/// Replaces a task line (the new line must be a task).
pub fn update_task(content: &str, task_id: &str, new_line: &str) -> CoreResult<String> {
    if TaskLine::parse(new_line).is_none() {
        return Err(CoreError::invalid("line", "not_a_task"));
    }
    replace_task_line(content, task_id, |_| Ok(vec![new_line.to_owned()]))
}

/// Deletes a task line.
pub fn delete_task(content: &str, task_id: &str) -> CoreResult<String> {
    replace_task_line(content, task_id, |_| Ok(Vec::new()))
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

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
    }

    #[test]
    fn with_id_adds_an_id_and_keeps_the_rest() {
        let id = Ulid::from_parts(1, 7);
        assert_eq!(
            with_id("# Hello\n", id).expect("ok"),
            "---\nid: 00000000010000000000000007\n---\n# Hello\n"
        );
        let has = "---\nid: 00000000010000000000000007\ntags: [a]\n---\nx\n";
        assert_eq!(with_id(has, id).expect("ok"), has);
    }

    #[test]
    fn capture_content_has_id_and_created() {
        let created = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").expect("ts");
        assert_eq!(
            capture_content(Ulid::from_parts(1, 1), &created, "كلمت أحمد النهارده").expect("ok"),
            "---\nid: 00000000010000000000000001\ncreated: 2026-09-27T14:32:00+03:00\n---\nكلمت أحمد النهارده"
        );
    }

    #[test]
    fn entity_content_is_canonical() {
        assert_eq!(
            entity_content(
                Ulid::from_parts(1, 2),
                domain::NoteKind::Person,
                &["أحمد سمير".to_owned(), "Ahmed S.".to_owned()]
            )
            .expect("ok"),
            "---\nid: 00000000010000000000000002\nkind: person\naliases: [أحمد سمير, Ahmed S.]\n---\n"
        );
    }

    #[test]
    fn relations_are_edited_through_frontmatter() {
        let c = "---\nid: X\n---\nbody\n";
        let added = add_relation(c, "related", "Churn notes").expect("ok");
        assert_eq!(added, "---\nid: X\nrelated: [\"[[Churn notes]]\"]\n---\nbody\n");
        let retyped = retype_relation(&added, "related", "supports", "Churn notes").expect("ok");
        assert_eq!(retyped, "---\nid: X\nsupports: [\"[[Churn notes]]\"]\n---\nbody\n");
        let removed = remove_relation(&retyped, "supports", "Churn notes").expect("ok");
        assert_eq!(removed, "---\nid: X\n---\nbody\n");
        assert_eq!(
            add_relation(c, "likes", "X"),
            Err(CoreError::invalid("rel_type", "unknown_relation"))
        );
    }

    const TASKS: &str = "# Tasks\n- [ ] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-09-27 (@2026-09-27 10:00) ^t-a1\n- [ ] One-off 📅 2026-09-29 ^t-b2\n";

    #[test]
    fn completing_a_recurring_task_writes_two_lines() {
        let out = complete_task(TASKS, "t-a1", d("2026-09-27"), Some("t-c3")).expect("ok");
        assert_eq!(
            out,
            "# Tasks\n- [ ] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-10-04 (@2026-10-04 10:00) ^t-c3\n- [x] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-09-27 ✅ 2026-09-27 (@2026-09-27 10:00) ^t-a1\n- [ ] One-off 📅 2026-09-29 ^t-b2\n"
        );
    }

    #[test]
    fn task_line_edits() {
        let done = complete_task(TASKS, "t-b2", d("2026-09-28"), None).expect("ok");
        assert!(done.ends_with("- [x] One-off 📅 2026-09-29 ✅ 2026-09-28 ^t-b2\n"));
        assert_eq!(
            complete_task(&done, "t-b2", d("2026-09-28"), None),
            Err(CoreError::TaskChange {
                reason: "not_open".into()
            })
        );
        let reopened = reopen_task(&done, "t-b2").expect("ok");
        assert_eq!(reopened, TASKS);
        let cancelled = cancel_task(TASKS, "t-b2", d("2026-09-28")).expect("ok");
        assert!(cancelled.ends_with("- [-] One-off 📅 2026-09-29 ❌ 2026-09-28 ^t-b2\n"));
        assert_eq!(
            delete_task(TASKS, "t-b2").expect("ok"),
            "# Tasks\n- [ ] Petrol Arrows invoice 🔁 every week on Sunday 📅 2026-09-27 (@2026-09-27 10:00) ^t-a1\n"
        );
        assert_eq!(
            delete_task(TASKS, "t-zz"),
            Err(CoreError::not_found("task"))
        );
        assert_eq!(
            update_task(TASKS, "t-b2", "not a task"),
            Err(CoreError::invalid("line", "not_a_task"))
        );
        assert_eq!(append_task("x", "- [ ] y ^t-1"), "x\n- [ ] y ^t-1\n");
        assert_eq!(append_task("", "- [ ] y ^t-1"), "- [ ] y ^t-1\n");
    }

    #[test]
    fn synthetic_task_ids_address_lines() {
        let c = "---\nid: N\n---\n- [ ] no id\n";
        let out = complete_task(c, "line:N:3", d("2026-09-27"), None).expect("ok");
        assert_eq!(out, "---\nid: N\n---\n- [x] no id ✅ 2026-09-27\n");
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
