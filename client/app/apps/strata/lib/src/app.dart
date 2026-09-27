import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/router/router.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// The Strata app: Material 3 themes from the design tokens, English and
/// Arabic UI, typed go_router navigation.
class StrataApp extends ConsumerWidget {
  /// Creates the app.
  const new({super.key, this.locale});

  /// Forces a UI locale (tests); `null` follows the system.
  final Locale? locale;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp.router(
      routerConfig: ref.watch(appRouterProvider),
      onGenerateTitle: (context) => context.l10n.appTitle,
      debugShowCheckedModeBanner: false,
      theme: StrataTheme.light(),
      darkTheme: StrataTheme.dark(),
      locale: locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
    );
  }
}
