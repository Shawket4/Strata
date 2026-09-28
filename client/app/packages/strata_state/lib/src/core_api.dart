/// The seam between the feature UIs and the Rust client core.
library;

import 'package:strata_bridge/strata_bridge.dart';

/// Every function of the flutter_rust_bridge facade (`strata_bridge`'s
/// `lib/src/generated/api/{app,intents,reminders,views}.dart`), mirrored 1:1:
/// same names, same named parameters, same generated types, streams as
/// `Stream<T>`.
///
/// Providers read the core only through this interface so widget tests can
/// swap in `FakeCoreApi` (`package:strata_state/testing.dart`); the app
/// injects `BridgeCoreApi` through `coreApiProvider`. Plumbing only (L15).
///
/// Keep it in sync with the facade: `test/core_api_drift_test.dart` parses the
/// generated files and this one and fails on any added, missing or changed
/// function.
abstract interface class CoreApi {
  // App and session (`api/app.rs`)
  /// Opens the core for this install: the device registry, and the last active
  /// account's database if one is signed in. Returns the session state to route
  /// on.
  Future<SessionState> initCore({required CoreConfig config});

  /// Streams the session state (login screen, main shell, restricted screens).
  Stream<SessionState> watchSession();

  /// Registers an account; it waits for admin approval (D22).
  Future<SignUpOutcome> signUp({required SignUpRequest request});

  /// Signs in as this device.
  Future<SessionState> signIn({required SignInRequest request});

  /// Signs out: `force = false` returns `NeedsConfirmation` while ops are
  /// unsynced.
  Future<SignOutOutcome> signOut({required bool force});

  /// Switches to another account with data on this device.
  Future<SessionState> switchAccount({required String userId});

  /// The user saw the "account disabled" screen: wipes the account's local
  /// data.
  Future<SessionState> acknowledgeAccountDisabled();

  /// Re-reads the profile (`GET /me`).
  Future<void> refreshAccount();

  /// App lifecycle (resume triggers a sync and a reminder refill, §12.4,
  /// §12.5b).
  Future<void> appLifecycle({required AppLifecycle state});

  /// "Sync now".
  Future<void> syncNow();

  /// Pull-to-refresh: reconnects the live channel, syncs and re-reads the
  /// server-only data; completes when done (offline too).
  Future<void> refresh();

  /// "Check again" on the waiting-for-approval screen (uses the sign-in kept in
  /// memory).
  Future<SessionState> checkApproval();

  /// Leaves the waiting-for-approval / rejected screen.
  Future<SessionState> dismissPending();

  /// Password strength for the sign-up meter.
  Future<PasswordStrength> passwordStrength({required String password});

  /// Changes the password (Settings → Account, and the password-change-required
  /// screen).
  Future<SessionState> changePassword({
    required String current,
    required String new_,
  });

  /// Sets the UI language (`en` | `ar`).
  Future<void> setUiLanguage({required String code});

  /// Sets the time zone (IANA name, e.g. `Africa/Cairo`).
  Future<void> setTimezone({required String iana});

  /// Sets the display name.
  Future<void> setDisplayName({required String name});

  /// Downloads the account's export (`GET /me/export`) to a file the user
  /// chose.
  Future<ExportSummary> downloadExport({required String path});

  /// "Delete now" (D25): refused while ops are unsynced unless `force`.
  Future<SessionState> deleteAccountNow({required bool force});

  /// Writes the unsynced ops to a readable file (disabled / deletion-pending
  /// accounts); returns how many.
  Future<int> exportUnsynced({required String path});

  /// Pauses (or resumes) sync.
  Future<void> setSyncPaused({required bool paused});

  // Intents (`api/intents.rs`)
  /// Captures text into the inbox (never refused).
  Future<String> capture({required String text});

  /// Creates a note (duplicate-checked unless `force`).
  Future<CreateOutcome> createNote({
    required String path,
    required String content,
    required bool force,
  });

