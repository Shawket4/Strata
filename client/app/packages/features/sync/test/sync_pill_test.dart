import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

SyncPill _pill(SyncPillKind display, String label) => SyncPill(
  connectivity: Connectivity.online,
  activity: StrataFixtures.syncActivity,
  pendingOps: 0,
  conflicts: 0,
  duplicates: 0,
  display: display,
  progressDone: 0,
  progressTotal: 0,
  label: label,
);

Future<void> _pump(
  WidgetTester tester,
  SyncPill pill, {
  bool dense = false,
  VoidCallback? onPressed,
  Variant? variant,
}) => pumpVariant(
  tester,
  variant ?? variants().first,
  Center(
    child: SyncStatusPill(pill: pill, dense: dense, onPressed: onPressed),
  ),
  scaffold: true,
);

void main() {
  const cases = <(SyncPillKind, String, StatusTone, IconData)>[
    (
      SyncPillKind.synced,
      'Synced · 14:32',
      StatusTone.success,
      Icons.cloud_done_outlined,
    ),
    (
      SyncPillKind.offline,
      'Offline · 3 queued',
      StatusTone.warning,
      Icons.cloud_off_outlined,
    ),
    (SyncPillKind.syncing, 'Syncing 12/40', StatusTone.info, Icons.sync),
    (
      SyncPillKind.conflict,
      '1 conflict',
      StatusTone.danger,
      Icons.report_problem_outlined,
    ),
    (
      SyncPillKind.duplicates,
      '1 already exists',
      StatusTone.danger,
      Icons.content_copy_outlined,
    ),
    (
      SyncPillKind.paused,
      'Paused · 3 queued',
      StatusTone.warning,
      Icons.pause_circle_outline,
    ),
    (
      SyncPillKind.error,
      'Sync failed',
      StatusTone.danger,
      Icons.sync_problem_outlined,
    ),
  ];
  for (final (display, label, tone, icon) in cases) {
    testWidgets('renders $display 1:1 from the view-model', (tester) async {
      await _pump(tester, _pill(display, label));
      final status = tester.widget<StatusPill>(find.byType(StatusPill));
      expect(status.label, label);
      expect(status.tone, tone);
      expect(status.icon, icon);
      expect(find.bySemanticsLabel('Sync status: $label'), findsOneWidget);
      expect(
        find.byType(CircularProgressIndicator),
        display == SyncPillKind.syncing ? findsOneWidget : findsNothing,
      );
    });
  }

  for (final v in variants()) {
    testWidgets('matrix $v', (tester) async {
      await _pump(
        tester,
        StrataFixtures.syncPillSyncing,
        variant: v,
        dense: v.sizeClass == SizeClass.medium,
        onPressed: () {},
      );
      if (v.sizeClass == SizeClass.medium) {
        expect(find.byTooltip('Syncing 12/40'), findsOneWidget);
      } else {
        expect(find.text('Syncing 12/40'), findsOneWidget);
      }
      final l10n = lookupSyncLocalizations(v.locale);
      expect(
        find.bySemanticsLabel(
          l10n.pillSemanticsWithHint(status: 'Syncing 12/40'),
        ),
        findsOneWidget,
      );
      expect(
        directionOf(tester, find.byType(SyncStatusPill)),
        v.rtl ? TextDirection.rtl : TextDirection.ltr,
      );
      expectNoErrors(tester);
      await expectAccessible(tester, contrast: v.textScale == 1);
    });
  }

  testWidgets('dense form keeps the label in tooltip and semantics', (
    tester,
  ) async {
    var taps = 0;
    await _pump(
      tester,
      StrataFixtures.syncPillOffline,
      dense: true,
      onPressed: () => taps++,
    );
    expect(find.byType(StatusPill), findsNothing);
    expect(find.byTooltip('Offline · 3 queued'), findsOneWidget);
    expect(
      find.bySemanticsLabel(
        'Sync status: Offline · 3 queued. Open sync status',
      ),
      findsOneWidget,
    );
    await tester.tap(find.byType(SyncStatusPill));
    expect(taps, 1);
    await expectAccessible(tester);
  });

  test('labels map every enum value', () async {
    final l10n = lookupSyncLocalizations(const Locale('en'));
    expect(
      [for (final p in SyncPhase.values) SyncLabels.phase(l10n, p)],
      [
        'Idle',
        'Downloading your vault',
        'Sending changes',
        'Pulling changes',
        'Waiting to retry',
      ],
    );
    expect(
      [for (final s in OutboxStatus.values) SyncLabels.status(l10n, s)],
      ['Queued', 'Sending', 'Conflict', 'Already exists?'],
    );
    expect({
      for (final d in SyncPillKind.values) SyncLabels.body(l10n, d),
    }, hasLength(SyncPillKind.values.length));
    expect(SyncLabels.kind(l10n, 'task.complete'), 'Complete task');
    expect(SyncLabels.kind(l10n, 'document.custody'), 'Document change');
    expect(SyncLabels.kind(l10n, 'future.kind'), 'Change');
    expect(
      SyncLabels.failure(
        l10n,
        const CoreFailure(code: 'offline', messageKey: 'error.offline'),
      ),
      "You're offline.",
    );
    expect(
      SyncLabels.failure(l10n, StateError('x')),
      'Something went wrong (internal).',
    );
  });
}
