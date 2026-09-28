import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/boot/core_bootstrap.dart';
import 'package:strata/src/l10n.dart';
import 'package:strata/src/router/router.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// The Strata app: Material 3 themes from the design tokens, English and
/// Arabic UI, typed go_router navigation routed by the session.
///
/// While the Rust core loads (`coreStartupProvider`) a splash is shown; a
/// failed start shows the core's error with Retry, or, when the build has no
/// usable server address, [MisconfiguredBuildScreen].
class StrataApp extends ConsumerWidget {
  /// Creates the app.
  const new({super.key, this.locale});

  /// Forces a UI locale (tests); `null` follows the account's UI language,
  /// else the system.
  final Locale? locale;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final startup = ref.watch(coreStartupProvider);
    // The session stream exists only once the core is open.
    final language = startup.hasValue
        ? ref.watch(sessionProvider).value?.account?.uiLanguage
        : null;
    final uiLocale = locale ?? (language == null ? null : Locale(language));
    Widget app({required Widget home}) => MaterialApp(
      onGenerateTitle: (context) => context.l10n.appTitle,
      debugShowCheckedModeBanner: false,
      theme: StrataTheme.light(),
      darkTheme: StrataTheme.dark(),
      locale: uiLocale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      home: home,
    );
    return switch (startup) {
      AsyncData() => MaterialApp.router(
        routerConfig: ref.watch(appRouterProvider),
        onGenerateTitle: (context) => context.l10n.appTitle,
        debugShowCheckedModeBanner: false,
        theme: StrataTheme.light(),
        darkTheme: StrataTheme.dark(),
        locale: uiLocale,
        supportedLocales: StrataLocalizations.supportedLocales,
        localizationsDelegates: StrataLocalizations.localizationsDelegates,
      ),
      AsyncError(
        error: CoreFailure(code: 'misconfigured_build', :final reason),
      ) =>
        app(home: MisconfiguredBuildScreen(reason: reason ?? '')),
      AsyncError(:final error) => app(
        home: BootFailedScreen(
          error: error,
          onRetry: () => ref.invalidate(coreStartupProvider),
        ),
      ),
      _ => app(home: const SplashScreen()),
    };
  }
}

/// Shown while the core loads.
class SplashScreen extends StatelessWidget {
  /// Creates the splash.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Scaffold(
      backgroundColor: colors.background,
      body: Column(
        children: [
          Expanded(
            child: Center(
              child: Semantics(
                label: context.appL10n.splashLoading,
                liveRegion: true,
                child: const Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    StrataWordmark(fontSize: 40),
                    SizedBox(height: StrataSpacing.s6),
                    SizedBox.square(
                      dimension: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ],
                ),
              ),
            ),
          ),
          const ExcludeSemantics(
            child: SizedBox(height: 160, child: StrataBands()),
          ),
        ],
      ),
    );
  }
}

/// The core could not start: its failure code and Retry.
class BootFailedScreen extends StatelessWidget {
  /// Creates the screen for [error].
  const new({required this.error, required this.onRetry, super.key});

  /// What the start threw (a `CoreFailure` from the core).
  final Object error;

  /// Starts again.
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = context.appL10n;
    final code = switch (error) {
      CoreFailure(:final code) => code,
      _ => 'internal',
    };
    return Scaffold(
      body: StrataEmptyState(
        icon: Icons.error_outline,
        title: l10n.bootFailedTitle,
        message: l10n.bootFailedBody(code: code),
        action: StrataAction(
          label: l10n.retry,
          icon: Icons.refresh,
          onPressed: onRetry,
        ),
      ),
    );
  }
}

/// The build has no usable server address (`STRATA_SERVER_URL` missing, not
/// `https://`, or plain `http://` in a release build): the core refused to
/// start. Fatal, with no retry: only another build fixes it.
class MisconfiguredBuildScreen extends StatelessWidget {
  /// Creates the screen for the core's [reason] (`missing`, `not_https`,
  /// `insecure_http`).
  const new({required this.reason, super.key});

  /// The core's reason code.
  final String reason;

  @override
  Widget build(BuildContext context) {
    final l10n = context.appL10n;
    return Scaffold(
      body: StrataEmptyState(
        icon: Icons.dns_outlined,
        title: l10n.misconfiguredBuildTitle,
        message: l10n.misconfiguredBuildBody(reason: reason),
      ),
    );
  }
}
