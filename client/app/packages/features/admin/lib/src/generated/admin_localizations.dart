import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'admin_localizations_ar.dart';
import 'admin_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AdminLocalizations
/// returned by `AdminLocalizations.of(context)`.
///
/// Applications need to include `AdminLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/admin_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AdminLocalizations.localizationsDelegates,
///   supportedLocales: AdminLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the AdminLocalizations.supportedLocales
/// property.
abstract class AdminLocalizations {
  AdminLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AdminLocalizations of(BuildContext context) {
    return Localizations.of<AdminLocalizations>(context, AdminLocalizations)!;
  }

  static const LocalizationsDelegate<AdminLocalizations> delegate =
      _AdminLocalizationsDelegate();

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

  /// Admin → Users title.
  ///
  /// In en, this message translates to:
  /// **'Users'**
  String get usersTitle;

  /// Breadcrumb over the users title.
  ///
  /// In en, this message translates to:
  /// **'Settings / Admin'**
  String get breadcrumb;

  /// Privacy note under the title.
  ///
  /// In en, this message translates to:
  /// **'Admins manage accounts. Nobody else can see a user\'s notes.'**
  String get intro;

  /// Creates an account.
  ///
  /// In en, this message translates to:
  /// **'Create account'**
  String get createAccount;

  /// Pending approvals heading.
  ///
  /// In en, this message translates to:
  /// **'Pending approval'**
  String get pendingTitle;

  /// Empty pending queue.
  ///
  /// In en, this message translates to:
  /// **'No account is waiting for approval.'**
  String get pendingEmpty;

  /// Users list heading.
  ///
  /// In en, this message translates to:
  /// **'All users'**
  String get allUsers;

  /// A pending account's subtitle.
  ///
  /// In en, this message translates to:
  /// **'@{username} · requested {date}'**
  String requested({required String username, required String date});

  /// A user row's subtitle.
  ///
  /// In en, this message translates to:
  /// **'@{username} · {role}'**
  String userSubtitle({required String username, required String role});

  /// Approves a pending account.
  ///
  /// In en, this message translates to:
  /// **'Approve'**
  String get approve;

  /// Rejects a pending account.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get reject;

  /// Accessibility label of Approve.
  ///
  /// In en, this message translates to:
  /// **'Approve {name}'**
  String approveSemantics({required String name});

  /// Accessibility label of Reject.
  ///
  /// In en, this message translates to:
  /// **'Reject {name}'**
  String rejectSemantics({required String name});

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

  /// Account status active.
  ///
  /// In en, this message translates to:
  /// **'Active'**
  String get statusActive;

  /// Account status disabled.
  ///
  /// In en, this message translates to:
  /// **'Disabled'**
  String get statusDisabled;

  /// Account status pending.
  ///
  /// In en, this message translates to:
  /// **'Pending'**
  String get statusPending;

  /// Account status rejected.
  ///
  /// In en, this message translates to:
  /// **'Rejected'**
  String get statusRejected;

  /// Account status deletion pending with the purge date.
  ///
  /// In en, this message translates to:
  /// **'Deletion {date}'**
  String statusDeletion({required String date});

  /// Account status deletion pending without a date.
  ///
  /// In en, this message translates to:
  /// **'Deletion scheduled'**
  String get statusDeletionNoDate;

  /// An account status this app version does not name.
  ///
  /// In en, this message translates to:
  /// **'{status}'**
  String statusOther({required String status});

  /// The user has not downloaded their export.
  ///
  /// In en, this message translates to:
  /// **'Export not downloaded yet'**
  String get exportNotDownloaded;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get columnName;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get columnUsername;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Role'**
  String get columnRole;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get columnStatus;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Created'**
  String get columnCreated;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Actions'**
  String get columnActions;

  /// Disables an account.
  ///
  /// In en, this message translates to:
  /// **'Disable'**
  String get disable;

  /// Enables an account.
  ///
  /// In en, this message translates to:
  /// **'Enable'**
  String get enable;

  /// Resets an account's password.
  ///
  /// In en, this message translates to:
  /// **'Reset password'**
  String get resetPassword;

  /// Schedules an account's deletion.
  ///
  /// In en, this message translates to:
  /// **'Schedule deletion'**
  String get scheduleDeletion;

  /// Cancels a scheduled deletion.
  ///
  /// In en, this message translates to:
  /// **'Cancel deletion'**
  String get cancelDeletion;

  /// Accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Disable {name}'**
  String disableSemantics({required String name});

  /// Accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Enable {name}'**
  String enableSemantics({required String name});

  /// Accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Reset password for {name}'**
  String resetSemantics({required String name});

  /// Accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Delete {name}…'**
  String deleteSemantics({required String name});

  /// Accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Cancel deletion of {name}'**
  String cancelDeletionSemantics({required String name});

  /// Title of a user's action sheet.
  ///
  /// In en, this message translates to:
  /// **'Actions for {name}'**
  String userActions({required String name});

  /// Footnote under the table.
  ///
  /// In en, this message translates to:
  /// **'Disable signs a user out everywhere and removes their data from their devices.'**
  String get footnoteDisable;

  /// Footnote under the table.
  ///
  /// In en, this message translates to:
  /// **'Reset password gives you a one-time password to pass on.'**
  String get footnoteReset;

