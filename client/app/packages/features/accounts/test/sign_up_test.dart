import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

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
    for (final v in variants()) {
      testWidgets('form with the approval notice $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          SignUpScreen(onBack: () {}),
          fake: FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut),
        );
        expect(find.text(l10n.createAccount), findsOneWidget);
        expect(find.textContaining('://'), findsNothing);
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

      testWidgets('strength meter $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..passwordStrengthAnswer.returns(
            const PasswordStrength(
              level: PasswordLevel.weak,
              length: 11,
              minLength: 10,
            ),
          );
        await pumpVariant(tester, v, const SignUpScreen(), fake: fake);
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldPassword),
          'nilefreight',
        );
        await settle(tester);
        expect(find.text(l10n.strengthWeak), findsOneWidget);
        expect(
          find.text(l10n.strengthDetail(length: 11, min: 10)),
          findsOneWidget,
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
    final v = variants().first;

    testWidgets('requests the account', (tester) async {
      var requested = 0;
      final fake = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      await pumpVariant(
        tester,
        v,
        SignUpScreen(onRequested: () => requested++),
        fake: fake,
      );
      await _fill(tester);
      expect(find.text('Passwords match'), findsOneWidget);
      await _request(tester);
      expect(
        fake.calls.last,
        const CoreCall('signUp', {'request': StrataFixtures.signUpRequest}),
      );
      expect(requested, 1);
    });

    testWidgets('the strength meter shows the core verdict', (tester) async {
      final fake = FakeCoreApi()
        ..passwordStrengthAnswer.returns(
          const PasswordStrength(
            level: PasswordLevel.strong,
            length: 19,
            minLength: 10,
          ),
        );
      await pumpVariant(tester, v, const SignUpScreen(), fake: fake);
      expect(find.byType(PasswordStrengthMeter), findsNothing);
      await tester.enterText(_field('Password'), 'nile-freight-2026-ok');
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('passwordStrength', {
          'password': 'nile-freight-2026-ok',
        }),
      );
      expect(find.text('Strong'), findsOneWidget);
      expect(find.text('· 19 characters · at least 10'), findsOneWidget);
      expect(
        find.bySemanticsLabel(RegExp('^Strong\n· 19 characters')),
        findsOneWidget,
      );
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
      await pumpVariant(tester, v, const SignUpScreen(), fake: fake);
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
