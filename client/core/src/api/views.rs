//! View-model streams (one per screen, PLAN §11) and one-shot reads.

use super::runtime::core;
use super::sink::DartSink;
use super::{lift, lift_async};
use crate::error::CoreError;
use crate::frb_generated::StreamSink;
use crate::graph;
use crate::view::build;
use crate::view::model::CoreFailure;
use chrono::NaiveDate;

use crate::view::model::{
    AdminUsersView, AskView, BlockItem, CitationPreview, Completions, ConflictScreen,
    DirectoryFilter, DirectorySort, DirectoryTab, DirectoryView, DuplicatePromptsView, EditorHint,
    EntityScreen, GlobalGraphView, GraphFilter, HomeView, InboxFilter, InboxView, LocalGraphView,
    MergePreview, NavView, NoteDiffView, NoteScreen, NotesListView, PlaceOption, RecentFilter,
    RecentNotesView, RecurrenceCompose, RecurrenceForm, RecurrencePreviewItem, RelationTypeItem,
    RepointChoice, SearchMode, SearchView, SettingsView, SyncStatusView, TagItem, TaskDraftPreview,
    TaskHomesView, TaskScreen, TasksView, TimeZoneItem,
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
    watch_inbox_filtered(InboxFilter::All, sink)
}

/// Inbox with a filter tab (All / Needs you / Conflicts).
pub fn watch_inbox_filtered(
    filter: InboxFilter,
    sink: StreamSink<InboxView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::INBOX | Topics::SUGGESTIONS | Topics::NOTES | Topics::SYNC | Topics::ACCOUNT,
            move |c, ctx| build::inbox(c, ctx, filter),
            DartSink(sink),
        )
    })
}

/// Navigation counts and pinned notes (sidebar, rail, bottom bar).
pub fn watch_nav(sink: StreamSink<NavView>) -> Result<(), CoreFailure> {
    lift(|| watch(Topics::ALL, build::nav, DartSink(sink)))
}

/// The "Recent" block with a filter (Edited / Created / Filed by AI).
pub fn watch_recent(
    filter: RecentFilter,
    sink: StreamSink<RecentNotesView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::NOTES | Topics::SUGGESTIONS | Topics::SYNC | Topics::ACCOUNT | Topics::TIME,
            move |c, ctx| build::recent(c, ctx, filter),
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
            Topics::NOTES | Topics::SYNC | Topics::ACCOUNT,
            move |c, ctx| build::notes_list(c, ctx, &folder),
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
            Topics::ENTITIES | Topics::NOTES | Topics::SUGGESTIONS | Topics::ACCOUNT,
            move |c, ctx| build::directory(c, ctx, tab, &query),
            DartSink(sink),
        )
    })
}

/// A directory tab with filters and an order.
pub fn watch_directory_filtered(
    tab: DirectoryTab,
    query: String,
    filter: DirectoryFilter,
    sort: DirectorySort,
    sink: StreamSink<DirectoryView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::ENTITIES | Topics::NOTES | Topics::SUGGESTIONS | Topics::ACCOUNT | Topics::TIME,
            move |c, ctx| build::directory_filtered(c, ctx, tab, &query, &filter, sort),
            DartSink(sink),
        )
    })
}

/// A person, company, document or place page.
pub fn watch_entity(id: String, sink: StreamSink<EntityScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::ALL,
            move |c, ctx| build::entity_screen(c, ctx, &id),
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
            Topics::TASKS
                | Topics::NOTES
                | Topics::SYNC
                | Topics::TIME
                | Topics::ACCOUNT
                | Topics::REMOTE,
            move |c, ctx| build::task_screen(c, ctx, &id),
            DartSink(sink),
        )
    })
}

/// Candidate homes of a new task.
pub fn watch_task_homes(sink: StreamSink<TaskHomesView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::TASKS | Topics::NOTES | Topics::ACCOUNT,
            crate::view::extra::task_homes,
            DartSink(sink),
        )
    })
}

/// Sync status and conflicts.
pub fn watch_sync_status(sink: StreamSink<SyncStatusView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC | Topics::NOTES | Topics::ACCOUNT | Topics::TIME,
            build::sync_status,
            DartSink(sink),
        )
    })
}

/// One conflict (side by side, D19).
pub fn watch_conflict(op_id: String, sink: StreamSink<ConflictScreen>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC | Topics::NOTES | Topics::ACCOUNT,
            move |c, ctx| build::conflict_screen(c, ctx, &op_id),
            DartSink(sink),
        )
    })
}

