//! Derivation of every index row of one note from its file (PLAN §7.4 derived tables): the
//! note row and search text, tags, aliases, body links, relations with sidecar provenance,
//! rejected edges, blocks, entity rows and aliases, places, documents and custody events,
//! mentions, tasks and reminders, and duplicate keys.
//!
//! Pure: the same file, sidecar, vault paths and time zone always derive the same rows, which
//! is what makes a full rebuild equal to the incremental state.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use domain::{MentionType, NoteKind as DNoteKind, RelationType};
use sha2::{Digest, Sha256};
use strata_common::{CustodyEventId, NoteId};
use strata_index::repo::entities::{CustodyEvent, Document, Entity};
use strata_index::repo::graph::{Block, Link, Relation};
use strata_index::repo::notes::Note;
use strata_index::repo::tasks::Task;
use strata_index::types::{
    By, CustodyType, DocCopy, DocStatus, EntityKind, Lang, LinkKind, NoteKind, Priority, TaskStatus,
};
use text_normalize::normalize_for_search;
use ulid::Ulid;
use vault_format::frontmatter::KnownKey;
use vault_format::sidecar::NoteSidecar;
use vault_format::tasks::{DateKind, extract_tasks};
use vault_format::{Anchor, Document as VDocument, PathIndex, RelationKey, Resolution, WikiLink};

use crate::state::{VaultState, name_key};

/// Default time of a date-only reminder `(@YYYY-MM-DD)`.
pub const DEFAULT_REMINDER_TIME: NaiveTime = match NaiveTime::from_hms_opt(9, 0, 0) {
    Some(t) => t,
    None => NaiveTime::MIN,
};

/// Sidecar key (unknown to `NoteSidecar`, kept in its `extra`) holding keep-both decisions
/// whose items are not both this note and another note (tasks of this note, or this note and
/// a task): `[{"kind": "task", "a": "t-…", "b": "t-…", "at": …}]`, as `dedupe::KeepBoth`.
pub const KEEP_BOTH_ITEMS_KEY: &str = "keep_both_items";

/// Everything the index needs to know about the rest of the vault.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// Live content files.
    pub index: &'a PathIndex,
    /// Notes by path (kinds and IDs).
    pub state: &'a VaultState,
    /// The user's time zone (reminders).
    pub tz: Tz,
}

impl Context<'_> {
    fn resolve(&self, link: &str, source: &str) -> Option<(NoteId, DNoteKind)> {
        match self.index.resolve(link, Some(source)) {
            Resolution::Resolved(p) => self.state.notes.get(&p).map(|m| (m.id, m.kind)),
            _ => None,
        }
    }
}

/// The stored duplicate keys of one item (`dedupe::Item::keys`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupeRow {
    /// Kind (`note`, `capture`, `task`, `person`, …).
    pub kind: String,
    /// Item ID (note ULID or task block ID).
    pub item_id: String,
    /// One row per name.
    pub rows: Vec<strata_index::repo::vault::DedupeKeyRow>,
}

/// A keep-both pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepBothRow {
    /// Kind.
    pub kind: String,
    /// One item.
    pub a: String,
    /// The other.
    pub b: String,
    /// When the user chose it.
    pub at: DateTime<Utc>,
}

