import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_state/strata_state.dart' as vm show RelationChip;
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';

const _id = 'p-ahmed-samir';

FakeCoreApi _fake([EntityScreen? screen]) {
  final fake = FakeCoreApi();
  fake.entity[screen?.id ?? _id].add(screen ?? StrataFixtures.entityScreen);
  fake.localGraph[(_id, 1)].add(StrataFixtures.localGraphView);
  fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
  fake.directory[(DirectoryTab.companies, '')].add(DirFixtures.companies);
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

/// Ahmed's page with a relation the user added.
const _userRelation = EntityScreen(
  id: _id,
  kind: EntityPageKind.entity,
  entity: EntityView(
    id: _id,
    kind: 'person',
    title: 'Ahmed Samir',
    aliases: [],
    properties: [],
    insights: [],
    openItems: [],
    timeline: [],
    mentions: [],
    related: [
      vm.RelationChip(
        relType: 'reports-to',
        target: StrataFixtures.ahmedSamirRef,
        by: 'user',
        relLabel: 'reports to',
        createdLabel: 'Added 3 Sep',
        citations: [StrataFixtures.citation],
      ),
    ],
    documents: [],
    pendingSync: false,
    titleDir: TextDir.ltr,
    initials: 'AS',
    path: 'people/Ahmed Samir.md',
    tags: [],
    userNotes: '',
    summaryCitations: [],
    openCount: 0,
    doneCount: 0,
    mentionCount: 0,
    summaryDir: TextDir.ltr,
  ),
);

void main() {
  group('EntityPage matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final notes = <(String, String?)>[];
        final opened = <String>[];
        var back = 0;
        final fake = await _pump(
          tester,
          v,
          EntityPage(
            _id,
            onOpenNote: (id, anchor) => notes.add((id, anchor)),
            onOpenEntity: opened.add,
            onBack: () => back++,
          ),
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        final docs = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchEntity', {'id': _id})),
        );
        expect(
          directionOf(tester, find.byType(EntityPageBody)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expect(find.text('Ahmed Samir'), findsWidgets);
        expect(find.text('AS'), findsOneWidget);
        expect(
          find.text(
            l10n.entitySubtitleActive(
              subtitle: l10n.entitySubtitle(kind: l10n.kindPerson, count: 12),
              when: '2 days ago',
            ),
          ),
          findsOneWidget,
        );
        expect(find.text('client'), findsOneWidget);
        expect(find.text('role'), findsOneWidget);
        expect(find.text('Operations manager'), findsOneWidget);
        expect(
          find.byTooltip(l10n.propertyActions(key: 'role')),
          findsOneWidget,
        );
        expect(find.text(l10n.addProperty), findsOneWidget);
        expect(
          find.byTooltip(l10n.removeAlias(alias: 'Ahmed Sameer')),
          findsOneWidget,
        );
        expect(find.text(l10n.addAlias), findsOneWidget);
        expect(find.text(l10n.summary), findsOneWidget);
        expect(find.text(l10n.insights), findsOneWidget);
        expect(find.text(l10n.openItems), findsOneWidget);
        expect(find.text(l10n.openDone(open: 1, done: 2)), findsOneWidget);
        expect(find.text(l10n.timeline), findsOneWidget);
        expect(find.text('12 Sep 2026'), findsOneWidget);
        expect(find.text('Prefers weekly invoicing'), findsOneWidget);
        expect(find.text(l10n.yourNotes), findsOneWidget);
        expect(find.text('Prefers WhatsApp to email.'), findsOneWidget);
        expect(find.text(docs.userNotesEdit), findsOneWidget);
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
          find.widgetWithText(OutlinedButton, l10n.merge),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.text('people/Ahmed Samir.md'),
          compact ? findsNothing : findsOneWidget,
        );
        // Related entities: the core's relation label (compact: in the
        // overview; wider: in the context panel, shown when expanded).
        expect(
          find.text('works at'),
          v.sizeClass == SizeClass.medium ? findsNothing : findsOneWidget,
        );
        expect(
          find.text(l10n.mentioningNotes),
          v.sizeClass == SizeClass.expanded ? findsOneWidget : findsNothing,
        );
        await expectAccessible(tester, contrast: v.textScale == 1);
        // Summary citation and a cited bullet open their source block.
        final citation = find.bySemanticsLabel(
          v.l10n.citationSemantics(label: 'Call 2026-09-12 — Acme'),
        );
        expect(citation, findsWidgets);
        await tapVisible(tester, citation.first);
        expect(notes, [('n-call-2026-09-12-acme', 'a1b2')]);
        if (compact) {
          await tester.tap(find.byTooltip(l10n.backToPeople));
          expect(back, 1);
        }
      });
    }
  });

  group('EntityPage interaction', () {
    testWidgets('compact tabs show highlighted mentions and the graph', (
      tester,
    ) async {
      final notes = <(String, String?)>[];
      final fake = await _pump(
        tester,
        _compact,
        EntityPage(_id, onOpenNote: (id, a) => notes.add((id, a))),
      );
      await tester.tap(find.text('Notes · 12'));
      await settle(tester);
      expect(find.text('Call 2026-09-12 — Acme'), findsOneWidget);
      expect(find.text('Sat'), findsOneWidget);
      final snippet = tester.widget<StrataHighlightedText>(
        find.byType(StrataHighlightedText),
      );
      expect(
        snippet.text,
        'أحمد طلب invoicing أسبوعي بدل شهري ابتداءً من أكتوبر.',
      );
      expect(snippet.highlights, const [TextRange(start: 0, end: 4)]);
      expect(snippet.textDirection, TextDirection.rtl);
      await tester.tap(find.text('Call 2026-09-12 — Acme'));
      expect(notes, [('n-call-2026-09-12-acme', null)]);
      await tester.tap(find.text('Graph'));
      await settle(tester);
      expect(
        fake.calls,
        contains(const CoreCall('watchLocalGraph', {'id': _id, 'depth': 1})),
      );
      await expectAccessible(tester);
    });

    testWidgets('refresh insights requests a relink', (tester) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      await tester.tap(find.text('Refresh insights'));
      await tester.pump();
      expect(fake.calls.last, const CoreCall('requestRelink', {'noteId': _id}));
      expect(find.text('Insights refresh queued'), findsOneWidget);
    });

    testWidgets('compact menu refreshes too', (tester) async {
      final fake = await _pump(tester, _compact, const EntityPage(_id));
      await tester.tap(find.byTooltip('More actions'));
      await settle(tester);
      await tester.tap(find.text('Refresh insights'));
      await settle(tester);
      expect(
        fake.calls,
        contains(const CoreCall('requestRelink', {'noteId': _id})),
      );
    });

    testWidgets('rejecting an AI link records the rejection', (tester) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      expect(
        find.text(
          'Ahmed is described as Acme’s operations manager in the call note.',
        ),
        findsOneWidget,
      );
      await tester.tap(
        find.byTooltip('Reject relation works at Acme Logistics'),
      );
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('rejectRelation', {
          'srcId': _id,
          'dstId': 'c-acme-logistics',
          'relType': 'works-at',
        }),
      );
    });

    testWidgets('removing a user link removes the relation', (tester) async {
      final fake = await _pump(
        tester,
        _expanded,
        const EntityPage(_id),
        _fake(_userRelation),
      );
      expect(find.text('reports to'), findsOneWidget);
      expect(find.text('Added 3 Sep'), findsOneWidget);
      await tester.tap(
        find.byTooltip('Reject relation reports to Ahmed Samir'),
      );
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('removeRelation', {
          'srcId': _id,
          'dstId': 'p-ahmed-samir',
          'relType': 'reports-to',
        }),
      );
    });

    testWidgets('repoint picks another entity of the target kind', (
      tester,
    ) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      await tester.tap(find.byTooltip('Repoint Acme Logistics'));
      await settle(tester);
      expect(find.text('Point Acme Logistics to…'), findsOneWidget);
      expect(
        fake.calls.last,
        const CoreCall('watchDirectory', {
          'tab': DirectoryTab.companies,
          'query': '',
        }),
      );
      // The current target cannot be chosen.
      final current = tester.widget<ListTile>(
        find.ancestor(
          of: find.text('Acme Logistics').last,
          matching: find.byType(ListTile),
        ),
      );
      expect(current.enabled, isFalse);
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(
        fake.calls.map((c) => c.method),
        isNot(contains('repointRelation')),
      );
    });

    testWidgets('repoint to a chosen entity', (tester) async {
      final fake = _fake();
      fake.directory[(DirectoryTab.companies, '')].add(
        DirectoryView(
          tab: DirectoryTab.companies,
          query: '',
          items: [
            ...DirFixtures.companies.items,
            const DirectoryItem(
              id: 'c-nile-freight',
              title: 'Nile Freight',
              aliases: [],
              kind: 'company',
              titleDir: TextDir.ltr,
              initials: 'NF',
              mentionCount: 3,
              tags: [],
              location: [],
              expiringSoon: false,
              breadcrumb: [],
              documentCount: 0,
              hasOpenItems: false,
            ),
          ],
          counts: StrataFixtures.directoryCounts,
          filter: emptyDirectoryFilter,
          sort: DirectorySort.name,
          filterOptions: const [],
          sections: const [],
          suggestions: const [],
          expiringCount: 0,
        ),
      );
      await _pump(tester, _expanded, const EntityPage(_id), fake);
      await tester.tap(find.byTooltip('Repoint Acme Logistics'));
      await settle(tester);
      await tester.tap(find.text('Nile Freight'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('repointRelation', {
          'srcId': _id,
          'dstId': 'c-acme-logistics',
          'relType': 'works-at',
          'newDstId': 'c-nile-freight',
        }),
      );
    });

    testWidgets('aliases are added and removed', (tester) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      await tester.tap(find.byTooltip('Remove alias Ahmed Sameer'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('removeAlias', {'id': _id, 'alias': 'Ahmed Sameer'}),
      );
      await tapVisible(tester, find.text('Add alias'));
      expect(find.widgetWithText(TextField, 'Alias'), findsOneWidget);
      await tester.enterText(
        find.widgetWithText(TextField, 'Alias'),
        'أبو علي',
      );
      await tester.tap(find.text('Add'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('addAlias', {'id': _id, 'alias': 'أبو علي'}),
      );
    });

    testWidgets('a property is edited, renamed, removed and added', (
      tester,
    ) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      Future<void> menu(String item) async {
        await tester.tap(find.byTooltip('role actions'));
        await settle(tester);
        await tester.tap(find.text(item));
        await settle(tester);
      }

      // Every value goes to the core as a list (one value stays a scalar
      // there).
      void expectValues(CoreCall call, String key, List<String> values) {
        expect(call.method, 'setPropertyValues');
        expect(call.args, {'id': _id, 'key': key, 'values': values});
      }

      await menu('Edit');
      expect(find.text('Edit role'), findsOneWidget);
      final key = find.widgetWithText(TextField, 'Property');
      final value = find.widgetWithText(TextField, 'Value');
      expect(tester.widget<TextField>(key).controller!.text, 'role');
      expect(
        tester.widget<TextField>(value).controller!.text,
        'Operations manager',
      );
      await tester.enterText(value, 'COO');
      await tester.tap(find.text('Save'));
      await settle(tester);
      expectValues(fake.calls.last, 'role', ['COO']);

      await menu('Edit');
      await tester.enterText(key, 'title');
      await tester.tap(find.text('Save'));
      await settle(tester);
      expect(
        fake.calls[fake.calls.length - 2],
        const CoreCall('removeProperty', {'id': _id, 'key': 'role'}),
      );
      expectValues(fake.calls.last, 'title', ['Operations manager']);

      await menu('Remove');
      expect(
        fake.calls.last,
        const CoreCall('removeProperty', {'id': _id, 'key': 'role'}),
      );

      // Several phone numbers: "Add value" adds a field, the close button
      // removes one.
      await tapVisible(tester, find.text('Add property'));
      await tester.enterText(
        find.widgetWithText(TextField, 'Property'),
        'phone',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Value'),
        '+20 100',
      );
      await tester.tap(find.text('Add value'));
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, 'Value').last,
        '+20 101',
      );
      await tester.tap(find.text('Add value'));
      await settle(tester);
      expect(find.widgetWithText(TextField, 'Value'), findsNWidgets(3));
      await tester.tap(find.byTooltip('Remove value').last);
      await settle(tester);
      await tester.tap(find.text('Save'));
      await settle(tester);
      expectValues(fake.calls.last, 'phone', ['+20 100', '+20 101']);
    });

    testWidgets('a failed edit is reported', (tester) async {
      final fake = _fake()
        ..removeAliasAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await _pump(tester, _expanded, const EntityPage(_id), fake);
      await tester.tap(find.byTooltip('Remove alias Ahmed Sameer'));
      await settle(tester);
      expect(find.text("That didn't work (offline)."), findsOneWidget);
    });

    testWidgets('your notes are edited in place', (tester) async {
      final fake = await _pump(tester, _expanded, const EntityPage(_id));
      await tapVisible(tester, find.text('Edit notes'));
      final field = find.widgetWithText(TextField, 'Your notes');
      expect(
        tester.widget<TextField>(field).controller!.text,
        'Prefers WhatsApp to email.',
      );
      await tester.enterText(field, 'Prefers calls after 4 pm.');
      await tester.tap(find.text('Save notes'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('updateUserNotes', {
          'id': _id,
          'text': 'Prefers calls after 4 pm.',
        }),
      );
      expect(find.text('Notes saved'), findsOneWidget);
      expect(find.text('Save notes'), findsNothing);
    });

    testWidgets('a failed notes save keeps the editor open', (tester) async {
      final fake = _fake()
        ..updateUserNotesAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await _pump(tester, _expanded, const EntityPage(_id), fake);
      await tapVisible(tester, find.text('Edit notes'));
      await tester.tap(find.text('Save notes'));
      await settle(tester);
      expect(find.text("Couldn't save your notes (offline)."), findsOneWidget);
      expect(find.text('Save notes'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(find.text('Prefers WhatsApp to email.'), findsOneWidget);
    });

    testWidgets('merge opens the page it merged into', (tester) async {
      final opened = <String>[];
      final fake = await _pump(
        tester,
        _compact,
        EntityPage(_id, onOpenEntity: opened.add),
      );
      await tester.tap(find.byTooltip('More actions'));
      await settle(tester);
      await tester.tap(find.text('Merge…'));
      await settle(tester);
      await tester.tap(find.text('Mona Hassan'));
      await settle(tester);
      expect(find.text('Merge pages'), findsOneWidget);
      expect(find.text('Aliases added'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Merge'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('mergeEntities', {
          'sourceId': _id,
          'intoId': 'p-mona-hassan',
        }),
      );
      expect(opened, ['p-mona-hassan']);
      expect(find.text('Merged'), findsOneWidget);
    });

    testWidgets('a cancelled merge changes nothing', (tester) async {
      final opened = <String>[];
      final fake = await _pump(
        tester,
        _expanded,
        EntityPage(_id, onOpenEntity: opened.add),
      );
      await tester.tap(find.widgetWithText(OutlinedButton, 'Merge…'));
      await settle(tester);
      await tester.tap(find.text('Mona Hassan'));
      await settle(tester);
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(fake.calls.map((c) => c.method), isNot(contains('mergeEntities')));
      expect(opened, isEmpty);
    });
  });

  group('EntityPage states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await _pump(tester, v, const EntityPage(_id), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupDocumentsLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('not found $v', (tester) async {
        await _pump(
          tester,
          v,
          const EntityPage('x-gone'),
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
        await _pump(tester, v, const EntityPage(_id), fake);
        fake.entity[_id].addError(
          const CoreFailure(code: 'store', messageKey: 'error.store'),
        );
        await settle(tester);
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).errorTitle),
          findsOneWidget,
        );
      });

      testWidgets('offline edits and an empty page $v', (tester) async {
        await _pump(
          tester,
          v,
          const EntityPage(_id),
          _fake(DirFixtures.ahmedPending),
        );
        final l10n = lookupDirectoryLocalizations(v.locale);
        final docs = lookupDocumentsLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.pendingSync), findsOneWidget);
        expect(find.text(l10n.noSummary), findsOneWidget);
        expect(find.text(docs.userNotesEmpty), findsOneWidget);
        expect(find.text('Watanya contract'), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('a document ID renders the document page $v', (tester) async {
        await _pump(
          tester,
          v,
          const EntityPage('d-watanya-contract'),
          _fake(StrataFixtures.entityScreenDocument),
        );
        expect(
          find.text(lookupDocumentsLocalizations(v.locale).whereItIs),
          findsOneWidget,
        );
      });

      testWidgets('a place ID renders the place page $v', (tester) async {
        await _pump(
          tester,
          v,
          const EntityPage('pl-nasr-city-office'),
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
