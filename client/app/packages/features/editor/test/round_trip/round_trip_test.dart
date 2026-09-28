// Round-trip golden tests of the editor's markdown source model (PLAN D2):
// every note of the corpus must come back byte-for-byte after load →
// serialise, and a scripted edit of one paragraph must change exactly that
// paragraph's bytes. The corpus files are the goldens (vault-format fixtures
// from crates/vault-format/tests/fixtures, copied as `vf_*.md`, plus Arabic
// and mixed notes, every task form, and edge cases).
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';
import 'package:super_editor/super_editor_test.dart';

import '../support/editor_fixtures.dart';

/// A corpus note: its bytes and decoded text.
final class CorpusNote {
  new(this.file)
    : bytes = file.readAsBytesSync(),
      name = file.uri.pathSegments.last;

  final File file;
  final List<int> bytes;
  final String name;

  /// The text the core streams: UTF-8 decoded, a leading byte-order mark
  /// kept as U+FEFF (`utf8.decode` would drop it).
  String get content {
    const bom = [0xEF, 0xBB, 0xBF];
    final hasBom =
        bytes.length >= 3 &&
        bytes[0] == bom[0] &&
        bytes[1] == bom[1] &&
        bytes[2] == bom[2];
    return hasBom
        ? '\uFEFF${utf8.decode(bytes.sublist(3))}'
        : utf8.decode(bytes);
  }

  List<EditorHint> get hints => frontmatterHint(content);

  /// Offset of the body in [content].
  int get bodyOffset => hints.isEmpty ? 0 : hints.first.end;

  /// Start offsets (in [content]) of the body's lines.
  List<int> get lineStarts {
    final starts = [bodyOffset];
    for (var i = bodyOffset; i < content.length; i++) {
      if (content.codeUnitAt(i) == 0x0A) starts.add(i + 1);
    }
    return starts;
  }

  /// End offset (before the terminator) of body line [index].
  int lineEnd(int index) {
    final starts = lineStarts;
    if (index == starts.length - 1) return content.length;
    var end = starts[index + 1] - 1;
    if (end > starts[index] && content.codeUnitAt(end - 1) == 0x0D) end--;
    return end;
  }
}

List<CorpusNote> loadCorpus() {
  final files =
      Directory('test/round_trip/corpus')
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.md'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  return [for (final f in files) CorpusNote(f)];
}

NoteView noteOf(CorpusNote note) => EditorFixtures.noteWith(
  content: note.content,
  hints: note.hints,
  tasks: const [],
);

NoteEditorController sessionOf(CorpusNote note) {
  final fake = FakeCoreApi()..editorHintsAnswer.returns(const []);
  addTearDown(fake.dispose);
  final controller = NoteEditorController(core: fake)..show(noteOf(note));
  addTearDown(controller.dispose);
  return controller;
}

DocumentPosition at(String nodeId, int offset) => DocumentPosition(
  nodeId: nodeId,
  nodePosition: TextNodePosition(offset: offset),
);

