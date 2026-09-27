import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_state/strata_state.dart' hide EntityScreen;
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

FakeCoreApi _fake() {
  final fake = FakeCoreApi();
  fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
  fake.directory[(DirectoryTab.people, 'أحمد')].add(DirFixtures.peopleSearch);
  fake.directory[(DirectoryTab.companies, '')].add(DirFixtures.companies);
  fake.directory[(DirectoryTab.documents, '')].add(DirFixtures.documents);
  fake.inbox.add(StrataFixtures.inboxView);
  fake.entity['p-ahmed-samir'].add(StrataFixtures.entityScreen);
  fake.entity['d-watanya-contract'].add(StrataFixtures.entityScreenDocument);
  return fake;
}

Variant _at(String size) => variants(
  sizes: {size: StrataTestSizesByName.of(size)},
  scales: const [1],
).first;

/// Size lookup by matrix name.
abstract final class StrataTestSizesByName {
  static Size of(String name) => switch (name) {
    'compact' => const Size(390, 844),
    'medium' => const Size(1024, 768),
    'expanded' => const Size(1440, 900),
    _ => const Size(1920, 1080),
  };
}

void main() {
  group('DirectoryScreen matrix', () {
    for (final v in variants()) {
      testWidgets('people $v', (tester) async {
        final fake = _fake();
        final opened = <String>[];
        await pumpVariant(
          tester,
          v,
          DirectoryScreen(onOpenEntity: opened.add),
          fake,
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          containsAll(const [
            CoreCall('watchDirectory', {
              'tab': DirectoryTab.people,
              'query': '',
            }),
            CoreCall('watchInbox'),
          ]),
        );
        expect(find.bySemanticsLabel(l10n.tabsLabel), findsOneWidget);
        expect(find.text(l10n.tabDocuments), findsOneWidget);
        expect(find.text(l10n.whoIs(mention: 'بابا')), findsOneWidget);
        expect(find.bySemanticsLabel(l10n.filtersLabel), findsOneWidget);
        final list = find.byType(CustomScrollView);
        if (list.evaluate().isNotEmpty) {
          await tester.scrollUntilVisible(
            find.text('Ahmed Samir'),
            100,
            scrollable: find
                .descendant(of: list, matching: find.byType(Scrollable))
                .first,
          );
        }
        expect(find.text('Ahmed Samir'), findsOneWidget);
        expect(find.text('أحمد سمير'), findsOneWidget);
        expect(find.text('Operations manager, Acme Logistics'), findsOneWidget);
        final table = find.bySemanticsLabel(
          l10n.tableCaption(tab: l10n.tabPeople),
        );
        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(table, findsNothing);
            expect(find.text(l10n.selectSomething), findsNothing);
          case SizeClass.medium:
            expect(table, findsNothing);
            expect(find.text(l10n.selectSomething), findsOneWidget);
          case SizeClass.expanded:
            expect(table, findsOneWidget);
            expect(find.text(l10n.newPerson), findsOneWidget);
        }
        if (v.textScale == 1) await expectAccessible(tester);
        await tester.ensureVisible(find.text('Ahmed Samir'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Ahmed Samir'));
        await tester.pump();
        await tester.pump();
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
    testWidgets('tabs and search re-query the core', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, _at('compact'), const DirectoryScreen(), fake);
      await tester.enterText(find.byType(TextField), 'أحمد');
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('watchDirectory', {
          'tab': DirectoryTab.people,
          'query': 'أحمد',
        }),
      );
      await tester.pump();
      expect(find.text('Mona Hassan'), findsNothing);
      await tester.tap(find.text('Companies'));
      await tester.pump();
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('watchDirectory', {
          'tab': DirectoryTab.companies,
          'query': '',
        }),
      );
      expect(find.text('Acme Logistics'), findsOneWidget);
    });

    testWidgets('suggestions are accepted or dismissed', (tester) async {
      final fake = _fake();
      await pumpVariant(tester, _at('compact'), const DirectoryScreen(), fake);
      await tester.tap(find.text('Dismiss'));
      expect(
        fake.calls.last,
        const CoreCall('rejectSuggestion', {'id': 's-who-is-baba'}),
      );
      await tester.tap(find.text('Create person…'));
      expect(
        fake.calls.last,
        const CoreCall('acceptSuggestion', {'id': 's-who-is-baba'}),
      );
    });

    testWidgets('expanded table moves with the keyboard', (tester) async {
      final fake = _fake();
      fake.entity['p-mona-hassan'].add(StrataFixtures.entityScreen);
      final opened = <String>[];
      await pumpVariant(
        tester,
        _at('expanded'),
        DirectoryScreen(onOpenEntity: opened.add),
        fake,
      );
      await tester.tap(find.text('Ahmed Samir'));
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(
        fake.calls,
        contains(const CoreCall('watchEntity', {'id': 'p-mona-hassan'})),
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(opened, ['p-mona-hassan']);
      await tester.tap(find.byTooltip('Close preview'));
      await tester.pump();
      expect(find.text('Preview'), findsNothing);
    });

    testWidgets('expanded documents tab shows the list and the page', (
      tester,
    ) async {
      final fake = _fake();
      await pumpVariant(tester, _at('expanded'), const DirectoryScreen(), fake);
      await tester.tap(find.text('Documents'));
      await tester.pump();
      await tester.pump();
      await tester.tap(find.text('Watanya contract'));
      await tester.pump();
      await tester.pump();
      expect(find.text('Where it is'), findsOneWidget);
      expect(find.text('Car licence'), findsOneWidget);
      await expectAccessible(tester);
    });

    testWidgets('new person goes through the duplicate check', (tester) async {
      final fake = _fake()
        ..createEntityAnswer.returns(StrataFixtures.createOutcomeDuplicate);
      final opened = <String>[];
      await pumpVariant(
        tester,
        _at('expanded'),
        DirectoryScreen(onOpenEntity: opened.add),
        fake,
      );
      await tester.tap(find.text('New person'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField).at(1), 'Ahmed Sameer');
      await tester.enterText(find.byType(TextField).at(2), 'أحمد');
      await tester.pump();
      await tester.tap(find.text('Create'));
      await tester.pumpAndSettle();
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
      await tester.pumpAndSettle();
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
        await pumpVariant(tester, v, const DirectoryScreen(), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('empty $v', (tester) async {
        final fake = FakeCoreApi();
        fake.directory[(DirectoryTab.people, '')].add(DirFixtures.emptyPeople);
        await pumpVariant(tester, v, const DirectoryScreen(), fake);
        final l10n = lookupDirectoryLocalizations(v.locale);
        expect(find.text(l10n.emptyPeople), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('no matches $v', (tester) async {
        final fake = FakeCoreApi();
        fake.directory[(DirectoryTab.people, '')].add(DirFixtures.emptyPeople);
        fake.directory[(DirectoryTab.people, 'zz')].add(DirFixtures.noMatches);
        await pumpVariant(tester, v, const DirectoryScreen(), fake);
        await tester.enterText(find.byType(TextField), 'zz');
        await tester.pump();
        await tester.pump();
        expect(
          find.text(
            lookupDirectoryLocalizations(v.locale).noMatches(query: 'zz'),
          ),
          findsOneWidget,
        );
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(tester, v, const DirectoryScreen(), fake);
        fake.directory[(DirectoryTab.people, '')].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await tester.pump();
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });
    }
  });
}
