import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

void _expectFrame(Variant v) {
  final wide = v.sizeClass != SizeClass.compact;
  // Wide windows show a centred alert card over the dimmed bands.
  expect(find.byType(StrataBands), wide ? findsOneWidget : findsNothing);
}

void main() {
  group('restricted screens matrix', () {
    for (final v in matrix()) {
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
            ..session.add(StrataFixtures.sessionDeletionPending)
            ..syncStatus.add(AccountFixtures.unsynced),
        );
        _expectFrame(v);
        expect(find.text(l10n.deletionTitle), findsOneWidget);
        expect(find.text(l10n.daysLeft(count: 12)), findsOneWidget);
        expect(find.textContaining('@shawket'), findsOneWidget);
        expect(find.text(l10n.downloadExport), findsOneWidget);
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
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('restricted screens intents', () {
    final v = matrix().first;

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

    testWidgets('actions the core lacks are disabled', (tester) async {
      await pumpVariant(
        tester,
        v,
        const DeletionPendingScreen(),
        fake: FakeCoreApi()..session.add(StrataFixtures.sessionDeletionPending),
      );
      for (final label in ['Download export (.zip)', 'Delete now']) {
        final button = find.ancestor(
          of: find.text(label),
          matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
        );
        expect(
          tester.widget<ButtonStyleButton>(button).onPressed,
          isNull,
          reason: label,
        );
      }
      expect(find.byTooltip('Not available yet'), findsNWidgets(3));
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
