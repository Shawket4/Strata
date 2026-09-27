import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

FakeCoreApi _fake(SettingsView view) => FakeCoreApi()
  ..session.add(StrataFixtures.sessionActive)
  ..settings.add(view)
  ..syncStatus.add(StrataFixtures.syncStatusView);

void main() {
  setUpAll(loadStrataFonts);

  goldens(
    'settings',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen()),
      fake: _fake(StrataFixtures.settingsView),
    ),
  );
  goldens(
    'settings_reminders',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.reminders)),
      fake: _fake(SettingsFixtures.denied),
    ),
  );
  goldens(
    'settings_ai',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.ai)),
      fake: _fake(StrataFixtures.settingsView),
    ),
  );
  goldens(
    'settings_devices',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.devices)),
      fake: _fake(SettingsFixtures.member),
    ),
  );
}
