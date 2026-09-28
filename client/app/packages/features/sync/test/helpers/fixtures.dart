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
      detail: '+2 lines, 1 changed',
      detailDir: TextDir.ltr,
      createdLabel: '14:41',
    ),
    OutboxItem(
      opId: '01J8ZQ5B',
      kind: 'note.create',
      title: 'Mona said Nile Freight rates go up 8% in November',
      status: OutboxStatus.pending,
      attempts: 0,
      created: _t.add(const Duration(minutes: 3)),
      detail: '“Mona said Nile Freight rates go up 8% in November”',
      detailDir: TextDir.ltr,
      createdLabel: '14:44',
    ),
    OutboxItem(
      opId: '01J8ZQ5C',
      kind: 'relation.remove',
      title: 'تجارب التسعير — ملخص',
      status: OutboxStatus.inflight,
      attempts: 1,
      created: _t.add(const Duration(minutes: 5)),
      detail: 'contradicts · Discount policy',
      detailDir: TextDir.ltr,
      createdLabel: '14:46',
    ),
  ];

  /// "1 conflict" on Weekly invoicing proposal.
  static final ConflictItem conflict = ConflictItem(
    opId: 'op-weekly',
    noteId: 'n-weekly',
    title: 'Weekly invoicing proposal',
    created: _t,
    createdLabel: 'today 14:41',
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
      lastSyncLabel: 'Today 14:32',
      display: SyncPillKind.offline,
      progressDone: 0,
      progressTotal: 0,
      label: 'Offline · 3 queued',
    ),
    bootstrapComplete: true,
    outbox: outbox,
    conflicts: [conflict],
    rejections: const [StrataFixtures.rejectionItem],
    paused: false,
    retryIntervalSecs: 30,
    nextRetryLabel: '14:47:30',
    retryLabel: 'Retrying automatically every 30 s · next at 14:47:30',
    log: [
      SyncLogItem(
        at: _t,
        atLabel: '14:46:10',
        kind: 'push_failed',
        detail: "Can't reach strata.home.lan",
      ),
      SyncLogItem(
        at: _t,
        atLabel: '14:32:05',
        kind: 'synced',
        detail: 'Pulled 4 changes',
      ),
    ],
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
        opsDone: 0,
        opsTotal: 0,
        pulled: 0,
      ),
      pendingOps: 3,
      conflicts: 0,
      duplicates: 0,
      lastSyncAt: DateTime.utc(2026, 9, 27, 11, 32),
      lastSyncLabel: 'Today 14:32',
      display: SyncPillKind.syncing,
      progressDone: 12,
      progressTotal: 40,
      label: 'Syncing 12/40',
    ),
    bootstrapComplete: false,
    outbox: outbox,
    conflicts: const [],
    rejections: const [],
    lastError: 'error.server',
    paused: false,
    log: [],
  );

  /// Pulling: 12 of 40 items, 9 pulled; sync paused afterwards.
  static final SyncStatusView pulling = SyncStatusView(
    pill: const SyncPill(
      connectivity: Connectivity.online,
      activity: SyncActivity(
        phase: SyncPhase.pulling,
        pagesDone: 0,
        ops: 0,
        opsDone: 12,
        opsTotal: 40,
        pulled: 9,
      ),
      pendingOps: 3,
      conflicts: 0,
      duplicates: 0,
      lastSyncLabel: 'Today 14:32',
      display: SyncPillKind.syncing,
      progressDone: 12,
      progressTotal: 40,
      label: 'Syncing 12/40',
    ),
    bootstrapComplete: true,
    outbox: outbox,
    conflicts: const [],
    rejections: const [],
    paused: false,
    log: const [],
  );

  /// Paused by the user.
  static final SyncStatusView paused = SyncStatusView(
    pill: const SyncPill(
      connectivity: Connectivity.online,
      activity: StrataFixtures.syncActivity,
      pendingOps: 3,
      conflicts: 0,
      duplicates: 0,
      lastSyncLabel: 'Today 14:32',
      display: SyncPillKind.paused,
      progressDone: 0,
      progressTotal: 0,
      label: 'Paused · 3 queued',
    ),
    bootstrapComplete: true,
    outbox: outbox,
    conflicts: const [],
    rejections: const [],
    paused: true,
    log: const [],
  );

  /// Everything synced.
  static final SyncStatusView synced = SyncStatusView(
    pill: StrataFixtures.syncPill,
    bootstrapComplete: true,
    outbox: const [],
    conflicts: const [],
    rejections: const [],
    paused: false,
    log: [],
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
        locationLabel: 'Line 6',
        allowedChoices: [
          HunkChoiceKind.ours,
          HunkChoiceKind.theirs,
          HunkChoiceKind.base,
          HunkChoiceKind.oursThenTheirs,
          HunkChoiceKind.theirsThenOurs,
          HunkChoiceKind.text,
        ],
      ),
      ConflictHunkView(
        id: 1,
        location: 'frontmatter:status',
        kind: 'frontmatter_key',
        base: 'draft',
        ours: 'review',
        theirs: 'final',
        locationLabel: 'status',
        allowedChoices: [
          HunkChoiceKind.ours,
          HunkChoiceKind.theirs,
          HunkChoiceKind.base,
        ],
      ),
    ],
    path: 'notes/clients/acme/weekly-invoicing-proposal.md',
    localOriginLabel: 'MacBook Pro · today 14:41 · edited offline',
    serverOriginLabel: 'Pixel 8 · today 14:38 · Shawket',
    baseLines: [],
    localLines: [
      AnnotatedLine(
        line: 1,
        text: '# Weekly invoicing proposal',
        change: LineChange.same,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 2,
        text: 'Owner: [[Ahmed Samir]] confirmed on the call.',
        change: LineChange.added,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 3,
        text: '- Payment terms: net 14, with a 2-day grace period.',
        change: LineChange.changedBoth,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 4,
        text: 'ملاحظة: أحمد وافق على الشروط',
        change: LineChange.added,
        dir: TextDir.rtl,
      ),
    ],
    serverLines: [
      AnnotatedLine(
        line: 1,
        text: '# Weekly invoicing proposal',
        change: LineChange.same,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 2,
        text: '- Payment terms: net 7.',
        change: LineChange.changedBoth,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 3,
        text: '- Minimum invoice EGP 5,000; smaller weeks roll over.',
        change: LineChange.removed,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 3,
        text: '- Minimum invoice EGP 7,500; smaller weeks roll over.',
        change: LineChange.added,
        dir: TextDir.ltr,
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
    path: 'notes/sales/discount-policy.md',
    localOriginLabel: 'MacBook Pro · today 14:41',
    serverOriginLabel: 'Pixel 8 · today 14:38',
    baseLines: [],
    localLines: [
      AnnotatedLine(
        line: 1,
        text: 'Discounts are capped at 3%.',
        change: LineChange.same,
        dir: TextDir.ltr,
      ),
      AnnotatedLine(
        line: 2,
        text: 'Loyalty: 5% after 12 months.',
        change: LineChange.added,
        dir: TextDir.ltr,
      ),
    ],
    serverLines: [
      AnnotatedLine(
        line: 1,
        text: 'Discounts are capped at 3% of annual contract value.',
        change: LineChange.changed,
        dir: TextDir.ltr,
      ),
    ],
    conflictCopyPath: 'notes/sales/discount-policy (conflict copy).md',
  );
}
