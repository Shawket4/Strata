import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

/// Global map, local mind map and mini graph × size class × theme ×
/// direction (1.0; compact also 2.0).
void main() {
  setUpAll(loadStrataFonts);

  group('global map goldens', () {
    screenGoldens(
      'global_map',
      () => const GlobalMapScreen(),
      () =>
          FakeCoreApi()
            ..globalGraphFilteredAnswer.returns(MapFixtures.globalSmall),
    );
  });

  group('mind map goldens', () {
    screenGoldens(
      'mind_map',
      () => const MindMapScreen('n-pricing-experiments'),
      () {
        final fake = FakeCoreApi();
        for (final depth in const [1, 2]) {
          fake.localGraphFiltered[('n-pricing-experiments', depth, '')].add(
            MapFixtures.pricingLocal,
          );
        }
        return fake;
      },
    );
  });

  group('mini graph goldens', () {
    screenGoldens(
      'mini_graph',
      () => const Align(
        alignment: Alignment.topCenter,
        child: SizedBox(
          width: 340,
          child: MiniGraph('n-pricing-experiments', onOpenMindMap: _noop),
        ),
      ),
      () => FakeCoreApi()
        ..localGraph[('n-pricing-experiments', 1)].add(
          MapFixtures.pricingLocal,
        ),
      cells: goldenVariants(wide: false),
    );
  });
}

void _noop(String _) {}
