import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_maps/src/generated/maps_localizations.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';

const _id = 'n-pricing-experiments';

FakeCoreApi _fake() {
  final fake = FakeCoreApi();
  for (final depth in const [1, 2, 3]) {
    fake.localGraphFiltered[(_id, depth, '')].add(MapFixtures.pricingLocal);
  }
  return fake;
}

int _initialDepth(Variant v) => v.sizeClass == SizeClass.compact ? 1 : 2;

void main() {
  group('MindMapScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = _fake();
        var back = 0;
        await pumpVariant(
          tester,
          v,
          MindMapScreen(_id, onBack: () => back++),
          fake: fake,
          scaffold: true,
        );
        final l10n = lookupMapsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(
            CoreCall('watchLocalGraphFiltered', {
              'id': _id,
              'depth': _initialDepth(v),
              'edgeKinds': const <String>[],
            }),
          ),
        );
        expect(find.byType(InteractiveViewer), findsOneWidget);
        expect(
          find.bySemanticsLabel(
            l10n.mindMapSemantics(title: 'Pricing experiments'),
          ),
          findsOneWidget,
        );
        expect(find.text('Discount policy'), findsOneWidget);
        expect(find.text('Acme Logistics'), findsOneWidget);
        expect(
          find.text(l10n.mindMapCounts(depth: 1, nodes: 9, edges: 8)),
          findsOneWidget,
        );
        if (v.sizeClass == SizeClass.compact) {
          expect(find.byTooltip(l10n.backToNote), findsOneWidget);
          await tester.tap(find.byTooltip(l10n.backToNote));
          expect(back, 1);
          expect(
            find.textContaining(l10n.dragHint, findRichText: true),
            findsNothing,
          );
        } else {
          expect(
            find.textContaining(l10n.dragHint, findRichText: true),
            findsOneWidget,
          );
          expect(find.text(l10n.saveLayout), findsOneWidget);
          expect(find.byTooltip(l10n.saveLayoutHint), findsOneWidget);
        }
        if (v.textScale == 1) await expectAccessible(tester);
      });
    }
  });

  group('MindMapScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const MindMapScreen(_id),
          fake: FakeCoreApi(),
          scaffold: true,
        );
        expect(
          find.bySemanticsLabel(lookupMapsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        final fake = FakeCoreApi();
        fake.localGraphFiltered[('n-gone', _initialDepth(v), '')].add(
          MapFixtures.missingLocal,
        );
        await pumpVariant(
          tester,
          v,
          const MindMapScreen('n-gone'),
          fake: fake,
          scaffold: true,
        );
        expect(
          find.text(lookupMapsLocalizations(v.locale).mindMapNotFound),
          findsOneWidget,
        );
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const MindMapScreen(_id),
          fake: fake,
          scaffold: true,
        );
        fake.localGraphFiltered[(_id, _initialDepth(v), '')].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await tester.pump();
        expect(
          find.text(lookupMapsLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });
    }
  });

  group('MindMapScreen interaction', () {
    final compact = variants(
      sizes: const {'compact': Size(390, 844)},
      scales: const [1],
    ).first;
    final expanded = variants(
      sizes: const {'expanded': Size(1440, 900)},
      scales: const [1],
    ).first;

    testWidgets('depth control re-subscribes at the chosen depth', (
      tester,
    ) async {
      final fake = _fake();
      await pumpVariant(
        tester,
        compact,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(find.bySemanticsLabel('Depth 3'));
      await tester.pump();
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('watchLocalGraphFiltered', {
          'id': _id,
          'depth': 3,
          'edgeKinds': <String>[],
        }),
      );
    });

    testWidgets('compact: tapping a node recentres on it', (tester) async {
      final fake = _fake();
      fake.localGraphFiltered[('n-discount-policy', 1, '')].add(
        MapFixtures.pricingLocal,
      );
      await pumpVariant(
        tester,
        compact,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(find.text('Discount policy'));
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('watchLocalGraphFiltered', {
          'id': 'n-discount-policy',
          'depth': 1,
          'edgeKinds': <String>[],
        }),
      );
    });

    testWidgets('compact: the edge sheet rejects an AI relation', (
      tester,
    ) async {
      final fake = _fake();
      await pumpVariant(
        tester,
        compact,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(
        find.bySemanticsLabel(
          'contradicts: Pricing experiments to Discount policy',
        ),
      );
      await tester.pumpAndSettle();
      expect(
        find.text('Pricing experiments to Discount policy'),
        findsOneWidget,
      );
      expect(find.text('Why AI suggested this'), findsOneWidget);
      await expectAccessible(tester);
      await tester.tap(find.text('Reject'));
      await tester.pumpAndSettle();
      expect(
        fake.calls.last,
        const CoreCall('removeRelation', {
          'srcId': _id,
          'dstId': 'n-discount-policy',
          'relType': 'contradicts',
        }),
      );
      expect(find.text('Why AI suggested this'), findsNothing);
    });

    testWidgets('expanded: retype from the edge card', (tester) async {
      final fake = _fake();
      await pumpVariant(
        tester,
        expanded,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(
        find.bySemanticsLabel('supports: Churn notes to Pricing experiments'),
      );
      await tester.pump();
      expect(find.text('Churn notes to Pricing experiments'), findsOneWidget);
      await tester.tap(find.text('Retype'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('part of').last);
      await tester.pumpAndSettle();
      expect(
        fake.calls.last,
        const CoreCall('retypeRelation', {
          'srcId': 'n-churn-notes',
          'dstId': _id,
          'relType': 'supports',
          'newType': 'part-of',
        }),
      );
    });

    testWidgets('expanded: selecting a node opens its panel', (tester) async {
      final fake = _fake();
      fake.note['n-churn-notes'].add(
        NoteScreen(id: 'n-churn-notes', note: StrataFixtures.noteView),
      );
      final opened = <String>[];
      await pumpVariant(
        tester,
        expanded,
        MindMapScreen(_id, onOpenNote: opened.add),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(find.text('Churn notes'));
      await tester.pump();
      await tester.pump();
      expect(
        fake.calls,
        contains(const CoreCall('watchNote', {'id': 'n-churn-notes'})),
      );
      expect(find.bySemanticsLabel('Selected node'), findsOneWidget);
      await expectAccessible(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Open note'));
      expect(opened, ['n-churn-notes']);
      fake.localGraphFiltered[('n-churn-notes', 2, '')].add(
        MapFixtures.pricingLocal,
      );
      await tester.tap(find.text('Centre map on Churn notes'));
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('watchLocalGraphFiltered', {
          'id': 'n-churn-notes',
          'depth': 2,
          'edgeKinds': <String>[],
        }),
      );
    });

    testWidgets('save layout writes the shown positions', (tester) async {
      final fake = _fake();
      await pumpVariant(
        tester,
        expanded,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      await tester.tap(find.text('Save layout'));
      await settle(tester);
      final name = find.widgetWithText(TextField, 'Map name');
      expect(
        tester.widget<TextField>(name).controller!.text,
        'Pricing experiments',
      );
      await tester.enterText(name, 'Pricing map');
      await tester.tap(find.widgetWithText(FilledButton, 'Save layout'));
      await settle(tester);
      final call = fake.calls.last;
      expect(call.method, 'saveLayout');
      expect(call.args['centerId'], _id);
      expect(call.args['name'], 'Pricing map');
      expect(call.args['positions'], [
        for (final node in MapFixtures.pricingLocal.nodes)
          NodePosition(id: node.id, x: node.x, y: node.y),
      ]);
      expect(find.textContaining('Saved to '), findsOneWidget);
    });

    testWidgets('offline: save layout is off', (tester) async {
      final local = MapFixtures.pricingLocal;
      final fake = FakeCoreApi();
      for (final depth in const [1, 2]) {
        fake.localGraphFiltered[(_id, depth, '')].add(
          LocalGraphView(
            center: local.center,
            found: true,
            depth: depth,
            nodes: local.nodes,
            edges: local.edges,
            relationCount: 5,
            aiRelationCount: 2,
            relationLabel: '5 relations · 2 by AI',
            saveLayout: Availability.offline,
            proposeRelation: local.proposeRelation,
          ),
        );
      }
      await pumpVariant(
        tester,
        expanded,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      expect(find.text('5 relations · 2 by AI'), findsOneWidget);
      final save = tester.widget<OutlinedButton>(
        find.widgetWithText(OutlinedButton, 'Save layout'),
      );
      expect(save.onPressed, isNull);
      expect(
        find.byTooltip('Saving layouts needs a connection to the server'),
        findsOneWidget,
      );
    });

    testWidgets('edge type chips hide edges', (tester) async {
      final fake = _fake();
      // The core answers the new selection without the contradicts edge.
      final kinds = selectedEdgeKinds({EdgeClass.contradicts});
      final local = MapFixtures.pricingLocal;
      fake.localGraphFiltered[(_id, 2, kinds.join(','))].add(
        LocalGraphView(
          center: local.center,
          found: local.found,
          depth: local.depth,
          nodes: local.nodes,
          edges: [
            for (final e in local.edges)
              if (e.kind != 'relation:contradicts') e,
          ],
          relationCount: local.relationCount,
          aiRelationCount: local.aiRelationCount,
          relationLabel: local.relationLabel,
          summary: local.summary,
          saveLayout: local.saveLayout,
          proposeRelation: local.proposeRelation,
        ),
      );
      await pumpVariant(
        tester,
        expanded,
        const MindMapScreen(_id),
        fake: fake,
        scaffold: true,
      );
      expect(
        find.bySemanticsLabel(
          'contradicts: Pricing experiments to Discount policy',
        ),
        findsOneWidget,
      );
      await tester.tap(find.widgetWithText(FilterChip, 'contradicts'));
      // Rebuild with the new selection, then the core's answer arrives.
      await tester.pump();
      await tester.pump();
      expect(
        find.bySemanticsLabel(
          'contradicts: Pricing experiments to Discount policy',
        ),
        findsNothing,
      );
      final call = fake.calls.last;
      expect(call.method, 'watchLocalGraphFiltered');
      expect((call.args['id'], call.args['depth']), (_id, 2));
      expect(call.args['edgeKinds'], kinds);
      expect(kinds, isNot(contains('relation:contradicts')));
      expect(kinds, containsAll(['link', 'relation:supports']));
    });
  });
}
