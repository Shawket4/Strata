import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/hosts.dart';
import '../helpers/matrix.dart';

FakeCoreApi _status(SyncStatusView view) => FakeCoreApi()
  ..session.add(StrataFixtures.sessionActive)
  ..syncStatus.add(view);

void main() {
  setUpAll(loadStrataFonts);

  goldens(
    'sync_status_offline',
    (v) => goldenFrame(
      v,
      SurfacePreview(sizeClass: v.sizeClass),
      fake: _status(SyncFixtures.offline),
    ),
  );
  goldens(
    'sync_status_syncing',
    (v) => goldenFrame(
      v,
      SurfacePreview(sizeClass: v.sizeClass),
      fake: _status(SyncFixtures.syncing),
    ),
  );
  goldens(
    'sync_page',
    (v) => goldenFrame(
      v,
      const Scaffold(body: SyncScreen()),
      fake: _status(SyncFixtures.synced),
    ),
  );
  goldens(
    'conflict',
    (v) => goldenFrame(
      v,
      const Scaffold(body: ConflictResolutionScreen(opId: 'op-weekly')),
      fake: FakeCoreApi()
        ..conflict['op-weekly'].add(
          const ConflictScreen(
            opId: 'op-weekly',
            conflict: SyncFixtures.detail,
          ),
        ),
    ),
  );
}
