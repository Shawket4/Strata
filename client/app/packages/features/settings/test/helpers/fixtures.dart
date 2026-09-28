import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Settings fixtures (SCREEN_SPEC AccountSheetCompact settings list,
/// ReminderNotifications settings column).
abstract final class SettingsFixtures {
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
