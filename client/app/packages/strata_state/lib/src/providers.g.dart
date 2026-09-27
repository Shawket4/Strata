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

/// Admin → Users (`CoreApi.loadAdminUsers`).

@ProviderFor(adminUsers)
final adminUsersProvider = AdminUsersProvider._();

/// Admin → Users (`CoreApi.loadAdminUsers`).

final class AdminUsersProvider
    extends
        $FunctionalProvider<
          AsyncValue<AdminUsersView>,
          AdminUsersView,
          FutureOr<AdminUsersView>
        >
    with $FutureModifier<AdminUsersView>, $FutureProvider<AdminUsersView> {
  /// Admin → Users (`CoreApi.loadAdminUsers`).
  AdminUsersProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'adminUsersProvider',
        isAutoDispose: true,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$adminUsersHash();

  @$internal
  @override
  $FutureProviderElement<AdminUsersView> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<AdminUsersView> create(Ref ref) {
    return adminUsers(ref);
  }
}

String _$adminUsersHash() => r'4104e0050658e5a91b0623257ec4a515e8a36206';
