// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'admin_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AdminLocalizationsEn extends AdminLocalizations {
  AdminLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get usersTitle => 'Users';

  @override
  String get breadcrumb => 'Settings / Admin';

  @override
  String get intro =>
      'Admins manage accounts. Nobody else can see a user\'s notes.';

  @override
  String get createAccount => 'Create account';

  @override
  String get pendingTitle => 'Pending approval';

  @override
  String get pendingEmpty => 'No account is waiting for approval.';

  @override
  String get allUsers => 'All users';

  @override
  String requested({required String username, required String date}) {
    return '@$username · requested $date';
  }

  @override
  String userSubtitle({required String username, required String role}) {
    return '@$username · $role';
  }

  @override
  String get approve => 'Approve';

  @override
  String get reject => 'Reject';

  @override
  String approveSemantics({required String name}) {
    return 'Approve $name';
  }

  @override
  String rejectSemantics({required String name}) {
    return 'Reject $name';
  }

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleMember => 'Member';

  @override
  String get statusActive => 'Active';

  @override
  String get statusDisabled => 'Disabled';

  @override
  String get statusPending => 'Pending';

  @override
  String get statusRejected => 'Rejected';

  @override
  String statusDeletion({required String date}) {
    return 'Deletion $date';
  }

  @override
  String get statusDeletionNoDate => 'Deletion scheduled';

  @override
  String statusOther({required String status}) {
    return '$status';
  }

  @override
  String get exportNotDownloaded => 'Export not downloaded yet';

  @override
  String exportDownloaded({required String date}) {
    return 'Export downloaded $date';
  }

  @override
  String get columnName => 'Name';

  @override
  String get columnUsername => 'Username';

  @override
  String get columnRole => 'Role';

  @override
  String get columnStatus => 'Status';

  @override
  String get columnCreated => 'Created';

  @override
  String get columnActions => 'Actions';

  @override
  String get disable => 'Disable';

  @override
  String get enable => 'Enable';

  @override
  String get resetPassword => 'Reset password';

  @override
  String get scheduleDeletion => 'Schedule deletion';

  @override
  String get cancelDeletion => 'Cancel deletion';

  @override
  String disableSemantics({required String name}) {
    return 'Disable $name';
  }

  @override
  String enableSemantics({required String name}) {
    return 'Enable $name';
  }

  @override
  String resetSemantics({required String name}) {
    return 'Reset password for $name';
  }

  @override
  String deleteSemantics({required String name}) {
    return 'Delete $name…';
  }

  @override
  String cancelDeletionSemantics({required String name}) {
    return 'Cancel deletion of $name';
  }

  @override
  String userActions({required String name}) {
    return 'Actions for $name';
  }

  @override
  String get footnoteDisable =>
      'Disable signs a user out everywhere and removes their data from their devices.';

  @override
  String get footnoteReset =>
      'Reset password gives you a one-time password to pass on.';

  @override
  String scheduleTitle({required String username}) {
    return 'Schedule deletion of @$username?';
  }

  @override
  String scheduleBody({required String name}) {
    return '$name\'s account is signed out everywhere and deleted when the grace period ends. Until then $name can sign in only to download an export of their notes. You won\'t see the export.';
  }

  @override
  String get cancel => 'Cancel';

  @override
  String oneTimeTitle({required String username}) {
    return 'One-time password for @$username';
  }

  @override
  String oneTimeBody({required String name}) {
    return 'Pass it on privately. It\'s shown only once; $name must choose a new password after signing in.';
  }

  @override
  String get copy => 'Copy';

  @override
  String get copied => 'Copied';

  @override
  String get done => 'Done';

  @override
  String get offlineTitle => 'Admin → Users needs a connection';

  @override
  String get offlineBody =>
      'Managing accounts happens on the server. Connect and try again.';

  @override
  String get notYetTitle => 'Not available yet';

  @override
  String get notYetBody => 'This server doesn\'t offer account management yet.';

  @override
  String get notAllowedTitle => 'Admins only';

  @override
  String get notAllowedBody => 'Only administrators can manage users.';

  @override
  String get loadFailed => 'Users could not be loaded';

  @override
  String get retry => 'Retry';

  @override
  String get loading => 'Loading users';

  @override
  String errorGeneric({required String code}) {
    return 'Something went wrong ($code).';
  }

  @override
  String get errorOffline => 'You\'re offline.';

  @override
  String atUsername({required String username}) {
    return '@$username';
  }

  @override
  String get you => 'you';

  @override
  String get yourAccount => 'Your account';

  @override
  String get searchLabel => 'Search users';

  @override
  String noMatches({required String query}) {
    return 'No user matches “$query”.';
  }

  @override
  String roleSemantics({required String name}) {
    return 'Role of $name';
  }

  @override
  String get makeAdmin => 'Make admin';

  @override
  String get makeMember => 'Make member';

  @override
  String get passwordChangePending => 'Must choose a new password';

  @override
  String get createTitle => 'Create account';

  @override
  String get createBody =>
      'The account is active right away. Pass the password on privately; they choose a new one after signing in.';

  @override
  String get fieldUsername => 'Username';

  @override
  String get fieldDisplayName => 'Display name';

  @override
  String get fieldPassword => 'Temporary password';

  @override
  String get fieldRole => 'Role';

  @override
  String get create => 'Create';

  @override
  String created({required String username}) {
    return 'Account @$username created';
  }

  @override
  String approved({required String name}) {
    return '$name approved';
  }

  @override
  String rejected({required String name}) {
    return '$name rejected';
  }
}
