import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'accounts_localizations_ar.dart';
import 'accounts_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AccountsLocalizations
/// returned by `AccountsLocalizations.of(context)`.
///
/// Applications need to include `AccountsLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/accounts_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AccountsLocalizations.localizationsDelegates,
///   supportedLocales: AccountsLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the AccountsLocalizations.supportedLocales
/// property.
abstract class AccountsLocalizations {
  AccountsLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AccountsLocalizations of(BuildContext context) {
    return Localizations.of<AccountsLocalizations>(
      context,
      AccountsLocalizations,
    )!;
  }

  static const LocalizationsDelegate<AccountsLocalizations> delegate =
      _AccountsLocalizationsDelegate();

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

  /// Sign-in screen title and button.
  ///
  /// In en, this message translates to:
  /// **'Sign in'**
  String get signInTitle;

  /// Sign-in subtitle (compact).
  ///
  /// In en, this message translates to:
  /// **'Your notes stay on your server and on this device.'**
  String get signInSubtitle;

  /// Sign-in subtitle (wide layouts).
  ///
  /// In en, this message translates to:
  /// **'One account per device. Your notes are private to you.'**
  String get signInSubtitleWide;

  /// Small caps line over the brand panel.
  ///
  /// In en, this message translates to:
  /// **'SELF-HOSTED KNOWLEDGE'**
  String get brandKicker;

  /// Brand panel tagline.
  ///
  /// In en, this message translates to:
  /// **'Captures filed, notes linked, people and companies kept current. In Arabic and English, online or off.'**
  String get brandTagline;

  /// Server URL field label.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get fieldServer;

  /// Username field label.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get fieldUsername;

  /// Password field label.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get fieldPassword;

  /// Confirm password field label.
  ///
  /// In en, this message translates to:
  /// **'Confirm password'**
  String get fieldConfirmPassword;

  /// New password field label.
  ///
  /// In en, this message translates to:
  /// **'New password'**
  String get fieldNewPassword;

  /// Current password field label.
  ///
  /// In en, this message translates to:
  /// **'Current password'**
  String get fieldCurrentPassword;

  /// Device name field label.
  ///
  /// In en, this message translates to:
  /// **'Device name'**
  String get fieldDeviceName;

  /// Display name field label.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get fieldDisplayName;

  /// Helper text under the device name.
  ///
  /// In en, this message translates to:
  /// **'Shown in your list of signed-in devices. You can rename it later.'**
  String get deviceNameHint;

  /// Tooltip of the show-password button.
  ///
  /// In en, this message translates to:
  /// **'Show password'**
  String get showPassword;

  /// Tooltip of the hide-password button.
  ///
  /// In en, this message translates to:
  /// **'Hide password'**
  String get hidePassword;

  /// Keyboard hint after the Enter key chip.
  ///
  /// In en, this message translates to:
  /// **'to sign in'**
  String get enterToSignIn;

  /// Prompt before the sign-up link.
  ///
  /// In en, this message translates to:
  /// **'New here?'**
  String get newHere;

  /// Sign-up link and screen title.
  ///
  /// In en, this message translates to:
  /// **'Create an account'**
  String get createAccount;

  /// Password help line.
  ///
  /// In en, this message translates to:
  /// **'Forgot it? Ask your admin to reset it.'**
  String get forgotPassword;

  /// Heading of the known accounts list.
  ///
  /// In en, this message translates to:
  /// **'Accounts on this device'**
  String get knownAccountsTitle;

  /// Switches to a known account.
  ///
  /// In en, this message translates to:
  /// **'Continue as {name}'**
  String continueAs({required String name});

  /// Sign-up subtitle with the server.
  ///
  /// In en, this message translates to:
  /// **'on {server}'**
  String signUpOn({required String server});

  /// Notice on the sign-up form.
  ///
  /// In en, this message translates to:
  /// **'An administrator must approve new accounts. You\'ll be able to sign in once approved.'**
  String get approvalNotice;

  /// Sign-up submit button.
  ///
  /// In en, this message translates to:
  /// **'Request account'**
  String get requestAccount;

  /// Prompt before the sign-in link.
  ///
  /// In en, this message translates to:
  /// **'Already have an account?'**
  String get haveAccount;

  /// Back button of the sign-up screen.
  ///
  /// In en, this message translates to:
  /// **'Back to sign in'**
  String get backToSignIn;

  /// Confirm-password helper when both fields are equal.
  ///
  /// In en, this message translates to:
  /// **'Passwords match'**
  String get passwordsMatch;

  /// Confirm-password error.
  ///
  /// In en, this message translates to:
  /// **'Passwords don\'t match'**
  String get passwordsDiffer;

  /// Status pill on the waiting screen.
  ///
  /// In en, this message translates to:
  /// **'Pending'**
  String get pendingPill;

  /// Waiting-for-approval title.
  ///
  /// In en, this message translates to:
  /// **'Waiting for approval'**
  String get pendingTitle;

  /// Pending approval body with the core's request time label.
  ///
  /// In en, this message translates to:
  /// **'Your request for @{username} was sent {requested}.'**
  String pendingBody({required String username, required String requested});

