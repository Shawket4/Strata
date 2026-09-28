import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

FakeCoreApi _fake(SettingsView view) => FakeCoreApi()
  ..session.add(StrataFixtures.sessionActive)
  ..settings.add(view)
  ..syncStatus.add(StrataFixtures.syncStatusView);

void main() {
  setUpAll(loadStrataFonts);

  screenGoldens(
    'settings',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen()),
      fake: _fake(SettingsFixtures.full),
    ),
  );
  screenGoldens(
    'settings_reminders',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.reminders)),
      fake: _fake(SettingsFixtures.denied),
    ),
  );
  screenGoldens(
    'settings_ai',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.ai)),
      fake: _fake(SettingsFixtures.full),
    ),
  );
  screenGoldens(
    'settings_devices',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.devices)),
      fake: _fake(SettingsFixtures.full),
    ),
  );
  screenGoldens(
    'settings_integrity',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SettingsScreen(section: SettingsSection.integrity)),
      fake: _fake(SettingsFixtures.full),
    ),
  );
}
