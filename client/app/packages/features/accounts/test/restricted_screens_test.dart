import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart' show lookupSyncLocalizations;
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';

void _expectFrame(Variant v) {
  final wide = v.sizeClass != SizeClass.compact;
  // Wide windows show a centred alert card over the dimmed bands.
  expect(find.byType(StrataBands), wide ? findsOneWidget : findsNothing);
}

void main() {
  group('restricted screens matrix', () {
    for (final v in variants()) {
      testWidgets('account disabled $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          const AccountDisabledScreen(),
          fake: FakeCoreApi()
            ..session.add(AccountFixtures.disabled)
            ..syncStatus.add(AccountFixtures.unsynced),
        );
        _expectFrame(v);
        expect(find.text(l10n.disabledTitle), findsOneWidget);
        expect(find.text(l10n.disabledBody(username: 'mona.h')), findsOne);
        expect(find.text(l10n.unsyncedCount(count: 2)), findsOneWidget);
        expect(find.text('Nile Freight — November rates'), findsOneWidget);
        expect(
          find.text('الفواتير الشهرية لازم تتراجع قبل الخميس'),
          findsOneWidget,
        );
        expect(find.text(l10n.exportFirst), findsOneWidget);
        expect(
          find.text(
            l10n.unsyncedItem(
              kind: lookupSyncLocalizations(v.locale).kindNoteUpdate,
              time: 'today 11:05',
            ),
          ),
          findsOneWidget,
        );
        expect(find.text(l10n.removeAndSignOut), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('deletion pending $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          const DeletionPendingScreen(),
          fake: FakeCoreApi()
            ..session.add(AccountFixtures.deletionPending)
            ..syncStatus.add(AccountFixtures.unsynced),
        );
        _expectFrame(v);
        expect(find.text(l10n.deletionTitle), findsOneWidget);
        expect(find.text(l10n.daysLeft(count: 14)), findsOneWidget);
        expect(
          find.text(
            l10n.deletionBody(username: 'karim', date: 'Sun 11 Oct 2026'),
          ),
          findsOneWidget,
        );
        expect(find.text(l10n.downloadExport), findsOneWidget);
        expect(find.text('18.4 MB · 412 notes'), findsOneWidget);
        expect(find.text(l10n.unsyncedNotInExport(count: 2)), findsOneWidget);
        expect(find.text(l10n.saveAsFile), findsOneWidget);
        expect(find.text(l10n.deleteNow), findsOneWidget);
        expect(find.text(l10n.readOnlyNote), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('password change required $v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          const PasswordChangeRequiredScreen(),
          fake: FakeCoreApi()..session.add(AccountFixtures.passwordChange),
        );
        _expectFrame(v);
        expect(find.text(l10n.passwordChangeTitle), findsOneWidget);
        expect(find.text(l10n.changePassword), findsOneWidget);
        expect(find.text(l10n.fieldTemporaryPassword), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('restricted screens intents', () {
    final v = variants().first;

    testWidgets('Remove and sign out acknowledges', (tester) async {
      final fake = await pumpVariant(
        tester,
        v,
        const AccountDisabledScreen(),
        fake: FakeCoreApi()..session.add(AccountFixtures.disabled),
      );
      await tapVisible(tester, find.text('Remove and sign out'));
      expect(fake.calls.last, const CoreCall('acknowledgeAccountDisabled'));
    });

    testWidgets('a failed acknowledge shows the core error', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.disabled)
        ..acknowledgeAccountDisabledAnswer.throws(
          const CoreFailure(code: 'storage', messageKey: 'error.storage'),
        );
      await pumpVariant(tester, v, const AccountDisabledScreen(), fake: fake);
      await tapVisible(tester, find.text('Remove and sign out'));
      expect(find.text('Something went wrong (storage).'), findsOneWidget);
    });

    testWidgets('nothing unsynced: no export offer', (tester) async {
      await pumpVariant(
        tester,
        v,
        const AccountDisabledScreen(),
        fake: FakeCoreApi()..session.add(StrataFixtures.sessionActive),
      );
      expect(find.text('Export them first'), findsNothing);
    });

    testWidgets('export the unsynced changes to a typed path', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.disabled)
        ..exportUnsyncedAnswer.returns(2);
      await pumpVariant(tester, v, const AccountDisabledScreen(), fake: fake);
      await tapVisible(tester, find.text('Export them first'));
      await tester.enterText(find.byType(TextField), '/home/mona/unsynced.md');
      await tester.tap(find.text('Save'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('exportUnsynced', {'path': '/home/mona/unsynced.md'}),
      );
      expect(find.text('2 changes saved'), findsOneWidget);
    });

    testWidgets('a cancelled path asks nothing', (tester) async {
      final fake = FakeCoreApi()..session.add(AccountFixtures.deletionPending);
      await pumpVariant(tester, v, const DeletionPendingScreen(), fake: fake);
      await tapVisible(tester, find.text('Download export (.zip)'));
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(fake.calls.where((c) => c.method == 'downloadExport'), isEmpty);
    });

    testWidgets('download the export and save the unsynced changes', (
      tester,
    ) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.deletionPending)
        ..exportUnsyncedAnswer.returns(2);
      await pumpVariant(tester, v, const DeletionPendingScreen(), fake: fake);
      await tapVisible(tester, find.text('Download export (.zip)'));
      await tester.enterText(find.byType(TextField), '/tmp/karim.zip');
      await tester.tap(find.text('Save'));
      await settle(tester);
      expect(
        find.text('Saved ${StrataFixtures.exportSummary.label}'),
        findsOneWidget,
      );
      await tapVisible(tester, find.text('Save them as a file'));
      await tester.enterText(find.byType(TextField), '/tmp/karim.md');
      await tester.tap(find.text('Save'));
      await settle(tester);
      expect(fake.calls.where((c) => !c.method.startsWith('watch')), [
        const CoreCall('downloadExport', {'path': '/tmp/karim.zip'}),
        const CoreCall('exportUnsynced', {'path': '/tmp/karim.md'}),
      ]);
    });

    testWidgets('Delete now confirms; unsynced changes force it', (
      tester,
    ) async {
      final fake = FakeCoreApi()..session.add(AccountFixtures.deletionPending);
      await pumpVariant(tester, v, const DeletionPendingScreen(), fake: fake);
      await tapVisible(tester, find.text('Delete now'));
      expect(find.text('Delete @karim now?'), findsOneWidget);
      expect(
        find.text("2 changes on this device weren't synced"),
        findsNWidgets(2),
      );
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(fake.calls.where((c) => c.method == 'deleteAccountNow'), isEmpty);
      await tapVisible(tester, find.text('Delete now'));
      await tester.tap(find.text('Delete anyway'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('deleteAccountNow', {'force': true}),
      );
    });

    testWidgets('Delete now with everything synced', (tester) async {
      final fake = FakeCoreApi()..session.add(AccountFixtures.deletionSynced);
      await pumpVariant(tester, v, const DeletionPendingScreen(), fake: fake);
      await tapVisible(tester, find.text('Delete now'));
      await tester.tap(find.text('Delete now').last);
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('deleteAccountNow', {'force': false}),
      );
    });

    testWidgets('choose a new password', (tester) async {
      final fake = FakeCoreApi()..session.add(AccountFixtures.passwordChange);
      await pumpVariant(
        tester,
        v,
        const PasswordChangeRequiredScreen(),
        fake: fake,
      );
      Finder field(String label) => find.widgetWithText(TextField, label);
      await tester.enterText(field('Temporary password'), 'tide-4821-sand');
      await tester.enterText(field('New password'), 'nile-freight-2026');
      await tester.enterText(field('Confirm password'), 'nile-freight');
      await tester.pump();
      expect(find.text("Passwords don't match"), findsOneWidget);
      await tester.enterText(field('Confirm password'), 'nile-freight-2026');
      await tester.pump();
      await tapVisible(
        tester,
        find.widgetWithText(FilledButton, 'Change password'),
      );
      expect(
        fake.calls.last,
        const CoreCall('changePassword', {
          'current': 'tide-4821-sand',
          'new_': 'nile-freight-2026',
        }),
      );
    });

    testWidgets('a refused new password shows the core error', (tester) async {
      final fake = FakeCoreApi()
        ..session.add(AccountFixtures.passwordChange)
        ..changePasswordAnswer.throws(AccountFixtures.invalidCredentials);
      await pumpVariant(
        tester,
        v,
        const PasswordChangeRequiredScreen(),
        fake: fake,
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'New password'),
        'x',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Confirm password'),
        'x',
      );
      await tester.pump();
      await tapVisible(
        tester,
        find.widgetWithText(FilledButton, 'Change password'),
      );
      expect(find.text('Username or password is incorrect'), findsOneWidget);
    });

    testWidgets('deletion pending can sign out', (tester) async {
      final fake = await pumpVariant(
        tester,
        v,
        const DeletionPendingScreen(),
        fake: FakeCoreApi()..session.add(StrataFixtures.sessionDeletionPending),
      );
      await tapVisible(tester, find.text('Sign out'));
      expect(fake.calls.last, const CoreCall('signOut', {'force': false}));
    });

    testWidgets('password change can sign out', (tester) async {
      final fake = await pumpVariant(
        tester,
        v,
        const PasswordChangeRequiredScreen(),
        fake: FakeCoreApi()..session.add(AccountFixtures.passwordChange),
      );
      await tapVisible(tester, find.text('Sign out'));
      expect(fake.calls.last, const CoreCall('signOut', {'force': false}));
    });
  });
}
