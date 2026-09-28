import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';

typedef _Key = (DirectoryTab, String, DirectoryFilter, DirectorySort);

_Key _key(DirectoryTab tab, [String query = '']) =>
    (tab, query, emptyDirectoryFilter, DirectorySort.name);

FakeCoreApi _fake() {
  final fake = FakeCoreApi();
  fake.directoryFiltered[_key(DirectoryTab.people)].add(DirFixtures.people);
  fake.directoryFiltered[_key(DirectoryTab.people, 'أحمد')].add(
    DirFixtures.peopleSearch,
  );
  fake.directoryFiltered[_key(DirectoryTab.companies)].add(
    DirFixtures.companies,
  );
  fake.directoryFiltered[_key(DirectoryTab.documents)].add(
    DirFixtures.documents,
  );
  fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
  fake.entity['p-ahmed-samir'].add(StrataFixtures.entityScreen);
  fake.entity['d-watanya-contract'].add(StrataFixtures.entityScreenDocument);
  return fake;
}

Variant _at(Size size) =>
    variants(sizes: {'at': size}, scales: const [1]).first;

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  Widget screen, [
  FakeCoreApi? fake,
]) => pumpVariant(tester, v, screen, fake: fake ?? _fake(), scaffold: true);

/// The directory row list's scrollable (compact and medium).
Future<void> _reveal(WidgetTester tester, Finder finder) async {
  final list = find.byType(CustomScrollView);
  if (list.evaluate().isNotEmpty) {
    await tester.scrollUntilVisible(
      finder,
      100,
      scrollable: find
          .descendant(of: list, matching: find.byType(Scrollable))
          .first,
    );
  }
}

