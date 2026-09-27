import 'dart:async';

import 'package:flutter/foundation.dart';
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
import 'package:strata_state/src/testing/fixtures.dart';

/// One recorded [CoreApi] call: the method name and its named arguments,
/// exactly as passed.
@immutable
final class CoreCall {
  /// Creates a record of `method(args)`.
  const new(this.method, [this.args = const {}]);

  /// The [CoreApi] method name (`capture`, `watchNote`, …).
  final String method;

  /// The named arguments by parameter name.
  final Map<String, Object?> args;

  @override
  bool operator ==(Object other) =>
      other is CoreCall &&
      other.method == method &&
      mapEquals(other.args, args);

  @override
  int get hashCode => Object.hash(
    method,
    Object.hashAllUnordered(
      args.entries.map((e) => Object.hash(e.key, e.value)),
    ),
  );

  @override
  String toString() => '$method($args)';
}

/// A controllable core stream: what the test [add]s is what subscribers get.
///
/// Like the core's own view streams, a new subscriber first receives the
/// latest value (if any), so fixtures can be added before or after the
/// widget under test subscribes.
final class FakeStream<T> implements EventSink<T> {
  final StreamController<T> _controller = StreamController<T>.broadcast(
    sync: true,
  );
  T? _latest;
  bool _hasLatest = false;

  /// The stream handed to the provider.
  Stream<T> get stream => Stream<T>.multi((subscriber) {
    if (_hasLatest) subscriber.add(_latest as T);
    final subscription = _controller.stream.listen(
      subscriber.add,
      onError: subscriber.addError,
      onDone: subscriber.close,
    );
    subscriber
      ..onCancel = subscription.cancel
      ..onPause = subscription.pause
      ..onResume = subscription.resume;
  });

  /// Whether a subscriber is listening now.
  bool get hasListener => _controller.hasListener;

  /// Emits [value] (and remembers it for later subscribers).
  @override
  void add(T value) {
    _latest = value;
    _hasLatest = true;
    _controller.add(value);
  }

  /// Emits an error (e.g. a `CoreFailure`); not replayed.
  @override
  void addError(Object error, [StackTrace? stackTrace]) =>
      _controller.addError(error, stackTrace);

  /// Closes the stream.
  @override
  void close() => unawaited(_controller.close());
}

/// One [FakeStream] per argument of a parameterised core stream
/// (`fake.note['n-1']`, `fake.directory[(DirectoryTab.people, '')]`).
final class FakeStreamFamily<K, T> {
  final Map<K, FakeStream<T>> _streams = {};

  /// The stream for [key] (created on first use).
  FakeStream<T> operator [](K key) => _streams.putIfAbsent(key, FakeStream.new);

  /// Keys that have been asked for so far.
  Iterable<K> get keys => _streams.keys;

  void _closeAll() {
    for (final stream in _streams.values) {
      stream.close();
    }
  }
}

/// The canned answer of a one-shot or intent: a value, or an error to throw.
final class FakeAnswer<T> {
  /// Creates an answer that returns [value].
  new(T value) : _value = value;

  T _value;
  Object? _error;
  StackTrace? _stackTrace;

  /// Answers with [value] from now on.
  void returns(T value) {
    _value = value;
    _error = null;
    _stackTrace = null;
  }

  /// Throws [error] (e.g. a `CoreFailure`) from now on.
  void throws(Object error, [StackTrace? stackTrace]) {
    _error = error;
    _stackTrace = stackTrace;
  }

  Future<T> _answer() {
    final error = _error;
    return error == null
        ? Future.value(_value)
        : Future.error(error, _stackTrace);
  }
}

/// A hand-written [CoreApi] test double for widget tests.
///
/// - **Streams:** every `watch*` method returns a [FakeStream] the test
///   controls: `fake.home.add(StrataFixtures.homeView)`,
///   `fake.note['n-1'].add(...)`.
/// - **One-shots and intents:** return their [FakeAnswer]
///   (`fake.searchAnswer.returns(...)`, `fake.signInAnswer.throws(failure)`);
///   defaults come from `StrataFixtures`.
/// - **Calls:** every call is appended to [calls] with its exact arguments.
final class FakeCoreApi implements CoreApi {
  /// Creates a fake with empty streams and fixture answers.
  new();

