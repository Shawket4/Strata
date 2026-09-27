import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Sync fixtures from SCREEN_SPEC SyncCompact / SyncMedium / SyncExpanded /
/// ConflictExpanded (offline with 3 queued, syncing, 1 conflict).
abstract final class SyncFixtures {
  static final DateTime _t = DateTime.utc(2026, 9, 27, 11, 41);

  /// Three queued ops, oldest first.
  static final List<OutboxItem> outbox = [
    OutboxItem(
      opId: '01J8ZQ5A',
      kind: 'note.update',
      title: 'Pricing experiments',
      status: OutboxStatus.pending,
      attempts: 2,
      created: _t,
    ),
    OutboxItem(
      opId: '01J8ZQ5B',
      kind: 'note.create',
      title: 'Mona said Nile Freight rates go up 8% in November',
      status: OutboxStatus.pending,
      attempts: 0,
      created: _t.add(const Duration(minutes: 3)),
    ),
    OutboxItem(
      opId: '01J8ZQ5C',
      kind: 'relation.remove',
      title: 'تجارب التسعير — ملخص',
      status: OutboxStatus.inflight,
      attempts: 1,
      created: _t.add(const Duration(minutes: 5)),
    ),
  ];

  /// "1 conflict" on Weekly invoicing proposal.
  static final ConflictItem conflict = ConflictItem(
    opId: 'op-weekly',
    noteId: 'n-weekly',
    title: 'Weekly invoicing proposal',
    created: _t,
  );

  /// Offline · 3 queued, 1 conflict, one rejected op.
  static final SyncStatusView offline = SyncStatusView(
    pill: SyncPill(
      connectivity: Connectivity.offline,
      activity: StrataFixtures.syncActivity,
      pendingOps: 3,
      conflicts: 1,
      duplicates: 0,
      lastSyncAt: DateTime.utc(2026, 9, 27, 11, 32),
    ),
    bootstrapComplete: true,
    outbox: outbox,
    conflicts: [conflict],
    rejections: const [StrataFixtures.rejectionItem],
  );

  /// Syncing: bootstrap page 12 of 40.
  static final SyncStatusView syncing = SyncStatusView(
    pill: SyncPill(
      connectivity: Connectivity.online,
      activity: const SyncActivity(
        phase: SyncPhase.bootstrapping,
        pagesDone: 12,
        pagesTotal: 40,
        ops: 0,
      ),
      pendingOps: 3,
      conflicts: 0,
      duplicates: 0,
      lastSyncAt: DateTime.utc(2026, 9, 27, 11, 32),
    ),
    bootstrapComplete: false,
    outbox: outbox,
    conflicts: const [],
    rejections: const [],
    lastError: 'error.server',
  );

  /// Everything synced.
  static final SyncStatusView synced = SyncStatusView(
    pill: StrataFixtures.syncPill,
    bootstrapComplete: true,
    outbox: const [],
    conflicts: const [],
    rejections: const [],
  );

  /// The ConflictExpanded note.
  static const ConflictDetail detail = ConflictDetail(
    noteId: 'n-weekly',
    title: 'Weekly invoicing proposal',
    base:
        '# Weekly invoicing proposal\n\n- Payment terms: net 30.\n'
        '- Minimum invoice EGP 5,000; smaller weeks roll over.\n',
    local:
        '# Weekly invoicing proposal\n\nOwner: [[Ahmed Samir]] confirmed on '
        'the call.\n- Payment terms: net 14, with a 2-day grace period.\n'
        '- Minimum invoice EGP 5,000; smaller weeks roll over.\n',
    server:
        '# Weekly invoicing proposal\n\n- Payment terms: net 7.\n'
        '- Minimum invoice EGP 7,500; smaller weeks roll over.\n',
    mergedPreview:
        '# Weekly invoicing proposal\n\nOwner: [[Ahmed Samir]] confirmed on '
        'the call.\n<<<<<<< mine\n- Payment terms: net 14, with a 2-day '
        'grace period.\n=======\n- Payment terms: net 7.\n>>>>>>> server\n'
        '- Minimum invoice EGP 7,500; smaller weeks roll over.\n',
    mergeClean: false,
    hunks: [
      ConflictHunkView(
        id: 0,
        location: 'body:6',
        kind: 'body',
        base: '- Payment terms: net 30.\n',
        ours: '- Payment terms: net 14, with a 2-day grace period.\n',
        theirs: '- Payment terms: net 7.\n',
      ),
      ConflictHunkView(
        id: 1,
        location: 'frontmatter:status',
        kind: 'frontmatter_key',
        base: 'draft',
        ours: 'review',
        theirs: 'final',
      ),
    ],
  );

  /// A cleanly merged conflict (no hunks).
  static const ConflictDetail clean = ConflictDetail(
    noteId: 'n-discount',
    title: 'Discount policy',
    base: 'Discounts are capped at 3%.\n',
    local: 'Discounts are capped at 3%.\nLoyalty: 5% after 12 months.\n',
    server: 'Discounts are capped at 3% of annual contract value.\n',
    mergedPreview:
        'Discounts are capped at 3% of annual contract value.\n'
        'Loyalty: 5% after 12 months.\n',
    mergeClean: true,
    hunks: [],
  );
}
