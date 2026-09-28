import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';

const _id = 'd-watanya-contract';

FakeCoreApi _fake([EntityScreen? screen]) {
  final fake = FakeCoreApi()
    ..placeOptionsAnswer.returns(StrataFixtures.placeOptions);
  fake.entity[screen?.id ?? _id].add(
    screen ?? StrataFixtures.entityScreenDocument,
  );
  fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
  fake.directory[(DirectoryTab.companies, '')].add(
    const DirectoryView(
      tab: DirectoryTab.companies,
      query: '',
      items: [
        DirectoryItem(
          id: 'c-acme-logistics',
          title: 'Acme Logistics',
          aliases: [],
          kind: 'company',
          titleDir: TextDir.ltr,
          initials: 'AL',
          mentionCount: 7,
          tags: [],
          location: [],
          expiringSoon: false,
          breadcrumb: [],
          documentCount: 0,
          hasOpenItems: false,
        ),
      ],
      counts: StrataFixtures.directoryCounts,
      filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
      sort: DirectorySort.name,
      filterOptions: [],
      sections: [],
      suggestions: [],
      expiringCount: 0,
    ),
  );
  return fake;
}

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  Widget page, [
  FakeCoreApi? fake,
]) => pumpVariant(tester, v, page, fake: fake ?? _fake(), scaffold: true);

final Variant _compact = variants(scales: const [1]).first;
final Variant _expanded = variants(
  sizes: const {'expanded': StrataTestSizes.expanded},
  scales: const [1],
).first;

Future<void> _openRecordMove(WidgetTester tester, [Variant? v]) async {
  final l10n = lookupDocumentsLocalizations(v?.locale ?? const Locale('en'));
  await tapVisible(tester, find.text(l10n.recordMove).first);
}

Future<void> _revealInForm(WidgetTester tester, Finder finder) =>
    tester.scrollUntilVisible(
      finder,
      100,
      scrollable: find
          .descendant(
            of: find.byType(RecordMoveForm),
            matching: find.byType(Scrollable),
          )
          .first,
    );

