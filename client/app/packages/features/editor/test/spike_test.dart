import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:super_editor/super_editor.dart';
import 'package:super_editor/super_editor_test.dart';

void main() {
  testWidgets('spike', (tester) async {
    final doc = MutableDocument(nodes: [
      ParagraphNode(id: 'a', text: AttributedText('# Title [[Note]]')),
      ParagraphNode(id: 'b', text: AttributedText('كلمت أحمد invoicing')),
    ]);
    final composer = MutableDocumentComposer();
    final editor = Editor(
      editables: {Editor.documentKey: doc, Editor.composerKey: composer},
      requestHandlers: List.from(defaultRequestHandlers),
      reactionPipeline: [],
    );
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SuperEditor(editor: editor, autofocus: true))));
    await tester.pumpAndSettle();
    expect(find.byType(SuperEditor), findsOneWidget);
    await tester.placeCaretInParagraph('b', 3);
    await tester.typeImeText('XY');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);await tester.pump();
    await tester.typeImeText('Z');
    await tester.pumpAndSettle();
    print(doc.toList().map((n) => (n as TextNode).text.toPlainText()).toList());
    print(composer.selection);
  });
}
