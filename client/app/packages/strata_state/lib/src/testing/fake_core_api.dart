import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:strata_bridge/strata_bridge.dart'
    show
        AdminUserItem,
        AdminUsersView,
        AppLifecycle,
        AskScope,
        AskView,
        BlockItem,
        CitationPreview,
        Completions,
        ConflictResolution,
        ConflictScreen,
        CoreConfig,
        CreateOutcome,
        CustodyDraft,
        DirectoryFilter,
        DirectorySort,
        DirectoryTab,
        DirectoryView,
        DocumentDraft,
        DuplicateChoice,
        DuplicatePromptsView,
        EditorHint,
        EntityScreen,
        ExportSummary,
        GlobalGraphView,
        GraphFilter,
        HomeView,
        ImportSummary,
        InboxFilter,
        InboxView,
        LinkOrCreateChoice,
        LocalGraphView,
        MentionEdit,
        MergePreview,
        NavView,
        NewUserRequest,
        NodePosition,
        NoteDiffView,
        NoteScreen,
        NotesListView,
        NotificationAction,
        NotificationOp,
        NotificationResult,
        PasswordStrength,
        PlaceDraft,
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
        SignInRequest,
        SignOutOutcome,
        SignUpOutcome,
        SignUpRequest,
        SuggestionEdits,
        SyncStatusView,
        TagItem,
        TaskDraft,
        TaskDraftPreview,
        TaskHomesView,
        TaskPatch,
        TaskScreen,
        TasksView,
        TimeZoneItem;
