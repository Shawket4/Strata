import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

/// Ask and Search × size class × theme × direction (1.0; compact also 2.0),
/// plus the offline states.
void main() {
  setUpAll(loadStrataFonts);

  group('ask goldens', () {
    screenGoldens(
      'ask',
      () => AskScreen(onOpenNote: (_, _) {}),
      () => FakeCoreApi()..askViewAnswer.returns(AskFixtures.available),
    );
    screenGoldens(
      'ask_not_yet_available',
      () => const AskScreen(),
      FakeCoreApi.new,
      cells: goldenVariants(wide: false),
    );
    screenGoldens(
      'ask_offline',
      () => const AskScreen(),
      () => FakeCoreApi()..askViewAnswer.returns(AskFixtures.offline),
      cells: goldenVariants(wide: false),
    );
  });

  group('search goldens', () {
    screenGoldens(
      'search',
      () => SearchScreen(initialQuery: 'pricing', onOpenNote: (_, _) {}),
      FakeCoreApi.new,
    );
    screenGoldens(
      'search_semantic_offline',
      () => const SearchScreen(initialQuery: 'pricing'),
      () => FakeCoreApi()..searchAnswer.returns(AskFixtures.semanticOffline),
      cells: goldenVariants(wide: false),
    );
  });
}
