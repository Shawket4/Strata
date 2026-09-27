import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';
import 'helpers/hosts.dart';
import 'helpers/matrix.dart';

FakeCoreApi _fake() => FakeCoreApi()
  ..session.add(StrataFixtures.sessionActive)
  ..settings.add(StrataFixtures.settingsView)
  ..syncStatus.add(AccountFixtures.unsynced);

Future<FakeCoreApi> _open(
  WidgetTester tester,
  Variant v, {
  FakeCoreApi? fake,
  VoidCallback? onDevices,
  VoidCallback? onAdmin,
}) async {
  final api = await pumpVariant(
    tester,
    v,
    AccountHost(onDevices: onDevices, onAdmin: onAdmin),
    fake: fake ?? _fake(),
  );
  await tester.tap(find.text(AccountHost.openLabel));
  await settle(tester);
  return api;
}

Future<void> _signOut(WidgetTester tester, [String label = 'Sign out']) async {
  await tapVisible(tester, find.text(label));
}

void main() {
  group('account sheet matrix', () {
    for (final v in matrix()) {
      testWidgets('who is signed in $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await _open(tester, v);
        expect(
          find.byType(BottomSheet),
          v.sizeClass == SizeClass.compact ? findsOneWidget : findsNothing,
        );
        expect(
          find.byType(Dialog),
          v.sizeClass == SizeClass.compact ? findsNothing : findsOneWidget,
        );
        expect(find.text('Shawket'), findsOneWidget);
        expect(find.text(l10n.atUsername(username: 'shawket')), findsOne);
        expect(find.text(l10n.roleAdmin), findsOneWidget);
        expect(find.text('shawket-laptop'), findsOneWidget);
        expect(find.text(l10n.devices), findsOneWidget);
        expect(find.text(l10n.adminUsers), findsOneWidget);
        expect(find.text(l10n.signOut), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('unsynced warning $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final fake = _fake()
          ..signOutAnswer.returns(StrataFixtures.signOutNeedsConfirmation);
        await _open(tester, v, fake: fake);
        await _signOut(tester, l10n.signOut);
        expect(find.byType(SignOutWarningDialog), findsOneWidget);
        expect(find.text(l10n.unsyncedTitle(count: 3)), findsOneWidget);
        expect(find.text(l10n.unsyncedBody), findsOneWidget);
        expect(find.text('Nile Freight — November rates'), findsOneWidget);
        expect(find.text(l10n.syncNow), findsOneWidget);
        expect(find.text(l10n.signOutAnyway), findsOneWidget);
        expect(find.text(l10n.cancel), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('account sheet intents', () {
    final v = matrix().first;

    testWidgets('a member sees no Admin → Users', (tester) async {
      await _open(
        tester,
        v,
        fake: _fake()
          ..settings.add(
            const SettingsView(
              account: StrataFixtures.accountSummary,
              reminders: StrataFixtures.remindersSetting,
              devices: Availability.available,
              ai: Availability.notYetAvailable,
              export_: Availability.available,
              integrity: Availability.notYetAvailable,
              admin: Availability.notAllowed,
            ),
          ),
      );
      expect(find.text('Admin → Users'), findsNothing);
    });

    testWidgets('Devices and Admin → Users close the sheet first', (
      tester,
    ) async {
      final opened = <String>[];
      await _open(
        tester,
        v,
        onDevices: () => opened.add('devices'),
        onAdmin: () => opened.add('admin'),
      );
      await tester.tap(find.text('Devices'));
      await settle(tester);
      expect(find.byType(AccountSheet), findsNothing);
      await tester.tap(find.text(AccountHost.openLabel));
      await settle(tester);
      await tester.tap(find.text('Admin → Users'));
      await settle(tester);
      expect(opened, ['devices', 'admin']);
    });

    testWidgets('nothing unsynced: signs out at once', (tester) async {
      final fake = await _open(tester, v);
      await _signOut(tester);
      expect(fake.calls.last, const CoreCall('signOut', {'force': false}));
      expect(find.byType(SignOutWarningDialog), findsNothing);
    });

    for (final (String choice, CoreCall? expected) in [
      ('Sync now', const CoreCall('syncNow')),
      ('Sign out anyway', const CoreCall('signOut', {'force': true})),
      ('Cancel', null),
    ]) {
      testWidgets('warning: $choice', (tester) async {
        final fake = _fake()
          ..signOutAnswer.returns(StrataFixtures.signOutNeedsConfirmation);
        await _open(tester, v, fake: fake);
        await _signOut(tester);
        await tester.tap(find.text(choice));
        await settle(tester);
        expect(find.byType(SignOutWarningDialog), findsNothing);
        expect(
          fake.calls.where(
            (c) => c.method == 'signOut' || c.method == 'syncNow',
          ),
          [
            const CoreCall('signOut', {'force': false}),
            ?expected,
          ],
        );
      });
    }

    testWidgets('a failed sign-out shows the core error', (tester) async {
      final fake = _fake()
        ..signOutAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await _open(tester, v, fake: fake);
      await _signOut(tester);
      expect(
        find.text(
          "Can't reach the server. Check the address and your connection.",
        ),
        findsOneWidget,
      );
    });
  });
}
