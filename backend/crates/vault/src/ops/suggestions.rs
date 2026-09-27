//! Suggestions (PLAN §7.5 Suggestions, §9.8): list, accept, reject and threaded replies.
//!
//! Manual kinds exist now; AI kinds (filing, relations, entity links, custody, tasks) arrive
//! with Phase 4 and reuse these endpoints.
//!
//! - `duplicate` (a capture that resembles existing items, §9.7): accepting means "keep
//!   both" — the pairs are recorded (sidecar + `dedupe_keep_both`) and never flagged again;
//!   rejecting dismisses the flag (the user may then delete the capture).

use strata_common::{NoteId, ReplyId, SuggestionId};
use strata_index::UserScope;
use strata_index::repo::suggestions::{self as srepo, Reply, Suggestion};
use strata_index::repo::sync::{self, NewChange};
use strata_index::types::{ChangeOp, ReplyAuthor, SuggestionStatus};

use crate::error::{Result, VaultError};
use crate::ops::notes::DuplicatePayload;
use crate::store::{Author, VaultService};

/// A suggestion with its thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionView {
    /// The row.
    pub suggestion: Suggestion,
    /// Replies in order.
    pub replies: Vec<Reply>,
}

/// The decoded payload of a suggestion.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    /// `duplicate`.
    Duplicate(DuplicatePayload),
    /// Any other kind (opaque `MessagePack`).
    Opaque(Vec<u8>),
}

impl SuggestionView {
    /// The payload, decoded when the kind is known.
    pub fn payload(&self) -> Payload {
        match self.suggestion.kind.as_str() {
            "duplicate" => rmp_serde::from_slice(&self.suggestion.payload).map_or_else(
                |_| Payload::Opaque(self.suggestion.payload.clone()),
                Payload::Duplicate,
            ),
            _ => Payload::Opaque(self.suggestion.payload.clone()),
        }
    }
}

impl VaultService {
    async fn view_suggestion(
        &self,
        tx: &mut strata_index::ScopedTx,
        s: Suggestion,
    ) -> Result<SuggestionView> {
        let replies = srepo::replies(tx, s.id).await?;
        Ok(SuggestionView {
            suggestion: s,
            replies,
        })
    }

    /// Suggestions with `status`, oldest first.
    pub async fn suggestions(
        &self,
        scope: &UserScope,
        status: SuggestionStatus,
    ) -> Result<Vec<SuggestionView>> {
        self.ready(scope).await?;
        let mut tx = self.inner.db.begin(scope).await?;
        let rows = srepo::list_suggestions(&mut tx, status).await?;
        let mut out = Vec::with_capacity(rows.len());
        for s in rows {
            out.push(self.view_suggestion(&mut tx, s).await?);
        }
        tx.commit().await?;
        Ok(out)
    }

    /// Accepts (`true`) or rejects a pending suggestion, applying its effect.
    pub async fn decide_suggestion(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        accept: bool,
    ) -> Result<SuggestionView> {
        self.exec(scope, move |core, scope| {
            Box::pin(async move {
                let mut tx = core.begin(&scope).await?;
                let s = srepo::get_suggestion(&mut tx, id)
                    .await?
                    .ok_or(VaultError::NotFound)?;
                tx.commit().await?;
                if s.status != SuggestionStatus::Pending {
                    return Err(VaultError::invalid("the suggestion was already decided"));
                }
                if accept
                    && s.kind == "duplicate"
                    && let Some(note) = s.note_id
                    && let Ok(p) = rmp_serde::from_slice::<DuplicatePayload>(&s.payload)
                {
                    let candidates: Vec<_> = p
                        .candidates
                        .iter()
                        .filter_map(super::notes::DuplicatePayloadItem::candidate)
                        .collect();
                    if core.state()?.note(note).is_some() && !candidates.is_empty() {
                        core.keep_both_notes(scope, note, &candidates, Author::User)
                            .await?;
                    }
                }
                let now = core.now();
                let mut tx = core.begin(&scope).await?;
                let status = if accept {
                    SuggestionStatus::Accepted
                } else {
                    SuggestionStatus::Rejected
                };
                let decided = srepo::decide_suggestion(&mut tx, id, status, now)
                    .await?
                    .ok_or_else(|| VaultError::invalid("the suggestion was already decided"))?;
                let id_text = id.to_string();
                sync::append_change(
                    &mut tx,
                    &NewChange {
                        entity_type: "suggestion",
                        entity_id: &id_text,
                        op: ChangeOp::Upsert,
                        version: None,
                        at: now,
                    },
                )
                .await?;
                let replies = srepo::replies(&mut tx, id).await?;
                tx.commit().await?;
                Ok(SuggestionView {
                    suggestion: decided,
                    replies,
                })
            })
        })
        .await
    }

    /// Adds a user reply to a pending suggestion's thread (the AI re-proposes in Phase 4).
    pub async fn reply_suggestion(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        body: String,
    ) -> Result<SuggestionView> {
        let text = body.trim().to_owned();
        if text.is_empty() {
            return Err(VaultError::invalid("the reply is empty"));
        }
        self.ready(scope).await?;
        let now = self.inner.clock.now();
        let mut tx = self.inner.db.begin(scope).await?;
        let s = srepo::get_suggestion(&mut tx, id)
            .await?
            .ok_or(VaultError::NotFound)?;
        if s.status != SuggestionStatus::Pending {
            return Err(VaultError::invalid("the suggestion was already decided"));
        }
        srepo::add_reply(
            &mut tx,
            &Reply {
                id: ReplyId::generate(self.inner.ids.as_ref()),
                suggestion_id: id,
                author: ReplyAuthor::User,
                body: text,
                created: now,
            },
        )
        .await?;
        let id_text = id.to_string();
        sync::append_change(
            &mut tx,
            &NewChange {
                entity_type: "suggestion",
                entity_id: &id_text,
                op: ChangeOp::Upsert,
                version: None,
                at: now,
            },
        )
        .await?;
        let view = self.view_suggestion(&mut tx, s).await?;
        tx.commit().await?;
        Ok(view)
    }

    /// Pending suggestions of a note.
    pub async fn note_suggestions(
        &self,
        scope: &UserScope,
        note: NoteId,
    ) -> Result<Vec<SuggestionView>> {
        Ok(self
            .suggestions(scope, SuggestionStatus::Pending)
            .await?
            .into_iter()
            .filter(|s| s.suggestion.note_id == Some(note))
            .collect())
    }
}
