import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/src/generated/maps_localizations.dart';
import 'package:strata_maps/src/graph/graph_viewport.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

FakeCoreApi _fake() => FakeCoreApi()
  ..globalGraphFilteredAnswer.returns(MapFixtures.globalSmall)
  ..localGraph[('c-acme-logistics', 1)].add(MapFixtures.acmeLocal);

/// Screen position of a node of the painted map.
Offset nodeOnScreen(WidgetTester tester, String id) {
  final viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
  final index = viewport.scene.index[id]!;
  final local = viewport.view.camera.toScreen(viewport.scene.positionOf(index));
  return tester.getTopLeft(find.byType(GraphViewport)) + local;
}

void main() {
  group('GlobalMapScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = _fake();
        await pumpVariant(tester, v, const GlobalMapScreen(), fake);
        final l10n = lookupMapsLocalizations(v.locale);
        expectNoErrors(tester);
        if (v.sizeClass == SizeClass.compact) {
          expect(find.text(l10n.mapCompactTitle), findsOneWidget);
          expect(find.byType(GraphViewport), findsNothing);
          expect(fake.calls, isNot(contains(const CoreCall('globalGraph'))));
        } else {
          expect(fake.calls, contains(const CoreCall('globalGraph')));
          expect(
            find.bySemanticsLabel(l10n.mapSemantics(nodes: 15, edges: 15)),
            findsOneWidget,
          );
          expect(find.byTooltip(l10n.zoomIn), findsOneWidget);
          expect(find.byTooltip(l10n.zoomToFit), findsOneWidget);
          final expanded = v.sizeClass == SizeClass.expanded;
          // Expanded: filters panel and minimap; medium: filters drawer.
          expect(
            find.text(l10n.edgeTypes),
            expanded ? findsOneWidget : findsNothing,
          );
          expect(
            find.bySemanticsLabel(l10n.minimap),
            expanded ? findsOneWidget : findsNothing,
          );
          expect(find.text(l10n.filters), findsOneWidget);
          expect(
            find.text(l10n.mapCounts(nodes: 15, edges: 15, clusters: 3)),
            expanded ? findsOneWidget : findsNothing,
          );
        }
        if (v.textScale == 1) await expectAccessible(tester);
      });
    }
  });

  group('GlobalMapScreen states', () {
    for (final v in variants(
      sizes: const {'medium': Size(1024, 768), 'expanded': Size(1440, 900)},
      scales: const [1],
    )) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const GlobalMapScreen(),
          _fake(),
          overrides: [
            globalGraphFilteredProvider(
              const GraphFilter(
                edgeKinds: [],
                nodeKinds: [],
                similarity: true,
                lens: GraphLens.notes,
                includeTags: false,
              ),
            ).overrideWith((ref) => Completer<GlobalGraphView>().future),
          ],
        );
        expect(
          find.bySemanticsLabel(lookupMapsLocalizations(v.locale).loading),
          findsOneWidget,
        );
        expect(find.byType(GraphViewport), findsNothing);
      });

      testWidgets('empty $v', (tester) async {
        final fake = _fake()
          ..globalGraphFilteredAnswer.returns(
            const GlobalGraphView(
              nodes: [],
              edges: [],
              clusters: [],
              filter: GraphFilter(
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
            ),
          );
        await pumpVariant(tester, v, const GlobalMapScreen(), fake);
        expect(
          find.text(lookupMapsLocalizations(v.locale).mapEmptyTitle),
          findsOneWidget,
        );
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final fake = _fake()
          ..globalGraphFilteredAnswer.throws(
            const CoreFailure(code: 'store', messageKey: 'error.store'),
          );
        await pumpVariant(tester, v, const GlobalMapScreen(), fake);
        final l10n = lookupMapsLocalizations(v.locale);
        expect(find.text(l10n.errorTitle), findsOneWidget);
        expect(find.text(l10n.errorMessage(code: 'store')), findsOneWidget);
        await expectAccessible(tester);
      });
    }
  });

  group('GlobalMapScreen interaction', () {
    final expanded = variants(
      sizes: const {'expanded': Size(1440, 900)},
      scales: const [1],
    ).first;
    final medium = variants(
      sizes: const {'medium': Size(1024, 768)},
      scales: const [1],
    ).first;

    testWidgets('tapping a node focuses it and dims the rest', (tester) async {
      String? opened;
      String? mapped;
      final fake = _fake();
      await pumpVariant(
        tester,
        expanded,
        GlobalMapScreen(
          onOpenNote: (id) => opened = id,
          onOpenMindMap: (id) => mapped = id,
        ),
        fake,
      );
      await tester.tapAt(nodeOnScreen(tester, 'c-acme-logistics'));
      await tester.pump();
      await tester.pump();
      expect(find.text('Focused on Acme Logistics'), findsOneWidget);
      expect(
        fake.calls,
        contains(
          const CoreCall('watchLocalGraph', {
            'id': 'c-acme-logistics',
            'depth': 1,
          }),
        ),
      );
      final viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      final scene = viewport.scene;
      expect(viewport.options.selected, scene.index['c-acme-logistics']);
      expect(viewport.options.highlight, {
        for (final id in const [
          'c-acme-logistics',
          'p-ahmed-samir',
          'n-call-acme',
          'n-weekly-invoicing',
        ])
          scene.index[id],
      });
      await tester.tap(find.text('Open note'));
      expect(opened, 'c-acme-logistics');
      await tester.tap(find.text('Open local map'));
      expect(mapped, 'c-acme-logistics');
      await tester.tap(find.text('Clear'));
      await tester.pump();
      expect(find.text('Focused on Acme Logistics'), findsNothing);
    });

    testWidgets('tapping empty canvas clears the selection', (tester) async {
      await pumpVariant(tester, expanded, const GlobalMapScreen(), _fake());
      await tester.tapAt(nodeOnScreen(tester, 'c-acme-logistics'));
      await tester.pump();
      expect(find.text('Focused on Acme Logistics'), findsOneWidget);
      final topLeft = tester.getTopLeft(find.byType(GraphViewport));
      await tester.tapAt(topLeft + const Offset(4, 300));
      await tester.pump();
      expect(find.text('Focused on Acme Logistics'), findsNothing);
    });

    testWidgets('search focuses a result from the core', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, expanded, const GlobalMapScreen(), fake);
      await tester.enterText(find.byType(TextField), 'pricing');
      await tester.pump();
      await tester.pump();
      expect(
        fake.calls,
        contains(
          const CoreCall('search', {
            'query': 'pricing',
            'mode': SearchMode.keyword,
          }),
        ),
      );
      await tester.tap(find.text('Pricing experiments').last);
      await tester.pump();
      await tester.pump();
      expect(find.text('Focused on Pricing experiments'), findsOneWidget);
      final viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      expect(viewport.view.camera.scale, greaterThanOrEqualTo(1));
    });

    testWidgets('zoom buttons change the camera and the zoom band', (
      tester,
    ) async {
      await pumpVariant(tester, expanded, const GlobalMapScreen(), _fake());
      final viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      final before = viewport.view.camera.scale;
      await tester.tap(find.byTooltip('Zoom in'));
      await tester.pump();
      expect(viewport.view.camera.scale, closeTo(before * 1.25, 1e-9));
      await tester.tap(find.byTooltip('Zoom out'));
      await tester.tap(find.byTooltip('Zoom out'));
      await tester.pump();
      expect(viewport.view.camera.scale, closeTo(before, 1e-9 + before * 0.2));
      await tester.tap(find.byTooltip('Zoom to fit'));
      await tester.pump();
      expect(viewport.view.camera.scale, closeTo(before, 1e-9));
    });

    testWidgets('filters hide edge classes and node kinds', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, expanded, const GlobalMapScreen(), fake);
      await tester.tap(find.widgetWithText(CheckboxListTile, 'contradicts'));
      await tester.pump();
      await tester.tap(find.widgetWithText(FilterChip, 'Person'));
      await tester.pump();
      await tester.scrollUntilVisible(
        find.text('Pricing & plans'),
        100,
        scrollable: find.ancestor(
          of: find.text('Edge types'),
          matching: find.byType(Scrollable),
        ),
      );
      final radio = find.widgetWithText(
        RadioListTile<String?>,
        'Pricing & plans',
      );
      await tester.ensureVisible(radio);
      await tester.pumpAndSettle();
      await tester.tap(radio);
      await tester.pump();
      // The core filters: the last query leaves out the hidden edge class
      // and node kind; the cluster focus only dims.
      GraphFilter lastQuery() =>
          fake.calls
                  .lastWhere((c) => c.method == 'globalGraphFiltered')
                  .args['filter']!
              as GraphFilter;
      expect(lastQuery().edgeKinds, isNot(contains('relation:contradicts')));
      expect(lastQuery().edgeKinds, contains('relation:supports'));
      expect(lastQuery().nodeKinds, isNot(contains('person')));
      expect(lastQuery().nodeKinds, contains('note'));
      var viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      expect(viewport.options.focusCluster, 'k-pricing');
      await tester.tap(find.text('Reset'));
      await tester.pump();
      viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      expect(lastQuery().edgeKinds, isEmpty);
      expect(lastQuery().nodeKinds, isEmpty);
      expect(viewport.options.focusCluster, isNull);
    });

    testWidgets('medium opens and closes the filters drawer', (tester) async {
      await pumpVariant(tester, medium, const GlobalMapScreen(), _fake());
      expect(find.text('Edge types'), findsNothing);
      await tester.tap(find.text('Filters'));
      await tester.pump();
      expect(find.text('Map filters'), findsOneWidget);
      expect(find.text('Edge types'), findsOneWidget);
      await expectAccessible(tester);
      await tester.tap(find.byTooltip('Close filters'));
      await tester.pump();
      expect(find.text('Edge types'), findsNothing);
    });

    testWidgets('hovering a node shows its card', (tester) async {
      await pumpVariant(tester, expanded, const GlobalMapScreen(), _fake());
      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await gesture.addPointer(location: Offset.zero);
      addTearDown(gesture.removePointer);
      await gesture.moveTo(nodeOnScreen(tester, 'p-ahmed-samir'));
      await tester.pump();
      expect(find.text('Ahmed Samir'), findsOneWidget);
      expect(find.text('6 links'), findsOneWidget);
    });

    testWidgets('keyboard zoom and escape', (tester) async {
      await pumpVariant(tester, expanded, const GlobalMapScreen(), _fake());
      final viewport = tester.widget<GraphViewport>(find.byType(GraphViewport));
      await tester.tapAt(nodeOnScreen(tester, 'c-acme-logistics'));
      await tester.pump();
      final before = viewport.view.camera.scale;
      await tester.sendKeyEvent(LogicalKeyboardKey.equal);
      await tester.pump();
      expect(viewport.view.camera.scale, closeTo(before * 1.25, 1e-9));
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.text('Focused on Acme Logistics'), findsNothing);
    });
  });
}
