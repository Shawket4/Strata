import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:strata/src/shell/app_shell.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_notes/strata_notes.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart' show DirectoryTab, SignInRequest;
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

part 'routes.g.dart';

/// A Material page from `package:flutter/material.dart`. go_router 18 detects
/// `material_ui`'s `MaterialApp`, so the app states its page type explicitly.
Page<void> _page(GoRouterState state, Widget child) =>
    MaterialPage<void>(key: state.pageKey, name: state.name, child: child);

/// Picks [compact] or [wide] for the window's size class (re-evaluated live
/// on resize).
class _BySize extends StatelessWidget {
  const new({required this.compact, required this.wide});

  final WidgetBuilder compact;
  final WidgetBuilder wide;

  @override
  Widget build(BuildContext context) =>
      SizeClass.of(context) == SizeClass.compact
      ? compact(context)
      : wide(context);
}

/// A top-level (outside the shell) screen with its own app bar.
class _StandaloneScreen extends StatelessWidget {
  const new({required this.title, required this.child});

  final String Function(StrataLocalizations l10n) title;
  final Widget child;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: Text(title(context.l10n))),
    body: child,
  );
}

// ---------------------------------------------------------------------------
// The adaptive shell
// ---------------------------------------------------------------------------

/// The adaptive shell: one branch per navigation destination, in the order
/// of [AppDestination].
@TypedStatefulShellRoute<AppShellRoute>(
  branches: [
    TypedStatefulShellBranch<HomeBranch>(
      routes: [TypedGoRoute<HomeRoute>(path: '/home')],
    ),
    TypedStatefulShellBranch<InboxBranch>(
      routes: [TypedGoRoute<InboxRoute>(path: '/inbox')],
    ),
    TypedStatefulShellBranch<TasksBranch>(
      routes: [
        TypedGoRoute<TasksRoute>(
          path: '/tasks',
          routes: [TypedGoRoute<TaskRoute>(path: ':taskId')],
        ),
      ],
    ),
    TypedStatefulShellBranch<NotesBranch>(
      routes: [
        TypedGoRoute<NotesRoute>(
          path: '/notes',
          routes: [
            TypedGoRoute<ConflictRoute>(path: 'conflicts/:opId'),
            TypedGoRoute<NoteEditorRoute>(
              path: ':noteId',
              routes: [TypedGoRoute<MindMapRoute>(path: 'map')],
            ),
          ],
        ),
      ],
    ),
    TypedStatefulShellBranch<MapBranch>(
      routes: [TypedGoRoute<MapRoute>(path: '/map')],
    ),
    TypedStatefulShellBranch<DirectoryBranch>(
      routes: [
        TypedGoRoute<DirectoryRoute>(
          path: '/directory',
          routes: [
            TypedGoRoute<DocumentRoute>(path: 'documents/:documentId'),
            TypedGoRoute<PlaceRoute>(path: 'places/:placeId'),
            TypedGoRoute<EntityRoute>(path: ':entityId'),
          ],
        ),
      ],
    ),
    TypedStatefulShellBranch<AskBranch>(
      routes: [TypedGoRoute<AskRoute>(path: '/ask')],
    ),
    TypedStatefulShellBranch<SettingsBranch>(
      routes: [
        TypedGoRoute<SettingsRoute>(
          path: '/settings',
          routes: [
            TypedGoRoute<AdminUsersRoute>(path: 'admin/users'),
            TypedGoRoute<SettingsSectionRoute>(path: ':section'),
          ],
        ),
      ],
    ),
  ],
)
class AppShellRoute extends StatefulShellRouteData {
  /// Creates the shell route.
  const new();

  @override
  Widget builder(
    BuildContext context,
    GoRouterState state,
    StatefulNavigationShell navigationShell,
  ) => AppShell(navigationShell: navigationShell, location: state.uri);
}

/// Home branch.
class HomeBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Inbox branch.
class InboxBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Tasks branch (rail and sidebar only; on compact tasks live on Home).
class TasksBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Notes branch.
class NotesBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Global map branch (medium and expanded only).
class MapBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Directory branch.
class DirectoryBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Ask branch.
class AskBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// Settings branch.
class SettingsBranch extends StatefulShellBranchData {
  /// Creates the branch.
  const new();
}

/// `/home`.
class HomeRoute extends GoRouteData with $HomeRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    HomeScreen(
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenNotes: () => const NotesRoute().go(context),
      onOpenInbox: () => const InboxRoute().go(context),
      onOpenTasks: () => const TasksRoute().go(context),
      onOpenTask: (id) => TaskRoute(taskId: id).go(context),
    ),
  );
}

/// `/inbox`.
class InboxRoute extends GoRouteData with $InboxRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    InboxScreen(
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
    ),
  );
}

/// `/tasks`.
class TasksRoute extends GoRouteData with $TasksRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    TasksScreen(
      onOpenTask: (id) => TaskRoute(taskId: id).go(context),
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
    ),
  );
}

