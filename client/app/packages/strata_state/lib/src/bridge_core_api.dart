import 'package:strata_bridge/strata_bridge.dart' as bridge;
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

  @override
  Future<SessionState> checkApproval() => bridge.checkApproval();

  @override
  Future<SessionState> dismissPending() => bridge.dismissPending();

  @override
  Future<PasswordStrength> passwordStrength({required String password}) =>
      bridge.passwordStrength(password: password);

  @override
  Future<SessionState> changePassword({
    required String current,
    required String new_,
  }) => bridge.changePassword(current: current, new_: new_);

  @override
  Future<void> setUiLanguage({required String code}) =>
      bridge.setUiLanguage(code: code);

  @override
  Future<void> setTimezone({required String iana}) =>
      bridge.setTimezone(iana: iana);

  @override
  Future<void> setDisplayName({required String name}) =>
      bridge.setDisplayName(name: name);

  @override
  Future<ExportSummary> downloadExport({required String path}) =>
      bridge.downloadExport(path: path);

  @override
  Future<SessionState> deleteAccountNow({required bool force}) =>
      bridge.deleteAccountNow(force: force);

  @override
  Future<int> exportUnsynced({required String path}) =>
      bridge.exportUnsynced(path: path);

  @override
  Future<void> setSyncPaused({required bool paused}) =>
      bridge.setSyncPaused(paused: paused);

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
  Future<String> updateNote({
    required String id,
    required String content,
    String? baseVersion,
  }) => bridge.updateNote(id: id, content: content, baseVersion: baseVersion);

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
  Future<String> addReminder({required String taskId, required DateTime at}) =>
      bridge.addReminder(taskId: taskId, at: at);

  @override
  Future<String> removeReminder({
    required String taskId,
    required DateTime at,
  }) => bridge.removeReminder(taskId: taskId, at: at);

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

  @override
  Future<MentionEdit> insertMention({
    required String noteId,
    required String content,
    required int start,
    required int end,
    required String entityId,
  }) => bridge.insertMention(
    noteId: noteId,
    content: content,
    start: start,
    end: end,
    entityId: entityId,
  );

  @override
  Future<void> pinNote({required String id, required bool pinned}) =>
      bridge.pinNote(id: id, pinned: pinned);

  @override
  Future<List<String>> acceptCapture({required String noteId}) =>
      bridge.acceptCapture(noteId: noteId);

  @override
  Future<List<String>> rejectCapture({required String noteId}) =>
      bridge.rejectCapture(noteId: noteId);

  @override
  Future<List<String>> acceptCaptures({required List<String> noteIds}) =>
      bridge.acceptCaptures(noteIds: noteIds);

  @override
  Future<List<String>> acceptAllReady() => bridge.acceptAllReady();

  @override
  Future<String> acceptSuggestionWith({
    required String id,
    required SuggestionEdits edits,
  }) => bridge.acceptSuggestionWith(id: id, edits: edits);

  @override
  Future<CreateOutcome> resolveLinkOrCreate({
    required String id,
    required LinkOrCreateChoice choice,
  }) => bridge.resolveLinkOrCreate(id: id, choice: choice);

  @override
  Future<String> acceptSuggestionChoice({
    required String id,
    required String documentId,
  }) => bridge.acceptSuggestionChoice(id: id, documentId: documentId);

  @override
  Future<String> undoSuggestion({required String id}) =>
      bridge.undoSuggestion(id: id);

  @override
  Future<void> acknowledgeSuggestion({required String id}) =>
      bridge.acknowledgeSuggestion(id: id);

  @override
  Future<String> resolveCaptureDuplicate({
    required String id,
    required DuplicateChoice choice,
  }) => bridge.resolveCaptureDuplicate(id: id, choice: choice);

  @override
  Future<String> replyToSuggestion({
    required String id,
    required String text,
  }) => bridge.replyToSuggestion(id: id, text: text);

  @override
  Future<CreateOutcome> createDocument({
    required DocumentDraft draft,
    required bool force,
  }) => bridge.createDocument(draft: draft, force: force);

  @override
  Future<CreateOutcome> createPlace({
    required PlaceDraft draft,
    required bool force,
  }) => bridge.createPlace(draft: draft, force: force);

  @override
  Future<String> mergeEntities({
    required String sourceId,
    required String intoId,
  }) => bridge.mergeEntities(sourceId: sourceId, intoId: intoId);

  @override
  Future<String> repointRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newDstId,
  }) => bridge.repointRelation(
    srcId: srcId,
    dstId: dstId,
    relType: relType,
    newDstId: newDstId,
  );

  @override
  Future<String> rejectRelation({
    required String srcId,
    required String dstId,
    required String relType,
  }) => bridge.rejectRelation(srcId: srcId, dstId: dstId, relType: relType);

  @override
  Future<String> updateUserNotes({required String id, required String text}) =>
      bridge.updateUserNotes(id: id, text: text);

  @override
  Future<String> setProperty({
    required String id,
    required String key,
    required String value,
  }) => bridge.setProperty(id: id, key: key, value: value);

  @override
  Future<String> setPropertyValues({
    required String id,
    required String key,
    required List<String> values,
  }) => bridge.setPropertyValues(id: id, key: key, values: values);

  @override
  Future<String> removeProperty({required String id, required String key}) =>
      bridge.removeProperty(id: id, key: key);

  @override
  Future<String> addAlias({required String id, required String alias}) =>
      bridge.addAlias(id: id, alias: alias);

  @override
  Future<String> removeAlias({required String id, required String alias}) =>
      bridge.removeAlias(id: id, alias: alias);

  @override
  Future<String> recordCustody({
    required String documentId,
    required CustodyDraft draft,
  }) => bridge.recordCustody(documentId: documentId, draft: draft);

  @override
  Future<void> setDefaultReminderTime({required String time}) =>
      bridge.setDefaultReminderTime(time: time);

  @override
  Future<void> setQuietHours({
    required bool enabled,
    required String from,
    required String until,
  }) => bridge.setQuietHours(enabled: enabled, from: from, until: until);

  @override
  Future<void> setSnoozeMinutes({required int minutes}) =>
      bridge.setSnoozeMinutes(minutes: minutes);

  @override
  Future<void> refreshSettings() => bridge.refreshSettings();

  @override
  Future<int> retryFailedJobs() => bridge.retryFailedJobs();

  @override
  Future<void> renameDevice({required String id, required String name}) =>
      bridge.renameDevice(id: id, name: name);

  @override
  Future<void> revokeDevice({required String id}) =>
      bridge.revokeDevice(id: id);

  @override
  Future<void> setDeviceReminders({
    required String id,
    required bool enabled,
  }) => bridge.setDeviceReminders(id: id, enabled: enabled);

  @override
  Future<void> refreshHistory({required String noteId}) =>
      bridge.refreshHistory(noteId: noteId);

  @override
  Future<void> revertNote({required String noteId, required String commit}) =>
      bridge.revertNote(noteId: noteId, commit: commit);

  @override
  Future<ExportSummary> exportVault({required String path}) =>
      bridge.exportVault(path: path);

  @override
  Future<ImportSummary> importVault({required String path}) =>
      bridge.importVault(path: path);

  @override
  Future<AdminUserItem> approveUser({required String id}) =>
      bridge.approveUser(id: id);

  @override
  Future<AdminUserItem> rejectUser({required String id}) =>
      bridge.rejectUser(id: id);

  @override
  Future<AdminUserItem> setUserRole({
    required String id,
    required String role,
  }) => bridge.setUserRole(id: id, role: role);

  @override
  Future<AdminUserItem> setUserEnabled({
    required String id,
    required bool enabled,
  }) => bridge.setUserEnabled(id: id, enabled: enabled);

  @override
  Future<String> resetPassword({required String id}) =>
      bridge.resetPassword(id: id);

  @override
  Future<AdminUserItem> scheduleDeletion({required String id}) =>
      bridge.scheduleDeletion(id: id);

  @override
  Future<AdminUserItem> cancelDeletion({required String id}) =>
      bridge.cancelDeletion(id: id);

  @override
  Future<AdminUserItem> createUser({required NewUserRequest request}) =>
      bridge.createUser(request: request);

  @override
  Future<String> ask({required String question, required AskScope scope}) =>
      bridge.ask(question: question, scope: scope);

  @override
  Future<void> stopAsk() => bridge.stopAsk();

  @override
  Future<void> newConversation() => bridge.newConversation();

  @override
  Future<String> saveAnswerAsNote({required String messageId}) =>
      bridge.saveAnswerAsNote(messageId: messageId);

  @override
  Future<void> refreshAiActivity() => bridge.refreshAiActivity();

  @override
  Future<void> rejectAiDecision({required String decisionId}) =>
      bridge.rejectAiDecision(decisionId: decisionId);

  @override
  Future<void> repointAiDecision({
    required String decisionId,
    required String targetId,
    String? hint,
  }) => bridge.repointAiDecision(
    decisionId: decisionId,
    targetId: targetId,
    hint: hint,
  );

  @override
  Future<void> retypeAiDecision({
    required String decisionId,
    required String relType,
  }) => bridge.retypeAiDecision(decisionId: decisionId, relType: relType);

  @override
  Future<void> refreshSimilarity() => bridge.refreshSimilarity();

  @override
  Future<String> saveLayout({
    required String centerId,
    required String name,
    required List<NodePosition> positions,
  }) => bridge.saveLayout(centerId: centerId, name: name, positions: positions);

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
  Stream<InboxView> watchInboxFiltered({required InboxFilter filter}) =>
      bridge.watchInboxFiltered(filter: filter);

  @override
  Stream<NavView> watchNav() => bridge.watchNav();

  @override
  Stream<RecentNotesView> watchRecent({required RecentFilter filter}) =>
      bridge.watchRecent(filter: filter);

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
  Stream<DirectoryView> watchDirectoryFiltered({
    required DirectoryTab tab,
    required String query,
    required DirectoryFilter filter,
    required DirectorySort sort,
  }) => bridge.watchDirectoryFiltered(
    tab: tab,
    query: query,
    filter: filter,
    sort: sort,
  );

  @override
  Stream<EntityScreen> watchEntity({required String id}) =>
      bridge.watchEntity(id: id);

  @override
  Stream<TasksView> watchTasks() => bridge.watchTasks();

  @override
  Stream<TaskScreen> watchTask({required String id}) =>
      bridge.watchTask(id: id);

  @override
  Stream<TaskHomesView> watchTaskHomes() => bridge.watchTaskHomes();

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
  Stream<LocalGraphView> watchLocalGraphFiltered({
    required String id,
    required int depth,
    required List<String> edgeKinds,
  }) => bridge.watchLocalGraphFiltered(
    id: id,
    depth: depth,
    edgeKinds: edgeKinds,
  );

  @override
  Future<GlobalGraphView> globalGraph() => bridge.globalGraph();

  @override
  Future<GlobalGraphView> globalGraphFiltered({required GraphFilter filter}) =>
      bridge.globalGraphFiltered(filter: filter);

  @override
  Future<SearchView> search({
    required String query,
    required SearchMode mode,
  }) => bridge.search(query: query, mode: mode);

  @override
  Future<SearchView> searchInFolder({
    required String query,
    required SearchMode mode,
    String? folder,
  }) => bridge.searchInFolder(query: query, mode: mode, folder: folder);

  @override
  Future<AskView> askView() => bridge.askView();

  @override
  Stream<AskView> watchAsk() => bridge.watchAsk();

  @override
  Future<List<EditorHint>> editorHints({required String content}) =>
      bridge.editorHints(content: content);

  @override
  Future<Completions> editorCompletions({
    required String noteId,
    required String content,
    required int cursor,
  }) => bridge.editorCompletions(
    noteId: noteId,
    content: content,
    cursor: cursor,
  );

  @override
  Future<List<TagItem>> tags({required String prefix}) =>
      bridge.tags(prefix: prefix);

  @override
  Future<List<BlockItem>> noteBlocks({required String noteId}) =>
      bridge.noteBlocks(noteId: noteId);

  @override
  Future<List<RelationTypeItem>> relationTypes() => bridge.relationTypes();

  @override
  Future<RecurrenceForm?> recurrenceForm({required String phrase}) =>
      bridge.recurrenceForm(phrase: phrase);

  @override
  Future<RecurrenceCompose> composeRecurrence({required RecurrenceForm form}) =>
      bridge.composeRecurrence(form: form);

  @override
  Future<List<RecurrencePreviewItem>> recurrencePreview({
    required String phrase,
    required DateTime from,
    required int count,
  }) => bridge.recurrencePreview(phrase: phrase, from: from, count: count);

  @override
  Future<TaskDraftPreview> parseTaskText({required String text}) =>
      bridge.parseTaskText(text: text);

  @override
  Future<List<PlaceOption>> placeOptions({String? documentId}) =>
      bridge.placeOptions(documentId: documentId);

  @override
  Future<MergePreview> mergePreview({
    required String sourceId,
    required String intoId,
  }) => bridge.mergePreview(sourceId: sourceId, intoId: intoId);

  @override
  Future<CitationPreview> resolveCitation({
    required String noteId,
    String? anchor,
  }) => bridge.resolveCitation(noteId: noteId, anchor: anchor);

  @override
  Future<NoteDiffView> noteRevisionDiff({
    required String noteId,
    required String commit,
  }) => bridge.noteRevisionDiff(noteId: noteId, commit: commit);

  @override
  Future<List<TimeZoneItem>> timezones({required String query}) =>
      bridge.timezones(query: query);

  @override
  Future<List<RepointChoice>> repointChoices({
    required String decisionId,
    required String query,
  }) => bridge.repointChoices(decisionId: decisionId, query: query);

  @override
  Future<AdminUsersView> loadAdminUsers({required String query}) =>
      bridge.loadAdminUsers(query: query);
}