  /// Every call made, in order.
  final List<CoreCall> calls = [];

  // Streams ------------------------------------------------------------------

  /// `watchSession`.
  final FakeStream<SessionState> session = FakeStream();

  /// `watchNotificationOps`.
  final FakeStream<NotificationOp> notificationOps = FakeStream();

  /// `watchHome`.
  final FakeStream<HomeView> home = FakeStream();

  /// `watchInbox`.
  final FakeStream<InboxView> inbox = FakeStream();

  /// `watchNote`, by note ID.
  final FakeStreamFamily<String, NoteScreen> note = FakeStreamFamily();

  /// `watchNotesList`, by folder.
  final FakeStreamFamily<String, NotesListView> notesList = FakeStreamFamily();

  /// `watchDirectory`, by `(tab, query)`.
  final FakeStreamFamily<(DirectoryTab, String), DirectoryView> directory =
      FakeStreamFamily();

  /// `watchEntity`, by ID.
  final FakeStreamFamily<String, EntityScreen> entity = FakeStreamFamily();

  /// `watchTasks`.
  final FakeStream<TasksView> tasks = FakeStream();

  /// `watchTask`, by task ID.
  final FakeStreamFamily<String, TaskScreen> task = FakeStreamFamily();

  /// `watchSyncStatus`.
  final FakeStream<SyncStatusView> syncStatus = FakeStream();

  /// `watchConflict`, by op ID.
  final FakeStreamFamily<String, ConflictScreen> conflict = FakeStreamFamily();

  /// `watchDuplicatePrompts`.
  final FakeStream<DuplicatePromptsView> duplicatePrompts = FakeStream();

  /// `watchSettings`.
  final FakeStream<SettingsView> settings = FakeStream();

  /// `watchLocalGraph`, by `(id, depth)`.
  final FakeStreamFamily<(String, int), LocalGraphView> localGraph =
      FakeStreamFamily();

  // Answers ------------------------------------------------------------------

  /// `initCore`.
  final FakeAnswer<SessionState> initCoreAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `signUp`.
  final FakeAnswer<SignUpOutcome> signUpAnswer = FakeAnswer(
    StrataFixtures.signUpOutcome,
  );

  /// `signIn`.
  final FakeAnswer<SessionState> signInAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `signOut`.
  final FakeAnswer<SignOutOutcome> signOutAnswer = FakeAnswer(
    StrataFixtures.signOutOutcome,
  );

  /// `switchAccount`.
  final FakeAnswer<SessionState> switchAccountAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `acknowledgeAccountDisabled`.
  final FakeAnswer<SessionState> acknowledgeAccountDisabledAnswer = FakeAnswer(
    StrataFixtures.sessionSignedOut,
  );

  /// `refreshAccount`.
  final FakeAnswer<void> refreshAccountAnswer = FakeAnswer(null);

  /// `appLifecycle`.
  final FakeAnswer<void> appLifecycleAnswer = FakeAnswer(null);

  /// `syncNow`.
  final FakeAnswer<void> syncNowAnswer = FakeAnswer(null);

  /// `capture` (the new capture's note ID).
  final FakeAnswer<String> captureAnswer = FakeAnswer(StrataFixtures.opId);

  /// `createNote`.
  final FakeAnswer<CreateOutcome> createNoteAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `updateNote`.
  final FakeAnswer<String> updateNoteAnswer = FakeAnswer(StrataFixtures.opId);

  /// `moveNote`.
  final FakeAnswer<String> moveNoteAnswer = FakeAnswer(StrataFixtures.opId);

  /// `deleteNote`.
  final FakeAnswer<String> deleteNoteAnswer = FakeAnswer(StrataFixtures.opId);

  /// `createEntity`.
  final FakeAnswer<CreateOutcome> createEntityAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `addRelation`.
  final FakeAnswer<String> addRelationAnswer = FakeAnswer(StrataFixtures.opId);

