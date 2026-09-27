// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'settings_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class SettingsLocalizationsEn extends SettingsLocalizations {
  SettingsLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get title => 'Settings';

  @override
  String get groupYou => 'YOU';

  @override
  String get groupApp => 'APP';

  @override
  String get groupAdmin => 'ADMIN';

  @override
  String get sectionAccount => 'Account';

  @override
  String get sectionDevices => 'Devices';

  @override
  String get sectionReminders => 'Reminders';

  @override
  String get sectionAi => 'AI';

  @override
  String get sectionIntegrity => 'Integrity';

  @override
  String get sectionData => 'Export & import';

  @override
  String get sectionSync => 'Sync';

  @override
  String get sectionAdmin => 'Users';

  @override
  String get sectionAbout => 'About';

  @override
  String get settingsNav => 'Settings sections';

  @override
  String get openAccount => 'Account and sign out';

  @override
  String atUsernameRole({required String username, required String role}) {
    return '@$username · $role';
  }

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleMember => 'Member';

  @override
  String get displayName => 'Display name';

  @override
  String get username => 'Username';

  @override
  String get role => 'Role';

  @override
  String get server => 'Server';

  @override
  String get language => 'Language';

  @override
  String get languageEn => 'English';

  @override
  String get languageAr => 'العربية';

  @override
  String get timezone => 'Time zone';

  @override
  String get changePasswordTitle => 'Change password';

  @override
  String get currentPassword => 'Current password';

  @override
  String get newPassword => 'New password';

  @override
  String get notAvailableYet => 'Not available yet';

  @override
  String get signOut => 'Sign out';

  @override
  String get thisDevice => 'This device';

  @override
  String get deviceListNote =>
      'Renaming and revoking devices comes with the device list.';

  @override
  String get rename => 'Rename';

  @override
  String get revoke => 'Revoke';

  @override
  String get remindersOnDevice => 'Reminders on this device';

  @override
  String get remindersOnDeviceHelp =>
      'Every device with reminders on notifies you.';

  @override
  String get remindersOffPermission => 'Reminders are off on this device';

  @override
  String get remindersOffPermissionBody =>
      'Allow notifications (and exact alarms on Android) for Strata in the system settings.';

  @override
  String get remindersPermissionUnknown =>
      'Notification permission not checked yet';

  @override
  String get remindersPermissionGranted => 'Notifications allowed';

  @override
  String get deliveryOs =>
      'Delivered by the system, even when Strata is closed';

  @override
  String get deliveryWhileRunning => 'Delivered while Strata is running';

  @override
  String scheduledCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count reminders scheduled',
      one: '1 reminder scheduled',
      zero: 'No reminders scheduled',
    );
    return '$_temp0';
  }

  @override
  String get defaultTime => 'Default reminder time';

  @override
  String get defaultTimeHelp => 'For tasks with a date but no time';

  @override
  String get remindersSyncNote =>
      'Marking a reminder Done or Snoozed on one device clears it on the others at the next sync.';

  @override
  String get aiBody =>
      'AI provider status, thresholds, auto-filing and the daily budget.';

  @override
  String get integrityBody =>
      'Warnings about files the server couldn\'t read or index.';

  @override
  String get dataBody =>
      'Download your whole vault as a zip, or import markdown files.';

  @override
  String get exportVault => 'Export vault (.zip)';

  @override
  String get importFiles => 'Import markdown…';

  @override
  String get available => 'Available';

  @override
  String get unavailableOffline => 'Needs a connection';

  @override
  String get unavailableNotYet => 'Not available yet';

  @override
  String get unavailableNotAllowed => 'Not available for this account';

  @override
  String get offlineBody => 'This needs your server. Connect and try again.';

  @override
  String get notYetBody => 'Your server doesn\'t offer this yet.';

  @override
  String get notAllowedBody => 'This account can\'t use this.';

  @override
  String get syncBody => 'Sync status, the outbox and conflicts.';

  @override
  String get openSyncStatus => 'Open sync status';

  @override
  String get adminBody =>
      'Approve new accounts, disable users, reset passwords and schedule deletions.';

  @override
  String get openAdminUsers => 'Open Admin → Users';

  @override
  String get aboutBody =>
      'Strata keeps your notes as markdown on your own server, with a copy on each device.';

  @override
  String get licences => 'Licences';

  @override
  String get fontsNote =>
      'Cairo, IBM Plex Mono and Quicksand are bundled under the SIL Open Font License.';

  @override
  String get back => 'Back';

  @override
  String get loading => 'Loading settings';

  @override
  String get loadFailed => 'Settings could not be loaded';

  @override
  String errorGeneric({required String code}) {
    return 'Something went wrong ($code).';
  }

  @override
  String get errorOffline => 'You\'re offline.';
}
