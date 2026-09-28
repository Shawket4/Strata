// Functional providers: one per view-model stream or one-shot of the core.
// Each body is a single `CoreApi` call; nothing is filtered, sorted, merged or
// derived here (PLAN §11.1, L15).
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata_bridge/strata_bridge.dart'
    show
        AdminUsersView,
        AskView,
        BlockItem,
        CitationPreview,
        Completions,
        ConflictScreen,
        DirectoryFilter,
        DirectorySort,
        DirectoryTab,
        DirectoryView,
        DuplicatePromptsView,
        EditorHint,
        EntityScreen,
        GlobalGraphView,
        GraphFilter,
        HomeView,
        InboxFilter,
        InboxView,
        LocalGraphView,
        MergePreview,
        NavView,
        NoteDiffView,
        NoteScreen,
        NotesListView,
        NotificationOp,
        PasswordStrength,
        PlaceOption,
        RecentFilter,
        RecentNotesView,
        RecurrenceCompose,
        RecurrenceForm,
        RecurrencePreviewItem,
        RelationTypeItem,
        RepointChoice,
        SearchMode,
        SearchView,
        SessionState,
        SettingsView,
        SyncStatusView,
        TagItem,
        TaskDraftPreview,
        TaskHomesView,
        TaskScreen,
        TasksView,
        TimeZoneItem;
import 'package:strata_state/src/core_api.dart';
import 'package:strata_state/src/file_picker.dart';

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