  /// `removeRelation`.
  final FakeAnswer<String> removeRelationAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `retypeRelation`.
  final FakeAnswer<String> retypeRelationAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `acceptSuggestion`.
  final FakeAnswer<String> acceptSuggestionAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `rejectSuggestion`.
  final FakeAnswer<String> rejectSuggestionAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `requestRelink`.
  final FakeAnswer<String> requestRelinkAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `createTask`.
  final FakeAnswer<CreateOutcome> createTaskAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `updateTask`.
  final FakeAnswer<String> updateTaskAnswer = FakeAnswer(StrataFixtures.opId);

  /// `completeTask`.
  final FakeAnswer<String> completeTaskAnswer = FakeAnswer(StrataFixtures.opId);

  /// `cancelTask`.
  final FakeAnswer<String> cancelTaskAnswer = FakeAnswer(StrataFixtures.opId);

  /// `reopenTask`.
  final FakeAnswer<String> reopenTaskAnswer = FakeAnswer(StrataFixtures.opId);

  /// `deleteTask`.
  final FakeAnswer<String> deleteTaskAnswer = FakeAnswer(StrataFixtures.opId);

  /// `resolveConflict`.
  final FakeAnswer<void> resolveConflictAnswer = FakeAnswer(null);

  /// `resolveDuplicate`.
  final FakeAnswer<void> resolveDuplicateAnswer = FakeAnswer(null);

  /// `dismissRejection`.
  final FakeAnswer<void> dismissRejectionAnswer = FakeAnswer(null);

  /// `setRemindersEnabled`.
  final FakeAnswer<void> setRemindersEnabledAnswer = FakeAnswer(null);

  /// `reportNotificationResult`.
  final FakeAnswer<void> reportNotificationResultAnswer = FakeAnswer(null);

  /// `notificationAction`.
  final FakeAnswer<String> notificationActionAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `globalGraph`.
  final FakeAnswer<GlobalGraphView> globalGraphAnswer = FakeAnswer(
    StrataFixtures.globalGraphView,
  );

  /// `search`.
  final FakeAnswer<SearchView> searchAnswer = FakeAnswer(
    StrataFixtures.searchView,
  );

  /// `askView`.
  final FakeAnswer<AskView> askViewAnswer = FakeAnswer(StrataFixtures.askView);

  /// `editorHints`.
  final FakeAnswer<List<EditorHint>> editorHintsAnswer = FakeAnswer(
    StrataFixtures.editorHints,
  );

  /// `loadAdminUsers`.
  final FakeAnswer<AdminUsersView> loadAdminUsersAnswer = FakeAnswer(
    StrataFixtures.adminUsersView,
  );

  /// Closes every stream.
  void dispose() {
    for (final stream in [
      session,
      notificationOps,
      home,
      inbox,
      tasks,
      syncStatus,
      duplicatePrompts,
      settings,
    ]) {
      stream.close();
    }
    for (final family in [
      note,
      notesList,
      directory,
      entity,
      task,
      conflict,
      localGraph,
    ]) {
      family._closeAll();
    }
  }

  Stream<T> _watch<T>(
    FakeStream<T> stream,
    String method, [
    Map<String, Object?> args = const {},
  ]) {
    calls.add(CoreCall(method, args));
    return stream.stream;
  }

  Future<T> _call<T>(
    FakeAnswer<T> answer,
    String method, [
    Map<String, Object?> args = const {},
  ]) {
    calls.add(CoreCall(method, args));
    return answer._answer();
  }

  // App and session ----------------------------------------------------------

  @override
  Future<SessionState> initCore({required CoreConfig config}) =>
      _call(initCoreAnswer, 'initCore', {'config': config});

  @override
  Stream<SessionState> watchSession() => _watch(session, 'watchSession');

  @override
  Future<SignUpOutcome> signUp({required SignUpRequest request}) =>
      _call(signUpAnswer, 'signUp', {'request': request});

