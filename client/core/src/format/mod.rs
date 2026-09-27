//! `vault-format` adapters (PLAN §12.1 `format/`): everything the core reads out of, or
//! writes into, markdown goes through the shared crate so the client and the server never
//! disagree (L16). This module only shapes the crate's results for the local index, the
//! write path and the editor.

pub mod completions;
pub mod diff;
pub mod direction;
pub mod edit;
pub mod hints;
pub mod labels;
pub mod recurrence;
pub mod task_text;

use chrono::NaiveDate;
use vault_format::custody::{self, CustodyEvent};
use vault_format::frontmatter::PropertyValue;
use vault_format::sections;
use vault_format::tasks::{self, DateKind, TaskLine};
use vault_format::wikilink::WikiLink;
use vault_format::{Anchor, Document, KnownKey, RelationKey};

/// A link found in a body or a frontmatter value, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRef {
    /// The link path (resolution input).
    pub path: String,
    /// `![[…]]`.
    pub embed: bool,
    /// `#Heading` / `#^block` text, as written after `#`.
    pub anchor: Option<String>,
}

/// A task line of a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTask {
    /// Block ID (`t-…`), or `None` when the line has none yet.
    pub block_id: Option<String>,
    /// 0-based line number in the file.
    pub line_no: usize,
    /// The line text.
    pub line: String,
    /// Description without fields.
    pub description: String,
    /// `open` | `done` | `cancelled`.
    pub status: domain::TaskStatus,
    /// Priority (`normal` when none).
    pub priority: domain::Priority,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🛫
    pub start: Option<NaiveDate>,
    /// ✅
    pub done: Option<NaiveDate>,
    /// ❌
    pub cancelled: Option<NaiveDate>,
    /// 🔁 phrase as written.
    pub recurrence_raw: Option<String>,
    /// The compiled RRULE, when understood.
    pub rrule: Option<String>,
    /// Why the recurrence was not understood.
    pub recurrence_error: Option<String>,
    /// Reminders `(@date time)`.
    pub reminders: Vec<tasks::Reminder>,
}

/// Everything the index needs from one note.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNote {
    /// Title = file name without `.md`.
    pub title: String,
    /// Display title (`title:` property, else `title`).
    pub display_title: String,
    /// `kind` (plain notes are [`domain::NoteKind::Note`]).
    pub kind: domain::NoteKind,
    /// Frontmatter properties in file order (key, value).
    pub properties: Vec<(String, PropertyValue)>,
    /// `aliases`.
    pub aliases: Vec<String>,
    /// Frontmatter and inline tags, deduplicated, in first-seen order.
    pub tags: Vec<String>,
    /// Body wikilinks and embeds.
    pub links: Vec<LinkRef>,
    /// Relation key → link paths.
    pub relations: Vec<(String, String)>,
    /// `created` (RFC 3339 as written).
    pub created: Option<String>,
    /// `updated`.
    pub updated: Option<String>,
    /// `role` (person).
    pub role: Option<String>,
    /// `industry` (company).
    pub industry: Option<String>,
    /// Document fields.
    pub document: Option<DocumentFields>,
    /// Place parent link path (`part-of`, first entry).
    pub place_parent: Option<String>,
    /// Custody events of a document, newest first.
    pub custody: Vec<CustodyEvent>,
    /// Task lines.
    pub tasks: Vec<ParsedTask>,
    /// The body text (for full-text search).
    pub body: String,
}

/// Structured document properties (§6.12).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocumentFields {
    /// `doc-type`.
    pub doc_type: Option<String>,
    /// `copy`.
    pub copy: Option<String>,
    /// `copy-of` (first link path).
    pub copy_of: Option<String>,
    /// `location` link path.
    pub location: Option<String>,
    /// `holder` link path.
    pub holder: Option<String>,
    /// `last-holder` link path.
    pub last_holder: Option<String>,
    /// `status`.
    pub status: Option<String>,
    /// `expires`.
    pub expires: Option<String>,
}

/// The title of a path (file name without `.md`).
pub fn title_of(path: &str) -> String {
    vault_format::filename::title_from_path(path).to_owned()
}

fn link_path_of(value: &str) -> Option<String> {
    WikiLink::parse_exact(value.trim()).map(|l| l.target().to_owned())
}

fn anchor_text(a: &Anchor) -> String {
    match a {
        Anchor::Heading(h) => h.clone(),
        Anchor::Block(b) => format!("^{b}"),
    }
}

