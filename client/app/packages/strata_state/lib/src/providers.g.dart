// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'providers.dart';

// **************************************************************************
// RiverpodGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint, type=warning
/// The Rust client core, as a [CoreApi].
///
/// Has no default: the app overrides it with `BridgeCoreApi`, tests with
/// `FakeCoreApi` (`package:strata_state/testing.dart`).

@ProviderFor(coreApi)
final coreApiProvider = CoreApiProvider._();

/// The Rust client core, as a [CoreApi].
///
/// Has no default: the app overrides it with `BridgeCoreApi`, tests with
/// `FakeCoreApi` (`package:strata_state/testing.dart`).

final class CoreApiProvider
    extends $FunctionalProvider<CoreApi, CoreApi, CoreApi>
    with $Provider<CoreApi> {
  /// The Rust client core, as a [CoreApi].
  ///
  /// Has no default: the app overrides it with `BridgeCoreApi`, tests with
  /// `FakeCoreApi` (`package:strata_state/testing.dart`).
  CoreApiProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'coreApiProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$coreApiHash();

  @$internal
  @override
  $ProviderElement<CoreApi> $createElement($ProviderPointer pointer) =>
      $ProviderElement(pointer);

  @override
  CoreApi create(Ref ref) {
    return coreApi(ref);
  }

  /// {@macro riverpod.override_with_value}
  Override overrideWithValue(CoreApi value) {
    return $ProviderOverride(
      origin: this,
      providerOverride: $SyncValueProvider<CoreApi>(value),
    );
  }
}

String _$coreApiHash() => r'f63a0489ec8b41ada476807c7523343d8e2b698f';

/// The session state: login screen, main shell or a restricted screen
/// (`CoreApi.watchSession`).

@ProviderFor(session)
final sessionProvider = SessionProvider._();

/// The session state: login screen, main shell or a restricted screen
/// (`CoreApi.watchSession`).

final class SessionProvider
    extends
        $FunctionalProvider<
          AsyncValue<SessionState>,
          SessionState,
          Stream<SessionState>
        >
    with $FutureModifier<SessionState>, $StreamProvider<SessionState> {
  /// The session state: login screen, main shell or a restricted screen
  /// (`CoreApi.watchSession`).
  SessionProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'sessionProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$sessionHash();

  @$internal
  @override
  $StreamProviderElement<SessionState> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<SessionState> create(Ref ref) {
    return session(ref);
  }
}

String _$sessionHash() => r'6819342eb78197be7719314ba2f782e870523e6a';

/// Sync status and conflicts (`CoreApi.watchSyncStatus`).

@ProviderFor(syncStatus)
final syncStatusProvider = SyncStatusProvider._();

/// Sync status and conflicts (`CoreApi.watchSyncStatus`).

final class SyncStatusProvider
    extends
        $FunctionalProvider<
          AsyncValue<SyncStatusView>,
          SyncStatusView,
          Stream<SyncStatusView>
        >
    with $FutureModifier<SyncStatusView>, $StreamProvider<SyncStatusView> {
  /// Sync status and conflicts (`CoreApi.watchSyncStatus`).
  SyncStatusProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'syncStatusProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$syncStatusHash();

  @$internal
  @override
  $StreamProviderElement<SyncStatusView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<SyncStatusView> create(Ref ref) {
    return syncStatus(ref);
  }
}

String _$syncStatusHash() => r'e16ea9ed809860bcc9d1604d7285cfea9c476fb7';

/// Notification ops the platform adapter executes
/// (`CoreApi.watchNotificationOps`).

@ProviderFor(notificationOps)
final notificationOpsProvider = NotificationOpsProvider._();

/// Notification ops the platform adapter executes
/// (`CoreApi.watchNotificationOps`).

final class NotificationOpsProvider
    extends
        $FunctionalProvider<
          AsyncValue<NotificationOp>,
          NotificationOp,
          Stream<NotificationOp>
        >
    with $FutureModifier<NotificationOp>, $StreamProvider<NotificationOp> {
  /// Notification ops the platform adapter executes
  /// (`CoreApi.watchNotificationOps`).
  NotificationOpsProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'notificationOpsProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$notificationOpsHash();

  @$internal
  @override
  $StreamProviderElement<NotificationOp> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<NotificationOp> create(Ref ref) {
    return notificationOps(ref);
  }
}

String _$notificationOpsHash() => r'0eb9afd2361016a5e8c072946b16640a495cc40d';

/// Home (`CoreApi.watchHome`).

@ProviderFor(home)
final homeProvider = HomeProvider._();

/// Home (`CoreApi.watchHome`).

final class HomeProvider
    extends
        $FunctionalProvider<AsyncValue<HomeView>, HomeView, Stream<HomeView>>
    with $FutureModifier<HomeView>, $StreamProvider<HomeView> {
  /// Home (`CoreApi.watchHome`).
  HomeProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'homeProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$homeHash();

  @$internal
  @override
  $StreamProviderElement<HomeView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<HomeView> create(Ref ref) {
    return home(ref);
  }
}

String _$homeHash() => r'652eb1aa361e4a0130ca352c91ca316d18f93a55';

/// Inbox (`CoreApi.watchInbox`).

@ProviderFor(inbox)
final inboxProvider = InboxProvider._();

/// Inbox (`CoreApi.watchInbox`).

final class InboxProvider
    extends
        $FunctionalProvider<AsyncValue<InboxView>, InboxView, Stream<InboxView>>
    with $FutureModifier<InboxView>, $StreamProvider<InboxView> {
  /// Inbox (`CoreApi.watchInbox`).
  InboxProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'inboxProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$inboxHash();

  @$internal
  @override
  $StreamProviderElement<InboxView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<InboxView> create(Ref ref) {
    return inbox(ref);
  }
}

String _$inboxHash() => r'62736427b9a80d4a7d5303c75c3b9961d7ffc61d';

/// One note (`CoreApi.watchNote`).

@ProviderFor(note)
final noteProvider = NoteFamily._();

/// One note (`CoreApi.watchNote`).

