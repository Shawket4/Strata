@Tags(['golden'])
library;

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../support/editor_fixtures.dart';
import '../support/harness.dart';

/// The note editor screen (content, conflict) × size class × theme ×
/// direction at text scale 1.0, and compact at 2.0.
void main() {
  setUpAll(loadStrataFonts);

  FakeCoreApi fakeWith(NoteEditorFixture fixture) {
    final fake = FakeCoreApi();
    fake.note[EditorFixtures.noteId].add(EditorFixtures.screenOf(fixture.note));
    return fake;
  }

  group('note editor goldens', () {
    for (final fixture in NoteEditorFixture.values) {
      for (final v in goldenVariants()) {
        unawaited(
          goldenTest(
            'note editor ${fixture.name} $v',
            fileName: 'note_editor_${fixture.name}_${v.id}',
            builder: () => goldenFrame(
              v,
              fakeWith(fixture),
              NoteEditorScreen(
                noteId: EditorFixtures.noteId,
                onOpenConflict: (_) {},
                onOpenLink: (_) {},
              ),
            ),
          ),
        );
      }
    }
  });
}

/// The states rendered in goldens.
enum NoteEditorFixture {
  content,
  conflict;

  NoteView get note => switch (this) {
    content => EditorFixtures.note,
    conflict => EditorFixtures.noteWith(sync: EditorFixtures.conflict),
  };
}
