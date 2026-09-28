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
  OpenNoteAt? onOpenLink,
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
        await pumpVariant(
          tester,
          v,
          const NoteEditorScreen(noteId: _id),
          fake: fake,
        );
        expect(find.text('Pricing experiments'), findsOneWidget);
        // The status line is the core's ready label; the version too.
        expect(find.text('Saved'), findsOneWidget);
        expect(find.text('v7'), findsOneWidget);
        expect(find.byType(SuperEditor), findsOneWidget);
        expect(
          find.byType(FormattingToolbar),
          v.sizeClass == SizeClass.compact ? findsOneWidget : findsNothing,
        );
        expect(find.byType(NoteConflictBanner), findsNothing);
        expect(fake.calls, contains(const CoreCall('watchNote', {'id': _id})));
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('conflict $v', (tester) async {
        final fake = fakeWith(
          EditorFixtures.noteWith(sync: EditorFixtures.conflict),
        );
        await pumpVariant(
          tester,
          v,
          const NoteEditorScreen(noteId: _id),
          fake: fake,
        );
        expect(find.byType(NoteConflictBanner), findsOneWidget);
        expect(
          find.text(
            v.rtl
                ? 'الملاحظة دي اتعدلت كمان على السيرفر'
                : 'This note was also changed on the server',
          ),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('loading, not found and error $v', (tester) async {
        final fake = fakeWith(null);
        await pumpVariant(
          tester,
          v,
          const NoteEditorScreen(noteId: _id),
          fake: fake,
        );
        expect(find.byType(CircularProgressIndicator), findsOneWidget);
        fake.note[_id].add(const NoteScreen(id: _id));
        await tester.pump();
        expect(
          find.text(v.rtl ? 'الملاحظة مش موجودة' : 'Note not found'),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
        fake.note[_id].addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(
          find.text(
            v.rtl ? 'مقدرناش نفتح الملاحظة دي' : "Couldn't open this note",
          ),
          findsOneWidget,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('rendering', () {
    testWidgets('styles hints and shows the note status', (tester) async {
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(sync: EditorFixtures.pending),
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

    testWidgets('lays out each line in the direction the core sent', (
      tester,
    ) async {
      const content =
          'English first paragraph.\n'
          'فقرة عربية بعدها [[Acme Logistics]] mixed.\n'
          '## الفرضيات\n'
          '---\n';
      await pumpEditor(
        tester,
        note: EditorFixtures.noteWith(
          content: content,
          hints: lineDirections(content, const {
            'English first paragraph.': TextDir.ltr,
            'فقرة عربية بعدها [[Acme Logistics]] mixed.': TextDir.rtl,
            '## الفرضيات': TextDir.rtl,
          }),
        ),
      );
      final components = tester
          .widgetList<TextComponent>(find.byType(TextComponent))
          .toList();
      expect(
        [for (final c in components) c.textDirection],
        [
          TextDirection.ltr,
          TextDirection.rtl,
          TextDirection.rtl,
          // No strong character: no direction hint, laid out left to right.
          TextDirection.ltr,
          TextDirection.ltr,
        ],
      );
      expect(
        [for (final c in components) c.textAlign],
        [
          TextAlign.left,
          TextAlign.right,
          TextAlign.right,
          TextAlign.left,
          TextAlign.left,
        ],
      );
    });

    testWidgets('live preview hides markers off the caret line', (
      tester,
    ) async {
      await pumpEditor(tester);
      bool hidden(String line, int offset) {
        final component = tester
            .widgetList<TextComponent>(find.byType(TextComponent))
            .firstWhere((c) => c.text.toPlainText().startsWith(line));
        return component.text
            .getAllAttributionsAt(offset)
            .contains(hiddenMarkerAttribution);
      }

      const bold = '- Launch offer: a ';
      // No caret: the bold markers and the heading's "## " are hidden, the
      // bold text and heading text are not.
      expect(hidden(bold, bold.length), isTrue);
      expect(hidden(bold, bold.length + 1), isTrue);
      expect(hidden(bold, bold.length + 2), isFalse);
      expect(hidden('## Hypotheses', 0), isTrue);
      expect(hidden('## Hypotheses', 2), isTrue);
      expect(hidden('## Hypotheses', 3), isFalse);
      expect(hidden('Three experiments', 50), isTrue, reason: '[[');
      expect(hidden('Three experiments', 52), isFalse);
      // The caret's line shows its source.
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Hyp'), 5);
      await tester.pump();
      expect(hidden('## Hypotheses', 0), isFalse);
      expect(hidden(bold, bold.length), isTrue);
      // Preview off: every marker shows.
      await tester.tap(find.byTooltip('Show markdown'));
      await tester.pump();
      expect(hidden(bold, bold.length), isFalse);
      expect(find.byTooltip('Hide markdown'), findsOneWidget);
    });

    test("markers are the core's exact ranges, placed on their lines", () {
      // `***both***` (an emphasis around a strong run), a heading with extra
      // spaces and closing hashes, and a setext heading whose underline is on
      // its second line: no width is derived from the kind.
      const content = '***both***\n###   Three ##\nSetext\n===\n';
      final source = MarkdownSource.parse(content, const []);
      final document = documentOf(source);
      EditorHint hint(HintKind kind, int start, int end, List<(int, int)> m) =>
          EditorHint(
            kind: kind,
            start: start,
            end: end,
            level: 0,
            markers: [for (final (s, e) in m) MarkerRange(start: s, end: e)],
          );
      final hints = LineHints.place(
        document: document,
        lines: linesOf(document),
        newline: '\n',
        bodyOffset: 0,
        hints: [
          hint(HintKind.italic, 0, 10, [(0, 1), (9, 10)]),
          hint(HintKind.bold, 1, 9, [(1, 3), (7, 9)]),
          hint(HintKind.heading, 11, 25, [(11, 17), (22, 25)]),
          hint(HintKind.heading, 26, 36, [(33, 36)]),
        ],
      );
      final ids = [for (final node in document) node.id];
      List<List<(int, int)>> markersOf(int line) => [
        for (final span in hints.of(ids[line])) span.markers,
      ];
      expect(markersOf(0), [
        [(0, 1), (9, 10)],
        [(1, 3), (7, 9)],
      ]);
      expect(markersOf(1), [
        [(0, 6), (11, 14)],
      ]);
      // The setext heading spans two lines; only the second has a marker.
      expect(markersOf(2), [<(int, int)>[]]);
      expect(markersOf(3), [
        [(0, 3)],
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
        CoreCall('updateNote', {
          'id': _id,
          'content': expected,
          'baseVersion': EditorFixtures.contentVersion,
        }),
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
          'baseVersion': EditorFixtures.contentVersion,
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
                  level: 0,
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

  group('completions', () {
    Completions completions(
      CompletionKind kind,
      int start,
      int end,
      List<CompletionItem> items,
    ) => Completions(
      kind: kind,
      replaceStart: start,
      replaceEnd: end,
      query: '',
      items: items,
    );

    final nextSteps = EditorFixtures.content.indexOf('## Next steps') + 13;

    testWidgets('each edit asks the core at the caret', (tester) async {
      final fake = await pumpEditor(tester);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' @Ah');
      await tester.pump();
      expect(
        callsOf(fake, 'editorCompletions').last,
        CoreCall('editorCompletions', {
          'noteId': _id,
          'content': EditorFixtures.content.replaceFirst(
            '## Next steps',
            '## Next steps @Ah',
          ),
          'cursor': nextSteps + 4,
        }),
      );
      // The core found nothing to complete: no popup.
      expect(find.byType(CompletionsPopup), findsNothing);
    });

    testWidgets('@ shows the core items and inserts the mention', (
      tester,
    ) async {
      final mentioned = EditorFixtures.content.replaceFirst(
        '## Next steps',
        '## Next steps with [[Ahmed Samir]]',
      );
      final fake = fakeWith(EditorFixtures.note)
        ..editorCompletionsAnswer.returns(
          completions(CompletionKind.mention, nextSteps + 6, nextSteps + 9, [
            const CompletionItem(
              label: 'Ahmed Samir',
              detail: 'Operations manager, Acme Logistics',
              insertText: '[[Ahmed Samir]]',
              targetId: 'p-ahmed-samir',
              entityKind: 'person',
              labelDir: TextDir.ltr,
            ),
            const CompletionItem(
              label: 'أحمد فتحي',
              detail: 'Nile Freight',
              insertText: '[[أحمد فتحي]]',
              targetId: 'p-ahmed-fathy',
              entityKind: 'person',
              labelDir: TextDir.rtl,
            ),
          ]),
        )
        ..insertMentionAnswer.returns(
          MentionEdit(content: mentioned, cursor: nextSteps + 20),
        );
      await pumpEditor(tester, fake: fake, sizeClass: SizeClass.compact);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' with @Ah');
      await tester.pump();
      expect(find.byType(CompletionsPopup), findsOneWidget);
      expect(find.text('People and companies'), findsOneWidget);
      expect(find.text('Operations manager, Acme Logistics'), findsOneWidget);
      expect(
        tester.widget<Text>(find.text('أحمد فتحي')).textDirection,
        TextDirection.rtl,
      );
      await tester.tap(find.text('Ahmed Samir'));
      await tester.pump();
      await tester.pump();
      expect(callsOf(fake, 'insertMention'), [
        CoreCall('insertMention', {
          'noteId': _id,
          'content': EditorFixtures.content.replaceFirst(
            '## Next steps',
            '## Next steps with @Ah',
          ),
          'start': nextSteps + 6,
          'end': nextSteps + 9,
          'entityId': 'p-ahmed-samir',
        }),
      ]);
      // The core's content (link + people: entry) is saved at once.
      expect(callsOf(fake, 'updateNote'), [
        CoreCall('updateNote', {
          'id': _id,
          'content': mentioned,
          'baseVersion': EditorFixtures.contentVersion,
        }),
      ]);
      expect(find.byType(CompletionsPopup), findsNothing);
    });

    testWidgets('[[ replaces the typed range with the core insert text', (
      tester,
    ) async {
      final fake = fakeWith(EditorFixtures.note)
        ..editorCompletionsAnswer.returns(
          completions(CompletionKind.wikiLink, nextSteps + 5, nextSteps + 10, [
            const CompletionItem(
              label: 'Churn notes',
              detail: 'notes/sales/Churn notes.md',
              insertText: '[[Churn notes]]',
              targetId: 'n-churn-notes',
              labelDir: TextDir.ltr,
            ),
          ]),
        );
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' see [[Chu');
      await tester.pump();
      expect(find.text('Notes'), findsOneWidget);
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

    for (final (kind, heading) in [
      (CompletionKind.tag, 'Tags'),
      (CompletionKind.blockRef, 'Blocks'),
    ]) {
      testWidgets('$kind: heading, items, dismiss', (tester) async {
        final fake = fakeWith(EditorFixtures.note)
          ..editorCompletionsAnswer.returns(
            completions(kind, nextSteps + 1, nextSteps + 5, [
              const CompletionItem(
                label: 'pricing',
                detail: '12',
                insertText: '#pricing',
                labelDir: TextDir.ltr,
              ),
            ]),
          );
        await pumpEditor(tester, fake: fake);
        await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
        await tester.typeImeText(' #pri');
        await tester.pump();
        expect(find.text(heading), findsOneWidget);
        expect(find.text('pricing'), findsOneWidget);
        await tester.tap(find.byTooltip('Dismiss suggestions'));
        await tester.pump();
        expect(find.byType(CompletionsPopup), findsNothing);
      });
    }

    testWidgets('no matches', (tester) async {
      final fake = fakeWith(EditorFixtures.note)
        ..editorCompletionsAnswer.returns(
          completions(CompletionKind.wikiLink, 0, 0, const []),
        );
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' [[zz');
      await tester.pump();
      expect(find.text('No matches'), findsOneWidget);
    });

    testWidgets('Escape closes the completions', (tester) async {
      final fake = fakeWith(EditorFixtures.note)
        ..editorCompletionsAnswer.returns(
          completions(CompletionKind.wikiLink, 0, 0, const []),
        );
      await pumpEditor(tester, fake: fake);
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, '## Next'), 13);
      await tester.typeImeText(' [[zz');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byType(CompletionsPopup), findsNothing);
    });

    test('glyphs map the completion and entity kinds', () {
      expect(
        completionGlyph(CompletionKind.mention, 'company'),
        NodeKind.company,
      );
      expect(completionGlyph(CompletionKind.mention, null), NodeKind.person);
      expect(completionGlyph(CompletionKind.tag, null), NodeKind.concept);
      expect(completionGlyph(CompletionKind.blockRef, null), NodeKind.note);
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
      expect(callsOf(fake, 'editorCompletions'), isNotEmpty);
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

    testWidgets('Open link opens the target the core resolved', (tester) async {
      final opened = <(String, String?)>[];
      await pumpEditor(
        tester,
        sizeClass: SizeClass.compact,
        onOpenLink: (id, anchor) => opened.add((id, anchor)),
      );
      await tester.placeCaretInParagraph(nodeIdOfLine(tester, 'Three'), 55);
      await tester.pump();
      await tester.ensureVisible(find.byTooltip('Open link'));
      await tester.tap(find.byTooltip('Open link'));
      expect(opened, [('n-subscription-tiers', null)]);
    });

    testWidgets('a link to a block opens at its anchor', (tester) async {
      const content = 'See [[Churn notes#^a1b2]] and [[Nowhere]].\n';
      final opened = <(String, String?)>[];
      await pumpEditor(
        tester,
        sizeClass: SizeClass.compact,
        note: EditorFixtures.noteWith(
          content: content,
          hints: [
            hintOf(
              content,
              HintKind.wikiLink,
              '[[Churn notes#^a1b2]]',
              targetId: 'n-churn-notes',
              targetAnchor: 'a1b2',
            ),
            hintOf(content, HintKind.wikiLink, '[[Nowhere]]'),
          ],
        ),
        onOpenLink: (id, anchor) => opened.add((id, anchor)),
      );
      final line = nodeIdOfLine(tester, 'See');
      await tester.placeCaretInParagraph(line, 8);
      await tester.pump();
      await tester.tap(find.byTooltip('Open link'));
      expect(opened, [('n-churn-notes', 'a1b2')]);
      // An unresolved link offers nothing to open.
      await tester.placeCaretInParagraph(line, 33);
      await tester.pump();
      expect(find.byTooltip('Open link'), findsNothing);
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