/// Open "Already exists" prompts.
pub fn watch_duplicate_prompts(sink: StreamSink<DuplicatePromptsView>) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::SYNC | Topics::NOTES | Topics::ACCOUNT,
            build::duplicate_prompts,
            DartSink(sink),
        )
    })
}

/// Settings.
pub fn watch_settings(sink: StreamSink<SettingsView>) -> Result<(), CoreFailure> {
    lift(|| {
        let server_url = core()?.env().server_url.clone();
        watch(
            Topics::SETTINGS | Topics::ACCOUNT | Topics::SYNC | Topics::REMOTE,
            move |c, ctx| build::settings_view(c, ctx, &server_url)?.ok_or(CoreError::NotSignedIn),
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
            Topics::NOTES | Topics::ENTITIES | Topics::ACCOUNT,
            move |c, ctx| graph::local_graph(c, ctx, &id, depth),
            DartSink(sink),
        )
    })
}

/// A note's local mind map with only the edges of `edge_kinds` (families such as `link`,
/// or full kinds such as `relation:supports`; empty = all), filtered in the core.
pub fn watch_local_graph_filtered(
    id: String,
    depth: u8,
    edge_kinds: Vec<String>,
    sink: StreamSink<LocalGraphView>,
) -> Result<(), CoreFailure> {
    lift(|| {
        watch(
            Topics::NOTES | Topics::ENTITIES | Topics::ACCOUNT,
            move |c, ctx| {
                let mut view = graph::local_graph(c, ctx, &id, depth)?;
                graph::retain_edge_kinds(&mut view.edges, &edge_kinds);
                Ok(view)
            },
            DartSink(sink),
        )
    })
}

/// The global map (positions from the cached, warm-started force layout).
pub fn global_graph() -> Result<GlobalGraphView, CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        let ctx = session.ctx();
        session.write(|c, _| Ok((graph::global_graph(c, &ctx)?, Topics::NONE)))
    })
}

/// The global map with filters and a lens applied in the core.
pub fn global_graph_filtered(filter: GraphFilter) -> Result<GlobalGraphView, CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        let ctx = session.ctx();
        session.write(|c, _| {
            Ok((
                graph::global_graph_filtered(c, &ctx, &filter)?,
                Topics::NONE,
            ))
        })
    })
}

/// Search: keyword locally (offline too); semantic and hybrid on the server.
pub async fn search(query: String, mode: SearchMode) -> Result<SearchView, CoreFailure> {
    search_in_folder(query, mode, None).await
}

/// Search limited to a folder (and its subfolders).
pub async fn search_in_folder(
    query: String,
    mode: SearchMode,
    folder: Option<String>,
) -> Result<SearchView, CoreFailure> {
    lift_async(async {
        let session = core()?.session()?;
        let offline = session.ctx().connectivity == crate::view::model::Connectivity::Offline;
        if mode == SearchMode::Keyword || offline {
            return session
                .read(|c, ctx| crate::search::search(c, ctx, &query, mode, folder.as_deref()));
        }
        session.search_remote(&query, mode, folder).await
    })
    .await
}

/// Ask (one-shot read of the conversation).
pub fn ask_view() -> Result<AskView, CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        let entries = session.ask_entries();
        session.read(|c, ctx| build::ask(c, ctx, &entries))
    })
}

/// Ask: the conversation as it streams.
pub fn watch_ask(sink: StreamSink<AskView>) -> Result<(), CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        let weak = std::sync::Arc::downgrade(&session);
        session
            .watch(
                Topics::ASK | Topics::SYNC | Topics::REMOTE | Topics::ENTITIES | Topics::ACCOUNT,
                move |c, ctx| {
                    let entries = weak.upgrade().map(|s| s.ask_entries()).unwrap_or_default();
                    build::ask(c, ctx, &entries)
                },
                DartSink(sink),
            )
            .map(|_| ())
    })
}

/// Editor highlight spans for text being typed (UTF-16 offsets); with a signed-in session,
/// wikilinks are resolved to note IDs as seen from the note at `path` (`""` = vault root).
pub fn editor_hints(content: String) -> Vec<EditorHint> {
    match core().and_then(|c| c.session()) {
        Ok(s) => s
            .read(|c, _| build::hints_resolved(c, "", &content))
            .unwrap_or_else(|_| build::hints_of(&content)),
        Err(_) => build::hints_of(&content),
    }
}

/// Editor completions at the caret (`[[`, `[[Note#^`, `@`, `#`), UTF-16 `cursor`.
pub fn editor_completions(
    note_id: String,
    content: String,
    cursor: u32,
) -> Result<Completions, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, ctx| crate::view::extra::completions(c, ctx, &note_id, &content, cursor))
    })
}