import 'package:strata_state/src/core_api.dart';
import 'package:strata_state/src/file_picker.dart';
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
  ///
  /// Live values are delivered synchronously, so one `tester.pump()` after
  /// [add] shows them; the replayed value is delivered asynchronously (a
  /// stream may not emit during `listen`), and live values wait behind it.
  Stream<T> get stream => Stream<T>.multi((subscriber) {
    var replaying = false;
    if (_hasLatest) {
      replaying = true;
      subscriber.add(_latest as T);
      scheduleMicrotask(() => replaying = false);
    }
    final subscription = _controller.stream.listen(
      (value) => replaying ? subscriber.add(value) : subscriber.addSync(value),
      onError: (Object error, StackTrace stackTrace) => replaying
          ? subscriber.addError(error, stackTrace)
          : subscriber.addErrorSync(error, stackTrace),
      onDone: () => replaying ? subscriber.close() : subscriber.closeSync(),
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

/// A scripted [FilePicker]: answers [saveFileAnswer] / [openFileAnswer]
/// and records `saveFile(suggestedName, type)` / `openFile(type)` in
/// [calls].
final class FakeFilePicker implements FilePicker {
  /// Creates a picker recording into [calls].
  new(this.calls);

  /// Where calls are recorded (the owning fake's `calls`).
  final List<CoreCall> calls;

  /// `saveFile`: the chosen path (`null`: cancelled).
  final FakeAnswer<String?> saveFileAnswer = FakeAnswer(
    '/home/shawket/Downloads/strata-vault.zip',
  );

  /// `openFile`: the chosen path (`null`: cancelled).
  final FakeAnswer<String?> openFileAnswer = FakeAnswer(
    '/home/shawket/Downloads/notes.zip',
  );

  @override
  Future<String?> saveFile({
    required String suggestedName,
    required PickedFileType type,
  }) {
    calls.add(
      CoreCall('saveFile', {'suggestedName': suggestedName, 'type': type}),
    );
    return saveFileAnswer._answer();
  }

  @override
  Future<String?> openFile({required PickedFileType type}) {
    calls.add(CoreCall('openFile', {'type': type}));
    return openFileAnswer._answer();
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

  /// Every call made, in order (the file dialogs of [files] included).
  final List<CoreCall> calls = [];

  /// The OS file dialogs the screens open (`filePickerProvider` in
  /// `StrataTestFrame`); their calls land in [calls] too.
  late final FakeFilePicker files = FakeFilePicker(calls);

  // Streams ------------------------------------------------------------------

  /// `watchSession`.
  final FakeStream<SessionState> session = FakeStream();

  /// `watchNotificationOps`.
  final FakeStream<NotificationOp> notificationOps = FakeStream();

  /// `watchHome`.
  final FakeStream<HomeView> home = FakeStream();

  /// `watchInbox`.
  final FakeStream<InboxView> inbox = FakeStream();

  /// `watchNav`.
  final FakeStream<NavView> nav = FakeStream();

  /// `watchTasks`.
  final FakeStream<TasksView> tasks = FakeStream();

  /// `watchTaskHomes`.
  final FakeStream<TaskHomesView> taskHomes = FakeStream();

  /// `watchSyncStatus`.
  final FakeStream<SyncStatusView> syncStatus = FakeStream();

  /// `watchDuplicatePrompts`.
  final FakeStream<DuplicatePromptsView> duplicatePrompts = FakeStream();

  /// `watchSettings`.
  final FakeStream<SettingsView> settings = FakeStream();

  /// `watchAsk`.
  final FakeStream<AskView> askStream = FakeStream();

  /// `watchInboxFiltered`, by `filter`.
  final FakeStreamFamily<InboxFilter, InboxView> inboxFiltered =
      FakeStreamFamily();

  /// `watchRecent`, by `filter`.
  final FakeStreamFamily<RecentFilter, RecentNotesView> recent =
      FakeStreamFamily();

  /// `watchNote`, by `id`.
  final FakeStreamFamily<String, NoteScreen> note = FakeStreamFamily();

  /// `watchNotesList`, by `folder`.
  final FakeStreamFamily<String, NotesListView> notesList = FakeStreamFamily();

  /// `watchDirectory`, by `tab, query`.
  final FakeStreamFamily<(DirectoryTab, String), DirectoryView> directory =
      FakeStreamFamily();

  /// `watchDirectoryFiltered`, by `tab, query, filter, sort`.
  final FakeStreamFamily<
    (DirectoryTab, String, DirectoryFilter, DirectorySort),
    DirectoryView
  >
  directoryFiltered = FakeStreamFamily();

  /// `watchEntity`, by `id`.
  final FakeStreamFamily<String, EntityScreen> entity = FakeStreamFamily();

  /// `watchTask`, by `id`.
  final FakeStreamFamily<String, TaskScreen> task = FakeStreamFamily();

  /// `watchConflict`, by `opId`.
  final FakeStreamFamily<String, ConflictScreen> conflict = FakeStreamFamily();

  /// `watchLocalGraph`, by `id, depth`.
  final FakeStreamFamily<(String, int), LocalGraphView> localGraph =
      FakeStreamFamily();

  /// `watchLocalGraphFiltered`, by `id, depth, edgeKinds.join(',')`.
  final FakeStreamFamily<(String, int, String), LocalGraphView>
  localGraphFiltered = FakeStreamFamily();

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

  /// `checkApproval`.
  final FakeAnswer<SessionState> checkApprovalAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `dismissPending`.
  final FakeAnswer<SessionState> dismissPendingAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `passwordStrength`.
  final FakeAnswer<PasswordStrength> passwordStrengthAnswer = FakeAnswer(
    StrataFixtures.passwordStrength,
  );

  /// `changePassword`.
  final FakeAnswer<SessionState> changePasswordAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `setUiLanguage`.
  final FakeAnswer<void> setUiLanguageAnswer = FakeAnswer(null);

  /// `setTimezone`.
  final FakeAnswer<void> setTimezoneAnswer = FakeAnswer(null);

  /// `setDisplayName`.
  final FakeAnswer<void> setDisplayNameAnswer = FakeAnswer(null);

  /// `downloadExport`.
  final FakeAnswer<ExportSummary> downloadExportAnswer = FakeAnswer(
    StrataFixtures.exportSummary,
  );

  /// `deleteAccountNow`.
  final FakeAnswer<SessionState> deleteAccountNowAnswer = FakeAnswer(
    StrataFixtures.sessionActive,
  );

  /// `exportUnsynced`.
  final FakeAnswer<int> exportUnsyncedAnswer = FakeAnswer(0);

  /// `setSyncPaused`.
  final FakeAnswer<void> setSyncPausedAnswer = FakeAnswer(null);

  /// `capture`.
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

  /// `addReminder`.
  final FakeAnswer<String> addReminderAnswer = FakeAnswer(StrataFixtures.opId);

  /// `removeReminder`.
  final FakeAnswer<String> removeReminderAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `resolveConflict`.
  final FakeAnswer<void> resolveConflictAnswer = FakeAnswer(null);

  /// `resolveDuplicate`.
  final FakeAnswer<void> resolveDuplicateAnswer = FakeAnswer(null);

  /// `dismissRejection`.
  final FakeAnswer<void> dismissRejectionAnswer = FakeAnswer(null);

  /// `setRemindersEnabled`.
  final FakeAnswer<void> setRemindersEnabledAnswer = FakeAnswer(null);

  /// `insertMention`.
  final FakeAnswer<MentionEdit> insertMentionAnswer = FakeAnswer(
    StrataFixtures.mentionEdit,
  );

  /// `pinNote`.
  final FakeAnswer<void> pinNoteAnswer = FakeAnswer(null);

  /// `acceptCapture`.
  final FakeAnswer<List<String>> acceptCaptureAnswer = FakeAnswer(const []);

  /// `rejectCapture`.
  final FakeAnswer<List<String>> rejectCaptureAnswer = FakeAnswer(const []);

  /// `acceptCaptures`.
  final FakeAnswer<List<String>> acceptCapturesAnswer = FakeAnswer(const []);

  /// `acceptAllReady`.
  final FakeAnswer<List<String>> acceptAllReadyAnswer = FakeAnswer(const []);

  /// `acceptSuggestionWith`.
  final FakeAnswer<String> acceptSuggestionWithAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `resolveLinkOrCreate`.
  final FakeAnswer<CreateOutcome> resolveLinkOrCreateAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `acceptSuggestionChoice`.
  final FakeAnswer<String> acceptSuggestionChoiceAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `undoSuggestion`.
  final FakeAnswer<String> undoSuggestionAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `acknowledgeSuggestion`.
  final FakeAnswer<void> acknowledgeSuggestionAnswer = FakeAnswer(null);

  /// `resolveCaptureDuplicate`.
  final FakeAnswer<String> resolveCaptureDuplicateAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `replyToSuggestion`.
  final FakeAnswer<String> replyToSuggestionAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `createDocument`.
  final FakeAnswer<CreateOutcome> createDocumentAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `createPlace`.
  final FakeAnswer<CreateOutcome> createPlaceAnswer = FakeAnswer(
    StrataFixtures.createOutcomeCreated,
  );

  /// `mergeEntities`.
  final FakeAnswer<String> mergeEntitiesAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `repointRelation`.
  final FakeAnswer<String> repointRelationAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `rejectRelation`.
  final FakeAnswer<String> rejectRelationAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `updateUserNotes`.
  final FakeAnswer<String> updateUserNotesAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `setProperty`.
  final FakeAnswer<String> setPropertyAnswer = FakeAnswer(StrataFixtures.opId);

  /// `setPropertyValues`.
  final FakeAnswer<String> setPropertyValuesAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `removeProperty`.
  final FakeAnswer<String> removePropertyAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `addAlias`.
  final FakeAnswer<String> addAliasAnswer = FakeAnswer(StrataFixtures.opId);

  /// `removeAlias`.
  final FakeAnswer<String> removeAliasAnswer = FakeAnswer(StrataFixtures.opId);

  /// `recordCustody`.
  final FakeAnswer<String> recordCustodyAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `setDefaultReminderTime`.
  final FakeAnswer<void> setDefaultReminderTimeAnswer = FakeAnswer(null);

  /// `setQuietHours`.
  final FakeAnswer<void> setQuietHoursAnswer = FakeAnswer(null);

  /// `setSnoozeMinutes`.
  final FakeAnswer<void> setSnoozeMinutesAnswer = FakeAnswer(null);

  /// `refreshSettings`.
  final FakeAnswer<void> refreshSettingsAnswer = FakeAnswer(null);

  /// `retryFailedJobs`.
  final FakeAnswer<int> retryFailedJobsAnswer = FakeAnswer(0);

  /// `renameDevice`.
  final FakeAnswer<void> renameDeviceAnswer = FakeAnswer(null);

  /// `revokeDevice`.
  final FakeAnswer<void> revokeDeviceAnswer = FakeAnswer(null);

  /// `setDeviceReminders`.
  final FakeAnswer<void> setDeviceRemindersAnswer = FakeAnswer(null);

  /// `refreshHistory`.
  final FakeAnswer<void> refreshHistoryAnswer = FakeAnswer(null);

  /// `revertNote`.
  final FakeAnswer<void> revertNoteAnswer = FakeAnswer(null);

  /// `exportVault`.
  final FakeAnswer<ExportSummary> exportVaultAnswer = FakeAnswer(
    StrataFixtures.exportSummary,
  );

  /// `importVault`.
  final FakeAnswer<ImportSummary> importVaultAnswer = FakeAnswer(
    StrataFixtures.importSummary,
  );

  /// `approveUser`.
  final FakeAnswer<AdminUserItem> approveUserAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `rejectUser`.
  final FakeAnswer<AdminUserItem> rejectUserAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `setUserRole`.
  final FakeAnswer<AdminUserItem> setUserRoleAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `setUserEnabled`.
  final FakeAnswer<AdminUserItem> setUserEnabledAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `resetPassword`.
  final FakeAnswer<String> resetPasswordAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `scheduleDeletion`.
  final FakeAnswer<AdminUserItem> scheduleDeletionAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `cancelDeletion`.
  final FakeAnswer<AdminUserItem> cancelDeletionAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `createUser`.
  final FakeAnswer<AdminUserItem> createUserAnswer = FakeAnswer(
    StrataFixtures.adminUserItem,
  );

  /// `ask`.
  final FakeAnswer<String> askAnswer = FakeAnswer(StrataFixtures.opId);

  /// `stopAsk`.
  final FakeAnswer<void> stopAskAnswer = FakeAnswer(null);

  /// `newConversation`.
  final FakeAnswer<void> newConversationAnswer = FakeAnswer(null);

  /// `saveAnswerAsNote`.
  final FakeAnswer<String> saveAnswerAsNoteAnswer = FakeAnswer(
    StrataFixtures.opId,
  );

  /// `refreshAiActivity`.
  final FakeAnswer<void> refreshAiActivityAnswer = FakeAnswer(null);

  /// `rejectAiDecision`.
  final FakeAnswer<void> rejectAiDecisionAnswer = FakeAnswer(null);

  /// `repointAiDecision`.
  final FakeAnswer<void> repointAiDecisionAnswer = FakeAnswer(null);

  /// `retypeAiDecision`.
  final FakeAnswer<void> retypeAiDecisionAnswer = FakeAnswer(null);

  /// `refreshSimilarity`.
  final FakeAnswer<void> refreshSimilarityAnswer = FakeAnswer(null);

  /// `saveLayout`.
  final FakeAnswer<String> saveLayoutAnswer = FakeAnswer(StrataFixtures.opId);

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

  /// `globalGraphFiltered`.
  final FakeAnswer<GlobalGraphView> globalGraphFilteredAnswer = FakeAnswer(
    StrataFixtures.globalGraphView,
  );

  /// `search`.
  final FakeAnswer<SearchView> searchAnswer = FakeAnswer(
    StrataFixtures.searchView,
  );

  /// `searchInFolder`.
  final FakeAnswer<SearchView> searchInFolderAnswer = FakeAnswer(
    StrataFixtures.searchView,
  );

  /// `askView`.
  final FakeAnswer<AskView> askViewAnswer = FakeAnswer(StrataFixtures.askView);

  /// `editorHints`.
  final FakeAnswer<List<EditorHint>> editorHintsAnswer = FakeAnswer(
    StrataFixtures.editorHints,
  );

  /// `editorCompletions`.
  final FakeAnswer<Completions> editorCompletionsAnswer = FakeAnswer(
    StrataFixtures.completions,
  );

  /// `tags`.
  final FakeAnswer<List<TagItem>> tagsAnswer = FakeAnswer(const []);

  /// `noteBlocks`.
  final FakeAnswer<List<BlockItem>> noteBlocksAnswer = FakeAnswer(const []);

  /// `relationTypes`.
  final FakeAnswer<List<RelationTypeItem>> relationTypesAnswer = FakeAnswer(
    const [],
  );

  /// `recurrenceForm`.
  final FakeAnswer<RecurrenceForm?> recurrenceFormAnswer = FakeAnswer(null);

  /// `composeRecurrence`.
  final FakeAnswer<RecurrenceCompose> composeRecurrenceAnswer = FakeAnswer(
    StrataFixtures.recurrenceCompose,
  );

  /// `recurrencePreview`.
  final FakeAnswer<List<RecurrencePreviewItem>> recurrencePreviewAnswer =
      FakeAnswer(const []);

  /// `parseTaskText`.
  final FakeAnswer<TaskDraftPreview> parseTaskTextAnswer = FakeAnswer(
    StrataFixtures.taskDraftPreview,
  );

  /// `placeOptions`.
  final FakeAnswer<List<PlaceOption>> placeOptionsAnswer = FakeAnswer(const []);

  /// `mergePreview`.
  final FakeAnswer<MergePreview> mergePreviewAnswer = FakeAnswer(
    StrataFixtures.mergePreview,
  );

  /// `resolveCitation`.
  final FakeAnswer<CitationPreview> resolveCitationAnswer = FakeAnswer(
    StrataFixtures.citationPreview,
  );

  /// `noteRevisionDiff`.
  final FakeAnswer<NoteDiffView> noteRevisionDiffAnswer = FakeAnswer(
    StrataFixtures.noteDiffView,
  );

  /// `loadAdminUsers`.
  final FakeAnswer<AdminUsersView> loadAdminUsersAnswer = FakeAnswer(
    StrataFixtures.adminUsersView,
  );

  /// `timezones`.
  final FakeAnswer<List<TimeZoneItem>> timezonesAnswer = FakeAnswer(
    StrataFixtures.timezones,
  );

  /// `repointChoices`.
  final FakeAnswer<List<RepointChoice>> repointChoicesAnswer = FakeAnswer(
    StrataFixtures.repointChoices,
  );

  /// Closes every stream.
  void dispose() {
    for (final stream in <FakeStream<Object?>>[
      session,
      notificationOps,
      home,
      inbox,
      nav,
      tasks,
      taskHomes,
      syncStatus,
      duplicatePrompts,
      settings,
      askStream,
    ]) {
      stream.close();
    }
    for (final family in <FakeStreamFamily<Object?, Object?>>[
      inboxFiltered,
      recent,
      note,
      notesList,
      directory,
      directoryFiltered,
      entity,
      task,
      conflict,
      localGraph,
      localGraphFiltered,
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

  @override
  Future<SessionState> checkApproval() =>
      _call(checkApprovalAnswer, 'checkApproval');

  @override
  Future<SessionState> dismissPending() =>
      _call(dismissPendingAnswer, 'dismissPending');

  @override
  Future<PasswordStrength> passwordStrength({required String password}) =>
      _call(passwordStrengthAnswer, 'passwordStrength', {'password': password});

  @override
  Future<SessionState> changePassword({
    required String current,
    required String new_,
  }) => _call(changePasswordAnswer, 'changePassword', {
    'current': current,
    'new_': new_,
  });

  @override
  Future<void> setUiLanguage({required String code}) =>
      _call(setUiLanguageAnswer, 'setUiLanguage', {'code': code});

  @override
  Future<void> setTimezone({required String iana}) =>
      _call(setTimezoneAnswer, 'setTimezone', {'iana': iana});

  @override
  Future<void> setDisplayName({required String name}) =>
      _call(setDisplayNameAnswer, 'setDisplayName', {'name': name});

  @override
  Future<ExportSummary> downloadExport({required String path}) =>
      _call(downloadExportAnswer, 'downloadExport', {'path': path});

  @override
  Future<SessionState> deleteAccountNow({required bool force}) =>
      _call(deleteAccountNowAnswer, 'deleteAccountNow', {'force': force});

  @override
  Future<int> exportUnsynced({required String path}) =>
      _call(exportUnsyncedAnswer, 'exportUnsynced', {'path': path});

  @override
  Future<void> setSyncPaused({required bool paused}) =>
      _call(setSyncPausedAnswer, 'setSyncPaused', {'paused': paused});

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
  Future<String> updateNote({
    required String id,
    required String content,
    String? baseVersion,
  }) => _call(updateNoteAnswer, 'updateNote', {
    'id': id,
    'content': content,
    'baseVersion': baseVersion,
  });

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
  Future<String> addReminder({required String taskId, required DateTime at}) =>
      _call(addReminderAnswer, 'addReminder', {'taskId': taskId, 'at': at});

  @override
  Future<String> removeReminder({
    required String taskId,
    required DateTime at,
  }) => _call(removeReminderAnswer, 'removeReminder', {
    'taskId': taskId,
    'at': at,
  });

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

  @override
  Future<MentionEdit> insertMention({
    required String noteId,
    required String content,
    required int start,
    required int end,
    required String entityId,
  }) => _call(insertMentionAnswer, 'insertMention', {
    'noteId': noteId,
    'content': content,
    'start': start,
    'end': end,
    'entityId': entityId,
  });

  @override
  Future<void> pinNote({required String id, required bool pinned}) =>
      _call(pinNoteAnswer, 'pinNote', {'id': id, 'pinned': pinned});

  @override
  Future<List<String>> acceptCapture({required String noteId}) =>
      _call(acceptCaptureAnswer, 'acceptCapture', {'noteId': noteId});

  @override
  Future<List<String>> rejectCapture({required String noteId}) =>
      _call(rejectCaptureAnswer, 'rejectCapture', {'noteId': noteId});

  @override
  Future<List<String>> acceptCaptures({required List<String> noteIds}) =>
      _call(acceptCapturesAnswer, 'acceptCaptures', {'noteIds': noteIds});

  @override
  Future<List<String>> acceptAllReady() =>
      _call(acceptAllReadyAnswer, 'acceptAllReady');

  @override
  Future<String> acceptSuggestionWith({
    required String id,
    required SuggestionEdits edits,
  }) => _call(acceptSuggestionWithAnswer, 'acceptSuggestionWith', {
    'id': id,
    'edits': edits,
  });

  @override
  Future<CreateOutcome> resolveLinkOrCreate({
    required String id,
    required LinkOrCreateChoice choice,
  }) => _call(resolveLinkOrCreateAnswer, 'resolveLinkOrCreate', {
    'id': id,
    'choice': choice,
  });

  @override
  Future<String> acceptSuggestionChoice({
    required String id,
    required String documentId,
  }) => _call(acceptSuggestionChoiceAnswer, 'acceptSuggestionChoice', {
    'id': id,
    'documentId': documentId,
  });

  @override
  Future<String> undoSuggestion({required String id}) =>
      _call(undoSuggestionAnswer, 'undoSuggestion', {'id': id});

  @override
  Future<void> acknowledgeSuggestion({required String id}) =>
      _call(acknowledgeSuggestionAnswer, 'acknowledgeSuggestion', {'id': id});

  @override
  Future<String> resolveCaptureDuplicate({
    required String id,
    required DuplicateChoice choice,
  }) => _call(resolveCaptureDuplicateAnswer, 'resolveCaptureDuplicate', {
    'id': id,
    'choice': choice,
  });

  @override
  Future<String> replyToSuggestion({
    required String id,
    required String text,
  }) => _call(replyToSuggestionAnswer, 'replyToSuggestion', {
    'id': id,
    'text': text,
  });

  @override
  Future<CreateOutcome> createDocument({
    required DocumentDraft draft,
    required bool force,
  }) => _call(createDocumentAnswer, 'createDocument', {
    'draft': draft,
    'force': force,
  });

  @override
  Future<CreateOutcome> createPlace({
    required PlaceDraft draft,
    required bool force,
  }) =>
      _call(createPlaceAnswer, 'createPlace', {'draft': draft, 'force': force});

  @override
  Future<String> mergeEntities({
    required String sourceId,
    required String intoId,
  }) => _call(mergeEntitiesAnswer, 'mergeEntities', {
    'sourceId': sourceId,
    'intoId': intoId,
  });

  @override
  Future<String> repointRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newDstId,
  }) => _call(repointRelationAnswer, 'repointRelation', {
    'srcId': srcId,
    'dstId': dstId,
    'relType': relType,
    'newDstId': newDstId,
  });

  @override
  Future<String> rejectRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => _call(rejectRelationAnswer, 'rejectRelation', {
    'srcId': srcId,
    'dstId': dstId,
    'relType': relType,
  });

  @override
  Future<String> updateUserNotes({required String id, required String text}) =>
      _call(updateUserNotesAnswer, 'updateUserNotes', {'id': id, 'text': text});

  @override
  Future<String> setProperty({
    required String id,
    required String key,
    required String value,
  }) => _call(setPropertyAnswer, 'setProperty', {
    'id': id,
    'key': key,
    'value': value,
  });

  @override
  Future<String> setPropertyValues({
    required String id,
    required String key,
    required List<String> values,
  }) => _call(setPropertyValuesAnswer, 'setPropertyValues', {
    'id': id,
    'key': key,
    'values': values,
  });

  @override
  Future<String> removeProperty({required String id, required String key}) =>
      _call(removePropertyAnswer, 'removeProperty', {'id': id, 'key': key});

  @override
  Future<String> addAlias({required String id, required String alias}) =>
      _call(addAliasAnswer, 'addAlias', {'id': id, 'alias': alias});

  @override
  Future<String> removeAlias({required String id, required String alias}) =>
      _call(removeAliasAnswer, 'removeAlias', {'id': id, 'alias': alias});

  @override
  Future<String> recordCustody({
    required String documentId,
    required CustodyDraft draft,
  }) => _call(recordCustodyAnswer, 'recordCustody', {
    'documentId': documentId,
    'draft': draft,
  });

  @override
  Future<void> setDefaultReminderTime({required String time}) => _call(
    setDefaultReminderTimeAnswer,
    'setDefaultReminderTime',
    {'time': time},
  );

  @override
  Future<void> setQuietHours({
    required bool enabled,
    required String from,
    required String until,
  }) => _call(setQuietHoursAnswer, 'setQuietHours', {
    'enabled': enabled,
    'from': from,
    'until': until,
  });

  @override
  Future<void> setSnoozeMinutes({required int minutes}) =>
      _call(setSnoozeMinutesAnswer, 'setSnoozeMinutes', {'minutes': minutes});

  @override
  Future<void> refreshSettings() =>
      _call(refreshSettingsAnswer, 'refreshSettings');

  @override
  Future<int> retryFailedJobs() =>
      _call(retryFailedJobsAnswer, 'retryFailedJobs');

  @override
  Future<void> renameDevice({required String id, required String name}) =>
      _call(renameDeviceAnswer, 'renameDevice', {'id': id, 'name': name});

  @override
  Future<void> revokeDevice({required String id}) =>
      _call(revokeDeviceAnswer, 'revokeDevice', {'id': id});

  @override
  Future<void> setDeviceReminders({
    required String id,
    required bool enabled,
  }) => _call(setDeviceRemindersAnswer, 'setDeviceReminders', {
    'id': id,
    'enabled': enabled,
  });

  @override
  Future<void> refreshHistory({required String noteId}) =>
      _call(refreshHistoryAnswer, 'refreshHistory', {'noteId': noteId});

  @override
  Future<void> revertNote({required String noteId, required String commit}) =>
      _call(revertNoteAnswer, 'revertNote', {
        'noteId': noteId,
        'commit': commit,
      });

  @override
  Future<ExportSummary> exportVault({required String path}) =>
      _call(exportVaultAnswer, 'exportVault', {'path': path});

  @override
  Future<ImportSummary> importVault({required String path}) =>
      _call(importVaultAnswer, 'importVault', {'path': path});

  @override
  Future<AdminUserItem> approveUser({required String id}) =>
      _call(approveUserAnswer, 'approveUser', {'id': id});

  @override
  Future<AdminUserItem> rejectUser({required String id}) =>
      _call(rejectUserAnswer, 'rejectUser', {'id': id});

  @override
  Future<AdminUserItem> setUserRole({
    required String id,
    required String role,
  }) => _call(setUserRoleAnswer, 'setUserRole', {'id': id, 'role': role});

  @override
  Future<AdminUserItem> setUserEnabled({
    required String id,
    required bool enabled,
  }) => _call(setUserEnabledAnswer, 'setUserEnabled', {
    'id': id,
    'enabled': enabled,
  });

  @override
  Future<String> resetPassword({required String id}) =>
      _call(resetPasswordAnswer, 'resetPassword', {'id': id});

  @override
  Future<AdminUserItem> scheduleDeletion({required String id}) =>
      _call(scheduleDeletionAnswer, 'scheduleDeletion', {'id': id});

  @override
  Future<AdminUserItem> cancelDeletion({required String id}) =>
      _call(cancelDeletionAnswer, 'cancelDeletion', {'id': id});

  @override
  Future<AdminUserItem> createUser({required NewUserRequest request}) =>
      _call(createUserAnswer, 'createUser', {'request': request});

  @override
  Future<String> ask({required String question, required AskScope scope}) =>
      _call(askAnswer, 'ask', {'question': question, 'scope': scope});

  @override
  Future<void> stopAsk() => _call(stopAskAnswer, 'stopAsk');

  @override
  Future<void> newConversation() =>
      _call(newConversationAnswer, 'newConversation');

  @override
  Future<String> saveAnswerAsNote({required String messageId}) => _call(
    saveAnswerAsNoteAnswer,
    'saveAnswerAsNote',
    {'messageId': messageId},
  );

  @override
  Future<void> refreshAiActivity() =>
      _call(refreshAiActivityAnswer, 'refreshAiActivity');

  @override
  Future<void> rejectAiDecision({required String decisionId}) => _call(
    rejectAiDecisionAnswer,
    'rejectAiDecision',
    {'decisionId': decisionId},
  );

  @override
  Future<void> repointAiDecision({
    required String decisionId,
    required String targetId,
    String? hint,
  }) => _call(repointAiDecisionAnswer, 'repointAiDecision', {
    'decisionId': decisionId,
    'targetId': targetId,
    'hint': hint,
  });

  @override
  Future<void> retypeAiDecision({
    required String decisionId,
    required String relType,
  }) => _call(retypeAiDecisionAnswer, 'retypeAiDecision', {
    'decisionId': decisionId,
    'relType': relType,
  });

  @override
  Future<void> refreshSimilarity() =>
      _call(refreshSimilarityAnswer, 'refreshSimilarity');

  @override
  Future<String> saveLayout({
    required String centerId,
    required String name,
    required List<NodePosition> positions,
  }) => _call(saveLayoutAnswer, 'saveLayout', {
    'centerId': centerId,
    'name': name,
    'positions': positions,
  });

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
  Stream<InboxView> watchInboxFiltered({required InboxFilter filter}) =>
      _watch(inboxFiltered[filter], 'watchInboxFiltered', {'filter': filter});

  @override
  Stream<NavView> watchNav() => _watch(nav, 'watchNav');

  @override
  Stream<RecentNotesView> watchRecent({required RecentFilter filter}) =>
      _watch(recent[filter], 'watchRecent', {'filter': filter});

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
  Stream<DirectoryView> watchDirectoryFiltered({
    required DirectoryTab tab,
    required String query,
    required DirectoryFilter filter,
    required DirectorySort sort,
  }) => _watch(
    directoryFiltered[(tab, query, filter, sort)],
    'watchDirectoryFiltered',
    {'tab': tab, 'query': query, 'filter': filter, 'sort': sort},
  );

  @override
  Stream<EntityScreen> watchEntity({required String id}) =>
      _watch(entity[id], 'watchEntity', {'id': id});

  @override
  Stream<TasksView> watchTasks() => _watch(tasks, 'watchTasks');

  @override
  Stream<TaskScreen> watchTask({required String id}) =>
      _watch(task[id], 'watchTask', {'id': id});

  @override
  Stream<TaskHomesView> watchTaskHomes() => _watch(taskHomes, 'watchTaskHomes');

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
  Stream<LocalGraphView> watchLocalGraphFiltered({
    required String id,
    required int depth,
    required List<String> edgeKinds,
  }) => _watch(
    localGraphFiltered[(id, depth, edgeKinds.join(','))],
    'watchLocalGraphFiltered',
    {'id': id, 'depth': depth, 'edgeKinds': edgeKinds},
  );

  @override
  Future<GlobalGraphView> globalGraph() =>
      _call(globalGraphAnswer, 'globalGraph');

  @override
  Future<GlobalGraphView> globalGraphFiltered({required GraphFilter filter}) =>
      _call(globalGraphFilteredAnswer, 'globalGraphFiltered', {
        'filter': filter,
      });

  @override
  Future<SearchView> search({
    required String query,
    required SearchMode mode,
  }) => _call(searchAnswer, 'search', {'query': query, 'mode': mode});

  @override
  Future<SearchView> searchInFolder({
    required String query,
    required SearchMode mode,
    String? folder,
  }) => _call(searchInFolderAnswer, 'searchInFolder', {
    'query': query,
    'mode': mode,
    'folder': folder,
  });

  @override
  Future<AskView> askView() => _call(askViewAnswer, 'askView');

  @override
  Stream<AskView> watchAsk() => _watch(askStream, 'watchAsk');

  @override
  Future<List<EditorHint>> editorHints({required String content}) =>
      _call(editorHintsAnswer, 'editorHints', {'content': content});

  @override
  Future<Completions> editorCompletions({
    required String noteId,
    required String content,
    required int cursor,
  }) => _call(editorCompletionsAnswer, 'editorCompletions', {
    'noteId': noteId,
    'content': content,
    'cursor': cursor,
  });

  @override
  Future<List<TagItem>> tags({required String prefix}) =>
      _call(tagsAnswer, 'tags', {'prefix': prefix});

  @override
  Future<List<BlockItem>> noteBlocks({required String noteId}) =>
      _call(noteBlocksAnswer, 'noteBlocks', {'noteId': noteId});

  @override
  Future<List<RelationTypeItem>> relationTypes() =>
      _call(relationTypesAnswer, 'relationTypes');

  @override
  Future<RecurrenceForm?> recurrenceForm({required String phrase}) =>
      _call(recurrenceFormAnswer, 'recurrenceForm', {'phrase': phrase});

  @override
  Future<RecurrenceCompose> composeRecurrence({required RecurrenceForm form}) =>
      _call(composeRecurrenceAnswer, 'composeRecurrence', {'form': form});

  @override
  Future<List<RecurrencePreviewItem>> recurrencePreview({
    required String phrase,
    required DateTime from,
    required int count,
  }) => _call(recurrencePreviewAnswer, 'recurrencePreview', {
    'phrase': phrase,
    'from': from,
    'count': count,
  });

  @override
  Future<TaskDraftPreview> parseTaskText({required String text}) =>
      _call(parseTaskTextAnswer, 'parseTaskText', {'text': text});

  @override
  Future<List<PlaceOption>> placeOptions({String? documentId}) =>
      _call(placeOptionsAnswer, 'placeOptions', {'documentId': documentId});

  @override
  Future<MergePreview> mergePreview({
    required String sourceId,
    required String intoId,
  }) => _call(mergePreviewAnswer, 'mergePreview', {
    'sourceId': sourceId,
    'intoId': intoId,
  });

  @override
  Future<CitationPreview> resolveCitation({
    required String noteId,
    String? anchor,
  }) => _call(resolveCitationAnswer, 'resolveCitation', {
    'noteId': noteId,
    'anchor': anchor,
  });

  @override
  Future<NoteDiffView> noteRevisionDiff({
    required String noteId,
    required String commit,
  }) => _call(noteRevisionDiffAnswer, 'noteRevisionDiff', {
    'noteId': noteId,
    'commit': commit,
  });

  @override
  Future<List<TimeZoneItem>> timezones({required String query}) =>
      _call(timezonesAnswer, 'timezones', {'query': query});

  @override
  Future<List<RepointChoice>> repointChoices({
    required String decisionId,
    required String query,
  }) => _call(repointChoicesAnswer, 'repointChoices', {
    'decisionId': decisionId,
    'query': query,
  });

  @override
  Future<AdminUsersView> loadAdminUsers({required String query}) =>
      _call(loadAdminUsersAnswer, 'loadAdminUsers', {'query': query});
}
