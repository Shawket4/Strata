import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_maps/strata_maps.dart' show MiniGraph;
import 'package:strata_notes/strata_notes.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'support/notes_fixtures.dart';

const String _folder = 'notes/sales';
const String _pricing = NotesFixtures.pricingId;

FakeCoreApi fakeWith({NoteView? note, NotesListView? list}) {
  final fake = FakeCoreApi()..editorHintsAnswer.returns(const []);
  fake.notesList[list?.folder ?? _folder].add(list ?? NotesFixtures.sales);
  fake.note[_pricing].add(
    NoteScreen(id: _pricing, note: note ?? NotesFixtures.pricingNote()),
  );
  return fake;
}

/// Records the host callbacks.
final class Host {
  final List<String> folders = [];
  final List<String> notes = [];
  final List<String> conflicts = [];
  final List<String> maps = [];
  int closed = 0;

  NotesScreen screen({String folder = _folder, String? selected}) =>
      NotesScreen(
        folder: folder,
        selectedNoteId: selected,
        onOpenFolder: folders.add,
        onOpenNote: notes.add,
        onCloseNote: () => closed++,
        onOpenConflict: conflicts.add,
        onOpenLocalMap: maps.add,
      );
}

Future<FakeCoreApi> pumpNotes(
  WidgetTester tester,
  Widget screen, {
  FakeCoreApi? fake,
  SizeClass sizeClass = SizeClass.expanded,
}) async {
  final api = fake ?? fakeWith();
  await pumpStrataScreen(tester, screen, fake: api, sizeClass: sizeClass);
  await tester.pump();
  return api;
}

/// Scrolls the note's scroll view until [finder] is built (large text
/// scales push it below the fold).
Future<void> revealInScroll(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isNotEmpty) return;
  await tester.dragUntilVisible(
    finder,
    find.byType(CustomScrollView).first,
    const Offset(0, -300),
  );
}

List<CoreCall> callsOf(FakeCoreApi fake, String method) =>
    fake.calls.where((c) => c.method == method).toList();

