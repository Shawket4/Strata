import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

/// Ask and Search × size class × theme × direction (1.0; compact also 2.0),
/// plus the offline states.
void main() {
  setUpAll(loadStrataFonts);

  group('ask goldens', () {
    screenGoldens(
      'ask',
      (v) => goldenFrame(
        v,
        AskScreen(onOpenNote: (_, _) {}),
        fake: FakeCoreApi()
          ..askStream.add(AskFixtures.available)
          ..resolveCitationAnswer.returns(AskFixtures.preview),
        scaffold: true,
      ),
    );
    screenGoldens(
      'ask_not_yet_available',
      (v) => goldenFrame(
        v,
        const AskScreen(),
        fake: FakeCoreApi()..askStream.add(AskFixtures.notYet),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
    screenGoldens(
      'ask_offline',
      (v) => goldenFrame(
        v,
        const AskScreen(),
        fake: FakeCoreApi()..askStream.add(AskFixtures.offline),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });

  group('search goldens', () {
    screenGoldens(
      'search',
      (v) => goldenFrame(
        v,
        SearchScreen(
          initialQuery: 'pricing',
          initialMode: SearchMode.hybrid,
          onOpenNote: (_, _) {},
        ),
        fake: FakeCoreApi()..searchAnswer.returns(AskFixtures.hybrid),
        scaffold: true,
      ),
    );
    screenGoldens(
      'search_semantic_offline',
      (v) => goldenFrame(
        v,
        const SearchScreen(initialQuery: 'pricing'),
        fake: FakeCoreApi()..searchAnswer.returns(AskFixtures.semanticOffline),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });
}
