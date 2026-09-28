// Prints the measured frame times: the numbers are the test's output.
// ignore_for_file: avoid_print

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

/// Performance budget of the global map painter (D3 = a): 10 000 nodes and
/// 40 000 edges at interactive frame times.
///
/// Method. A synthetic graph (seeded, so every run paints the same thing) with
/// the node-kind and edge-kind mix of a real vault is painted:
///
/// 1. **Painter only.** `GraphPainter.paint` records into a
///    `ui.PictureRecorder` at far (fit all), mid (60 %) and near (120 %) zoom;
///    3 warm-up frames, then the median of 15 frames is asserted. This is the
///    UI-thread cost of a frame of pan/zoom (the widget tree does not rebuild
///    while panning: the camera repaints the `CustomPaint` directly).
/// 2. **Whole frame.** The real `GlobalMapScreen` is pumped with the graph,
///    the camera is moved, and the median `tester.pump()` (build, layout,
///    paint of the widget tree) is asserted.
///
/// Widget tests run in the JIT (debug) VM with no GPU, so these are
/// conservative stand-ins for profile mode (AOT is typically 2–4× faster)
/// and cover the UI thread only; raster cost is measured in the integration
/// profile run on devices. The budgets below are therefore the 60 Hz frame
/// budget (16 ms) for the painter and twice that for a debug-mode full frame.
const double paintBudgetMs = 16;
const double frameBudgetMs = 32;

GlobalGraphView syntheticGraph({int nodes = 10000, int edges = 40000}) {
  final random = math.Random(20260927);
  const kinds = [
    (0.85, 'note'),
    (0.90, 'concept'),
    (0.94, 'person'),
    (0.97, 'company'),
    (0.99, 'document'),
    (1.00, 'place'),
  ];
  const edgeKinds = [
    (0.50, 'link'),
    (0.65, 'relation:related'),
    (0.70, 'relation:part-of'),
    (0.75, 'relation:supports'),
    (0.78, 'relation:contradicts'),
    (0.83, 'relation:follows-up'),
    (0.85, 'relation:duplicates'),
    (0.95, 'mention'),
    (1.00, 'similarity'),
  ];
  String pick(List<(double, String)> table) {
    final r = random.nextDouble();
    return table.firstWhere((e) => r <= e.$1).$2;
  }

  final side = math.sqrt(nodes).ceil();
  final graphNodes = [
    for (var i = 0; i < nodes; i++)
      GraphNode(
        id: 'n$i',
        title: 'Note $i',
        kind: GraphNodeKind.values.byName(pick(kinds)),
        depth: 0,
        clusterId: 'k${(i % side) ~/ 10}-${(i ~/ side) ~/ 10}',
        degree: random.nextInt(12),
        x: (i % side) * 60 + random.nextDouble() * 30,
        y: (i ~/ side) * 60 + random.nextDouble() * 30,
        titleDir: TextDir.ltr,
        updatedLabel: '',
        labelRank: 0,
        isHub: false,
      ),
  ];
  final graphEdges = [
    for (var e = 0; e < edges; e++)
      () {
        final a = random.nextInt(nodes);
        // Mostly local edges (like a laid-out graph), some long ones.
        final b = random.nextDouble() < 0.9
            ? (a + 1 + random.nextInt(3 * side)) % nodes
            : random.nextInt(nodes);
        final kind = pick(edgeKinds);
        return GraphEdge(
          src: 'n$a',
          dst: 'n$b',
          kind: kind,
          by: kind.startsWith('relation') ? 'ai' : null,
          confidence: kind.startsWith('relation') ? 0.8 : null,
          id: '',
          label: '',
        );
      }(),
  ];
  return GlobalGraphView(
    nodes: graphNodes,
    edges: graphEdges,
    clusters: [],
    filter: const GraphFilter(
      edgeKinds: [],
      nodeKinds: [],
      similarity: false,
      lens: GraphLens.notes,
      includeTags: false,
    ),
    edgeCounts: [],
    nodeCounts: [],
    neighbours: [],
    similarity: Availability.available,
  );
}

