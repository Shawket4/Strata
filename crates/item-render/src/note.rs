//! What every note written by Strata carries (PLAN §6.4): its `id`, and `created`/`updated`
//! when the writer knows them.
//!
//! **Times are UTC.** Every time Strata writes is UTC in whole seconds (`2026-09-27T11:32:00Z`);
//! the functions here take `DateTime<Utc>`, so a local offset cannot reach a file. A new note's
//! `created` is the device's creation time carried by the create op (never the time the
//! server received it), and `updated` starts equal to it.

use chrono::{DateTime, FixedOffset, SubsecRound, Utc};
use ulid::Ulid;
use vault_format::{Document, KnownKey};

use crate::RenderError;

/// A time as Strata writes it into a note: UTC, whole seconds.
pub fn written_time(t: &DateTime<Utc>) -> DateTime<FixedOffset> {
    t.trunc_subsecs(0).fixed_offset()
}

/// Frontmatter edits that make `doc` a note with `id`: sets `id` (when missing or different),
/// `created` (when given and missing) and `updated` (when given), both as [`written_time`].
/// Every other byte is kept. Fails when the existing frontmatter cannot be edited.
pub fn stamp(
    doc: &mut Document,
    id: Ulid,
    created: Option<&DateTime<Utc>>,
    updated: Option<&DateTime<Utc>>,
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
        fm.set_created(&written_time(c))
            .map_err(RenderError::property("created"))?;
    }
    if let Some(u) = updated {
        fm.set_updated(&written_time(u))
            .map_err(RenderError::property("updated"))?;
    }
    Ok(())
}

/// A new note made by `note.create` / `POST /notes` from the user's `content`: `id` set,
/// `created` set to the device's creation time `created` unless the content has one, and
/// `updated` set to `created`. Both the server and the device write these bytes.
pub fn new_note(content: &str, id: Ulid, created: &DateTime<Utc>) -> Result<String, RenderError> {
    let mut doc = Document::parse(content);
    stamp(&mut doc, id, Some(created), Some(created))?;
    Ok(doc.render())
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

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).expect("ts").to_utc()
    }

    #[test]
    fn stamp_sets_missing_values_only() {
        let t = ts("2026-09-27T14:32:00.75+03:00");
        let mut doc = Document::parse("---\ncreated: 2020-01-01T00:00:00Z\ntags: [a]\n---\nx\n");
        stamp(&mut doc, id(), Some(&t), Some(&t)).expect("stamp");
        assert_eq!(
            doc.render(),
            "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ntags: [a]\ncreated: 2020-01-01T00:00:00Z\nupdated: 2026-09-27T11:32:00Z\n---\nx\n"
        );
    }

    #[test]
    fn new_notes_carry_the_device_creation_time_in_utc() {
        let t = ts("2026-09-28T08:15:30+03:00");
        assert_eq!(
            new_note("---\ntags: [pricing]\n---\n# Pricing\n", id(), &t),
            Ok("---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ntags: [pricing]\ncreated: 2026-09-28T05:15:30Z\nupdated: 2026-09-28T05:15:30Z\n---\n# Pricing\n".to_owned())
        );
        // A `created` the user wrote is kept; `updated` is the creation time.
        assert_eq!(
            new_note("---\ncreated: 2020-01-01T10:00:00+02:00\n---\nx\n", id(), &t),
            Ok("---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\ncreated: 2020-01-01T10:00:00+02:00\nupdated: 2026-09-28T05:15:30Z\n---\nx\n".to_owned())
        );
        assert!(matches!(
            new_note("---\na: [\n---\nx\n", id(), &t),
            Err(RenderError::Unreadable(_))
        ));
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
