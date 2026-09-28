//! A note's AI follow-up thread (owner decision 2026-09-28): `.meta/threads/<note-id>.json`,
//! written in one `ai: thread <path>` commit per exchange, with its `note_threads` row and a
//! `thread` change-log row in the same transaction (devices receive the thread in sync).

use strata_common::NoteId;
use strata_index::UserScope;
use strata_index::repo::sync::{self as sync_repo, NewChange};
use strata_index::repo::threads;
use strata_index::types::ChangeOp;
use vault_format::thread::{NoteThread, ThreadMessage};

use crate::VaultService;
use crate::error::{Result, VaultError};
use crate::fsio;
use crate::ops::files::{Expect, FileWrite};
use crate::store::{Author, Core};

/// Restores the `note_threads` rows from the thread `files` (note, JSON) after the derived
/// rows were cleared (`stratad reindex`), for notes the index holds, and logs a `thread`
/// change for each thread that differs from `before` (note, version) or disappeared.
pub(crate) async fn reload(
    tx: &mut strata_index::ScopedTx,
    state: &crate::state::VaultState,
    files: &[(NoteId, String)],
    before: &[(NoteId, String)],
    at: chrono::DateTime<chrono::Utc>,
) -> Result<usize> {
    let mut now: Vec<(NoteId, String)> = Vec::new();
    for (note, text) in files {
        if state.note(*note).is_none() && !state.trash_by_id.contains_key(note) {
            continue;
        }
        let version = fsio::version_of(text.as_bytes());
        threads::put(tx, *note, text, &version).await?;
        now.push((*note, version));
    }
    let mut changes: Vec<(String, ChangeOp, Option<String>)> = Vec::new();
    for (note, v) in &now {
        if !before.iter().any(|(n, bv)| n == note && bv == v) {
            changes.push((note.to_string(), ChangeOp::Upsert, Some(v.clone())));
        }
    }
    for (note, _) in before {
        if !now.iter().any(|(n, _)| n == note) {
            changes.push((note.to_string(), ChangeOp::Delete, None));
        }
    }
    changes.sort();
    for (id, op, version) in &changes {
        sync_repo::append_change(
            tx,
            &NewChange {
                entity_type: "thread",
                entity_id: id,
                op: *op,
                version: version.as_deref(),
                at,
            },
        )
        .await?;
    }
    Ok(changes.len())
}

/// The job name in the commit message.
const THREAD_JOB: &str = "thread";

impl Core {
    /// See [`VaultService::append_thread`].
    pub async fn append_thread(
        &mut self,
        scope: UserScope,
        note: NoteId,
        question: ThreadMessage,
        answer: ThreadMessage,
    ) -> Result<NoteThread> {
        let path = NoteThread::path_for(note.as_ulid());
        if self.state()?.note(note).is_none() {
            return Err(VaultError::NotFound);
        }
        let current = self.read_text(&path).await?;
        let mut thread = current
            .as_deref()
            .and_then(|t| NoteThread::from_json(t).ok())
            .unwrap_or_else(|| NoteThread::new(note.as_ulid()));
        thread.push_exchange(question, answer);
        let json = thread
            .to_json()
            .map_err(|e| VaultError::Internal(format!("thread encoding: {e}")))?;
        let version = fsio::version_of(json.as_bytes());
        let now = self.now();
        let (text, v) = (json.clone(), version.clone());
        self.write_file(
            scope,
            FileWrite {
                path,
                content: Some(json.into_bytes()),
                expect: Expect::Any,
                author: Author::Ai(THREAD_JOB.to_owned()),
                op: THREAD_JOB.to_owned(),
                index: Some(Box::new(move |tx| {
                    Box::pin(async move {
                        if threads::put(tx, note, &text, &v).await? {
                            sync_repo::append_change(
                                tx,
                                &NewChange {
                                    entity_type: "thread",
                                    entity_id: &note.to_string(),
                                    op: ChangeOp::Upsert,
                                    version: Some(&v),
                                    at: now,
                                },
                            )
                            .await?;
                        }
                        Ok(())
                    })
                })),
            },
        )
        .await?;
        Ok(thread)
    }
}

impl VaultService {
    /// Appends a question and its answer to the thread about `note` (created on the first
    /// exchange), in one `ai: thread .meta/threads/<id>.json` commit. `404` when the note is
    /// not live. Re-appending messages already in the thread adds nothing.
    pub async fn append_thread(
        &self,
        scope: &UserScope,
        note: NoteId,
        question: ThreadMessage,
        answer: ThreadMessage,
    ) -> Result<NoteThread> {
        self.exec(scope, move |core, s| {
            Box::pin(core.append_thread(s, note, question, answer))
        })
        .await
    }

    /// The thread about `note` as stored (`None` when there is none yet).
    pub async fn note_thread(&self, scope: &UserScope, note: NoteId) -> Result<Option<NoteThread>> {
        self.ready(scope).await?;
        let dir = self.vault_dir(scope.user_id());
        let rel = NoteThread::path_for(note.as_ulid());
        let bytes = crate::store::blocking(move || Ok(fsio::read(&dir, &rel)?)).await?;
        Ok(bytes.and_then(|b| NoteThread::from_json(&String::from_utf8_lossy(&b)).ok()))
    }
}
