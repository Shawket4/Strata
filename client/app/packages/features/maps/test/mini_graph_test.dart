import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/src/generated/maps_localizations.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

void main() {
  for (final v in variants()) {
    testWidgets('mini graph $v', (tester) async {
      final fake = FakeCoreApi()
        ..localGraph[('n-pricing-experiments', 1)].add(
          MapFixtures.pricingLocal,
        );
      final opened = <String>[];
      await pumpVariant(
        tester,
        v,
        Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: 340,
            child: MiniGraph(
              'n-pricing-experiments',
              onOpenMindMap: opened.add,
            ),
          ),
        ),
        fake,
      );
      final l10n = lookupMapsLocalizations(v.locale);
      expectNoErrors(tester);
      expect(
        fake.calls,
        contains(
          const CoreCall('watchLocalGraph', {
            'id': 'n-pricing-experiments',
            'depth': 1,
          }),
        ),
      );
      expect(
        find.bySemanticsLabel(
          l10n.miniGraphSemantics(title: 'Pricing experiments', count: 8),
        ),
        findsOneWidget,
      );
      if (v.textScale == 1) await expectAccessible(tester);
      await tester.tap(find.text(l10n.openInMap));
      expect(opened, ['n-pricing-experiments']);
    });
  }

  testWidgets('mini graph renders nothing but its frame while loading', (
    tester,
  ) async {
    await pumpVariant(
      tester,
      variants().first,
      const MiniGraph('n-pricing-experiments'),
      FakeCoreApi(),
    );
    expect(find.text('Open in map'), findsNothing);
    expect(find.text('Graph'), findsOneWidget);
  });
}
