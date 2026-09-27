import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

const _id = 'd-watanya-contract';

FakeCoreApi _fake([EntityScreen? screen]) {
  final fake = FakeCoreApi();
  fake.entity[screen?.id ?? _id].add(
    screen ?? StrataFixtures.entityScreenDocument,
  );
  return fake;
}

void main() {
  group('DocumentScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = _fake();
        final opened = <String>[];
        final notes = <(String, String?)>[];
        var back = 0;
        await pumpVariant(
          tester,
          v,
          DocumentScreen(
            _id,
            onOpenEntity: opened.add,
            onOpenNote: (id, anchor) => notes.add((id, anchor)),
            onBack: () => back++,
          ),
          fake,
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
        );
        expect(find.text('Watanya contract'), findsWidgets);
        expect(find.text('عقد وطنية'), findsOneWidget);
        expect(find.text(l10n.whereItIs), findsOneWidget);
        expect(find.text(l10n.custodyHistory), findsOneWidget);
        expect(find.text(l10n.statusStored), findsOneWidget);
        expect(find.text(l10n.nobodyHasIt), findsOneWidget);
        expect(find.bySemanticsLabel(l10n.locationLabel), findsOneWidget);
        final compact = v.sizeClass == SizeClass.compact;
        expect(
          find.byTooltip(l10n.backToDocuments),
          compact ? findsOneWidget : findsNothing,
        );
        expect(
          find.bySemanticsLabel(l10n.contextLabel),
          v.sizeClass == SizeClass.expanded ? findsOneWidget : findsNothing,
        );
        expect(
          find.byTooltip(l10n.showContext),
          v.sizeClass == SizeClass.medium ? findsOneWidget : findsNothing,
        );
        if (v.textScale == 1) await expectAccessible(tester);
        // The place breadcrumb and the last holder open their pages.
        final shady = find.bySemanticsLabel('Shady').first;
        await tester.ensureVisible(shady);
        await tester.pump();
        await tester.tap(shady);
        expect(opened, ['p-shady']);
        final office = find.bySemanticsLabel('Nasr City office').first;
        await tester.ensureVisible(office);
        await tester.pump();
        await tester.tap(office);
        expect(opened.last, 'pl-nasr-city-office');
        // A custody citation opens its block.
        final chip = find.bySemanticsLabel(
          lookupStrataLocalizations(v.locale)
              .citationSemantics(label: '2026-09-20 18-05'),
        );
        await tester.ensureVisible(chip);
        await tester.pump();
        await tester.tap(chip);
        expect(notes, [('n-capture-watanya-safe', 'c7d8')]);
        if (compact) {
          await tester.ensureVisible(find.byTooltip(l10n.backToDocuments));
          await tester.tap(find.byTooltip(l10n.backToDocuments));
          expect(back, 1);
        }
      });
    }
  });

  group('DocumentScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(tester, v, const DocumentScreen(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const DocumentScreen('x-gone'),
          _fake(DocFixtures.notFound),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).notFoundTitle),
          findsOneWidget,
        );
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(tester, v, const DocumentScreen(_id), fake);
        fake.entity[_id].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await tester.pump();
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });

      testWidgets('checked out with a holder $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const DocumentScreen('d-car-licence'),
          _fake(DocFixtures.carLicence),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.statusCheckedOut), findsOneWidget);
        expect(find.text(l10n.withLabel), findsOneWidget);
        expect(find.text(l10n.nobodyHasIt), findsNothing);
        await expectAccessible(tester);
      });

      testWidgets('no location, custody or copies $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const DocumentScreen('d-bare'),
          _fake(DocFixtures.bareDocument),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.locationUnknown), findsOneWidget);
        expect(find.text(l10n.noCustody), findsOneWidget);
        await expectAccessible(tester);
      });
    }
  });

  group('Record a move', () {
    for (final v in variants()) {
      testWidgets('form $v', (tester) async {
        final fake = _fake();
        fake.directory[(DirectoryTab.places, '')].add(
          const DirectoryView(
            tab: DirectoryTab.places,
            query: '',
            items: [
              DirectoryItem(
                id: 'pl-nasr-city-office',
                title: 'Nasr City office',
                aliases: ['مكتب مدينة نصر'],
              ),
              DirectoryItem(
                id: 'pl-nasr-city-cabinet-b',
                title: 'Cabinet B',
                subtitle: 'Nasr City office',
                aliases: [],
              ),
            ],
            counts: StrataFixtures.directoryCounts,
          ),
        );
        fake.directory[(DirectoryTab.people, '')].add(
          StrataFixtures.directoryView,
        );
        await pumpVariant(tester, v, const DocumentScreen(_id), fake);
        final l10n = lookupDocumentsLocalizations(v.locale);
        final record = find.text(l10n.recordMove).first;
        await tester.ensureVisible(record);
        await tester.pumpAndSettle();
        await tester.tap(record);
        await tester.pumpAndSettle();
        expectNoErrors(tester);
        expect(find.text(l10n.whatHappened), findsOneWidget);
        expect(
          find.text(
            l10n.recordMoveSubtitle(
              document: 'Watanya contract',
              place: 'Safe — Nasr City office',
            ),
          ),
          findsOneWidget,
        );
        expect(
          fake.calls,
          containsAll(const [
            CoreCall('watchDirectory', {
              'tab': DirectoryTab.places,
              'query': '',
            }),
            CoreCall('watchDirectory', {
              'tab': DirectoryTab.people,
              'query': '',
            }),
          ]),
        );
        expect(find.text(l10n.recordMoveUnavailable), findsOneWidget);
        final submit = tester.widget<FilledButton>(
          find.widgetWithText(FilledButton, l10n.recordMoveSubmit),
        );
        expect(submit.onPressed, isNull);
        if (v.textScale == 1) {
          await tester.tap(find.text(l10n.eventHandedTo));
          await tester.pump();
          final chip = tester.widget<ChoiceChip>(
            find.widgetWithText(ChoiceChip, l10n.eventHandedTo),
          );
          expect(chip.selected, isTrue);
          await tester.ensureVisible(find.text('Cabinet B'));
          await tester.tap(find.text('Cabinet B'));
          await tester.pump();
          await expectAccessible(tester);
        }
        await tester.tap(find.text(l10n.cancel));
        await tester.pumpAndSettle();
        expect(find.text(l10n.whatHappened), findsNothing);
        // Nothing was recorded: there is no custody intent yet.
        expect(fake.calls.where((c) => !c.method.startsWith('watch')), isEmpty);
      });
    }
  });
}
