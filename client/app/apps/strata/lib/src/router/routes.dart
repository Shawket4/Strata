import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:strata/src/shell/app_shell.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_notes/strata_notes.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_tasks/strata_tasks.dart';

part 'routes.g.dart';

/// A Material page from `package:flutter/material.dart`. go_router 18 detects
/// `material_ui`'s `MaterialApp`, so the app states its page type explicitly.
Page<void> _page(GoRouterState state, Widget child) =>
    MaterialPage<void>(key: state.pageKey, name: state.name, child: child);

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
      routes: [TypedGoRoute<TasksRoute>(path: '/tasks')],
    ),
    TypedStatefulShellBranch<NotesBranch>(
      routes: [
        TypedGoRoute<NotesRoute>(
          path: '/notes',
          routes: [TypedGoRoute<NoteEditorRoute>(path: ':noteId')],
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
          routes: [TypedGoRoute<DocumentsRoute>(path: 'documents')],
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
          routes: [TypedGoRoute<AdminUsersRoute>(path: 'admin/users')],
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
  ) => AppShell(navigationShell: navigationShell);
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
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const HomeScreen());
}

/// `/inbox`.
class InboxRoute extends GoRouteData with $InboxRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const InboxScreen());
}

/// `/tasks`.
class TasksRoute extends GoRouteData with $TasksRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const TasksScreen());
}

/// `/notes`.
class NotesRoute extends GoRouteData with $NotesRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const NotesScreen());
}

/// `/notes/:noteId`: the note editor.
class NoteEditorRoute extends GoRouteData with $NoteEditorRoute {
  /// Creates the route for [noteId].
  const new({required this.noteId});

  /// The note's stable ID (from the core).
  final String noteId;

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const NoteEditorScreen());
}

/// `/map`.
class MapRoute extends GoRouteData with $MapRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const MapScreen());
}

/// `/directory`.
class DirectoryRoute extends GoRouteData with $DirectoryRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const DirectoryScreen());
}

/// `/directory/documents`.
class DocumentsRoute extends GoRouteData with $DocumentsRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const DocumentsScreen());
}

/// `/ask`.
class AskRoute extends GoRouteData with $AskRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const AskScreen());
}

/// `/settings`.
class SettingsRoute extends GoRouteData with $SettingsRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const SettingsScreen());
}

/// `/settings/admin/users` (admins only; the core decides visibility).
class AdminUsersRoute extends GoRouteData with $AdminUsersRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) =>
      _page(state, const AdminUsersScreen());
}

/// `/sync`: sync status and conflicts, outside the shell.
@TypedGoRoute<SyncRoute>(path: '/sync')
class SyncRoute extends GoRouteData with $SyncRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _StandaloneScreen(
      title: (l10n) => l10n.featureSync,
      child: const SyncScreen(),
    ),
  );
}

/// `/sign-in`: accounts, outside the shell.
@TypedGoRoute<SignInRoute>(path: '/sign-in')
class SignInRoute extends GoRouteData with $SignInRoute {
  /// Creates the route.
  const new();

  @override
  Page<void> buildPage(BuildContext context, GoRouterState state) => _page(
    state,
    _StandaloneScreen(
      title: (l10n) => l10n.featureAccounts,
      child: const SignInScreen(),
    ),
  );
}