/// `/tasks/:taskId`: task detail; also the deep link of a reminder
/// notification (PLAN §11.1).
class TaskRoute extends GoRouteData with $TaskRoute {
  /// Creates the route for [taskId].
  const new({required this.taskId});

  /// The task's block ID.
  final String taskId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _BySize(
      compact: (context) => Scaffold(
        appBar: AppBar(),
        body: TaskDetailScreen(
          taskId: taskId,
          onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
          onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
        ),
      ),
      wide: (context) => TasksScreen(
        initialTaskId: taskId,
        onOpenTask: (id) => TaskRoute(taskId: id).go(context),
        onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
        onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
      ),
    ),
  );
}

/// Opens a note, optionally at a block or heading ([OpenNoteAt]).
void _openNote(BuildContext context, String id, [String? anchor]) =>
    NoteEditorRoute(noteId: id).go(context);

/// `/notes?folder=`: the notes list of a folder.
class NotesRoute extends GoRouteData with $NotesRoute {
  /// Creates the route for [folder] (`''` = vault root).
  const new({this.folder = ''});

  /// The folder shown.
  final String folder;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    NotesScreen(
      folder: folder,
      onOpenFolder: (path) => NotesRoute(folder: path).go(context),
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenLink: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenConflict: (opId) => ConflictRoute(opId: opId).go(context),
      onOpenLocalMap: (id) => MindMapRoute(noteId: id).go(context),
    ),
  );
}

/// `/notes/:noteId`: a note (list + detail on wider windows, the note alone
/// on compact).
class NoteEditorRoute extends GoRouteData with $NoteEditorRoute {
  /// Creates the route for [noteId].
  const new({required this.noteId});

  /// The note's stable ID (from the core).
  final String noteId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    NotesScreen(
      selectedNoteId: noteId,
      onOpenFolder: (path) => NotesRoute(folder: path).go(context),
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onCloseNote: () => const NotesRoute().go(context),
      onOpenLink: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenConflict: (opId) => ConflictRoute(opId: opId).go(context),
      onOpenLocalMap: (id) => MindMapRoute(noteId: id).go(context),
    ),
  );
}

/// `/notes/:noteId/map`: the note's local mind map.
class MindMapRoute extends GoRouteData with $MindMapRoute {
  /// Creates the route for [noteId].
  const new({required this.noteId});

  /// The focused note.
  final String noteId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    MindMapScreen(
      noteId,
      onOpenNote: (id) => MindMapRoute(noteId: id).go(context),
      onBack: () => NoteEditorRoute(noteId: noteId).go(context),
    ),
  );
}

/// `/notes/conflicts/:opId`: side-by-side conflict resolution (D19).
class ConflictRoute extends GoRouteData with $ConflictRoute {
  /// Creates the route for the conflicting op [opId].
  const new({required this.opId});

  /// The conflicting op.
  final String opId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    Scaffold(
      body: SafeArea(
        child: ConflictResolutionScreen(
          opId: opId,
          onClose: () => const NotesRoute().go(context),
          onResolved: () => const NotesRoute().go(context),
        ),
      ),
    ),
  );
}

/// `/map`.
class MapRoute extends GoRouteData with $MapRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    MapScreen(
      onOpenNote: (id) => NoteEditorRoute(noteId: id).go(context),
      onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
    ),
  );
}

/// `/directory?tab=`: people, companies, documents and places.
class DirectoryRoute extends GoRouteData with $DirectoryRoute {
  /// Creates the route showing [tab] first.
  const new({this.tab = DirectoryTab.people});

  /// The tab shown first.
  final DirectoryTab tab;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    DirectoryScreen(
      initialTab: tab,
      onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
      onOpenNote: (id, anchor) => _openNote(context, id, anchor),
      onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
    ),
  );
}

/// `/directory/:entityId`: a person or company page.
class EntityRoute extends GoRouteData with $EntityRoute {
  /// Creates the route for [entityId].
  const new({required this.entityId});

  /// The entity's note ID.
  final String entityId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _BySize(
      compact: (context) => EntityScreen(
        entityId,
        onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
        onOpenNote: (id, anchor) => _openNote(context, id, anchor),
        onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
        onBack: () => const DirectoryRoute().go(context),
      ),
      wide: (context) => DirectoryScreen(
        selectedId: entityId,
        onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
        onOpenNote: (id, anchor) => _openNote(context, id, anchor),
        onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
      ),
    ),
  );
}

/// `/directory/documents/:documentId`: a document page.
class DocumentRoute extends GoRouteData with $DocumentRoute {
  /// Creates the route for [documentId].
  const new({required this.documentId});

  /// The document's note ID.
  final String documentId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    DocumentScreen(
      documentId,
      onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
      onOpenNote: (id, anchor) => _openNote(context, id, anchor),
      onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
      onBack: () =>
          const DirectoryRoute(tab: DirectoryTab.documents).go(context),
    ),
  );
}

/// `/directory/places/:placeId`: a place page.
class PlaceRoute extends GoRouteData with $PlaceRoute {
  /// Creates the route for [placeId].
  const new({required this.placeId});