/// Parses a note for the index.
#[allow(clippy::too_many_lines)] // one straight pass over the note's parts
pub fn parse_note(path: &str, content: &str) -> ParsedNote {
    let doc = Document::parse(content);
    let title = title_of(path);
    let fm = doc.frontmatter().filter(|f| f.error().is_none());
    let text = |k: KnownKey| {
        fm.and_then(|f| f.text(k))
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let kind = fm
        .and_then(vault_format::Frontmatter::kind)
        .and_then(|k| k.as_str().parse::<domain::NoteKind>().ok())
        .unwrap_or(domain::NoteKind::Note);
    let properties = fm.map_or_else(Vec::new, |f| {
        f.keys()
            .map(|k| {
                (
                    k.to_owned(),
                    f.get(k).cloned().unwrap_or(PropertyValue::Null),
                )
            })
            .collect()
    });
    let analysis = doc.analyze_body();
    let mut tags: Vec<String> = Vec::new();
    let mut push_tag = |t: &str| {
        let t = t.trim_start_matches('#').to_owned();
        if !t.is_empty() && !tags.contains(&t) {
            tags.push(t);
        }
    };
    if let Some(f) = fm {
        for t in f.tags() {
            push_tag(&t);
        }
    }
    for t in &analysis.tags {
        push_tag(&t.name);
    }
    let links = analysis
        .links
        .iter()
        .filter(|l| !l.path.is_empty())
        .map(|l| LinkRef {
            path: l.target().to_owned(),
            embed: l.embed,
            anchor: l.anchor.as_ref().map(anchor_text),
        })
        .collect();
    let mut relations = Vec::new();
    if let Some(f) = fm {
        for rk in RelationKey::all() {
            for l in f.relation_links(rk) {
                relations.push((rk.as_str().to_owned(), l.target().to_owned()));
            }
        }
    }
    let link_value = |k: KnownKey| text(k).and_then(|v| link_path_of(&v));
    let document = (kind == domain::NoteKind::Document).then(|| DocumentFields {
        doc_type: text(KnownKey::DocType),
        copy: text(KnownKey::Copy),
        copy_of: relations
            .iter()
            .find(|(k, _)| k == "copy-of")
            .map(|(_, p)| p.clone()),
        location: link_value(KnownKey::Location),
        holder: link_value(KnownKey::Holder),
        last_holder: link_value(KnownKey::LastHolder),
        status: text(KnownKey::Status),
        expires: text(KnownKey::Expires),
    });
    let place_parent = (kind == domain::NoteKind::Place)
        .then(|| {
            relations
                .iter()
                .find(|(k, _)| k == "part-of")
                .map(|(_, p)| p.clone())
        })
        .flatten();
    let custody = if kind == domain::NoteKind::Document {
        custody_events(doc.body())
    } else {
        Vec::new()
    };
    let body_start_line = content[..doc.body_offset().min(content.len())]
        .matches('\n')
        .count();
    let tasks = tasks::extract_tasks(doc.body())
        .into_iter()
        .map(|t| parse_task(&t.task, body_start_line + t.line_number))
        .collect();
    ParsedNote {
        display_title: text(KnownKey::Title).unwrap_or_else(|| title.clone()),
        title,
        kind,
        properties,
        aliases: fm.map_or_else(Vec::new, vault_format::Frontmatter::aliases),
        tags,
        links,
        relations,
        created: text(KnownKey::Created),
        updated: text(KnownKey::Updated),
        role: text(KnownKey::Role),
        industry: text(KnownKey::Industry),
        document,
        place_parent,
        custody,
        tasks,
        body: doc.body().to_owned(),
    }
}

fn custody_events(body: &str) -> Vec<CustodyEvent> {
    sections::sections(body)
        .into_iter()
        .find(|s| s.level == 2 && s.title.eq_ignore_ascii_case("Custody"))
        .map(|s| custody::parse_section(&body[s.own_content_span]).0)
        .unwrap_or_default()
}

/// Parses one task line.
pub fn parse_task(t: &TaskLine, line_no: usize) -> ParsedTask {
    let (rrule, recurrence_error) = match t.recurrence() {
        Some(Ok(rule)) => (Some(rule.to_rrule()), None),
        Some(Err(e)) => (None, Some(e.to_string())),
        None => (None, None),
    };
    ParsedTask {
        block_id: t.block_id().map(str::to_owned),
        line_no,
        line: t.as_str().to_owned(),
        description: t.description().to_owned(),
        status: t.status().lifecycle(),
        priority: t.priority().unwrap_or(domain::Priority::Normal),
        due: t.date(DateKind::Due),
        scheduled: t.date(DateKind::Scheduled),
        start: t.date(DateKind::Start),
        done: t.date(DateKind::Done),
        cancelled: t.date(DateKind::Cancelled),
        recurrence_raw: t.recurrence_text().map(str::to_owned),
        rrule,
        recurrence_error,
        reminders: t.reminders(),
    }
}

/// Human-readable text of a property value (for the properties panel).
pub fn property_display(v: &PropertyValue) -> Vec<String> {
    match v {
        PropertyValue::Null | PropertyValue::Other => Vec::new(),
        PropertyValue::Text(s) => vec![s.clone()],
        PropertyValue::List(l) => l.clone(),
    }
}

#[cfg(test)]
mod tests;