void main() {
  group('DirectoryScreen matrix', () {
    for (final v in variants()) {
      testWidgets('people $v', (tester) async {
        final opened = <String>[];
        final fake = await _pump(
          tester,
          v,
          DirectoryScreen(onOpenEntity: opened.add),
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(
            const CoreCall('watchDirectoryFiltered', {
              'tab': DirectoryTab.people,
              'query': '',
              'filter': emptyDirectoryFilter,
              'sort': DirectorySort.name,
            }),
          ),
        );
        expect(fake.calls.map((c) => c.method), isNot(contains('watchInbox')));
        expect(
          directionOf(tester, find.byType(DirectoryScreen)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expect(find.bySemanticsLabel(l10n.tabsLabel), findsOneWidget);
        expect(find.text(l10n.tabDocuments), findsOneWidget);
        // The tab's suggestion, with its answers.
        expect(find.text(l10n.whoIs(mention: 'أبو علي')), findsOneWidget);
        expect(
          find.text('Called “Abu Ali” in two notes about Acme.'),
          findsOneWidget,
        );
        expect(find.text('أبو علي هيبعت العقد بكرة'), findsOneWidget);
        expect(find.text(l10n.linkTo(title: 'Ahmed Samir')), findsOneWidget);
        expect(find.text(l10n.createPerson), findsOneWidget);
        // The core's filter chips and the sort order.
        expect(find.bySemanticsLabel(l10n.filtersLabel), findsOneWidget);
        expect(find.text('Operations manager · 1'), findsOneWidget);
        expect(find.text('Acme Logistics · 1'), findsOneWidget);
        expect(find.text('Open items · 1'), findsOneWidget);
        expect(find.byTooltip(l10n.sortBy), findsOneWidget);
        expect(find.text(l10n.sortName), findsOneWidget);
        await _reveal(tester, find.text('Ahmed Samir'));
        expect(find.text('Ahmed Samir'), findsOneWidget);
        expect(find.text('أحمد سمير'), findsWidgets);
        expect(find.text('Operations manager, Acme Logistics'), findsOneWidget);
        expect(find.text('AS'), findsOneWidget);
        final table = find.bySemanticsLabel(
          l10n.tableCaption(tab: l10n.tabPeople),
        );
        switch (v.sizeClass) {
          case SizeClass.compact || SizeClass.medium:
            expect(table, findsNothing);
            expect(find.text('All people · A–Z'), findsOneWidget);
            expect(
              find.text(l10n.rowActivity(count: 12, when: '2 days ago')),
              findsOneWidget,
            );
            expect(
              find.text(l10n.selectSomething),
              v.sizeClass == SizeClass.medium ? findsOneWidget : findsNothing,
            );
          case SizeClass.expanded:
            expect(table, findsOneWidget);
            expect(find.text(l10n.colActivity), findsOneWidget);
            expect(find.text(l10n.mentionCount(count: 12)), findsOneWidget);
            expect(find.text('2 days ago'), findsOneWidget);
            expect(find.text(l10n.newPerson), findsOneWidget);
        }
        await expectAccessible(tester, contrast: v.textScale == 1);
        await tapVisible(tester, find.text('Ahmed Samir'));
        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(opened, ['p-ahmed-samir']);
          case SizeClass.medium:
            expect(opened, isEmpty);
            expect(find.text(l10n.summary), findsOneWidget);
          case SizeClass.expanded:
            expect(
              find.bySemanticsLabel(l10n.previewOf(title: 'Ahmed Samir')),
              findsOneWidget,
            );
            await tester.tap(find.text(l10n.openPage));
            expect(opened, ['p-ahmed-samir']);
        }
      });
    }
  });

  group('DirectoryScreen interaction', () {
    final compact = _at(StrataTestSizes.compact);
    final expanded = _at(StrataTestSizes.expanded);

    testWidgets('tabs and search re-query the core', (tester) async {
      final fake = await _pump(tester, compact, const DirectoryScreen());
      await tester.enterText(find.byType(TextField), 'أحمد');
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('watchDirectoryFiltered', {
          'tab': DirectoryTab.people,
          'query': 'أحمد',
          'filter': emptyDirectoryFilter,
          'sort': DirectorySort.name,
        }),
      );
      expect(find.text('Mona Hassan'), findsNothing);
      await tester.tap(find.text('Companies'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('watchDirectoryFiltered', {
          'tab': DirectoryTab.companies,
          'query': '',
          'filter': emptyDirectoryFilter,
          'sort': DirectorySort.name,
        }),
      );
      expect(find.text('Acme Logistics'), findsOneWidget);
    });

    testWidgets('a filter chip re-queries with the facet applied', (
      tester,
    ) async {
      final fake = await _pump(tester, compact, const DirectoryScreen());
      await tapVisible(tester, find.text('Operations manager · 1'));
      final call = fake.calls.last;
      expect(call.method, 'watchDirectoryFiltered');
      expect(call.args['tab'], DirectoryTab.people);
      expect(call.args['sort'], DirectorySort.name);
      final filter = call.args['filter']! as DirectoryFilter;
      expect(filter.role, 'Operations manager');
      expect(filter.tags, isEmpty);
      expect(filter.companyId, isNull);
      expect(filter.hasOpenItems, isFalse);
      fake
          .directoryFiltered[(
            DirectoryTab.people,
            '',
            filter,
            DirectorySort.name,
          )]
          .add(DirFixtures.peopleByRole);
      await settle(tester);
      expect(find.text('Results'), findsOneWidget);
      expect(find.text('Mona Hassan'), findsNothing);
      final chip = tester.widget<FilterChip>(find.byType(FilterChip));
      expect(chip.selected, isTrue);
      // Tapping the selected chip clears the facet.
      await tapVisible(tester, find.text('Operations manager · 1'));
      final cleared = fake.calls.last.args['filter']! as DirectoryFilter;
      expect(cleared.role, isNull);
    });

    testWidgets('sort by last activity', (tester) async {
      final fake = await _pump(tester, compact, const DirectoryScreen());
      await tester.tap(find.byTooltip('Sort'));
      await settle(tester);
      expect(find.text('Recently moved'), findsNothing);
      await tester.tap(find.text('Last active').last);
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('watchDirectoryFiltered', {
          'tab': DirectoryTab.people,
          'query': '',
          'filter': emptyDirectoryFilter,
          'sort': DirectorySort.lastActive,
        }),
      );
    });

    testWidgets('who-is links, creates or is dismissed', (tester) async {
      final fake = await _pump(tester, compact, const DirectoryScreen());
      await tapVisible(tester, find.text("It's Ahmed Samir"));
      expect(
        fake.calls.last,
        const CoreCall('resolveLinkOrCreate', {
          'id': 's-who-is-abu-ali',
          'choice': LinkOrCreateChoice(
            kind: LinkOrCreateKind.link,
            entityId: 'p-ahmed-samir',
            force: false,
          ),
        }),
      );
      fake.resolveLinkOrCreateAnswer.returns(
        StrataFixtures.createOutcomeDuplicate,
      );
      await tapVisible(tester, find.text('Create person…'));
      expect(
        fake.calls.last,
        const CoreCall('resolveLinkOrCreate', {
          'id': 's-who-is-abu-ali',
          'choice': LinkOrCreateChoice(
            kind: LinkOrCreateKind.create,
            name: 'أبو علي',
            entityKind: 'person',
            force: false,
          ),
        }),
      );
      final existing =
          StrataFixtures.createOutcomeDuplicate.candidates.first.title;
      expect(find.text('Already exists: $existing'), findsOneWidget);
      fake.resolveLinkOrCreateAnswer.returns(
        StrataFixtures.createOutcomeCreated,
      );
      await tester.tap(find.text('Create anyway'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('resolveLinkOrCreate', {
          'id': 's-who-is-abu-ali',
          'choice': LinkOrCreateChoice(
            kind: LinkOrCreateKind.create,
            name: 'أبو علي',
            entityKind: 'person',
            force: true,
          ),
        }),
      );
      await tapVisible(tester, find.text('Dismiss'));
      expect(
        fake.calls.last,
        const CoreCall('rejectSuggestion', {'id': 's-who-is-abu-ali'}),
      );
    });

    testWidgets('a failed answer is reported', (tester) async {
      final fake = _fake()
        ..rejectSuggestionAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await _pump(tester, compact, const DirectoryScreen(), fake);
      await tapVisible(tester, find.text('Dismiss'));
      expect(find.text("That didn't work (offline)."), findsOneWidget);
    });

    testWidgets('expanded table moves with the keyboard', (tester) async {
      final fake = _fake();
      fake.entity['p-mona-hassan'].add(StrataFixtures.entityScreen);
      final opened = <String>[];
      await _pump(
        tester,
        expanded,
        DirectoryScreen(onOpenEntity: opened.add),
        fake,
      );
      await tester.tap(find.text('Ahmed Samir'));
      await settle(tester);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await settle(tester);
      expect(
        fake.calls,
        contains(const CoreCall('watchEntity', {'id': 'p-mona-hassan'})),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(opened, ['p-mona-hassan']);
      await tester.tap(find.byTooltip('Close preview'));
      await settle(tester);
      expect(find.text('Preview'), findsNothing);
    });

    testWidgets('merge from the preview picks, previews and merges', (
      tester,
    ) async {
      final fake = _fake();
      fake.entity['p-mona-hassan'].add(StrataFixtures.entityScreen);
      await _pump(
        tester,
        expanded,
        const DirectoryScreen(selectedId: 'p-ahmed-samir'),
        fake,
      );
      await tester.tap(find.text('Merge…'));
      await settle(tester);
      expect(find.text('Merge Ahmed Samir into…'), findsOneWidget);
      expect(
        fake.calls.last,
        const CoreCall('watchDirectory', {
          'tab': DirectoryTab.people,
          'query': '',
        }),
      );
      final self = tester.widget<ListTile>(
        find.ancestor(
          of: find.text('Ahmed Samir').last,
          matching: find.byType(ListTile),
        ),
      );
      expect(self.enabled, isFalse);
      await tester.tap(find.text('Mona Hassan').last);
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('mergePreview', {
          'sourceId': 'p-ahmed-samir',
          'intoId': 'p-mona-hassan',
        }),
      );
      expect(
        find.text('Ahmed Samir will be merged into Mona Hassan.'),
        findsOneWidget,
      );
      expect(find.text('Moves 12 mentions and 3 relations.'), findsOneWidget);
      expect(find.text('أحمد سمير'), findsWidgets);
      await expectAccessible(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Merge'));
      await settle(tester);
      expect(
        fake.calls,
        containsAllInOrder(const [
          CoreCall('mergeEntities', {
            'sourceId': 'p-ahmed-samir',
            'intoId': 'p-mona-hassan',
          }),
          CoreCall('watchEntity', {'id': 'p-mona-hassan'}),
        ]),
      );
      expect(find.byType(AlertDialog), findsNothing);
    });

    testWidgets('expanded documents tab shows the list and the page', (
      tester,
    ) async {
      await _pump(tester, expanded, const DirectoryScreen());
      await tester.tap(find.text('Documents'));
      await settle(tester);
      await tester.tap(find.text('Watanya contract'));
      await settle(tester);
      expect(find.text('Where it is'), findsOneWidget);
      expect(find.text('Car licence'), findsOneWidget);
      await tester.tap(find.byTooltip('Sort'));
      await settle(tester);
      expect(find.text('Recently moved'), findsOneWidget);
      await tester.tapAt(Offset.zero);
      await settle(tester);
      await expectAccessible(tester);
    });

    testWidgets('new person goes through the duplicate check', (tester) async {
      final fake = _fake()
        ..createEntityAnswer.returns(StrataFixtures.createOutcomeDuplicate);
      final opened = <String>[];
      await _pump(
        tester,
        expanded,
        DirectoryScreen(onOpenEntity: opened.add),
        fake,
      );
      await tester.tap(find.text('New person'));
      await settle(tester);
      final fields = find.descendant(
        of: find.byType(Dialog),
        matching: find.byType(TextField),
      );
      await tester.enterText(fields.at(0), 'Ahmed Sameer');
      await tester.enterText(fields.at(1), 'أحمد');
      await settle(tester);
      await tester.tap(find.text('Create'));
      await settle(tester);
      expect(fake.calls.last.method, 'createEntity');
      expect(fake.calls.last.args, {
        'kind': 'person',
        'name': 'Ahmed Sameer',
        'aliases': ['أحمد'],
        'force': false,
      });
      expect(find.text('Already exists'), findsOneWidget);
      fake.createEntityAnswer.returns(StrataFixtures.createOutcomeCreated);
      await tester.tap(find.text('Create anyway'));
      await settle(tester);
      expect(fake.calls.last.method, 'createEntity');
      expect(fake.calls.last.args, {
        'kind': 'person',
        'name': 'Ahmed Sameer',
        'aliases': ['أحمد'],
        'force': true,
      });
      expect(opened, [StrataFixtures.createOutcomeCreated.id]);
    });
  });

  group('DirectoryScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await _pump(tester, v, const DirectoryScreen(), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('empty $v', (tester) async {
        final fake = FakeCoreApi();
        fake.directoryFiltered[_key(DirectoryTab.people)].add(
          DirFixtures.emptyPeople,
        );
        await _pump(tester, v, const DirectoryScreen(), fake);
        final l10n = lookupDirectoryLocalizations(v.locale);
        expect(find.text(l10n.emptyPeople), findsOneWidget);
        expect(find.text(l10n.emptyMessage), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('no matches $v', (tester) async {
        final fake = FakeCoreApi();
        fake.directoryFiltered[_key(DirectoryTab.people)].add(
          DirFixtures.emptyPeople,
        );
        fake.directoryFiltered[_key(DirectoryTab.people, 'zz')].add(
          DirFixtures.noMatches,
        );
        await _pump(tester, v, const DirectoryScreen(), fake);
        await tester.enterText(find.byType(TextField), 'zz');
        await settle(tester);
        final l10n = lookupDirectoryLocalizations(v.locale);
        expect(find.text(l10n.noMatches(query: 'zz')), findsOneWidget);
        expect(find.text(l10n.noMatchesMessage), findsOneWidget);
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await _pump(tester, v, const DirectoryScreen(), fake);
        fake.directoryFiltered[_key(DirectoryTab.people)].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await settle(tester);
        final docs = lookupDocumentsLocalizations(v.locale);
        expect(find.text(docs.errorTitle), findsOneWidget);
        expect(find.text(docs.errorMessage(code: 'store')), findsOneWidget);
      });
    }
  });
}
