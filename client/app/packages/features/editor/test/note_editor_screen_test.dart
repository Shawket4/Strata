import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';
import 'package:super_editor/super_editor_test.dart';

import 'support/editor_fixtures.dart';
import 'support/harness.dart';

const String _id = EditorFixtures.noteId;

FakeCoreApi fakeWith(NoteView? note) {
  final fake = FakeCoreApi()..editorHintsAnswer.returns(const []);
  if (note != null) {
    fake.note[_id].add(NoteScreen(id: _id, note: note));
  }
  return fake;
}

Future<FakeCoreApi> pumpEditor(
  WidgetTester tester, {
  NoteView? note,
  FakeCoreApi? fake,
  SizeClass sizeClass = SizeClass.expanded,
  ValueChanged<String>? onOpenLink,
  ValueChanged<String>? onOpenConflict,
}) async {
  final api = fake ?? fakeWith(note ?? EditorFixtures.note);
  await pumpStrataScreen(
    tester,
    NoteEditorScreen(
      noteId: _id,
      onOpenLink: onOpenLink,
      onOpenConflict: onOpenConflict,
    ),
    fake: api,
    sizeClass: sizeClass,
  );
  await tester.pump();
  return api;
}

MutableDocument documentIn(WidgetTester tester) =>
    tester.widget<SuperEditor>(find.byType(SuperEditor)).editor.document;

String nodeIdOfLine(WidgetTester tester, String startsWith) =>
    documentIn(tester)
        .firstWhere(
          (n) => (n as TextNode).text.toPlainText().startsWith(startsWith),
        )
        .id;

List<CoreCall> callsOf(FakeCoreApi fake, String method) =>
    fake.calls.where((c) => c.method == method).toList();