  /// Waiting-for-approval explanation.
  ///
  /// In en, this message translates to:
  /// **'We\'ll let you sign in as soon as an administrator approves your account.'**
  String get pendingNext;

  /// Retries the sign-in to see if the account was approved.
  ///
  /// In en, this message translates to:
  /// **'Check again'**
  String get checkAgain;

  /// Returns to the sign-in screen.
  ///
  /// In en, this message translates to:
  /// **'Use a different account'**
  String get useAnotherAccount;

  /// Rejected variant title.
  ///
  /// In en, this message translates to:
  /// **'This account request wasn\'t approved.'**
  String get rejectedTitle;

  /// Rejected variant body.
  ///
  /// In en, this message translates to:
  /// **'Contact your administrator if this looks wrong. Nothing was stored on this device.'**
  String get rejectedBody;

  /// Account role admin.
  ///
  /// In en, this message translates to:
  /// **'Admin'**
  String get roleAdmin;

  /// Account role member.
  ///
  /// In en, this message translates to:
  /// **'Member'**
  String get roleMember;

  /// Accessibility label of the account sheet.
  ///
  /// In en, this message translates to:
  /// **'Account'**
  String get accountSheetLabel;

  /// A username as shown under a name.
  ///
  /// In en, this message translates to:
  /// **'@{username}'**
  String atUsername({required String username});

  /// Label of the current device.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDevice;

  /// Opens the devices settings.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get devices;

  /// Opens the admin user list.
  ///
  /// In en, this message translates to:
  /// **'Manage users'**
  String get adminUsers;

  /// Sign-out action.
  ///
  /// In en, this message translates to:
  /// **'Sign out'**
  String get signOut;

  /// Sign-out warning title.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 change hasn\'t synced yet} other{{count} changes haven\'t synced yet}}'**
  String unsyncedTitle({required int count});

  /// Sign-out warning body.
  ///
  /// In en, this message translates to:
  /// **'Signing out removes this account\'s data from this device. Sync first to keep them.'**
  String get unsyncedBody;

  /// Sync first, then sign out later.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get syncNow;

  /// Signs out, discarding unsynced changes.
  ///
  /// In en, this message translates to:
  /// **'Sign out anyway'**
  String get signOutAnyway;

  /// Cancels.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// Account disabled title.
  ///
  /// In en, this message translates to:
  /// **'This account was disabled'**
  String get disabledTitle;

  /// Account disabled explanation.
  ///
  /// In en, this message translates to:
  /// **'An administrator disabled @{username}. Local data on this device will be removed.'**
  String disabledBody({required String username});

  /// Account disabled reassurance.
  ///
  /// In en, this message translates to:
  /// **'Notes that already synced stay with the account on the server.'**
  String get disabledServerNote;

  /// Unsynced changes heading.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No unsynced changes} =1{1 unsynced change} other{{count} unsynced changes}}'**
  String unsyncedCount({required int count});

  /// Tag next to unsynced changes.
  ///
  /// In en, this message translates to:
  /// **'only on this device'**
  String get onlyOnDevice;

  /// Exports unsynced changes before removal.
  ///
  /// In en, this message translates to:
  /// **'Export them first'**
  String get exportFirst;

  /// Explains the export.
  ///
  /// In en, this message translates to:
  /// **'Export saves them as markdown files you can keep.'**
  String get exportFirstNote;

  /// Acknowledges the disabled account.
  ///
  /// In en, this message translates to:
  /// **'Remove and sign out'**
  String get removeAndSignOut;

  /// Deletion pending title.
  ///
  /// In en, this message translates to:
  /// **'Your account is being deleted'**
  String get deletionTitle;

  /// Deletion countdown pill.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Deleted today} =1{1 day left} other{{count} days left}}'**
  String daysLeft({required int count});

  /// Deletion pending body with the core's date label.
  ///
  /// In en, this message translates to:
  /// **'An administrator scheduled @{username} for deletion on {date}. Download your notes before then.'**
  String deletionBody({required String username, required String date});

  /// Downloads the account export.
  ///
  /// In en, this message translates to:
  /// **'Download export (.zip)'**
  String get downloadExport;

  /// Deletion pending: unsynced ops heading.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 change on this device wasn\'t synced} other{{count} changes on this device weren\'t synced}}'**
  String unsyncedNotInExport({required int count});

  /// Deletion pending: unsynced ops are not exported.
  ///
  /// In en, this message translates to:
  /// **'They aren\'t in the export.'**
  String get notInExport;

  /// Saves unsynced ops as a file.
  ///
  /// In en, this message translates to:
  /// **'Save them as a file'**
  String get saveAsFile;

  /// Deletes the account immediately.
  ///
  /// In en, this message translates to:
  /// **'Delete now'**
  String get deleteNow;

  /// Deletion pending: read-only notice.
  ///
  /// In en, this message translates to:
  /// **'Your notes are read-only until then.'**
  String get readOnlyNote;

  /// Password change required title.
  ///
  /// In en, this message translates to:
  /// **'Choose a new password'**
  String get passwordChangeTitle;

