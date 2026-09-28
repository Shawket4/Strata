//! What every note written by Strata carries (PLAN §6.4): its `id`, and `created`/`updated`
//! when the writer knows them.

use chrono::{DateTime, FixedOffset};
use ulid::Ulid;
use vault_format::{Document, KnownKey};

use crate::RenderError;

/// Frontmatter edits that make `doc` a note with `id`: sets `id` (when missing or different),
/// `created` (when given and missing) and `updated` (when given). Every other byte is kept.
/// Fails when the existing frontmatter cannot be edited.
pub fn stamp(
    doc: &mut Document,
    id: Ulid,
    created: Option<&DateTime<FixedOffset>>,
    updated: Option<&DateTime<FixedOffset>>,
) -> Result<(), RenderError> {
    let fm = doc.frontmatter_mut();
    if let Some(e) = fm.error() {
        return Err(RenderError::Unreadable(e.clone()));
    }
    if fm.id().ok().flatten() != Some(id) {
        fm.set_id(id).map_err(RenderError::property("id"))?;
    }
    if let Some(c) = created
        && fm.created().ok().flatten().is_none()
    {
        fm.set_created(c)
            .map_err(RenderError::property("created"))?;
    }
    if let Some(u) = updated {
        fm.set_updated(u)
            .map_err(RenderError::property("updated"))?;
    }
    Ok(())
}

/// `content` with its `id` property set to `id` (added when missing). Content that already
/// has this `id` is returned unchanged, byte for byte.
pub fn with_id(content: &str, id: Ulid) -> Result<String, RenderError> {
    let mut doc = Document::parse(content);
    if let Some(e) = doc.frontmatter().and_then(|f| f.error()) {
        return Err(RenderError::Unreadable(e.clone()));
    }
    if doc
        .frontmatter()
        .and_then(|f| f.id().ok().flatten())
        .is_some_and(|existing| existing == id)
    {
        return Ok(content.to_owned());
    }
    stamp(&mut doc, id, None, None)?;
    Ok(doc.render())
}

/// A list property as written on create (aliases, tags): each item trimmed, leading `#`
/// removed, empty items and repeats dropped, first occurrence order kept.
pub fn clean_list(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in items {
        let t = i.trim().trim_start_matches('#').trim().to_owned();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Sets a list-valued known key when the list is not empty.
pub(crate) fn set_list_if_any(
    doc: &mut Document,
    key: KnownKey,
    items: Vec<String>,
) -> Result<(), RenderError> {
    if items.is_empty() {
        return Ok(());
    }
    doc.frontmatter_mut()
        .set_list(key, items)
        .map_err(RenderError::property(key.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn id() -> Ulid {
        Ulid::from_string("01J8ZK3M4X7Q9W2E5R6T8Y0V1H").expect("ulid")
    }

    #[test]
    fn stamp_sets_missing_values_only() {
        let ts = DateTime::parse_from_rfc3339("2026-09-27T14:32:00+03:00").expect("ts");
        let mut doc = Document::parse("---\ncreated: 2020-01-01T00:00:00Z\ntags: [a]\n---\nx\n");
        stamp(&mut doc, id(), Some(&ts), Some(&ts)).expect("stamp");
        assert_eq!(
            doc.render(),
            "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ntags: [a]\ncreated: 2020-01-01T00:00:00Z\nupdated: 2026-09-27T14:32:00+03:00\n---\nx\n"
        );
    }

    #[test]
    fn stamp_refuses_unreadable_frontmatter() {
        let mut doc = Document::parse("---\na: [\n---\nx\n");
        assert!(matches!(
            stamp(&mut doc, id(), None, None),
            Err(RenderError::Unreadable(_))
        ));
    }

    #[test]
    fn with_id_adds_an_id_and_keeps_the_rest() {
        assert_eq!(
            with_id("# Hello\n", id()),
            Ok("---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n---\n# Hello\n".to_owned())
        );
        let has = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ntags:   [a]\n---\nx\n";
        assert_eq!(with_id(has, id()), Ok(has.to_owned()));
    }

    #[test]
    fn lists_are_cleaned() {
        let items: Vec<String> = [" #client ", "client", "", "  ", "أحمد", "#"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(clean_list(&items), vec!["client", "أحمد"]);
    }
}
