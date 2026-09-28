import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/src/generated/maps_localizations.dart';
import 'package:strata_maps/src/graph/graph_viewport.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';

/// Live resize: the global map is medium/expanded only; the filters panel
/// is a side panel on expanded and a drawer on medium (PLAN §11, §16.5).
void main() {
  testWidgets('global map follows the window across size classes', (
    tester,
  ) async {
    final fake = FakeCoreApi()
      ..globalGraphFilteredAnswer.returns(MapFixtures.globalSmall);
    await pumpVariant(
      tester,
      variants().first,
      const GlobalMapScreen(),
      fake: fake,
      scaffold: true,
    );
    final l10n = lookupMapsLocalizations(const Locale('en'));
    expect(find.text(l10n.mapCompactTitle), findsOneWidget);

    tester.view.physicalSize = StrataTestSizes.expanded;
    await tester.pump();
    await tester.pump();
    expect(find.byType(GraphViewport), findsOneWidget);
    expect(find.text(l10n.edgeTypes), findsOneWidget);

    tester.view.physicalSize = StrataTestSizes.medium;
    await tester.pump();
    expect(find.byType(GraphViewport), findsOneWidget);
    expect(find.text(l10n.edgeTypes), findsNothing);
    expect(find.text(l10n.filters), findsOneWidget);
  });
}