/// The OS file dialogs (export and import paths).
///
/// Has no default: the app shell overrides it with its `file_selector`
/// picker, tests with `FakeFilePicker` (`package:strata_state/testing.dart`).
@Riverpod(keepAlive: true, retry: noCoreRetry)
FilePicker filePicker(Ref ref) => throw UnimplementedError(
  'filePickerProvider must be overridden (the app shell picker in the app, '
  'FakeFilePicker in tests).',
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

/// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).
@Riverpod(retry: noCoreRetry)
Stream<InboxView> inboxFiltered(Ref ref, InboxFilter filter) =>
    ref.watch(coreApiProvider).watchInboxFiltered(filter: filter);

/// Navigation counts and pinned notes (`CoreApi.watchNav`).
@Riverpod(retry: noCoreRetry)
Stream<NavView> nav(Ref ref) => ref.watch(coreApiProvider).watchNav();

/// The "Recent" block with a filter (`CoreApi.watchRecent`).
@Riverpod(retry: noCoreRetry)
Stream<RecentNotesView> recent(Ref ref, RecentFilter filter) =>
    ref.watch(coreApiProvider).watchRecent(filter: filter);

/// A directory tab with filters and sort applied in the core
/// (`CoreApi.watchDirectoryFiltered`).
@Riverpod(retry: noCoreRetry)
Stream<DirectoryView> directoryFiltered(
  Ref ref,
  DirectoryTab tab,
  String query,
  DirectoryFilter filter,
  DirectorySort sort,
) => ref
    .watch(coreApiProvider)
    .watchDirectoryFiltered(tab: tab, query: query, filter: filter, sort: sort);

/// The home-note picker of the new-task sheet (`CoreApi.watchTaskHomes`).
@Riverpod(retry: noCoreRetry)
Stream<TaskHomesView> taskHomes(Ref ref) =>
    ref.watch(coreApiProvider).watchTaskHomes();

/// The Ask conversation, streamed (`CoreApi.watchAsk`).
@Riverpod(retry: noCoreRetry)
Stream<AskView> askConversation(Ref ref) =>
    ref.watch(coreApiProvider).watchAsk();

/// A note's local mind map with only the edges of [edgeKinds] (empty: all),
/// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
/// list instance while the selection is unchanged.
@Riverpod(retry: noCoreRetry)
Stream<LocalGraphView> localGraphFiltered(
  Ref ref,
  String id,
  int depth,
  List<String> edgeKinds,
) => ref
    .watch(coreApiProvider)
    .watchLocalGraphFiltered(id: id, depth: depth, edgeKinds: edgeKinds);

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

/// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).
@Riverpod(retry: noCoreRetry)
Future<AdminUsersView> adminUsers(Ref ref, String query) =>
    ref.watch(coreApiProvider).loadAdminUsers(query: query);

/// The global map with filters and lens applied in the core
/// (`CoreApi.globalGraphFiltered`).
@Riverpod(retry: noCoreRetry)
Future<GlobalGraphView> globalGraphFiltered(Ref ref, GraphFilter filter) =>
    ref.watch(coreApiProvider).globalGraphFiltered(filter: filter);

/// Search limited to a folder (`CoreApi.searchInFolder`).
@Riverpod(retry: noCoreRetry)
Future<SearchView> searchInFolder(
  Ref ref,
  String query,
  SearchMode mode,
  String? folder,
) => ref
    .watch(coreApiProvider)
    .searchInFolder(query: query, mode: mode, folder: folder);

/// Editor completions at [cursor] (`CoreApi.editorCompletions`).
@Riverpod(retry: noCoreRetry)
Future<Completions> editorCompletions(
  Ref ref,
  String noteId,
  String content,
  int cursor,
) => ref
    .watch(coreApiProvider)
    .editorCompletions(noteId: noteId, content: content, cursor: cursor);

/// Vault tags starting with [prefix] (`CoreApi.tags`).
@Riverpod(retry: noCoreRetry)
Future<List<TagItem>> tags(Ref ref, String prefix) =>
    ref.watch(coreApiProvider).tags(prefix: prefix);

/// The blocks of a note (`CoreApi.noteBlocks`).
@Riverpod(retry: noCoreRetry)
Future<List<BlockItem>> noteBlocks(Ref ref, String noteId) =>
    ref.watch(coreApiProvider).noteBlocks(noteId: noteId);

/// Relation types with labels (`CoreApi.relationTypes`).
@Riverpod(retry: noCoreRetry)
Future<List<RelationTypeItem>> relationTypes(Ref ref) =>
    ref.watch(coreApiProvider).relationTypes();

/// The recurrence form of a phrase (`CoreApi.recurrenceForm`).
@Riverpod(retry: noCoreRetry)
Future<RecurrenceForm?> recurrenceForm(Ref ref, String phrase) =>
    ref.watch(coreApiProvider).recurrenceForm(phrase: phrase);

/// A recurrence form compiled to its phrase and summary
/// (`CoreApi.composeRecurrence`).
@Riverpod(retry: noCoreRetry)
Future<RecurrenceCompose> composeRecurrence(Ref ref, RecurrenceForm form) =>
    ref.watch(coreApiProvider).composeRecurrence(form: form);

/// The next dates of a recurrence (`CoreApi.recurrencePreview`).
@Riverpod(retry: noCoreRetry)
Future<List<RecurrencePreviewItem>> recurrencePreview(
  Ref ref,
  String phrase,
  DateTime from,
  int count,
) => ref
    .watch(coreApiProvider)
    .recurrencePreview(phrase: phrase, from: from, count: count);

/// A new task's text as the core understands it (`CoreApi.parseTaskText`).
@Riverpod(retry: noCoreRetry)
Future<TaskDraftPreview> parseTaskText(Ref ref, String text) =>
    ref.watch(coreApiProvider).parseTaskText(text: text);

/// The time zones of the Settings picker matching [query]
/// (`CoreApi.timezones`).
@Riverpod(retry: noCoreRetry)
Future<List<TimeZoneItem>> timeZones(Ref ref, String query) =>
    ref.watch(coreApiProvider).timezones(query: query);

/// New targets for an AI decision (`CoreApi.repointChoices`).
@Riverpod(retry: noCoreRetry)
Future<List<RepointChoice>> repointChoices(
  Ref ref,
  String decisionId,
  String query,
) => ref
    .watch(coreApiProvider)
    .repointChoices(decisionId: decisionId, query: query);

/// Places for the location picker (`CoreApi.placeOptions`).
@Riverpod(retry: noCoreRetry)
Future<List<PlaceOption>> placeOptions(Ref ref, String? documentId) =>
    ref.watch(coreApiProvider).placeOptions(documentId: documentId);

/// What merging two entities does (`CoreApi.mergePreview`).
@Riverpod(retry: noCoreRetry)
Future<MergePreview> mergePreview(Ref ref, String sourceId, String intoId) =>
    ref.watch(coreApiProvider).mergePreview(sourceId: sourceId, intoId: intoId);

/// The block a citation points to (`CoreApi.resolveCitation`).
@Riverpod(retry: noCoreRetry)
Future<CitationPreview> resolveCitation(
  Ref ref,
  String noteId,
  String? anchor,
) => ref.watch(coreApiProvider).resolveCitation(noteId: noteId, anchor: anchor);

/// A revision compared with the current note (`CoreApi.noteRevisionDiff`).
@Riverpod(retry: noCoreRetry)
Future<NoteDiffView> noteRevisionDiff(Ref ref, String noteId, String commit) =>
    ref.watch(coreApiProvider).noteRevisionDiff(noteId: noteId, commit: commit);

/// The sign-up password meter (`CoreApi.passwordStrength`).
@Riverpod(retry: noCoreRetry)
Future<PasswordStrength> passwordStrength(Ref ref, String password) =>
    ref.watch(coreApiProvider).passwordStrength(password: password);
