// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'routes.dart';

// **************************************************************************
// GoRouterGenerator
// **************************************************************************

List<RouteBase> get $appRoutes => [$appShellRoute, $syncRoute, $signInRoute];

RouteBase get $appShellRoute => StatefulShellRouteData.$route(
  factory: $AppShellRouteExtension._fromState,
  branches: [
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/home',
          hasOverriddenOnExit: false,
          factory: $HomeRoute._fromState,
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/inbox',
          hasOverriddenOnExit: false,
          factory: $InboxRoute._fromState,
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/tasks',
          hasOverriddenOnExit: false,
          factory: $TasksRoute._fromState,
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/notes',
          hasOverriddenOnExit: false,
          factory: $NotesRoute._fromState,
          routes: [
            GoRouteData.$route(
              path: ':noteId',
              hasOverriddenOnExit: false,
              factory: $NoteEditorRoute._fromState,
            ),
          ],
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/map',
          hasOverriddenOnExit: false,
          factory: $MapRoute._fromState,
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/directory',
          hasOverriddenOnExit: false,
          factory: $DirectoryRoute._fromState,
          routes: [
            GoRouteData.$route(
              path: 'documents',
              hasOverriddenOnExit: false,
              factory: $DocumentsRoute._fromState,
            ),
          ],
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/ask',
          hasOverriddenOnExit: false,
          factory: $AskRoute._fromState,
        ),
      ],
    ),
    StatefulShellBranchData.$branch(
      routes: [
        GoRouteData.$route(
          path: '/settings',
          hasOverriddenOnExit: false,
          factory: $SettingsRoute._fromState,
          routes: [
            GoRouteData.$route(
              path: 'admin/users',
              hasOverriddenOnExit: false,
              factory: $AdminUsersRoute._fromState,
            ),
          ],
        ),
      ],
    ),
  ],
);

extension $AppShellRouteExtension on AppShellRoute {
  static AppShellRoute _fromState(GoRouterState state) => const AppShellRoute();
}

mixin $HomeRoute on GoRouteData {
  static HomeRoute _fromState(GoRouterState state) => const HomeRoute();

  @override
  String get location => GoRouteData.$location('/home');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $InboxRoute on GoRouteData {
  static InboxRoute _fromState(GoRouterState state) => const InboxRoute();

  @override
  String get location => GoRouteData.$location('/inbox');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $TasksRoute on GoRouteData {
  static TasksRoute _fromState(GoRouterState state) => const TasksRoute();

  @override
  String get location => GoRouteData.$location('/tasks');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $NotesRoute on GoRouteData {
  static NotesRoute _fromState(GoRouterState state) => const NotesRoute();

  @override
  String get location => GoRouteData.$location('/notes');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $NoteEditorRoute on GoRouteData {
  static NoteEditorRoute _fromState(GoRouterState state) =>
      NoteEditorRoute(noteId: state.pathParameters['noteId']!);

  NoteEditorRoute get _self => this as NoteEditorRoute;

  @override
  String get location =>
      GoRouteData.$location('/notes/${Uri.encodeComponent(_self.noteId)}');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $MapRoute on GoRouteData {
  static MapRoute _fromState(GoRouterState state) => const MapRoute();

  @override
  String get location => GoRouteData.$location('/map');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $DirectoryRoute on GoRouteData {
  static DirectoryRoute _fromState(GoRouterState state) =>
      const DirectoryRoute();

  @override
  String get location => GoRouteData.$location('/directory');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $DocumentsRoute on GoRouteData {
  static DocumentsRoute _fromState(GoRouterState state) =>
      const DocumentsRoute();

  @override
  String get location => GoRouteData.$location('/directory/documents');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $AskRoute on GoRouteData {
  static AskRoute _fromState(GoRouterState state) => const AskRoute();

  @override
  String get location => GoRouteData.$location('/ask');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $SettingsRoute on GoRouteData {
  static SettingsRoute _fromState(GoRouterState state) => const SettingsRoute();

  @override
  String get location => GoRouteData.$location('/settings');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

mixin $AdminUsersRoute on GoRouteData {
  static AdminUsersRoute _fromState(GoRouterState state) =>
      const AdminUsersRoute();

  @override
  String get location => GoRouteData.$location('/settings/admin/users');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

RouteBase get $syncRoute => GoRouteData.$route(
  path: '/sync',
  hasOverriddenOnExit: false,
  factory: $SyncRoute._fromState,
);

mixin $SyncRoute on GoRouteData {
  static SyncRoute _fromState(GoRouterState state) => const SyncRoute();

  @override
  String get location => GoRouteData.$location('/sync');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}

RouteBase get $signInRoute => GoRouteData.$route(
  path: '/sign-in',
  hasOverriddenOnExit: false,
  factory: $SignInRoute._fromState,
);

mixin $SignInRoute on GoRouteData {
  static SignInRoute _fromState(GoRouterState state) => const SignInRoute();

  @override
  String get location => GoRouteData.$location('/sign-in');

  @override
  void go(BuildContext context) => context.go(location);

  @override
  Future<T?> push<T>(BuildContext context) => context.push<T>(location);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location);

  @override
  void replace(BuildContext context) => context.replace(location);
}
