import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

import 'helpers/fixtures.dart';

/// The owner decision "No server address in the UI": the server is fixed at
/// build time (`STRATA_SERVER_URL`), so sign-in and sign-up have no server
/// field, never show an address and never send one; the core supplies it.
void main() {
  group('no server field', () {
    for (final v in variants()) {
      testWidgets('sign in $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(AccountFixtures.signedOut);
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        // Username, password and device name only.
        expect(find.byType(TextField), findsNWidgets(3));
        expect(find.widgetWithText(TextField, l10n.fieldUsername), findsOne);
        expect(find.widgetWithText(TextField, l10n.fieldPassword), findsOne);
        expect(find.widgetWithText(TextField, l10n.fieldDeviceName), findsOne);
        expect(find.text(v.rtl ? 'الخادم' : 'Server'), findsNothing);
        expect(find.textContaining('://'), findsNothing);
        expect(find.textContaining(StrataFixtures.serverUrl), findsNothing);

        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldUsername),
          'shawket',
        );
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldPassword),
          'pw',
        );
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.signInTitle),
        );
        expect(
          fake.calls.last,
          const CoreCall('signIn', {
            'request': SignInRequest(
              username: 'shawket',
              password: 'pw',
              deviceName: 'shawket-laptop',
            ),
          }),
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('sign up $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(AccountFixtures.signedOut);
        await pumpVariant(tester, v, SignUpScreen(onBack: () {}), fake: fake);
        // Display name, username, password and its confirmation only.
        expect(find.byType(TextField), findsNWidgets(4));
        expect(find.widgetWithText(TextField, l10n.fieldDisplayName), findsOne);
        expect(find.widgetWithText(TextField, l10n.fieldUsername), findsOne);
        expect(find.widgetWithText(TextField, l10n.fieldPassword), findsOne);
        expect(
          find.widgetWithText(TextField, l10n.fieldConfirmPassword),
          findsOne,
        );
        expect(find.text(v.rtl ? 'الخادم' : 'Server'), findsNothing);
        expect(find.textContaining('://'), findsNothing);

        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldDisplayName),
          'Mona Hassan',
        );
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldUsername),
          'mona',
        );
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldPassword),
          'nile-freight-2026',
        );
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldConfirmPassword),
          'nile-freight-2026',
        );
        await settle(tester);
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.requestAccount),
        );
        expect(
          fake.calls.last,
          const CoreCall('signUp', {'request': StrataFixtures.signUpRequest}),
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });
}
