@Tags(['golden'])
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

/// Home at compact / medium / expanded × light/dark × LTR/RTL (1.0; compact
/// also 2.0), and the offline and empty states on compact.
void main() {
  setUpAll(loadStrataFonts);

  screenGoldens(
    'home',
    (v) => goldenFrame(
      v,
      const HomeScreen(),
      fake: FakeCoreApi()..home.add(StrataFixtures.homeView),
      scaffold: true,
    ),
  );
  screenGoldens(
    'home_empty',
    cells: goldenVariants(wide: false),
    (v) => goldenFrame(
      v,
      const HomeScreen(),
      scaffold: true,
      fake: FakeCoreApi()
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
              upcomingGroups: [],
              todayCount: 0,
            ),
            sync_: StrataFixtures.syncPillOffline,
            todayLabel: '',
            greeting: '',
            displayName: '',
            inboxPreview: [],
            needsYouCount: 0,
            contradictionsCount: 0,
            inboxSummary: '',
            aiActivity: Availability.available,
            aiActivityItems: [],
            aiActivityHeadline: '',
            openItems: Availability.available,
            openItemList: [],
            pinned: [],
          ),
        ),
    ),
  );
}
