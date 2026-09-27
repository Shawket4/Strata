import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';

import 'support/editor_fixtures.dart';

void main() {
  testWidgets('smoke', (tester) async {
    final fake = FakeCoreApi();
    fake.note[EditorFixtures.noteId].add(EditorFixtures.screenOf(EditorFixtures.note));
    await pumpStrataScreen(tester, const NoteEditorScreen(noteId: EditorFixtures.noteId), fake: fake, sizeClass: SizeClass.expanded);
    await tester.pump();
    for (final t in tester.widgetList<TextComponent>(find.byType(TextComponent))) {
      final spans = t.text.getAttributionSpansInRange(attributionFilter: (_) => true, range: SpanRange(0, t.text.length)).map((s) => '${s.attribution.id}@${s.start}-${s.end}').toList();
      // ignore: avoid_print
      print('${t.text.toPlainText()} :: $spans');
    }
  });
}
