@Tags(['golden'])
library;

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_notes/strata_notes.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../support/notes_fixtures.dart';

FakeCoreApi _fake({NoteView? note}) {
  final fake = FakeCoreApi()..editorHintsAnswer.returns(const []);
  fake.notesList['notes/sales'].add(NotesFixtures.sales);
  fake.note[NotesFixtures.pricingId].add(
    NoteScreen(
      id: NotesFixtures.pricingId,
      note: note ?? NotesFixtures.pricingNote(),
    ),
  );
  fake.note[NotesFixtures.arabicId].add(
    NoteScreen(id: NotesFixtures.arabicId, note: NotesFixtures.arabicNote),
  );
  return fake;
}

/// Opens the medium context drawer (NoteMedium shows it open).
Future<void> _openDrawer(WidgetTester tester) async {
  await tester.pumpAndSettle();
  final toggle = find.byIcon(Icons.view_sidebar_outlined);
  if (toggle.evaluate().isNotEmpty) {
    await tester.tap(toggle);
    await tester.pumpAndSettle();
  }
}

/// The notes screen (list only, and with a note open) × size class × theme
/// × direction at text scale 1.0 and compact at 2.0; NoteExpandedDarkAr;
/// the conflict hand-off.
void main() {
  setUpAll(loadStrataFonts);

  group('notes goldens', () {
    for (final v in goldenVariants()) {
      unawaited(
        goldenTest(
          'notes list $v',
          fileName: 'notes_list_${v.id}',
          constraints: BoxConstraints.tight(v.size),
          builder: () => goldenFrame(
            v,
            const NotesScreen(folder: 'notes/sales'),
            fake: _fake(),
          ),
        ),
      );
      unawaited(
        goldenTest(
          'note $v',
          fileName: 'note_${v.id}',
          constraints: BoxConstraints.tight(v.size),
          pumpBeforeTest: v.sizeClass == SizeClass.medium
              ? _openDrawer
              : onlyPumpAndSettle,
          builder: () => goldenFrame(
            v,
            NotesScreen(
              folder: 'notes/sales',
              selectedNoteId: NotesFixtures.pricingId,
              onOpenNote: (_) {},
              onOpenLocalMap: (_) {},
              onOpenConflict: (_) {},
            ),
            fake: _fake(),
          ),
        ),
      );
    }

    const darkAr = Variant(
      sizeName: 'expanded',
      size: StrataTestSizes.expanded,
      brightness: Brightness.dark,
      locale: StrataLocales.english,
      textScale: 1,
    );
    unawaited(
      goldenTest(
        'NoteExpandedDarkAr',
        fileName: 'note_arabic_content_${darkAr.id}',
        constraints: BoxConstraints.tight(darkAr.size),
        builder: () => goldenFrame(
          darkAr,
          NotesScreen(
            folder: 'notes/sales',
            selectedNoteId: NotesFixtures.arabicId,
            onOpenNote: (_) {},
            onOpenLocalMap: (_) {},
          ),
          fake: _fake(),
        ),
      ),
    );

    const conflict = Variant(
      sizeName: 'expanded',
      size: StrataTestSizes.expanded,
      brightness: Brightness.light,
      locale: StrataLocales.english,
      textScale: 1,
    );
    unawaited(
      goldenTest(
        'note conflict hand-off',
        fileName: 'note_conflict_${conflict.id}',
        constraints: BoxConstraints.tight(conflict.size),
        builder: () => goldenFrame(
          conflict,
          NotesScreen(
            folder: 'notes/sales',
            selectedNoteId: NotesFixtures.pricingId,
            onOpenConflict: (_) {},
            onOpenLocalMap: (_) {},
          ),
          fake: _fake(
            note: NotesFixtures.pricingNote(sync: NotesFixtures.conflictSync),
          ),
        ),
      ),
    );
  });
}
