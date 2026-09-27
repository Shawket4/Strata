import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';

void main() {
  final scene = GraphScene.from(
    MapFixtures.globalSmall.nodes,
    MapFixtures.globalSmall.edges,
  );
  const viewport = Size(640, 480);
  final fit = GraphCamera.fit(scene.bounds, viewport);
  final acme = scene.index['c-acme-logistics']!;

  group('GraphScene', () {
    test('indexes nodes and drops edges with unknown ends', () {
      final partial = GraphScene.from(MapFixtures.globalSmall.nodes, [
        ...MapFixtures.globalSmall.edges,
        const GraphEdge(
          src: 'n-pricing-experiments',
          dst: 'n-unknown',
          kind: 'link',
        ),
      ]);
      expect(partial.nodeCount, 15);
      expect(partial.edgeCount, 15);
      expect(partial.index['c-acme-logistics'], 6);
      expect(partial.kinds[6], NodeKind.company);
      expect(partial.edgeClasses, [
        EdgeClass.concept,
        EdgeClass.partOf,
        EdgeClass.contradicts,
        EdgeClass.supports,
        EdgeClass.duplicates,
        EdgeClass.bodyLink,
        EdgeClass.similarity,
        EdgeClass.followsUp,
        EdgeClass.mention,
        EdgeClass.mention,
        EdgeClass.entity,
        EdgeClass.related,
        EdgeClass.custody,
        EdgeClass.custody,
        EdgeClass.placeNesting,
      ]);
      expect(partial.bounds, const Rect.fromLTRB(-170, -110, 400, 350));
    });

    test('note radius grows with degree, 4 to 12', () {
      expect(GraphScene.radiusOf(NodeKind.note, 0), 4);
      expect(GraphScene.radiusOf(NodeKind.note, 6), 7);
      expect(GraphScene.radiusOf(NodeKind.note, 40), 12);
      expect(GraphScene.radiusOf(NodeKind.person, 40), 7);
    });
  });

  group('hit-testing', () {
    test('finds the node under a point within radius and slop', () {
      expect(scene.hitTest(const Offset(320, -20)), acme);
      expect(scene.hitTest(const Offset(326, -20)), acme);
      expect(scene.hitTest(const Offset(332, -20)), isNull);
      expect(scene.hitTest(const Offset(-500, -500)), isNull);
    });

    test('prefers the nearest of overlapping candidates', () {
      final close = GraphScene.from([
        MapFixtures.globalSmall.nodes[0],
        const GraphNode(
          id: 'n-b',
          title: 'B',
          kind: 'note',
          depth: 0,
          degree: 0,
          x: 6,
          y: 0,
        ),
      ], const []);
      expect(close.hitTest(const Offset(4, 0)), 1);
      expect(close.hitTest(const Offset(2, 0)), 0);
    });

    test('works across grid cells and with zoom-scaled radii', () {
      // Node at (320, -20) sits in a cell; probe from the neighbouring cell.
      expect(scene.hitTest(const Offset(320, 10), radiusFactor: 4), acme);
      expect(scene.hitTest(const Offset(320, 10)), isNull);
    });

    test('camera round-trips screen and world points', () {
      final camera = fit
          .zoomed(1.7, const Offset(100, 80))
          .panned(const Offset(-30, 12));
      const world = Offset(320, -20);
      final screen = camera.toScreen(world);
      expect(camera.toWorld(screen).dx, closeTo(world.dx, 1e-9));
      expect(camera.toWorld(screen).dy, closeTo(world.dy, 1e-9));
      expect(ZoomBand.of(0.39), ZoomBand.far);
      expect(ZoomBand.of(0.4), ZoomBand.mid);
      expect(ZoomBand.of(1), ZoomBand.near);
    });
  });

  group('edge kind mapping', () {
    test('every core edge kind maps to its drawing class', () {
      expect(edgeClassOf('link'), EdgeClass.bodyLink);
      expect(edgeClassOf('embed'), EdgeClass.bodyLink);
      expect(edgeClassOf('relation:follows-up'), EdgeClass.followsUp);
      expect(edgeClassOf('custody:holder'), EdgeClass.custody);
      expect(edgeClassOf('part-of-place'), EdgeClass.placeNesting);
      expect(edgeClassOf('entity:client-of'), EdgeClass.entity);
      expect(lineStyleOf(EdgeClass.contradicts).midCross, isTrue);
      expect(lineStyleOf(EdgeClass.duplicates).doubleLine, isTrue);
      expect(lineStyleOf(EdgeClass.placeNesting).dashPattern, [1, 3]);
      expect(relationWireTypes[EdgeClass.partOf], 'part-of');
    });
  });
}
