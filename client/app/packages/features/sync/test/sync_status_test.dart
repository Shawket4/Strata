import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/hosts.dart';
import 'helpers/matrix.dart';

Future<FakeCoreApi> _open(
  WidgetTester tester,
  Variant v,
  void Function(FakeCoreApi fake) seed, {
  List<String>? opened,
  void Function(FakeCoreApi fake)? afterOpen,
}) async {
  final fake = FakeCoreApi()..session.add(StrataFixtures.sessionActive);
  seed(fake);
  await pumpVariant(tester, v, SyncHost(opened: opened), fake: fake);
  await tester.tap(find.text(SyncHost.openLabel));
  await settle(tester);
  afterOpen?.call(fake);
  await settle(tester);
  return fake;
}

void _expectSurface(Variant v) {
  switch (v.sizeClass) {
    case SizeClass.compact:
      expect(find.byType(BottomSheet), findsOneWidget);
      expect(find.byType(SyncDrawer), findsNothing);
      expect(find.byType(SyncPopover), findsNothing);
    case SizeClass.medium:
      expect(find.byType(SyncDrawer), findsOneWidget);
      expect(find.byType(BottomSheet), findsNothing);
    case SizeClass.expanded:
      expect(find.byType(SyncPopover), findsOneWidget);
      expect(find.byType(SyncDrawer), findsNothing);
  }
}

void main() {
  group('sync status matrix', () {
    for (final v in matrix()) {
      testWidgets('offline with outbox and conflict $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.offline));
        _expectSurface(v);
        expect(find.text(l10n.pillOffline(count: 3)), findsOneWidget);
        expect(find.text(l10n.bodyOffline), findsOneWidget);
        expect(find.text(l10n.outboxTitle(count: 3)), findsOneWidget);
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(find.text('تجارب التسعير — ملخص'), findsOneWidget);
        expect(find.text(l10n.conflictsTitle(count: 1)), findsOneWidget);
        expect(find.text('Weekly invoicing proposal'), findsOneWidget);
        expect(find.text(l10n.rejectionsTitle), findsOneWidget);
        expect(find.text(l10n.retryNow), findsOneWidget);
        expect(
          find.bySemanticsLabel(
            l10n.reviewSemantics(title: 'Weekly invoicing proposal'),
          ),
          findsOneWidget,
        );
        expect(
          directionOf(tester, find.byType(SyncStatusContent)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('syncing $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.syncing));
        _expectSurface(v);
        expect(find.text(l10n.phaseBootstrapping), findsOneWidget);
        expect(find.text(l10n.progressPages(done: 12, total: 40)), findsOne);
        expect(find.byType(LinearProgressIndicator), findsOneWidget);
        expect(find.text(l10n.snapshotPending), findsOneWidget);
        expect(find.text('error.server'), findsOneWidget);
        expect(find.text(l10n.syncNow), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('synced, empty outbox $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.synced));
        _expectSurface(v);
        expect(find.text(l10n.pillOnline(count: 0)), findsOneWidget);
        expect(find.text(l10n.outboxEmpty), findsOneWidget);
        expect(find.text(l10n.conflictsTitle(count: 1)), findsNothing);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('error $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(
          tester,
          v,
          (_) {},
          afterOpen: (f) => f.syncStatus.addError(StrataFixtures.coreFailure),
        );
        _expectSurface(v);
        expect(find.text(l10n.loadFailed), findsOneWidget);
        expect(
          find.text(l10n.errorGeneric(code: 'pending_changes')),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('sync status', () {
    testWidgets('loading until the core emits', (tester) async {
      await _open(tester, matrix().first, (_) {});
      expect(find.bySemanticsLabel('Loading sync status'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('forwards Retry now, Dismiss and Review', (tester) async {
      final opened = <String>[];
      final fake = await _open(
        tester,
        matrix().first,
        (f) => f.syncStatus.add(SyncFixtures.offline),
        opened: opened,
      );
      expect(fake.calls, contains(const CoreCall('watchSyncStatus')));
      await tester.ensureVisible(find.text('Retry now'));
      await tester.tap(find.text('Retry now'));
      await tester.pump();
      expect(fake.calls.last, const CoreCall('syncNow'));

      await tester.ensureVisible(find.text('Dismiss'));
      await tester.tap(find.text('Dismiss'));
      await tester.pump();
      expect(
        fake.calls.last,
        CoreCall('dismissRejection', {
          'opId': StrataFixtures.rejectionItem.opId,
        }),
      );

      await tester.ensureVisible(find.text('Review'));
      await tester.tap(find.text('Review'));
      await settle(tester);
      expect(opened, ['op-weekly']);
      expect(find.byType(BottomSheet), findsNothing);
    });

    testWidgets('the close button closes each surface', (tester) async {
      for (final size in [
        StrataTestSizes.compact,
        StrataTestSizes.medium,
        StrataTestSizes.expanded,
      ]) {
        final v = matrix().firstWhere((v) => v.size == size);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.synced));
        await tester.tap(find.byTooltip('Close'));
        await settle(tester);
        expect(find.byType(SyncStatusContent), findsNothing);
      }
    });

    testWidgets('shows the server URL of the signed-in account', (
      tester,
    ) async {
      await _open(
        tester,
        matrix().first,
        (f) => f.syncStatus.add(SyncFixtures.synced),
      );
      expect(find.text(StrataFixtures.serverUrl), findsOneWidget);
    });

    testWidgets('the page renders the panel', (tester) async {
      final fake = FakeCoreApi()..syncStatus.add(SyncFixtures.offline);
      await pumpVariant(
        tester,
        matrix().first,
        const Scaffold(body: SyncScreen()),
        fake: fake,
      );
      expect(find.byType(SyncStatusContent), findsOneWidget);
      expect(SyncScreen.icon, Icons.sync);
    });
  });
}
