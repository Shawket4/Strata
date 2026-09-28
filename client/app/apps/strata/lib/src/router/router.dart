import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata/src/boot/core_bootstrap.dart';
import 'package:strata/src/router/routes.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

part 'router.g.dart';

/// Where [kind] may be, as location prefixes, and where it lands otherwise
/// (1:1 on the core's session kind).
({List<String> allowed, String home}) sessionPlaces(SessionKind kind) =>
    switch (kind) {
      // The pending request lives in the session: the approval screen until
      // the core says otherwise (approved, or dismissed → signed out).
      SessionKind.pendingApproval || SessionKind.rejected => (
        allowed: [const ApprovalRoute().location],
        home: const ApprovalRoute().location,
      ),
      SessionKind.notInitialised || SessionKind.signedOut => (
        allowed: [const SignInRoute().location, const SignUpRoute().location],
        home: const SignInRoute().location,
      ),
      SessionKind.active => (
        allowed: [
          const HomeRoute().location,
          const InboxRoute().location,
          const TasksRoute().location,
          const NotesRoute().location,
          const MapRoute().location,
          const DirectoryRoute().location,
          const AskRoute().location,
          const SettingsRoute().location,
          const SyncRoute().location,
        ],
        home: const HomeRoute().location,
      ),
      SessionKind.passwordChangeRequired => (
        allowed: [const PasswordChangeRoute().location],
        home: const PasswordChangeRoute().location,
      ),
      SessionKind.disabled => (
        allowed: [const AccountDisabledRoute().location],
        home: const AccountDisabledRoute().location,
      ),
      SessionKind.deletionPending => (
        allowed: [const DeletionPendingRoute().location],
        home: const DeletionPendingRoute().location,
      ),
    };

/// The redirect for [location] under [session]: `null` keeps it.
String? sessionRedirect(SessionState? session, Uri location) {
  if (session == null) return null;
  final places = sessionPlaces(session.kind);
  final path = location.path;
  final ok = places.allowed.any(
    (prefix) => path == prefix || path.startsWith('$prefix/'),
  );
  return ok ? null : places.home;
}

/// The app's [GoRouter] with the generated typed routes, routed by the
/// session state (§12.7): signed out → sign in, disabled / deletion pending
/// / password change → their restricted screen, active → the shell.
@Riverpod(keepAlive: true)
GoRouter appRouter(Ref ref) {
  final session = ValueNotifier<SessionState?>(
    ref.read(coreStartupProvider).value,
  );
  ref
    ..listen(sessionProvider, (_, next) {
      if (next case AsyncData(:final value)) session.value = value;
    })
    ..onDispose(session.dispose);
  final router = GoRouter(
    initialLocation: sessionPlaces(
      session.value?.kind ?? SessionKind.notInitialised,
    ).home,
    routes: $appRoutes,
    refreshListenable: session,
    redirect: (context, state) => sessionRedirect(session.value, state.uri),
    errorBuilder: (context, state) => Scaffold(
      body: StrataEmptyState(
        icon: Icons.error_outline,
        title: context.l10n.errorPageNotFound,
        message: state.uri.toString(),
      ),
    ),
  );
  ref.onDispose(router.dispose);
  return router;
}
