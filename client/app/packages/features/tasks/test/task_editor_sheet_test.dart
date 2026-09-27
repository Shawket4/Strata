import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';

import 'helpers/harness.dart';

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

void main() {
  group('TaskEditorSheet', () {
    for (final v in matrix()) {
      testWidgets('creates a task from the typed text [$v]', (tester) async {
        final created = <String>[];
        final fake = await pumpVariant(
          tester,
          v,
          TaskEditorSheet(onCreated: created.add),
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.editorNewTaskTitle), findsOneWidget);
        expect(find.text(s.editorParsedUnavailable), findsOneWidget);
        expect(find.text(s.editorDefaultHome), findsOneWidget);
        await expectAccessible(tester);
        await tester.enterText(
          find.byType(TextField).first,
          'Send weekly invoicing proposal to Ahmed',
        );
        await tester.enterText(
          find.byType(TextField).last,
          'every week on Tuesday',
        );
        await tester.pump();
        await _tap(tester, find.text(s.commonSave));
        expect(
          fake.calls,
          contains(
            const CoreCall('createTask', {
              'draft': TaskDraft(
                description: 'Send weekly invoicing proposal to Ahmed',
                recurrence: 'every week on Tuesday',
                reminders: [],
              ),
              'force': false,
            }),
          ),
        );
        expect(created, ['n-weekly-invoicing-request']);
        expectNoErrors(tester);
      });

      testWidgets('duplicate check: candidates and Create anyway [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..createTaskAnswer.returns(StrataFixtures.createOutcomeDuplicate);
        final opened = <CandidateItem>[];
        await pumpVariant(
          tester,
          v,
          TaskEditorSheet(noteId: 'n-tasks', onOpenExisting: opened.add),
          fake: fake,
        );
        final s = lookupTasksLocalizations(v.locale);
        await tester.enterText(
          find.byType(TextField).first,
          "remind me to make watanya's invoice",
        );
        await tester.pump();
        await _tap(tester, find.text(s.commonSave));
        expect(find.byType(DuplicateCandidatesView), findsOneWidget);
        expect(find.text("Make Watanya's ETA invoice"), findsOneWidget);
        expect(
          find.text(s.dupMatch(level: s.dupMatchNear, score: '0.91')),
          findsOneWidget,
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
        await _tap(tester, find.text(s.dupCreateAnyway));
        expect(
          fake.calls,
          contains(
            const CoreCall('createTask', {
              'draft': TaskDraft(
                noteId: 'n-tasks',
                description: "remind me to make watanya's invoice",
                reminders: [],
              ),
              'force': true,
            }),
          ),
        );
      });
    }

    testWidgets('show() is a bottom sheet on compact and a dialog otherwise', (
      tester,
    ) async {
      for (final v in [matrix().first, matrix().last]) {
        await tester.pumpWidget(const SizedBox());
        await pumpVariant(
          tester,
          v,
          Builder(
            builder: (context) => TextButton(
              onPressed: () => TaskEditorSheet.show(context),
              child: const Text('open'),
            ),
          ),
        );
        await _tap(tester, find.text('open'));
        expect(find.byType(TaskEditorSheet), findsOneWidget);
        expect(
          find.byType(v.sizeName == 'compact' ? BottomSheet : Dialog),
          findsOneWidget,
        );
      }
    });
  });
}
