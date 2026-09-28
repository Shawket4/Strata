import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/hosts.dart';

void main() {
  setUpAll(loadStrataFonts);

  screenGoldens(
    'sign_in',
    (v) => goldenFrame(
      v,
      const SignInScreen(),
      fake: FakeCoreApi()..session.add(AccountFixtures.signedOut),
    ),
  );
  screenGoldens(
    'sign_up',
    (v) => goldenFrame(
      v,
      SignUpScreen(serverUrl: StrataFixtures.serverUrl, onBack: () {}),
      fake: FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut),
    ),
  );
  screenGoldens(
    'pending_approval',
    (v) => goldenFrame(
      v,
      const PendingApprovalScreen(),
      fake: FakeCoreApi()..session.add(AccountFixtures.waiting),
    ),
  );
  screenGoldens(
    'not_approved',
    (v) => goldenFrame(
      v,
      const PendingApprovalScreen(),
      fake: FakeCoreApi()..session.add(AccountFixtures.notApproved),
    ),
  );
  screenGoldens(
    'account_sheet',
    (v) => goldenFrame(
      v,
      AccountSheetPreview(sizeClass: v.sizeClass),
      fake: FakeCoreApi()
        ..session.add(AccountFixtures.active)
        ..settings.add(StrataFixtures.settingsView),
    ),
  );
  screenGoldens(
    'sign_out_warning',
    (v) => goldenFrame(
      v,
      const Scaffold(body: Center(child: SignOutWarningDialog(unsyncedOps: 2))),
      fake: FakeCoreApi()..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  screenGoldens(
    'account_disabled',
    (v) => goldenFrame(
      v,
      const AccountDisabledScreen(),
      fake: FakeCoreApi()
        ..session.add(AccountFixtures.disabled)
        ..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  screenGoldens(
    'deletion_pending',
    (v) => goldenFrame(
      v,
      const DeletionPendingScreen(),
      fake: FakeCoreApi()
        ..session.add(AccountFixtures.deletionPending)
        ..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  screenGoldens(
    'password_change',
    (v) => goldenFrame(
      v,
      const PasswordChangeRequiredScreen(),
      fake: FakeCoreApi()..session.add(AccountFixtures.passwordChange),
    ),
  );
}
