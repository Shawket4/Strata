import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';

Finder _field(String label) => find.widgetWithText(TextField, label);

Future<void> _fill(WidgetTester tester) async {
  await tester.enterText(_field('Username'), 'shawket');
  await tester.enterText(_field('Password'), 'correct horse battery staple');
}

Future<void> _submit(WidgetTester tester, [String label = 'Sign in']) async {
  final button = find.widgetWithText(FilledButton, label);
  await tester.ensureVisible(button);
  await tester.tap(button);
  await settle(tester);
}

void main() {
  group('sign in matrix', () {
    for (final v in variants()) {
      testWidgets('prefilled with a known account $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(AccountFixtures.signedOut);
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        expect(find.text(l10n.signInTitle), findsWidgets);
        expect(find.text('S'), findsOneWidget);
        expect(find.textContaining('://'), findsNothing);
        expect(find.text('shawket-laptop'), findsWidgets);
        expect(find.text(l10n.continueAs(name: 'Shawket')), findsOneWidget);
        expect(find.text(l10n.createAccount), findsOneWidget);
        switch (v.sizeClass) {
          case SizeClass.compact || SizeClass.medium:
            expect(find.byType(BrandPanel), findsNothing);
            expect(find.byType(StrataBands), findsOneWidget);
            expect(
              find.byType(KeyboardHintChip),
              v.sizeClass == SizeClass.medium ? findsOneWidget : findsNothing,
            );
          case SizeClass.expanded:
            expect(find.byType(BrandPanel), findsOneWidget);
            expect(find.text(l10n.brandTagline), findsOneWidget);
            expect(find.byType(KeyboardHintChip), findsOneWidget);
        }
        expect(
          directionOf(tester, find.byType(SignInScreen)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('wrong password $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..session.add(StrataFixtures.sessionSignedOut)
          ..signInAnswer.throws(AccountFixtures.invalidCredentials);
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        await _submit(tester, l10n.signInTitle);
        expect(find.text(l10n.errorInvalidCredentials), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('sign in intents', () {
    final v = variants().first;

    testWidgets('signs in with exactly what was typed', (tester) async {
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(tester, v, const SignInScreen(), fake: fake);
      await _fill(tester);
      await _submit(tester);
      expect(
        fake.calls.last,
        const CoreCall('signIn', {'request': StrataFixtures.signInRequest}),
      );
    });

    testWidgets('Enter on the device name submits', (tester) async {
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(tester, v, const SignInScreen(), fake: fake);
      await _fill(tester);
      await tester.tap(_field('Device name'));
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      expect(fake.calls.last.method, 'signIn');
    });

    for (final failure in [AccountFixtures.pending, AccountFixtures.rejected]) {
      testWidgets('${failure.code} leads to the approval screen', (
        tester,
      ) async {
        var pending = 0;
        final fake = FakeCoreApi()
          ..session.add(StrataFixtures.sessionSignedOut)
          ..signInAnswer.throws(failure);
        await pumpVariant(
          tester,
          v,
          SignInScreen(onPendingApproval: () => pending++),
          fake: fake,
        );
        await _fill(tester);
        await _submit(tester);
        expect(pending, 1);
        expect(find.byType(FormAlert), findsNothing);
      });
    }

    testWidgets('continues as a known account', (tester) async {
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(tester, v, const SignInScreen(), fake: fake);
      await tapVisible(tester, find.text('Continue as Shawket'));
      expect(
        fake.calls.last,
        const CoreCall('switchAccount', {'userId': 'u-shawket'}),
      );
    });

    testWidgets('a failed switch shows the core error', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(StrataFixtures.sessionSignedOut)
        ..switchAccountAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await pumpVariant(tester, v, const SignInScreen(), fake: fake);
      await tapVisible(tester, find.text('Continue as Shawket'));
      expect(
        find.text("Can't reach the server. Check your connection."),
        findsOneWidget,
      );
    });

    testWidgets('Create an account opens sign-up', (tester) async {
      var opened = 0;
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(
        tester,
        v,
        SignInScreen(onCreateAccount: () => opened++),
        fake: fake,
      );
      await tapVisible(tester, find.text('Create an account'));
      expect(opened, 1);
    });

    testWidgets('shows and hides the password', (tester) async {
      await pumpVariant(tester, v, const SignInScreen());
      bool obscured() => tester
          .widget<EditableText>(
            find.descendant(
              of: _field('Password'),
              matching: find.byType(EditableText),
            ),
          )
          .obscureText;
      expect(obscured(), isTrue);
      await tester.tap(find.byTooltip('Show password'));
      await tester.pump();
      expect(obscured(), isFalse);
      expect(find.byTooltip('Hide password'), findsOneWidget);
    });

    test('maps every core failure code', () async {
      final l10n = lookupAccountsLocalizations(const Locale('en'));
      CoreFailure f(String code, {String? field, int? status}) => CoreFailure(
        code: code,
        messageKey: 'error.$code',
        field: field,
        status: status,
      );
      expect(l10n.failure(f('account_disabled')), l10n.errorAccountDisabled);
      expect(
        l10n.failure(f('account_deletion_pending')),
        l10n.errorAccountDeletion,
      );
      expect(l10n.failure(f('session_expired')), l10n.errorSessionExpired);
      expect(l10n.failure(f('rate_limited')), l10n.errorRateLimited);
      expect(l10n.failure(f('not_available')), l10n.errorNotAvailable);
      expect(
        l10n.failure(f('invalid_input', field: 'server_url')),
        'Check the server_url field.',
      );
      expect(
        l10n.failure(f('server', status: 502)),
        'The server answered with an error (502).',
      );
      expect(l10n.failure(f('storage')), 'Something went wrong (storage).');
      expect(l10n.failure(Exception()), 'Something went wrong (internal).');
      expect(l10n.role('admin'), 'Admin');
      expect(l10n.role('member'), 'Member');
      expect(SignInScreen.icon, Icons.login);
    });
  });
}
