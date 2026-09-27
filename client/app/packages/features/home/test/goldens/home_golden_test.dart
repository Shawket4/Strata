@Tags(['golden'])
library;

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/harness.dart';

/// Home at compact / medium / expanded × light/dark × LTR/RTL (1.0; compact
/// also 2.0), and the offline and empty states on compact.
void main() {
  setUpAll(loadStrataFonts);

  for (final v in goldenMatrix()) {
    unawaited(
      goldenTest(
        'home $v',
        fileName: 'home_${v.id}',
        builder: () => goldenScreen(
          v,
          const HomeScreen(),
          FakeCoreApi()..home.add(StrataFixtures.homeView),
        ),
      ),
    );
  }
  for (final v in goldenMatrix(
    sizes: const {'compact': StrataTestSizes.compact},
  )) {
    unawaited(
      goldenTest(
        'home empty $v',
        fileName: 'home_empty_${v.id}',
        builder: () => goldenScreen(
          v,
          const HomeScreen(),
          FakeCoreApi()
            ..home.add(
              HomeView(
                recentNotes: const [],
                inboxCount: 0,
                tasks: const TaskSections(
                  overdue: [],
                  today: [],
                  upcoming: [],
                  recurring: [],
                  noDate: [],
                ),
                sync_: StrataFixtures.syncPillOffline,
              ),
            ),
        ),
      ),
    );
  }
}
