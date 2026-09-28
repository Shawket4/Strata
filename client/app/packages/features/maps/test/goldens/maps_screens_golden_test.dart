import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

/// Global map, local mind map and mini graph × size class × theme ×
/// direction (1.0; compact also 2.0).
void main() {
  setUpAll(loadStrataFonts);

  group('global map goldens', () {
    screenGoldens(
      'global_map',
      (v) => goldenFrame(
        v,
        const GlobalMapScreen(),
        fake: FakeCoreApi()
          ..globalGraphFilteredAnswer.returns(MapFixtures.globalSmall),
        scaffold: true,
      ),
    );
  });

  group('mind map goldens', () {
    screenGoldens(
      'mind_map',
      (v) => goldenFrame(
        v,
        const MindMapScreen('n-pricing-experiments'),
        fake: (() {
          final fake = FakeCoreApi();
          for (final depth in const [1, 2]) {
            fake.localGraphFiltered[('n-pricing-experiments', depth, '')].add(
              MapFixtures.pricingLocal,
            );
          }
          return fake;
        })(),
        scaffold: true,
      ),
    );
  });

  group('mini graph goldens', () {
    screenGoldens(
      'mini_graph',
      (v) => goldenFrame(
        v,
        const Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: 340,
            child: MiniGraph('n-pricing-experiments', onOpenMindMap: _noop),
          ),
        ),
        fake: FakeCoreApi()
          ..localGraph[('n-pricing-experiments', 1)].add(
            MapFixtures.pricingLocal,
          ),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });
}

void _noop(String _) {}
