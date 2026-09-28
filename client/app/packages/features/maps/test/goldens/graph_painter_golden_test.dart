import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart' show GraphNodeKind;
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

/// A painted graph at a fixed camera, in the Strata theme.
class _PaintedGraph extends StatelessWidget {
  const new({
    required this.scene,
    required this.camera,
    this.options = const GraphPaintOptions(),
    this.dark = false,
    this.rtl = false,
  });

  final GraphScene scene;
  final GraphCamera camera;
  final GraphPaintOptions options;
  final bool dark;
  final bool rtl;

  @override
  Widget build(BuildContext context) {
    final view = GraphViewController()..camera = camera;
    return Theme(
      data: dark ? StrataTheme.dark() : StrataTheme.light(),
      child: Directionality(
        textDirection: rtl ? TextDirection.rtl : TextDirection.ltr,
        child: Builder(
          builder: (context) => ColoredBox(
            color: context.strataColors.surface,
            child: CustomPaint(
              size: const Size(640, 480),
              painter: GraphPainter(
                scene: scene,
                view: view,
                palette: GraphPalette.of(context),
                cache: GraphRenderCache(),
                textDirection: rtl ? TextDirection.rtl : TextDirection.ltr,
                options: options,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

void main() {
  final scene = GraphScene.from(
    MapFixtures.globalSmall.nodes,
    MapFixtures.globalSmall.edges,
  );
  // What the core answers for "contradicts and body links off, people off"
  // (the core filters, L15; the painter only paints).
  final small = MapFixtures.globalSmall;
  final kept = {
    for (final n in small.nodes)
      if (n.kind != GraphNodeKind.person) n.id,
  };
  final filteredScene = GraphScene.from(
    [
      for (final n in small.nodes)
        if (kept.contains(n.id)) n,
    ],
    [
      for (final e in small.edges)
        if (kept.contains(e.src) &&
            kept.contains(e.dst) &&
            e.kind != 'relation:contradicts' &&
            e.kind != 'link' &&
            e.kind != 'embed')
          e,
    ],
  );
  const viewport = Size(640, 480);
  final fit = GraphCamera.fit(scene.bounds, viewport);
  GraphCamera at(double scale) => const GraphCamera()
      .centredOn(scene.bounds.center, viewport)
      .zoomed(scale, Offset(viewport.width / 2, viewport.height / 2));
  final acme = scene.index['c-acme-logistics']!;

  group('painter goldens', () {
    setUpAll(loadStrataFonts);
    final cases = <String, Widget Function()>{
      'near_selected_light': () => _PaintedGraph(
        scene: scene,
        camera: at(1.2),
        options: GraphPaintOptions(
          selected: acme,
          highlight: {
            acme,
            scene.index['p-ahmed-samir']!,
            scene.index['n-call-acme']!,
            scene.index['n-weekly-invoicing']!,
          },
        ),
      ),
      'mid_light': () => _PaintedGraph(scene: scene, camera: at(0.7)),
      'far_dark': () =>
          _PaintedGraph(scene: scene, camera: at(0.35), dark: true),
      'fit_dark_rtl': () =>
          _PaintedGraph(scene: scene, camera: fit, dark: true, rtl: true),
      'filtered_light': () => _PaintedGraph(
        scene: filteredScene,
        camera: fit,
        options: const GraphPaintOptions(focusCluster: 'k-pricing'),
      ),
      'hover_light': () => _PaintedGraph(
        scene: scene,
        camera: fit,
        options: GraphPaintOptions(hovered: scene.index['k-pricing']),
      ),
    };
    for (final entry in cases.entries) {
      unawaited(
        goldenTest(
          'graph painter ${entry.key}',
          fileName: 'graph_painter_${entry.key}',
          constraints: BoxConstraints.tight(viewport),
          builder: entry.value,
        ),
      );
    }
  });
}