  @override
  Future<SessionState> signIn({required SignInRequest request}) =>
      _call(signInAnswer, 'signIn', {'request': request});

  @override
  Future<SignOutOutcome> signOut({required bool force}) =>
      _call(signOutAnswer, 'signOut', {'force': force});

  @override
  Future<SessionState> switchAccount({required String userId}) =>
      _call(switchAccountAnswer, 'switchAccount', {'userId': userId});

  @override
  Future<SessionState> acknowledgeAccountDisabled() =>
      _call(acknowledgeAccountDisabledAnswer, 'acknowledgeAccountDisabled');

  @override
  Future<void> refreshAccount() =>
      _call(refreshAccountAnswer, 'refreshAccount');

  @override
  Future<void> appLifecycle({required AppLifecycle state}) =>
      _call(appLifecycleAnswer, 'appLifecycle', {'state': state});

  @override
  Future<void> syncNow() => _call(syncNowAnswer, 'syncNow');

  // Intents ------------------------------------------------------------------

  @override
  Future<String> capture({required String text}) =>
      _call(captureAnswer, 'capture', {'text': text});

  @override
  Future<CreateOutcome> createNote({
    required String path,
    required String content,
    required bool force,
  }) => _call(createNoteAnswer, 'createNote', {
    'path': path,
    'content': content,
    'force': force,
  });

  @override
  Future<String> updateNote({required String id, required String content}) =>
      _call(updateNoteAnswer, 'updateNote', {'id': id, 'content': content});

  @override
  Future<String> moveNote({required String id, required String newPath}) =>
      _call(moveNoteAnswer, 'moveNote', {'id': id, 'newPath': newPath});

  @override
  Future<String> deleteNote({required String id}) =>
      _call(deleteNoteAnswer, 'deleteNote', {'id': id});

  @override
  Future<CreateOutcome> createEntity({
    required String kind,
    required String name,
    required List<String> aliases,
    required bool force,
  }) => _call(createEntityAnswer, 'createEntity', {
    'kind': kind,
    'name': name,
    'aliases': aliases,
    'force': force,
  });

  @override
  Future<String> addRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => _call(addRelationAnswer, 'addRelation', {
    'srcId': srcId,
    'dstId': dstId,
    'relType': relType,
  });

  @override
  Future<String> removeRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => _call(removeRelationAnswer, 'removeRelation', {
    'srcId': srcId,
    'dstId': dstId,
    'relType': relType,
  });

