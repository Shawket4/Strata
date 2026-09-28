import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';

const _id = 'pl-nasr-city-office';

FakeCoreApi _fake([EntityScreen? screen]) {
  final fake = FakeCoreApi()
    ..placeOptionsAnswer.returns(StrataFixtures.placeOptions);
  fake.entity[screen?.id ?? _id].add(
    screen ?? StrataFixtures.entityScreenPlace,
  );
  return fake;
}

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  Widget page, [
  FakeCoreApi? fake,
]) => pumpVariant(tester, v, page, fake: fake ?? _fake(), scaffold: true);

void main() {
  group('PlaceScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final opened = <String>[];
        final fake = await _pump(
          tester,
          v,
          PlaceScreen(_id, onOpenEntity: opened.add, onBack: () {}),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
        );
        expect(
          directionOf(tester, find.byType(PlacePage)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expect(find.text('Nasr City office'), findsWidgets);
        expect(find.text(l10n.everythingHere), findsOneWidget);
        expect(find.text(l10n.recentMovements), findsOneWidget);
        expect(find.text(l10n.topLevelPlace), findsOneWidget);
        expect(
          find.bySemanticsLabel(
            l10n.placesTreeLabel(place: 'Nasr City office'),
          ),
          findsOneWidget,
        );
        // The core's tree at every depth with document counts.
        expect(find.text('Top shelf'), findsOneWidget);
        expect(find.text(l10n.placeDocuments(count: 1)), findsNWidgets(2));
        expect(find.text(l10n.placeDocuments(count: 0)), findsOneWidget);
        // Out with people, and a movement at a nested place.
        expect(find.text(l10n.outWithPeople), findsOneWidget);
        expect(find.text('Watanya contract — copy'), findsOneWidget);
        expect(
          find.text(l10n.atPlace(place: 'Safe — Nasr City office')),
          findsOneWidget,
        );
        expect(find.text(l10n.userNotesEmpty), findsOneWidget);
        final compact = v.sizeClass == SizeClass.compact;
        expect(
          find.bySemanticsLabel(
            l10n.everythingHereCaption(place: 'Nasr City office'),
          ),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.text('places/Nasr City office.md'),
          compact ? findsNothing : findsOneWidget,
        );
        expect(find.text(l10n.recordMove), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);
        await tapVisible(tester, find.bySemanticsLabel('Top shelf').first);
        expect(opened, ['pl-cabinet-b-top']);
      });
    }
  });

  group('PlaceScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await _pump(tester, v, const PlaceScreen(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await _pump(
          tester,
          v,
          const PlaceScreen('x-gone'),
          _fake(DocFixtures.notFound),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).notFoundTitle),
          findsOneWidget,
        );
      });

      testWidgets('nested and empty $v', (tester) async {
        await _pump(
          tester,
          v,
          const PlaceScreen('pl-nasr-city-safe'),
          _fake(DocFixtures.safe),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          find.text(l10n.partOf(place: 'Nasr City office')),
          findsOneWidget,
        );
        expect(find.text(l10n.noDocumentsHere), findsOneWidget);
        expect(find.text(l10n.noMovements), findsOneWidget);
        expect(find.text(l10n.noSubPlaces), findsOneWidget);
        expect(find.text(l10n.nobodyOut), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('record a move lists the documents here $v', (tester) async {
        final fake = await _pump(tester, v, const PlaceScreen(_id));
        final l10n = lookupDocumentsLocalizations(v.locale);
        await tapVisible(tester, find.text(l10n.recordMove).first);
        expect(find.text(l10n.whatHappened), findsOneWidget);
        expect(find.text('Petrol Arrows commercial register'), findsWidgets);
        expect(
          fake.calls,
          contains(const CoreCall('placeOptions', {'documentId': null})),
        );
        // Without a document chosen, nothing is recorded.
        await tapVisible(tester, find.text(l10n.recordMoveSubmit));
        expect(find.text(l10n.chooseDocument), findsOneWidget);
        expect(
          fake.calls.map((c) => c.method),
          isNot(contains('recordCustody')),
        );
      });
    }
  });
}
