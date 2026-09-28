import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

import 'helpers/fixtures.dart';

/// What the bridge surfaces when the core panics or a transport error is
/// not mapped to a `CoreFailure`.
final StateError _untyped = StateError('PanicException: connection reset');

Finder _field(String label) => find.widgetWithText(TextField, label);

/// The button is enabled again and shows its label, not the spinner.
void _expectIdle(WidgetTester tester, String label) {
  final button = find.widgetWithText(FilledButton, label);
  expect(tester.widget<FilledButton>(button).onPressed, isNotNull);
  expect(
    find.descendant(
      of: button,
      matching: find.byType(CircularProgressIndicator),
    ),
    findsNothing,
  );
}

/// The error went to Flutter's error reporting (and only once).
void _expectReported(WidgetTester tester) {
  expect(tester.takeException(), same(_untyped));
  expect(tester.takeException(), isNull);
}

void main() {
  group('an untyped error shows the connection message and resets busy', () {
    for (final v in variants()) {
      testWidgets('sign in $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..session.add(StrataFixtures.sessionSignedOut)
          ..signInAnswer.throws(_untyped);
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        await tester.enterText(_field(l10n.fieldUsername), 'shawket');
        await tester.enterText(_field(l10n.fieldPassword), 'tide-4821-sand');
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.signInTitle),
        );
        expect(fake.calls.last.method, 'signIn');
        _expectReported(tester);
        expect(find.text(l10n.errorUnreachable), findsOneWidget);
        _expectIdle(tester, l10n.signInTitle);
      });

      testWidgets('sign up $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..session.add(StrataFixtures.sessionSignedOut)
          ..signUpAnswer.throws(_untyped);
        var requested = false;
        await pumpVariant(
          tester,
          v,
          SignUpScreen(onBack: () {}, onRequested: () => requested = true),
          fake: fake,
        );
        await tester.enterText(_field(l10n.fieldDisplayName), 'Mona Hassan');
        await tester.enterText(_field(l10n.fieldUsername), 'mona');
        await tester.enterText(_field(l10n.fieldPassword), 'nile-freight-2026');
        await tester.enterText(
          _field(l10n.fieldConfirmPassword),
          'nile-freight-2026',
        );
        await tester.pump();
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.requestAccount),
        );
        expect(fake.calls.last.method, 'signUp');
        _expectReported(tester);
        expect(requested, isFalse);
        expect(find.text(l10n.errorUnreachable), findsOneWidget);
        _expectIdle(tester, l10n.requestAccount);
      });
    }
  });

  group('the other account actions', () {
    final v = variants().first;
    final l10n = lookupAccountsLocalizations(v.locale);

    testWidgets('continue as a known account', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.signedOut)
        ..switchAccountAnswer.throws(_untyped);
      await pumpVariant(tester, v, const SignInScreen(), fake: fake);
      await tapVisible(tester, find.text(l10n.continueAs(name: 'Shawket')));
      expect(fake.calls.last.method, 'switchAccount');
      _expectReported(tester);
      expect(find.text(l10n.errorUnreachable), findsOneWidget);
    });

    testWidgets('check approval again', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.waiting)
        ..checkApprovalAnswer.throws(_untyped);
      await pumpVariant(tester, v, const PendingApprovalScreen(), fake: fake);
      await tapVisible(tester, find.text(l10n.checkAgain));
      expect(fake.calls.last, const CoreCall('checkApproval'));
      _expectReported(tester);
      expect(find.text(l10n.errorUnreachable), findsOneWidget);
      _expectIdle(tester, l10n.checkAgain);
    });

    testWidgets('change the password', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.passwordChange)
        ..changePasswordAnswer.throws(_untyped);
      await pumpVariant(
        tester,
        v,
        const PasswordChangeRequiredScreen(),
        fake: fake,
      );
      await tester.enterText(
        _field(l10n.fieldTemporaryPassword),
        'tide-4821-sand',
      );
      await tester.enterText(_field(l10n.fieldNewPassword), 'nile-2026-x');
      await tester.enterText(_field(l10n.fieldConfirmPassword), 'nile-2026-x');
      await tester.pump();
      await tapVisible(
        tester,
        find.widgetWithText(FilledButton, l10n.changePassword),
      );
      expect(fake.calls.last.method, 'changePassword');
      _expectReported(tester);
      expect(find.text(l10n.errorUnreachable), findsOneWidget);
      _expectIdle(tester, l10n.changePassword);
    });

    test('failure() renders anything untyped as the connection message', () {
      expect(l10n.failure(_untyped), l10n.errorUnreachable);
      expect(
        l10n.failure(AccountFixtures.invalidCredentials),
        l10n.errorInvalidCredentials,
      );
    });
  });
}
