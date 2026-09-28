import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

import 'helpers/fixtures.dart';

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  SessionState session, {
  FakeCoreApi? fake,
  VoidCallback? onUseAnotherAccount,
}) => pumpVariant(
  tester,
  v,
  PendingApprovalScreen(onUseAnotherAccount: onUseAnotherAccount),
  fake: (fake ?? FakeCoreApi())..session.add(session),
);

void main() {
  group('pending approval matrix', () {
    for (final v in variants()) {
      testWidgets('waiting $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await _pump(tester, v, AccountFixtures.waiting);
        expect(find.text(l10n.pendingTitle), findsOneWidget);
        expect(find.text(l10n.pendingPill), findsOneWidget);
        expect(
          find.text(
            l10n.pendingBody(username: 'sara.n', requested: '2 hours ago'),
          ),
          findsOneWidget,
        );
        expect(find.text('Last checked 14:32'), findsOneWidget);
        expect(find.text(StrataFixtures.serverUrl), findsOneWidget);
        expect(
          tester
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, l10n.checkAgain),
              )
              .onPressed,
          isNotNull,
        );
        expect(find.text(l10n.useAnotherAccount), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('not approved $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await _pump(tester, v, AccountFixtures.notApproved);
        expect(find.text(l10n.rejectedTitle), findsOneWidget);
        expect(find.text(l10n.rejectedBody), findsOneWidget);
        expect(find.text(l10n.checkAgain), findsNothing);
        expect(find.text(l10n.useAnotherAccount), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('pending approval intents', () {
    final v = variants().first;

    Future<FakeCoreApi> checkAgain(
      WidgetTester tester,
      FakeCoreApi fake,
    ) async {
      await _pump(tester, v, AccountFixtures.waiting, fake: fake);
      await tester.tap(find.text('Check again'));
      await settle(tester);
      return fake;
    }

    testWidgets('Check again asks the core', (tester) async {
      final fake = await checkAgain(tester, FakeCoreApi());
      expect(fake.calls.last, const CoreCall('checkApproval'));
    });

    testWidgets('still pending: a notice', (tester) async {
      await checkAgain(
        tester,
        FakeCoreApi()..checkApprovalAnswer.throws(AccountFixtures.pending),
      );
      expect(find.text('Still waiting for approval.'), findsOneWidget);
      expect(find.text('Waiting for approval'), findsOneWidget);
    });

    testWidgets('rejected meanwhile: the session shows the variant', (
      tester,
    ) async {
      final fake = await checkAgain(
        tester,
        FakeCoreApi()..checkApprovalAnswer.throws(AccountFixtures.rejected),
      );
      expect(find.byType(SnackBar), findsNothing);
      fake.session.add(AccountFixtures.notApproved);
      await settle(tester);
      expect(
        find.text("This account request wasn't approved."),
        findsOneWidget,
      );
    });

    testWidgets('another failure is shown', (tester) async {
      await checkAgain(
        tester,
        FakeCoreApi()
          ..checkApprovalAnswer.throws(AccountFixtures.invalidCredentials),
      );
      expect(find.text('Username or password is incorrect'), findsOneWidget);
    });

    testWidgets('without the sign-in in memory, Check again is off', (
      tester,
    ) async {
      await _pump(tester, v, AccountFixtures.waitingNoRetry);
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Check again'),
            )
            .onPressed,
        isNull,
      );
      expect(find.textContaining('Last checked'), findsNothing);
    });

    for (final session in [
      AccountFixtures.waiting,
      AccountFixtures.notApproved,
    ]) {
      testWidgets('Use a different account dismisses ${session.kind}', (
        tester,
      ) async {
        var other = 0;
        final fake = await _pump(
          tester,
          v,
          session,
          onUseAnotherAccount: () => other++,
        );
        await tapVisible(tester, find.text('Use a different account'));
        expect(fake.calls.last, const CoreCall('dismissPending'));
        expect(other, 1);
      });
    }
  });
}
