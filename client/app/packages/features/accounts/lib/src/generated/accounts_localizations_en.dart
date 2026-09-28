// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'accounts_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AccountsLocalizationsEn extends AccountsLocalizations {
  AccountsLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get signInTitle => 'Sign in';

  @override
  String get signInSubtitle =>
      'Your notes stay on your server and on this device.';

  @override
  String get signInSubtitleWide =>
      'One account per device. Your notes are private to you.';

  @override
  String get brandKicker => 'SELF-HOSTED KNOWLEDGE';

  @override
  String get brandTagline =>
      'Captures filed, notes linked, people and companies kept current. In Arabic and English, online or off.';

  @override
  String get fieldUsername => 'Username';

  @override
  String get fieldPassword => 'Password';

  @override
  String get fieldConfirmPassword => 'Confirm password';

  @override
  String get fieldNewPassword => 'New password';

  @override
  String get fieldCurrentPassword => 'Current password';

  @override
  String get fieldDeviceName => 'Device name';

  @override
  String get fieldDisplayName => 'Display name';

  @override
  String get deviceNameHint =>
      'Shown in your list of signed-in devices. You can rename it later.';

  @override
  String get showPassword => 'Show password';

  @override
  String get hidePassword => 'Hide password';

  @override
  String get enterToSignIn => 'to sign in';

  @override
  String get newHere => 'New here?';

  @override
  String get createAccount => 'Create an account';

  @override
  String get forgotPassword => 'Forgot it? Ask your admin to reset it.';

  @override
  String get knownAccountsTitle => 'Accounts on this device';

  @override
  String continueAs({required String name}) {
    return 'Continue as $name';
  }

  @override
  String get approvalNotice =>
      'An administrator must approve new accounts. You\'ll be able to sign in once approved.';

  @override
  String get requestAccount => 'Request account';

  @override
  String get haveAccount => 'Already have an account?';

  @override
  String get backToSignIn => 'Back to sign in';

  @override
  String get passwordsMatch => 'Passwords match';

  @override
  String get passwordsDiffer => 'Passwords don\'t match';

  @override
  String get pendingPill => 'Pending';

  @override
  String get pendingTitle => 'Waiting for approval';

  @override
  String pendingBody({required String username, required String requested}) {
    return 'Your request for @$username was sent $requested.';
  }

  @override
  String get pendingNext =>
      'We\'ll let you sign in as soon as an administrator approves your account.';

  @override
  String get checkAgain => 'Check again';

  @override
  String get useAnotherAccount => 'Use a different account';

  @override
  String get rejectedTitle => 'This account request wasn\'t approved.';

  @override
  String get rejectedBody =>
      'Contact your administrator if this looks wrong. Nothing was stored on this device.';

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleMember => 'Member';

  @override
  String get accountSheetLabel => 'Account';

  @override
  String atUsername({required String username}) {
    return '@$username';
  }

  @override
  String get thisDevice => 'This device';

  @override
  String get devices => 'Devices';

  @override
  String get adminUsers => 'Manage users';

  @override
  String get signOut => 'Sign out';

  @override
  String unsyncedTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes haven\'t synced yet',
      one: '1 change hasn\'t synced yet',
    );
    return '$_temp0';
  }

  @override
  String get unsyncedBody =>
      'Signing out removes this account\'s data from this device. Sync first to keep them.';

  @override
  String get syncNow => 'Sync now';

  @override
  String get signOutAnyway => 'Sign out anyway';

  @override
  String get cancel => 'Cancel';

  @override
  String get disabledTitle => 'This account was disabled';

  @override
  String disabledBody({required String username}) {
    return 'An administrator disabled @$username. Local data on this device will be removed.';
  }

  @override
  String get disabledServerNote =>
      'Notes that already synced stay with the account on the server.';

  @override
  String unsyncedCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count unsynced changes',
      one: '1 unsynced change',
      zero: 'No unsynced changes',
    );
    return '$_temp0';
  }

  @override
  String get onlyOnDevice => 'only on this device';

  @override
  String get exportFirst => 'Export them first';

  @override
  String get exportFirstNote =>
      'Export saves them as markdown files you can keep.';

  @override
  String get removeAndSignOut => 'Remove and sign out';

  @override
  String get deletionTitle => 'Your account is being deleted';

  @override
  String daysLeft({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count days left',
      one: '1 day left',
      zero: 'Deleted today',
    );
    return '$_temp0';
  }

  @override
  String deletionBody({required String username, required String date}) {
    return 'An administrator scheduled @$username for deletion on $date. Download your notes before then.';
  }

  @override
  String get downloadExport => 'Download export (.zip)';

  @override
  String unsyncedNotInExport({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes on this device weren\'t synced',
      one: '1 change on this device wasn\'t synced',
    );
    return '$_temp0';
  }

  @override
  String get notInExport => 'They aren\'t in the export.';

  @override
  String get saveAsFile => 'Save them as a file';

  @override
  String get deleteNow => 'Delete now';

  @override
  String get readOnlyNote => 'Your notes are read-only until then.';

  @override
  String get passwordChangeTitle => 'Choose a new password';

  @override
  String get passwordChangeBody =>
      'An administrator reset your password. Choose a new one to continue.';

  @override
  String get changePassword => 'Change password';

  @override
  String get errorInvalidCredentials => 'Username or password is incorrect';

  @override
  String get errorAccountPending => 'This account is waiting for approval.';

  @override
  String get errorAccountRejected => 'This account request wasn\'t approved.';

  @override
  String get errorAccountDisabled =>
      'This account was disabled by an administrator.';

  @override
  String get errorAccountDeletion => 'This account is scheduled for deletion.';

  @override
  String get errorSessionExpired => 'Your session ended. Sign in again.';

  @override
  String get errorRateLimited =>
      'Too many attempts. Wait a minute and try again.';

  @override
  String errorInvalidInput({required String field}) {
    return 'Check the $field field.';
  }

  @override
  String errorServer({required String status}) {
    return 'The server answered with an error ($status).';
  }

  @override
  String get errorNotAvailable => 'This isn\'t available yet.';

  @override
  String get errorUnreachable =>
      'Couldn\'t reach the server. Check your connection and try again.';

  @override
  String errorGeneric({required String code}) {
    return 'Something went wrong ($code).';
  }

  @override
  String unsyncedItem({required String kind, required String time}) {
    return '$kind · $time';
  }

  @override
  String get strengthWeak => 'Weak';

  @override
  String get strengthFair => 'Fair';

  @override
  String get strengthStrong => 'Strong';

  @override
  String strengthDetail({required int length, required int min}) {
    String _temp0 = intl.Intl.pluralLogic(
      length,
      locale: localeName,
      other: '$length characters',
      one: '1 character',
    );
    return '· $_temp0 · at least $min';
  }

  @override
  String thisDeviceDetail({required String name, required String signedIn}) {
    return '$name · signed in $signedIn';
  }

  @override
  String pendingCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pending',
      one: '1 pending',
      zero: 'none pending',
    );
    return '$_temp0';
  }

  @override
  String exportSaved({required String label}) {
    return 'Saved $label';
  }

  @override
  String unsyncedSaved({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes saved',
      one: '1 change saved',
    );
    return '$_temp0';
  }

  @override
  String deleteNowTitle({required String username}) {
    return 'Delete @$username now?';
  }

  @override
  String get deleteNowBody =>
      'The account and every note on the server are deleted now. This can\'t be undone.';

  @override
  String get deleteAnyway => 'Delete anyway';

  @override
  String get fieldTemporaryPassword => 'Temporary password';

  @override
  String get passwordChanged => 'Password changed';

  @override
  String get stillPending => 'Still waiting for approval.';

  @override
  String get strengthTooShort => 'Too short';
}
