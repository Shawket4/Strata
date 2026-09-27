//! View-model streams (one per screen, PLAN §11) and one-shot reads.

use super::runtime::core;
use super::sink::DartSink;
use super::{lift, lift_async};
use crate::error::CoreError;
use crate::frb_generated::StreamSink;
use crate::graph;
use crate::net::NetError;
use crate::view::build;
use crate::view::model::CoreFailure;
use crate::view::model::{
    AdminUsersView, AskView, Availability, ConflictScreen, DirectoryTab, DirectoryView,
    DuplicatePromptsView, EditorHint, EntityScreen, GlobalGraphView, HomeView, InboxView,
    LocalGraphView, NoteScreen, NotesListView, SearchMode, SearchView, SettingsView,
    SyncStatusView, TaskScreen, TasksView,
};
use crate::view::{Topics, ViewSink};

fn watch<V, B>(topics: Topics, build: B, sink: impl ViewSink<V> + 'static) -> Result<(), CoreError>
where
    V: PartialEq + Clone + Send + 'static,
    B: Fn(&rusqlite::Connection, &crate::view::ViewCtx) -> crate::error::CoreResult<V>
        + Send
        + 'static,
{
    core()?.session()?.watch(topics, build, sink).map(|_| ())
}

/// Home: recent notes, inbox count, task sections, sync pill.
pub fn watch_home(sink: StreamSink<HomeView>) -> Result<(), CoreFailure> {
    lift(|| watch(Topics::ALL, build::home, DartSink(sink)))
}

/// Inbox: captures with suggestions, other suggestions.
pub fn watch_inbox(sink: StreamSink<InboxView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::INBOX | Topics::SUGGESTIONS | Topics::NOTES | Topics::SYNC,
            |c, _| build::inbox(c),
            DartSink(sink),
        )
    })
}

/// A note (editor, properties, relations, backlinks, tasks).
pub fn watch_note(id: String, sink: StreamSink<NoteScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::ALL,
            move |c, ctx| build::note_screen(c, ctx, &id),
            DartSink(sink),
        )
    })
}

/// A folder of the notes tree (`""` = vault root).
pub fn watch_notes_list(
    folder: String,
    sink: StreamSink<NotesListView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::NOTES | Topics::SYNC,
            move |c, _| build::notes_list(c, &folder),
            DartSink(sink),
        )
    })
}

/// A directory tab, filtered by `query`.
pub fn watch_directory(
    tab: DirectoryTab,
    query: String,
    sink: StreamSink<DirectoryView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::ENTITIES | Topics::NOTES,
            move |c, _| build::directory(c, tab, &query),
            DartSink(sink),
        )
    })
}

/// A person, company, document or place page.
pub fn watch_entity(id: String, sink: StreamSink<EntityScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::ALL,
            move |c, _| build::entity_screen(c, &id),
            DartSink(sink),
        )
    })
}

/// The Tasks destination.
pub fn watch_tasks(sink: StreamSink<TasksView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::TASKS | Topics::NOTES | Topics::SYNC | Topics::TIME | Topics::ACCOUNT,
            build::tasks_view,
            DartSink(sink),
        )
    })
}

/// Task detail.
pub fn watch_task(id: String, sink: StreamSink<TaskScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::TASKS | Topics::NOTES | Topics::SYNC,
            move |c, ctx| build::task_screen(c, ctx, &id),
            DartSink(sink),
        )
    })
}

/// Sync status and conflicts.
pub fn watch_sync_status(sink: StreamSink<SyncStatusView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC | Topics::NOTES,
            build::sync_status,
            DartSink(sink),
        )
    })
}

/// One conflict (side by side, D19).
pub fn watch_conflict(op_id: String, sink: StreamSink<ConflictScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC | Topics::NOTES,
            move |c, _| build::conflict_screen(c, &op_id),
            DartSink(sink),
        )
    })
}

/// Open "Already exists" prompts.
pub fn watch_duplicate_prompts(sink: StreamSink<DuplicatePromptsView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC,
            |c, _| build::duplicate_prompts(c),
            DartSink(sink),
        )
    })
}

/// Settings.
pub fn watch_settings(sink: StreamSink<SettingsView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SETTINGS | Topics::ACCOUNT | Topics::SYNC,
            |c, ctx| build::settings_view(c, ctx)?.ok_or(CoreError::NotSignedIn),
            DartSink(sink),
        )
    })
}

/// A note's neighbourhood laid out radially (local mind map).
pub fn watch_local_graph(
    id: String,
    depth: u8,
    sink: StreamSink<LocalGraphView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::NOTES | Topics::ENTITIES,
            move |c, _| graph::local_graph(c, &id, depth),
            DartSink(sink),
        )
    })
}

/// The global map (positions from the cached, warm-started force layout).
pub fn global_graph() -> Result<GlobalGraphView, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .write(|c, _| Ok((graph::global_graph(c)?, Topics::NONE)))
    })
}

/// Local search.
pub fn search(query: String, mode: SearchMode) -> Result<SearchView, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, ctx| crate::search::search(c, ctx, &query, mode))
    })
}

/// Ask (online only; the `/ask` stream is not in the contract yet).
pub fn ask_view() -> Result<AskView, CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        Ok(build::ask(&session.ctx()))
    })
}

/// Editor highlight spans for text being typed (UTF-16 offsets).
pub fn editor_hints(content: String) -> Vec<EditorHint> {
    build::hints_of(&content)
}

/// Admin → Users (online, admins only).
pub async fn load_admin_users() -> Result<AdminUsersView, CoreFailure> {
    lift_async(async {
        let core = core()?;
        let session = core.session()?;
        let is_admin =
            session.read(|c, _| Ok(build::account_summary(c)?.is_some_and(|a| a.is_admin)))?;
        if !is_admin {
            return Ok(AdminUsersView {
                availability: Availability::NotAllowed,
                pending: Vec::new(),
                users: Vec::new(),
            });
        }
        let url = session.server_url()?;
        match core
            .env()
            .account_api
            .admin_users(url, session.tokens())
            .await
        {
            Ok(users) => Ok(build::admin_users(users)),
            Err(NetError::Offline(_)) => Ok(AdminUsersView {
                availability: Availability::Offline,
                pending: Vec::new(),
                users: Vec::new(),
            }),
            Err(e) => Err(e.into()),
        }
    })
    .await
}
