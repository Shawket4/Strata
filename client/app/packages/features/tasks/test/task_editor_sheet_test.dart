import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';

Future<void> _tap(WidgetTester tester, Finder finder) =>
    tapVisible(tester, finder.first);

/// The core's reading of [text]: the text itself, nothing understood.
TaskDraftPreview _plain(String text) => TaskDraftPreview(
  description: text,
  descriptionDir: TextDir.ltr,
  reminders: const [],
  links: const [],
  chips: const [],
  draft: TaskDraft(description: text, reminders: const []),
);

void main() {
  group('TaskEditorSheet', () {
    for (final v in variants()) {
      testWidgets('creates a task from the typed text [$v]', (tester) async {
        final created = <String>[];
        final fake = FakeCoreApi()..taskHomes.add(StrataFixtures.taskHomesView);
        await pumpVariant(
          tester,
          v,
          TaskEditorSheet(onCreated: created.add),
          fake: fake,
          scaffold: true,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.editorNewTaskTitle), findsOneWidget);
        expect(find.text(s.editorParsedNothing), findsOneWidget);
        expect(
          find.text(s.editorHomeItem(title: 'Tasks', count: 3)),
          findsOneWidget,
        );
        await expectAccessible(tester, contrast: v.textScale == 1);
        await tester.enterText(
          find.byType(TextField).first,
          'Send weekly invoicing proposal to @Ahmed by Tuesday',
        );
        await settle(tester);
        // What the core understood, as chips.
        expect(
          fake.calls,
          contains(
            const CoreCall('parseTaskText', {
              'text': 'Send weekly invoicing proposal to @Ahmed by Tuesday',
            }),
          ),
        );
        expect(find.text('Due Tue 29 Sep'), findsOneWidget);
        expect(find.text('Ahmed Samir'), findsOneWidget);
        await tester.enterText(
          find.byType(TextField).at(1),
          'every week on Tuesday',
        );
        await settle(tester);
        await _tap(tester, find.text(s.commonSave));
        expect(
          fake.calls.last,
          CoreCall('createTask', {
            'draft': TaskDraft(
              description: 'Send weekly invoicing proposal to Ahmed',
              due: DateTime.utc(2026, 9, 29),
              recurrence: 'every week on Tuesday',
              reminders: const [],
            ),
            'force': false,
          }),
        );
        expect(created, ['n-weekly-invoicing-request']);
        expectNoErrors(tester);
      });

      testWidgets('duplicate check: candidates and Create anyway [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..createTaskAnswer.returns(StrataFixtures.createOutcomeDuplicate)
          ..parseTaskTextAnswer.returns(
            _plain("remind me to make watanya's invoice"),
          );
        final opened = <CandidateItem>[];
        await pumpVariant(
          tester,
          v,
          TaskEditorSheet(noteId: 'n-tasks', onOpenExisting: opened.add),
          fake: fake,
          scaffold: true,
        );
        final s = lookupTasksLocalizations(v.locale);
        await tester.enterText(
          find.byType(TextField).first,
          "remind me to make watanya's invoice",
        );
        await settle(tester);
        await _tap(tester, find.text(s.commonSave));
        expect(find.byType(DuplicateCandidatesView), findsOneWidget);
        expect(find.text('notes/Tasks.md'), findsOneWidget);
        expect(find.text('Same wording as an open task'), findsOneWidget);
        expect(find.text("Make Watanya's ETA invoice"), findsOneWidget);
        expect(
          find.text(s.dupMatch(level: s.dupMatchNear, score: '0.91')),
          findsOneWidget,
        );
        await expectAccessible(tester, contrast: v.textScale == 1);
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

    testWidgets('the task goes to the chosen home note', (tester) async {
      final fake = FakeCoreApi()
        ..taskHomes.add(StrataFixtures.taskHomesView)
        ..parseTaskTextAnswer.returns(_plain('Call Ahmed back'));
      await pumpVariant(
        tester,
        variants().first,
        const TaskEditorSheet(),
        fake: fake,
        scaffold: true,
      );
      await tester.enterText(find.byType(TextField).first, 'Call Ahmed back');
      await settle(tester);
      await _tap(tester, find.text('Tasks · 3 open'));
      await _tap(tester, find.text('Call 2026-09-12 — Acme · 1 open').last);
      await _tap(tester, find.text('Save'));
      expect(
        fake.calls.last,
        const CoreCall('createTask', {
          'draft': TaskDraft(
            noteId: 'n-call-2026-09-12-acme',
            description: 'Call Ahmed back',
            reminders: [],
          ),
          'force': false,
        }),
      );
    });

    testWidgets('show() is a bottom sheet on compact and a dialog otherwise', (
      tester,
    ) async {
      for (final v in [variants().first, variants().last]) {
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
          scaffold: true,
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
