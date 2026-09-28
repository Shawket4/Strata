//! Suggestions (PLAN §7.5 Suggestions, §9.8): list, accept, reject and threaded replies.
//!
//! Manual kinds exist now; AI kinds (filing, relations, entity links, custody, tasks) arrive
//! with Phase 4 and reuse these endpoints.
//!
//! - `duplicates` (a pair found by the nightly sweep, §9.7): rejecting records keep-both for
//!   the pair; accepting only records the decision (merging is the user's action).
//! - `duplicate` (a capture that resembles existing items, §9.7): accepting means "keep
//!   both" — the pairs are recorded (sidecar + `dedupe_keep_both`) and never flagged again;
//!   rejecting dismisses the flag (the user may then delete the capture).

use strata_common::{NoteId, ReplyId, SuggestionId};
use strata_index::UserScope;
use strata_index::repo::suggestions::{self as srepo, Reply, Suggestion};
use strata_index::repo::sync::{self, NewChange};
use strata_index::types::{ChangeOp, ReplyAuthor, SuggestionStatus};
use sync_model::suggestions::{
    DuplicatePayload, DuplicatesPayload, PayloadError, SuggestionPayload, kinds,
};

use crate::error::{Result, VaultError};
use crate::receipt::AfterWrite;
use crate::store::{Author, VaultService};

/// A suggestion with its thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionView {
    /// The row.
    pub suggestion: Suggestion,
    /// Replies in order.
    pub replies: Vec<Reply>,
}

impl SuggestionView {
    /// The payload, decoded by the suggestion's kind (`sync_model::suggestions`); an unknown
    /// kind or undecodable bytes are an error the caller shows as opaque.
    pub fn payload(&self) -> std::result::Result<SuggestionPayload, PayloadError> {
        SuggestionPayload::decode(&self.suggestion.kind, &self.suggestion.payload)
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

    /// Accepts (`true`) or rejects a pending suggestion, applying its effect, now (an online
    /// decision: the server's clock is the time of the decision).
    pub async fn decide_suggestion(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        accept: bool,
    ) -> Result<SuggestionView> {
        self.decide_suggestion_at(scope, id, accept, None).await
    }

    /// [`Self::decide_suggestion`] made on a device at `at` (UTC; `None` = now): notes an
    /// acceptance creates get `at` as `created`/`updated`.
    pub async fn decide_suggestion_at(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        accept: bool,
        at: Option<chrono::DateTime<chrono::Utc>>,
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
                // AI suggestions (filing, entity links, custody, tasks, corrections, accepted
                // duplicates) are decided by the AI pipelines' rules.
                if crate::ops::ai_decide::handles(&s.kind, accept) {
                    return core.decide_ai(scope, s, accept, None, at).await;
                }
                // A pushed op's result is stored with the decision below, not with the
                // keep-both writes before it (they are idempotent if the op is replayed).
                let receipt = core.receipt.take();
                if accept
                    && s.kind == kinds::DUPLICATE
                    && let Some(note) = s.note_id
                    && let Ok(p) = rmp_serde::from_slice::<DuplicatePayload>(&s.payload)
                {
                    let candidates: Vec<_> = p
                        .candidates
                        .iter()
                        .map(super::notes::item_candidate)
                        .collect();
                    if core.state()?.note(note).is_some() && !candidates.is_empty() {
                        core.keep_both_notes(scope, note, &candidates, Author::User)
                            .await?;
                    }
                }
                // Rejecting "these are duplicates" means they are distinct: never again.
                if !accept
                    && s.kind == kinds::DUPLICATES
                    && let Some(note) = s.note_id
                    && let Ok(p) = rmp_serde::from_slice::<DuplicatesPayload>(&s.payload)
                {
                    core.reject_duplicates(scope, note, &p).await?;
                }
                core.receipt = receipt;
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
                let pending = crate::receipt::settle(
                    core.receipt.as_ref(),
                    &mut tx,
                    &AfterWrite::database_only(),
                    now,
                )
                .await?;
                let replies = srepo::replies(&mut tx, id).await?;
                tx.commit().await?;
                if let Some(p) = pending {
                    p.commit();
                }
                core.inner
                    .notify(core.user, &suggestion_notice(&decided, false));
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
        let reply = ReplyId::generate(self.inner.ids.as_ref());
        self.reply_suggestion_as(scope, id, reply, body).await
    }

    /// [`Self::reply_suggestion`] with a client-generated reply ID (sync). A reply whose ID
    /// is already in the thread is not added again.
    pub async fn reply_suggestion_as(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        reply_id: ReplyId,
        body: String,
    ) -> Result<SuggestionView> {
        let text = body.trim().to_owned();
        if text.is_empty() {
            return Err(VaultError::invalid("the reply is empty"));
        }
        if text.contains('\0') {
            // PostgreSQL text cannot store NUL.
            return Err(VaultError::invalid("text must not contain NUL characters"));
        }
        self.ready(scope).await?;
        let now = self.inner.clock.now();
        let mut tx = self.inner.db.begin(scope).await?;
        let s = srepo::get_suggestion(&mut tx, id)
            .await?
            .ok_or(VaultError::NotFound)?;
        if srepo::replies(&mut tx, id)
            .await?
            .iter()
            .any(|r| r.id == reply_id)
        {
            let view = self.view_suggestion(&mut tx, s).await?;
            tx.commit().await?;
            return Ok(view);
        }
        if s.status != SuggestionStatus::Pending {
            return Err(VaultError::invalid("the suggestion was already decided"));
        }
        srepo::add_reply(
            &mut tx,
            &Reply {
                id: reply_id,
                suggestion_id: id,
                author: ReplyAuthor::User,
                body: text,
                created: now,
            },
        )
        .await?;
        // The AI re-proposes with the reply in context (§9.8 threaded suggestions).
        Self::enqueue_reply_job(&mut tx, self.inner.ids.as_ref(), &s, now).await?;
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
        let pending = crate::receipt::settle(
            crate::receipt::current().as_ref(),
            &mut tx,
            &AfterWrite::database_only(),
            now,
        )
        .await?;
        let view = self.view_suggestion(&mut tx, s).await?;
        tx.commit().await?;
        if let Some(p) = pending {
            p.commit();
        }
        self.inner
            .notify(scope.user_id(), &suggestion_notice(&view.suggestion, false));
        Ok(view)
    }

    /// Records a new pending suggestion (with its change-log row) and announces it. An armed
    /// op receipt in scope is settled in the same transaction.
    pub async fn create_suggestion(
        &self,
        scope: &UserScope,
        id: SuggestionId,
        note: Option<NoteId>,
        kind: &str,
        payload: &[u8],
    ) -> Result<Suggestion> {
        let now = self.inner.clock.now();
        let mut tx = self.inner.db.begin(scope).await?;
        let s = srepo::create_suggestion(&mut tx, id, note, kind, payload, now).await?;
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
        let pending = crate::receipt::settle(
            crate::receipt::current().as_ref(),
            &mut tx,
            &AfterWrite::database_only(),
            now,
        )
        .await?;
        tx.commit().await?;
        if let Some(p) = pending {
            p.commit();
        }
        self.inner
            .notify(scope.user_id(), &suggestion_notice(&s, true));
        Ok(s)
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

fn suggestion_notice(s: &Suggestion, created: bool) -> crate::events::Committed {
    crate::events::Committed {
        suggestions: vec![crate::events::SuggestionEvent {
            id: s.id,
            note_id: s.note_id,
            kind: s.kind.clone(),
            status: s.status.as_str().to_owned(),
            created,
        }],
        ..crate::events::Committed::default()
    }
}
