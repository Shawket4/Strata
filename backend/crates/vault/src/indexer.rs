//! Writes derived rows (see [`crate::derive`]) into the user's index inside a scoped
//! transaction. Batches are written in two phases so rows can reference notes created in the
//! same batch: first every `notes` row and the entity/place/document shells, then everything
//! else.

use strata_common::NoteId;
use strata_index::ScopedTx;
use strata_index::repo::entities::{self, Document};
use strata_index::repo::notes::{self, SearchText};
use strata_index::repo::{dedupe, graph, tasks, vault as vrepo};

use crate::derive::Derived;
use crate::error::Result;

/// Writes a batch of derived notes (replacing whatever the index held for them).
pub async fn write(tx: &mut ScopedTx, batch: &[Derived]) -> Result<()> {
    // Phase 1: notes and entity shells.
    for d in batch {
        let Some(note) = &d.note else { continue };
        notes::upsert_note(tx, note).await?;
        match (&d.entity, note.trashed) {
            (Some(e), false) => {
                entities::upsert_entity(tx, e).await?;
                if d.place_parent.is_some() {
                    entities::upsert_place(tx, note.id, None).await?;
                } else {
                    vrepo::delete_place(tx, note.id).await?;
                }
                match &d.document {
                    Some(doc) => {
                        entities::upsert_document(
                            tx,
                            &Document {
                                copy_of: None,
                                location_id: None,
                                holder_id: None,
                                last_holder_id: None,
                                ..doc.clone()
                            },
                        )
                        .await?;
                    }
                    None => vrepo::delete_document(tx, note.id).await?,
                }
            }
            _ => {
                vrepo::delete_entity(tx, note.id).await?;
            }
        }
    }
    // Phase 2: everything that references other rows.
    for d in batch {
        let Some(note) = &d.note else { continue };
        let id = note.id;
        vrepo::delete_note_dedupe_keys(tx, id).await?;
        vrepo::delete_rejected_from(tx, id).await?;
        for (dst, t, at) in &d.rejected {
            graph::add_rejected(tx, id, *dst, t, *at).await?;
        }
        for k in &d.keep_both {
            dedupe::add_keep_both(tx, &k.kind, &k.a, &k.b, k.at).await?;
        }
        graph::replace_tags(tx, id, &d.tags).await?;
        graph::replace_aliases(tx, id, &d.aliases).await?;
        graph::replace_links(tx, id, &d.links).await?;
        vrepo::delete_relations_from(tx, id).await?;
        for r in &d.relations {
            graph::upsert_relation(tx, r).await?;
        }
        graph::replace_blocks(tx, id, &d.blocks).await?;
        if d.entity.is_some() {
            entities::replace_entity_aliases(tx, id, &d.entity_aliases).await?;
        }
        if let Some(parent) = d.place_parent {
            entities::upsert_place(tx, id, parent).await?;
        }
        if let Some(doc) = &d.document {
            entities::upsert_document(tx, doc).await?;
            vrepo::replace_custody(tx, id, &d.custody).await?;
        }
        vrepo::delete_mentions_in(tx, id).await?;
        for (entity, first, last) in &d.mentions {
            vrepo::put_mention(tx, *entity, id, *first, *last).await?;
        }
        let rows: Vec<tasks::Task> = d.tasks.iter().map(|(t, _)| t.clone()).collect();
        tasks::replace_note_tasks(tx, id, &rows).await?;
        for (t, reminders) in &d.tasks {
            tasks::replace_reminders(tx, &t.id, reminders).await?;
        }
        for k in &d.dedupe {
            dedupe::upsert_key(tx, &k.kind, &k.item_id, &k.exact, &k.trigram).await?;
        }
        notes::set_note_search(
            tx,
            id,
            SearchText {
                title: &d.search.0,
                tags: &d.search.1,
                body: &d.search.2,
            },
        )
        .await?;
    }
    Ok(())
}

/// Removes a purged note and every derived row of it (keep-both pairs recorded in its own
/// sidecar are passed in `own`).
pub async fn purge(tx: &mut ScopedTx, id: NoteId, own: Option<&Derived>) -> Result<()> {
    vrepo::delete_note_dedupe_keys(tx, id).await?;
    vrepo::delete_rejected_from(tx, id).await?;
    if let Some(d) = own {
        for k in &d.keep_both {
            vrepo::delete_keep_both(tx, &k.kind, &k.a, &k.b).await?;
        }
    }
    notes::delete_note(tx, id).await?;
    Ok(())
}
