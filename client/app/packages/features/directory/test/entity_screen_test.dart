import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart' hide EntityScreen;
import 'package:strata_state/strata_state.dart' as vm show EntityScreen;
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

const _id = 'p-ahmed-samir';

FakeCoreApi _fake([vm.EntityScreen? screen]) {
  final fake = FakeCoreApi();
  fake.entity[screen?.id ?? _id].add(screen ?? StrataFixtures.entityScreen);
  fake.localGraph[(_id, 1)].add(StrataFixtures.localGraphView);
  return fake;
}

void main() {
  group('EntityScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = _fake();
        final notes = <(String, String?)>[];
        final opened = <String>[];
        var back = 0;
        await pumpVariant(
          tester,
          v,
          EntityScreen(
            _id,
            onOpenNote: (id, anchor) => notes.add((id, anchor)),
            onOpenEntity: opened.add,
            onBack: () => back++,
          ),
          fake,
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
        );
        expect(find.text('Ahmed Samir'), findsWidgets);
        expect(find.text(l10n.summary), findsOneWidget);
        expect(find.text(l10n.insights), findsOneWidget);
        expect(find.text(l10n.openItems), findsOneWidget);
        expect(find.text(l10n.timeline), findsOneWidget);
        expect(find.text('Prefers weekly invoicing'), findsOneWidget);
        expect(find.text(l10n.yourNotes), findsOneWidget);
        final compact = v.sizeClass == SizeClass.compact;
        expect(
          find.bySemanticsLabel(l10n.entityViews),
          compact ? findsOneWidget : findsNothing,
        );
        expect(
          find.text(l10n.refreshInsights),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.text(l10n.mentioningNotes),
          v.sizeClass == SizeClass.expanded ? findsOneWidget : findsNothing,
        );
        if (v.textScale == 1) await expectAccessible(tester);
        // A cited bullet opens its source block.
        final citation = find
            .bySemanticsLabel(
              lookupStrataLocalizations(v.locale)
                  .citationSemantics(label: 'Call 2026-09-12 — Acme'),
            )
            .first;
        await tester.ensureVisible(citation);
        await tester.pump();
        await tester.tap(citation);
        expect(notes, [('n-call-2026-09-12-acme', 'a1b2')]);
        if (compact) {
          await tester.tap(find.byTooltip(l10n.backToPeople));
          expect(back, 1);
        }
      });
    }
  });

  group('EntityScreen interaction', () {
    final compact = variants(scales: const [1]).first;
    final expanded = variants(
      sizes: const {'expanded': Size(1440, 900)},
      scales: const [1],
    ).first;

    testWidgets('compact tabs show mentions and the graph', (tester) async {
      final fake = _fake();
      final notes = <(String, String?)>[];
      await pumpVariant(
        tester,
        compact,
        EntityScreen(_id, onOpenNote: (id, a) => notes.add((id, a))),
        fake,
      );
      await tester.tap(find.text('Notes · 1'));
      await tester.pumpAndSettle();
      expect(find.text('Call 2026-09-12 — Acme'), findsOneWidget);
      await tester.tap(find.text('Call 2026-09-12 — Acme'));
      expect(notes, [('n-call-2026-09-12-acme', null)]);
      await tester.tap(find.text('Graph'));
      await tester.pumpAndSettle();
      expect(
        fake.calls,
        contains(const CoreCall('watchLocalGraph', {'id': _id, 'depth': 1})),
      );
      await expectAccessible(tester);
    });

    testWidgets('refresh insights requests a relink', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, expanded, const EntityScreen(_id), fake);
      await tester.tap(find.text('Refresh insights'));
      await tester.pump();
      expect(fake.calls.last, const CoreCall('requestRelink', {'noteId': _id}));
      expect(find.text('Insights refresh queued'), findsOneWidget);
    });

    testWidgets('compact menu refreshes too', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, compact, const EntityScreen(_id), fake);
      await tester.tap(find.byTooltip('More actions'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Refresh insights'));
      await tester.pumpAndSettle();
      expect(
        fake.calls,
        contains(const CoreCall('requestRelink', {'noteId': _id})),
      );
    });

    testWidgets('rejecting an AI link removes the relation', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, expanded, const EntityScreen(_id), fake);
      await tester.tap(
        find.byTooltip('Reject relation works-at Acme Logistics'),
      );
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('removeRelation', {
          'srcId': _id,
          'dstId': 'c-acme-logistics',
          'relType': 'works-at',
        }),
      );
      final repoint = tester.widget<IconButton>(
        find.widgetWithIcon(IconButton, Icons.alt_route),
      );
      expect(repoint.onPressed, isNull);
    });
  });

  group('EntityScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(tester, v, const EntityScreen(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const EntityScreen('x-gone'),
          _fake(DirFixtures.notFound),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).notFoundTitle),
          findsOneWidget,
        );
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(tester, v, const EntityScreen(_id), fake);
        fake.entity[_id].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await tester.pump();
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });

      testWidgets('offline edits and an empty page $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const EntityScreen(_id),
          _fake(DirFixtures.ahmedPending),
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.pendingSync), findsOneWidget);
        expect(find.text(l10n.noSummary), findsOneWidget);
        expect(find.text('Watanya contract'), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('a document ID renders the document page $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const EntityScreen('d-watanya-contract'),
          _fake(StrataFixtures.entityScreenDocument),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).whereItIs),
          findsOneWidget,
        );
      });

      testWidgets('a place ID renders the place page $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const EntityScreen('pl-nasr-city-office'),
          _fake(StrataFixtures.entityScreenPlace),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).everythingHere),
          findsOneWidget,
        );
      });
    }
  });
}
