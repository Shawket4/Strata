import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Account fixtures from SCREEN_SPEC (LoginCompact … DeletionPending*).
abstract final class AccountFixtures {
  /// Mona, disabled by an admin, with 2 unsynced ops.
  static const SessionState disabled = SessionState(
    kind: SessionKind.disabled,
    account: AccountSummary(
      userId: 'u-mona',
      username: 'mona.h',
      displayName: 'Mona Hassan',
      role: 'member',
      isAdmin: false,
      serverUrl: StrataFixtures.serverUrl,
      timezone: 'Africa/Cairo',
      uiLanguage: 'en',
      initials: '',
    ),
    knownAccounts: [],
    deviceName: "Mona's Pixel",
    unsyncedOps: 2,
  );

  /// Password reset by an admin.
  static const SessionState passwordChange = SessionState(
    kind: SessionKind.passwordChangeRequired,
    account: StrataFixtures.accountSummary,
    knownAccounts: [],
    deviceName: 'shawket-laptop',
    unsyncedOps: 0,
  );

  /// The two unsynced ops of AccountDisabledCompact.
  static final SyncStatusView unsynced = SyncStatusView(
    pill: StrataFixtures.syncPillOffline,
    bootstrapComplete: true,
    outbox: [
      OutboxItem(
        opId: 'op-1',
        kind: 'note.update',
        title: 'Nile Freight — November rates',
        status: OutboxStatus.pending,
        attempts: 1,
        created: DateTime.utc(2026, 9, 27, 9, 5),
        detail: '',
        detailDir: TextDir.ltr,
        createdLabel: '',
      ),
      OutboxItem(
        opId: 'op-2',
        kind: 'note.create',
        title: 'الفواتير الشهرية لازم تتراجع قبل الخميس',
        status: OutboxStatus.pending,
        attempts: 0,
        created: DateTime.utc(2026, 9, 27, 10, 40),
        detail: '',
        detailDir: TextDir.ltr,
        createdLabel: '',
      ),
    ],
    conflicts: const [],
    rejections: const [],
    paused: false,
    log: [],
  );

  /// `invalid_credentials`.
  static const CoreFailure invalidCredentials = CoreFailure(
    code: 'invalid_credentials',
    messageKey: 'error.invalid_credentials',
  );

  /// `account_pending`.
  static const CoreFailure pending = CoreFailure(
    code: 'account_pending',
    messageKey: 'error.account_pending',
  );

  /// `account_rejected`.
  static const CoreFailure rejected = CoreFailure(
    code: 'account_rejected',
    messageKey: 'error.account_rejected',
  );
}
