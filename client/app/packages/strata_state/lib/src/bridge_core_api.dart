import 'package:strata_bridge/strata_bridge.dart' as bridge;
import 'package:strata_bridge/strata_bridge.dart'
    show
        AdminUsersView,
        AppLifecycle,
        AskView,
        ConflictResolution,
        ConflictScreen,
        CoreConfig,
        CreateOutcome,
        DirectoryTab,
        DirectoryView,
        DuplicateChoice,
        DuplicatePromptsView,
        EditorHint,
        EntityScreen,
        GlobalGraphView,
        HomeView,
        InboxView,
        LocalGraphView,
        NoteScreen,
        NotesListView,
        NotificationAction,
        NotificationOp,
        NotificationResult,
        SearchMode,
        SearchView,
        SessionState,
        SettingsView,
        SignInRequest,
        SignOutOutcome,
        SignUpOutcome,
        SignUpRequest,
        SyncStatusView,
        TaskDraft,
        TaskPatch,
        TaskScreen,
        TasksView;
import 'package:strata_state/src/core_api.dart';

/// The production [CoreApi]: every method forwards to the generated
/// top-level function of the same name, with the same arguments. The core
/// must be loaded (`loadStrataCore`) before the first call.
final class BridgeCoreApi implements CoreApi {
  /// Creates the bridge-backed API.
  const new();

  // App and session (`api/app.rs`)
  @override
  Future<SessionState> initCore({required CoreConfig config}) =>
      bridge.initCore(config: config);

  @override
  Stream<SessionState> watchSession() => bridge.watchSession();

  @override
  Future<SignUpOutcome> signUp({required SignUpRequest request}) =>
      bridge.signUp(request: request);

  @override
  Future<SessionState> signIn({required SignInRequest request}) =>
      bridge.signIn(request: request);

  @override
  Future<SignOutOutcome> signOut({required bool force}) =>
      bridge.signOut(force: force);

  @override
  Future<SessionState> switchAccount({required String userId}) =>
      bridge.switchAccount(userId: userId);

  @override
  Future<SessionState> acknowledgeAccountDisabled() =>
      bridge.acknowledgeAccountDisabled();

  @override
  Future<void> refreshAccount() => bridge.refreshAccount();

  @override
  Future<void> appLifecycle({required AppLifecycle state}) =>
      bridge.appLifecycle(state: state);

  @override
  Future<void> syncNow() => bridge.syncNow();

  // Intents (`api/intents.rs`)
  @override
  Future<String> capture({required String text}) => bridge.capture(text: text);

  @override
  Future<CreateOutcome> createNote({
    required String path,
    required String content,
    required bool force,
  }) => bridge.createNote(path: path, content: content, force: force);

  @override
  Future<String> updateNote({required String id, required String content}) =>
      bridge.updateNote(id: id, content: content);

  @override
  Future<String> moveNote({required String id, required String newPath}) =>
      bridge.moveNote(id: id, newPath: newPath);

  @override
  Future<String> deleteNote({required String id}) => bridge.deleteNote(id: id);

  @override
  Future<CreateOutcome> createEntity({
    required String kind,
    required String name,
    required List<String> aliases,
    required bool force,
  }) => bridge.createEntity(
    kind: kind,
    name: name,
    aliases: aliases,
    force: force,
  );

  @override
  Future<String> addRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => bridge.addRelation(srcId: srcId, dstId: dstId, relType: relType);

  @override
  Future<String> removeRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => bridge.removeRelation(srcId: srcId, dstId: dstId, relType: relType);

