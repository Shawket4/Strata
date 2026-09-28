//! [`VaultService`] entry points for writes: each runs on the user's actor.

use strata_common::NoteId;
use strata_index::UserScope;
use vault_format::RelationKey;

use crate::error::{Candidate, Result};
use crate::model::{Captured, NoteView};
use crate::ops::entities::{EntityPatch, NewCustodyEvent, NewEntity};
use crate::ops::notes::CreateNote;
use crate::ops::relations::AiEdge;
use crate::ops::tasks::{NewTask, TaskPatch, Transition};
use crate::reconcile::{self, Report};
use crate::store::{Author, VaultService};

macro_rules! on_actor {
    ($self:ident, $scope:ident, |$core:ident, $s:ident| $body:expr) => {
        $self
            .exec($scope, move |$core, $s| {
                Box::pin(async move { $body.await })
            })
            .await
    };
}

impl VaultService {
    /// `POST /notes`.
    pub async fn create_note(&self, scope: &UserScope, req: CreateNote) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.create_note(
            s,
            req,
            Author::User
        ))
    }

    /// `PUT /notes/{id}`.
    pub async fn update_note(
        &self,
        scope: &UserScope,
        id: NoteId,
        content: String,
        if_match: String,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.update_note(
            s,
            id,
            content,
            &if_match,
            Author::User
        ))
    }

    /// An `ai:` edit of a note's content (Phase 4 jobs; tests).
    pub async fn ai_update_note(
        &self,
        scope: &UserScope,
        job: String,
        id: NoteId,
        content: String,
        if_match: String,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.update_note(
            s,
            id,
            content,
            &if_match,
            Author::Ai(job)
        ))
    }

    /// `POST /notes/{id}/move`.
    pub async fn move_note(
        &self,
        scope: &UserScope,
        id: NoteId,
        new_path: String,
        if_match: Option<String>,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.move_note(
            s,
            id,
            new_path,
            if_match.as_deref(),
            Author::User
        ))
    }

    /// `DELETE /notes/{id}` (soft delete).
    pub async fn delete_note(&self, scope: &UserScope, id: NoteId) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.delete_note(s, id, Author::User))
    }

    /// `POST /trash/{id}/restore`.
    pub async fn restore_note(&self, scope: &UserScope, id: NoteId) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.restore_note(
            s,
            id,
            Author::User
        ))
    }

    /// `DELETE /trash/{id}` (purge).
    pub async fn purge_note(&self, scope: &UserScope, id: NoteId) -> Result<()> {
        on_actor!(self, scope, |core, s| core.purge_note(s, id, Author::User))
    }

    /// `POST /notes/{id}/revert`.
    pub async fn revert_note(
        &self,
        scope: &UserScope,
        id: NoteId,
        commit: String,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.revert_note(
            s,
            id,
            &commit,
            Author::User
        ))
    }

    /// `POST /commits/{commit}/revert`.
    pub async fn revert_commit(
        &self,
        scope: &UserScope,
        commit: String,
    ) -> Result<(Option<String>, Vec<String>)> {
        on_actor!(self, scope, |core, s| core.revert_commit(
            s,
            &commit,
            Author::User
        ))
    }

    /// Appends a block ID (citations; `ai: <job>` when a job needs it).
    pub async fn append_block_id(
        &self,
        scope: &UserScope,
        id: NoteId,
        block_start: usize,
        block_id: String,
        author: Author,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.append_block_id(
            s,
            id,
            block_start,
            &block_id,
            author
        ))
    }

    /// `POST /capture`: the device's capture time `created` names the inbox file.
    pub async fn capture(
        &self,
        scope: &UserScope,
        text: String,
        created: chrono::DateTime<chrono::Utc>,
    ) -> Result<Captured> {
        on_actor!(self, scope, |core, s| core
            .capture_as(s, text, None, created))
    }

    /// A capture pushed by a device: client-generated ID and capture time (sync).
    pub async fn capture_as(
        &self,
        scope: &UserScope,
        text: String,
        id: NoteId,
        created: chrono::DateTime<chrono::Utc>,
    ) -> Result<Captured> {
        on_actor!(self, scope, |core, s| core.capture_as(
            s,
            text,
            Some(id),
            created
        ))
    }

    /// A pushed task edit (`task.update|complete|cancel|reopen|delete`) with the device's
    /// dates and next-occurrence ID; `if_match` is the line version (sync).
    pub async fn apply_task_sync_op(
        &self,
        scope: &UserScope,
        op: sync_model::Op,
        if_match: Option<String>,
    ) -> Result<()> {
        on_actor!(self, scope, |core, s| core.apply_task_sync_op(
            s,
            &op,
            if_match.as_deref()
        ))
    }

    /// The task line of `id`: (note, line version, line), read on the writer.
    pub async fn task_line(
        &self,
        scope: &UserScope,
        id: String,
    ) -> Result<(NoteId, String, String)> {
        self.exec(scope, move |core, s| {
            Box::pin(async move { core.task_line(&s, &id).await })
        })
        .await
    }

    /// Records keep-both pairs for a note.
    pub async fn keep_both_notes(
        &self,
        scope: &UserScope,
        id: NoteId,
        others: Vec<Candidate>,
    ) -> Result<()> {
        on_actor!(self, scope, |core, s| core.keep_both_notes(
            s,
            id,
            &others,
            Author::User
        ))
    }

    /// `POST /relations`.
    pub async fn add_relation(
        &self,
        scope: &UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
    ) -> Result<bool> {
        on_actor!(self, scope, |core, s| core.add_relation(s, src, dst, rel))
    }

    /// `PATCH /relations`.
    pub async fn retype_relation(
        &self,
        scope: &UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
        new_rel: RelationKey,
    ) -> Result<()> {
        on_actor!(self, scope, |core, s| core
            .retype_relation(s, src, dst, rel, new_rel))
    }

    /// `DELETE /relations`. Returns whether the removed edge was by AI (and so rejected).
    pub async fn remove_relation(
        &self,
        scope: &UserScope,
        src: NoteId,
        dst: NoteId,
        rel: RelationKey,
    ) -> Result<bool> {
        on_actor!(self, scope, |core, s| core
            .remove_relation(s, src, dst, rel))
    }

    /// AI edges in one `ai: <job>` commit.
    pub async fn ai_add_relations(
        &self,
        scope: &UserScope,
        job: String,
        src: NoteId,
        edges: Vec<AiEdge>,
    ) -> Result<Option<String>> {
        on_actor!(self, scope, |core, s| core
            .ai_add_relations(s, &job, src, edges))
    }

    /// `POST /entities`, `/documents`, `/places`.
    pub async fn create_entity(&self, scope: &UserScope, req: NewEntity) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.create_entity(s, req))
    }

    /// A create with user relations to existing notes, in one commit (sync `document.create`
    /// with `copy_of`, `companies`, `people`).
    pub async fn create_entity_linked(
        &self,
        scope: &UserScope,
        req: NewEntity,
        links: Vec<(RelationKey, NoteId)>,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core
            .create_entity_linked(s, req, &links))
    }

    /// `PATCH /entities/{id}` (and documents/places).
    pub async fn patch_entity(
        &self,
        scope: &UserScope,
        id: NoteId,
        patch: EntityPatch,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core.patch_entity(s, id, patch))
    }

    /// `POST /entities/{id}/merge`.
    pub async fn merge_entities(
        &self,
        scope: &UserScope,
        loser: NoteId,
        survivor: NoteId,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core
            .merge_entities(s, loser, survivor))
    }

    /// `POST /documents/{id}/custody`.
    pub async fn add_custody_event(
        &self,
        scope: &UserScope,
        document: NoteId,
        ev: NewCustodyEvent,
    ) -> Result<NoteView> {
        on_actor!(self, scope, |core, s| core
            .add_custody_event(s, document, ev))
    }

    /// `POST /tasks`.
    pub async fn create_task(&self, scope: &UserScope, req: NewTask) -> Result<String> {
        on_actor!(self, scope, |core, s| core.create_task(s, req))
    }

    /// `PATCH /tasks/{id}`.
    pub async fn patch_task(
        &self,
        scope: &UserScope,
        id: String,
        patch: TaskPatch,
        if_match: Option<String>,
    ) -> Result<()> {
        on_actor!(self, scope, |core, s| core.patch_task(
            s,
            &id,
            patch,
            if_match.as_deref()
        ))
    }

    /// `POST /tasks/{id}/complete|cancel|reopen`.
    pub async fn transition_task(
        &self,
        scope: &UserScope,
        id: String,
        transition: Transition,
        if_match: Option<String>,
    ) -> Result<Option<String>> {
        on_actor!(self, scope, |core, s| core.transition_task(
            s,
            &id,
            transition,
            if_match.as_deref()
        ))
    }

    /// Reconciles the vault now (as on load: repairs, commits, warnings) and returns what
    /// was found.
    pub async fn reconcile_now(&self, scope: &UserScope) -> Result<Report> {
        self.exec_unloaded(scope, |core, s| {
            Box::pin(async move { reconcile::reconcile(core, s, false).await })
        })
        .await
    }

    /// `stratad verify --user`: what reconciliation would find and repair, changing nothing.
    pub async fn verify(&self, scope: &UserScope) -> Result<Report> {
        self.exec_unloaded(scope, |core, s| {
            Box::pin(async move { reconcile::check(core, s).await })
        })
        .await
    }

    /// `stratad reindex --user`: rebuilds every derived row of the user from the vault.
    pub async fn reindex(&self, scope: &UserScope) -> Result<usize> {
        self.exec_unloaded(scope, |core, s| {
            Box::pin(async move { reconcile::reindex(core, s).await })
        })
        .await
    }
}