  @override
  Future<String> retypeRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newType,
  }) => _call(retypeRelationAnswer, 'retypeRelation', {
    'srcId': srcId,
    'dstId': dstId,
    'relType': relType,
    'newType': newType,
  });

  @override
  Future<String> acceptSuggestion({required String id}) =>
      _call(acceptSuggestionAnswer, 'acceptSuggestion', {'id': id});

  @override
  Future<String> rejectSuggestion({required String id}) =>
      _call(rejectSuggestionAnswer, 'rejectSuggestion', {'id': id});

  @override
  Future<String> requestRelink({required String noteId}) =>
      _call(requestRelinkAnswer, 'requestRelink', {'noteId': noteId});

  @override
  Future<CreateOutcome> createTask({
    required TaskDraft draft,
    required bool force,
  }) => _call(createTaskAnswer, 'createTask', {'draft': draft, 'force': force});

  @override
  Future<String> updateTask({
    required String taskId,
    required TaskPatch patch,
  }) =>
      _call(updateTaskAnswer, 'updateTask', {'taskId': taskId, 'patch': patch});

  @override
  Future<String> completeTask({required String taskId}) =>
      _call(completeTaskAnswer, 'completeTask', {'taskId': taskId});

  @override
  Future<String> cancelTask({required String taskId}) =>
      _call(cancelTaskAnswer, 'cancelTask', {'taskId': taskId});

  @override
  Future<String> reopenTask({required String taskId}) =>
      _call(reopenTaskAnswer, 'reopenTask', {'taskId': taskId});

  @override
  Future<String> deleteTask({required String taskId}) =>
      _call(deleteTaskAnswer, 'deleteTask', {'taskId': taskId});

  @override
  Future<void> resolveConflict({
    required String opId,
    required ConflictResolution resolution,
  }) => _call(resolveConflictAnswer, 'resolveConflict', {
    'opId': opId,
    'resolution': resolution,
  });

  @override
  Future<void> resolveDuplicate({
    required String opId,
    required DuplicateChoice choice,
  }) => _call(resolveDuplicateAnswer, 'resolveDuplicate', {
    'opId': opId,
    'choice': choice,
  });

  @override
  Future<void> dismissRejection({required String opId}) =>
      _call(dismissRejectionAnswer, 'dismissRejection', {'opId': opId});

  @override
  Future<void> setRemindersEnabled({required bool enabled}) => _call(
    setRemindersEnabledAnswer,
    'setRemindersEnabled',
    {'enabled': enabled},
  );

  // Reminders ----------------------------------------------------------------

  @override
  Stream<NotificationOp> watchNotificationOps() =>
      _watch(notificationOps, 'watchNotificationOps');

  @override
  Future<void> reportNotificationResult({
    required int id,
    required NotificationResult result,
  }) => _call(reportNotificationResultAnswer, 'reportNotificationResult', {
    'id': id,
    'result': result,
  });

  @override
  Future<String> notificationAction({
    required int id,
    required NotificationAction action,
  }) => _call(notificationActionAnswer, 'notificationAction', {
    'id': id,
    'action': action,
  });

  // Views --------------------------------------------------------------------

  @override
  Stream<HomeView> watchHome() => _watch(home, 'watchHome');

  @override
  Stream<InboxView> watchInbox() => _watch(inbox, 'watchInbox');

  @override
  Stream<NoteScreen> watchNote({required String id}) =>
      _watch(note[id], 'watchNote', {'id': id});

  @override
  Stream<NotesListView> watchNotesList({required String folder}) =>
      _watch(notesList[folder], 'watchNotesList', {'folder': folder});

  @override
  Stream<DirectoryView> watchDirectory({
    required DirectoryTab tab,
    required String query,
  }) => _watch(directory[(tab, query)], 'watchDirectory', {
    'tab': tab,
    'query': query,
  });

  @override
  Stream<EntityScreen> watchEntity({required String id}) =>
      _watch(entity[id], 'watchEntity', {'id': id});

  @override
  Stream<TasksView> watchTasks() => _watch(tasks, 'watchTasks');

  @override
  Stream<TaskScreen> watchTask({required String id}) =>
      _watch(task[id], 'watchTask', {'id': id});

  @override
  Stream<SyncStatusView> watchSyncStatus() =>
      _watch(syncStatus, 'watchSyncStatus');

  @override
  Stream<ConflictScreen> watchConflict({required String opId}) =>
      _watch(conflict[opId], 'watchConflict', {'opId': opId});

  @override
  Stream<DuplicatePromptsView> watchDuplicatePrompts() =>
      _watch(duplicatePrompts, 'watchDuplicatePrompts');

  @override
  Stream<SettingsView> watchSettings() => _watch(settings, 'watchSettings');

  @override
  Stream<LocalGraphView> watchLocalGraph({
    required String id,
    required int depth,
  }) => _watch(localGraph[(id, depth)], 'watchLocalGraph', {
    'id': id,
    'depth': depth,
  });

  @override
  Future<GlobalGraphView> globalGraph() =>
      _call(globalGraphAnswer, 'globalGraph');

  @override
  Future<SearchView> search({
    required String query,
    required SearchMode mode,
  }) => _call(searchAnswer, 'search', {'query': query, 'mode': mode});

  @override
  Future<AskView> askView() => _call(askViewAnswer, 'askView');

  @override
  Future<List<EditorHint>> editorHints({required String content}) =>
      _call(editorHintsAnswer, 'editorHints', {'content': content});

  @override
  Future<AdminUsersView> loadAdminUsers() =>
      _call(loadAdminUsersAnswer, 'loadAdminUsers');
}