void main() {
  group('layout matrix', () {
    for (final v in variants()) {
      testWidgets('content $v', (tester) async {
        final fake = fakeWith(EditorFixtures.note);
        await pumpVariant(tester, v, const NoteEditorScreen(noteId: _id), fake);
        final l10n = v.rtl ? 'محفوظة' : 'Saved';
        expect(find.text('Pricing experiments'), findsOneWidget);
        expect(find.text(l10n), findsOneWidget);
        expect(find.byType(SuperEditor), findsOneWidget);
        expect(
          find.byType(FormattingToolbar),
          v.sizeClass == SizeClass.compact ? findsOneWidget : findsNothing,
        );
        expect(find.byType(NoteConflictBanner), findsNothing);
        expect(fake.calls, contains(const CoreCall('watchNote', {'id': _id})));
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('conflict $v', (tester) async {
        final fake = fakeWith(
          EditorFixtures.noteWith(sync: EditorFixtures.conflict),
        );
        await pumpVariant(tester, v, const NoteEditorScreen(noteId: _id), fake);
        expect(find.byType(NoteConflictBanner), findsOneWidget);
        expect(
          find.text(
            v.rtl
                ? 'الملاحظة دي اتعدلت كمان على السيرفر'
                : 'This note was also changed on the server',
          ),
          findsOneWidget,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('loading, not found and error $v', (tester) async {
        final fake = fakeWith(null);
        await pumpVariant(tester, v, const NoteEditorScreen(noteId: _id), fake);
        expect(find.byType(CircularProgressIndicator), findsOneWidget);
        fake.note[_id].add(const NoteScreen(id: _id));
        await tester.pump();
        expect(
          find.text(v.rtl ? 'الملاحظة مش موجودة' : 'Note not found'),
          findsOneWidget,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
        fake.note[_id].addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(
          find.text(
            v.rtl ? 'مقدرناش نفتح الملاحظة دي' : "Couldn't open this note",
          ),
          findsOneWidget,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('rendering', () {
    testWidgets('styles hints and shows the note status', (tester) async {
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(
          sync: const NoteSyncState(kind: NoteSyncKind.pending, pendingOps: 2),
        ),
      );
      expect(
        find.text('Saved on this device · 2 changes to sync'),
        findsOneWidget,
      );
      expect(find.text('notes/sales/Pricing experiments.md'), findsOneWidget);
      final document = documentIn(tester);
      expect(
        [for (final n in document) (n as TextNode).text.toPlainText()].first,
        startsWith('Three experiments'),
        reason: 'the frontmatter is not part of the editor body',
      );
    });

    testWidgets('lays out each paragraph in its own direction', (tester) async {
      const content =
          'English first paragraph.\n'
          'فقرة عربية بعدها [[Acme Logistics]] mixed.\n';
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(content: content, hints: const []),
      );
      final components = tester
          .widgetList<TextComponent>(find.byType(TextComponent))
          .toList();
      expect(components[0].textDirection, TextDirection.ltr);
      expect(components[1].textDirection, TextDirection.rtl);
    });

    testWidgets('a line takes the direction of its first strong letter', (
      tester,
    ) async {
      const content =
          '## الفرضيات\n'
          '- خصم ولاء 5% على التجديد.\n'
          '12. Twelfth item\n'
          '- [ ] مهمة 📅 2026-10-01\n'
          '> 5% English quote\n'
          '---\n';
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(content: content, hints: const []),
      );
      final directions = [
        for (final c in tester.widgetList<TextComponent>(
          find.byType(TextComponent),
        ))
          c.textDirection,
      ];
      expect(directions, [
        TextDirection.rtl,
        TextDirection.rtl,
        TextDirection.ltr,
        TextDirection.rtl,
        TextDirection.ltr,
        TextDirection.ltr,
        TextDirection.ltr,
      ]);
    });

    testWidgets('renders task lines with checkboxes in their state', (
      tester,
    ) async {
      await pumpEditor(tester);
      final boxes = tester.widgetList<Checkbox>(find.byType(Checkbox)).toList();
      expect(boxes.map((b) => b.value), [false, true]);
      expect(
        find.bySemanticsLabel('Complete task: Draft two pricing page variants'),
        findsOneWidget,
      );
      expect(
        find.bySemanticsLabel('Reopen task: Pull churn by tenure from billing'),
        findsOneWidget,
      );
    });
  });

  group('saving', () {
    testWidgets('Save calls updateNote with the id and the full markdown', (
      tester,
    ) async {
      final fake = await pumpEditor(tester, sizeClass: SizeClass.compact);
      final line = nodeIdOfLine(tester, 'Three experiments');
      await tester.placeCaretInParagraph(line, 0);
      await tester.typeImeText('Draft: ');
      await tester.pump();
      expect(find.text('Unsaved changes'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      final expected = EditorFixtures.content.replaceFirst(
        'Three experiments',
        'Draft: Three experiments',
      );
      expect(callsOf(fake, 'updateNote'), [
        CoreCall('updateNote', {'id': _id, 'content': expected}),
      ]);
    });

    testWidgets('⌘/Ctrl-S saves', (tester) async {
      final fake = await pumpEditor(tester);
      final line = nodeIdOfLine(tester, '## Next steps');
      await tester.placeCaretInParagraph(line, 13);
      await tester.typeImeText(' (Q4)');
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(callsOf(fake, 'updateNote'), [
        CoreCall('updateNote', {
          'id': _id,
          'content': EditorFixtures.content.replaceFirst(
            '## Next steps',
            '## Next steps (Q4)',
          ),
        }),
      ]);
    });

    testWidgets('a failed save keeps the text and says so', (tester) async {
      final fake = fakeWith(EditorFixtures.note)
        ..updateNoteAnswer.throws(StrataFixtures.coreFailure);
      await pumpStrataScreen(
        tester,
        const Scaffold(body: NoteEditorScreen(noteId: _id)),
        fake: fake,
        sizeClass: SizeClass.expanded,
      );
      await tester.pump();
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 0);
      await tester.typeImeText('X');
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      await tester.pump();
      expect(
        find.text("Couldn't save. Your text is still here."),
        findsOneWidget,
      );
      expect(find.text('Unsaved changes'), findsOneWidget);
    });

    testWidgets('a new view from the core keeps unsaved body text', (
      tester,
    ) async {
      final fake = await pumpEditor(tester);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 0);
      await tester.typeImeText('Draft ');
      // The core adds a relation: new frontmatter, same body.
      final newContent = EditorFixtures.content.replaceFirst(
        'related: ["[[Churn notes]]"]\n',
        'related: ["[[Churn notes]]"]\npeople: ["[[Ahmed Samir]]"]\n',
      );
      final shift = newContent.length - EditorFixtures.content.length;
      fake.note[_id].add(
        NoteScreen(
          id: _id,
          note: EditorFixtures.noteWith(
            content: newContent,
            hints: [
              for (final h in EditorFixtures.hints)
                EditorHint(
                  kind: h.kind,
                  start: h.kind == HintKind.frontmatter ? 0 : h.start + shift,
                  end: h.end + shift,
                ),
            ],
          ),
        ),
      );
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      expect(
        callsOf(fake, 'updateNote').single.args['content'],
        newContent.replaceFirst('Three experiments', 'Draft Three experiments'),
      );
    });

    testWidgets('a new view replaces the body when nothing is being edited', (
      tester,
    ) async {
      final fake = await pumpEditor(tester);
      fake.note[_id].add(
        NoteScreen(
          id: _id,
          note: EditorFixtures.noteWith(
            content: 'Rewritten on another device.\n',
            hints: const [],
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      expect(
        [
          for (final n in documentIn(tester))
            (n as TextNode).text.toPlainText(),
        ],
        ['Rewritten on another device.', ''],
      );
    });
  });

  group('task lines', () {
    testWidgets('checking an open task completes it', (tester) async {
      final fake = await pumpEditor(tester);
      await tester.tap(find.byType(Checkbox).first);
      await tester.pump();
      expect(callsOf(fake, 'completeTask'), [
        const CoreCall('completeTask', {'taskId': 't-01j9p1'}),
      ]);
    });

    testWidgets('unchecking a done task reopens it', (tester) async {
      final fake = await pumpEditor(tester);
      await tester.tap(find.byType(Checkbox).last);
      await tester.pump();
      expect(callsOf(fake, 'reopenTask'), [
        const CoreCall('reopenTask', {'taskId': 't-01j9p2'}),
      ]);
    });

    testWidgets('unsaved text is saved before a toggle', (tester) async {
      final body = EditorFixtures.content.indexOf('Three');
      final fake = fakeWith(
        EditorFixtures.note,
      )..editorHintsAnswer.returns(shiftedHints(EditorFixtures.hints, body, 1));
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 0);
      await tester.typeImeText('A');
      await tester.tap(find.byType(Checkbox).first);
      await tester.pump();
      await tester.pump();
      final methods = [
        for (final c in fake.calls)
          if (c.method == 'updateNote' || c.method == 'completeTask') c.method,
      ];
      expect(methods, ['updateNote', 'completeTask']);
    });

    testWidgets('a task line the core does not know yet is disabled', (
      tester,
    ) async {
      final fake = await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(tasks: const []),
      );
      await tester.tap(find.byType(Checkbox).first, warnIfMissed: false);
      await tester.pump();
      expect(callsOf(fake, 'completeTask'), isEmpty);
      expect(
        find.bySemanticsLabel('Task line (save to enable)'),
        findsNWidgets(2),
      );
    });
  });

  group('autocomplete', () {
    testWidgets('@ suggests people and companies and inserts a mention', (
      tester,
    ) async {
      final fake = fakeWith(EditorFixtures.note);
      fake.directory[(DirectoryTab.people, 'Ah')].add(
        EditorFixtures.people('Ah', const [EditorFixtures.ahmed]),
      );
      fake.directory[(DirectoryTab.companies, 'Ah')].add(
        EditorFixtures.companies('Ah', const []),
      );
      await pumpEditor(tester, fake: fake, sizeClass: SizeClass.compact);
      final line = nodeIdOfLine(tester, '## Next steps');
      await tester.placeCaretInParagraph(line, 13);
      await tester.typeImeText(' with @Ah');
      await tester.pump();
      expect(find.byType(CompletionsPanel), findsOneWidget);
      expect(find.text('People'), findsOneWidget);
      expect(find.text('Ahmed Samir'), findsOneWidget);
      expect(
        fake.calls,
        contains(
          const CoreCall('watchDirectory', {
            'tab': DirectoryTab.people,
            'query': 'Ah',
          }),
        ),
      );
      await tester.tap(find.text('Ahmed Samir'));
      await tester.pump();
      expect(
        fake.calls,
        contains(
          const CoreCall('addRelation', {
            'srcId': _id,
            'dstId': '01J8ZK0AHMED00000000000000',
            'relType': 'people',
          }),
        ),
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      expect(
        callsOf(fake, 'updateNote').single.args['content'],
        EditorFixtures.content.replaceFirst(
          '## Next steps',
          '## Next steps with [[Ahmed Samir]]',
        ),
      );
    });

    testWidgets('@ offers companies as companies: relations', (tester) async {
      final fake = fakeWith(EditorFixtures.note);
      fake.directory[(DirectoryTab.people, 'Ac')].add(
        EditorFixtures.people('Ac', const []),
      );
      fake.directory[(DirectoryTab.companies, 'Ac')].add(
        EditorFixtures.companies('Ac', const [EditorFixtures.acme]),
      );
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' @Ac');
      await tester.pump();
      expect(find.text('Companies'), findsOneWidget);
      await tester.tap(find.text('Acme Logistics'));
      await tester.pump();
      expect(callsOf(fake, 'addRelation'), [
        const CoreCall('addRelation', {
          'srcId': _id,
          'dstId': '01J8ZK0ACME000000000000000',
          'relType': 'companies',
        }),
      ]);
    });

    testWidgets('# opens tag autocomplete (the core has no tag list yet)', (
      tester,
    ) async {
      await pumpEditor(tester);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' #pri');
      await tester.pump();
      expect(
        find.text("Tag suggestions aren't available yet."),
        findsOneWidget,
      );
      await tester.tap(find.byTooltip('Dismiss suggestions'));
      await tester.pump();
      expect(find.text("Tag suggestions aren't available yet."), findsNothing);
    });

    testWidgets('a heading marker is not a tag trigger', (tester) async {
      await pumpEditor(tester);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 0);
      await tester.typeImeText('## ');
      await tester.pump();
      expect(find.byType(CompletionsPanel), findsOneWidget);
      expect(find.text("Tag suggestions aren't available yet."), findsNothing);
    });

    testWidgets('[[ searches notes and completes the link', (tester) async {
      final fake = fakeWith(EditorFixtures.note)
        ..searchAnswer.returns(
          const SearchView(
            query: 'Chu',
            mode: SearchMode.keyword,
            results: [
              SearchHit(
                noteId: 'n-churn',
                title: 'Churn notes',
                path: 'notes/sales/Churn notes.md',
                kind: 'note',
                snippet: 'Customers past 12 months churn 40% less.',
              ),
            ],
            availability: Availability.available,
          ),
        );
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' see [[Chu');
      await tester.pump();
      await tester.pump();
      expect(
        fake.calls,
        contains(
          const CoreCall('search', {
            'query': 'Chu',
            'mode': SearchMode.keyword,
          }),
        ),
      );
      await tester.tap(find.text('Churn notes'));
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      expect(
        callsOf(fake, 'updateNote').single.args['content'],
        EditorFixtures.content.replaceFirst(
          '## Next steps',
          '## Next steps see [[Churn notes]]',
        ),
      );
    });

    testWidgets('[[Note#^ opens the block reference picker state', (
      tester,
    ) async {
      await pumpEditor(tester);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' [[Churn notes#^');
      await tester.pump();
      expect(
        find.text("Block references can't be listed yet."),
        findsOneWidget,
      );
    });
  });

  group('toolbar and links', () {
    testWidgets('toolbar buttons type markdown', (tester) async {
      final fake = await pumpEditor(tester, sizeClass: SizeClass.compact);
      final line = nodeIdOfLine(tester, 'Three');
      await tester.placeCaretInParagraph(line, 0);
      await tester.tap(find.byTooltip('Heading'));
      await tester.pump();
      await tester.tap(find.byTooltip('Bold'));
      await tester.pump();
      await tester.typeImeText('Q4');
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      expect(
        callsOf(fake, 'updateNote').single.args['content'],
        EditorFixtures.content.replaceFirst(
          'Three experiments',
          '# **Q4**Three experiments',
        ),
      );
    });

    testWidgets('checklist and mention buttons', (tester) async {
      final fake = await pumpEditor(tester, sizeClass: SizeClass.compact);
      final line = nodeIdOfLine(tester, 'Three');
      await tester.placeCaretInParagraph(line, 0);
      await tester.tap(find.byTooltip('Checklist item'));
      await tester.pump();
      await tester.tap(find.byTooltip('Mention a person or company'));
      await tester.pump();
      expect(find.byType(CompletionsPanel), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await tester.pump();
      expect(
        callsOf(fake, 'updateNote').single.args['content'],
        EditorFixtures.content.replaceFirst(
          'Three experiments',
          '- [ ] @Three experiments',
        ),
      );
    });

    testWidgets('undo reverts the last edit', (tester) async {
      await pumpEditor(tester, sizeClass: SizeClass.compact);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 0);
      await tester.typeImeText('Z');
      expect(find.text('Unsaved changes'), findsOneWidget);
      await tester.tap(find.byTooltip('Undo'));
      await tester.pump();
      expect(find.text('Saved'), findsOneWidget);
    });

    testWidgets('Open link forwards the wikilink under the caret', (
      tester,
    ) async {
      final opened = <String>[];
      await pumpEditor(
        tester,
        sizeClass: SizeClass.compact,
        onOpenLink: opened.add,
      );
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 55);
      await tester.pump();
      await tester.ensureVisible(find.byTooltip('Open link'));
      await tester.tap(find.byTooltip('Open link'));
      expect(opened, ['[[Subscription tiers]]']);
    });

    testWidgets('Resolve opens the conflict screen for the op', (tester) async {
      final opened = <String>[];
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(sync: EditorFixtures.conflict),
        onOpenConflict: opened.add,
      );
      await tester.tap(find.text('Resolve'));
      expect(opened, [EditorFixtures.conflictOpId]);
      expect(find.text('Conflict'), findsOneWidget);
    });

    testWidgets('Retry re-subscribes to the note', (tester) async {
      final fake = fakeWith(null);
      await pumpEditor(tester, fake: fake);
      fake.note[_id].addError(StrataFixtures.coreFailure);
      await tester.pump();
      await tester.tap(find.text('Retry'));
      await tester.pump();
      expect(callsOf(fake, 'watchNote'), hasLength(2));
    });
  });
}
