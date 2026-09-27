/// Riverpod plumbing between the Rust client core and the feature UIs
/// (PLAN §11.1, L15): the [CoreApi] seam over the `strata_bridge` facade, its
/// production implementation [BridgeCoreApi], and one provider per view-model
/// stream or one-shot. Providers only adapt; nothing is filtered, sorted or
/// derived in Dart.
///
/// Also re-exports the generated view-model types, so feature packages depend
/// on this package only. The bridge's functions and loader are hidden: widgets
/// reach the core through providers and `coreApiProvider`, never directly.
library;

export 'package:strata_bridge/strata_bridge.dart'
    hide
        ExternalLibrary,
        StrataCore,
        acceptSuggestion,
        acknowledgeAccountDisabled,
        addRelation,
        appLifecycle,
        askView,
        cancelTask,
        capture,
        completeTask,
        createEntity,
        createNote,
        createTask,
        deleteNote,
        deleteTask,
        dismissRejection,
        editorHints,
        globalGraph,
        initCore,
        loadAdminUsers,
        loadStrataCore,
        moveNote,
        notificationAction,
        refreshAccount,
        rejectSuggestion,
        removeRelation,
        reopenTask,
        reportNotificationResult,
        requestRelink,
        resolveConflict,
        resolveDuplicate,
        retypeRelation,
        search,
        setRemindersEnabled,
        signIn,
        signOut,
        signUp,
        strataAppDataDirectory,
        switchAccount,
        syncNow,
        updateNote,
        updateTask,
        watchConflict,
        watchDirectory,
        watchDuplicatePrompts,
        watchEntity,
        watchHome,
        watchInbox,
        watchLocalGraph,
        watchNote,
        watchNotesList,
        watchNotificationOps,
        watchSession,
        watchSettings,
        watchSyncStatus,
        watchTask,
        watchTasks;

export 'src/bridge_core_api.dart';
export 'src/core_api.dart';
export 'src/providers.dart';