  @override
  Future<String> retypeRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newType,
  }) => bridge.retypeRelation(
    srcId: srcId,
    dstId: dstId,
    relType: relType,
    newType: newType,
  );

  @override
  Future<String> acceptSuggestion({required String id}) =>
      bridge.acceptSuggestion(id: id);

  @override
  Future<String> rejectSuggestion({required String id}) =>
      bridge.rejectSuggestion(id: id);

  @override
  Future<String> requestRelink({required String noteId}) =>
      bridge.requestRelink(noteId: noteId);

  @override
  Future<CreateOutcome> createTask({
    required TaskDraft draft,
    required bool force,
  }) => bridge.createTask(draft: draft, force: force);

  @override
  Future<String> updateTask({
    required String taskId,
    required TaskPatch patch,
  }) => bridge.updateTask(taskId: taskId, patch: patch);

  @override
  Future<String> completeTask({required String taskId}) =>
      bridge.completeTask(taskId: taskId);

  @override
  Future<String> cancelTask({required String taskId}) =>
      bridge.cancelTask(taskId: taskId);

  @override
  Future<String> reopenTask({required String taskId}) =>
      bridge.reopenTask(taskId: taskId);

  @override
  Future<String> deleteTask({required String taskId}) =>
      bridge.deleteTask(taskId: taskId);

  @override
  Future<void> resolveConflict({
    required String opId,
    required ConflictResolution resolution,
  }) => bridge.resolveConflict(opId: opId, resolution: resolution);

  @override
  Future<void> resolveDuplicate({
    required String opId,
    required DuplicateChoice choice,
  }) => bridge.resolveDuplicate(opId: opId, choice: choice);

  @override
  Future<void> dismissRejection({required String opId}) =>
      bridge.dismissRejection(opId: opId);

  @override
  Future<void> setRemindersEnabled({required bool enabled}) =>
      bridge.setRemindersEnabled(enabled: enabled);

  // Reminders (`api/reminders.rs`)
  @override
  Stream<NotificationOp> watchNotificationOps() =>
      bridge.watchNotificationOps();

  @override
  Future<void> reportNotificationResult({
    required int id,
    required NotificationResult result,
  }) => bridge.reportNotificationResult(id: id, result: result);

  @override
  Future<String> notificationAction({
    required int id,
    required NotificationAction action,
  }) => bridge.notificationAction(id: id, action: action);

  // Views (`api/views.rs`)
  @override
  Stream<HomeView> watchHome() => bridge.watchHome();

  @override
  Stream<InboxView> watchInbox() => bridge.watchInbox();

  @override
  Stream<NoteScreen> watchNote({required String id}) =>
      bridge.watchNote(id: id);

  @override
  Stream<NotesListView> watchNotesList({required String folder}) =>
      bridge.watchNotesList(folder: folder);

  @override
  Stream<DirectoryView> watchDirectory({
    required DirectoryTab tab,
    required String query,
  }) => bridge.watchDirectory(tab: tab, query: query);

  @override
  Stream<EntityScreen> watchEntity({required String id}) =>
      bridge.watchEntity(id: id);

  @override
  Stream<TasksView> watchTasks() => bridge.watchTasks();

  @override
  Stream<TaskScreen> watchTask({required String id}) =>
      bridge.watchTask(id: id);

  @override
  Stream<SyncStatusView> watchSyncStatus() => bridge.watchSyncStatus();

  @override
  Stream<ConflictScreen> watchConflict({required String opId}) =>
      bridge.watchConflict(opId: opId);

  @override
  Stream<DuplicatePromptsView> watchDuplicatePrompts() =>
      bridge.watchDuplicatePrompts();

  @override
  Stream<SettingsView> watchSettings() => bridge.watchSettings();

  @override
  Stream<LocalGraphView> watchLocalGraph({
    required String id,
    required int depth,
  }) => bridge.watchLocalGraph(id: id, depth: depth);

  @override
  Future<GlobalGraphView> globalGraph() => bridge.globalGraph();

  @override
  Future<SearchView> search({
    required String query,
    required SearchMode mode,
  }) => bridge.search(query: query, mode: mode);

  @override
  Future<AskView> askView() => bridge.askView();

  @override
  Future<List<EditorHint>> editorHints({required String content}) =>
      bridge.editorHints(content: content);

  @override
  Future<AdminUsersView> loadAdminUsers() => bridge.loadAdminUsers();
}
