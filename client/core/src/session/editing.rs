//! Intents behind the screens' explicit actions (`CORE_GAPS.md`): saving an editor's content
//! against the version it was built on, capture-level inbox decisions, link-or-create,
//! custody choices, suggestion threads, documents and places, merges, corrections of AI links
//! (D13), the user-owned `## Notes` section, properties and aliases, custody moves, reminders
//! on existing tasks, pins and reminder/sync settings. Every mutation is one or more outbox
//! ops (§12.3); decisions ("what does Accept mean for this capture") are taken here.

use std::collections::BTreeMap;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use sync_model::ops as sm;
use ulid::Ulid;
use vault_format::Document;
use vault_format::custody::Role;

use super::{Session, TaskEdit};
use crate::error::{CoreError, CoreResult};
use crate::store::write::{Intent, LocalEntity};
use crate::store::{cache, notes, settings, sync_state};
use crate::sync::model::{Op, Version};
use crate::view::Topics;
use crate::view::build;
use crate::view::model::{
    CreateOutcome, CustodyDraft, DocumentDraft, DuplicateChoice, LinkOrCreateChoice,
    LinkOrCreateKind, PlaceDraft, SuggestionEdits, SuggestionItem, SuggestionKind,
};

fn ulid_of(id: &str, field: &str) -> CoreResult<Ulid> {
    Ulid::from_string(id).map_err(|_| CoreError::invalid(field, "not_a_ulid"))
}

fn hhmm(s: &str, field: &str) -> CoreResult<String> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .map(|t| t.format("%H:%M").to_string())
        .map_err(|_| CoreError::invalid(field, "not_a_time"))
}

/// Replaces the body of the user-owned `## Notes` section (created at the end when missing).
pub fn with_user_notes(content: &str, text: &str) -> String {
    let mut doc = Document::parse(content);
    let body = doc.body().to_owned();
    let nl = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let mut text = text.trim_end().replace("\r\n", "\n").replace('\n', nl);
    if !text.is_empty() {
        text.push_str(nl);
    }
    let section = vault_format::sections::sections(&body)
        .into_iter()
        .find(|s| s.level == 2 && s.title.eq_ignore_ascii_case("notes"));
    let new_body = if let Some(s) = section {
        let mut out = String::new();
        out.push_str(&body[..s.content_span.start]);
        out.push_str(&text);
        let rest = &body[s.content_span.end..];
        if !rest.is_empty() && !text.is_empty() {
            out.push_str(nl);
        }
        out.push_str(rest);
        out
    } else {
        let mut out = body.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(nl);
        }
        if !out.is_empty() {
            out.push_str(nl);
        }
        out.push_str("## Notes");
        out.push_str(nl);
        out.push_str(&text);
        out
    };
    doc.set_body(new_body);
    doc.render()
}

impl Session {
    fn run_op(&self, local: LocalEntity, op: Op) -> CoreResult<String> {
        let op_id = self.env.ids.ulid();
        self.execute(&Intent { op_id, local, op })?;
        Ok(op_id.to_string())
    }

    /// Saves an editor's full content made against `base_version` (the `content_version` the
    /// editor loaded). Current → saved as is; the server's base → 3-way merged with the
    /// changes made since (D19); otherwise, or when the merge conflicts, `stale_edit`.
    pub fn update_note_at(
        &self,
        id: &str,
        content: &str,
        base_version: Option<&str>,
    ) -> CoreResult<String> {
        let Some(base) = base_version else {
            return self.update_note(id, content);
        };
        let (current, server_base) = self.read(|c, _| {
            Ok((
                notes::current(c, id)?,
                notes::get(c, id)?.and_then(|n| {
                    n.base_version
                        .zip(n.base_content)
                        .filter(|(v, _)| v == base)
                        .map(|(_, c)| c)
                }),
            ))
        })?;
        let current = current.ok_or_else(|| CoreError::not_found("note"))?;
        if Version::of_text(&current.content).as_str() == base {
            return self.update_note(id, content);
        }
        let Some(base_text) = server_base else {
            return Err(CoreError::StaleEdit);
        };
        match sync_model::merge(&base_text, content, &current.content) {
            sync_model::MergeOutcome::Clean(text) => self.update_note(id, &text),
            sync_model::MergeOutcome::Conflicted(_) => Err(CoreError::StaleEdit),
        }
    }

