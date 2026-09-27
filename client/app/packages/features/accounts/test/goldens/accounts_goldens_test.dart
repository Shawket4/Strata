import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/hosts.dart';
import '../helpers/matrix.dart';

void main() {
  setUpAll(loadStrataFonts);

  goldens(
    'sign_in',
    (v) => goldenFrame(
      v,
      const SignInScreen(),
      fake: FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut),
    ),
  );
  goldens(
    'sign_up',
    (v) => goldenFrame(
      v,
      SignUpScreen(serverUrl: StrataFixtures.serverUrl, onBack: () {}),
      fake: FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut),
    ),
  );
  goldens(
    'pending_approval',
    (v) => goldenFrame(
      v,
      const PendingApprovalScreen(request: StrataFixtures.signInRequest),
    ),
  );
  goldens(
    'not_approved',
    (v) => goldenFrame(
      v,
      const PendingApprovalScreen(
        request: StrataFixtures.signInRequest,
        rejected: true,
      ),
    ),
  );
  goldens(
    'account_sheet',
    (v) => goldenFrame(
      v,
      AccountSheetPreview(sizeClass: v.sizeClass),
      fake: FakeCoreApi()
        ..session.add(StrataFixtures.sessionActive)
        ..settings.add(StrataFixtures.settingsView),
    ),
  );
  goldens(
    'sign_out_warning',
    (v) => goldenFrame(
      v,
      const Scaffold(body: Center(child: SignOutWarningDialog(unsyncedOps: 2))),
      fake: FakeCoreApi()..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  goldens(
    'account_disabled',
    (v) => goldenFrame(
      v,
      const AccountDisabledScreen(),
      fake: FakeCoreApi()
        ..session.add(AccountFixtures.disabled)
        ..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  goldens(
    'deletion_pending',
    (v) => goldenFrame(
      v,
      const DeletionPendingScreen(),
      fake: FakeCoreApi()
        ..session.add(StrataFixtures.sessionDeletionPending)
        ..syncStatus.add(AccountFixtures.unsynced),
    ),
  );
  goldens(
    'password_change',
    (v) => goldenFrame(
      v,
      const PasswordChangeRequiredScreen(),
      fake: FakeCoreApi()..session.add(AccountFixtures.passwordChange),
    ),
  );
}