/// Vault tags with note counts, filtered by prefix.
pub fn tags(prefix: String) -> Result<Vec<TagItem>, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, _| crate::view::extra::tags(c, &prefix))
    })
}

/// The blocks of a note (block reference picker).
pub fn note_blocks(note_id: String) -> Result<Vec<BlockItem>, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, _| crate::view::extra::blocks(c, &note_id))
    })
}

/// Relation types the user can pick, with labels in the UI language.
pub fn relation_types() -> Result<Vec<RelationTypeItem>, CoreFailure> {
    lift(|| {
        let lang = core()?.session()?.ctx().lang;
        Ok(vault_format::RelationKey::all()
            .map(|k| RelationTypeItem {
                key: k.as_str().to_owned(),
                label: crate::format::labels::relation_label(k.as_str(), lang),
            })
            .collect())
    })
}

/// A recurrence phrase as the editor's form (`None`: outside the grammar).
pub fn recurrence_form(phrase: String) -> Option<RecurrenceForm> {
    crate::format::recurrence::form_of_phrase(&phrase)
}

/// Compiles the editor's form to its phrase and summary.
pub fn compose_recurrence(form: RecurrenceForm) -> Result<RecurrenceCompose, CoreFailure> {
    lift(|| {
        let labels = labels_now()?;
        crate::format::recurrence::compose(&form, &labels)
    })
}

/// The first `count` occurrences of `phrase` from `from` (the due date).
pub fn recurrence_preview(
    phrase: String,
    from: NaiveDate,
    count: u32,
) -> Result<Vec<RecurrencePreviewItem>, CoreFailure> {
    lift(|| {
        let rule = vault_format::tasks::parse_recurrence(&phrase)
            .map_err(|_| CoreError::invalid("recurrence", "not_understood"))?;
        Ok(crate::format::recurrence::preview(
            &rule,
            from,
            count.min(24),
            &labels_now()?,
        ))
    })
}

fn labels_now() -> Result<crate::format::labels::Labels, CoreError> {
    Ok(core()?.session()?.ctx().labels())
}

/// A new task's text as the core understands it ("Understood as" chips).
pub fn parse_task_text(text: String) -> Result<TaskDraftPreview, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, ctx| crate::view::extra::task_draft_preview(c, ctx, &text))
    })
}

/// Places for the custody picker (`document_id` marks the document's current place).
pub fn place_options(document_id: Option<String>) -> Result<Vec<PlaceOption>, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, _| crate::view::extra::place_options(c, document_id.as_deref()))
    })
}

/// What merging `source_id` into `into_id` moves.
pub fn merge_preview(source_id: String, into_id: String) -> Result<MergePreview, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, _| crate::view::extra::merge_preview(c, &source_id, &into_id))
    })
}

/// The block a citation points to (source preview).
pub fn resolve_citation(
    note_id: String,
    anchor: Option<String>,
) -> Result<CitationPreview, CoreFailure> {
    lift(|| {
        core()?.session()?.read(|c, ctx| {
            crate::view::extra::citation_preview(c, ctx, &note_id, anchor.as_deref())
        })
    })
}

/// A revision compared with the note's current content (online).
pub async fn note_revision_diff(
    note_id: String,
    commit: String,
) -> Result<NoteDiffView, CoreFailure> {
    lift_async(async {
        core()?
            .session()?
            .note_revision_diff(&note_id, &commit)
            .await
    })
    .await
}

/// The time zones of the Settings picker matching `query` (IANA ID, name or region in either
/// language, offset label), with names in the UI language and current offsets, sorted by
/// offset; `set_timezone(item.id)` applies one.
pub fn timezones(query: String) -> Result<Vec<TimeZoneItem>, CoreFailure> {
    lift(|| {
        let ctx = core()?.session()?.ctx();
        Ok(crate::format::timezones::timezones(
            ctx.now, ctx.tz, ctx.lang, &query,
        ))
    })
}

/// New targets for an AI decision of Home's activity feed (Repoint), matching `query`;
/// `repoint_ai_decision(decision_id, choice.id, None)` applies one.
pub fn repoint_choices(
    decision_id: String,
    query: String,
) -> Result<Vec<RepointChoice>, CoreFailure> {
    lift(|| {
        core()?
            .session()?
            .read(|c, _| crate::view::extra::repoint_choices(c, &decision_id, &query))
    })
}

/// Admin → Users (online, admins only), filtered by `query`.
pub async fn load_admin_users(query: String) -> Result<AdminUsersView, CoreFailure> {
    lift_async(async { core()?.session()?.admin_users(&query).await }).await
}