    fn pending_suggestions(&self) -> CoreResult<Vec<SuggestionItem>> {
        self.read(|c, ctx| build::suggestion_items(c, ctx, true))
    }

    fn suggestion(&self, id: &str) -> CoreResult<SuggestionItem> {
        self.read(|c, ctx| build::suggestion_items(c, ctx, false))?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| CoreError::not_found("suggestion"))
    }

    fn accept_with(&self, id: &str, edits: Option<sm::SuggestionEdits>) -> CoreResult<String> {
        self.run_op(
            LocalEntity::Suggestion(id.to_owned()),
            Op::SuggestionAccept(sm::SuggestionAccept {
                id: ulid_of(id, "id")?,
                edits,
                created: self.env.clock.now(),
            }),
        )
    }

    /// Accepts what the AI proposed for a capture: every pending suggestion that needs no
    /// choice. Fails with `needs_choice` when nothing can be accepted as is.
    pub fn accept_capture(&self, note_id: &str) -> CoreResult<Vec<String>> {
        let ready: Vec<SuggestionItem> = self
            .pending_suggestions()?
            .into_iter()
            .filter(|s| s.note_id.as_deref() == Some(note_id) && s.can_accept)
            .collect();
        if ready.is_empty() {
            return Err(CoreError::invalid("capture", "needs_choice"));
        }
        ready
            .iter()
            .map(|s| self.accept_suggestion(&s.id))
            .collect()
    }

    /// Rejects every pending suggestion of a capture (the capture stays in the inbox).
    pub fn reject_capture(&self, note_id: &str) -> CoreResult<Vec<String>> {
        self.pending_suggestions()?
            .into_iter()
            .filter(|s| s.note_id.as_deref() == Some(note_id))
            .map(|s| self.reject_suggestion(&s.id))
            .collect()
    }

    /// Accepts the listed captures that are ready (captures needing a choice are skipped).
    pub fn accept_captures(&self, note_ids: &[String]) -> CoreResult<Vec<String>> {
        let pending = self.pending_suggestions()?;
        let mut ops = Vec::new();
        for id in note_ids {
            let of: Vec<&SuggestionItem> = pending
                .iter()
                .filter(|s| s.note_id.as_deref() == Some(id.as_str()))
                .collect();
            if !of.is_empty() && of.iter().all(|s| s.can_accept) {
                for s in of {
                    ops.push(self.accept_suggestion(&s.id)?);
                }
            }
        }
        Ok(ops)
    }

    /// "Accept all ready".
    pub fn accept_all_ready(&self) -> CoreResult<Vec<String>> {
        let ids = self.read(|c, ctx| {
            Ok(build::inbox(c, ctx, crate::view::model::InboxFilter::All)?
                .captures
                .into_iter()
                .filter(|c| c.ready)
                .map(|c| c.note_id)
                .collect::<Vec<_>>())
        })?;
        self.accept_captures(&ids)
    }

    /// Accepts a proposal with the user's edits (`suggestion.accept` with edits): filing title,
    /// folder and tags; task text, due date, recurrence and reminders; the entity to point at
    /// and the aliases to add.
    pub fn accept_suggestion_with(&self, id: &str, e: SuggestionEdits) -> CoreResult<String> {
        self.suggestion(id)?;
        self.accept_with(
            id,
            Some(sm::SuggestionEdits {
                title: e.title,
                tags: e.tags,
                folder: e.folder,
                target_id: e
                    .target_id
                    .as_deref()
                    .map(|t| ulid_of(t, "target_id"))
                    .transpose()?,
                aliases: e.aliases,
                text: e.text,
                due: e.due,
                recurrence: e.recurrence,
                reminders: e.reminders,
            }),
        )
    }

    /// "Who is “بابا”?": link the mention to an existing entity or create one; either way the
    /// mention becomes an alias of the entity. A create with look-alikes returns them (no
    /// op) until repeated with `force`.
    pub fn resolve_link_or_create(
        &self,
        id: &str,
        choice: &LinkOrCreateChoice,
    ) -> CoreResult<CreateOutcome> {
        let s = self.suggestion(id)?;
        let mention = s.detail.mention.clone();
        let target = match choice.kind {
            LinkOrCreateKind::Link => choice
                .entity_id
                .clone()
                .ok_or_else(|| CoreError::invalid("entity_id", "missing"))?,
            LinkOrCreateKind::Create => {
                let name = choice
                    .name
                    .as_deref()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .ok_or_else(|| CoreError::invalid("name", "empty"))?;
                // The payload says which kind the mention is; the user may override it.
                let kind_name = choice
                    .entity_kind
                    .clone()
                    .filter(|k| !k.is_empty())
                    .or_else(|| Some(s.detail.entity_kind.clone()).filter(|k| !k.is_empty()))
                    .unwrap_or_else(|| "person".to_owned());
                let kind = match kind_name.as_str() {
                    "person" => domain::NoteKind::Person,
                    "company" => domain::NoteKind::Company,
                    _ => return Err(CoreError::invalid("entity_kind", "unknown")),
                };
                let aliases: Vec<String> = if mention.is_empty() || mention == name {
                    Vec::new()
                } else {
                    vec![mention.clone()]
                };
                let created = self.create_entity(kind, name, &aliases, choice.force)?;
                match created.id {
                    Some(id) => id,
                    None => return Ok(created),
                }
            }
        };
        self.accept_with(
            id,
            Some(sm::SuggestionEdits {
                target_id: Some(ulid_of(&target, "entity_id")?),
                aliases: (!mention.is_empty()).then(|| vec![mention.clone()]),
                ..sm::SuggestionEdits::default()
            }),
        )?;
        Ok(CreateOutcome {
            id: Some(target),
            candidates: Vec::new(),
        })
    }

    /// Picks the document of an ambiguous custody suggestion ("Which contract?").
    pub fn accept_suggestion_choice(&self, id: &str, document_id: &str) -> CoreResult<String> {
        let s = self.suggestion(id)?;
        if !s
            .detail
            .document_choices
            .iter()
            .any(|d| d.id.as_deref() == Some(document_id))
        {
            return Err(CoreError::invalid("document_id", "not_a_choice"));
        }
        self.accept_with(
            id,
            Some(sm::SuggestionEdits {
                target_id: Some(ulid_of(document_id, "document_id")?),
                ..sm::SuggestionEdits::default()
            }),
        )
    }

    /// Undo on a suggestion: a pending one is rejected; an AI change that was applied
    /// automatically needs the server's AI-decision undo (not in the contract yet).
    pub fn undo_suggestion(&self, id: &str) -> CoreResult<String> {
        let s = self.suggestion(id)?;
        if s.status == "pending" {
            return self.reject_suggestion(id);
        }
        Err(CoreError::NotAvailable {
            feature: "undo_ai_change".to_owned(),
        })
    }

    /// "Looks right" on an automatically applied AI change (hides it from the inbox).
    pub fn acknowledge_suggestion(&self, id: &str) -> CoreResult<()> {
        self.suggestion(id)?;
        self.write(|c, now| {
            let changed = cache::acknowledge(c, id, now)?;
            Ok((
                (),
                if changed {
                    Topics::INBOX | Topics::SUGGESTIONS
                } else {
                    Topics::NONE
                },
            ))
        })
    }

    /// A capture flagged as a duplicate: keep it (the server records "keep both") or discard
    /// it (the suggestion is rejected and the capture deleted).
    pub fn resolve_capture_duplicate(
        &self,
        id: &str,
        choice: DuplicateChoice,
    ) -> CoreResult<String> {
        let s = self.suggestion(id)?;
        if s.detail.kind != SuggestionKind::Duplicate {
            return Err(CoreError::invalid("suggestion", "not_a_duplicate"));
        }
        match choice {
            DuplicateChoice::CreateAnyway => self.accept_suggestion(id),
            DuplicateChoice::Discard => {
                let op = self.reject_suggestion(id)?;
                if let Some(note) = &s.note_id
                    && self.note_exists(note)?
                {
                    self.delete_note(note)?;
                }
                Ok(op)
            }
        }
    }

    /// Replies in a suggestion's thread (§9.8; the AI re-proposes with the reply).
    pub fn reply_to_suggestion(&self, id: &str, text: &str) -> CoreResult<String> {
        if text.trim().is_empty() {
            return Err(CoreError::invalid("text", "empty"));
        }
        self.suggestion(id)?;
        self.run_op(
            LocalEntity::Suggestion(id.to_owned()),
            Op::SuggestionReply(sm::SuggestionReply {
                id: ulid_of(id, "id")?,
                reply_id: self.env.ids.ulid(),
                text: text.trim().to_owned(),
            }),
        )
    }

    fn ids_of(&self, ids: &[String], field: &str) -> CoreResult<Vec<Ulid>> {
        ids.iter()
            .map(|i| {
                if !self.note_exists(i)? {
                    return Err(CoreError::not_found("note"));
                }
                ulid_of(i, field)
            })
            .collect()
    }

    /// Creates a document (duplicate-checked unless `force`).
    pub fn create_document(&self, d: &DocumentDraft, force: bool) -> CoreResult<CreateOutcome> {
        let name = d.name.trim();
        if name.is_empty() {
            return Err(CoreError::invalid("name", "empty"));
        }
        let copy = d
            .copy
            .as_deref()
            .map(|c| {
                c.parse::<domain::CopyKind>()
                    .map_err(|_| CoreError::invalid("copy", "unknown"))
            })
            .transpose()?;
        let copy_of = d
            .copy_of
            .as_deref()
            .map(|i| self.ids_of(&[i.to_owned()], "copy_of").map(|v| v[0]))
            .transpose()?;
        let id = self.env.ids.ulid();
        let refs: Vec<&str> = d.aliases.iter().map(String::as_str).collect();
        let item = dedupe::Item::entity(
            domain::DedupeKind::Document,
            Some(&id.to_string()),
            name,
            &refs,
        );
        self.create_item(
            &id.to_string(),
            &item,
            force,
            Op::DocumentCreate(sm::DocumentCreate {
                id,
                name: name.to_owned(),
                aliases: d.aliases.clone(),
                doc_type: d.doc_type.clone().filter(|t| !t.trim().is_empty()),
                copy,
                copy_of,
                companies: self.ids_of(&d.companies, "companies")?,
                people: self.ids_of(&d.people, "people")?,
                expires: d.expires,
                created: self.env.clock.now(),
                force,
            }),
        )
    }

    /// Creates a place (inside `parent_id` when given; duplicate-checked unless `force`).
    pub fn create_place(&self, p: &PlaceDraft, force: bool) -> CoreResult<CreateOutcome> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err(CoreError::invalid("name", "empty"));
        }
        let parent_id = p
            .parent_id
            .as_deref()
            .map(|i| self.ids_of(&[i.to_owned()], "parent_id").map(|v| v[0]))
            .transpose()?;
        let id = self.env.ids.ulid();
        let refs: Vec<&str> = p.aliases.iter().map(String::as_str).collect();
        let item = dedupe::Item::entity(
            domain::DedupeKind::Place,
            Some(&id.to_string()),
            name,
            &refs,
        );
        self.create_item(
            &id.to_string(),
            &item,
            force,
            Op::PlaceCreate(sm::PlaceCreate {
                id,
                name: name.to_owned(),
                aliases: p.aliases.clone(),
                parent_id,
                address: p.address.clone().filter(|a| !a.trim().is_empty()),
                created: self.env.clock.now(),
                force,
            }),
        )
    }

    fn create_item(
        &self,
        id: &str,
        item: &dedupe::Item,
        force: bool,
        op: Op,
    ) -> CoreResult<CreateOutcome> {
        if !force {
            let candidates: Vec<_> = self
                .read(|c, _| crate::search::duplicates::check(c, item))?
                .into_iter()
                .map(super::candidate_item)
                .collect();
            if !candidates.is_empty() {
                return Ok(CreateOutcome {
                    id: None,
                    candidates,
                });
            }
        }
        self.run_op(LocalEntity::Note(id.to_owned()), op)?;
        Ok(CreateOutcome {
            id: Some(id.to_owned()),
            candidates: Vec::new(),
        })
    }

    /// Merges entity `source` into `into` (the server rewrites links and trashes `source`).
    pub fn merge_entities(&self, source: &str, into: &str) -> CoreResult<String> {
        if source == into {
            return Err(CoreError::invalid("into", "same_entity"));
        }
        let kinds = self.read(|c, _| {
            let k = |id: &str| -> CoreResult<Option<String>> {
                Ok(notes::get(c, id)?.filter(|n| !n.deleted).map(|n| n.kind))
            };
            Ok((k(source)?, k(into)?))
        })?;
        match kinds {
            (Some(a), Some(b)) if a == b && matches!(a.as_str(), "person" | "company") => {}
            (Some(_), Some(_)) => return Err(CoreError::invalid("into", "different_kind")),
            _ => return Err(CoreError::not_found("note")),
        }
        self.run_op(
            LocalEntity::Note(source.to_owned()),
            Op::EntityMerge(sm::EntityMerge {
                id: ulid_of(source, "source")?,
                into_id: ulid_of(into, "into")?,
            }),
        )
    }

    /// Points an AI link at another entity ("this Ahmed is Ahmed Fathy", D13): the old edge
    /// is removed (the server records the rejection) and the new one added. Returns the add's
    /// op ID.
    pub fn repoint_relation(
        &self,
        src: &str,
        dst: &str,
        rel_type: &str,
        new_dst: &str,
    ) -> CoreResult<String> {
        if dst == new_dst {
            return Err(CoreError::invalid("new_dst", "same_target"));
        }
        self.remove_relation(src, dst, rel_type)?;
        self.add_relation(src, new_dst, rel_type)
    }

    /// Rejects a relation (§6.5: an AI edge is never re-proposed). Undo re-adds it
    /// (`add_relation`).
    pub fn reject_relation(&self, src: &str, dst: &str, rel_type: &str) -> CoreResult<String> {
        self.remove_relation(src, dst, rel_type)
    }

    /// Replaces the user-owned `## Notes` section of an entity, document or place (the AI
    /// never edits it; the UI never splices markdown).
    pub fn update_user_notes(&self, id: &str, text: &str) -> CoreResult<String> {
        let current = self
            .read(|c, _| notes::current(c, id))?
            .ok_or_else(|| CoreError::not_found("note"))?;
        self.update_note(id, &with_user_notes(&current.content, text))
    }

    fn patch_op(&self, id: &str, patch: sm::EntityPatch) -> CoreResult<String> {
        let kind = self
            .read(|c, _| Ok(notes::get(c, id)?.filter(|n| !n.deleted).map(|n| n.kind)))?
            .ok_or_else(|| CoreError::not_found("note"))?;
        let op = match kind.as_str() {
            "person" | "company" | "concept" => Op::EntityPatch(patch),
            "document" => Op::DocumentPatch(patch),
            "place" => Op::PlacePatch(patch),
            _ => return Err(CoreError::invalid("id", "not_an_entity")),
        };
        self.run_op(LocalEntity::Note(id.to_owned()), op)
    }

    fn empty_patch(id: &str) -> CoreResult<sm::EntityPatch> {
        Ok(sm::EntityPatch {
            id: ulid_of(id, "id")?,
            set: BTreeMap::new(),
            unset: Vec::new(),
            add_aliases: Vec::new(),
            remove_aliases: Vec::new(),
            set_lists: BTreeMap::new(),
        })
    }

    /// Sets a property of an entity, document or place ("+ Add property").
    pub fn set_property(&self, id: &str, key: &str, value: &str) -> CoreResult<String> {
        let key = key.trim();
        if key.is_empty() {
            return Err(CoreError::invalid("key", "empty"));
        }
        let mut p = Self::empty_patch(id)?;
        p.set.insert(key.to_owned(), value.to_owned());
        self.patch_op(id, p)
    }

    /// Sets a property to a list of values (several phone numbers, `aliases`, `tags`): the list
    /// replaces the whole value (items trimmed, blanks and repeats dropped, tags lose a leading
    /// `#`: the shared `sync_model::apply::clean_list_value`); an empty list removes the key.
    /// Relation lists change through relation intents (`add_relation`, …) and are refused here.
    pub fn set_property_values(&self, id: &str, key: &str, values: &[String]) -> CoreResult<String> {
        let key = key.trim();
        if key.is_empty() {
            return Err(CoreError::invalid("key", "empty"));
        }
        let mut p = Self::empty_patch(id)?;
        p.set_lists.insert(key.to_owned(), values.to_vec());
        self.patch_op(id, p)
    }

    /// Removes a property.
    pub fn remove_property(&self, id: &str, key: &str) -> CoreResult<String> {
        let mut p = Self::empty_patch(id)?;
        p.unset.push(key.to_owned());
        self.patch_op(id, p)
    }

    /// Adds an alias.
    pub fn add_alias(&self, id: &str, alias: &str) -> CoreResult<String> {
        let alias = alias.trim();
        if alias.is_empty() {
            return Err(CoreError::invalid("alias", "empty"));
        }
        let mut p = Self::empty_patch(id)?;
        p.add_aliases.push(alias.to_owned());
        self.patch_op(id, p)
    }

    /// Removes an alias.
    pub fn remove_alias(&self, id: &str, alias: &str) -> CoreResult<String> {
        let mut p = Self::empty_patch(id)?;
        p.remove_aliases.push(alias.to_owned());
        self.patch_op(id, p)
    }

    /// Records a custody event of a document ("Record a move", §6.12; `by: user`).
    pub fn record_custody(&self, document_id: &str, d: &CustodyDraft) -> CoreResult<String> {
        let event = d
            .kind
            .parse::<domain::CustodyEventType>()
            .map_err(|_| CoreError::invalid("kind", "unknown"))?;
        let is_doc = self.read(|c, _| {
            Ok(notes::get(c, document_id)?.is_some_and(|n| !n.deleted && n.kind == "document"))
        })?;
        if !is_doc {
            return Err(CoreError::not_found("document"));
        }
        let opt = |v: &Option<String>, field: &str| -> CoreResult<Option<Ulid>> {
            v.as_deref()
                .map(|i| self.ids_of(&[i.to_owned()], field).map(|v| v[0]))
                .transpose()
        };
        let place_id = opt(&d.place_id, "place_id")?;
        let person_id = opt(&d.person_id, "person_id")?;
        let counterparty_id = opt(&d.counterparty_id, "counterparty_id")?;
        // The event type's primary argument is required (the shared custody grammar).
        let missing = match vault_format::custody::primary(event) {
            Some(Role::Place) => place_id.is_none().then_some("place_id"),
            Some(Role::Person) => person_id.is_none().then_some("person_id"),
            Some(Role::Counterparty) => counterparty_id.is_none().then_some("counterparty_id"),
            None => None,
        };
        if let Some(field) = missing {
            return Err(CoreError::invalid(field, "missing"));
        }
        self.run_op(
            LocalEntity::Note(document_id.to_owned()),
            Op::DocumentCustody(sm::DocumentCustody {
                document_id: ulid_of(document_id, "document_id")?,
                event,
                at: d.date.unwrap_or_else(|| self.today()),
                place_id,
                person_id,
                counterparty_id,
                note: d
                    .note
                    .as_deref()
                    .and_then(vault_format::custody::clean_note),
            }),
        )
    }

    fn reminders_of(&self, task_id: &str) -> CoreResult<Vec<NaiveDateTime>> {
        self.read(|c, _| {
            let default_time = build::default_reminder_time(c)?;
            let mut st = c.prepare(
                "SELECT remind_date, remind_time FROM task_reminders WHERE task_id = ?1
                 ORDER BY remind_date, remind_time",
            )?;
            let rows: Vec<(String, String)> = st
                .query_map([task_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            Ok(rows
                .iter()
                .filter_map(|(d, t)| {
                    let day = NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
                    let time = NaiveTime::parse_from_str(t, "%H:%M").unwrap_or(default_time);
                    Some(day.and_time(time))
                })
                .collect())
        })
    }

    /// Adds a reminder to a task (a local wall-clock time).
    pub fn add_reminder(&self, task_id: &str, at: NaiveDateTime) -> CoreResult<String> {
        let mut r = self.reminders_of(task_id)?;
        if r.contains(&at) {
            return Err(CoreError::invalid("reminder", "exists"));
        }
        r.push(at);
        r.sort();
        self.update_task(
            task_id,
            &TaskEdit {
                reminders: Some(r),
                ..TaskEdit::default()
            },
        )
    }

    /// Removes a reminder (`ReminderItem::local_at`).
    pub fn remove_reminder(&self, task_id: &str, at: NaiveDateTime) -> CoreResult<String> {
        let mut r = self.reminders_of(task_id)?;
        let before = r.len();
        r.retain(|x| *x != at);
        if r.len() == before {
            return Err(CoreError::not_found("reminder"));
        }
        self.update_task(
            task_id,
            &TaskEdit {
                reminders: Some(r),
                ..TaskEdit::default()
            },
        )
    }

    /// Pins a note to the sidebar (this device) or unpins it.
    pub fn pin_note(&self, id: &str, pinned: bool) -> CoreResult<()> {
        if pinned && !self.note_exists(id)? {
            return Err(CoreError::not_found("note"));
        }
        self.write(|c, _| {
            let changed = cache::set_pinned(c, id, pinned)?;
            Ok(((), if changed { Topics::NOTES } else { Topics::NONE }))
        })
    }

    fn set_setting(&self, pairs: &[(&str, String)]) -> CoreResult<()> {
        self.write(|c, _| {
            let mut changed = false;
            for (k, v) in pairs {
                changed |= settings::set(c, k, v)?;
            }
            Ok((
                (),
                if changed {
                    Topics::SETTINGS | Topics::TASKS
                } else {
                    Topics::NONE
                },
            ))
        })
    }

    /// Time used for date-only reminders (`HH:MM`).
    pub fn set_default_reminder_time(&self, time: &str) -> CoreResult<()> {
        self.set_setting(&[(settings::DEFAULT_REMINDER_TIME, hhmm(time, "time")?)])
    }

    /// Quiet hours (`HH:MM`): reminders inside are delivered at the end.
    pub fn set_quiet_hours(&self, enabled: bool, from: &str, until: &str) -> CoreResult<()> {
        self.set_setting(&[
            (settings::QUIET_ENABLED, enabled.to_string()),
            (settings::QUIET_FROM, hhmm(from, "from")?),
            (settings::QUIET_UNTIL, hhmm(until, "until")?),
        ])
    }

    /// The notification's Snooze length (1–1440 minutes).
    pub fn set_snooze_minutes(&self, minutes: u32) -> CoreResult<()> {
        if !(1..=1440).contains(&minutes) {
            return Err(CoreError::invalid("minutes", "out_of_range"));
        }
        self.set_setting(&[(settings::SNOOZE_MINUTES, minutes.to_string())])
    }

    /// Pauses (or resumes) sync: while paused, cycles push and pull nothing.
    pub fn set_sync_paused(&self, paused: bool) -> CoreResult<()> {
        self.write(|c, now| {
            let before = sync_state::get(c)?.paused;
            if before == paused {
                return Ok(((), Topics::NONE));
            }
            sync_state::update(c, |s| s.paused = paused)?;
            cache::log(c, now, if paused { "paused" } else { "resumed" }, "")?;
            Ok(((), Topics::SYNC))
        })
    }

    /// Writes the unsynced ops to a readable file (disabled / deletion-pending accounts).
    pub fn export_unsynced(&self, path: &str) -> CoreResult<u32> {
        self.read(|c, _| super::account_ops::export_unsynced(c, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_notes_section_is_replaced_or_added() {
        assert_eq!(
            with_user_notes(
                "---\nid: x\n---\n## Summary\nAI text\n## Notes\nold\n### Sub\nkept?\n",
                "new line\nsecond"
            ),
            "---\nid: x\n---\n## Summary\nAI text\n## Notes\nnew line\nsecond\n"
        );
        assert_eq!(
            with_user_notes("---\nid: x\n---\n## Summary\nAI text\n", "mine"),
            "---\nid: x\n---\n## Summary\nAI text\n\n## Notes\nmine\n"
        );
        assert_eq!(
            with_user_notes(
                "## Notes\nold\n## Timeline\n- 2026-09-01 — x [[a]]\n",
                "new"
            ),
            "## Notes\nnew\n\n## Timeline\n- 2026-09-01 — x [[a]]\n"
        );
    }
}
