//! Rebasing queued ops after a pull (D19 on the device).
//!
//! The server 3-way merges a stale `note.update` against the base it finds in the note's git
//! history, but only a base the server once wrote: a queued edit made on top of an earlier,
//! still-unpushed (or server-merged) local edit carries a base version the server has never
//! seen. So whenever a pull brings a new server version of a note, the device rewrites the
//! note's **pending** ops on top of it, in outbox order:
//!
//! - `note.update`: `sync_model::merge(base the edit was made against, the edit, the new
//!   server state before it)`. A clean merge becomes the op's content and its base becomes the
//!   new state (so the push fast-forwards); a conflicting merge leaves the op unchanged, and the
//!   push then gets the server's conflict answer (a conflict copy), exactly as without rebase.
//! - every other op that needs a base (task edits, relations, patches) gets the base version of
//!   the new state it now applies to (`sync-model` rules: the task line's version for task
//!   edits, the note's version otherwise).
//!
//! Ops already sent (`inflight`) or waiting for the user (`conflict`, `duplicate`) are left as
//! they are. The rebase is deterministic and runs in the pull's transaction.

use rusqlite::Connection;
use sync_model::MergeOutcome;

use crate::error::CoreResult;
use crate::store::notes::{NoteBase, NoteState};
use crate::store::outbox::{self, OpStatus};
use crate::store::write::{DbLinks, LocalEntity, apply_to_note, base_version_for};
use crate::sync::model::{Op, Version};

/// What a rebase did (tests, the sync log).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RebaseReport {
    /// Ops whose payload or base changed.
    pub rebased: u32,
    /// `note.update`s left as they are because the merge conflicts.
    pub conflicting: u32,
}

/// Rebases the pending ops of note `id` onto `new_base` (the server state just pulled).
#[allow(clippy::single_match_else)] // two cases per op, as documented
pub fn rebase_note(
    conn: &Connection,
    id: &str,
    new_base: Option<&NoteBase>,
) -> CoreResult<RebaseReport> {
    let mut report = RebaseReport::default();
    let ops = outbox::live_for(conn, &LocalEntity::Note(id.to_owned()).key())?;
    if !ops.iter().any(|o| o.status == OpStatus::Pending) {
        return Ok(report);
    }
    let links = DbLinks::load(conn)?;
    let mut state: Option<NoteState> = new_base.map(|b| NoteState {
        path: b.path.clone(),
        content: b.content.clone(),
    });
    for op in ops {
        if op.status != OpStatus::Pending {
            if let Ok(next) = apply_to_note(state.clone(), &op.op, &links) {
                state = next;
            }
            continue;
        }
        match (&op.op, &state) {
            (Op::NoteUpdate(u), Some(current)) => {
                let base = op.base_content.clone();
                let new_version = Version::of_text(&current.content);
                if op.base_version.as_ref() == Some(&new_version) {
                    // Already made against this state.
                    state = Some(NoteState {
                        path: current.path.clone(),
                        content: u.content.clone(),
                    });
                    continue;
                }
                let merged = match base.as_deref() {
                    Some(base) => sync_model::merge(base, &u.content, &current.content),
                    // Unknown base (an op queued before v3): merge against the empty text, as
                    // the server does for an unknown base.
                    None => sync_model::merge("", &u.content, &current.content),
                };
                match merged {
                    MergeOutcome::Clean(text) => {
                        let mut next = u.clone();
                        next.content.clone_from(&text);
                        outbox::rebase(
                            conn,
                            &op.op_id,
                            &Op::NoteUpdate(next),
                            Some(&new_version),
                            Some(&current.content),
                        )?;
                        report.rebased += 1;
                        state = Some(NoteState {
                            path: current.path.clone(),
                            content: text,
                        });
                    }
                    MergeOutcome::Conflicted(_) => {
                        report.conflicting += 1;
                        state = Some(NoteState {
                            path: current.path.clone(),
                            content: u.content.clone(),
                        });
                    }
                }
            }
            _ => {
                if op.op.kind().requires_base_version()
                    && let Ok(base_version) = base_version_for(state.as_ref(), &op.op)
                    && base_version != op.base_version
                {
                    outbox::rebase(
                        conn,
                        &op.op_id,
                        &op.op,
                        base_version.as_ref(),
                        op.base_content.as_deref(),
                    )?;
                    report.rebased += 1;
                }
                if let Ok(next) = apply_to_note(state.clone(), &op.op, &links) {
                    state = next;
                }
            }
        }
    }
    Ok(report)
}