  /// The place's note ID.
  final String placeId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    PlaceScreen(
      placeId,
      onOpenEntity: (id) => EntityRoute(entityId: id).go(context),
      onOpenNote: (id, anchor) => _openNote(context, id, anchor),
      onOpenMindMap: (id) => MindMapRoute(noteId: id).go(context),
      onBack: () => const DirectoryRoute(tab: DirectoryTab.places).go(context),
    ),
  );
}

/// `/ask`.
class AskRoute extends GoRouteData with $AskRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const AskScreen());
}

/// Builds the settings screen for [section] with the app's navigation.
Widget _settings(BuildContext context, SettingsSection? section) =>
    SettingsScreen(
      section: section,
      adminPane: const AdminUsersScreen(embedded: true),
      onSelectSection: (next) => switch (next) {
        null => const SettingsRoute().go(context),
        SettingsSection.admin when SizeClass.of(context) == SizeClass.compact =>
          const AdminUsersRoute().go(context),
        final section => SettingsSectionRoute(
          section: section.name,
        ).go(context),
      },
      onOpenConflict: (opId) => ConflictRoute(opId: opId).go(context),
    );

/// `/settings`.
class SettingsRoute extends GoRouteData with $SettingsRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, Builder(builder: (context) => _settings(context, null)));
}

/// `/settings/:section` (`account`, `devices`, `reminders`, `ai`,
/// `integrity`, `data`, `sync`, `admin`, `about`).
class SettingsSectionRoute extends GoRouteData with $SettingsSectionRoute {
  /// Creates the route for [section].
  const new({required this.section});

  /// The section's name (`SettingsSection.name`).
  final String section;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    Builder(
      builder: (context) =>
          _settings(context, SettingsSection.tryParse(section)),
    ),
  );
}

/// `/settings/admin/users` (admins only; the core decides availability):
/// full screen on compact, the Settings pane on wider windows.
class AdminUsersRoute extends GoRouteData with $AdminUsersRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _BySize(
      compact: (_) => const AdminUsersScreen(),
      wide: (context) => _settings(context, SettingsSection.admin),
    ),
  );
}

// ---------------------------------------------------------------------------
// Outside the shell
// ---------------------------------------------------------------------------

/// `/sync`: sync status and conflicts as a page.
@TypedGoRoute<SyncRoute>(path: '/sync')
class SyncRoute extends GoRouteData with $SyncRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _StandaloneScreen(
      title: (l10n) => l10n.featureSync,
      child: SyncScreen(
        onOpenConflict: (opId) => ConflictRoute(opId: opId).go(context),
      ),
    ),
  );
}

/// `/sign-in`.
@TypedGoRoute<SignInRoute>(path: '/sign-in')
class SignInRoute extends GoRouteData with $SignInRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    SignInScreen(
      onCreateAccount: (server) => SignUpRoute(server: server).go(context),
      onPendingApproval: (request, {required rejected}) =>
          ApprovalRoute(rejected: rejected, $extra: request).go(context),
    ),
  );
}

/// `/sign-up`.
@TypedGoRoute<SignUpRoute>(path: '/sign-up')
class SignUpRoute extends GoRouteData with $SignUpRoute {
  /// Creates the route (with the server URL typed on sign-in).
  const new({this.server});

  /// Server URL prefill.
  final String? server;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    SignUpScreen(
      serverUrl: server,
      onBack: () => const SignInRoute().go(context),
      onRequested: (request) => ApprovalRoute($extra: request).go(context),
    ),
  );
}

/// `/approval`: waiting for approval (or not approved). The sign-in to
/// retry travels in memory (`extra`), never in the URL.
@TypedGoRoute<ApprovalRoute>(path: '/approval')
class ApprovalRoute extends GoRouteData with $ApprovalRoute {
  /// Creates the route.
  const new({this.rejected = false, this.$extra});

  /// Shows the "not approved" variant.
  final bool rejected;

  /// The sign-in "Check again" retries.
  final SignInRequest? $extra;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) {
    final request = $extra;
    return _page(
      state,
      request == null
          ? SignInScreen(
              onCreateAccount: (server) =>
                  SignUpRoute(server: server).go(context),
            )
          : PendingApprovalScreen(
              request: request,
              rejected: rejected,
              onUseAnotherAccount: () => const SignInRoute().go(context),
            ),
    );
  }
}

/// `/account-disabled` (§12.7).
@TypedGoRoute<AccountDisabledRoute>(path: '/account-disabled')
class AccountDisabledRoute extends GoRouteData with $AccountDisabledRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const AccountDisabledScreen());
}

/// `/deletion-pending` (§11 screen 15).
@TypedGoRoute<DeletionPendingRoute>(path: '/deletion-pending')
class DeletionPendingRoute extends GoRouteData with $DeletionPendingRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const DeletionPendingScreen());
}

/// `/password-change`: an admin reset the password.
@TypedGoRoute<PasswordChangeRoute>(path: '/password-change')
class PasswordChangeRoute extends GoRouteData with $PasswordChangeRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const PasswordChangeRequiredScreen());
}
