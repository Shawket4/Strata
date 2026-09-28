import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// The server address baked in at build time (`STRATA_DEFAULT_SERVER`),
/// as the core reports it on the signed-out session.
const _buildDefault = 'https://strata.shawket.example';

/// A fresh install: no account used before, the core's default server.
SessionState _fresh({String? serverUrl = _buildDefault}) => SessionState(
  kind: SessionKind.signedOut,
  knownAccounts: const [],
  serverUrl: serverUrl,
  deviceName: 'shawket-laptop',
  unsyncedOps: 0,
);

Finder _field(String label) => find.widgetWithText(TextField, label);

/// The editable text inside the labelled field.
EditableText _editable(WidgetTester tester, String label) =>
    tester.widget<EditableText>(
      find.descendant(of: _field(label), matching: find.byType(EditableText)),
    );

void main() {
  group('the build default prefills the server field', () {
    for (final v in variants()) {
      testWidgets('sign in $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(_fresh());
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        final server = _editable(tester, l10n.fieldServer);
        expect(server.controller.text, _buildDefault);
        expect(server.readOnly, isFalse);
        expect(
          tester.widget<TextField>(_field(l10n.fieldServer)).enabled,
          isNot(isFalse),
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('sign up $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(_fresh());
        await pumpVariant(tester, v, const SignUpScreen(), fake: fake);
        final server = _editable(tester, l10n.fieldServer);
        expect(server.controller.text, _buildDefault);
        expect(server.readOnly, isFalse);
        expect(find.text(l10n.signUpOn(server: _buildDefault)), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('stays editable and signs in to the typed address $v', (
        tester,
      ) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(_fresh());
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        await tester.enterText(
          _field(l10n.fieldServer),
          'http://127.0.0.1:8080',
        );
        await tester.enterText(_field(l10n.fieldUsername), 'shawket');
        await tester.enterText(_field(l10n.fieldPassword), 'pw');
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.signInTitle),
        );
        expect(
          fake.calls.last,
          const CoreCall('signIn', {
            'request': SignInRequest(
              serverUrl: 'http://127.0.0.1:8080',
              username: 'shawket',
              password: 'pw',
              deviceName: 'shawket-laptop',
            ),
          }),
        );
      });

      testWidgets('without a build default the field is empty $v', (
        tester,
      ) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()..session.add(_fresh(serverUrl: null));
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        expect(_editable(tester, l10n.fieldServer).controller.text, '');
        expectNoErrors(tester);
      });

      testWidgets('a plain http address is refused, localized $v', (
        tester,
      ) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..session.add(_fresh(serverUrl: 'http://strata.shawket.example'))
          ..signInAnswer.throws(
            const CoreFailure(
              code: 'invalid_input',
              messageKey: 'error.invalid_input',
              field: 'server_url',
              reason: 'insecure_http',
            ),
          );
        await pumpVariant(tester, v, const SignInScreen(), fake: fake);
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.signInTitle),
        );
        expect(find.text(l10n.errorInsecureServer), findsOneWidget);
        expect(find.byType(FormAlert), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  test('the refusal reads the same in English and Arabic', () {
    const refused = CoreFailure(
      code: 'invalid_input',
      messageKey: 'error.invalid_input',
      field: 'server_url',
      reason: 'insecure_http',
    );
    expect(
      lookupAccountsLocalizations(const Locale('en')).failure(refused),
      'Use an https:// address. '
      'Plain http:// works only for this device (localhost).',
    );
    expect(
      lookupAccountsLocalizations(const Locale('ar')).failure(refused),
      'استخدم عنوان يبدأ بـ https://. '
      'عنوان http:// العادي بيشتغل بس مع الجهاز ده (localhost).',
    );
    // Other invalid server addresses keep the generic message.
    expect(
      lookupAccountsLocalizations(const Locale('en')).failure(
        const CoreFailure(
          code: 'invalid_input',
          messageKey: 'error.invalid_input',
          field: 'server_url',
        ),
      ),
      'Check the server_url field.',
    );
  });
}
