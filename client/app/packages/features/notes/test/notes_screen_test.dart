import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_notes/strata_notes.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'support/harness.dart';
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
        await pumpVariant(tester, v, const NotesScreen(folder: _folder), fake);
        expect(find.byType(NotesListPane), findsOneWidget);
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(find.text('archive'), findsOneWidget);
        expect(find.text(_folder), findsOneWidget);
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
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('note $v', (tester) async {
        final fake = fakeWith();
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake,
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
        await revealInScroll(tester, find.byType(StrataNoteEditor));
        expect(find.byType(StrataNoteEditor), findsOneWidget);
        expect(
          fake.calls,
          contains(const CoreCall('watchNote', {'id': _pricing})),
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('empty, loading and error $v', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: 'notes/ops'),
          fake,
        );
        expect(find.byType(CircularProgressIndicator), findsOneWidget);
        fake.notesList['notes/ops'].add(NotesFixtures.emptyFolder);
        await tester.pump();
        expect(
          find.text(v.rtl ? 'مفيش ملاحظات هنا لسه' : 'No notes here yet'),
          findsOneWidget,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
        fake.notesList['notes/ops'].addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(
          find.text(
            v.rtl ? 'مقدرناش نفتح المجلد ده' : "Couldn't load this folder",
          ),
          findsOneWidget,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('offline and conflict $v', (tester) async {
        final fake = fakeWith(
          note: NotesFixtures.pricingNote(
            history: Availability.offline,
            sync: const NoteSyncState(
              kind: NoteSyncKind.conflict,
              pendingOps: 1,
              conflictOpId: NotesFixtures.conflictOpId,
            ),
          ),
        );
        await pumpVariant(
          tester,
          v,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake,
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
        expectNoRenderErrors(tester);
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

    testWidgets('search shows the core keyword results', (tester) async {
      final fake = fakeWith()
        ..searchAnswer.returns(
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
              ),
            ],
            availability: Availability.available,
          ),
        );
      final host = Host();
      await pumpNotes(tester, host.screen(), fake: fake);
      await tester.enterText(find.byType(TextField), 'churn');
      await tester.pump();
      await tester.pump();
      expect(callsOf(fake, 'search'), [
        const CoreCall('search', {
          'query': 'churn',
          'mode': SearchMode.keyword,
        }),
      ]);
      expect(find.text('Most exits happen at the first renewal.'), findsOne);
      await tester.tap(find.text('Churn notes'));
      expect(host.notes, ['n-churn-notes']);
      await tester.tap(find.byTooltip('Clear search'));
      await tester.pump();
      await tester.pump();
      expect(find.text('Weekly invoicing proposal'), findsOneWidget);
    });

    testWidgets('search with no results says so', (tester) async {
      final fake = fakeWith()
        ..searchAnswer.returns(
          const SearchView(
            query: 'zzz',
            mode: SearchMode.keyword,
            results: [],
            availability: Availability.available,
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
      await tester.tap(find.text('Reject'));
      await tester.pumpAndSettle();
      expect(callsOf(fake, 'removeRelation'), [
        const CoreCall('removeRelation', {
          'srcId': _pricing,
          'dstId': 'n-discount-policy',
          'relType': 'contradicts',
        }),
      ]);
      expect(find.text('Suggested by AI'), findsNothing);
    });

    testWidgets('long-press sheet: Retype → supports', (tester) async {
      final fake = await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      await tester.longPress(find.text('Discount policy'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Retype'));
      await tester.pumpAndSettle();
      expect(find.text('Change relation type'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(SimpleDialog),
          matching: find.text('contradicts'),
        ),
        findsNothing,
        reason: 'the current type is not offered',
      );
      await tester.tap(find.text('supports'));
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
      expect(callsOf(fake, 'removeRelation'), [
        const CoreCall('removeRelation', {
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
      await tester.tap(
        find.descendant(of: panel, matching: find.text('Q4 hiring plan')),
      );
      expect(host.notes, ['n-q4-hiring']);
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
      Availability.available: 'Versions of this note appear here.',
      Availability.offline:
          "History needs a connection. It's back when you're online.",
      Availability.notYetAvailable: "History isn't available yet.",
      Availability.notAllowed: "History isn't available for this account.",
    };
    for (final entry in historyMessages.entries) {
      testWidgets('history renders ${entry.key.name}', (tester) async {
        await pumpNotes(
          tester,
          const NotesScreen(folder: _folder, selectedNoteId: _pricing),
          fake: fakeWith(note: NotesFixtures.pricingNote(history: entry.key)),
        );
        await tester.tap(find.text('History').first);
        await tester.pump();
        expect(find.text(entry.value), findsOneWidget);
      });
    }
  });

  group('compact note page', () {
    testWidgets('tabs switch between note, links and history', (tester) async {
      await pumpNotes(
        tester,
        const NotesScreen(folder: _folder, selectedNoteId: _pricing),
        sizeClass: SizeClass.compact,
      );
      expect(find.byType(StrataNoteEditor), findsOneWidget);
      await tester.tap(find.text('Links'));
      await tester.pump();
      expect(find.byType(BacklinksSection), findsOneWidget);
      await tester.scrollUntilVisible(find.byType(LocalGraphSlot), 200);
      expect(find.byType(LocalGraphSlot), findsOneWidget);
      expect(find.byType(StrataNoteEditor), findsNothing);
      await tester.tap(find.text('History'));
      await tester.pump();
      expect(find.text('Versions of this note appear here.'), findsOneWidget);
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
          note: NotesFixtures.pricingNote(
            sync: const NoteSyncState(
              kind: NoteSyncKind.conflict,
              pendingOps: 1,
              conflictOpId: NotesFixtures.conflictOpId,
            ),
          ),
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
    });
  });
}
