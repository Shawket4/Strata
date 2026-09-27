// Functional providers: one per view-model stream or one-shot of the core.
// Each body is a single `CoreApi` call; nothing is filtered, sorted, merged or
// derived here (PLAN §11.1, L15).
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata_bridge/strata_bridge.dart'
    show
        AdminUsersView,
        AskView,
        ConflictScreen,
        DirectoryTab,
        DirectoryView,
        DuplicatePromptsView,
        EditorHint,
        EntityScreen,
        GlobalGraphView,
        HomeView,
        InboxView,
        LocalGraphView,
        NoteScreen,
        NotesListView,
        NotificationOp,
        SearchMode,
        SearchView,
        SessionState,
        SettingsView,
        SyncStatusView,
        TaskScreen,
        TasksView;
import 'package:strata_state/src/core_api.dart';

part 'providers.g.dart';

/// Riverpod's retry policy for every core provider: never re-subscribe.
///
/// The Rust core owns retries and backoff (§12.4); an error on one of its
/// streams or one-shots is final and is rendered as such.
Duration? noCoreRetry(int retryCount, Object error) => null;

/// The Rust client core, as a [CoreApi].
///
/// Has no default: the app overrides it with `BridgeCoreApi`, tests with
/// `FakeCoreApi` (`package:strata_state/testing.dart`).
@Riverpod(keepAlive: true, retry: noCoreRetry)
CoreApi coreApi(Ref ref) => throw UnimplementedError(
  'coreApiProvider must be overridden (BridgeCoreApi in the app, '
  'FakeCoreApi in tests).',
);

// ---------------------------------------------------------------------------
// App-wide streams (kept alive for the whole session)
// ---------------------------------------------------------------------------

/// The session state: login screen, main shell or a restricted screen
/// (`CoreApi.watchSession`).
@Riverpod(keepAlive: true, retry: noCoreRetry)
Stream<SessionState> session(Ref ref) =>
    ref.watch(coreApiProvider).watchSession();

/// Sync status and conflicts (`CoreApi.watchSyncStatus`).
@Riverpod(keepAlive: true, retry: noCoreRetry)
Stream<SyncStatusView> syncStatus(Ref ref) =>
    ref.watch(coreApiProvider).watchSyncStatus();

/// Notification ops the platform adapter executes
/// (`CoreApi.watchNotificationOps`).
@Riverpod(keepAlive: true, retry: noCoreRetry)
Stream<NotificationOp> notificationOps(Ref ref) =>
    ref.watch(coreApiProvider).watchNotificationOps();

// ---------------------------------------------------------------------------
// Screen streams (disposed when no widget listens)
// ---------------------------------------------------------------------------

/// Home (`CoreApi.watchHome`).
@Riverpod(retry: noCoreRetry)
Stream<HomeView> home(Ref ref) => ref.watch(coreApiProvider).watchHome();

/// Inbox (`CoreApi.watchInbox`).
@Riverpod(retry: noCoreRetry)
Stream<InboxView> inbox(Ref ref) => ref.watch(coreApiProvider).watchInbox();

/// One note (`CoreApi.watchNote`).
@Riverpod(retry: noCoreRetry)
Stream<NoteScreen> note(Ref ref, String id) =>
    ref.watch(coreApiProvider).watchNote(id: id);

/// One folder of the notes tree, `''` = vault root
/// (`CoreApi.watchNotesList`).
@Riverpod(retry: noCoreRetry)
Stream<NotesListView> notesList(Ref ref, String folder) =>
    ref.watch(coreApiProvider).watchNotesList(folder: folder);

/// A directory tab filtered by [query] (`CoreApi.watchDirectory`).
@Riverpod(retry: noCoreRetry)
Stream<DirectoryView> directory(Ref ref, DirectoryTab tab, String query) =>
    ref.watch(coreApiProvider).watchDirectory(tab: tab, query: query);

/// A person, company, document or place page (`CoreApi.watchEntity`).
@Riverpod(retry: noCoreRetry)
Stream<EntityScreen> entity(Ref ref, String id) =>
    ref.watch(coreApiProvider).watchEntity(id: id);

/// The Tasks destination (`CoreApi.watchTasks`).
@Riverpod(retry: noCoreRetry)
Stream<TasksView> tasks(Ref ref) => ref.watch(coreApiProvider).watchTasks();

/// Task detail (`CoreApi.watchTask`).
@Riverpod(retry: noCoreRetry)
Stream<TaskScreen> task(Ref ref, String id) =>
    ref.watch(coreApiProvider).watchTask(id: id);

/// One sync conflict (`CoreApi.watchConflict`).
@Riverpod(retry: noCoreRetry)
Stream<ConflictScreen> conflict(Ref ref, String opId) =>
    ref.watch(coreApiProvider).watchConflict(opId: opId);

/// Open "Already exists" prompts (`CoreApi.watchDuplicatePrompts`).
@Riverpod(retry: noCoreRetry)
Stream<DuplicatePromptsView> duplicatePrompts(Ref ref) =>
    ref.watch(coreApiProvider).watchDuplicatePrompts();

/// Settings (`CoreApi.watchSettings`).
@Riverpod(retry: noCoreRetry)
Stream<SettingsView> settings(Ref ref) =>
    ref.watch(coreApiProvider).watchSettings();

/// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).
@Riverpod(retry: noCoreRetry)
Stream<LocalGraphView> localGraph(Ref ref, String id, int depth) =>
    ref.watch(coreApiProvider).watchLocalGraph(id: id, depth: depth);

// ---------------------------------------------------------------------------
// One-shots (a new read on invalidate / refresh)
// ---------------------------------------------------------------------------

/// The global map (`CoreApi.globalGraph`).
@Riverpod(retry: noCoreRetry)
Future<GlobalGraphView> globalGraph(Ref ref) =>
    ref.watch(coreApiProvider).globalGraph();

/// Local search (`CoreApi.search`).
@Riverpod(retry: noCoreRetry)
Future<SearchView> search(Ref ref, String query, SearchMode mode) =>
    ref.watch(coreApiProvider).search(query: query, mode: mode);

/// Ask (`CoreApi.askView`).
@Riverpod(retry: noCoreRetry)
Future<AskView> askView(Ref ref) => ref.watch(coreApiProvider).askView();

/// Editor highlight spans for [content] (`CoreApi.editorHints`).
@Riverpod(retry: noCoreRetry)
Future<List<EditorHint>> editorHints(Ref ref, String content) =>
    ref.watch(coreApiProvider).editorHints(content: content);

/// Admin → Users (`CoreApi.loadAdminUsers`).
@Riverpod(retry: noCoreRetry)
Future<AdminUsersView> adminUsers(Ref ref) =>
    ref.watch(coreApiProvider).loadAdminUsers();
