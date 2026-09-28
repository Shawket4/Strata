import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/matrix.dart';

SyncPill _pill({
  Connectivity connectivity = Connectivity.online,
  int pending = 0,
  int conflicts = 0,
  SyncPhase phase = SyncPhase.idle,
}) => SyncPill(
  connectivity: connectivity,
  activity: SyncActivity(
    phase: phase,
    pagesDone: 0,
    ops: 0,
    opsDone: 0,
    opsTotal: 0,
    pulled: 0,
  ),
  pendingOps: pending,
  conflicts: conflicts,
  duplicates: 0,
  display: SyncPillKind.synced,
  progressDone: 0,
  progressTotal: 0,
  label: '',
);

Future<void> _pump(
  WidgetTester tester,
  SyncPill pill, {
  bool dense = false,
  VoidCallback? onPressed,
  Variant? variant,
}) => pumpVariant(
  tester,
  variant ?? matrix().first,
  Scaffold(
    body: Center(
      child: SyncStatusPill(pill: pill, dense: dense, onPressed: onPressed),
    ),
  ),
);

void main() {
  const cases = <(String, Connectivity, int, StatusTone, IconData)>[
    (
      'Synced',
      Connectivity.online,
      0,
      StatusTone.success,
      Icons.cloud_done_outlined,
    ),
    (
      'Online · 2 queued',
      Connectivity.online,
      2,
      StatusTone.success,
      Icons.cloud_done_outlined,
    ),
    (
      'Offline · 3 queued',
      Connectivity.offline,
      3,
      StatusTone.warning,
      Icons.cloud_off_outlined,
    ),
    (
      'Offline',
      Connectivity.offline,
      0,
      StatusTone.warning,
      Icons.cloud_off_outlined,
    ),
    (
      'Not synced yet',
      Connectivity.unknown,
      0,
      StatusTone.neutral,
      Icons.cloud_queue_outlined,
    ),
  ];
  for (final (label, connectivity, pending, tone, icon) in cases) {
    testWidgets('renders $label 1:1 from the view-model', (tester) async {
      await _pump(tester, _pill(connectivity: connectivity, pending: pending));
      final status = tester.widget<StatusPill>(find.byType(StatusPill));
      expect(status.label, label);
      expect(status.tone, tone);
      expect(status.icon, icon);
      expect(find.bySemanticsLabel('Sync status: $label'), findsOneWidget);
    });
  }

  testWidgets('adds the conflict badge in the danger tone', (tester) async {
    await _pump(tester, _pill(pending: 1, conflicts: 1));
    final status = tester.widget<StatusPill>(find.byType(StatusPill));
    expect(status.label, 'Online · 1 queued · 1 conflict');
    expect(status.tone, StatusTone.danger);
  });

  testWidgets('shows a spinner while the engine runs', (tester) async {
    await _pump(tester, _pill(phase: SyncPhase.pushing));
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    await _pump(tester, _pill());
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });

  testWidgets('dense form keeps the label in tooltip and semantics', (
    tester,
  ) async {
    var taps = 0;
    await _pump(
      tester,
      _pill(connectivity: Connectivity.offline, pending: 3),
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

  testWidgets('Arabic copy and RTL', (tester) async {
    final ar = matrix().firstWhere((v) => v.rtl);
    await _pump(
      tester,
      _pill(connectivity: Connectivity.offline, pending: 3),
      variant: ar,
    );
    final l10n = lookupSyncLocalizations(ar.locale);
    expect(find.text(l10n.pillOffline(count: 3)), findsOneWidget);
    expect(directionOf(tester, find.byType(StatusPill)), TextDirection.rtl);
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
