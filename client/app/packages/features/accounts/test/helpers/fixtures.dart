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
      initials: 'MH',
    ),
    knownAccounts: [],
    deviceName: "Mona's Pixel",
    unsyncedOps: 2,
  );

  /// Shawket on his laptop: account sheet details (this device, 3 devices,
  /// 2 sign-ups waiting).
  static final SessionState active = SessionState(
    kind: SessionKind.active,
    account: const AccountSummary(
      userId: 'u-shawket',
      username: 'shawket',
      displayName: 'Shawket',
      role: 'admin',
      isAdmin: true,
      serverUrl: StrataFixtures.serverUrl,
      timezone: 'Africa/Cairo',
      uiLanguage: 'en',
      initials: 'S',
    ),
    knownAccounts: const [],
    deviceName: "Shawket's Pixel 9",
    unsyncedOps: 0,
    thisDevice: DeviceItem(
      id: 'd-pixel',
      name: "Shawket's Pixel 9",
      platform: 'android',
      lastSeen: StrataFixtures.now,
      lastSeenLabel: 'Now',
      signedIn: StrataFixtures.now,
      signedInLabel: '12 Sep',
      isThisDevice: true,
      remindersEnabled: true,
    ),
    deviceCount: 3,
    pendingApprovals: 2,
  );

  /// The login screen with one known account.
  static const SessionState signedOut = SessionState(
    kind: SessionKind.signedOut,
    knownAccounts: [
      KnownAccountItem(
        userId: 'u-shawket',
        username: 'shawket',
        displayName: 'Shawket',
        initials: 'S',
      ),
    ],
    deviceName: 'shawket-laptop',
    unsyncedOps: 0,
  );

  static final PendingApproval _request = PendingApproval(
    username: 'sara.n',
    requestedAt: DateTime.utc(2026, 9, 27, 9, 30),
    requestedLabel: '2 hours ago',
    lastCheckedAt: DateTime.utc(2026, 9, 27, 11, 32),
    lastCheckedLabel: 'Last checked 14:32',
    canCheck: true,
  );

  /// Sara's sign-up, waiting for approval.
  static final SessionState waiting = SessionState(
    kind: SessionKind.pendingApproval,
    knownAccounts: const [],
    deviceName: "Sara's iPad",
    unsyncedOps: 0,
    pending: _request,
  );

  /// Sara's sign-up after the app restarted: the password is not kept, so
  /// "Check again" is not possible (sign in again instead).
  static final SessionState waitingNoRetry = SessionState(
    kind: SessionKind.pendingApproval,
    knownAccounts: const [],
    deviceName: "Sara's iPad",
    unsyncedOps: 0,
    pending: PendingApproval(
      username: 'sara.n',
      requestedAt: DateTime.utc(2026, 9, 27, 9, 30),
      requestedLabel: '2 hours ago',
      canCheck: false,
    ),
  );

  /// Sara's sign-up was turned down.
  static final SessionState notApproved = SessionState(
    kind: SessionKind.rejected,
    knownAccounts: const [],
    deviceName: "Sara's iPad",
    unsyncedOps: 0,
    pending: _request,
  );

  /// Karim, scheduled for deletion on Sun 11 Oct 2026, 2 ops never synced.
  static final SessionState deletionPending = SessionState(
    kind: SessionKind.deletionPending,
    account: const AccountSummary(
      userId: 'u-karim',
      username: 'karim',
      displayName: 'Karim Adel',
      role: 'member',
      isAdmin: false,
      serverUrl: StrataFixtures.serverUrl,
      timezone: 'Africa/Cairo',
      uiLanguage: 'en',
      initials: 'KA',
    ),
    knownAccounts: const [],
    deviceName: 'karim-phone',
    unsyncedOps: 2,
    deletionAt: DateTime.utc(2026, 10, 11, 9),
    daysRemaining: 14,
    deletionLabel: 'Sun 11 Oct 2026',
    exportSizeBytes: BigInt.from(19293798),
    exportNoteCount: 412,
    exportLabel: '18.4 MB · 412 notes',
  );

  /// Karim's deletion with nothing left to sync.
  static final SessionState deletionSynced = SessionState(
    kind: SessionKind.deletionPending,
    account: deletionPending.account,
    knownAccounts: const [],
    deviceName: 'karim-phone',
    unsyncedOps: 0,
    daysRemaining: 14,
    deletionLabel: 'Sun 11 Oct 2026',
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
        createdLabel: 'today 11:05',
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
        createdLabel: 'today 12:40',
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