void main() {
  group('DocumentScreen matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final opened = <String>[];
        final notes = <(String, String?)>[];
        var back = 0;
        final fake = await _pump(
          tester,
          v,
          DocumentScreen(
            _id,
            onOpenEntity: opened.add,
            onOpenNote: (id, anchor) => notes.add((id, anchor)),
            onBack: () => back++,
          ),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
        );
        expect(
          directionOf(tester, find.byType(DocumentPage)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expect(find.text('Watanya contract'), findsWidgets);
        expect(find.text('عقد وطنية'), findsOneWidget);
        expect(find.text(l10n.whereItIs), findsOneWidget);
        expect(find.text(l10n.custodyHistory), findsOneWidget);
        expect(find.text(l10n.statusStored), findsOneWidget);
        expect(find.text(l10n.nobodyHasIt), findsOneWidget);
        expect(find.text('Last with Shady · 20 Sep'), findsOneWidget);
        expect(find.bySemanticsLabel(l10n.locationLabel), findsOneWidget);
        // Custody: the core's sentences, dates and the AI tag.
        expect(
          find.text('Shady put it in Safe — Nasr City office'),
          findsOneWidget,
        );
        expect(find.text('20 Sep'), findsOneWidget);
        expect(find.text('Handed to Shady'), findsOneWidget);
        expect(find.text(l10n.aiConfidence(value: '0.91')), findsOneWidget);
        // Copies with where they are, and the user's notes.
        expect(find.text('Watanya contract — copy'), findsOneWidget);
        expect(find.text(l10n.withHolder(name: 'Shady')), findsOneWidget);
        expect(find.text('The copy is for the accountant.'), findsOneWidget);
        expect(find.text(l10n.userNotesEdit), findsOneWidget);
        final compact = v.sizeClass == SizeClass.compact;
        expect(find.text('Expires 31 Mar 2027'), findsOneWidget);
        expect(
          find.text('Renew the Watanya contract'),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.text('documents/Watanya contract.md'),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.text(l10n.mentioningNotes),
          v.sizeClass == SizeClass.medium ? findsNothing : findsOneWidget,
        );
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
        await expectAccessible(tester, contrast: v.textScale == 1);
        // The place breadcrumb and a concerned company open their pages.
        await tapVisible(
          tester,
          find.bySemanticsLabel('Nasr City office').first,
        );
        expect(opened, ['pl-nasr-city-office']);
        await tapVisible(tester, find.bySemanticsLabel('Watanya').first);
        expect(opened.last, 'c-watanya');
        // A custody citation opens its block.
        await tapVisible(
          tester,
          find.bySemanticsLabel(
            v.l10n.citationSemantics(label: '2026-09-20 18-05'),
          ),
        );
        expect(notes, [('n-capture-watanya-safe', 'c7d8')]);
        if (compact) {
          await tapVisible(tester, find.byTooltip(l10n.backToDocuments));
          expect(back, 1);
        }
      });
    }
  });

  group('DocumentScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await _pump(tester, v, const DocumentScreen(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await _pump(
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
        await _pump(tester, v, const DocumentScreen(_id), fake);
        fake.entity[_id].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await settle(tester);
        final l10n = lookupDocumentsLocalizations(v.locale);
        expect(find.text(l10n.errorTitle), findsOneWidget);
        expect(find.text(l10n.errorMessage(code: 'store')), findsOneWidget);
      });

      testWidgets('checked out, expiring, not synced $v', (tester) async {
        await _pump(
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
        expect(find.text(l10n.expiringSoon), findsOneWidget);
        expect(find.text(l10n.pendingSync), findsOneWidget);
        expect(find.text('Handed to Shawket'), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('no location, custody, copies or notes $v', (tester) async {
        await _pump(
          tester,
          v,
          const DocumentScreen('d-bare'),
          _fake(DocFixtures.bareDocument),
        );
        final l10n = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.locationUnknown), findsOneWidget);
        expect(find.text(l10n.noCustody), findsOneWidget);
        expect(find.text(l10n.userNotesEmpty), findsOneWidget);
        expect(
          find.text(l10n.noRenewal),
          v.sizeClass == SizeClass.compact ? findsNothing : findsOneWidget,
        );
        await expectAccessible(tester);
      });
    }
  });

  group('Record a move', () {
    for (final v in variants()) {
      testWidgets('form $v', (tester) async {
        final fake = await _pump(tester, v, const DocumentScreen(_id));
        final l10n = lookupDocumentsLocalizations(v.locale);
        await _openRecordMove(tester, v);
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
            CoreCall('placeOptions', {'documentId': _id}),
            CoreCall('watchDirectory', {
              'tab': DirectoryTab.people,
              'query': '',
            }),
          ]),
        );
        // Compact Arabic wraps the event chips so that a field label sits
        // at the sheet's scroll edge (clipped); its colours are the ones
        // checked in the left-to-right sheet.
        await expectAccessible(
          tester,
          contrast:
              v.textScale == 1 && !(v.rtl && v.sizeClass == SizeClass.compact),
        );
        await _revealInForm(tester, find.text(l10n.currentPlace));
        expect(find.text(l10n.currentPlace), findsOneWidget);
        await _revealInForm(tester, find.text('Cabinet B'));
        expect(find.text('Cabinet B'), findsOneWidget);
        expect(find.text(l10n.thirdParty), findsNothing);
        await tester.tap(find.text(l10n.cancel));
        await settle(tester);
        expect(find.text(l10n.whatHappened), findsNothing);
        expect(
          fake.calls.map((c) => c.method),
          isNot(contains('recordCustody')),
        );
      });
    }

    testWidgets('records a hand-over through the core', (tester) async {
      final fake = await _pump(tester, _compact, const DocumentScreen(_id));
      await _openRecordMove(tester);
      await tapVisible(tester, find.text('Handed to'));
      await tapVisible(tester, find.text('Cabinet B'));
      await tapVisible(tester, find.text('Mona Hassan'));
      await tapVisible(tester, find.text('Record move'));
      final call = fake.calls.last;
      expect(call.method, 'recordCustody');
      expect(call.args['documentId'], _id);
      final draft = call.args['draft']! as CustodyDraft;
      expect(draft.kind, 'handed-to');
      expect(draft.placeId, 'pl-nasr-city-cabinet-b');
      expect(draft.personId, 'p-mona-hassan');
      expect(draft.counterpartyId, isNull);
      // "Today" is left to the core (today in the account's zone).
      expect(draft.date, isNull);
      expect(draft.note, '');
      expect(find.text('What happened'), findsNothing);
      expect(find.text('Move recorded'), findsOneWidget);
    });

    testWidgets('the note is sent as typed', (tester) async {
      final fake = await _pump(tester, _compact, const DocumentScreen(_id));
      await _openRecordMove(tester);
      await tapVisible(tester, find.text('Handed to'));
      await tapVisible(tester, find.text('Mona Hassan'));
      await _revealInForm(tester, find.text('Note'));
      await tester.enterText(
        find.widgetWithText(TextField, 'Optional, e.g. for the audit'),
        'عشان المراجعة',
      );
      await tapVisible(tester, find.text('Record move'));
      final draft = fake.calls.last.args['draft']! as CustodyDraft;
      expect(
        (draft.kind, draft.date, draft.note),
        ('handed-to', null, 'عشان المراجعة'),
      );
    });

    testWidgets('sent to a third party picks the company', (tester) async {
      final fake = await _pump(tester, _expanded, const DocumentScreen(_id));
      await _openRecordMove(tester);
      await tapVisible(tester, find.text('Sent to third party'));
      await _revealInForm(tester, find.text('Third party'));
      expect(find.text('Third party'), findsOneWidget);
      expect(
        fake.calls,
        contains(
          const CoreCall('watchDirectory', {
            'tab': DirectoryTab.companies,
            'query': '',
          }),
        ),
      );
      await _revealInForm(tester, find.text('Acme Logistics'));
      await tapVisible(tester, find.text('Acme Logistics'));
      await tapVisible(tester, find.text('Record move'));
      final draft = fake.calls.last.args['draft']! as CustodyDraft;
      expect(draft.kind, 'sent-to');
      expect(draft.counterpartyId, 'c-acme-logistics');
      expect(draft.placeId, isNull);
    });

    for (final (field, message) in const [
      ('place_id', 'Choose where it went.'),
      ('person_id', 'Choose who has it.'),
      ('counterparty_id', 'Choose the third party.'),
      (null, "Couldn't record the move (invalid_input)."),
    ]) {
      testWidgets('a missing $field is explained', (tester) async {
        final fake = _fake()
          ..recordCustodyAnswer.throws(
            CoreFailure(
              code: 'invalid_input',
              messageKey: 'error.invalid_input',
              field: field,
            ),
          );
        await _pump(tester, _compact, const DocumentScreen(_id), fake);
        await _openRecordMove(tester);
        await tapVisible(tester, find.text('Record move'));
        expect(find.text(message), findsOneWidget);
        expect(find.text('What happened'), findsOneWidget);
      });
    }
  });

  group('Your notes', () {
    testWidgets('are saved through the core', (tester) async {
      final fake = await _pump(tester, _expanded, const DocumentScreen(_id));
      await tapVisible(tester, find.text('Edit notes'));
      await tester.enterText(
        find.widgetWithText(TextField, 'Your notes'),
        'Scan before lending.',
      );
      await tapVisible(tester, find.text('Save notes'));
      expect(
        fake.calls.last,
        const CoreCall('updateUserNotes', {
          'id': _id,
          'text': 'Scan before lending.',
        }),
      );
      expect(find.text('Notes saved'), findsOneWidget);
    });
  });
}
