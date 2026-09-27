import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/src/generated/ask_localizations.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

void main() {
  group('SearchScreen matrix', () {
    for (final v in variants()) {
      testWidgets('results $v', (tester) async {
        final fake = FakeCoreApi();
        fake.note['n-pricing-experiments'].add(
          NoteScreen(
            id: 'n-pricing-experiments',
            note: StrataFixtures.noteView,
          ),
        );
        final opened = <(String, String?)>[];
        await pumpVariant(
          tester,
          v,
          SearchScreen(onOpenNote: (id, anchor) => opened.add((id, anchor))),
          fake,
        );
        final l10n = lookupAskLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.searchPrompt), findsOneWidget);
        expect(fake.calls, isEmpty);
        await tester.enterText(find.byType(TextField), 'pricing');
        await tester.pump();
        await tester.pump();
        expect(
          fake.calls.last,
          const CoreCall('search', {
            'query': 'pricing',
            'mode': SearchMode.keyword,
          }),
        );
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(
          find.text('… a 5% loyalty discount on renewals …'),
          findsOneWidget,
        );
        expect(find.text(l10n.resultsCount(count: 2)), findsOneWidget);
        expect(find.bySemanticsLabel(l10n.searchMode), findsOneWidget);
        expectNoErrors(tester);
        if (v.textScale == 1) await expectAccessible(tester);
        await tester.tap(find.text('Pricing experiments'));
        await tester.pump();
        await tester.pump();
        if (v.sizeClass == SizeClass.expanded) {
          expect(opened, isEmpty);
          expect(find.bySemanticsLabel(l10n.preview), findsOneWidget);
          await tester.tap(find.text(l10n.openNote));
          expect(opened, [(StrataFixtures.noteView.id, null)]);
        } else {
          expect(opened, [('n-pricing-experiments', null)]);
        }
      });
    }
  });

  group('SearchScreen modes and states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('semantic offline $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const SearchScreen(initialQuery: 'pricing'),
          fake,
        );
        fake.searchAnswer.returns(AskFixtures.semanticOffline);
        final l10n = lookupAskLocalizations(v.locale);
        await tester.tap(find.text(l10n.modeSemantic));
        await tester.pump();
        await tester.pump();
        expect(
          fake.calls.last,
          const CoreCall('search', {
            'query': 'pricing',
            'mode': SearchMode.semantic,
          }),
        );
        expect(
          find.text(l10n.modeOffline(mode: l10n.modeSemantic)),
          findsOneWidget,
        );
        await expectAccessible(tester);
      });

      testWidgets('no results $v', (tester) async {
        final fake = FakeCoreApi()..searchAnswer.returns(AskFixtures.noResults);
        await pumpVariant(
          tester,
          v,
          const SearchScreen(initialQuery: 'zz'),
          fake,
        );
        expect(
          find.text(lookupAskLocalizations(v.locale).noResults(query: 'zz')),
          findsOneWidget,
        );
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi()
          ..searchAnswer.throws(
            const CoreFailure(code: 'store', messageKey: 'error.store'),
          );
        await pumpVariant(
          tester,
          v,
          const SearchScreen(initialQuery: 'x'),
          fake,
        );
        expect(
          find.text(lookupAskLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });
    }
  });
}
