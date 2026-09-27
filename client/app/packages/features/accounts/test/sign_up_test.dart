import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/matrix.dart';

Finder _field(String label) => find.widgetWithText(TextField, label);

Future<void> _fill(
  WidgetTester tester, {
  String confirm = 'nile-freight-2026',
  AccountsLocalizations? l10n,
}) async {
  final t = l10n ?? lookupAccountsLocalizations(const Locale('en'));
  await tester.enterText(_field(t.fieldDisplayName), 'Mona Hassan');
  await tester.enterText(_field(t.fieldUsername), 'mona');
  await tester.enterText(_field(t.fieldPassword), 'nile-freight-2026');
  await tester.enterText(_field(t.fieldConfirmPassword), confirm);
  await tester.pump();
}

Future<void> _request(WidgetTester tester) async {
  final button = find.widgetWithText(FilledButton, 'Request account');
  await tester.ensureVisible(button);
  await tester.tap(button);
  await settle(tester);
}

void main() {
  group('sign up matrix', () {
    for (final v in matrix()) {
      testWidgets('form with the approval notice $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          SignUpScreen(serverUrl: StrataFixtures.serverUrl, onBack: () {}),
          fake: FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut),
        );
        expect(find.text(l10n.createAccount), findsOneWidget);
        expect(
          find.text(l10n.signUpOn(server: StrataFixtures.serverUrl)),
          findsOneWidget,
        );
        expect(find.text(l10n.approvalNotice), findsOneWidget);
        expect(find.byTooltip(l10n.backToSignIn), findsOneWidget);
        expect(
          tester
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, l10n.requestAccount),
              )
              .onPressed,
          isNull,
        );
        expect(
          find.byType(BrandPanel),
          v.sizeClass == SizeClass.expanded ? findsOneWidget : findsNothing,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('passwords differ $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(tester, v, const SignUpScreen());
        await _fill(tester, confirm: 'nile-freight', l10n: l10n);
        expect(find.text(l10n.passwordsDiffer), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('sign up intents', () {
    final v = matrix().first;

    testWidgets('requests the account and hands over the sign-in', (
      tester,
    ) async {
      final requested = <SignInRequest>[];
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(
        tester,
        v,
        SignUpScreen(
          serverUrl: StrataFixtures.serverUrl,
          onRequested: requested.add,
        ),
        fake: fake,
      );
      await _fill(tester);
      expect(find.text('Passwords match'), findsOneWidget);
      await _request(tester);
      expect(
        fake.calls.last,
        const CoreCall('signUp', {'request': StrataFixtures.signUpRequest}),
      );
      expect(requested, [
        const SignInRequest(
          serverUrl: StrataFixtures.serverUrl,
          username: 'mona',
          password: 'nile-freight-2026',
          deviceName: 'shawket-laptop',
        ),
      ]);
    });

    testWidgets('a refused sign-up shows the core error', (tester) async {
      final fake = FakeCoreApi()
        ..signUpAnswer.throws(
          const CoreFailure(
            code: 'invalid_input',
            messageKey: 'error.invalid_input',
            field: 'username',
            reason: 'taken',
          ),
        );
      await pumpVariant(
        tester,
        v,
        const SignUpScreen(serverUrl: StrataFixtures.serverUrl),
        fake: fake,
      );
      await _fill(tester);
      await _request(tester);
      expect(find.text('Check the username field.'), findsOneWidget);
    });

    testWidgets('back and the sign-in link return', (tester) async {
      var back = 0;
      await pumpVariant(tester, v, SignUpScreen(onBack: () => back++));
      await tester.tap(find.byTooltip('Back to sign in'));
      await tapVisible(tester, find.widgetWithText(TextButton, 'Sign in'));
      expect(back, 2);
    });
  });
}