  /// Saves a note's full content made against `base_version`
  /// (`NoteView::content_version` when the editor loaded it; `None` saves
  /// unconditionally). A stale base is 3-way merged with the changes made
  /// since, or refused with `stale_edit`.
  Future<String> updateNote({
    required String id,
    required String content,
    String? baseVersion,
  });

  /// Moves/renames a note.
  Future<String> moveNote({required String id, required String newPath});

  /// Deletes a note.
  Future<String> deleteNote({required String id});

  /// Creates a person, company or concept (`kind` = `person` | `company` |
  /// `concept`).
  Future<CreateOutcome> createEntity({
    required String kind,
    required String name,
    required List<String> aliases,
    required bool force,
  });

  /// Adds a relation.
  Future<String> addRelation({
    required String srcId,
    required String dstId,
    required String relType,
  });

  /// Removes a relation.
  Future<String> removeRelation({
    required String srcId,
    required String dstId,
    required String relType,
  });

  /// Changes a relation's type.
  Future<String> retypeRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newType,
  });

  /// Accepts a suggestion.
  Future<String> acceptSuggestion({required String id});

  /// Rejects a suggestion.
  Future<String> rejectSuggestion({required String id});

  /// Queues a relink request.
  Future<String> requestRelink({required String noteId});

  /// Creates a task (duplicate-checked unless `force`).
  Future<CreateOutcome> createTask({
    required TaskDraft draft,
    required bool force,
  });

  /// Edits a task's fields.
  Future<String> updateTask({required String taskId, required TaskPatch patch});

  /// Completes a task (a recurring one gets its next occurrence).
  Future<String> completeTask({required String taskId});

  /// Cancels a task.
  Future<String> cancelTask({required String taskId});

  /// Reopens a task.
  Future<String> reopenTask({required String taskId});

  /// Deletes a task.
  Future<String> deleteTask({required String taskId});

  /// Adds a reminder to a task (local wall-clock time).
  Future<String> addReminder({required String taskId, required DateTime at});

  /// Removes a reminder (`ReminderItem::local_at`).
  Future<String> removeReminder({required String taskId, required DateTime at});

  /// Resolves a sync conflict (D19).
  Future<void> resolveConflict({
    required String opId,
    required ConflictResolution resolution,
  });

  /// Answers an "Already exists" prompt from a push.
  Future<void> resolveDuplicate({
    required String opId,
    required DuplicateChoice choice,
  });

  /// Dismisses a rolled-back op's notice.
  Future<void> dismissRejection({required String opId});

  /// Reminders on/off for this device.
  Future<void> setRemindersEnabled({required bool enabled});

  /// Inserts an `@mention` (UTF-16 range `start..end` of the typed `@query`) as
  /// a link and adds the entity to `people:`/`companies:`: the new content and
  /// caret, saved with `update_note`.
  Future<MentionEdit> insertMention({
    required String noteId,
    required String content,
    required int start,
    required int end,
    required String entityId,
  });

  /// Pins a note to the sidebar (this device) or unpins it.
  Future<void> pinNote({required String id, required bool pinned});

  /// Accepts what the AI proposed for a capture (every suggestion that needs no
  /// choice).
  Future<List<String>> acceptCapture({required String noteId});

  /// Rejects every pending suggestion of a capture.
  Future<List<String>> rejectCapture({required String noteId});

  /// Accepts the listed captures that are ready (bulk bar).
  Future<List<String>> acceptCaptures({required List<String> noteIds});

  /// "Accept all ready".
  Future<List<String>> acceptAllReady();

  /// Accepts a proposal with the user's edits.
  Future<String> acceptSuggestionWith({
    required String id,
    required SuggestionEdits edits,
  });

  /// "Who is “بابا”?": link to an existing entity or create one (the mention
  /// becomes an alias).
  Future<CreateOutcome> resolveLinkOrCreate({
    required String id,
    required LinkOrCreateChoice choice,
  });

  /// Picks the document of an ambiguous custody suggestion.
  Future<String> acceptSuggestionChoice({
    required String id,
    required String documentId,
  });

  /// Undo on a suggestion (rejects a pending one; `not_available` for an AI
  /// change applied automatically until the server's undo endpoint exists).
  Future<String> undoSuggestion({required String id});

  /// "Looks right" on an AI change applied automatically.
  Future<void> acknowledgeSuggestion({required String id});

  /// A capture flagged as a duplicate: keep it or discard it.
  Future<String> resolveCaptureDuplicate({
    required String id,
    required DuplicateChoice choice,
  });

  /// Replies in a suggestion's thread.
  Future<String> replyToSuggestion({required String id, required String text});

  /// Creates a document (duplicate-checked unless `force`).
  Future<CreateOutcome> createDocument({
    required DocumentDraft draft,
    required bool force,
  });

  /// Creates a place (duplicate-checked unless `force`).
  Future<CreateOutcome> createPlace({
    required PlaceDraft draft,
    required bool force,
  });

  /// Merges entity `source_id` into `into_id`.
  Future<String> mergeEntities({
    required String sourceId,
    required String intoId,
  });

  /// Points a relation at another target (D13 "this Ahmed is Ahmed Fathy").
  Future<String> repointRelation({
    required String srcId,
    required String dstId,
    required String relType,
    required String newDstId,
  });

  /// Rejects a relation (an AI edge is recorded as rejected and never
  /// re-proposed; undo with `add_relation`).
  Future<String> rejectRelation({
    required String srcId,
    required String dstId,
    required String relType,
  });

  /// Replaces the user-owned `## Notes` section of an entity, document or
  /// place.
  Future<String> updateUserNotes({required String id, required String text});

  /// Sets a property of an entity, document or place.
  Future<String> setProperty({
    required String id,
    required String key,
    required String value,
  });

  /// Sets a property to a list of values (several phone numbers, aliases,
  /// tags); the list replaces the whole value, an empty list removes it.
  Future<String> setPropertyValues({
    required String id,
    required String key,
    required List<String> values,
  });

  /// Removes a property.
  Future<String> removeProperty({required String id, required String key});

  /// Adds an alias.
  Future<String> addAlias({required String id, required String alias});

  /// Removes an alias.
  Future<String> removeAlias({required String id, required String alias});

  /// Records a custody event of a document ("Record a move").
  Future<String> recordCustody({
    required String documentId,
    required CustodyDraft draft,
  });

  /// Time used for date-only reminders (`HH:MM`).
  Future<void> setDefaultReminderTime({required String time});

  /// Quiet hours (`HH:MM`).
  Future<void> setQuietHours({
    required bool enabled,
    required String from,
    required String until,
  });

  /// The notification's Snooze length (minutes).
  Future<void> setSnoozeMinutes({required int minutes});

  /// Re-reads devices, AI status, integrity warnings and the approval count
  /// (online).
  Future<void> refreshSettings();

  /// Queues the account's failed AI jobs again and re-reads the AI status;
  /// returns how many were queued.
  Future<int> retryFailedJobs();

  /// Renames a device.
  Future<void> renameDevice({required String id, required String name});

  /// Signs another device out.
  Future<void> revokeDevice({required String id});

  /// Reminders on/off for a device ("Deliver to").
  Future<void> setDeviceReminders({required String id, required bool enabled});

  /// Fetches a note's history (online).
  Future<void> refreshHistory({required String noteId});

  /// Reverts a note to a revision (online).
  Future<void> revertNote({required String noteId, required String commit});

  /// Downloads the vault export to a file the user chose.
  Future<ExportSummary> exportVault({required String path});

  /// Imports a zip archive.
  Future<ImportSummary> importVault({required String path});

  /// Admin: approves a sign-up.
  Future<AdminUserItem> approveUser({required String id});

  /// Admin: rejects a sign-up.
  Future<AdminUserItem> rejectUser({required String id});

  /// Admin: sets a user's role (`admin` | `member`).
  Future<AdminUserItem> setUserRole({required String id, required String role});

  /// Admin: disables (`enabled = false`) or enables an account.
  Future<AdminUserItem> setUserEnabled({
    required String id,
    required bool enabled,
  });

  /// Admin: resets a password; returns the one-time temporary password.
  Future<String> resetPassword({required String id});

  /// Admin: schedules an account's deletion (D25).
  Future<AdminUserItem> scheduleDeletion({required String id});

  /// Admin: cancels a scheduled deletion.
  Future<AdminUserItem> cancelDeletion({required String id});

  /// Admin: creates an active account.
  Future<AdminUserItem> createUser({required NewUserRequest request});

  /// Ask: asks a question in `scope`; the answer streams into `watch_ask`.
  /// Returns the answer's ID when it ended.
  Future<String> ask({required String question, required AskScope scope});

  /// Ask: stops the streaming answer.
  Future<void> stopAsk();

  /// Ask: starts a new conversation.
  Future<void> newConversation();

  /// Ask: saves an answer as a note (§9.5); returns the note's ID.
  Future<String> saveAnswerAsNote({required String messageId});

  /// Home: re-reads the AI activity feed (the server's AI decisions).
  Future<void> refreshAiActivity();

  /// D13: undoes an AI decision (the server reverts it and never re-proposes
  /// it).
  Future<void> rejectAiDecision({required String decisionId});

  /// D13: points an AI decision at another entity; `hint` is the user's short
  /// explanation.
  Future<void> repointAiDecision({
    required String decisionId,
    required String targetId,
    String? hint,
  });

  /// D13: changes the type of an AI relation.
  Future<void> retypeAiDecision({
    required String decisionId,
    required String relType,
  });

  /// Map: fetches the similarity edges shown by the similarity lens (online
  /// only).
  Future<void> refreshSimilarity();

  /// Map: saves the arranged local map as `maps/<name>.canvas`; returns the
  /// map's path.
  Future<String> saveLayout({
    required String centerId,
    required String name,
    required List<NodePosition> positions,
  });

  // Reminders (`api/reminders.rs`)
  /// The notification-ops stream (schedule / update / cancel / show now). It
  /// does not depend on a session: while signed out it stays open and receives
  /// nothing; the ops of whichever account signs in arrive on it.
  Stream<NotificationOp> watchNotificationOps();

  /// The platform's result of an op.
  Future<void> reportNotificationResult({
    required int id,
    required NotificationResult result,
  });

  /// A notification action (Done / Snooze), applied through the outbox.
  Future<String> notificationAction({
    required int id,
    required NotificationAction action,
  });

  // Views (`api/views.rs`)
  /// Home: recent notes, inbox count, task sections, sync pill.
  Stream<HomeView> watchHome();

  /// Inbox: captures with suggestions, other suggestions.
  Stream<InboxView> watchInbox();

  /// Inbox with a filter tab (All / Needs you / Conflicts).
  Stream<InboxView> watchInboxFiltered({required InboxFilter filter});

  /// Navigation counts and pinned notes (sidebar, rail, bottom bar).
  Stream<NavView> watchNav();

  /// The "Recent" block with a filter (Edited / Created / Filed by AI).
  Stream<RecentNotesView> watchRecent({required RecentFilter filter});

  /// A note (editor, properties, relations, backlinks, tasks).
  Stream<NoteScreen> watchNote({required String id});

  /// A folder of the notes tree (`""` = vault root).
  Stream<NotesListView> watchNotesList({required String folder});

  /// A directory tab, filtered by `query`.
  Stream<DirectoryView> watchDirectory({
    required DirectoryTab tab,
    required String query,
  });

  /// A directory tab with filters and an order.
  Stream<DirectoryView> watchDirectoryFiltered({
    required DirectoryTab tab,
    required String query,
    required DirectoryFilter filter,
    required DirectorySort sort,
  });

  /// A person, company, document or place page.
  Stream<EntityScreen> watchEntity({required String id});

  /// The Tasks destination.
  Stream<TasksView> watchTasks();

  /// Task detail.
  Stream<TaskScreen> watchTask({required String id});

  /// Candidate homes of a new task.
  Stream<TaskHomesView> watchTaskHomes();

  /// Sync status and conflicts.
  Stream<SyncStatusView> watchSyncStatus();

  /// One conflict (side by side, D19).
  Stream<ConflictScreen> watchConflict({required String opId});

  /// Open "Already exists" prompts.
  Stream<DuplicatePromptsView> watchDuplicatePrompts();

  /// Settings.
  Stream<SettingsView> watchSettings();

  /// A note's neighbourhood laid out radially (local mind map).
  Stream<LocalGraphView> watchLocalGraph({
    required String id,
    required int depth,
  });

  /// A note's local mind map with only the edges of `edge_kinds` (families such
  /// as `link`, or full kinds such as `relation:supports`; empty = all),
  /// filtered in the core.
  Stream<LocalGraphView> watchLocalGraphFiltered({
    required String id,
    required int depth,
    required List<String> edgeKinds,
  });

  /// The global map (positions from the cached, warm-started force layout).
  Future<GlobalGraphView> globalGraph();

  /// The global map with filters and a lens applied in the core.
  Future<GlobalGraphView> globalGraphFiltered({required GraphFilter filter});

  /// Search: keyword locally (offline too); semantic and hybrid on the server.
  Future<SearchView> search({required String query, required SearchMode mode});

  /// Search limited to a folder (and its subfolders).
  Future<SearchView> searchInFolder({
    required String query,
    required SearchMode mode,
    String? folder,
  });

  /// Ask (one-shot read of the conversation).
  Future<AskView> askView();

  /// Ask: the conversation as it streams.
  Stream<AskView> watchAsk();

  /// Editor highlight spans for text being typed (UTF-16 offsets); with a
  /// signed-in session, wikilinks are resolved to note IDs as seen from the
  /// note at `path` (`""` = vault root).
  Future<List<EditorHint>> editorHints({required String content});

  /// Editor completions at the caret (`[[`, `[[Note#^`, `@`, `#`), UTF-16
  /// `cursor`.
  Future<Completions> editorCompletions({
    required String noteId,
    required String content,
    required int cursor,
  });

  /// Vault tags with note counts, filtered by prefix.
  Future<List<TagItem>> tags({required String prefix});

  /// The blocks of a note (block reference picker).
  Future<List<BlockItem>> noteBlocks({required String noteId});

  /// Relation types the user can pick, with labels in the UI language.
  Future<List<RelationTypeItem>> relationTypes();

  /// A recurrence phrase as the editor's form (`None`: outside the grammar).
  Future<RecurrenceForm?> recurrenceForm({required String phrase});

  /// Compiles the editor's form to its phrase and summary.
  Future<RecurrenceCompose> composeRecurrence({required RecurrenceForm form});

  /// The first `count` occurrences of `phrase` from `from` (the due date).
  Future<List<RecurrencePreviewItem>> recurrencePreview({
    required String phrase,
    required DateTime from,
    required int count,
  });

  /// A new task's text as the core understands it ("Understood as" chips).
  Future<TaskDraftPreview> parseTaskText({required String text});

  /// Places for the custody picker (`document_id` marks the document's current
  /// place).
  Future<List<PlaceOption>> placeOptions({String? documentId});

  /// What merging `source_id` into `into_id` moves.
  Future<MergePreview> mergePreview({
    required String sourceId,
    required String intoId,
  });

  /// The block a citation points to (source preview).
  Future<CitationPreview> resolveCitation({
    required String noteId,
    String? anchor,
  });

  /// A revision compared with the note's current content (online).
  Future<NoteDiffView> noteRevisionDiff({
    required String noteId,
    required String commit,
  });

  /// The time zones of the Settings picker matching `query`, in the UI
  /// language, sorted by offset (`setTimezone(iana: item.id)` applies one).
  Future<List<TimeZoneItem>> timezones({required String query});

  /// New targets for an AI decision of Home's activity feed (Repoint).
  Future<List<RepointChoice>> repointChoices({
    required String decisionId,
    required String query,
  });

  /// Admin → Users (online, admins only), filtered by `query`.
  Future<AdminUsersView> loadAdminUsers({required String query});
}