double median(List<double> values) {
  final sorted = [...values]..sort();
  return sorted[sorted.length ~/ 2];
}

void main() {
  final view = syntheticGraph();

  test('scene build of 10k nodes / 40k edges', () {
    final watch = Stopwatch()..start();
    final scene = GraphScene.from(view.nodes, view.edges);
    watch.stop();
    print('scene build: ${watch.elapsedMilliseconds} ms');
    expect(scene.nodeCount, 10000);
    expect(scene.edgeCount, 40000);
    // A one-off per view-model value, not per frame.
    expect(watch.elapsedMilliseconds, lessThan(500));
  });

  testWidgets('painter frame time at far / mid / near zoom', (tester) async {
    final scene = GraphScene.from(view.nodes, view.edges);
    const size = Size(1100, 844);
    late BuildContext context;
    await tester.pumpWidget(
      Theme(
        data: StrataTheme.light(),
        child: Builder(
          builder: (c) {
            context = c;
            return const SizedBox();
          },
        ),
      ),
    );
    final palette = GraphPalette.of(context);
    final cache = GraphRenderCache();
    addTearDown(cache.dispose);
    final fit = GraphCamera.fit(scene.bounds, size);
    final centre = scene.bounds.center;
    final cameras = {
      'far': fit,
      'mid': fit.centredOn(centre, size).zoomed(0.6 / fit.scale, Offset.zero),
      'near': const GraphCamera()
          .centredOn(centre, size)
          .zoomed(1.2, Offset.zero),
    };
    for (final entry in cameras.entries) {
      final controller = GraphViewController()..camera = entry.value;
      addTearDown(controller.dispose);
      final times = <double>[];
      for (var frame = 0; frame < 18; frame++) {
        // Pan a little every frame, like a drag.
        controller.camera = controller.camera.panned(const Offset(3, 2));
        final painter = GraphPainter(
          scene: scene,
          view: controller,
          palette: palette,
          cache: cache,
          textDirection: TextDirection.ltr,
          options: const GraphPaintOptions(
            selected: 5000,
            highlight: {5000, 5001, 5002},
            hovered: 4200,
          ),
        );
        final recorder = ui.PictureRecorder();
        final watch = Stopwatch()..start();
        painter.paint(Canvas(recorder), size);
        watch.stop();
        recorder.endRecording().dispose();
        if (frame >= 3) times.add(watch.elapsedMicroseconds / 1000);
      }
      final ms = median(times);
      print('paint ${entry.key}: median ${ms.toStringAsFixed(2)} ms');
      expect(ms, lessThan(paintBudgetMs), reason: '${entry.key} zoom');
    }
  });

  testWidgets('whole frame of the global map while panning', (tester) async {
    final fake = FakeCoreApi()..globalGraphFilteredAnswer.returns(view);
    await pumpStrataScreen(
      tester,
      const Scaffold(body: GlobalMapScreen()),
      fake: fake,
      sizeClass: SizeClass.expanded,
    );
    await tester.pump();
    final viewport = tester.widget<CustomPaint>(
      find.descendant(
        of: find.byType(GlobalMapScreen),
        matching: find.byWidgetPredicate(
          (w) => w is CustomPaint && w.painter is GraphPainter,
        ),
      ),
    );
    final controller = (viewport.painter! as GraphPainter).view;
    controller.camera = controller.camera.zoomed(2, const Offset(400, 400));
    final times = <double>[];
    for (var frame = 0; frame < 18; frame++) {
      controller.camera = controller.camera.panned(const Offset(4, 3));
      final watch = Stopwatch()..start();
      await tester.pump();
      watch.stop();
      if (frame >= 3) times.add(watch.elapsedMicroseconds / 1000);
    }
    final ms = median(times);
    print('whole frame: median ${ms.toStringAsFixed(2)} ms');
    expect(ms, lessThan(frameBudgetMs));
  });
}
