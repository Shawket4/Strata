import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'settings_localizations_ar.dart';
import 'settings_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of SettingsLocalizations
/// returned by `SettingsLocalizations.of(context)`.
///
/// Applications need to include `SettingsLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/settings_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: SettingsLocalizations.localizationsDelegates,
///   supportedLocales: SettingsLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the SettingsLocalizations.supportedLocales
/// property.
abstract class SettingsLocalizations {
  SettingsLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static SettingsLocalizations of(BuildContext context) {
    return Localizations.of<SettingsLocalizations>(
      context,
      SettingsLocalizations,
    )!;
  }

  static const LocalizationsDelegate<SettingsLocalizations> delegate =
      _SettingsLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('en'),
  ];

  /// Settings title.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get title;

  /// Settings nav group.
  ///
  /// In en, this message translates to:
  /// **'YOU'**
  String get groupYou;

  /// Settings nav group.
  ///
  /// In en, this message translates to:
  /// **'APP'**
  String get groupApp;

  /// Settings nav group.
  ///
  /// In en, this message translates to:
  /// **'ADMIN'**
  String get groupAdmin;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Account'**
  String get sectionAccount;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get sectionDevices;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Reminders'**
  String get sectionReminders;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'AI'**
  String get sectionAi;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Integrity'**
  String get sectionIntegrity;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Export & import'**
  String get sectionData;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get sectionSync;

  /// Section: user management (admin).
  ///
  /// In en, this message translates to:
  /// **'Users'**
  String get sectionAdmin;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get sectionAbout;

  /// Accessibility label of the settings navigation.
  ///
  /// In en, this message translates to:
  /// **'Settings sections'**
  String get settingsNav;

  /// Opens the account sheet.
  ///
  /// In en, this message translates to:
  /// **'Account and sign out'**
  String get openAccount;

  /// Account header subtitle.
  ///
  /// In en, this message translates to:
  /// **'@{username} · {role}'**
  String atUsernameRole({required String username, required String role});

  /// Role admin.
  ///
  /// In en, this message translates to:
  /// **'Admin'**
  String get roleAdmin;

  /// Role member.
  ///
  /// In en, this message translates to:
  /// **'Member'**
  String get roleMember;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get displayName;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get username;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Role'**
  String get role;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get server;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// The English UI language (in its own script).
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get languageEn;

  /// The Arabic UI language (in its own script).
  ///
  /// In en, this message translates to:
  /// **'العربية'**
  String get languageAr;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Time zone'**
  String get timezone;

  /// Password change group.
  ///
  /// In en, this message translates to:
  /// **'Change password'**
  String get changePasswordTitle;

  /// Field.
  ///
  /// In en, this message translates to:
  /// **'Current password'**
  String get currentPassword;

  /// Field.
  ///
  /// In en, this message translates to:
  /// **'New password'**
  String get newPassword;

  /// Sign out.
  ///
  /// In en, this message translates to:
  /// **'Sign out'**
  String get signOut;

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDevice;

  /// Renames a device.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get rename;

  /// Revokes a device.
  ///
  /// In en, this message translates to:
  /// **'Revoke'**
  String get revoke;

  /// Switch label.
  ///
  /// In en, this message translates to:
  /// **'Reminders on this device'**
  String get remindersOnDevice;

  /// Switch helper.
  ///
  /// In en, this message translates to:
  /// **'Every device with reminders on notifies you.'**
  String get remindersOnDeviceHelp;

  /// Permission denied banner title.
  ///
  /// In en, this message translates to:
  /// **'Reminders are off on this device'**
  String get remindersOffPermission;

  /// Permission denied banner body.
  ///
  /// In en, this message translates to:
  /// **'Allow notifications (and exact alarms on Android) for Strata in the system settings.'**
  String get remindersOffPermissionBody;

  /// Permission unknown.
  ///
  /// In en, this message translates to:
  /// **'Notification permission not checked yet'**
  String get remindersPermissionUnknown;

  /// Permission granted.
  ///
  /// In en, this message translates to:
  /// **'Notifications allowed'**
  String get remindersPermissionGranted;

  /// Notification mode OS scheduled.
  ///
  /// In en, this message translates to:
  /// **'Delivered by the system, even when Strata is closed'**
  String get deliveryOs;

  /// Notification mode while running (Linux).
  ///
  /// In en, this message translates to:
  /// **'Delivered while Strata is running'**
  String get deliveryWhileRunning;

  /// Reminders scheduled with the OS.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No reminders scheduled} =1{1 reminder scheduled} other{{count} reminders scheduled}}'**
  String scheduledCount({required int count});

  /// Label.
  ///
  /// In en, this message translates to:
  /// **'Default reminder time'**
  String get defaultTime;

  /// Helper.
  ///
  /// In en, this message translates to:
  /// **'For tasks with a date but no time'**
  String get defaultTimeHelp;

  /// Footnote.
  ///
  /// In en, this message translates to:
  /// **'Marking a reminder Done or Snoozed on one device clears it on the others at the next sync.'**
  String get remindersSyncNote;

  /// AI section description.
  ///
  /// In en, this message translates to:
  /// **'AI provider status, thresholds, auto-filing and the daily budget.'**
  String get aiBody;

  /// Integrity section description.
  ///
  /// In en, this message translates to:
  /// **'Warnings about files the server couldn\'t read or index.'**
  String get integrityBody;

  /// Export & import description.
  ///
  /// In en, this message translates to:
  /// **'Download your whole vault as a zip, or import markdown files.'**
  String get dataBody;

  /// Export action.
  ///
  /// In en, this message translates to:
  /// **'Export vault (.zip)'**
  String get exportVault;

  /// Import action.
  ///
  /// In en, this message translates to:
  /// **'Import markdown…'**
  String get importFiles;

  /// Availability available.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get available;

  /// Availability offline.
  ///
  /// In en, this message translates to:
  /// **'Needs a connection'**
  String get unavailableOffline;

  /// Availability not yet available.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get unavailableNotYet;

  /// Availability not allowed.
  ///
  /// In en, this message translates to:
  /// **'Not available for this account'**
  String get unavailableNotAllowed;

  /// Offline state body.
  ///
  /// In en, this message translates to:
  /// **'This needs your server. Connect and try again.'**
  String get offlineBody;

  /// Not yet available body.
  ///
  /// In en, this message translates to:
  /// **'Your server doesn\'t offer this yet.'**
  String get notYetBody;

  /// Not allowed body.
  ///
  /// In en, this message translates to:
  /// **'This account can\'t use this.'**
  String get notAllowedBody;

  /// Sync section description.
  ///
  /// In en, this message translates to:
  /// **'Sync status, the outbox and conflicts.'**
  String get syncBody;

  /// Opens the sync status.
  ///
  /// In en, this message translates to:
  /// **'Open sync status'**
  String get openSyncStatus;

  /// Admin section description.
  ///
  /// In en, this message translates to:
  /// **'Approve new accounts, disable users, reset passwords and schedule deletions.'**
  String get adminBody;

  /// Opens the admin user list.
  ///
  /// In en, this message translates to:
  /// **'Manage users'**
  String get openAdminUsers;

  /// About text.
  ///
  /// In en, this message translates to:
  /// **'Strata keeps your notes as markdown on your own server, with a copy on each device.'**
  String get aboutBody;

  /// Opens the licences page (incl. the bundled fonts).
  ///
  /// In en, this message translates to:
  /// **'Licences'**
  String get licences;

  /// Font credits.
  ///
  /// In en, this message translates to:
  /// **'Cairo, IBM Plex Mono and Quicksand are bundled under the SIL Open Font License.'**
  String get fontsNote;

  /// Back button.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get back;

  /// Loading label.
  ///
  /// In en, this message translates to:
  /// **'Loading settings'**
  String get loading;

  /// Error title.
  ///
  /// In en, this message translates to:
  /// **'Settings could not be loaded'**
  String get loadFailed;

  /// Any core error.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong ({code}).'**
  String errorGeneric({required String code});

  /// Core error offline.
  ///
  /// In en, this message translates to:
  /// **'You\'re offline.'**
  String get errorOffline;

  /// Heading of the password change group.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get passwordGroup;

  /// Edit button tooltip.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// Edit display name dialog title.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get editDisplayNameTitle;

  /// Edit time zone dialog title.
  ///
  /// In en, this message translates to:
  /// **'Time zone'**
  String get editTimezoneTitle;

  /// Time zone field help.
  ///
  /// In en, this message translates to:
  /// **'Dates and reminders follow it.'**
  String get timezoneHelp;

  /// Time zone picker: search field hint.
  ///
  /// In en, this message translates to:
  /// **'Search a city, region or UTC offset'**
  String get timezoneSearch;

  /// Time zone picker: nothing found.
  ///
  /// In en, this message translates to:
  /// **'No time zone matches “{query}”'**
  String timezoneNoMatch({required String query});

  /// Time zone picker: the account's zone.
  ///
  /// In en, this message translates to:
  /// **'Current'**
  String get timezoneCurrent;

  /// Save button.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// Cancel button.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// Snack bar after changing the password.
  ///
  /// In en, this message translates to:
  /// **'Password changed'**
  String get passwordChanged;

  /// Snack bar after a setting is saved.
  ///
  /// In en, this message translates to:
  /// **'Saved'**
  String get saved;

  /// Refresh button tooltip.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// Badge on the current device's row.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDeviceBadge;

  /// Device row subtitle.
  ///
  /// In en, this message translates to:
  /// **'Last seen {lastSeen} · signed in {signedIn}'**
  String deviceDetail({required String lastSeen, required String signedIn});

  /// Per-device reminders switch label.
  ///
  /// In en, this message translates to:
  /// **'Reminders on {name}'**
  String deviceReminders({required String name});

  /// Rename device button label.
  ///
  /// In en, this message translates to:
  /// **'Rename {name}'**
  String renameDevice({required String name});

  /// Revoke device button label.
  ///
  /// In en, this message translates to:
  /// **'Sign out {name}'**
  String revokeDevice({required String name});

  /// Rename dialog title.
  ///
  /// In en, this message translates to:
  /// **'Rename device'**
  String get renameTitle;

  /// Revoke dialog title.
  ///
  /// In en, this message translates to:
  /// **'Sign {name} out?'**
  String revokeTitle({required String name});

  /// Revoke dialog body.
  ///
  /// In en, this message translates to:
  /// **'The device is signed out and its local copy of your notes is removed the next time it connects.'**
  String get revokeBody;

  /// Empty device list.
  ///
  /// In en, this message translates to:
  /// **'No devices yet.'**
  String get noDevices;

  /// Snooze setting title.
  ///
  /// In en, this message translates to:
  /// **'Snooze length'**
  String get snooze;

  /// Snooze length option.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 minute} other{{count} minutes}}'**
  String snoozeMinutes({required int count});

  /// Quiet hours switch.
  ///
  /// In en, this message translates to:
  /// **'Quiet hours'**
  String get quietHours;

  /// Quiet hours help.
  ///
  /// In en, this message translates to:
  /// **'No reminder rings between these times; they wait until the end.'**
  String get quietHoursHelp;

  /// Quiet hours start field.
  ///
  /// In en, this message translates to:
  /// **'From (HH:MM)'**
  String get quietFrom;

  /// Quiet hours end field.
  ///
  /// In en, this message translates to:
  /// **'Until (HH:MM)'**
  String get quietUntil;

  /// Default reminder time field.
  ///
  /// In en, this message translates to:
  /// **'Time (HH:MM)'**
  String get timeField;

  /// AI status headline with the provider.
  ///
  /// In en, this message translates to:
  /// **'AI on · {provider}'**
  String aiOn({required String provider});

  /// AI status headline.
  ///
  /// In en, this message translates to:
  /// **'AI on'**
  String get aiOnNoProvider;

  /// AI disabled.
  ///
  /// In en, this message translates to:
  /// **'AI off on this server'**
  String get aiOff;

  /// AI queue depth.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Nothing waiting for AI} =1{1 job waiting} other{{count} jobs waiting}}'**
  String aiQueue({required int count});

  /// AI budget meter title.
  ///
  /// In en, this message translates to:
  /// **'Daily budget'**
  String get aiBudget;

  /// Embedding progress.
  ///
  /// In en, this message translates to:
  /// **'Search index {percent}%'**
  String aiEmbeddings({required int percent});

  /// AI thresholds read-only note.
  ///
  /// In en, this message translates to:
  /// **'Thresholds and auto-filing are set on the server.'**
  String get aiThresholdsNote;

  /// Integrity list empty.
  ///
  /// In en, this message translates to:
  /// **'No integrity warnings.'**
  String get integrityEmpty;

  /// Integrity: temp_file_removed.
  ///
  /// In en, this message translates to:
  /// **'A leftover temporary file was removed'**
  String get integrityTempFile;

  /// Integrity: uncommitted_changes.
  ///
  /// In en, this message translates to:
  /// **'Uncommitted changes were found and saved'**
  String get integrityUncommitted;

  /// Integrity: id_assigned.
  ///
  /// In en, this message translates to:
  /// **'A note without an ID got one'**
  String get integrityIdAssigned;

  /// Integrity: out_of_band_edit.
  ///
  /// In en, this message translates to:
  /// **'A file was edited outside Strata and re-read'**
  String get integrityOutOfBand;

  /// Integrity: missing_file.
  ///
  /// In en, this message translates to:
  /// **'A file is missing from the vault'**
  String get integrityMissingFile;

  /// Integrity: sidecar_repaired.
  ///
  /// In en, this message translates to:
  /// **'A metadata file was repaired'**
  String get integritySidecar;

  /// Integrity: orphan_sidecar_removed.
  ///
  /// In en, this message translates to:
  /// **'An orphaned metadata file was removed'**
  String get integrityOrphanSidecar;

  /// Integrity: index_repaired.
  ///
  /// In en, this message translates to:
  /// **'The search index was repaired'**
  String get integrityIndexRepaired;

  /// Integrity: interrupted_write_rolled_back.
  ///
  /// In en, this message translates to:
  /// **'An interrupted write was rolled back'**
  String get integrityRolledBack;

  /// Integrity: op_result_recovered.
  ///
  /// In en, this message translates to:
  /// **'A change\'s result was recovered'**
  String get integrityRecovered;

  /// Integrity: unknown kind.
  ///
  /// In en, this message translates to:
  /// **'Integrity warning: {kind}'**
  String integrityOther({required String kind});

  /// Snack bar after an export.
  ///
  /// In en, this message translates to:
  /// **'Exported {label}'**
  String exportDone({required String label});

  /// Snack bar after an import.
  ///
  /// In en, this message translates to:
  /// **'{imported} imported · {skipped} skipped'**
  String importDone({required int imported, required int skipped});
}

class _SettingsLocalizationsDelegate
    extends LocalizationsDelegate<SettingsLocalizations> {
  const _SettingsLocalizationsDelegate();

  @override
  Future<SettingsLocalizations> load(Locale locale) {
    return SynchronousFuture<SettingsLocalizations>(
      lookupSettingsLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_SettingsLocalizationsDelegate old) => false;
}

SettingsLocalizations lookupSettingsLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return SettingsLocalizationsAr();
    case 'en':
      return SettingsLocalizationsEn();
  }

  throw FlutterError(
    'SettingsLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