void main() {
  group('layout matrix', () {
    for (final v in variants()) {
      testWidgets('list $v', (tester) async {
        final fake = fakeWith();
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: _folder),
          fake: fake,
        );
        expect(find.byType(NotesListPane), findsOneWidget);
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(find.text('archive'), findsOneWidget);
        // The core's breadcrumb and count.
        expect(find.text('sales'), findsOneWidget);
        expect(find.text('notes'), findsOneWidget);
        expect(find.text(v.rtl ? '7 ملاحظات' : '7 notes'), findsOneWidget);
        expect(find.text('14:31'), findsOneWidget);
        expect(
          fake.calls.first,
          const CoreCall('watchNotesList', {'folder': _folder}),
        );
        expect(find.byType(NoteDetailPane), findsNothing);
        expect(find.byType(CompactNotePage), findsNothing);
        expect(
          find.text(v.rtl ? 'اختار ملاحظة' : 'Select a note'),
          v.sizeClass == SizeClass.compact ? findsNothing : findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('note $v', (tester) async {
        final fake = fakeWith();
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake: fake,
        );
        final compact = v.sizeClass == SizeClass.compact;
        expect(
          find.byType(NotesListPane),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.byType(CompactNotePage),
          compact ? findsOneWidget : findsNothing,
        );
        expect(
          find.byType(NoteDetailPane),
          compact ? findsNothing : findsOneWidget,
        );
        expect(
          find.byType(NoteContextPanel),
          v.sizeClass == SizeClass.expanded ? findsOneWidget : findsNothing,
          reason:
              'context: panel on expanded, drawer (closed) on medium, '
              'tab on compact',
        );
        expect(
          find.byType(FormattingToolbar),
          compact ? findsOneWidget : findsNothing,
        );
        expect(find.byType(PropertiesPanel), findsOneWidget);
        expect(find.text('Saved'), findsWidgets);
        expect(find.text('v7'), findsOneWidget);
        await revealInScroll(tester, find.byType(StrataNoteEditor));
        expect(find.byType(StrataNoteEditor), findsOneWidget);
        expect(
          fake.calls,
          contains(const CoreCall('watchNote', {'id': _pricing})),
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('empty, loading and error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: 'notes/ops'),
          fake: fake,
        );
        expect(find.byType(CircularProgressIndicator), findsOneWidget);
        fake.notesList['notes/ops'].add(NotesFixtures.emptyFolder);
        await tester.pump();
        expect(
          find.text(v.rtl ? 'مفيش ملاحظات هنا لسه' : 'No notes here yet'),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
        fake.notesList['notes/ops'].addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(
          find.text(
            v.rtl ? 'مقدرناش نفتح المجلد ده' : "Couldn't load this folder",
          ),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('offline and conflict $v', (tester) async {
        final fake = fakeWith(
          note: NotesFixtures.pricingNote(
            history: Availability.offline,
            sync: NotesFixtures.conflictSync,
          ),
        );
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake: fake,
        );
        await revealInScroll(tester, find.byType(NoteConflictBanner));
        expect(find.byType(NoteConflictBanner), findsOneWidget);
        if (v.sizeClass == SizeClass.expanded) {
          final historyTab = find
              .descendant(
                of: find.byType(NoteContextPanel),
                matching: find.text(v.rtl ? 'السجل' : 'History'),
              )
              .first;
          await tester.ensureVisible(historyTab);
          await tester.pump();
          await tester.tap(historyTab);
          await tester.pump();
          expect(
            find.text(
              v.rtl
                  ? 'السجل محتاج اتصال. هيرجع أول ما تبقى أونلاين.'
                  : "History needs a connection. It's back when you're online.",
            ),
            findsOneWidget,
          );
        }
        expectNoErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('list', () {
    testWidgets('shows snippets, the selection and the unsynced dot', (
      tester,
    ) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      expect(
        find.text(
          'Three experiments to run in Q4 before we lock the Subscription '
          'tiers. Ahmed Samir runs the Acme pilot.',
        ),
        findsOneWidget,
      );
      expect(find.bySemanticsLabel(RegExp('Not synced yet')), findsOneWidget);
      final rows = tester.widgetList<NoteRow>(find.byType(NoteRow)).toList();
      expect(rows.map((r) => r.selected), [true, false, false, false]);
    });

    testWidgets('opens notes and folders through the host', (tester) async {
      final host = Host();
      await pumpNotes(tester, host.screen());
      await tester.tap(find.text('Churn notes'));
      await tester.tap(find.text('archive'));
      await tester.tap(find.text('Notes'));
      expect(host.notes, ['n-churn-notes']);
      expect(host.folders, ['notes/sales/archive', '']);
    });

    testWidgets('keeps the selection itself without a host', (tester) async {
      final fake = await pumpNotes(tester, const NotesScreen(folder: _folder));
      await tester.tap(find.text('Pricing experiments'));
      await tester.pump();
      await tester.pump();
      expect(find.byType(NoteDetailPane), findsOneWidget);
      expect(
        fake.calls,
        contains(const CoreCall('watchNote', {'id': _pricing})),
      );
    });

    testWidgets('search asks the core within the folder', (tester) async {
      final fake = fakeWith()
        ..searchInFolderAnswer.returns(
          const SearchView(
            query: 'churn',
            mode: SearchMode.keyword,
            results: [
              SearchHit(
                noteId: 'n-churn-notes',
                title: 'Churn notes',
                path: 'notes/sales/Churn notes.md',
                kind: 'note',
                snippet: 'Most exits happen at the first renewal.',
                titleDir: TextDir.ltr,
                snippetDir: TextDir.ltr,
                highlights: [HighlightSpan(start: 5, end: 10)],
                score: 0,
              ),
            ],
            availability: Availability.available,
            availableModes: [],
          ),
        );
      final host = Host();
      await pumpNotes(tester, host.screen(), fake: fake);
      await tester.enterText(find.byType(TextField), 'churn');
      await tester.pump();
      await tester.pump();
      expect(callsOf(fake, 'searchInFolder'), [
        const CoreCall('searchInFolder', {
          'query': 'churn',
          'mode': SearchMode.keyword,
          'folder': _folder,
        }),
      ]);
      final snippet = tester.widget<StrataHighlightedText>(
        find.byType(StrataHighlightedText),
      );
      expect(snippet.text, 'Most exits happen at the first renewal.');
      expect(snippet.highlights, const [TextRange(start: 5, end: 10)]);
      await tester.tap(find.text('Churn notes'));
      expect(host.notes, ['n-churn-notes']);
      await tester.tap(find.byTooltip('Clear search'));
      await tester.pump();
      await tester.pump();
      expect(find.text('Weekly invoicing proposal'), findsOneWidget);
    });

    testWidgets('search with no results says so', (tester) async {
      final fake = fakeWith()
        ..searchInFolderAnswer.returns(
          const SearchView(
            query: 'zzz',
            mode: SearchMode.keyword,
            results: [],
            availability: Availability.available,
            availableModes: [],
          ),
        );
      await pumpNotes(tester, const NotesScreen(folder: _folder), fake: fake);
      await tester.enterText(find.byType(TextField), 'zzz');
      await tester.pump();
      await tester.pump();
      expect(find.text('No notes match “zzz”'), findsOneWidget);
    });

    testWidgets('⌘/Ctrl-F focuses the search field', (tester) async {
      await pumpNotes(tester, const NotesScreen(folder: _folder));
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      final field = tester.widget<TextField>(find.byType(TextField));
      expect(field.focusNode!.hasFocus, isTrue);
    });

    testWidgets('right-click → Delete note asks, then deletes', (tester) async {
      final fake = await pumpNotes(tester, const NotesScreen(folder: _folder));
      await tester.tap(
        find.text('Churn notes'),
        buttons: kSecondaryMouseButton,
        kind: PointerDeviceKind.mouse,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete note'));
      await tester.pumpAndSettle();
      expect(find.text('Delete “Churn notes”?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();
      expect(callsOf(fake, 'deleteNote'), [
        const CoreCall('deleteNote', {'id': 'n-churn-notes'}),
      ]);
    });

    testWidgets('Retry re-subscribes to the folder', (tester) async {
      final fake = FakeCoreApi();
      await pumpNotes(tester, const NotesScreen(folder: 'x'), fake: fake);
      fake.notesList['x'].addError(StrataFixtures.coreFailure);
      await tester.pump();
      await tester.tap(find.text('Retry'));
      await tester.pump();
      expect(callsOf(fake, 'watchNotesList'), hasLength(2));
    });
  });

  group('properties and relations', () {
    testWidgets('chips show the type, target and AI confidence', (
      tester,
    ) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      expect(
        find.bySemanticsLabel(
          RegExp(r'^contradicts: Discount policy, .*0\.72'),
        ),
        findsOneWidget,
      );
      expect(
        find.bySemanticsLabel('part of: Subscription tiers'),
        findsOneWidget,
      );
      expect(find.text('AI · 0.81'), findsOneWidget);
      expect(find.text('\u2068#pricing\u2069'), findsOneWidget);
    });

    testWidgets('tapping a chip opens its target', (tester) async {
      final host = Host();
      await pumpNotes(tester, host.screen(selected: _pricing));
      await tester.tap(find.text('Subscription tiers'));
      expect(host.notes, ['n-subscription-tiers']);
    });

    testWidgets('long-press sheet: reason and Reject', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      await tester.longPress(find.text('Discount policy'));
      await tester.pumpAndSettle();
      expect(find.text('Suggested by AI'), findsOneWidget);
      expect(find.text('confidence 0.72'), findsOneWidget);
      expect(
        find.text(
          'States a flat 10% discount, while target caps discounts at 3%.',
        ),
        findsOneWidget,
      );
      expect(find.text('14:05'), findsOneWidget);
      expect(find.byType(CitationChip), findsNWidgets(2));
      await tester.tap(find.text('Reject'));
      await tester.pumpAndSettle();
      expect(callsOf(fake, 'rejectRelation'), [
        const CoreCall('rejectRelation', {
          'srcId': _pricing,
          'dstId': 'n-discount-policy',
          'relType': 'contradicts',
        }),
      ]);
      expect(find.text('Suggested by AI'), findsNothing);
    });

    testWidgets('long-press sheet: Retype → supports', (tester) async {
      final fake = fakeWith()
        ..relationTypesAnswer.returns(const [
          RelationTypeItem(key: 'related', label: 'related'),
          RelationTypeItem(key: 'supports', label: 'supports'),
          RelationTypeItem(key: 'contradicts', label: 'contradicts'),
        ]);
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        fake: fake,
        sizeClass: SizeClass.compact,
      );
      await tester.longPress(find.text('Discount policy'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Retype'));
      await tester.pumpAndSettle();
      expect(find.text('Change relation type'), findsOneWidget);
      expect(callsOf(fake, 'relationTypes'), hasLength(1));
      // The core's types; the current one is checked.
      expect(
        find.descendant(
          of: find.byType(SimpleDialog),
          matching: find.byIcon(Icons.check),
        ),
        findsOneWidget,
      );
      await tester.tap(
        find.descendant(
          of: find.byType(SimpleDialog),
          matching: find.text('supports'),
        ),
      );
      await tester.pumpAndSettle();
      expect(callsOf(fake, 'retypeRelation'), [
        const CoreCall('retypeRelation', {
          'srcId': _pricing,
          'dstId': 'n-discount-policy',
          'relType': 'contradicts',
          'newType': 'supports',
        }),
      ]);
    });

    testWidgets('hover card on desktop layouts', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(tester.getCenter(find.text('Churn notes').last));
      await tester.pump();
      expect(
        find.text('Both discuss retention of customers past 12 months.'),
        findsOneWidget,
      );
      await tester.tap(find.text('Reject'));
      await tester.pump();
      expect(callsOf(fake, 'rejectRelation'), [
        const CoreCall('rejectRelation', {
          'srcId': _pricing,
          'dstId': 'n-churn-notes',
          'relType': 'related',
        }),
      ]);
      await mouse.moveTo(Offset.zero);
      await tester.pump();
      expect(
        find.text('Both discuss retention of customers past 12 months.'),
        findsNothing,
      );
    });

    testWidgets('a citation of an AI relation opens its block', (tester) async {
      final opened = <(String, String?)>[];
      await pumpNotes(
        tester,
        NotesScreen(
          folder: _folder,
          selectedNoteId: _pricing,
          onOpenLink: (id, anchor) => opened.add((id, anchor)),
        ),
        sizeClass: SizeClass.compact,
      );
      await tester.longPress(find.text('Discount policy'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(CitationChip).last);
      expect(opened, [('n-discount-policy', 'cap')]);
    });

    testWidgets('user relations have no AI actions', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      await tester.longPress(find.text('Subscription tiers'));
      await tester.pump();
      expect(find.text('Suggested by AI'), findsNothing);
    });
  });

  group('context', () {
    testWidgets('backlinks are grouped by relation type', (tester) async {
      final host = Host();
      await pumpNotes(tester, host.screen(selected: _pricing));
      final panel = find.byType(NoteContextPanel);
      for (final label in ['supports', 'follows up', 'body links']) {
        expect(
          find.descendant(of: panel, matching: find.text(label)),
          findsOneWidget,
        );
      }
      expect(find.text('Backlinks 6'), findsOneWidget);
      expect(
        find.text('Ahmed asked whether annual prepay gets a discount.'),
        findsOneWidget,
      );
      expect(
        find.descendant(of: panel, matching: find.text('AI · 0.77')),
        findsOneWidget,
      );
      await tester.tap(
        find.descendant(of: panel, matching: find.text('Q4 hiring plan')),
      );
      expect(host.notes, ['n-q4-hiring']);
    });

    testWidgets('the mini graph shows the core local graph', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      expect(
        find.descendant(
          of: find.byType(LocalGraphSlot),
          matching: find.byType(MiniGraph),
        ),
        findsOneWidget,
      );
      expect(
        fake.calls,
        contains(
          const CoreCall('watchLocalGraph', {'id': _pricing, 'depth': 1}),
        ),
      );
    });

    testWidgets('Open map and the local map button open the mind map', (
      tester,
    ) async {
      final host = Host();
      await pumpNotes(tester, host.screen(selected: _pricing));
      await tester.tap(find.text('Open map'));
      await tester.tap(find.byTooltip('Open local mind map'));
      expect(host.maps, [_pricing, _pricing]);
    });

    testWidgets('the expanded panel hides and shows', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      await tester.tap(find.byTooltip('Hide context panel'));
      await tester.pump();
      expect(find.byType(NoteContextPanel), findsNothing);
      await tester.tap(find.byType(TextField));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.period);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(find.byType(NoteContextPanel), findsOneWidget);
    });

    testWidgets('the medium drawer opens and closes', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.medium,
      );
      expect(find.byType(NoteContextPanel), findsNothing);
      await tester.tap(find.byTooltip('Show context panel'));
      await tester.pump();
      expect(find.byType(NoteContextPanel), findsOneWidget);
      expect(find.text('Context'), findsOneWidget);
      await tester.tap(find.byTooltip('Close'));
      await tester.pump();
      expect(find.byType(NoteContextPanel), findsNothing);
    });

    const historyMessages = {
      Availability.offline:
          "History needs a connection. It's back when you're online.",
      Availability.notYetAvailable: "History isn't available yet.",
      Availability.notAllowed: "History isn't available for this account.",
    };
    for (final entry in historyMessages.entries) {
      testWidgets('history renders ${entry.key.name}', (tester) async {
        final fake = await pumpNotes(
          tester,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake: fakeWith(note: NotesFixtures.pricingNote(history: entry.key)),
        );
        await tester.tap(find.text('History').first);
        await tester.pump();
        expect(find.text(entry.value), findsOneWidget);
        expect(callsOf(fake, 'refreshHistory'), isEmpty);
      });
    }

    testWidgets('history lists the core entries and refreshes', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      await tester.tap(find.text('History').first);
      await tester.pump();
      expect(callsOf(fake, 'refreshHistory').last.args, {'noteId': _pricing});
      expect(find.text('All 3 versions'), findsOneWidget);
      expect(find.text('ai: contradicts [[Discount policy]]'), findsOneWidget);
      expect(find.text('Strata AI · today 14:05'), findsOneWidget);
      expect(find.text('You · today 14:31'), findsOneWidget);
      // The current version cannot be reverted to.
      expect(find.text('Revert'), findsNWidgets(2));
    });

    testWidgets('history: changes and revert', (tester) async {
      final fake = fakeWith()
        ..noteRevisionDiffAnswer.returns(
          const NoteDiffView(
            noteId: _pricing,
            commit: 'c6',
            summary: '+1 line, 1 removed',
            lines: [
              DiffLine(
                kind: DiffLineKind.removed,
                oldLine: 3,
                text: '- Launch offer: a flat 5% discount.',
                dir: TextDir.ltr,
              ),
              DiffLine(
                kind: DiffLineKind.added,
                newLine: 3,
                text: '- Launch offer: a flat 10% discount.',
                dir: TextDir.ltr,
              ),
            ],
          ),
        );
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        fake: fake,
      );
      await tester.tap(find.text('History').first);
      await tester.pump();
      await tester.tap(find.text('Changes').at(1));
      await tester.pumpAndSettle();
      expect(find.text('v6 compared with now'), findsOneWidget);
      expect(find.text('+1 line, 1 removed'), findsOneWidget);
      expect(find.text('- Launch offer: a flat 5% discount.'), findsOneWidget);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Revert').first);
      await tester.pumpAndSettle();
      expect(find.text('Revert to v6?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Revert'));
      await tester.pumpAndSettle();
      expect(
        fake.calls.where(
          (c) => c.method == 'noteRevisionDiff' || c.method == 'revertNote',
        ),
        [
          const CoreCall('noteRevisionDiff', {
            'noteId': _pricing,
            'commit': 'c6',
          }),
          const CoreCall('revertNote', {'noteId': _pricing, 'commit': 'c6'}),
        ],
      );
    });
  });

  group('note header', () {
    testWidgets('meta line, word count and pin', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
      );
      expect(find.text('Created 18 Sep'), findsOneWidget);
      expect(find.text('edited today 14:31 by Shawket'), findsOneWidget);
      expect(find.text('214 words'), findsOneWidget);
      await tester.tap(find.byTooltip('Pin to sidebar'));
      await tester.pump();
      expect(callsOf(fake, 'pinNote'), [
        const CoreCall('pinNote', {'id': _pricing, 'pinned': true}),
      ]);
    });

    testWidgets('a pinned note unpins', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        fake: fakeWith(note: NotesFixtures.pricingNote(pinned: true)),
        sizeClass: SizeClass.compact,
      );
      await tester.tap(find.byTooltip('Unpin from sidebar'));
      await tester.pump();
      expect(callsOf(fake, 'pinNote'), [
        const CoreCall('pinNote', {'id': _pricing, 'pinned': false}),
      ]);
    });

    testWidgets('a linked block is resolved by the core', (tester) async {
      final fake = fakeWith()
        ..resolveCitationAnswer.returns(
          const CitationPreview(
            noteId: _pricing,
            title: 'Pricing experiments',
            path: 'notes/sales/Pricing experiments.md',
            blockText: 'Launch offer: a flat 10% discount on the first plan.',
            blockDir: TextDir.ltr,
            heading: 'Hypotheses',
            tags: [],
          ),
        );
      await pumpNotes(
        tester,
        const NotesScreen(
          folder: _folder,
          selectedNoteId: _pricing,
          anchor: 'a1b2',
        ),
        fake: fake,
      );
      expect(callsOf(fake, 'resolveCitation'), [
        const CoreCall('resolveCitation', {
          'noteId': _pricing,
          'anchor': 'a1b2',
        }),
      ]);
      expect(find.text('Linked block'), findsOneWidget);
      expect(find.text('Hypotheses'), findsWidgets);
      expect(
        find.text('Launch offer: a flat 10% discount on the first plan.'),
        findsOneWidget,
      );
    });

    testWidgets('a block that is gone says so', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(
          folder: _folder,
          selectedNoteId: _pricing,
          anchor: 'zz99',
        ),
        sizeClass: SizeClass.compact,
      );
      expect(find.text("That block isn't in this note anymore."), findsOne);
    });

    testWidgets('a duplicate-flagged note opens the prompt', (tester) async {
      final duplicates = <String>[];
      await pumpNotes(
        tester,
        NotesScreen(
          folder: _folder,
          selectedNoteId: _pricing,
          onOpenDuplicate: duplicates.add,
        ),
        fake: fakeWith(
          note: NotesFixtures.pricingNote(sync: NotesFixtures.duplicateSync),
        ),
      );
      expect(find.text('This note may already exist'), findsOneWidget);
      expect(find.text('Already exists?'), findsOneWidget);
      await tester.tap(find.text('Review'));
      expect(duplicates, [NotesFixtures.duplicateOpId]);
    });
  });

  group('compact note page', () {
    testWidgets('tabs switch between note, links and history', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      expect(find.byType(StrataNoteEditor), findsOneWidget);
      await tester.tap(find.text('Links 6'));
      await tester.pump();
      expect(find.byType(BacklinksSection), findsOneWidget);
      await tester.scrollUntilVisible(find.byType(LocalGraphSlot), 200);
      expect(find.byType(LocalGraphSlot), findsOneWidget);
      expect(find.byType(StrataNoteEditor), findsNothing);
      await tester.tap(find.text('History'));
      await tester.pump();
      expect(find.text('All 3 versions'), findsOneWidget);
    });

    testWidgets('back returns to the list', (tester) async {
      final host = Host();
      await pumpNotes(
        tester,
        host.screen(selected: _pricing),
        sizeClass: SizeClass.compact,
      );
      await tester.tap(find.byTooltip('Back to notes'));
      expect(host.closed, 1);
    });

    testWidgets('properties expand from the chips row', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      expect(find.text('Tags'), findsNothing);
      await tester.tap(find.byTooltip('Expand properties'));
      await tester.pump();
      expect(find.text('Tags'), findsOneWidget);
    });

    testWidgets('Resolve hands the conflict to the sync screen', (
      tester,
    ) async {
      final host = Host();
      await pumpNotes(
        tester,
        host.screen(selected: _pricing),
        fake: fakeWith(
          note: NotesFixtures.pricingNote(sync: NotesFixtures.conflictSync),
        ),
        sizeClass: SizeClass.compact,
      );
      await tester.tap(find.text('Resolve'));
      expect(host.conflicts, [NotesFixtures.conflictOpId]);
    });
  });

  group('Arabic content', () {
    testWidgets('paragraphs take their own direction in an English UI', (
      tester,
    ) async {
      final fake = fakeWith();
      fake.note[NotesFixtures.arabicId].add(
        NoteScreen(id: NotesFixtures.arabicId, note: NotesFixtures.arabicNote),
      );
      await pumpNotes(
        tester,
        const NotesScreen(
          folder: _folder,
          selectedNoteId: NotesFixtures.arabicId,
        ),
        fake: fake,
      );
      final editor = tester.widget<StrataNoteEditor>(
        find.byType(StrataNoteEditor),
      );
      expect(editor.controller.content, NotesFixtures.arabicContent);
      expect(
        Directionality.of(tester.element(find.byType(StrataNoteEditor))),
        TextDirection.ltr,
      );
      // Each line takes the direction the core sent.
      expect(editor.controller.lineHints.directions.values.toSet(), {
        TextDirection.rtl,
        TextDirection.ltr,
      });
    });
  });
}