final class NoteProvider
    extends
        $FunctionalProvider<
          AsyncValue<NoteScreen>,
          NoteScreen,
          Stream<NoteScreen>
        >
    with $FutureModifier<NoteScreen>, $StreamProvider<NoteScreen> {
  /// One note (`CoreApi.watchNote`).
  NoteProvider._({
    required NoteFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'noteProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$noteHash();

  @override
  String toString() {
    return r'noteProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<NoteScreen> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<NoteScreen> create(Ref ref) {
    final argument = this.argument as String;
    return note(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is NoteProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$noteHash() => r'febbbc4683efe1298b25a2970eb710f6fa6e5a71';

/// One note (`CoreApi.watchNote`).

final class NoteFamily extends $Family
    with $FunctionalFamilyOverride<Stream<NoteScreen>, String> {
  NoteFamily._()
    : super(
        retry: noCoreRetry,
        name: r'noteProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// One note (`CoreApi.watchNote`).

  NoteProvider call(String id) => NoteProvider._(argument: id, from: this);

  @override
  String toString() => r'noteProvider';
}

/// One folder of the notes tree, `''` = vault root
/// (`CoreApi.watchNotesList`).

@ProviderFor(notesList)
final notesListProvider = NotesListFamily._();

/// One folder of the notes tree, `''` = vault root
/// (`CoreApi.watchNotesList`).

final class NotesListProvider
    extends
        $FunctionalProvider<
          AsyncValue<NotesListView>,
          NotesListView,
          Stream<NotesListView>
        >
    with $FutureModifier<NotesListView>, $StreamProvider<NotesListView> {
  /// One folder of the notes tree, `''` = vault root
  /// (`CoreApi.watchNotesList`).
  NotesListProvider._({
    required NotesListFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'notesListProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$notesListHash();

  @override
  String toString() {
    return r'notesListProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<NotesListView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<NotesListView> create(Ref ref) {
    final argument = this.argument as String;
    return notesList(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is NotesListProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$notesListHash() => r'30fb0cfbe7a1230b7e651e8c24b592a4215ea9dd';

/// One folder of the notes tree, `''` = vault root
/// (`CoreApi.watchNotesList`).

final class NotesListFamily extends $Family
    with $FunctionalFamilyOverride<Stream<NotesListView>, String> {
  NotesListFamily._()
    : super(
        retry: noCoreRetry,
        name: r'notesListProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// One folder of the notes tree, `''` = vault root
  /// (`CoreApi.watchNotesList`).

  NotesListProvider call(String folder) =>
      NotesListProvider._(argument: folder, from: this);

  @override
  String toString() => r'notesListProvider';
}

/// A directory tab filtered by [query] (`CoreApi.watchDirectory`).

@ProviderFor(directory)
final directoryProvider = DirectoryFamily._();

/// A directory tab filtered by [query] (`CoreApi.watchDirectory`).

final class DirectoryProvider
    extends
        $FunctionalProvider<
          AsyncValue<DirectoryView>,
          DirectoryView,
          Stream<DirectoryView>
        >
    with $FutureModifier<DirectoryView>, $StreamProvider<DirectoryView> {
  /// A directory tab filtered by [query] (`CoreApi.watchDirectory`).
  DirectoryProvider._({
    required DirectoryFamily super.from,
    required (DirectoryTab, String) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'directoryProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$directoryHash();

  @override
  String toString() {
    return r'directoryProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $StreamProviderElement<DirectoryView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<DirectoryView> create(Ref ref) {
    final argument = this.argument as (DirectoryTab, String);
    return directory(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is DirectoryProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$directoryHash() => r'357a37d1397d5aa0b46c457cc25c9442e72ae9da';

/// A directory tab filtered by [query] (`CoreApi.watchDirectory`).

final class DirectoryFamily extends $Family
    with
        $FunctionalFamilyOverride<
          Stream<DirectoryView>,
          (DirectoryTab, String)
        > {
  DirectoryFamily._()
    : super(
        retry: noCoreRetry,
        name: r'directoryProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A directory tab filtered by [query] (`CoreApi.watchDirectory`).

  DirectoryProvider call(DirectoryTab tab, String query) =>
      DirectoryProvider._(argument: (tab, query), from: this);

  @override
  String toString() => r'directoryProvider';
}

/// A person, company, document or place page (`CoreApi.watchEntity`).

@ProviderFor(entity)
final entityProvider = EntityFamily._();

/// A person, company, document or place page (`CoreApi.watchEntity`).

final class EntityProvider
    extends
        $FunctionalProvider<
          AsyncValue<EntityScreen>,
          EntityScreen,
          Stream<EntityScreen>
        >
    with $FutureModifier<EntityScreen>, $StreamProvider<EntityScreen> {
  /// A person, company, document or place page (`CoreApi.watchEntity`).
  EntityProvider._({
    required EntityFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'entityProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$entityHash();

  @override
  String toString() {
    return r'entityProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<EntityScreen> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<EntityScreen> create(Ref ref) {
    final argument = this.argument as String;
    return entity(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is EntityProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$entityHash() => r'fafac1ee2305ff1b2351da67fa532fa4c5622d74';

/// A person, company, document or place page (`CoreApi.watchEntity`).

final class EntityFamily extends $Family
    with $FunctionalFamilyOverride<Stream<EntityScreen>, String> {
  EntityFamily._()
    : super(
        retry: noCoreRetry,
        name: r'entityProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A person, company, document or place page (`CoreApi.watchEntity`).

  EntityProvider call(String id) => EntityProvider._(argument: id, from: this);

  @override
  String toString() => r'entityProvider';
}

/// The Tasks destination (`CoreApi.watchTasks`).

@ProviderFor(tasks)
final tasksProvider = TasksProvider._();

/// The Tasks destination (`CoreApi.watchTasks`).

final class TasksProvider
    extends
        $FunctionalProvider<AsyncValue<TasksView>, TasksView, Stream<TasksView>>
    with $FutureModifier<TasksView>, $StreamProvider<TasksView> {
  /// The Tasks destination (`CoreApi.watchTasks`).
  TasksProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'tasksProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$tasksHash();

  @$internal
  @override
  $StreamProviderElement<TasksView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<TasksView> create(Ref ref) {
    return tasks(ref);
  }
}

String _$tasksHash() => r'bbfe689b8c3121e0974e19f7b73b5fc15dc7c89e';

/// Task detail (`CoreApi.watchTask`).

@ProviderFor(task)
final taskProvider = TaskFamily._();

/// Task detail (`CoreApi.watchTask`).

final class TaskProvider
    extends
        $FunctionalProvider<
          AsyncValue<TaskScreen>,
          TaskScreen,
          Stream<TaskScreen>
        >
    with $FutureModifier<TaskScreen>, $StreamProvider<TaskScreen> {
  /// Task detail (`CoreApi.watchTask`).
  TaskProvider._({
    required TaskFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'taskProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$taskHash();

  @override
  String toString() {
    return r'taskProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<TaskScreen> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<TaskScreen> create(Ref ref) {
    final argument = this.argument as String;
    return task(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is TaskProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$taskHash() => r'4dfdc4be83b640fa52ce988a94076a97bd540d70';

/// Task detail (`CoreApi.watchTask`).

final class TaskFamily extends $Family
    with $FunctionalFamilyOverride<Stream<TaskScreen>, String> {
  TaskFamily._()
    : super(
        retry: noCoreRetry,
        name: r'taskProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Task detail (`CoreApi.watchTask`).

  TaskProvider call(String id) => TaskProvider._(argument: id, from: this);

  @override
  String toString() => r'taskProvider';
}

/// One sync conflict (`CoreApi.watchConflict`).

@ProviderFor(conflict)
final conflictProvider = ConflictFamily._();

/// One sync conflict (`CoreApi.watchConflict`).

final class ConflictProvider
    extends
        $FunctionalProvider<
          AsyncValue<ConflictScreen>,
          ConflictScreen,
          Stream<ConflictScreen>
        >
    with $FutureModifier<ConflictScreen>, $StreamProvider<ConflictScreen> {
  /// One sync conflict (`CoreApi.watchConflict`).
  ConflictProvider._({
    required ConflictFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'conflictProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$conflictHash();

  @override
  String toString() {
    return r'conflictProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<ConflictScreen> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<ConflictScreen> create(Ref ref) {
    final argument = this.argument as String;
    return conflict(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is ConflictProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$conflictHash() => r'713fbfab19e095dc346b4d00c542265a0a55fb94';

/// One sync conflict (`CoreApi.watchConflict`).

final class ConflictFamily extends $Family
    with $FunctionalFamilyOverride<Stream<ConflictScreen>, String> {
  ConflictFamily._()
    : super(
        retry: noCoreRetry,
        name: r'conflictProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// One sync conflict (`CoreApi.watchConflict`).

  ConflictProvider call(String opId) =>
      ConflictProvider._(argument: opId, from: this);

  @override
  String toString() => r'conflictProvider';
}

/// Open "Already exists" prompts (`CoreApi.watchDuplicatePrompts`).

@ProviderFor(duplicatePrompts)
final duplicatePromptsProvider = DuplicatePromptsProvider._();

/// Open "Already exists" prompts (`CoreApi.watchDuplicatePrompts`).

final class DuplicatePromptsProvider
    extends
        $FunctionalProvider<
          AsyncValue<DuplicatePromptsView>,
          DuplicatePromptsView,
          Stream<DuplicatePromptsView>
        >
    with
        $FutureModifier<DuplicatePromptsView>,
        $StreamProvider<DuplicatePromptsView> {
  /// Open "Already exists" prompts (`CoreApi.watchDuplicatePrompts`).
  DuplicatePromptsProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'duplicatePromptsProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$duplicatePromptsHash();

  @$internal
  @override
  $StreamProviderElement<DuplicatePromptsView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<DuplicatePromptsView> create(Ref ref) {
    return duplicatePrompts(ref);
  }
}

String _$duplicatePromptsHash() => r'6c8668f07340cd2c720db1d09a1bad3749268df7';

/// Settings (`CoreApi.watchSettings`).

@ProviderFor(settings)
final settingsProvider = SettingsProvider._();

/// Settings (`CoreApi.watchSettings`).

final class SettingsProvider
    extends
        $FunctionalProvider<
          AsyncValue<SettingsView>,
          SettingsView,
          Stream<SettingsView>
        >
    with $FutureModifier<SettingsView>, $StreamProvider<SettingsView> {
  /// Settings (`CoreApi.watchSettings`).
  SettingsProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'settingsProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$settingsHash();

  @$internal
  @override
  $StreamProviderElement<SettingsView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<SettingsView> create(Ref ref) {
    return settings(ref);
  }
}

String _$settingsHash() => r'9ab23b7968c08a7fc5ce1c2db2db62c0fce203da';

/// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).

@ProviderFor(inboxFiltered)
final inboxFilteredProvider = InboxFilteredFamily._();

/// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).

final class InboxFilteredProvider
    extends
        $FunctionalProvider<AsyncValue<InboxView>, InboxView, Stream<InboxView>>
    with $FutureModifier<InboxView>, $StreamProvider<InboxView> {
  /// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).
  InboxFilteredProvider._({
    required InboxFilteredFamily super.from,
    required InboxFilter super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'inboxFilteredProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$inboxFilteredHash();

  @override
  String toString() {
    return r'inboxFilteredProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<InboxView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<InboxView> create(Ref ref) {
    final argument = this.argument as InboxFilter;
    return inboxFiltered(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is InboxFilteredProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$inboxFilteredHash() => r'92ff85a26f1a263989b761cb9bc46bbac5d919a0';

/// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).

final class InboxFilteredFamily extends $Family
    with $FunctionalFamilyOverride<Stream<InboxView>, InboxFilter> {
  InboxFilteredFamily._()
    : super(
        retry: noCoreRetry,
        name: r'inboxFilteredProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Inbox with a filter tab (`CoreApi.watchInboxFiltered`).

  InboxFilteredProvider call(InboxFilter filter) =>
      InboxFilteredProvider._(argument: filter, from: this);

  @override
  String toString() => r'inboxFilteredProvider';
}

/// Navigation counts and pinned notes (`CoreApi.watchNav`).

@ProviderFor(nav)
final navProvider = NavProvider._();

/// Navigation counts and pinned notes (`CoreApi.watchNav`).

final class NavProvider
    extends $FunctionalProvider<AsyncValue<NavView>, NavView, Stream<NavView>>
    with $FutureModifier<NavView>, $StreamProvider<NavView> {
  /// Navigation counts and pinned notes (`CoreApi.watchNav`).
  NavProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'navProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$navHash();

  @$internal
  @override
  $StreamProviderElement<NavView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<NavView> create(Ref ref) {
    return nav(ref);
  }
}

String _$navHash() => r'5514eecb6a42abec55f023db7bd58075b6fad37d';

/// The "Recent" block with a filter (`CoreApi.watchRecent`).

@ProviderFor(recent)
final recentProvider = RecentFamily._();

/// The "Recent" block with a filter (`CoreApi.watchRecent`).

final class RecentProvider
    extends
        $FunctionalProvider<
          AsyncValue<RecentNotesView>,
          RecentNotesView,
          Stream<RecentNotesView>
        >
    with $FutureModifier<RecentNotesView>, $StreamProvider<RecentNotesView> {
  /// The "Recent" block with a filter (`CoreApi.watchRecent`).
  RecentProvider._({
    required RecentFamily super.from,
    required RecentFilter super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'recentProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$recentHash();

  @override
  String toString() {
    return r'recentProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $StreamProviderElement<RecentNotesView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<RecentNotesView> create(Ref ref) {
    final argument = this.argument as RecentFilter;
    return recent(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is RecentProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$recentHash() => r'55db316c6151ce93c474a359679241aae84eb011';

/// The "Recent" block with a filter (`CoreApi.watchRecent`).

final class RecentFamily extends $Family
    with $FunctionalFamilyOverride<Stream<RecentNotesView>, RecentFilter> {
  RecentFamily._()
    : super(
        retry: noCoreRetry,
        name: r'recentProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The "Recent" block with a filter (`CoreApi.watchRecent`).

  RecentProvider call(RecentFilter filter) =>
      RecentProvider._(argument: filter, from: this);

  @override
  String toString() => r'recentProvider';
}

/// A directory tab with filters and sort applied in the core
/// (`CoreApi.watchDirectoryFiltered`).

@ProviderFor(directoryFiltered)
final directoryFilteredProvider = DirectoryFilteredFamily._();

/// A directory tab with filters and sort applied in the core
/// (`CoreApi.watchDirectoryFiltered`).

final class DirectoryFilteredProvider
    extends
        $FunctionalProvider<
          AsyncValue<DirectoryView>,
          DirectoryView,
          Stream<DirectoryView>
        >
    with $FutureModifier<DirectoryView>, $StreamProvider<DirectoryView> {
  /// A directory tab with filters and sort applied in the core
  /// (`CoreApi.watchDirectoryFiltered`).
  DirectoryFilteredProvider._({
    required DirectoryFilteredFamily super.from,
    required (DirectoryTab, String, DirectoryFilter, DirectorySort)
    super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'directoryFilteredProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$directoryFilteredHash();

  @override
  String toString() {
    return r'directoryFilteredProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $StreamProviderElement<DirectoryView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<DirectoryView> create(Ref ref) {
    final argument =
        this.argument as (DirectoryTab, String, DirectoryFilter, DirectorySort);
    return directoryFiltered(
      ref,
      argument.$1,
      argument.$2,
      argument.$3,
      argument.$4,
    );
  }

  @override
  bool operator ==(Object other) {
    return other is DirectoryFilteredProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$directoryFilteredHash() => r'87b5bfd3e4fd03e70f55b89b97ebbb7c6c22cb4b';

/// A directory tab with filters and sort applied in the core
/// (`CoreApi.watchDirectoryFiltered`).

final class DirectoryFilteredFamily extends $Family
    with
        $FunctionalFamilyOverride<
          Stream<DirectoryView>,
          (DirectoryTab, String, DirectoryFilter, DirectorySort)
        > {
  DirectoryFilteredFamily._()
    : super(
        retry: noCoreRetry,
        name: r'directoryFilteredProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A directory tab with filters and sort applied in the core
  /// (`CoreApi.watchDirectoryFiltered`).

  DirectoryFilteredProvider call(
    DirectoryTab tab,
    String query,
    DirectoryFilter filter,
    DirectorySort sort,
  ) => DirectoryFilteredProvider._(
    argument: (tab, query, filter, sort),
    from: this,
  );

  @override
  String toString() => r'directoryFilteredProvider';
}

/// The home-note picker of the new-task sheet (`CoreApi.watchTaskHomes`).

@ProviderFor(taskHomes)
final taskHomesProvider = TaskHomesProvider._();

/// The home-note picker of the new-task sheet (`CoreApi.watchTaskHomes`).

final class TaskHomesProvider
    extends
        $FunctionalProvider<
          AsyncValue<TaskHomesView>,
          TaskHomesView,
          Stream<TaskHomesView>
        >
    with $FutureModifier<TaskHomesView>, $StreamProvider<TaskHomesView> {
  /// The home-note picker of the new-task sheet (`CoreApi.watchTaskHomes`).
  TaskHomesProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'taskHomesProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$taskHomesHash();

  @$internal
  @override
  $StreamProviderElement<TaskHomesView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<TaskHomesView> create(Ref ref) {
    return taskHomes(ref);
  }
}

String _$taskHomesHash() => r'921b37bb2bcb2fe91f2f1c742eb2fce7a59f46cd';

/// The Ask conversation, streamed (`CoreApi.watchAsk`).

@ProviderFor(askConversation)
final askConversationProvider = AskConversationProvider._();

/// The Ask conversation, streamed (`CoreApi.watchAsk`).

final class AskConversationProvider
    extends $FunctionalProvider<AsyncValue<AskView>, AskView, Stream<AskView>>
    with $FutureModifier<AskView>, $StreamProvider<AskView> {
  /// The Ask conversation, streamed (`CoreApi.watchAsk`).
  AskConversationProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'askConversationProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$askConversationHash();

  @$internal
  @override
  $StreamProviderElement<AskView> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<AskView> create(Ref ref) {
    return askConversation(ref);
  }
}

String _$askConversationHash() => r'3287ef6363445af2b1074cbebd4f2796e1a38032';

/// A note's local mind map with only the edges of [edgeKinds] (empty: all),
/// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
/// list instance while the selection is unchanged.

@ProviderFor(localGraphFiltered)
final localGraphFilteredProvider = LocalGraphFilteredFamily._();

/// A note's local mind map with only the edges of [edgeKinds] (empty: all),
/// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
/// list instance while the selection is unchanged.

final class LocalGraphFilteredProvider
    extends
        $FunctionalProvider<
          AsyncValue<LocalGraphView>,
          LocalGraphView,
          Stream<LocalGraphView>
        >
    with $FutureModifier<LocalGraphView>, $StreamProvider<LocalGraphView> {
  /// A note's local mind map with only the edges of [edgeKinds] (empty: all),
  /// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
  /// list instance while the selection is unchanged.
  LocalGraphFilteredProvider._({
    required LocalGraphFilteredFamily super.from,
    required (String, int, List<String>) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'localGraphFilteredProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$localGraphFilteredHash();

  @override
  String toString() {
    return r'localGraphFilteredProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $StreamProviderElement<LocalGraphView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<LocalGraphView> create(Ref ref) {
    final argument = this.argument as (String, int, List<String>);
    return localGraphFiltered(ref, argument.$1, argument.$2, argument.$3);
  }

  @override
  bool operator ==(Object other) {
    return other is LocalGraphFilteredProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$localGraphFilteredHash() =>
    r'aaa6411d70b15eb30610cce1d9252a1bf0a97169';

/// A note's local mind map with only the edges of [edgeKinds] (empty: all),
/// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
/// list instance while the selection is unchanged.

final class LocalGraphFilteredFamily extends $Family
    with
        $FunctionalFamilyOverride<
          Stream<LocalGraphView>,
          (String, int, List<String>)
        > {
  LocalGraphFilteredFamily._()
    : super(
        retry: noCoreRetry,
        name: r'localGraphFilteredProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A note's local mind map with only the edges of [edgeKinds] (empty: all),
  /// filtered in the core (`CoreApi.watchLocalGraphFiltered`). Pass the same
  /// list instance while the selection is unchanged.

  LocalGraphFilteredProvider call(
    String id,
    int depth,
    List<String> edgeKinds,
  ) => LocalGraphFilteredProvider._(
    argument: (id, depth, edgeKinds),
    from: this,
  );

  @override
  String toString() => r'localGraphFilteredProvider';
}

/// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).

@ProviderFor(localGraph)
final localGraphProvider = LocalGraphFamily._();

/// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).

final class LocalGraphProvider
    extends
        $FunctionalProvider<
          AsyncValue<LocalGraphView>,
          LocalGraphView,
          Stream<LocalGraphView>
        >
    with $FutureModifier<LocalGraphView>, $StreamProvider<LocalGraphView> {
  /// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).
  LocalGraphProvider._({
    required LocalGraphFamily super.from,
    required (String, int) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'localGraphProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$localGraphHash();

  @override
  String toString() {
    return r'localGraphProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $StreamProviderElement<LocalGraphView> $createElement(
    $ProviderPointer pointer,
  ) => $StreamProviderElement(pointer);

  @override
  Stream<LocalGraphView> create(Ref ref) {
    final argument = this.argument as (String, int);
    return localGraph(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is LocalGraphProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$localGraphHash() => r'637a9d8dfd3f6cafdcf710651374091a060fdbce';

/// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).

final class LocalGraphFamily extends $Family
    with $FunctionalFamilyOverride<Stream<LocalGraphView>, (String, int)> {
  LocalGraphFamily._()
    : super(
        retry: noCoreRetry,
        name: r'localGraphProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A note's local mind map at [depth] (`CoreApi.watchLocalGraph`).

  LocalGraphProvider call(String id, int depth) =>
      LocalGraphProvider._(argument: (id, depth), from: this);

  @override
  String toString() => r'localGraphProvider';
}

/// The global map (`CoreApi.globalGraph`).

@ProviderFor(globalGraph)
final globalGraphProvider = GlobalGraphProvider._();

/// The global map (`CoreApi.globalGraph`).

final class GlobalGraphProvider
    extends
        $FunctionalProvider<
          AsyncValue<GlobalGraphView>,
          GlobalGraphView,
          FutureOr<GlobalGraphView>
        >
    with $FutureModifier<GlobalGraphView>, $FutureProvider<GlobalGraphView> {
  /// The global map (`CoreApi.globalGraph`).
  GlobalGraphProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'globalGraphProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$globalGraphHash();

  @$internal
  @override
  $FutureProviderElement<GlobalGraphView> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<GlobalGraphView> create(Ref ref) {
    return globalGraph(ref);
  }
}

String _$globalGraphHash() => r'63921d7d48c3030a58fbaa182f2b3eaf1614406b';

/// Local search (`CoreApi.search`).

@ProviderFor(search)
final searchProvider = SearchFamily._();

/// Local search (`CoreApi.search`).

final class SearchProvider
    extends
        $FunctionalProvider<
          AsyncValue<SearchView>,
          SearchView,
          FutureOr<SearchView>
        >
    with $FutureModifier<SearchView>, $FutureProvider<SearchView> {
  /// Local search (`CoreApi.search`).
  SearchProvider._({
    required SearchFamily super.from,
    required (String, SearchMode) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'searchProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$searchHash();

  @override
  String toString() {
    return r'searchProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<SearchView> $createElement($ProviderPointer pointer) =>
      $FutureProviderElement(pointer);

  @override
  FutureOr<SearchView> create(Ref ref) {
    final argument = this.argument as (String, SearchMode);
    return search(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is SearchProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$searchHash() => r'b954eb95926beedd811c144aa4532679ba2be647';

/// Local search (`CoreApi.search`).

final class SearchFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<SearchView>, (String, SearchMode)> {
  SearchFamily._()
    : super(
        retry: noCoreRetry,
        name: r'searchProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Local search (`CoreApi.search`).

  SearchProvider call(String query, SearchMode mode) =>
      SearchProvider._(argument: (query, mode), from: this);

  @override
  String toString() => r'searchProvider';
}

/// Ask (`CoreApi.askView`).

@ProviderFor(askView)
final askViewProvider = AskViewProvider._();

/// Ask (`CoreApi.askView`).

final class AskViewProvider
    extends $FunctionalProvider<AsyncValue<AskView>, AskView, FutureOr<AskView>>
    with $FutureModifier<AskView>, $FutureProvider<AskView> {
  /// Ask (`CoreApi.askView`).
  AskViewProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'askViewProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$askViewHash();

  @$internal
  @override
  $FutureProviderElement<AskView> $createElement($ProviderPointer pointer) =>
      $FutureProviderElement(pointer);

  @override
  FutureOr<AskView> create(Ref ref) {
    return askView(ref);
  }
}

String _$askViewHash() => r'7bac1b05397cd6c63a9e84eeb3ed4d4da46ce0b4';

/// Editor highlight spans for [content] (`CoreApi.editorHints`).

@ProviderFor(editorHints)
final editorHintsProvider = EditorHintsFamily._();

/// Editor highlight spans for [content] (`CoreApi.editorHints`).

final class EditorHintsProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<EditorHint>>,
          List<EditorHint>,
          FutureOr<List<EditorHint>>
        >
    with $FutureModifier<List<EditorHint>>, $FutureProvider<List<EditorHint>> {
  /// Editor highlight spans for [content] (`CoreApi.editorHints`).
  EditorHintsProvider._({
    required EditorHintsFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'editorHintsProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$editorHintsHash();

  @override
  String toString() {
    return r'editorHintsProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<List<EditorHint>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<EditorHint>> create(Ref ref) {
    final argument = this.argument as String;
    return editorHints(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is EditorHintsProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$editorHintsHash() => r'3994ef7728c5645637321cc138ea5af74f8bb617';

/// Editor highlight spans for [content] (`CoreApi.editorHints`).

final class EditorHintsFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<List<EditorHint>>, String> {
  EditorHintsFamily._()
    : super(
        retry: noCoreRetry,
        name: r'editorHintsProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Editor highlight spans for [content] (`CoreApi.editorHints`).

  EditorHintsProvider call(String content) =>
      EditorHintsProvider._(argument: content, from: this);

  @override
  String toString() => r'editorHintsProvider';
}

/// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).

@ProviderFor(adminUsers)
final adminUsersProvider = AdminUsersFamily._();

/// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).

final class AdminUsersProvider
    extends
        $FunctionalProvider<
          AsyncValue<AdminUsersView>,
          AdminUsersView,
          FutureOr<AdminUsersView>
        >
    with $FutureModifier<AdminUsersView>, $FutureProvider<AdminUsersView> {
  /// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).
  AdminUsersProvider._({
    required AdminUsersFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'adminUsersProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$adminUsersHash();

  @override
  String toString() {
    return r'adminUsersProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<AdminUsersView> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<AdminUsersView> create(Ref ref) {
    final argument = this.argument as String;
    return adminUsers(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is AdminUsersProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$adminUsersHash() => r'806c1b3b740933454df7e96475876be9130ce255';

/// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).

final class AdminUsersFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<AdminUsersView>, String> {
  AdminUsersFamily._()
    : super(
        retry: noCoreRetry,
        name: r'adminUsersProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Admin → Users filtered by [query] (`CoreApi.loadAdminUsers`).

  AdminUsersProvider call(String query) =>
      AdminUsersProvider._(argument: query, from: this);

  @override
  String toString() => r'adminUsersProvider';
}

/// The global map with filters and lens applied in the core
/// (`CoreApi.globalGraphFiltered`).

@ProviderFor(globalGraphFiltered)
final globalGraphFilteredProvider = GlobalGraphFilteredFamily._();

/// The global map with filters and lens applied in the core
/// (`CoreApi.globalGraphFiltered`).

final class GlobalGraphFilteredProvider
    extends
        $FunctionalProvider<
          AsyncValue<GlobalGraphView>,
          GlobalGraphView,
          FutureOr<GlobalGraphView>
        >
    with $FutureModifier<GlobalGraphView>, $FutureProvider<GlobalGraphView> {
  /// The global map with filters and lens applied in the core
  /// (`CoreApi.globalGraphFiltered`).
  GlobalGraphFilteredProvider._({
    required GlobalGraphFilteredFamily super.from,
    required GraphFilter super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'globalGraphFilteredProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$globalGraphFilteredHash();

  @override
  String toString() {
    return r'globalGraphFilteredProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<GlobalGraphView> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<GlobalGraphView> create(Ref ref) {
    final argument = this.argument as GraphFilter;
    return globalGraphFiltered(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is GlobalGraphFilteredProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$globalGraphFilteredHash() =>
    r'de6ffbd86383c94aa588ca3b635a0c0abc60f8e1';

/// The global map with filters and lens applied in the core
/// (`CoreApi.globalGraphFiltered`).

final class GlobalGraphFilteredFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<GlobalGraphView>, GraphFilter> {
  GlobalGraphFilteredFamily._()
    : super(
        retry: noCoreRetry,
        name: r'globalGraphFilteredProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The global map with filters and lens applied in the core
  /// (`CoreApi.globalGraphFiltered`).

  GlobalGraphFilteredProvider call(GraphFilter filter) =>
      GlobalGraphFilteredProvider._(argument: filter, from: this);

  @override
  String toString() => r'globalGraphFilteredProvider';
}

/// Search limited to a folder (`CoreApi.searchInFolder`).

@ProviderFor(searchInFolder)
final searchInFolderProvider = SearchInFolderFamily._();

/// Search limited to a folder (`CoreApi.searchInFolder`).

final class SearchInFolderProvider
    extends
        $FunctionalProvider<
          AsyncValue<SearchView>,
          SearchView,
          FutureOr<SearchView>
        >
    with $FutureModifier<SearchView>, $FutureProvider<SearchView> {
  /// Search limited to a folder (`CoreApi.searchInFolder`).
  SearchInFolderProvider._({
    required SearchInFolderFamily super.from,
    required (String, SearchMode, String?) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'searchInFolderProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$searchInFolderHash();

  @override
  String toString() {
    return r'searchInFolderProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<SearchView> $createElement($ProviderPointer pointer) =>
      $FutureProviderElement(pointer);

  @override
  FutureOr<SearchView> create(Ref ref) {
    final argument = this.argument as (String, SearchMode, String?);
    return searchInFolder(ref, argument.$1, argument.$2, argument.$3);
  }

  @override
  bool operator ==(Object other) {
    return other is SearchInFolderProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$searchInFolderHash() => r'af6fa18109c348704866e59365c17087e5ed461f';

/// Search limited to a folder (`CoreApi.searchInFolder`).

final class SearchInFolderFamily extends $Family
    with
        $FunctionalFamilyOverride<
          FutureOr<SearchView>,
          (String, SearchMode, String?)
        > {
  SearchInFolderFamily._()
    : super(
        retry: noCoreRetry,
        name: r'searchInFolderProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Search limited to a folder (`CoreApi.searchInFolder`).

  SearchInFolderProvider call(String query, SearchMode mode, String? folder) =>
      SearchInFolderProvider._(argument: (query, mode, folder), from: this);

  @override
  String toString() => r'searchInFolderProvider';
}

/// Editor completions at [cursor] (`CoreApi.editorCompletions`).

@ProviderFor(editorCompletions)
final editorCompletionsProvider = EditorCompletionsFamily._();

/// Editor completions at [cursor] (`CoreApi.editorCompletions`).

final class EditorCompletionsProvider
    extends
        $FunctionalProvider<
          AsyncValue<Completions>,
          Completions,
          FutureOr<Completions>
        >
    with $FutureModifier<Completions>, $FutureProvider<Completions> {
  /// Editor completions at [cursor] (`CoreApi.editorCompletions`).
  EditorCompletionsProvider._({
    required EditorCompletionsFamily super.from,
    required (String, String, int) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'editorCompletionsProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$editorCompletionsHash();

  @override
  String toString() {
    return r'editorCompletionsProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<Completions> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<Completions> create(Ref ref) {
    final argument = this.argument as (String, String, int);
    return editorCompletions(ref, argument.$1, argument.$2, argument.$3);
  }

  @override
  bool operator ==(Object other) {
    return other is EditorCompletionsProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$editorCompletionsHash() => r'8d703c203293e4622fd81297de304f5299c0a31b';

/// Editor completions at [cursor] (`CoreApi.editorCompletions`).

final class EditorCompletionsFamily extends $Family
    with
        $FunctionalFamilyOverride<
          FutureOr<Completions>,
          (String, String, int)
        > {
  EditorCompletionsFamily._()
    : super(
        retry: noCoreRetry,
        name: r'editorCompletionsProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Editor completions at [cursor] (`CoreApi.editorCompletions`).

  EditorCompletionsProvider call(String noteId, String content, int cursor) =>
      EditorCompletionsProvider._(
        argument: (noteId, content, cursor),
        from: this,
      );

  @override
  String toString() => r'editorCompletionsProvider';
}

/// Vault tags starting with [prefix] (`CoreApi.tags`).

@ProviderFor(tags)
final tagsProvider = TagsFamily._();

/// Vault tags starting with [prefix] (`CoreApi.tags`).

final class TagsProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<TagItem>>,
          List<TagItem>,
          FutureOr<List<TagItem>>
        >
    with $FutureModifier<List<TagItem>>, $FutureProvider<List<TagItem>> {
  /// Vault tags starting with [prefix] (`CoreApi.tags`).
  TagsProvider._({
    required TagsFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'tagsProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$tagsHash();

  @override
  String toString() {
    return r'tagsProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<List<TagItem>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<TagItem>> create(Ref ref) {
    final argument = this.argument as String;
    return tags(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is TagsProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$tagsHash() => r'9c62f5997eab9b110cad1e67ea17191f2ca6b877';

/// Vault tags starting with [prefix] (`CoreApi.tags`).

final class TagsFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<List<TagItem>>, String> {
  TagsFamily._()
    : super(
        retry: noCoreRetry,
        name: r'tagsProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Vault tags starting with [prefix] (`CoreApi.tags`).

  TagsProvider call(String prefix) =>
      TagsProvider._(argument: prefix, from: this);

  @override
  String toString() => r'tagsProvider';
}

/// The blocks of a note (`CoreApi.noteBlocks`).

@ProviderFor(noteBlocks)
final noteBlocksProvider = NoteBlocksFamily._();

/// The blocks of a note (`CoreApi.noteBlocks`).

final class NoteBlocksProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<BlockItem>>,
          List<BlockItem>,
          FutureOr<List<BlockItem>>
        >
    with $FutureModifier<List<BlockItem>>, $FutureProvider<List<BlockItem>> {
  /// The blocks of a note (`CoreApi.noteBlocks`).
  NoteBlocksProvider._({
    required NoteBlocksFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'noteBlocksProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$noteBlocksHash();

  @override
  String toString() {
    return r'noteBlocksProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<List<BlockItem>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<BlockItem>> create(Ref ref) {
    final argument = this.argument as String;
    return noteBlocks(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is NoteBlocksProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$noteBlocksHash() => r'dd6d2c1e2dfc201cbc6d8cc4caa4b09adb8e3b22';

/// The blocks of a note (`CoreApi.noteBlocks`).

final class NoteBlocksFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<List<BlockItem>>, String> {
  NoteBlocksFamily._()
    : super(
        retry: noCoreRetry,
        name: r'noteBlocksProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The blocks of a note (`CoreApi.noteBlocks`).

  NoteBlocksProvider call(String noteId) =>
      NoteBlocksProvider._(argument: noteId, from: this);

  @override
  String toString() => r'noteBlocksProvider';
}

/// Relation types with labels (`CoreApi.relationTypes`).

@ProviderFor(relationTypes)
final relationTypesProvider = RelationTypesProvider._();

/// Relation types with labels (`CoreApi.relationTypes`).

final class RelationTypesProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<RelationTypeItem>>,
          List<RelationTypeItem>,
          FutureOr<List<RelationTypeItem>>
        >
    with
        $FutureModifier<List<RelationTypeItem>>,
        $FutureProvider<List<RelationTypeItem>> {
  /// Relation types with labels (`CoreApi.relationTypes`).
  RelationTypesProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'relationTypesProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$relationTypesHash();

  @$internal
  @override
  $FutureProviderElement<List<RelationTypeItem>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<RelationTypeItem>> create(Ref ref) {
    return relationTypes(ref);
  }
}

String _$relationTypesHash() => r'44ffb88248640d8b725a1477746706db68063e5e';

/// The recurrence form of a phrase (`CoreApi.recurrenceForm`).

@ProviderFor(recurrenceForm)
final recurrenceFormProvider = RecurrenceFormFamily._();

/// The recurrence form of a phrase (`CoreApi.recurrenceForm`).

final class RecurrenceFormProvider
    extends
        $FunctionalProvider<
          AsyncValue<RecurrenceForm?>,
          RecurrenceForm?,
          FutureOr<RecurrenceForm?>
        >
    with $FutureModifier<RecurrenceForm?>, $FutureProvider<RecurrenceForm?> {
  /// The recurrence form of a phrase (`CoreApi.recurrenceForm`).
  RecurrenceFormProvider._({
    required RecurrenceFormFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'recurrenceFormProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$recurrenceFormHash();

  @override
  String toString() {
    return r'recurrenceFormProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<RecurrenceForm?> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<RecurrenceForm?> create(Ref ref) {
    final argument = this.argument as String;
    return recurrenceForm(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is RecurrenceFormProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$recurrenceFormHash() => r'309ce51730286521c1e310cafc6dfe27abf6e2f2';

/// The recurrence form of a phrase (`CoreApi.recurrenceForm`).

final class RecurrenceFormFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<RecurrenceForm?>, String> {
  RecurrenceFormFamily._()
    : super(
        retry: noCoreRetry,
        name: r'recurrenceFormProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The recurrence form of a phrase (`CoreApi.recurrenceForm`).

  RecurrenceFormProvider call(String phrase) =>
      RecurrenceFormProvider._(argument: phrase, from: this);

  @override
  String toString() => r'recurrenceFormProvider';
}

/// A recurrence form compiled to its phrase and summary
/// (`CoreApi.composeRecurrence`).

@ProviderFor(composeRecurrence)
final composeRecurrenceProvider = ComposeRecurrenceFamily._();

/// A recurrence form compiled to its phrase and summary
/// (`CoreApi.composeRecurrence`).

final class ComposeRecurrenceProvider
    extends
        $FunctionalProvider<
          AsyncValue<RecurrenceCompose>,
          RecurrenceCompose,
          FutureOr<RecurrenceCompose>
        >
    with
        $FutureModifier<RecurrenceCompose>,
        $FutureProvider<RecurrenceCompose> {
  /// A recurrence form compiled to its phrase and summary
  /// (`CoreApi.composeRecurrence`).
  ComposeRecurrenceProvider._({
    required ComposeRecurrenceFamily super.from,
    required RecurrenceForm super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'composeRecurrenceProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$composeRecurrenceHash();

  @override
  String toString() {
    return r'composeRecurrenceProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<RecurrenceCompose> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<RecurrenceCompose> create(Ref ref) {
    final argument = this.argument as RecurrenceForm;
    return composeRecurrence(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is ComposeRecurrenceProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$composeRecurrenceHash() => r'452a67b222b5c41f4af7c74a901c37f3f96179c7';

/// A recurrence form compiled to its phrase and summary
/// (`CoreApi.composeRecurrence`).

final class ComposeRecurrenceFamily extends $Family
    with
        $FunctionalFamilyOverride<FutureOr<RecurrenceCompose>, RecurrenceForm> {
  ComposeRecurrenceFamily._()
    : super(
        retry: noCoreRetry,
        name: r'composeRecurrenceProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A recurrence form compiled to its phrase and summary
  /// (`CoreApi.composeRecurrence`).

  ComposeRecurrenceProvider call(RecurrenceForm form) =>
      ComposeRecurrenceProvider._(argument: form, from: this);

  @override
  String toString() => r'composeRecurrenceProvider';
}

/// The next dates of a recurrence (`CoreApi.recurrencePreview`).

@ProviderFor(recurrencePreview)
final recurrencePreviewProvider = RecurrencePreviewFamily._();

/// The next dates of a recurrence (`CoreApi.recurrencePreview`).

final class RecurrencePreviewProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<RecurrencePreviewItem>>,
          List<RecurrencePreviewItem>,
          FutureOr<List<RecurrencePreviewItem>>
        >
    with
        $FutureModifier<List<RecurrencePreviewItem>>,
        $FutureProvider<List<RecurrencePreviewItem>> {
  /// The next dates of a recurrence (`CoreApi.recurrencePreview`).
  RecurrencePreviewProvider._({
    required RecurrencePreviewFamily super.from,
    required (String, DateTime, int) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'recurrencePreviewProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$recurrencePreviewHash();

  @override
  String toString() {
    return r'recurrencePreviewProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<List<RecurrencePreviewItem>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<RecurrencePreviewItem>> create(Ref ref) {
    final argument = this.argument as (String, DateTime, int);
    return recurrencePreview(ref, argument.$1, argument.$2, argument.$3);
  }

  @override
  bool operator ==(Object other) {
    return other is RecurrencePreviewProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$recurrencePreviewHash() => r'0b5769ef59bf35582ddd9d9b2961c95fb4066fab';

/// The next dates of a recurrence (`CoreApi.recurrencePreview`).

final class RecurrencePreviewFamily extends $Family
    with
        $FunctionalFamilyOverride<
          FutureOr<List<RecurrencePreviewItem>>,
          (String, DateTime, int)
        > {
  RecurrencePreviewFamily._()
    : super(
        retry: noCoreRetry,
        name: r'recurrencePreviewProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The next dates of a recurrence (`CoreApi.recurrencePreview`).

  RecurrencePreviewProvider call(String phrase, DateTime from, int count) =>
      RecurrencePreviewProvider._(argument: (phrase, from, count), from: this);

  @override
  String toString() => r'recurrencePreviewProvider';
}

/// A new task's text as the core understands it (`CoreApi.parseTaskText`).

@ProviderFor(parseTaskText)
final parseTaskTextProvider = ParseTaskTextFamily._();

/// A new task's text as the core understands it (`CoreApi.parseTaskText`).

final class ParseTaskTextProvider
    extends
        $FunctionalProvider<
          AsyncValue<TaskDraftPreview>,
          TaskDraftPreview,
          FutureOr<TaskDraftPreview>
        >
    with $FutureModifier<TaskDraftPreview>, $FutureProvider<TaskDraftPreview> {
  /// A new task's text as the core understands it (`CoreApi.parseTaskText`).
  ParseTaskTextProvider._({
    required ParseTaskTextFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'parseTaskTextProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$parseTaskTextHash();

  @override
  String toString() {
    return r'parseTaskTextProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<TaskDraftPreview> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<TaskDraftPreview> create(Ref ref) {
    final argument = this.argument as String;
    return parseTaskText(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is ParseTaskTextProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$parseTaskTextHash() => r'1067c8fde05ef905d6fb26af2b6ef6a6c0c95b6e';

/// A new task's text as the core understands it (`CoreApi.parseTaskText`).

final class ParseTaskTextFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<TaskDraftPreview>, String> {
  ParseTaskTextFamily._()
    : super(
        retry: noCoreRetry,
        name: r'parseTaskTextProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A new task's text as the core understands it (`CoreApi.parseTaskText`).

  ParseTaskTextProvider call(String text) =>
      ParseTaskTextProvider._(argument: text, from: this);

  @override
  String toString() => r'parseTaskTextProvider';
}

/// Places for the location picker (`CoreApi.placeOptions`).

@ProviderFor(placeOptions)
final placeOptionsProvider = PlaceOptionsFamily._();

/// Places for the location picker (`CoreApi.placeOptions`).

final class PlaceOptionsProvider
    extends
        $FunctionalProvider<
          AsyncValue<List<PlaceOption>>,
          List<PlaceOption>,
          FutureOr<List<PlaceOption>>
        >
    with
        $FutureModifier<List<PlaceOption>>,
        $FutureProvider<List<PlaceOption>> {
  /// Places for the location picker (`CoreApi.placeOptions`).
  PlaceOptionsProvider._({
    required PlaceOptionsFamily super.from,
    required String? super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'placeOptionsProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$placeOptionsHash();

  @override
  String toString() {
    return r'placeOptionsProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<List<PlaceOption>> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<List<PlaceOption>> create(Ref ref) {
    final argument = this.argument as String?;
    return placeOptions(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is PlaceOptionsProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$placeOptionsHash() => r'7355d49c376b29cb80a8e2fb6c5fc69a8de1930c';

/// Places for the location picker (`CoreApi.placeOptions`).

final class PlaceOptionsFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<List<PlaceOption>>, String?> {
  PlaceOptionsFamily._()
    : super(
        retry: noCoreRetry,
        name: r'placeOptionsProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// Places for the location picker (`CoreApi.placeOptions`).

  PlaceOptionsProvider call(String? documentId) =>
      PlaceOptionsProvider._(argument: documentId, from: this);

  @override
  String toString() => r'placeOptionsProvider';
}

/// What merging two entities does (`CoreApi.mergePreview`).

@ProviderFor(mergePreview)
final mergePreviewProvider = MergePreviewFamily._();

/// What merging two entities does (`CoreApi.mergePreview`).

final class MergePreviewProvider
    extends
        $FunctionalProvider<
          AsyncValue<MergePreview>,
          MergePreview,
          FutureOr<MergePreview>
        >
    with $FutureModifier<MergePreview>, $FutureProvider<MergePreview> {
  /// What merging two entities does (`CoreApi.mergePreview`).
  MergePreviewProvider._({
    required MergePreviewFamily super.from,
    required (String, String) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'mergePreviewProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$mergePreviewHash();

  @override
  String toString() {
    return r'mergePreviewProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<MergePreview> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<MergePreview> create(Ref ref) {
    final argument = this.argument as (String, String);
    return mergePreview(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is MergePreviewProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$mergePreviewHash() => r'79ba55c15d446e9251220aae495cd8fa8087dd5b';

/// What merging two entities does (`CoreApi.mergePreview`).

final class MergePreviewFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<MergePreview>, (String, String)> {
  MergePreviewFamily._()
    : super(
        retry: noCoreRetry,
        name: r'mergePreviewProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// What merging two entities does (`CoreApi.mergePreview`).

  MergePreviewProvider call(String sourceId, String intoId) =>
      MergePreviewProvider._(argument: (sourceId, intoId), from: this);

  @override
  String toString() => r'mergePreviewProvider';
}

/// The block a citation points to (`CoreApi.resolveCitation`).

@ProviderFor(resolveCitation)
final resolveCitationProvider = ResolveCitationFamily._();

/// The block a citation points to (`CoreApi.resolveCitation`).

final class ResolveCitationProvider
    extends
        $FunctionalProvider<
          AsyncValue<CitationPreview>,
          CitationPreview,
          FutureOr<CitationPreview>
        >
    with $FutureModifier<CitationPreview>, $FutureProvider<CitationPreview> {
  /// The block a citation points to (`CoreApi.resolveCitation`).
  ResolveCitationProvider._({
    required ResolveCitationFamily super.from,
    required (String, String?) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'resolveCitationProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$resolveCitationHash();

  @override
  String toString() {
    return r'resolveCitationProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<CitationPreview> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<CitationPreview> create(Ref ref) {
    final argument = this.argument as (String, String?);
    return resolveCitation(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is ResolveCitationProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$resolveCitationHash() => r'a4f004aeafef6dc76c15e0c5e5c9b5d60123a9a6';

/// The block a citation points to (`CoreApi.resolveCitation`).

final class ResolveCitationFamily extends $Family
    with
        $FunctionalFamilyOverride<
          FutureOr<CitationPreview>,
          (String, String?)
        > {
  ResolveCitationFamily._()
    : super(
        retry: noCoreRetry,
        name: r'resolveCitationProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The block a citation points to (`CoreApi.resolveCitation`).

  ResolveCitationProvider call(String noteId, String? anchor) =>
      ResolveCitationProvider._(argument: (noteId, anchor), from: this);

  @override
  String toString() => r'resolveCitationProvider';
}

/// A revision compared with the current note (`CoreApi.noteRevisionDiff`).

@ProviderFor(noteRevisionDiff)
final noteRevisionDiffProvider = NoteRevisionDiffFamily._();

/// A revision compared with the current note (`CoreApi.noteRevisionDiff`).

final class NoteRevisionDiffProvider
    extends
        $FunctionalProvider<
          AsyncValue<NoteDiffView>,
          NoteDiffView,
          FutureOr<NoteDiffView>
        >
    with $FutureModifier<NoteDiffView>, $FutureProvider<NoteDiffView> {
  /// A revision compared with the current note (`CoreApi.noteRevisionDiff`).
  NoteRevisionDiffProvider._({
    required NoteRevisionDiffFamily super.from,
    required (String, String) super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'noteRevisionDiffProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$noteRevisionDiffHash();

  @override
  String toString() {
    return r'noteRevisionDiffProvider'
        ''
        '$argument';
  }

  @$internal
  @override
  $FutureProviderElement<NoteDiffView> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<NoteDiffView> create(Ref ref) {
    final argument = this.argument as (String, String);
    return noteRevisionDiff(ref, argument.$1, argument.$2);
  }

  @override
  bool operator ==(Object other) {
    return other is NoteRevisionDiffProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$noteRevisionDiffHash() => r'91ca73f1735534f53769e02940155afeb7e9d609';

/// A revision compared with the current note (`CoreApi.noteRevisionDiff`).

final class NoteRevisionDiffFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<NoteDiffView>, (String, String)> {
  NoteRevisionDiffFamily._()
    : super(
        retry: noCoreRetry,
        name: r'noteRevisionDiffProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// A revision compared with the current note (`CoreApi.noteRevisionDiff`).

  NoteRevisionDiffProvider call(String noteId, String commit) =>
      NoteRevisionDiffProvider._(argument: (noteId, commit), from: this);

  @override
  String toString() => r'noteRevisionDiffProvider';
}

/// The sign-up password meter (`CoreApi.passwordStrength`).

@ProviderFor(passwordStrength)
final passwordStrengthProvider = PasswordStrengthFamily._();

/// The sign-up password meter (`CoreApi.passwordStrength`).

final class PasswordStrengthProvider
    extends
        $FunctionalProvider<
          AsyncValue<PasswordStrength>,
          PasswordStrength,
          FutureOr<PasswordStrength>
        >
    with $FutureModifier<PasswordStrength>, $FutureProvider<PasswordStrength> {
  /// The sign-up password meter (`CoreApi.passwordStrength`).
  PasswordStrengthProvider._({
    required PasswordStrengthFamily super.from,
    required String super.argument,
  }) : super(
         retry: noCoreRetry,
         name: r'passwordStrengthProvider',
         isAutoDispose: true,
         dependencies: null,
         $allTransitiveDependencies: null,
       );

  @override
  String debugGetCreateSourceHash() => _$passwordStrengthHash();

  @override
  String toString() {
    return r'passwordStrengthProvider'
        ''
        '($argument)';
  }

  @$internal
  @override
  $FutureProviderElement<PasswordStrength> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<PasswordStrength> create(Ref ref) {
    final argument = this.argument as String;
    return passwordStrength(ref, argument);
  }

  @override
  bool operator ==(Object other) {
    return other is PasswordStrengthProvider && other.argument == argument;
  }

  @override
  int get hashCode {
    return argument.hashCode;
  }
}

String _$passwordStrengthHash() => r'6b877ebe7d47800f1a809bf0444ebd1c2229e9e2';

/// The sign-up password meter (`CoreApi.passwordStrength`).

final class PasswordStrengthFamily extends $Family
    with $FunctionalFamilyOverride<FutureOr<PasswordStrength>, String> {
  PasswordStrengthFamily._()
    : super(
        retry: noCoreRetry,
        name: r'passwordStrengthProvider',
        dependencies: null,
        $allTransitiveDependencies: null,
        isAutoDispose: true,
      );

  /// The sign-up password meter (`CoreApi.passwordStrength`).

  PasswordStrengthProvider call(String password) =>
      PasswordStrengthProvider._(argument: password, from: this);

  @override
  String toString() => r'passwordStrengthProvider';
}
