import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

void main() {
  setUpAll(loadStrataFonts);

  goldens(
    'admin_users',
    (v) => goldenFrame(
      v,
      const AdminUsersScreen(),
      fake: FakeCoreApi()..loadAdminUsersAnswer.returns(AdminFixtures.view),
    ),
  );
  goldens(
    'admin_users_offline',
    (v) => goldenFrame(
      v,
      const AdminUsersScreen(),
      fake: FakeCoreApi()
        ..loadAdminUsersAnswer.returns(
          AdminFixtures.unavailable(Availability.offline),
        ),
    ),
  );
  goldens(
    'admin_dialogs',
    (v) => goldenFrame(
      v,
      Scaffold(
        body: Center(
          child: OneTimePasswordDialog(
            user: AdminFixtures.view.users[1],
            password: 'tide-4821-sand',
          ),
        ),
      ),
    ),
  );
}
