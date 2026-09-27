import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

void main() {
  group('pending approval matrix', () {
    for (final v in matrix()) {
      for (final rejected in [false, true]) {
        testWidgets('${rejected ? 'not approved' : 'waiting'} $v', (
          tester,
        ) async {
          final l10n = lookupAccountsLocalizations(v.locale);
          await pumpVariant(
            tester,
            v,
            PendingApprovalScreen(
              request: StrataFixtures.signInRequest,
              rejected: rejected,
              onUseAnotherAccount: () {},
            ),
          );
          if (rejected) {
            expect(find.text(l10n.rejectedTitle), findsOneWidget);
            expect(find.text(l10n.checkAgain), findsNothing);
          } else {
            expect(find.text(l10n.pendingTitle), findsOneWidget);
            expect(find.text(l10n.pendingPill), findsOneWidget);
            expect(
              find.text(l10n.pendingBody(username: 'shawket')),
              findsOneWidget,
            );
            expect(find.text(l10n.checkAgain), findsOneWidget);
          }
          expect(find.text(l10n.useAnotherAccount), findsOneWidget);
          expectNoErrors(tester);
          await expectAccessible(tester, contrast: v.textScale == 1);
        });
      }
    }
  });

  group('pending approval intents', () {
    final v = matrix().first;

    Future<FakeCoreApi> pump(WidgetTester tester, FakeCoreApi fake) async {
      await pumpVariant(
        tester,
        v,
        const PendingApprovalScreen(request: StrataFixtures.signInRequest),
        fake: fake,
      );
      await tester.tap(find.text('Check again'));
      await settle(tester);
      return fake;
    }

    testWidgets('Check again retries the sign-in', (tester) async {
      final fake = await pump(tester, FakeCoreApi());
      expect(
        fake.calls.last,
        const CoreCall('signIn', {'request': StrataFixtures.signInRequest}),
      );
    });

    testWidgets('still pending: a notice', (tester) async {
      await pump(
        tester,
        FakeCoreApi()..signInAnswer.throws(AccountFixtures.pending),
      );
      expect(find.text('Still waiting for approval.'), findsOneWidget);
      expect(find.text('Waiting for approval'), findsOneWidget);
    });

    testWidgets('rejected meanwhile: the not-approved variant', (tester) async {
      await pump(
        tester,
        FakeCoreApi()..signInAnswer.throws(AccountFixtures.rejected),
      );
      expect(
        find.text("This account request wasn't approved."),
        findsOneWidget,
      );
    });

    testWidgets('another failure is shown', (tester) async {
      await pump(
        tester,
        FakeCoreApi()..signInAnswer.throws(AccountFixtures.invalidCredentials),
      );
      expect(find.text('Username or password is incorrect'), findsOneWidget);
    });

    testWidgets('Use a different account', (tester) async {
      var other = 0;
      await pumpVariant(
        tester,
        v,
        PendingApprovalScreen(
          request: StrataFixtures.signInRequest,
          onUseAnotherAccount: () => other++,
        ),
      );
      await tester.tap(find.text('Use a different account'));
      expect(other, 1);
      expect(find.byType(PendingApprovalScreen), findsOneWidget);
    });
  });
}
