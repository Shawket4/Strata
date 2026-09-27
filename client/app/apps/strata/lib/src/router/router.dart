import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata/src/router/routes.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

part 'router.g.dart';

/// The app's [GoRouter] with the generated typed routes.
@Riverpod(keepAlive: true)
GoRouter appRouter(Ref ref) {
  final router = GoRouter(
    initialLocation: const HomeRoute().location,
    routes: $appRoutes,
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