/// The rows derived from one note.
#[derive(Debug, Clone, Default)]
pub struct Derived {
    /// The `notes` row.
    pub note: Option<Note>,
    /// Normalised search text: title, tags, body.
    pub search: (String, String, String),
    /// Tags (frontmatter + inline), sorted, unique.
    pub tags: Vec<String>,
    /// Frontmatter aliases.
    pub aliases: Vec<String>,
    /// Body links in order.
    pub links: Vec<Link>,
    /// Outgoing relations.
    pub relations: Vec<Relation>,
    /// Rejected edges from the sidecar: (target, type, at).
    pub rejected: Vec<(NoteId, String, DateTime<Utc>)>,
    /// Blocks with IDs.
    pub blocks: Vec<Block>,
    /// Entity row (people, companies, documents, places).
    pub entity: Option<Entity>,
    /// Entity aliases: (alias, normalised).
    pub entity_aliases: Vec<(String, String)>,
    /// `Some(parent)` for places.
    pub place_parent: Option<Option<NoteId>>,
    /// Document row.
    pub document: Option<Document>,
    /// Custody events, newest first.
    pub custody: Vec<CustodyEvent>,
    /// Mentions: (entity, first seen, last seen).
    pub mentions: Vec<(NoteId, DateTime<Utc>, DateTime<Utc>)>,
    /// Tasks with their reminder instants.
    pub tasks: Vec<(Task, Vec<DateTime<Utc>>)>,
    /// Duplicate keys.
    pub dedupe: Vec<DedupeRow>,
    /// Keep-both pairs recorded in the sidecar.
    pub keep_both: Vec<KeepBothRow>,
    /// Lower-cased link names this note refers to.
    pub link_names: BTreeSet<String>,
    /// The note's kind.
    pub kind: Option<DNoteKind>,
}

fn to_utc(ts: DateTime<chrono::FixedOffset>) -> DateTime<Utc> {
    ts.with_timezone(&Utc)
}

fn ulid_time(id: NoteId) -> DateTime<Utc> {
    let ms = id.as_ulid().timestamp_ms();
    DateTime::from_timestamp_millis(i64::try_from(ms).unwrap_or(0)).unwrap_or_default()
}

/// The display title of a note: frontmatter `title` if set, else the file name.
pub fn title_of(path: &str, doc: &VDocument) -> String {
    doc.frontmatter()
        .and_then(|f| f.title())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map_or_else(
            || vault_format::filename::title_from_path(crate::paths::file_name(path)).to_owned(),
            str::to_owned,
        )
}

/// The note kind in frontmatter (`note` when absent or unknown).
pub fn kind_of(doc: &VDocument) -> DNoteKind {
    doc.frontmatter()
        .and_then(vault_format::Frontmatter::kind)
        .and_then(|k| k.known())
        .unwrap_or(DNoteKind::Note)
}

/// The dedupe kind of a note.
pub fn dedupe_kind(path: &str, kind: DNoteKind) -> &'static str {
    if kind == DNoteKind::Note && is_inbox(path) {
        "capture"
    } else {
        kind.as_str()
    }
}

/// Task text with wikilinks replaced by their display text (for keys and search).
pub fn strip_links(text: &str) -> String {
    let links = vault_format::wikilink::find_all(text);
    vault_format::rewrite::replace_links(text, &links, |l| {
        Some(l.alias.clone().unwrap_or_else(|| l.target().to_owned()))
    })
    .text
}

fn map_priority(p: Option<domain::Priority>) -> Option<Priority> {
    match p? {
        domain::Priority::Highest => Some(Priority::Highest),
        domain::Priority::High => Some(Priority::High),
        domain::Priority::Medium => Some(Priority::Medium),
        domain::Priority::Normal => None,
        domain::Priority::Low => Some(Priority::Low),
        domain::Priority::Lowest => Some(Priority::Lowest),
    }
}

fn index_kind(kind: DNoteKind) -> NoteKind {
    match kind {
        DNoteKind::Note => NoteKind::Note,
        DNoteKind::Concept => NoteKind::Concept,
        DNoteKind::Person => NoteKind::Person,
        DNoteKind::Company => NoteKind::Company,
        DNoteKind::Document => NoteKind::Document,
        DNoteKind::Place => NoteKind::Place,
    }
}

/// The entity kind of a note kind.
pub fn entity_kind(kind: DNoteKind) -> Option<EntityKind> {
    match kind {
        DNoteKind::Person => Some(EntityKind::Person),
        DNoteKind::Company => Some(EntityKind::Company),
        DNoteKind::Document => Some(EntityKind::Document),
        DNoteKind::Place => Some(EntityKind::Place),
        DNoteKind::Note | DNoteKind::Concept => None,
    }
}