void main() {
  final corpus = loadCorpus();

  test('the corpus holds the vault-format fixtures and every added case', () {
    expect(corpus.map((n) => n.name), [
      'arabic_mixed.md',
      'bom.md',
      'code_blocks.md',
      'crlf_no_trailing_newline.md',
      'edge_whitespace.md',
      'embeds_and_links.md',
      'empty.md',
      'frontmatter_no_trailing_newline.md',
      'headings_lists.md',
      'only_frontmatter.md',
      'only_newline.md',
      'tasks_every_form.md',
      'vf_document_watanya.md',
      'vf_every_link_form.md',
      'vf_mixed_arabic_crlf.md',
      'vf_obsidian_messy.md',
      'vf_obsidian_messy_canonical.md',
      'vf_obsidian_messy_expected.md',
      'vf_person_ahmed.md',
      'vf_tasks.md',
    ]);
  });

  group('load → serialise is byte-exact', () {
    for (final note in corpus) {
      test(note.name, () {
        final source = MarkdownSource.parse(note.content, note.hints);
        final document = documentOf(source);
        final body = MarkdownSource.joinLines(linesOf(document), '\n');
        expect(utf8.encode('${source.frontmatter}$body'), note.bytes);
        expect(utf8.encode(source.content), note.bytes);
      });
    }
  });

  group('the frontmatter is kept out of the editor body', () {
    for (final note in corpus.where((n) => n.hints.isNotEmpty)) {
      test(note.name, () {
        final controller = sessionOf(note);
        final text = [
          for (final node in controller.document)
            (node as TextNode).text.toPlainText(),
        ];
        expect(text.first, isNot('---'));
        expect(
          controller.document.length,
          note.lineStarts.length,
          reason: 'one paragraph per body line',
        );
      });
    }
  });

  group('the editor session saves an unchanged note byte-exact', () {
    for (final note in corpus) {
      test(note.name, () {
        final controller = sessionOf(note);
        expect(utf8.encode(controller.content), note.bytes);
        expect(controller.isDirty, isFalse);
      });
    }
  });

  group('an edit of one paragraph changes only its bytes', () {
    for (final note in corpus) {
      final index = note.lineStarts.length ~/ 2;
      test('${note.name}: insert inside line $index', () {
        final controller = sessionOf(note);
        final node = controller.document.getNodeAt(index)!;
        final length = (node as TextNode).text.length;
        final offset = length ~/ 2;
        controller.editor!.execute([
          InsertTextRequest(
            documentPosition: at(node.id, offset),
            textToInsert: 'تعديل edit',
            attributions: const {},
          ),
        ]);
        final start = note.lineStarts[index] + offset;
        final expected =
            '${note.content.substring(0, start)}تعديل edit'
            '${note.content.substring(start)}';
        expect(utf8.encode(controller.content), utf8.encode(expected));
        expect(controller.isDirty, isTrue);
      });

      test('${note.name}: replace line $index', () {
        final controller = sessionOf(note);
        final node = controller.document.getNodeAt(index)! as TextNode;
        final length = node.text.length;
        controller.editor!.execute([
          if (length > 0)
            DeleteContentRequest(
              documentRange: DocumentRange(
                start: at(node.id, 0),
                end: at(node.id, length),
              ),
            ),
          InsertTextRequest(
            documentPosition: at(node.id, 0),
            textToInsert: '- [ ] New task 📅 2026-10-01 (@2026-10-01 09:00)',
            attributions: const {},
          ),
        ]);
        final expected =
            '${note.content.substring(0, note.lineStarts[index])}'
            '- [ ] New task 📅 2026-10-01 (@2026-10-01 09:00)'
            '${note.content.substring(note.lineEnd(index))}';
        expect(utf8.encode(controller.content), utf8.encode(expected));
      });

      test('${note.name}: split line $index with Enter', () {
        final controller = sessionOf(note);
        final node = controller.document.getNodeAt(index)! as TextNode;
        final offset = node.text.length ~/ 2;
        controller.editor!.execute([
          ChangeSelectionRequest(
            DocumentSelection.collapsed(position: at(node.id, offset)),
            SelectionChangeType.placeCaret,
            SelectionReason.userInteraction,
          ),
          InsertNewlineAtCaretRequest(),
        ]);
        final source = MarkdownSource.parse(note.content, note.hints);
        final start = note.lineStarts[index] + offset;
        final expected =
            '${note.content.substring(0, start)}${source.newline}'
            '${note.content.substring(start)}';
        expect(utf8.encode(controller.content), utf8.encode(expected));
      });
    }
  });

  test('merging two lines removes exactly the terminator between them', () {
    final note = corpus.firstWhere(
      (n) => n.name == 'crlf_no_trailing_newline.md',
    );
    final controller = sessionOf(note);
    final first = controller.document.getNodeAt(0)! as TextNode;
    final second = controller.document.getNodeAt(1)!;
    controller.editor!.execute([
      DeleteContentRequest(
        documentRange: DocumentRange(
          start: at(first.id, first.text.length),
          end: at(second.id, 0),
        ),
      ),
    ]);
    final end = note.lineEnd(0);
    final expected =
        '${note.content.substring(0, end)}'
        '${note.content.substring(note.lineStarts[1])}';
    expect(controller.content, expected);
  });

  test('a mixed-terminator note keeps untouched terminators on a split', () {
    const content = 'one\r\ntwo\nthree\r\nfour';
    final controller = sessionOf(
      CorpusNote(File('test/round_trip/corpus/empty.md')),
    )..show(EditorFixtures.noteWith(content: content, hints: const []));
    final two = controller.document.getNodeAt(1)! as TextNode;
    controller.editor!.execute([
      ChangeSelectionRequest(
        DocumentSelection.collapsed(position: at(two.id, 1)),
        SelectionChangeType.placeCaret,
        SelectionReason.userInteraction,
      ),
      InsertNewlineAtCaretRequest(),
    ]);
    // The new line uses the note's first terminator; `\n` after "two" and
    // `\r\n` after "three" are untouched.
    expect(controller.content, 'one\r\nt\r\nwo\nthree\r\nfour');
  });

  group('typing through the mounted editor saves exact bytes', () {
    // File and the body line typed into.
    for (final (name, index) in [
      ('vf_mixed_arabic_crlf.md', 3),
      ('tasks_every_form.md', 14),
      ('arabic_mixed.md', 2),
      ('arabic_mixed.md', 13),
      ('edge_whitespace.md', 4),
    ]) {
      testWidgets('$name line $index', (tester) async {
        final note = corpus.firstWhere((n) => n.name == name);
        final fake = FakeCoreApi()..editorHintsAnswer.returns(const []);
        fake.note[EditorFixtures.noteId].add(
          NoteScreen(id: EditorFixtures.noteId, note: noteOf(note)),
        );
        await pumpStrataScreen(
          tester,
          const NoteEditorScreen(noteId: EditorFixtures.noteId),
          fake: fake,
          sizeClass: SizeClass.expanded,
        );
        await tester.pump();
        final editor = tester
            .widget<SuperEditor>(find.byType(SuperEditor))
            .editor;
        final document = editor.document;
        final node = document.getNodeAt(index)! as TextNode;
        final offset = node.text.length ~/ 2;
        // Focus the editor by tapping, then put the caret at the exact offset
        // (a tap at a bidi run boundary is visually ambiguous).
        await tester.placeCaretInParagraph(document.last.id, 0);
        editor.execute([
          ChangeSelectionRequest(
            DocumentSelection.collapsed(position: at(node.id, offset)),
            SelectionChangeType.placeCaret,
            SelectionReason.userInteraction,
          ),
        ]);
        await tester.pump();
        await tester.typeImeText(' نص typed ');
        await tester.tap(find.widgetWithText(FilledButton, 'Save'));
        await tester.pump();
        final start = note.lineStarts[index] + offset;
        final expected =
            '${note.content.substring(0, start)} نص typed '
            '${note.content.substring(start)}';
        final saves = fake.calls.where((c) => c.method == 'updateNote');
        expect(saves, [
          CoreCall('updateNote', {
            'id': EditorFixtures.noteId,
            'content': expected,
            'baseVersion': EditorFixtures.contentVersion,
          }),
        ]);
        expect(
          utf8.encode(saves.single.args['content']! as String),
          utf8.encode(expected),
        );
      });
    }
  });
}
