import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/strata.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'helpers/boot.dart';
import 'helpers/fakes.dart';
import 'helpers/matrix.dart';

void main() {
  group('startup', () {
    testWidgets('splash while the core loads, then initCore', (tester) async {
      final gate = Completer<void>();
      final app = await boot(tester, bootstrap: FakeBootstrap(gate: gate));
      expect(find.byType(SplashScreen), findsOneWidget);
      expect(find.bySemanticsLabel('Opening Strata'), findsOneWidget);
      expect(app.fake.calls, isEmpty);
      gate.complete();
      await settle(tester);
      expect(find.byType(SplashScreen), findsNothing);
      expect(
        app.fake.calls.first,
        const CoreCall('initCore', {'config': StrataFixtures.coreConfig}),
      );
      expect(find.byType(AdaptiveScaffold), findsOneWidget);
    });

    testWidgets('a failed start shows the code and retries', (tester) async {
      final bootstrap = FakeBootstrap(
        error: const CoreFailure(code: 'storage', messageKey: 'error.storage'),
      );
      await boot(tester, bootstrap: bootstrap);
      expect(find.byType(BootFailedScreen), findsOneWidget);
      expect(
        find.text('The local database could not be opened (storage).'),
        findsOneWidget,
      );
      await expectAccessible(tester);
      bootstrap.error = null;
      await tester.tap(find.text('Retry'));
      await settle(tester);
      expect(bootstrap.loads, 2);
      expect(find.byType(AdaptiveScaffold), findsOneWidget);
    });

    testWidgets('an initCore failure is shown too', (tester) async {
      final fake = FakeCoreApi()..initCoreAnswer.throws(StateError('boom'));
      await boot(tester, fake: fake, stubInit: false);
      expect(
        find.text('The local database could not be opened (internal).'),
        findsOneWidget,
      );
    });
  });

  group('routing by session state', () {
    final cases = <(SessionState, Type, String)>[
      (StrataFixtures.sessionSignedOut, SignInScreen, '/sign-in'),
      (StrataFixtures.sessionActive, AdaptiveScaffold, '/home'),
      (
        StrataFixtures.sessionDeletionPending,
        DeletionPendingScreen,
        '/deletion-pending',
      ),
      (
        const SessionState(
          kind: SessionKind.disabled,
          account: StrataFixtures.accountSummary,
          knownAccounts: [],
          deviceName: 'shawket-laptop',
          unsyncedOps: 1,
        ),
        AccountDisabledScreen,
        '/account-disabled',
      ),
      (
        const SessionState(
          kind: SessionKind.passwordChangeRequired,
          account: StrataFixtures.accountSummary,
          knownAccounts: [],
          deviceName: 'shawket-laptop',
          unsyncedOps: 0,
        ),
        PasswordChangeRequiredScreen,
        '/password-change',
      ),
      (
        const SessionState(
          kind: SessionKind.notInitialised,
          knownAccounts: [],
          deviceName: '',
          unsyncedOps: 0,
        ),
        SignInScreen,
        '/sign-in',
      ),
    ];
    for (final (session, screen, location) in cases) {
      testWidgets('${session.kind.name} → $location', (tester) async {
        final app = await boot(tester, session: session);
        expect(find.byType(screen), findsOneWidget);
        expect(app.router.state.uri.path, location);
        // Every other place redirects back.
        await go(tester, app, '/settings');
        expect(
          app.router.state.uri.path,
          session.kind == SessionKind.active ? '/settings' : location,
        );
      });
    }

    testWidgets('the session stream moves between states', (tester) async {
      final app = await boot(tester, session: StrataFixtures.sessionSignedOut);
      expect(find.byType(SignInScreen), findsOneWidget);
      app.fake.session.add(StrataFixtures.sessionActive);
      await settle(tester);
      expect(find.byType(AdaptiveScaffold), findsOneWidget);
      expect(app.router.state.uri.path, '/home');
      app.fake.session.add(StrataFixtures.sessionDeletionPending);
      await settle(tester);
      expect(find.byType(DeletionPendingScreen), findsOneWidget);
      app.fake.session.add(StrataFixtures.sessionSignedOut);
      await settle(tester);
      expect(find.byType(SignInScreen), findsOneWidget);
    });

    testWidgets('sign-up and approval stay reachable while signed out', (
      tester,
    ) async {
      final app = await boot(tester, session: StrataFixtures.sessionSignedOut);
      await tapVisible(tester, find.text('Create an account'));
      expect(find.byType(SignUpScreen), findsOneWidget);
      expect(app.router.state.uri.path, '/sign-up');
      expect(
        app.router.state.uri.queryParameters['server'],
        StrataFixtures.serverUrl,
      );
      await tester.tap(find.byTooltip('Back to sign in'));
      await settle(tester);
      expect(find.byType(SignInScreen), findsOneWidget);
      // A pending account leads to the approval screen with the request.
      app.fake.signInAnswer.throws(
        const CoreFailure(
          code: 'account_pending',
          messageKey: 'error.account_pending',
        ),
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Username'),
        'shawket',
      );
      await tapVisible(tester, find.widgetWithText(FilledButton, 'Sign in'));
      expect(find.byType(PendingApprovalScreen), findsOneWidget);
      expect(app.router.state.uri.path, '/approval');
      await tapVisible(tester, find.text('Check again'));
      expect(app.fake.calls.last.method, 'signIn');
      await tapVisible(tester, find.text('Use a different account'));
      expect(find.byType(SignInScreen), findsOneWidget);
    });

    testWidgets('the account UI language picks the locale', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(
          const SessionState(
            kind: SessionKind.active,
            account: AccountSummary(
              userId: 'u-1',
              username: 'mona',
              displayName: 'Mona',
              role: 'member',
              isAdmin: false,
              serverUrl: StrataFixtures.serverUrl,
              timezone: 'Africa/Cairo',
              uiLanguage: 'ar',
            ),
            knownAccounts: [],
            deviceName: 'd',
            unsyncedOps: 0,
          ),
        );
      tester.view
        ..physicalSize = StrataTestSizes.compact
        ..devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await _bootWithoutLocale(tester, fake);
      expect(
        Directionality.of(tester.element(find.byType(AdaptiveScaffold))),
        TextDirection.rtl,
      );
    });
  });
}

Future<void> _bootWithoutLocale(WidgetTester tester, FakeCoreApi fake) async {
  final app = await boot(tester, fake: fake, emitSession: false);
  // Re-pump without a forced locale.
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: app.container,
      child: const StrataApp(),
    ),
  );
  await settle(tester);
}
