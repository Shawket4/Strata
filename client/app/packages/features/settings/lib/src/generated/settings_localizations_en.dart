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
  String get signOut => 'Sign out';

  @override
  String get thisDevice => 'This device';

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
  String get openAdminUsers => 'Manage users';

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

  @override
  String get passwordGroup => 'Password';

  @override
  String get edit => 'Edit';

  @override
  String get editDisplayNameTitle => 'Display name';

  @override
  String get editTimezoneTitle => 'Time zone';

  @override
  String get timezoneHelp => 'Dates and reminders follow it.';

  @override
  String get timezoneSearch => 'Search a city, region or UTC offset';

  @override
  String timezoneNoMatch({required String query}) {
    return 'No time zone matches “$query”';
  }

  @override
  String get timezoneCurrent => 'Current';

  @override
  String get save => 'Save';

  @override
  String get cancel => 'Cancel';

  @override
  String get passwordChanged => 'Password changed';

  @override
  String get saved => 'Saved';

  @override
  String get refresh => 'Refresh';

  @override
  String get thisDeviceBadge => 'This device';

  @override
  String deviceDetail({required String lastSeen, required String signedIn}) {
    return 'Last seen $lastSeen · signed in $signedIn';
  }

  @override
  String deviceReminders({required String name}) {
    return 'Reminders on $name';
  }

  @override
  String renameDevice({required String name}) {
    return 'Rename $name';
  }

  @override
  String revokeDevice({required String name}) {
    return 'Sign out $name';
  }

  @override
  String get renameTitle => 'Rename device';

  @override
  String revokeTitle({required String name}) {
    return 'Sign $name out?';
  }

  @override
  String get revokeBody =>
      'The device is signed out and its local copy of your notes is removed the next time it connects.';

  @override
  String get noDevices => 'No devices yet.';

  @override
  String get snooze => 'Snooze length';

  @override
  String snoozeMinutes({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String get quietHours => 'Quiet hours';

  @override
  String get quietHoursHelp =>
      'No reminder rings between these times; they wait until the end.';

  @override
  String get quietFrom => 'From (HH:MM)';

  @override
  String get quietUntil => 'Until (HH:MM)';

  @override
  String get timeField => 'Time (HH:MM)';

  @override
  String aiOn({required String provider}) {
    return 'AI on · $provider';
  }

  @override
  String get aiOnNoProvider => 'AI on';

  @override
  String get aiOff => 'AI off on this server';

  @override
  String aiQueue({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count jobs waiting',
      one: '1 job waiting',
      zero: 'Nothing waiting for AI',
    );
    return '$_temp0';
  }

  @override
  String get aiBudget => 'Daily budget';

  @override
  String aiEmbeddings({required int percent}) {
    return 'Search index $percent%';
  }

  @override
  String get aiThresholdsNote =>
      'Thresholds and auto-filing are set on the server.';

  @override
  String get integrityEmpty => 'No integrity warnings.';

  @override
  String get integrityTempFile => 'A leftover temporary file was removed';

  @override
  String get integrityUncommitted => 'Uncommitted changes were found and saved';

  @override
  String get integrityIdAssigned => 'A note without an ID got one';

  @override
  String get integrityOutOfBand =>
      'A file was edited outside Strata and re-read';

  @override
  String get integrityMissingFile => 'A file is missing from the vault';

  @override
  String get integritySidecar => 'A metadata file was repaired';

  @override
  String get integrityOrphanSidecar => 'An orphaned metadata file was removed';

  @override
  String get integrityIndexRepaired => 'The search index was repaired';

  @override
  String get integrityRolledBack => 'An interrupted write was rolled back';

  @override
  String get integrityRecovered => 'A change\'s result was recovered';

  @override
  String integrityOther({required String kind}) {
    return 'Integrity warning: $kind';
  }

  @override
  String exportDone({required String label}) {
    return 'Exported $label';
  }

  @override
  String importDone({required int imported, required int skipped}) {
    return '$imported imported · $skipped skipped';
  }
}