  /// Password change required explanation.
  ///
  /// In en, this message translates to:
  /// **'An administrator reset your password. Choose a new one to continue.'**
  String get passwordChangeBody;

  /// Submits a password change.
  ///
  /// In en, this message translates to:
  /// **'Change password'**
  String get changePassword;

  /// Core error invalid_credentials.
  ///
  /// In en, this message translates to:
  /// **'Username or password is incorrect'**
  String get errorInvalidCredentials;

  /// Core error offline.
  ///
  /// In en, this message translates to:
  /// **'Can\'t reach the server. Check the address and your connection.'**
  String get errorOffline;

  /// Core error account_pending.
  ///
  /// In en, this message translates to:
  /// **'This account is waiting for approval.'**
  String get errorAccountPending;

  /// Core error account_rejected.
  ///
  /// In en, this message translates to:
  /// **'This account request wasn\'t approved.'**
  String get errorAccountRejected;

  /// Core error account_disabled.
  ///
  /// In en, this message translates to:
  /// **'This account was disabled by an administrator.'**
  String get errorAccountDisabled;

  /// Core error account_deletion_pending.
  ///
  /// In en, this message translates to:
  /// **'This account is scheduled for deletion.'**
  String get errorAccountDeletion;

  /// Core error session_expired.
  ///
  /// In en, this message translates to:
  /// **'Your session ended. Sign in again.'**
  String get errorSessionExpired;

  /// Core error rate_limited.
  ///
  /// In en, this message translates to:
  /// **'Too many attempts. Wait a minute and try again.'**
  String get errorRateLimited;

  /// Core error invalid_input.
  ///
  /// In en, this message translates to:
  /// **'Check the {field} field.'**
  String errorInvalidInput({required String field});

  /// Core error server.
  ///
  /// In en, this message translates to:
  /// **'The server answered with an error ({status}).'**
  String errorServer({required String status});

  /// Core error not_available.
  ///
  /// In en, this message translates to:
  /// **'This isn\'t available yet.'**
  String get errorNotAvailable;

  /// Any other core error.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong ({code}).'**
  String errorGeneric({required String code});

  /// An unsynced op: its kind and when it was queued.
  ///
  /// In en, this message translates to:
  /// **'{kind} · {time}'**
  String unsyncedItem({required String kind, required String time});

  /// Password strength: weak.
  ///
  /// In en, this message translates to:
  /// **'Weak'**
  String get strengthWeak;

  /// Password strength: fair.
  ///
  /// In en, this message translates to:
  /// **'Fair'**
  String get strengthFair;

  /// Password strength: strong.
  ///
  /// In en, this message translates to:
  /// **'Strong'**
  String get strengthStrong;

  /// Password length against the minimum, after the level.
  ///
  /// In en, this message translates to:
  /// **'· {length, plural, =1{1 character} other{{length} characters}} · at least {min}'**
  String strengthDetail({required int length, required int min});

  /// This device row in the account sheet.
  ///
  /// In en, this message translates to:
  /// **'{name} · signed in {signedIn}'**
  String thisDeviceDetail({required String name, required String signedIn});

  /// Pending approvals next to Manage users.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{none pending} =1{1 pending} other{{count} pending}}'**
  String pendingCount({required int count});

  /// After an export download.
  ///
  /// In en, this message translates to:
  /// **'Saved {label}'**
  String exportSaved({required String label});

  /// After exporting unsynced changes.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 change saved} other{{count} changes saved}}'**
  String unsyncedSaved({required int count});

  /// Delete now confirmation title.
  ///
  /// In en, this message translates to:
  /// **'Delete @{username} now?'**
  String deleteNowTitle({required String username});

  /// Delete now confirmation body.
  ///
  /// In en, this message translates to:
  /// **'The account and every note on the server are deleted now. This can\'t be undone.'**
  String get deleteNowBody;

  /// Delete now while changes are unsynced.
  ///
  /// In en, this message translates to:
  /// **'Delete anyway'**
  String get deleteAnyway;

  /// The one-time password field.
  ///
  /// In en, this message translates to:
  /// **'Temporary password'**
  String get fieldTemporaryPassword;

  /// After changing the password.
  ///
  /// In en, this message translates to:
  /// **'Password changed'**
  String get passwordChanged;

  /// Check again: still pending.
  ///
  /// In en, this message translates to:
  /// **'Still waiting for approval.'**
  String get stillPending;

  /// Password strength: too short.
  ///
  /// In en, this message translates to:
  /// **'Too short'**
  String get strengthTooShort;
}

class _AccountsLocalizationsDelegate
    extends LocalizationsDelegate<AccountsLocalizations> {
  const _AccountsLocalizationsDelegate();

  @override
  Future<AccountsLocalizations> load(Locale locale) {
    return SynchronousFuture<AccountsLocalizations>(
      lookupAccountsLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AccountsLocalizationsDelegate old) => false;
}

AccountsLocalizations lookupAccountsLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return AccountsLocalizationsAr();
    case 'en':
      return AccountsLocalizationsEn();
  }

  throw FlutterError(
    'AccountsLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
