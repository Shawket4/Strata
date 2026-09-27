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

  // Intents (`api/intents.rs`)
  /// Captures text into the inbox (never refused).
  Future<String> capture({required String text});

  /// Creates a note (duplicate-checked unless `force`).
  Future<CreateOutcome> createNote({
    required String path,
    required String content,
    required bool force,
  });

  /// Saves a note's content.
  Future<String> updateNote({required String id, required String content});

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

  // Reminders (`api/reminders.rs`)
  /// The notification-ops stream (schedule / update / cancel / show now).
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

  /// A note (editor, properties, relations, backlinks, tasks).
  Stream<NoteScreen> watchNote({required String id});

  /// A folder of the notes tree (`""` = vault root).
  Stream<NotesListView> watchNotesList({required String folder});

  /// A directory tab, filtered by `query`.
  Stream<DirectoryView> watchDirectory({
    required DirectoryTab tab,
    required String query,
  });

  /// A person, company, document or place page.
  Stream<EntityScreen> watchEntity({required String id});

  /// The Tasks destination.
  Stream<TasksView> watchTasks();

  /// Task detail.
  Stream<TaskScreen> watchTask({required String id});

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

  /// The global map (positions from the cached, warm-started force layout).
  Future<GlobalGraphView> globalGraph();

  /// Local search.
  Future<SearchView> search({required String query, required SearchMode mode});

  /// Ask (online only; the `/ask` stream is not in the contract yet).
  Future<AskView> askView();

  /// Editor highlight spans for text being typed (UTF-16 offsets).
  Future<List<EditorHint>> editorHints({required String content});

  /// Admin → Users (online, admins only).
  Future<AdminUsersView> loadAdminUsers();
}
