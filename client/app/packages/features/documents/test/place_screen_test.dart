import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

const _id = 'pl-nasr-city-office';

FakeCoreApi _fake([EntityScreen? screen]) {
  final fake = FakeCoreApi();
  fake.entity[screen?.id ?? _id].add(
    screen ?? StrataFixtures.entityScreenPlace,
  );
  return fake;
}

void main() {
  group('PlaceScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = _fake();
        final opened = <String>[];
        await pumpVariant(
          tester,
          v,
          PlaceScreen(_id, onOpenEntity: opened.add, onBack: () {}),
          fake,
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
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
        final compact = v.sizeClass == SizeClass.compact;
        expect(
          find.bySemanticsLabel(
            l10n.everythingHereCaption(place: 'Nasr City office'),
          ),
          compact ? findsNothing : findsOneWidget,
        );
        expect(find.text(l10n.recordMove), findsOneWidget);
        if (v.textScale == 1) await expectAccessible(tester);
        await tester.ensureVisible(find.bySemanticsLabel('Cabinet B').first);
        await tester.tap(find.bySemanticsLabel('Cabinet B').first);
        expect(opened, ['pl-nasr-city-cabinet-b']);
      });
    }
  });

  group('PlaceScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(tester, v, const PlaceScreen(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await pumpVariant(
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
        await pumpVariant(
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
        await expectAccessible(tester);
      });

      testWidgets('record a move lists the documents here $v', (tester) async {
        await pumpVariant(tester, v, const PlaceScreen(_id), _fake());
        final l10n = lookupDocumentsLocalizations(v.locale);
        await tester.tap(find.text(l10n.recordMove));
        await tester.pumpAndSettle();
        expect(find.text(l10n.whatHappened), findsOneWidget);
        expect(find.text('Petrol Arrows commercial register'), findsWidgets);
      });
    }
  });
}
