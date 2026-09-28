import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/hosts.dart';

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
    for (final v in variants()) {
      testWidgets('offline with outbox and conflict $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.offline));
        _expectSurface(v);
        expect(find.text('Offline · 3 queued'), findsOneWidget);
        expect(find.text(l10n.bodyOffline), findsOneWidget);
        expect(find.text('Today 14:32'), findsOneWidget);
        expect(find.text(l10n.outboxTitle(count: 3)), findsOneWidget);
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(find.text('+2 lines, 1 changed'), findsOneWidget);
        expect(find.text('14:41'), findsOneWidget);
        expect(find.text('contradicts → Discount policy'), findsOneWidget);
        expect(find.text('today 14:41'), findsOneWidget);
        expect(
          find.text('Retrying automatically every 30 s · next at 14:47:30'),
          findsOneWidget,
        );
        expect(find.text(l10n.pauseSync), findsOneWidget);
        expect(find.text(l10n.syncLog), findsOneWidget);
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
        expect(find.text('Syncing 12/40'), findsOneWidget);
        expect(find.text(l10n.bodySyncing), findsOneWidget);
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
        expect(find.text('Synced · 14:32'), findsOneWidget);
        expect(find.text(l10n.outboxEmpty), findsOneWidget);
        expect(find.text(l10n.conflictsTitle(count: 1)), findsNothing);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('pulling $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.pulling));
        _expectSurface(v);
        expect(find.text(l10n.phasePulling), findsOneWidget);
        expect(find.text(l10n.progressItems(done: 12, total: 40)), findsOne);
        expect(find.text(l10n.pulledCount(count: 9)), findsOneWidget);
        expect(
          tester
              .widget<LinearProgressIndicator>(
                find.byType(LinearProgressIndicator),
              )
              .value,
          0.3,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('paused $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _open(tester, v, (f) => f.syncStatus.add(SyncFixtures.paused));
        _expectSurface(v);
        expect(find.text('Paused · 3 queued'), findsOneWidget);
        expect(find.text(l10n.bodyPaused), findsOneWidget);
        expect(find.text(l10n.resumeSync), findsOneWidget);
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
      await _open(tester, variants().first, (_) {});
      expect(find.bySemanticsLabel('Loading sync status'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('forwards Retry now, Dismiss and Review', (tester) async {
      final opened = <String>[];
      final fake = await _open(
        tester,
        variants().first,
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

    testWidgets('pause and resume', (tester) async {
      final fake = await _open(
        tester,
        variants().first,
        (f) => f.syncStatus.add(SyncFixtures.offline),
      );
      await tapVisible(tester, find.text('Pause sync'));
      expect(
        fake.calls.last,
        const CoreCall('setSyncPaused', {'paused': true}),
      );
      fake.syncStatus.add(SyncFixtures.paused);
      await settle(tester);
      await tapVisible(tester, find.text('Resume sync'));
      expect(
        fake.calls.last,
        const CoreCall('setSyncPaused', {'paused': false}),
      );
    });

    testWidgets('the sync log unfolds', (tester) async {
      await _open(
        tester,
        variants().first,
        (f) => f.syncStatus.add(SyncFixtures.offline),
      );
      expect(find.text("Can't reach strata.home.lan"), findsNothing);
      await tapVisible(tester, find.text('Sync log'));
      expect(find.text('14:46:10'), findsOneWidget);
      expect(find.text("Can't reach strata.home.lan"), findsOneWidget);
      expect(find.text('Pulled 4 changes'), findsOneWidget);
    });

    testWidgets('an empty sync log says so', (tester) async {
      await _open(
        tester,
        variants().first,
        (f) => f.syncStatus.add(SyncFixtures.synced),
      );
      await tapVisible(tester, find.text('Sync log'));
      expect(find.text('Nothing logged yet.'), findsOneWidget);
    });

    testWidgets('the close button closes each surface', (tester) async {
      for (final size in [
        StrataTestSizes.compact,
        StrataTestSizes.medium,
        StrataTestSizes.expanded,
      ]) {
        final v = variants().firstWhere((v) => v.size == size);
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
        variants().first,
        (f) => f.syncStatus.add(SyncFixtures.synced),
      );
      expect(find.text(StrataFixtures.serverUrl), findsOneWidget);
    });

    testWidgets('the page renders the panel', (tester) async {
      final fake = FakeCoreApi()..syncStatus.add(SyncFixtures.offline);
      await pumpVariant(
        tester,
        variants().first,
        const Scaffold(body: SyncScreen()),
        fake: fake,
      );
      expect(find.byType(SyncStatusContent), findsOneWidget);
      expect(SyncScreen.icon, Icons.sync);
    });
  });
}