  /// Schedule deletion confirmation title.
  ///
  /// In en, this message translates to:
  /// **'Schedule deletion of @{username}?'**
  String scheduleTitle({required String username});

  /// Schedule deletion confirmation body (the purge date is known only once scheduled).
  ///
  /// In en, this message translates to:
  /// **'{name}\'s account is signed out everywhere and deleted when the grace period ends. Until then {name} can sign in only to download an export of their notes. You won\'t see the export.'**
  String scheduleBody({required String name});

  /// Schedule deletion confirmation body with the purge date the core computed ("Deleted on 11 Oct 2026").
  ///
  /// In en, this message translates to:
  /// **'{name}\'s account is signed out everywhere. {date}. Until then {name} can sign in only to download an export of their notes. You won\'t see the export.'**
  String scheduleBodyDated({required String name, required String date});

  /// Cancels a dialog.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// One-time password dialog title.
  ///
  /// In en, this message translates to:
  /// **'One-time password for @{username}'**
  String oneTimeTitle({required String username});

  /// One-time password dialog body.
  ///
  /// In en, this message translates to:
  /// **'Pass it on privately. It\'s shown only once; {name} must choose a new password after signing in.'**
  String oneTimeBody({required String name});

  /// Copies the one-time password.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copy;

  /// Snackbar after copying.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get copied;

  /// Closes the one-time password dialog.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// Offline state title.
  ///
  /// In en, this message translates to:
  /// **'Admin → Users needs a connection'**
  String get offlineTitle;

  /// Offline state body.
  ///
  /// In en, this message translates to:
  /// **'Managing accounts happens on the server. Connect and try again.'**
  String get offlineBody;

  /// Not-yet-available state title.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get notYetTitle;

  /// Not-yet-available state body.
  ///
  /// In en, this message translates to:
  /// **'This server doesn\'t offer account management yet.'**
  String get notYetBody;

  /// Not-allowed state title.
  ///
  /// In en, this message translates to:
  /// **'Admins only'**
  String get notAllowedTitle;

  /// Not-allowed state body.
  ///
  /// In en, this message translates to:
  /// **'Only administrators can manage users.'**
  String get notAllowedBody;

  /// Error state title.
  ///
  /// In en, this message translates to:
  /// **'Users could not be loaded'**
  String get loadFailed;

  /// Reloads.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;

  /// Loading label.
  ///
  /// In en, this message translates to:
  /// **'Loading users'**
  String get loading;

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

  /// A username.
  ///
  /// In en, this message translates to:
  /// **'@{username}'**
  String atUsername({required String username});

  /// Badge next to the signed-in admin's own row.
  ///
  /// In en, this message translates to:
  /// **'you'**
  String get you;

  /// Actions cell of the admin's own row.
  ///
  /// In en, this message translates to:
  /// **'Your account'**
  String get yourAccount;

  /// Search field label.
  ///
  /// In en, this message translates to:
  /// **'Search users'**
  String get searchLabel;

  /// Empty search result.
  ///
  /// In en, this message translates to:
  /// **'No user matches “{query}”.'**
  String noMatches({required String query});

  /// Role picker label.
  ///
  /// In en, this message translates to:
  /// **'Role of {name}'**
  String roleSemantics({required String name});

  /// Compact action: set role to admin.
  ///
  /// In en, this message translates to:
  /// **'Make admin'**
  String get makeAdmin;

  /// Compact action: set role to member.
  ///
  /// In en, this message translates to:
  /// **'Make member'**
  String get makeMember;

  /// Status note after a reset.
  ///
  /// In en, this message translates to:
  /// **'Must choose a new password'**
  String get passwordChangePending;

  /// Create account dialog title.
  ///
  /// In en, this message translates to:
  /// **'Create account'**
  String get createTitle;

  /// Create account dialog body.
  ///
  /// In en, this message translates to:
  /// **'The account is active right away. Pass the password on privately; they choose a new one after signing in.'**
  String get createBody;

  /// Create account field.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get fieldUsername;

  /// Create account field.
  ///
  /// In en, this message translates to:
  /// **'Display name'**
  String get fieldDisplayName;

  /// Create account field.
  ///
  /// In en, this message translates to:
  /// **'Temporary password'**
  String get fieldPassword;

  /// Create account field.
  ///
  /// In en, this message translates to:
  /// **'Role'**
  String get fieldRole;

  /// Create account confirm.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get create;

  /// Snack bar after creating an account.
  ///
  /// In en, this message translates to:
  /// **'Account @{username} created'**
  String created({required String username});

  /// Snack bar after approving.
  ///
  /// In en, this message translates to:
  /// **'{name} approved'**
  String approved({required String name});

  /// Snack bar after rejecting.
  ///
  /// In en, this message translates to:
  /// **'{name} rejected'**
  String rejected({required String name});
}

class _AdminLocalizationsDelegate
    extends LocalizationsDelegate<AdminLocalizations> {
  const _AdminLocalizationsDelegate();

  @override
  Future<AdminLocalizations> load(Locale locale) {
    return SynchronousFuture<AdminLocalizations>(
      lookupAdminLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AdminLocalizationsDelegate old) => false;
}

AdminLocalizations lookupAdminLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return AdminLocalizationsAr();
    case 'en':
      return AdminLocalizationsEn();
  }

  throw FlutterError(
    'AdminLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
