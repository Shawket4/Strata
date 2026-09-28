import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Settings fixtures (SCREEN_SPEC AccountSheetCompact settings list,
/// ReminderNotifications settings column).
abstract final class SettingsFixtures {
  /// Shawket (admin) on the laptop: three devices, AI paused at the budget
  /// with two failed jobs, two integrity warnings, quiet hours on.
  static final SettingsView full = withFailedJobs(2);

  /// [full] with [failedJobs] failed AI jobs.
  static SettingsView withFailedJobs(int failedJobs) => SettingsView(
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
    reminders: const RemindersSetting(
      enabled: true,
      permission: NotificationPermission.granted,
      mode: NotificationMode.osScheduled,
      scheduled: 4,
      defaultTime: '09:00',
      snoozeMinutes: 10,
      quietEnabled: true,
      quietFrom: '22:00',
      quietUntil: '07:00',
    ),
    devices: Availability.available,
    ai: Availability.available,
    export_: Availability.available,
    integrity: Availability.available,
    admin: Availability.available,
    deviceList: [
      _device(
        'd-laptop',
        'shawket-laptop',
        'linux',
        lastSeen: 'Now',
        signedIn: '4 Jan',
        thisDevice: true,
        reminders: true,
      ),
      _device(
        'd-pixel',
        'Pixel 9',
        'android',
        lastSeen: '2h ago',
        signedIn: '12 Mar',
        reminders: true,
      ),
      _device(
        'd-mac',
        'MacBook Pro',
        'macos',
        lastSeen: '3 days ago',
        signedIn: '2 Feb',
      ),
    ],
    aiStatus: AiStatusView(
      enabled: true,
      provider: 'Anthropic',
      pausedLabel: 'AI paused — daily budget reached',
      queueDepth: 3,
      failedJobs: failedJobs,
      budgetUsedPercent: 100,
      budgetLabel: r'$2.00 of $2.00 today',
      embeddingPercent: 84,
    ),
    integrityWarnings: const [
      IntegrityItem(
        id: 'w-1',
        kind: 'out_of_band_edit',
        messageKey: 'integrity.out_of_band_edit',
        path: 'notes/sales/Pricing experiments.md',
        createdLabel: 'Today 09:12',
      ),
      IntegrityItem(
        id: 'w-2',
        kind: 'disk_full',
        messageKey: 'integrity.disk_full',
        createdLabel: '12 Sep',
      ),
    ],
    refreshedLabel: 'Updated 14:32',
  );

  static DeviceItem _device(
    String id,
    String name,
    String platform, {
    required String lastSeen,
    required String signedIn,
    bool thisDevice = false,
    bool reminders = false,
  }) => DeviceItem(
    id: id,
    name: name,
    platform: platform,
    lastSeen: StrataFixtures.now,
    lastSeenLabel: lastSeen,
    signedIn: StrataFixtures.now,
    signedInLabel: signedIn,
    isThisDevice: thisDevice,
    remindersEnabled: reminders,
  );

  /// Reminders denied by the platform, Linux delivery.
  static const SettingsView denied = SettingsView(
    account: StrataFixtures.accountSummary,
    reminders: RemindersSetting(
      enabled: true,
      permission: NotificationPermission.denied,
      mode: NotificationMode.whileRunning,
      scheduled: 0,
      defaultTime: '09:00',
      snoozeMinutes: 0,
      quietEnabled: false,
      quietFrom: '',
      quietUntil: '',
    ),
    devices: Availability.offline,
    ai: Availability.offline,
    export_: Availability.offline,
    integrity: Availability.notAllowed,
    admin: Availability.notAllowed,
    deviceList: [],
    integrityWarnings: [],
  );

  /// Online member with reminders off.
  static const SettingsView member = SettingsView(
    account: AccountSummary(
      userId: 'u-sara',
      username: 'sara.n',
      displayName: 'Sara Nabil',
      role: 'member',
      isAdmin: false,
      serverUrl: StrataFixtures.serverUrl,
      timezone: 'Africa/Cairo',
      uiLanguage: 'ar',
      initials: '',
    ),
    reminders: RemindersSetting(
      enabled: false,
      permission: NotificationPermission.unknown,
      mode: NotificationMode.osScheduled,
      scheduled: 1,
      defaultTime: '08:30',
      snoozeMinutes: 0,
      quietEnabled: false,
      quietFrom: '',
      quietUntil: '',
    ),
    devices: Availability.available,
    ai: Availability.available,
    export_: Availability.notYetAvailable,
    integrity: Availability.available,
    admin: Availability.notAllowed,
    deviceList: [],
    integrityWarnings: [],
  );
}