/// A deterministic ID for the `n`th custody line of a document.
fn custody_id(doc: NoteId, n: usize, line: &str, at: DateTime<Utc>) -> CustodyEventId {
    let mut h = Sha256::new();
    h.update(doc.to_string().as_bytes());
    h.update(n.to_le_bytes());
    h.update(line.as_bytes());
    let digest = h.finalize();
    let mut rand = [0u8; 16];
    rand.copy_from_slice(&digest[..16]);
    let random = u128::from_be_bytes(rand) & ((1u128 << 80) - 1);
    let ms = u64::try_from(at.timestamp_millis().max(0)).unwrap_or(0);
    CustodyEventId::from_ulid(Ulid::from_parts(ms, random))
}

fn link_values(doc: &VDocument) -> Vec<WikiLink> {
    let Some(fm) = doc.frontmatter() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in KnownKey::all().filter(|k| k.holds_links()) {
        for v in fm.list(key) {
            if let Some(l) = WikiLink::parse_exact(v.trim()) {
                out.push(l);
            }
        }
    }
    out
}

/// Derives every index row of the note at `path` (or of a trashed note, whose only row is
/// `notes` with `trashed = true`).
#[allow(clippy::too_many_lines)] // one linear pass; splitting would scatter the rules
pub fn derive(
    path: &str,
    text: &str,
    sidecar: Option<&NoteSidecar>,
    ctx: &Context<'_>,
    trashed: bool,
) -> Option<Derived> {
    let doc = VDocument::parse(text);
    let fm = doc.frontmatter();
    let id = NoteId::from_ulid(fm.and_then(|f| f.id().ok().flatten())?);
    let kind = kind_of(&doc);
    let title = title_of(path, &doc);
    let created = fm
        .and_then(|f| f.created().ok().flatten())
        .map_or_else(|| ulid_time(id), to_utc);
    let updated = fm
        .and_then(|f| f.updated().ok().flatten())
        .map_or(created, to_utc)
        .max(created);
    let lang = fm
        .and_then(vault_format::Frontmatter::lang)
        .and_then(|l| l.known())
        .and_then(|l| l.as_str().parse::<Lang>().ok());
    let body = doc.body();
    let mut out = Derived {
        note: Some(Note {
            id,
            path: path.to_owned(),
            title: title.clone(),
            kind: index_kind(kind),
            lang,
            created,
            updated,
            content_hash: crate::fsio::version_of(text.as_bytes()),
            word_count: i32::try_from(body.split_whitespace().count()).unwrap_or(i32::MAX),
            trashed,
        }),
        kind: Some(kind),
        ..Derived::default()
    };
    if let Some(s) = sidecar {
        for r in &s.rejected {
            out.rejected.push((
                NoteId::from_ulid(r.target_id),
                r.kind.as_str().to_owned(),
                to_utc(r.at),
            ));
        }
        let own = dedupe_kind(crate::paths::untrash_path(path).unwrap_or(path), kind).to_owned();
        for k in &s.keep_both {
            let (a, b) = (id.to_string(), k.other_id.to_string());
            if a != b {
                out.keep_both.push(KeepBothRow {
                    kind: own.clone(),
                    a,
                    b,
                    at: to_utc(k.at),
                });
            }
        }
        if let Some(serde_json::Value::Array(items)) = s.extra.get(KEEP_BOTH_ITEMS_KEY) {
            for item in items {
                let get = |k: &str| item.get(k).and_then(serde_json::Value::as_str);
                if let (Some(kind), Some(a), Some(b), Some(at)) =
                    (get("kind"), get("a"), get("b"), get("at"))
                    && let Ok(at) = DateTime::parse_from_rfc3339(at)
                    && a != b
                {
                    out.keep_both.push(KeepBothRow {
                        kind: kind.to_owned(),
                        a: a.to_owned(),
                        b: b.to_owned(),
                        at: to_utc(at),
                    });
                }
            }
        }
    }

    if trashed {
        return Some(out);
    }
    let analysis = doc.analyze_body();

    // Tags and aliases.
    let mut tags: BTreeSet<String> = fm
        .map(vault_format::Frontmatter::tags)
        .unwrap_or_default()
        .into_iter()
        .collect();
    tags.extend(analysis.tags.iter().map(|t| t.name.clone()));
    out.tags = tags.into_iter().collect();
    let mut aliases: Vec<String> = Vec::new();
    for a in fm
        .map(vault_format::Frontmatter::aliases)
        .unwrap_or_default()
    {
        let a = a.trim().to_owned();
        if !a.is_empty() && !aliases.contains(&a) {
            aliases.push(a);
        }
    }
    aliases.sort();
    out.aliases.clone_from(&aliases);
    out.search = (
        normalize_for_search(&title),
        normalize_for_search(&out.tags.join(" ")),
        normalize_for_search(body),
    );

    // Body links.
    for (ord, l) in analysis.links.iter().enumerate() {
        let dst = if l.path.trim().is_empty() {
            Some(id)
        } else {
            ctx.resolve(&l.path, path).map(|(i, _)| i)
        };
        let (anchor, block_id) = match &l.anchor {
            Some(Anchor::Heading(h)) => (Some(h.clone()), None),
            Some(Anchor::Block(b)) => (None, Some(b.clone())),
            None => (None, None),
        };
        out.links.push(Link {
            src_id: id,
            ord: i32::try_from(ord).unwrap_or(i32::MAX),
            dst_id: dst,
            dst_raw: l.path.clone(),
            kind: if l.embed {
                LinkKind::Embed
            } else {
                LinkKind::Link
            },
            anchor,
            block_id,
        });
        if !l.path.trim().is_empty() {
            out.link_names.insert(name_key(l.path.trim()));
        }
    }
    for l in link_values(&doc) {
        if !l.path.trim().is_empty() {
            out.link_names.insert(name_key(l.path.trim()));
        }
    }

    // Relations (frontmatter keys) with sidecar provenance.
    let mut seen: BTreeSet<(String, NoteId)> = BTreeSet::new();
    let mut targets: BTreeMap<RelationKey, Vec<(NoteId, DNoteKind)>> = BTreeMap::new();
    if let Some(f) = fm.filter(|f| f.error().is_none()) {
        for rk in RelationKey::all() {
            for l in f.relation_links(rk) {
                let Some((dst, dkind)) = ctx.resolve(&l.path, path) else {
                    continue;
                };
                if dst == id || !seen.insert((rk.as_str().to_owned(), dst)) {
                    continue;
                }
                targets.entry(rk).or_default().push((dst, dkind));
                let prov = sidecar.and_then(|s| {
                    s.relations
                        .iter()
                        .find(|r| r.kind == rk && NoteId::from_ulid(r.target_id) == dst)
                });
                #[allow(clippy::cast_possible_truncation)]
                out.relations.push(Relation {
                    src_id: id,
                    dst_id: dst,
                    rel_type: rk.as_str().to_owned(),
                    by: match prov.map(|p| p.by) {
                        Some(domain::RelationOrigin::Ai) => By::Ai,
                        _ => By::User,
                    },
                    confidence: prov.and_then(|p| p.confidence).map(|c| c as f32),
                    reason: prov.and_then(|p| p.reason.clone()),
                    created: prov.map_or(created, |p| to_utc(p.created)),
                    decision_id: None,
                });
            }
        }
    }
    out.relations
        .sort_by(|a, b| (&a.rel_type, a.dst_id).cmp(&(&b.rel_type, b.dst_id)));
    // Blocks with IDs.
    let body_offset = doc.body_offset();
    let mut block_ids = BTreeSet::new();
    for b in &analysis.blocks {
        let Some(bid) = &b.id else { continue };
        if !block_ids.insert(bid.id.clone()) {
            continue;
        }
        out.blocks.push(Block {
            note_id: id,
            block_id: bid.id.clone(),
            heading_path: b.heading_path.join("/"),
            text: body[b.span.clone()].to_owned(),
            start_offset: i32::try_from(body_offset + b.span.start).unwrap_or(i32::MAX),
            end_offset: i32::try_from(body_offset + b.span.end).unwrap_or(i32::MAX),
        });
    }

    // Entities.
    if let Some(ek) = entity_kind(kind) {
        let text_of = |k: KnownKey| {
            fm.and_then(|f| f.text(k))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        out.entity = Some(Entity {
            note_id: id,
            kind: ek,
            display_name: title.clone(),
            role: text_of(KnownKey::Role),
            industry: text_of(KnownKey::Industry),
        });
        let mut names = vec![title.clone()];
        names.extend(aliases.iter().cloned());
        let mut seen_alias = BTreeSet::new();
        for n in names {
            let norm = normalize_for_search(&n);
            if !norm.is_empty() && seen_alias.insert(n.clone()) {
                out.entity_aliases.push((n, norm));
            }
        }
        let rel = |rk: RelationKey, want: DNoteKind| {
            targets
                .get(&rk)
                .and_then(|v| v.iter().find(|(_, k)| *k == want).map(|(i, _)| *i))
        };
        if kind == DNoteKind::Place {
            out.place_parent = Some(rel(
                RelationKey::Note(RelationType::PartOf),
                DNoteKind::Place,
            ));
        }
        if kind == DNoteKind::Document {
            out.document = Some(derive_document(&doc, id, path, ctx, &targets));
            out.custody = derive_custody(&doc, id, path, ctx, updated);
        }
    }

    // Mentions (people:/companies:).
    for mt in [MentionType::People, MentionType::Companies] {
        for (dst, dkind) in targets.get(&RelationKey::Mention(mt)).into_iter().flatten() {
            if entity_kind(*dkind).is_some() {
                out.mentions.push((*dst, created, updated));
            }
        }
    }

    // Tasks.
    let fm_lines = text[..body_offset].matches('\n').count();
    let mut task_ids = BTreeSet::new();
    for t in extract_tasks(body) {
        let Some(tid) = t.task.block_id().map(str::to_owned) else {
            continue;
        };
        if !task_ids.insert(tid.clone()) {
            continue;
        }
        let task = &t.task;
        let reference = task
            .date(DateKind::Due)
            .or_else(|| task.date(DateKind::Scheduled))
            .or_else(|| task.date(DateKind::Start));
        let (rrule, understood) = match task.recurrence() {
            None => (None, true),
            Some(Ok(rule)) => (
                Some(reference.map_or_else(|| rule.to_rrule(), |d| rule.to_rrule_anchored(d))),
                true,
            ),
            Some(Err(_)) => (None, false),
        };
        let line = i32::try_from(fm_lines + t.line_number).unwrap_or(i32::MAX);
        let description = task.description().to_owned();
        let mut reminders: Vec<DateTime<Utc>> = task
            .reminders()
            .iter()
            .map(|r| {
                vault_format::tasks::reminder_instant(r.date, r.time, DEFAULT_REMINDER_TIME, ctx.tz)
                    .with_timezone(&Utc)
            })
            .collect();
        reminders.sort();
        reminders.dedup();
        if task.status().is_open() {
            out.dedupe
                .push(crate::dup::rows(&crate::dup::task_item(&tid, task)));
        }
        out.tasks.push((
            Task {
                id: tid,
                note_id: id,
                text: description,
                status: match task.status().lifecycle() {
                    domain::TaskStatus::Open => TaskStatus::Open,
                    domain::TaskStatus::Done => TaskStatus::Done,
                    domain::TaskStatus::Cancelled => TaskStatus::Cancelled,
                },
                due: task.date(DateKind::Due),
                scheduled: task.date(DateKind::Scheduled),
                start: task.date(DateKind::Start),
                recurrence_raw: task.recurrence_text().map(str::to_owned),
                rrule,
                recurrence_understood: understood,
                priority: map_priority(task.priority()),
                done_at: task.date(DateKind::Done),
                line_start: line,
                line_end: line,
            },
            reminders,
        ));
    }

    // Duplicate keys of the note itself.
    out.dedupe.push(crate::dup::rows(&crate::dup::note_item(
        path,
        Some(id),
        &doc,
    )));
    Some(out)
}

fn derive_document(
    doc: &VDocument,
    id: NoteId,
    path: &str,
    ctx: &Context<'_>,
    targets: &BTreeMap<RelationKey, Vec<(NoteId, DNoteKind)>>,
) -> Document {
    let fm = doc.frontmatter();
    let link_to = |key: KnownKey, want: &[DNoteKind]| {
        let l = fm.and_then(|f| f.link(key))?;
        let (dst, k) = ctx.resolve(&l.path, path)?;
        (want.contains(&k) && dst != id).then_some(dst)
    };
    let entities = [
        DNoteKind::Person,
        DNoteKind::Company,
        DNoteKind::Document,
        DNoteKind::Place,
    ];
    Document {
        note_id: id,
        doc_type: fm
            .and_then(vault_format::Frontmatter::doc_type)
            .map(|d| d.as_str().trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "other".to_owned()),
        copy: fm
            .and_then(vault_format::Frontmatter::copy_kind)
            .and_then(|c| c.known())
            .and_then(|c| c.as_str().parse::<DocCopy>().ok())
            .unwrap_or(DocCopy::Original),
        copy_of: targets
            .get(&RelationKey::Document(domain::DocumentRelationType::CopyOf))
            .and_then(|v| v.iter().find(|(_, k)| *k == DNoteKind::Document))
            .map(|(i, _)| *i),
        location_id: link_to(KnownKey::Location, &[DNoteKind::Place]),
        holder_id: link_to(KnownKey::Holder, &entities),
        last_holder_id: link_to(KnownKey::LastHolder, &entities),
        status: fm
            .and_then(vault_format::Frontmatter::status)
            .and_then(|s| s.known())
            .and_then(|s| s.as_str().parse::<DocStatus>().ok())
            .unwrap_or(DocStatus::Stored),
        expires: fm.and_then(|f| f.expires().ok().flatten()),
    }
}

/// The custody events of a document body (the `## Custody` section), newest first.
pub fn custody_events(body: &str) -> Vec<vault_format::custody::CustodyEvent> {
    let secs = vault_format::sections::sections(body);
    let Some(s) = secs
        .iter()
        .find(|s| s.level == 2 && s.title.trim().eq_ignore_ascii_case("Custody"))
    else {
        return Vec::new();
    };
    vault_format::custody::parse_section(&body[s.own_content_span.clone()]).0
}

fn derive_custody(
    doc: &VDocument,
    id: NoteId,
    path: &str,
    ctx: &Context<'_>,
    updated: DateTime<Utc>,
) -> Vec<CustodyEvent> {
    let entity = |link: Option<&String>, want: &[DNoteKind]| {
        let l = WikiLink::parse_exact(link?.trim())?;
        let (dst, k) = ctx.resolve(&l.path, path)?;
        want.contains(&k).then_some(dst)
    };
    let people = [
        DNoteKind::Person,
        DNoteKind::Company,
        DNoteKind::Place,
        DNoteKind::Document,
    ];
    let mut out = Vec::new();
    for (n, e) in custody_events(doc.body()).iter().enumerate() {
        let at = Utc.from_utc_datetime(&e.date.and_time(NaiveTime::MIN));
        let source = e
            .citations
            .first()
            .and_then(|c| WikiLink::parse_exact(c.trim()));
        let source_note = source
            .as_ref()
            .and_then(|l| ctx.resolve(&l.path, path).map(|(i, _)| i));
        let source_block = source.as_ref().and_then(|l| match &l.anchor {
            Some(Anchor::Block(b)) if source_note.is_some() => Some(b.clone()),
            _ => None,
        });
        out.push(CustodyEvent {
            id: custody_id(id, n, &e.to_line(), at),
            document_id: id,
            event_type: e
                .kind
                .as_str()
                .parse::<CustodyType>()
                .unwrap_or(CustodyType::StoredAt),
            at,
            place_id: entity(e.place.as_ref(), &[DNoteKind::Place]),
            person_id: entity(e.person.as_ref(), &people),
            counterparty_id: entity(e.counterparty.as_ref(), &people),
            by: By::User,
            confidence: None,
            source_note_id: source_note,
            source_block_id: source_block,
            created: updated,
        });
    }
    out
}

/// Is `path` inside the inbox? (The shared rule: a file directly in `inbox/`.)
pub fn is_inbox(path: &str) -> bool {
    item_render::paths::is_inbox_path(path)
}
