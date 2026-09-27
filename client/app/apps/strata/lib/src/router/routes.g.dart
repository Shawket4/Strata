// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'routes.dart';

// **************************************************************************
// GoRouterGenerator
// **************************************************************************

List<RouteBase> get $appRoutes => [
  $appShellRoute,
  $syncRoute,
  $signInRoute,
  $signUpRoute,
  $approvalRoute,
  $accountDisabledRoute,
  $deletionPendingRoute,
  $passwordChangeRoute,
];

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
          routes: [
            GoRouteData.$route(
              path: ':taskId',
              hasOverriddenOnExit: false,
              factory: $TaskRoute._fromState,
            ),
          ],
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
              path: 'conflicts/:opId',
              hasOverriddenOnExit: false,
              factory: $ConflictRoute._fromState,
            ),
            GoRouteData.$route(
              path: ':noteId',
              hasOverriddenOnExit: false,
              factory: $NoteEditorRoute._fromState,
              routes: [
                GoRouteData.$route(
                  path: 'map',
                  hasOverriddenOnExit: false,
                  factory: $MindMapRoute._fromState,
                ),
              ],
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
              path: 'documents/:documentId',
              hasOverriddenOnExit: false,
              factory: $DocumentRoute._fromState,
            ),
            GoRouteData.$route(
              path: 'places/:placeId',
              hasOverriddenOnExit: false,
              factory: $PlaceRoute._fromState,
            ),
            GoRouteData.$route(
              path: ':entityId',
              hasOverriddenOnExit: false,
              factory: $EntityRoute._fromState,
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
            GoRouteData.$route(
              path: ':section',
              hasOverriddenOnExit: false,
              factory: $SettingsSectionRoute._fromState,
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

mixin $TaskRoute on GoRouteData {
  static TaskRoute _fromState(GoRouterState state) =>
      TaskRoute(taskId: state.pathParameters['taskId']!);

  TaskRoute get _self => this as TaskRoute;

  @override
  String get location =>
      GoRouteData.$location('/tasks/${Uri.encodeComponent(_self.taskId)}');

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
  static NotesRoute _fromState(GoRouterState state) =>
      NotesRoute(folder: state.uri.queryParameters['folder'] ?? '');

  NotesRoute get _self => this as NotesRoute;

  @override
  String get location => GoRouteData.$location(
    '/notes',
    queryParams: {if (_self.folder != '') 'folder': _self.folder},
  );

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

mixin $ConflictRoute on GoRouteData {
  static ConflictRoute _fromState(GoRouterState state) =>
      ConflictRoute(opId: state.pathParameters['opId']!);

  ConflictRoute get _self => this as ConflictRoute;

  @override
  String get location => GoRouteData.$location(
    '/notes/conflicts/${Uri.encodeComponent(_self.opId)}',
  );

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

mixin $MindMapRoute on GoRouteData {
  static MindMapRoute _fromState(GoRouterState state) =>
      MindMapRoute(noteId: state.pathParameters['noteId']!);

  MindMapRoute get _self => this as MindMapRoute;

  @override
  String get location =>
      GoRouteData.$location('/notes/${Uri.encodeComponent(_self.noteId)}/map');

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
  static DirectoryRoute _fromState(GoRouterState state) => DirectoryRoute(
    tab:
        _$convertMapValue(
          'tab',
          state.uri.queryParameters,
          _$DirectoryTabEnumMap._$fromName,
        ) ??
        DirectoryTab.people,
  );

  DirectoryRoute get _self => this as DirectoryRoute;

  @override
  String get location => GoRouteData.$location(
    '/directory',
    queryParams: {
      if (_self.tab != DirectoryTab.people)
        'tab': _$DirectoryTabEnumMap[_self.tab],
    },
  );

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

const _$DirectoryTabEnumMap = {
  DirectoryTab.people: 'people',
  DirectoryTab.companies: 'companies',
  DirectoryTab.documents: 'documents',
  DirectoryTab.places: 'places',
};

mixin $DocumentRoute on GoRouteData {
  static DocumentRoute _fromState(GoRouterState state) =>
      DocumentRoute(documentId: state.pathParameters['documentId']!);

  DocumentRoute get _self => this as DocumentRoute;

  @override
  String get location => GoRouteData.$location(
    '/directory/documents/${Uri.encodeComponent(_self.documentId)}',
  );

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

mixin $PlaceRoute on GoRouteData {
  static PlaceRoute _fromState(GoRouterState state) =>
      PlaceRoute(placeId: state.pathParameters['placeId']!);

  PlaceRoute get _self => this as PlaceRoute;

  @override
  String get location => GoRouteData.$location(
    '/directory/places/${Uri.encodeComponent(_self.placeId)}',
  );

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

mixin $EntityRoute on GoRouteData {
  static EntityRoute _fromState(GoRouterState state) =>
      EntityRoute(entityId: state.pathParameters['entityId']!);

  EntityRoute get _self => this as EntityRoute;

  @override
  String get location => GoRouteData.$location(
    '/directory/${Uri.encodeComponent(_self.entityId)}',
  );

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

mixin $SettingsSectionRoute on GoRouteData {
  static SettingsSectionRoute _fromState(GoRouterState state) =>
      SettingsSectionRoute(section: state.pathParameters['section']!);

  SettingsSectionRoute get _self => this as SettingsSectionRoute;

  @override
  String get location =>
      GoRouteData.$location('/settings/${Uri.encodeComponent(_self.section)}');

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

T? _$convertMapValue<T>(
  String key,
  Map<String, String> map,
  T? Function(String) converter,
) {
  final value = map[key];
  return value == null ? null : converter(value);
}

extension<T extends Enum> on Map<T, String> {
  T? _$fromName(String? value) =>
      entries.where((element) => element.value == value).firstOrNull?.key;
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

RouteBase get $signUpRoute => GoRouteData.$route(
  path: '/sign-up',
  hasOverriddenOnExit: false,
  factory: $SignUpRoute._fromState,
);

mixin $SignUpRoute on GoRouteData {
  static SignUpRoute _fromState(GoRouterState state) =>
      SignUpRoute(server: state.uri.queryParameters['server']);

  SignUpRoute get _self => this as SignUpRoute;

  @override
  String get location => GoRouteData.$location(
    '/sign-up',
    queryParams: {if (_self.server != null) 'server': _self.server},
  );

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

RouteBase get $approvalRoute => GoRouteData.$route(
  path: '/approval',
  hasOverriddenOnExit: false,
  factory: $ApprovalRoute._fromState,
);

mixin $ApprovalRoute on GoRouteData {
  static ApprovalRoute _fromState(GoRouterState state) => ApprovalRoute(
    rejected:
        _$convertMapValue(
          'rejected',
          state.uri.queryParameters,
          _$boolConverter,
        ) ??
        false,
    $extra: state.extra as SignInRequest?,
  );

  ApprovalRoute get _self => this as ApprovalRoute;

  @override
  String get location => GoRouteData.$location(
    '/approval',
    queryParams: {
      if (_self.rejected != false) 'rejected': _self.rejected.toString(),
    },
  );

  @override
  void go(BuildContext context) => context.go(location, extra: _self.$extra);

  @override
  Future<T?> push<T>(BuildContext context) =>
      context.push<T>(location, extra: _self.$extra);

  @override
  void pushReplacement(BuildContext context) =>
      context.pushReplacement(location, extra: _self.$extra);

  @override
  void replace(BuildContext context) =>
      context.replace(location, extra: _self.$extra);
}

bool _$boolConverter(String value) {
  switch (value) {
    case 'true':
      return true;
    case 'false':
      return false;
    default:
      throw UnsupportedError('Cannot convert "$value" into a bool.');
  }
}

RouteBase get $accountDisabledRoute => GoRouteData.$route(
  path: '/account-disabled',
  hasOverriddenOnExit: false,
  factory: $AccountDisabledRoute._fromState,
);

mixin $AccountDisabledRoute on GoRouteData {
  static AccountDisabledRoute _fromState(GoRouterState state) =>
      const AccountDisabledRoute();

  @override
  String get location => GoRouteData.$location('/account-disabled');

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

RouteBase get $deletionPendingRoute => GoRouteData.$route(
  path: '/deletion-pending',
  hasOverriddenOnExit: false,
  factory: $DeletionPendingRoute._fromState,
);

mixin $DeletionPendingRoute on GoRouteData {
  static DeletionPendingRoute _fromState(GoRouterState state) =>
      const DeletionPendingRoute();

  @override
  String get location => GoRouteData.$location('/deletion-pending');

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

RouteBase get $passwordChangeRoute => GoRouteData.$route(
  path: '/password-change',
  hasOverriddenOnExit: false,
  factory: $PasswordChangeRoute._fromState,
);

mixin $PasswordChangeRoute on GoRouteData {
  static PasswordChangeRoute _fromState(GoRouterState state) =>
      const PasswordChangeRoute();

  @override
  String get location => GoRouteData.$location('/password-change');

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
